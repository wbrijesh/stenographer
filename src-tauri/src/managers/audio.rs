//! `AudioRecordingManager` — owns the microphone life-cycle and the
//! capture → downmix → resample(16 kHz) → Silero VAD pipeline.
//!
//! Ported from Handy's `managers/audio.rs`, stripped to macOS-only and to the
//! settings actually present in Stenographer (on-demand microphone only; no
//! always-on / clamshell / lazy-close knobs). The interface matches
//! CONTRACTS.md.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Instant;

use log::{debug, error, info};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio_toolkit::{
    audio::{is_microphone_access_denied, is_no_input_device_error},
    list_input_devices as toolkit_list_input_devices,
    list_output_devices as toolkit_list_output_devices,
    vad::SmoothedVad,
    AudioRecorder, SileroVad,
};
use crate::settings::{get_settings, AppSettings};

const WHISPER_SAMPLE_RATE: usize = 16000;

/// Relative path (under the Tauri Resource base directory) of the bundled
/// Silero ONNX model. The orchestrator must ship the file here.
const VAD_RESOURCE_PATH: &str = "resources/models/silero_vad_v4.onnx";

/* ──────────────────────── public DTOs / errors ──────────────────────── */

/// Device descriptor returned to the frontend.
#[derive(Debug, Clone, Serialize, Type)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

/// Errors surfaced by the recording manager. The two variants the contract
/// requires (`MicPermissionDenied`, `NoInputDevice`) are parsed out of cpal's
/// backend-specific error strings exactly as Handy does.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", content = "message")]
pub enum AudioError {
    /// The OS denied microphone access (TCC / permission prompt declined).
    MicPermissionDenied(String),
    /// No usable input device is available.
    NoInputDevice(String),
    /// VAD model failed to load / initialise.
    VadInit(String),
    /// Recorder is already active for another binding.
    AlreadyRecording,
    /// Catch-all for anything else (stream build, backend errors, …).
    Other(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::MicPermissionDenied(m) => write!(f, "microphone permission denied: {m}"),
            AudioError::NoInputDevice(m) => write!(f, "no input device: {m}"),
            AudioError::VadInit(m) => write!(f, "VAD init failed: {m}"),
            AudioError::AlreadyRecording => write!(f, "already recording"),
            AudioError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for AudioError {}

impl AudioError {
    /// Classify a raw error string from the audio backend into the right
    /// variant, mirroring Handy's string-parsing approach.
    fn from_backend_str(msg: String) -> Self {
        if is_microphone_access_denied(&msg) {
            AudioError::MicPermissionDenied(msg)
        } else if is_no_input_device_error(&msg) {
            AudioError::NoInputDevice(msg)
        } else {
            AudioError::Other(msg)
        }
    }
}

/* ──────────────────────────── state ─────────────────────────────────── */

#[derive(Clone, Debug)]
enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

fn set_mute(mute: bool) {
    // macOS: toggle system output mute via AppleScript. Fails silently if
    // unsupported.
    use std::process::Command;
    let script = format!(
        "set volume output muted {}",
        if mute { "true" } else { "false" }
    );
    let _ = Command::new("osascript").args(["-e", &script]).output();
}

/* ──────────────────── recorder construction helper ──────────────────── */

fn create_audio_recorder(
    vad_path: &str,
    app_handle: &AppHandle,
) -> Result<AudioRecorder, AudioError> {
    let silero = SileroVad::new(vad_path, 0.3)
        .map_err(|e| AudioError::VadInit(format!("Failed to create SileroVad: {e}")))?;
    let smoothed_vad = SmoothedVad::new(Box::new(silero), 15, 15, 2);

    // Recorder with VAD plus a spectrum-level callback that forwards updates to
    // the frontend via the `mic-level` event.
    let recorder = AudioRecorder::new()
        .map_err(|e| AudioError::Other(format!("Failed to create AudioRecorder: {e}")))?
        .with_vad(Box::new(smoothed_vad))
        .with_level_callback({
            let app_handle = app_handle.clone();
            move |levels| {
                emit_levels(&app_handle, &levels);
            }
        });

    Ok(recorder)
}

/// Emit the per-bucket microphone spectrum levels (each in 0..1) to the
/// frontend as the `mic-level` event. Mirrors Handy's `overlay::emit_levels`.
fn emit_levels(app_handle: &AppHandle, levels: &Vec<f32>) {
    // Broadcast to all webviews (main window picks this up).
    let _ = app_handle.emit("mic-level", levels);

    // Direct emit to the overlay's webview window handle.
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.emit("mic-level", levels);
    }

    // Likely fix: target the overlay panel webview explicitly via Tauri 2's
    // targeted emit. A `&str` is accepted as a labeled `EventTarget`, which
    // reliably reaches the non-activating NSPanel webview even when the broadcast
    // `emit` / window handle path doesn't (e.g. panel webview registered as a
    // separate event target).
    let _ = app_handle.emit_to("recording_overlay", "mic-level", levels);
}

/* ──────────────────────────── manager ───────────────────────────────── */

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    app_handle: AppHandle,

    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_open: Arc<Mutex<bool>>,
    is_recording: Arc<Mutex<bool>>,
    is_paused: Arc<Mutex<bool>>,
    did_mute: Arc<Mutex<bool>>,
}

