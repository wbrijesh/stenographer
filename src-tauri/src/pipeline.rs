//! Phase-2 integration glue: builds the real [`PipelineActions`] that the
//! [`Coordinator`] fires on each transition, and installs the tray/overlay event
//! listeners.
//!
//! ## Arc-cycle avoidance
//! The coordinator owns the actions, and the app owns the coordinator (managed
//! state). If the action closures captured the manager `Arc`s (or the
//! coordinator `Arc`) directly, we'd create reference cycles that leak. Instead
//! each closure captures only an [`AppHandle`] and fetches the managers from
//! Tauri state (`app.state::<Arc<…>>()`) when it runs.
//!
//! ## Threading
//! - `record_start` runs on the coordinator's actor thread and must stay quick:
//!   it kicks model/VAD preload, plays the (async) start sound, and starts the
//!   mic. None of those block.
//! - `record_stop` must **not** block the actor thread (transcription can take
//!   seconds), so it spawns a `std::thread` that does stop → transcribe → paste
//!   → cleanup → `notify_processing_finished`.
//! - `cancel` is quick and stays on the actor thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Listener, Manager};

/// Set while a recording is active so the live (streaming) partial-transcription
/// loop runs. Cleared first thing on stop/cancel so the loop releases the
/// transcription engine before the authoritative final transcription runs.
static LIVE_ACTIVE: AtomicBool = AtomicBool::new(false);

use crate::coordinator::{Coordinator, PipelineActions, TriggerInput};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::model::ModelManager;
use crate::managers::transcription::TranscriptionManager;
use crate::tray::{self, TrayState};
use crate::{audio_feedback, clipboard, overlay};

/// Build the real pipeline actions for the coordinator.
pub fn build_pipeline_actions(app_handle: AppHandle) -> PipelineActions {
    let start_app = app_handle.clone();
    let stop_app = app_handle.clone();
    let cancel_app = app_handle.clone();

    PipelineActions {
        record_start: Box::new(move || record_start(&start_app)),
        record_stop: Box::new(move || record_stop(stop_app.clone())),
        cancel: Box::new(move || cancel(&cancel_app)),
    }
}

/// Run a UI closure on the macOS main thread (AppKit requires it).
///
/// All overlay (NSPanel) and tray (`TrayIcon`) updates MUST run on the main
/// thread; `run_on_main_thread` schedules the closure asynchronously, which is
/// fine for these fire-and-forget UI updates.
fn on_main(app: &AppHandle, f: impl FnOnce(AppHandle) + Send + 'static) {
    let a = app.clone();
    if let Err(e) = app.run_on_main_thread(move || f(a)) {
        log::error!("run_on_main_thread failed: {e}");
    }
}

fn record_start(app: &AppHandle) {
    // Overlay/tray are AppKit and must run on the main thread.
    on_main(app, |app| {
        tray::set_state(&app, TrayState::Recording);
        overlay::show_recording(&app);
    });
    audio_feedback::play_start_sound(app);

    let transcription = app.state::<Arc<TranscriptionManager>>();
    let audio = app.state::<Arc<AudioRecordingManager>>();

    // Kick model load + VAD preload in parallel; both are internally async /
    // idempotent so we don't block here.
    transcription.initiate_model_load();
    audio.preload_vad();

    if let Err(e) = audio.start_recording("fn") {
        log::error!("start_recording failed: {e}");
        // Recover: surface the error, reset UI, and release the pipeline stage.
        let _ = app.emit_recording_error(&e.to_string());
        // Overlay/tray are AppKit and must run on the main thread.
        on_main(app, |app| {
            overlay::hide_overlay(&app);
            tray::set_state(&app, TrayState::Idle);
        });
        if let Some(coordinator) = app.try_state::<Arc<Coordinator>>() {
            coordinator.notify_processing_finished();
        }
        return;
    }

    // Start the live (streaming) partial-transcription loop. It periodically
    // snapshots the audio-so-far, transcribes it, and emits the growing text to
    // the overlay. The authoritative final transcription at stop is unchanged.
    LIVE_ACTIVE.store(true, Ordering::Release);
    let live_app = app.clone();
    std::thread::spawn(move || {
        live_transcription_loop(live_app);
    });
}

/// The live (streaming) partial-transcription loop. Runs on its own thread while
/// `LIVE_ACTIVE` is set. Managed state is fetched INSIDE the loop (mirroring the
/// rest of pipeline.rs) so we don't capture manager `Arc`s.
fn live_transcription_loop(app: AppHandle) {
    loop {
        if !LIVE_ACTIVE.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2500));
        if !LIVE_ACTIVE.load(Ordering::Acquire) {
            break;
        }

        let audio = app.state::<Arc<AudioRecordingManager>>().current_samples();
        // ~0.5s @ 16kHz — skip tiny/empty snapshots.
        if audio.len() >= 8000 {
            if let Ok(text) = app.state::<Arc<TranscriptionManager>>().transcribe(audio) {
                if LIVE_ACTIVE.load(Ordering::Acquire) && !text.trim().is_empty() {
                    use tauri::Emitter;
                    let _ = app.emit_to("recording_overlay", "partial-transcript", text.clone());
                    let _ = app.emit("partial-transcript", text);
                }
            }
        }
    }
}

