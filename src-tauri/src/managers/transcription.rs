//! Transcription engine load/unload + idle eviction + transcribe.
//!
//! Ported from Handy (`managers/transcription.rs`), stripped to macOS + the two
//! engines in Stenographer's catalog (Whisper via whisper.cpp, Parakeet via
//! ONNX). Verified against the installed `transcribe-rs` 0.3.11 public API.
//!
//! Divergences from Handy (0.3.3 there, 0.3.11 here):
//! - Catalog reduced to Whisper + Parakeet only, so `LoadedEngine` has two
//!   variants. The 0.3.11 API for both (`WhisperEngine::load`,
//!   `ParakeetModel::load(dir, &Quantization)`, `transcribe_with(&[f32], &params)`)
//!   matches Handy's usage.
//! - Stenographer's `settings` has no `custom_words`, `app_language`,
//!   `custom_filler_words`, or accelerator fields, and `selected_model` is
//!   `Option<String>`. Post-processing (custom words / filler filtering) and the
//!   `apply_accelerator_settings` helper from Handy are therefore omitted.
//! - `ModelUnloadTimeout` here has no `to_seconds()` method, so the idle limit is
//!   computed inline.
//! - The idle watcher does not consult an `AudioRecordingManager` (that module is
//!   still a stub owned by another agent). Activity is refreshed on every
//!   `transcribe()` call, and the `Immediately` variant is skipped by the timed
//!   watcher, so the model is not unloaded mid-session in practice.

#![allow(dead_code)]

use crate::managers::model::{EngineType, ModelManager};
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout};
use anyhow::Result;
use log::{debug, error, info, warn};
use serde::Serialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};
use transcribe_rs::{
    onnx::{
        parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity},
        Quantization,
    },
    whisper_cpp::{WhisperEngine, WhisperInferenceParams},
};

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
}

/// RAII guard that clears the `is_loading` flag and notifies waiters on drop.
pub struct LoadingGuard {
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        let mut is_loading = self.is_loading.lock().unwrap();
        *is_loading = false;
        self.loading_condvar.notify_all();
    }
}