impl AudioRecordingManager {
    /* ---------- construction ------------------------------------------- */

    pub fn new(app: AppHandle) -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            app_handle: app,
            recorder: Arc::new(Mutex::new(None)),
            is_open: Arc::new(Mutex::new(false)),
            is_recording: Arc::new(Mutex::new(false)),
            is_paused: Arc::new(Mutex::new(false)),
            did_mute: Arc::new(Mutex::new(false)),
        }
    }

    /* ---------- VAD preload (idempotent) ------------------------------- */

    /// Load the Silero ONNX model and construct the recorder. Idempotent: a
    /// second call is a no-op. Errors are logged (not returned) so this can be
    /// called fire-and-forget at startup; `start_recording` surfaces failures.
    pub fn preload_vad(&self) {
        if let Err(e) = self.ensure_recorder() {
            error!("preload_vad failed: {e}");
        }
    }

    fn ensure_recorder(&self) -> Result<(), AudioError> {
        let mut recorder_opt = self.recorder.lock().unwrap();
        if recorder_opt.is_some() {
            return Ok(());
        }

        let vad_path = self
            .app_handle
            .path()
            .resolve(VAD_RESOURCE_PATH, tauri::path::BaseDirectory::Resource)
            .map_err(|e| AudioError::VadInit(format!("Failed to resolve VAD path: {e}")))?;

        let vad_path_str = vad_path
            .to_str()
            .ok_or_else(|| AudioError::VadInit("VAD path is not valid UTF-8".to_string()))?;

        *recorder_opt = Some(create_audio_recorder(vad_path_str, &self.app_handle)?);
        Ok(())
    }

    /* ---------- device selection --------------------------------------- */

    fn get_effective_microphone_device(&self, settings: &AppSettings) -> Option<cpal::Device> {
        let device_name = settings.selected_microphone.as_ref()?;

        match toolkit_list_input_devices() {
            Ok(devices) => devices
                .into_iter()
                .find(|d| d.name == *device_name)
                .map(|d| d.device),
            Err(e) => {
                debug!("Failed to list devices, using default: {e}");
                None
            }
        }
    }

    /* ---------- microphone stream life-cycle --------------------------- */

    /// Apply system mute if `mute_while_recording` is set and the stream is open.
    pub fn apply_mute(&self) {
        let settings = get_settings(&self.app_handle);
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if settings.mute_while_recording && *self.is_open.lock().unwrap() {
            set_mute(true);
            *did_mute_guard = true;
            debug!("Mute applied");
        }
    }

    /// Remove system mute if we applied it.
    pub fn remove_mute(&self) {
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
            *did_mute_guard = false;
            debug!("Mute removed");
        }
    }

    fn start_microphone_stream(&self) -> Result<(), AudioError> {
        let mut open_flag = self.is_open.lock().unwrap();
        if *open_flag {
            debug!("Microphone stream already active");
            return Ok(());
        }

        let start_time = Instant::now();

        *self.did_mute.lock().unwrap() = false;

        let settings = get_settings(&self.app_handle);
        let selected_device = self.get_effective_microphone_device(&settings);

        // Pre-flight: if nothing is selected AND there are no devices at all,
        // fail early with a clear NoInputDevice error.
        if selected_device.is_none() {
            let has_any_device = toolkit_list_input_devices()
                .map(|devices| !devices.is_empty())
                .unwrap_or(false);
            if !has_any_device {
                return Err(AudioError::NoInputDevice("No input device found".to_string()));
            }
        }

        // Ensure the VAD/recorder exists.
        self.ensure_recorder()?;

        let mut recorder_opt = self.recorder.lock().unwrap();
        if let Some(rec) = recorder_opt.as_mut() {
            rec.open(selected_device)
                .map_err(|e| AudioError::from_backend_str(e.to_string()))?;
        }

        *open_flag = true;
        info!("Microphone stream initialized in {:?}", start_time.elapsed());
        Ok(())
    }

    fn stop_microphone_stream(&self) {
        let mut open_flag = self.is_open.lock().unwrap();
        if !*open_flag {
            return;
        }

        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
        }
        *did_mute_guard = false;

        if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
            if *self.is_recording.lock().unwrap() {
                let _ = rec.stop();
                *self.is_recording.lock().unwrap() = false;
            }
            let _ = rec.close();
        }

        *open_flag = false;
        debug!("Microphone stream stopped");
    }

    /// Restart the stream so a newly-selected device takes effect.
    pub fn update_selected_device(&self) -> Result<(), AudioError> {
        if *self.is_open.lock().unwrap() {
            self.stop_microphone_stream();
            self.start_microphone_stream()?;
        }
        Ok(())
    }

    /* ---------- recording ---------------------------------------------- */

    /// Begin recording for `binding_id`. Opens the microphone stream on demand.
    pub fn start_recording(&self, binding_id: &str) -> Result<(), AudioError> {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Idle = *state {
            // On-demand: open the microphone if it isn't already.
            self.start_microphone_stream().map_err(|e| {
                error!("Failed to open microphone stream: {e}");
                e
            })?;

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                if rec.start().is_ok() {
                    *self.is_recording.lock().unwrap() = true;
                    *self.is_paused.lock().unwrap() = false;
                    *state = RecordingState::Recording {
                        binding_id: binding_id.to_string(),
                    };
                    debug!("Recording started for binding {binding_id}");
                    return Ok(());
                }
            }
            Err(AudioError::Other("Recorder not available".to_string()))
        } else {
            Err(AudioError::AlreadyRecording)
        }
    }

    /// Stop recording and return the 16 kHz mono, VAD-filtered samples.
    /// Blocking drain. Returns an empty vec if not currently recording.
    pub fn stop_recording(&self) -> Vec<f32> {
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording { .. } => {
                *state = RecordingState::Idle;
                drop(state);

                let samples = if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    // If paused, resume the cpal stream first so the stop-drain can
                    // receive the EndOfStream sentinel from the callback. The
                    // preserved buffer is returned unchanged.
                    if *self.is_paused.lock().unwrap() {
                        rec.resume();
                    }
                    match rec.stop() {
                        Ok(buf) => buf,
                        Err(e) => {
                            error!("stop() failed: {e}");
                            Vec::new()
                        }
                    }
                } else {
                    error!("Recorder not available");
                    Vec::new()
                };

                *self.is_recording.lock().unwrap() = false;
                *self.is_paused.lock().unwrap() = false;

                // On-demand: close the mic after the clip.
                self.stop_microphone_stream();

                // Pad very short clips so downstream Whisper has enough audio.
                let s_len = samples.len();
                if s_len < WHISPER_SAMPLE_RATE && s_len > 0 {
                    let mut padded = samples;
                    padded.resize(WHISPER_SAMPLE_RATE * 5 / 4, 0.0);
                    padded
                } else {
                    samples
                }
            }
            _ => Vec::new(),
        }
    }

    /// Snapshot the live audio captured so far (16 kHz mono, VAD-filtered) for
    /// streaming partial transcription. Returns empty if not currently
    /// recording. No padding — partials don't need it.
    pub fn current_samples(&self) -> Vec<f32> {
        if !self.is_recording() {
            return Vec::new();
        }
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.snapshot()
        } else {
            Vec::new()
        }
    }

    pub fn is_recording(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            RecordingState::Recording { .. }
        )
    }

    /// Pause the microphone without ending the session: capture stops feeding
    /// audio but the already-dictated buffer is preserved and the session stays
    /// active (`is_recording()` remains true). No-op if not recording or already
    /// paused.
    pub fn pause_recording(&self) {
        if !self.is_recording() {
            return;
        }
        let mut paused = self.is_paused.lock().unwrap();
        if *paused {
            return;
        }
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.pause();
        }
        *paused = true;
        debug!("Recording paused");
    }

    /// Resume the microphone after [`AudioRecordingManager::pause_recording`].
    /// No-op if not recording or not currently paused.
    pub fn resume_recording(&self) {
        if !self.is_recording() {
            return;
        }
        let mut paused = self.is_paused.lock().unwrap();
        if !*paused {
            return;
        }
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.resume();
        }
        *paused = false;
        debug!("Recording resumed");
    }

    /// Whether the active recording is currently paused. `is_recording()` stays
    /// true while paused.
    pub fn is_paused(&self) -> bool {
        *self.is_paused.lock().unwrap()
    }

    /// Cancel any ongoing recording without returning audio.
    pub fn cancel_recording(&self) {
        let mut state = self.state.lock().unwrap();
        if let RecordingState::Recording { .. } = *state {
            *state = RecordingState::Idle;
            drop(state);

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                // Resume a paused stream so the stop-drain completes cleanly.
                if *self.is_paused.lock().unwrap() {
                    rec.resume();
                }
                let _ = rec.stop();
            }
            *self.is_recording.lock().unwrap() = false;
            *self.is_paused.lock().unwrap() = false;
            self.stop_microphone_stream();
        }
    }

    /* ---------- device enumeration (static) ---------------------------- */

    pub fn list_input_devices() -> Vec<DeviceInfo> {
        toolkit_list_input_devices()
            .map(|devices| {
                devices
                    .into_iter()
                    .map(|d| DeviceInfo {
                        name: d.name,
                        is_default: d.is_default,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn list_output_devices() -> Vec<DeviceInfo> {
        toolkit_list_output_devices()
            .map(|devices| {
                devices
                    .into_iter()
                    .map(|d| DeviceInfo {
                        name: d.name,
                        is_default: d.is_default,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