fn record_stop(app: AppHandle) {
    // Stop the live partial-transcription loop FIRST, before stop_recording /
    // the final transcribe, so it releases the transcription engine before the
    // authoritative final transcription runs.
    LIVE_ACTIVE.store(false, Ordering::Release);

    // Off the actor thread: transcription can take seconds.
    std::thread::spawn(move || {
        // Overlay/tray are AppKit and must run on the main thread.
        on_main(&app, |app| {
            tray::set_state(&app, TrayState::Transcribing);
            overlay::show_transcribing(&app);
        });
        audio_feedback::play_stop_sound(&app);

        let audio = app.state::<Arc<AudioRecordingManager>>();
        let samples = audio.stop_recording();

        if samples.is_empty() {
            log::info!("record_stop: no audio captured");
        } else {
            let transcription = app.state::<Arc<TranscriptionManager>>();
            match transcription.transcribe(samples) {
                Ok(text) if !text.trim().is_empty() => {
                    // On-device cleanup (Apple FoundationModels). Only when the
                    // user has it enabled AND the model is available; falls back
                    // to the raw text on any cleanup failure. The overlay keeps
                    // showing the transcribing state during this ~1-2s.
                    let to_paste = if crate::settings::get_settings(&app).cleanup_enabled
                        && crate::llm::is_available()
                    {
                        let cleaned = crate::llm::cleanup(&text).unwrap_or_else(|| text.clone());
                        log::info!(
                            "cleanup: raw {} chars -> cleaned {} chars",
                            text.len(),
                            cleaned.len()
                        );
                        cleaned
                    } else {
                        text
                    };
                    // `clipboard::paste` self-marshals to the main thread and
                    // blocks; do NOT wrap it in run_on_main_thread here.
                    if let Err(e) = clipboard::paste(to_paste, app.clone()) {
                        log::error!("paste failed: {e}");
                    }
                }
                Ok(_) => log::info!("record_stop: transcription empty, nothing to paste"),
                Err(e) => log::error!("transcription failed: {e}"),
            }
        }

        // Overlay/tray are AppKit and must run on the main thread.
        on_main(&app, |app| {
            overlay::hide_overlay(&app);
            tray::set_state(&app, TrayState::Idle);
        });

        if let Some(coordinator) = app.try_state::<Arc<Coordinator>>() {
            coordinator.notify_processing_finished();
        }
    });
}

fn cancel(app: &AppHandle) {
    // Stop the live partial-transcription loop.
    LIVE_ACTIVE.store(false, Ordering::Release);

    let audio = app.state::<Arc<AudioRecordingManager>>();
    audio.cancel_recording();
    // Overlay/tray are AppKit and must run on the main thread.
    on_main(app, |app| {
        overlay::hide_overlay(&app);
        tray::set_state(&app, TrayState::Idle);
    });
    if let Some(coordinator) = app.try_state::<Arc<Coordinator>>() {
        coordinator.notify_processing_finished();
    }
}

/// Install listeners for tray/overlay-originated control events. Forwards them
/// to the coordinator / model managers. Call once in setup.
pub fn install_event_listeners(app: &AppHandle) {
    // Tray "Cancel" and overlay cancel button → submit Cancel.
    let cancel_app = app.clone();
    app.listen_any(tray::TRAY_CANCEL_EVENT, move |_| {
        if let Some(coordinator) = cancel_app.try_state::<Arc<Coordinator>>() {
            coordinator.submit(TriggerInput::Cancel);
        }
    });

    let overlay_cancel_app = app.clone();
    app.listen_any("overlay-cancel", move |_| {
        if let Some(coordinator) = overlay_cancel_app.try_state::<Arc<Coordinator>>() {
            coordinator.submit(TriggerInput::Cancel);
        }
    });

    // Overlay mic-icon click → toggle pause/resume of the active recording.
    // Each event flips the paused state (only while recording), then broadcasts
    // the new paused state as `recording-paused` so the overlay can swap its mic
    // icon. Emitted to the overlay window specifically AND app-wide.
    let pause_app = app.clone();
    app.listen_any("overlay-toggle-pause", move |_| {
        use tauri::Emitter;
        let audio = pause_app.state::<Arc<AudioRecordingManager>>();
        if !audio.is_recording() {
            return;
        }
        let now_paused = if audio.is_paused() {
            audio.resume_recording();
            false
        } else {
            audio.pause_recording();
            true
        };
        let _ = pause_app.emit_to("recording_overlay", "recording-paused", now_paused);
        let _ = pause_app.emit("recording-paused", now_paused);
    });

    // Tray model select → switch active model + reload.
    let model_app = app.clone();
    app.listen_any(tray::TRAY_MODEL_SELECT_EVENT, move |event| {
        // The payload is a JSON-encoded string (the model id).
        let raw = event.payload();
        let model_id = serde_json::from_str::<String>(raw).unwrap_or_else(|_| raw.to_string());
        if model_id.is_empty() {
            return;
        }
        if let Some(model_manager) = model_app.try_state::<Arc<ModelManager>>() {
            if let Err(e) = model_manager.set_active(&model_id) {
                log::error!("tray model select: set_active({model_id}) failed: {e}");
                return;
            }
        }
        if let Some(transcription) = model_app.try_state::<Arc<TranscriptionManager>>() {
            transcription.initiate_model_load();
        }
    });
}

/// Small helper trait to emit the `recording-error` event (CONTRACTS §6) with a
/// typed-ish payload without pulling another import into the closures.
trait RecordingErrorExt {
    fn emit_recording_error(&self, message: &str) -> Result<(), tauri::Error>;
}

impl RecordingErrorExt for AppHandle {
    fn emit_recording_error(&self, message: &str) -> Result<(), tauri::Error> {
        use tauri::Emitter;
        self.emit(
            "recording-error",
            serde_json::json!({ "error_type": message }),
        )
    }
}