#[derive(Clone)]
pub struct TranscriptionManager {
    engine: Arc<Mutex<Option<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    app_handle: AppHandle,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    shutdown_signal: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl TranscriptionManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Result<Self> {
        let manager = Self {
            engine: Arc::new(Mutex::new(None)),
            model_manager,
            app_handle: app_handle.clone(),
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(Self::now_ms())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
        };

        // Start the idle watcher.
        {
            let app_handle_cloned = app_handle.clone();
            let manager_cloned = manager.clone();
            let shutdown_signal = manager.shutdown_signal.clone();
            let handle = thread::spawn(move || {
                debug!("Idle watcher thread started");
                while !shutdown_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10));

                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let settings = get_settings(&app_handle_cloned);
                    let timeout = settings.model_unload_timeout;

                    // `Immediately` is handled by maybe_unload_immediately() after
                    // each transcription; treating it as 0s here would unload the
                    // model mid-recording.
                    if timeout == ModelUnloadTimeout::Immediately {
                        continue;
                    }

                    if let Some(limit_seconds) = Self::timeout_to_seconds(timeout) {
                        let last = manager_cloned.last_activity.load(Ordering::Relaxed);
                        let now_ms = Self::now_ms();
                        let idle_ms = now_ms.saturating_sub(last);
                        let limit_ms = limit_seconds * 1000;

                        if idle_ms > limit_ms && manager_cloned.is_model_loaded() {
                            let unload_start = std::time::Instant::now();
                            info!(
                                "Model idle for {}s (limit: {}s), unloading",
                                idle_ms / 1000,
                                limit_seconds
                            );
                            match manager_cloned.unload_model() {
                                Ok(()) => info!(
                                    "Model unloaded due to inactivity (took {}ms)",
                                    unload_start.elapsed().as_millis()
                                ),
                                Err(e) => error!("Failed to unload idle model: {}", e),
                            }
                        }
                    }
                }
                debug!("Idle watcher thread shutting down gracefully");
            });
            *manager.watcher_handle.lock().unwrap() = Some(handle);
        }

        Ok(manager)
    }

    /// Convert an unload-timeout setting to a concrete idle limit in seconds.
    /// `Never` and `Immediately` return `None` (the timed watcher skips them).
    fn timeout_to_seconds(timeout: ModelUnloadTimeout) -> Option<u64> {
        match timeout {
            ModelUnloadTimeout::Never | ModelUnloadTimeout::Immediately => None,
            ModelUnloadTimeout::Seconds(s) => Some(s),
            ModelUnloadTimeout::Minutes(m) => Some(m * 60),
        }
    }

    /// Lock the engine mutex, recovering from poison if a previous transcription panicked.
    fn lock_engine(&self) -> MutexGuard<'_, Option<LoadedEngine>> {
        self.engine.lock().unwrap_or_else(|poisoned| {
            warn!("Engine mutex was poisoned by a previous panic, recovering");
            poisoned.into_inner()
        })
    }

    pub fn is_model_loaded(&self) -> bool {
        self.lock_engine().is_some()
    }

    /// Atomically claim the loading slot. Returns `None` if a load is already in
    /// progress; otherwise a [`LoadingGuard`] whose drop clears the flag.
    pub fn try_start_loading(&self) -> Option<LoadingGuard> {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading {
            return None;
        }
        *is_loading = true;
        Some(LoadingGuard {
            is_loading: self.is_loading.clone(),
            loading_condvar: self.loading_condvar.clone(),
        })
    }

    pub fn unload_model(&self) -> Result<()> {
        let unload_start = std::time::Instant::now();
        debug!("Starting to unload model");

        {
            let mut engine = self.lock_engine();
            *engine = None; // Dropping the engine frees all resources.
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = None;
        }

        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "unloaded".to_string(),
                model_id: None,
                model_name: None,
                error: None,
            },
        );

        debug!(
            "Model unloaded (took {}ms)",
            unload_start.elapsed().as_millis()
        );
        Ok(())
    }

    /// Contract alias for [`unload_model`](Self::unload_model).
    pub fn unload(&self) {
        if let Err(e) = self.unload_model() {
            warn!("unload() failed: {}", e);
        }
    }

    /// Apply a new unload timeout by persisting it to settings. The idle watcher
    /// reads the setting on its next tick.
    pub fn set_unload_timeout(&self, timeout: ModelUnloadTimeout) {
        let mut settings = get_settings(&self.app_handle);
        settings.model_unload_timeout = timeout;
        write_settings(&self.app_handle, &settings);
        // Refresh activity so a freshly shortened timeout doesn't unload instantly.
        self.touch_activity();
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn touch_activity(&self) {
        self.last_activity.store(Self::now_ms(), Ordering::Relaxed);
    }

    /// Unload immediately if the setting is `Immediately` and a model is loaded.
    pub fn maybe_unload_immediately(&self, context: &str) {
        let settings = get_settings(&self.app_handle);
        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately
            && self.is_model_loaded()
        {
            info!("Immediately unloading model after {}", context);
            if let Err(e) = self.unload_model() {
                warn!("Failed to immediately unload model: {}", e);
            }
        }
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        let load_start = std::time::Instant::now();
        debug!("Starting to load model: {}", model_id);

        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_started".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: None,
                error: None,
            },
        );

        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        let emit_loading_failed = |error_msg: &str| {
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
        };

        if !model_info.is_downloaded {
            let error_msg = "Model not downloaded";
            emit_loading_failed(error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }

        let model_path = self.model_manager.get_model_path(model_id)?;

        let loaded_engine = match model_info.engine_type {
            EngineType::Whisper => {
                let engine = WhisperEngine::load(&model_path).map_err(|e| {
                    let error_msg = format!("Failed to load whisper model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Whisper(engine)
            }
            EngineType::Parakeet => {
                let engine =
                    ParakeetModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                        let error_msg = format!("Failed to load parakeet model {}: {}", model_id, e);
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Parakeet(engine)
            }
        };

        {
            let mut engine = self.lock_engine();
            *engine = Some(loaded_engine);
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = Some(model_id.to_string());
        }

        // Reset idle timer so the watcher doesn't immediately unload a just-loaded model.
        self.touch_activity();

        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_completed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );

        debug!(
            "Successfully loaded transcription model: {} (took {}ms)",
            model_id,
            load_start.elapsed().as_millis()
        );
        Ok(())
    }

    /// Kick off loading the active model in a background thread, unless one is
    /// already loaded or loading.
    pub fn initiate_model_load(&self) {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading || self.is_model_loaded() {
            return;
        }

        *is_loading = true;
        let self_clone = self.clone();
        thread::spawn(move || {
            let settings = get_settings(&self_clone.app_handle);
            match settings.selected_model {
                Some(model_id) if !model_id.is_empty() => {
                    if let Err(e) = self_clone.load_model(&model_id) {
                        error!("Failed to load model: {}", e);
                    }
                }
                _ => warn!("No model selected; skipping background load"),
            }
            let mut is_loading = self_clone.is_loading.lock().unwrap();
            *is_loading = false;
            self_clone.loading_condvar.notify_all();
        });
    }

    pub fn get_current_model(&self) -> Option<String> {
        self.current_model_id.lock().unwrap().clone()
    }

    /// Transcribe `audio`. When `quiet` is true (the live/partial loop, which
    /// fires every ~2.5s during recording), result/timing lines are logged at
    /// `debug!` instead of `info!` to keep the log file from filling with
    /// repeated partial transcripts. Error/warn logging is unaffected.
    pub fn transcribe(&self, audio: Vec<f32>, quiet: bool) -> Result<String, String> {
        self.transcribe_inner(audio, quiet).map_err(|e| e.to_string())
    }

    fn transcribe_inner(&self, audio: Vec<f32>, quiet: bool) -> Result<String> {
        self.touch_activity();
        let st = std::time::Instant::now();

        debug!("Audio vector length: {}", audio.len());
        if audio.is_empty() {
            debug!("Empty audio vector");
            self.maybe_unload_immediately("empty audio");
            return Ok(String::new());
        }

        let settings = get_settings(&self.app_handle);
        let selected_model = settings.selected_model.clone().unwrap_or_default();

        // Wait for any in-flight load to finish, then ensure something is loaded.
        {
            let mut is_loading = self.is_loading.lock().unwrap();
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }
        }
        if !self.is_model_loaded() {
            // Best-effort lazy load (covers the `Immediately` timeout path).
            if !selected_model.is_empty() {
                if let Err(e) = self.load_model(&selected_model) {
                    return Err(anyhow::anyhow!("Failed to load model for transcription: {}", e));
                }
            } else {
                return Err(anyhow::anyhow!("No model selected for transcription."));
            }
        }

        // Validate the selected language against the model's supported languages.
        let validated_language = if settings.selected_language == "auto" {
            "auto".to_string()
        } else {
            let is_supported = self
                .model_manager
                .get_model_info(&selected_model)
                .map(|info| {
                    info.supported_languages.is_empty()
                        || info
                            .supported_languages
                            .contains(&settings.selected_language)
                })
                .unwrap_or(true);

            if is_supported {
                settings.selected_language.clone()
            } else {
                warn!(
                    "Language '{}' not supported by current model, falling back to auto-detect",
                    settings.selected_language
                );
                "auto".to_string()
            }
        };

        // Transcribe. catch_unwind prevents an engine panic from poisoning the
        // mutex (which would hang the app). On panic the engine is dropped.
        let result = {
            let mut engine_guard = self.lock_engine();
            let mut engine = match engine_guard.take() {
                Some(e) => e,
                None => {
                    return Err(anyhow::anyhow!(
                        "Model failed to load. Please check your model settings."
                    ));
                }
            };
            drop(engine_guard);

            let transcribe_result = catch_unwind(AssertUnwindSafe(
                || -> Result<transcribe_rs::TranscriptionResult> {
                    match &mut engine {
                        LoadedEngine::Whisper(whisper_engine) => {
                            let whisper_language = if validated_language == "auto" {
                                None
                            } else if validated_language == "zh-Hans"
                                || validated_language == "zh-Hant"
                            {
                                Some("zh".to_string())
                            } else {
                                Some(validated_language.clone())
                            };

                            let params = WhisperInferenceParams {
                                language: whisper_language,
                                translate: settings.translate_to_english,
                                ..Default::default()
                            };

                            whisper_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))
                        }
                        LoadedEngine::Parakeet(parakeet_engine) => {
                            let params = ParakeetParams {
                                timestamp_granularity: Some(TimestampGranularity::Segment),
                                ..Default::default()
                            };
                            parakeet_engine.transcribe_with(&audio, &params).map_err(|e| {
                                anyhow::anyhow!("Parakeet transcription failed: {}", e)
                            })
                        }
                    }
                },
            ));

            match transcribe_result {
                Ok(inner_result) => {
                    // Success or normal error — put the engine back.
                    let mut engine_guard = self.lock_engine();
                    *engine_guard = Some(engine);
                    inner_result?
                }
                Err(panic_payload) => {
                    // Engine panicked — drop it (effectively unloading).
                    let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    error!(
                        "Transcription engine panicked: {}. Model has been unloaded.",
                        panic_msg
                    );

                    {
                        let mut current_model =
                            self.current_model_id.lock().unwrap_or_else(|e| e.into_inner());
                        *current_model = None;
                    }

                    let _ = self.app_handle.emit(
                        "model-state-changed",
                        ModelStateEvent {
                            event_type: "unloaded".to_string(),
                            model_id: None,
                            model_name: None,
                            error: Some(format!("Engine panicked: {}", panic_msg)),
                        },
                    );

                    return Err(anyhow::anyhow!(
                        "Transcription engine panicked: {}. The model has been unloaded and will reload on next attempt.",
                        panic_msg
                    ));
                }
            }
        };

        let final_result = result.text;

        let translation_note = if settings.translate_to_english {
            " (translated)"
        } else {
            ""
        };
        // Partial (live-loop) transcriptions log at debug to avoid spamming the
        // log every ~2.5s; the final transcription on stop logs at info as before.
        if quiet {
            debug!(
                "Partial transcription completed in {}ms{}",
                st.elapsed().as_millis(),
                translation_note
            );
            if final_result.is_empty() {
                debug!("Partial transcription result is empty");
            } else {
                debug!("Partial transcription result: {}", final_result);
            }
        } else {
            info!(
                "Transcription completed in {}ms{}",
                st.elapsed().as_millis(),
                translation_note
            );
            if final_result.is_empty() {
                info!("Transcription result is empty");
            } else {
                info!("Transcription result: {}", final_result);
            }
        }

        self.maybe_unload_immediately("transcription");
        Ok(final_result)
    }
}

impl Drop for TranscriptionManager {
    fn drop(&mut self) {
        // Skip shutdown unless this is the last clone. The watcher thread holds
        // its own clone, so engine's strong_count is >= 2 while it's alive.
        if Arc::strong_count(&self.engine) > 1 {
            return;
        }

        self.shutdown_signal.store(true, Ordering::Relaxed);

        if let Some(handle) = self.watcher_handle.lock().unwrap().take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join idle watcher thread: {:?}", e);
            } else {
                debug!("Idle watcher thread joined successfully");
            }
        }
    }
}
