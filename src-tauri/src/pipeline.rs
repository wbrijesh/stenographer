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

use std::sync::Arc;

use tauri::{AppHandle, Listener, Manager};

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

fn record_start(app: &AppHandle) {
    tray::set_state(app, TrayState::Recording);
    overlay::show_recording(app);
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
        overlay::hide_overlay(app);
        tray::set_state(app, TrayState::Idle);
        if let Some(coordinator) = app.try_state::<Arc<Coordinator>>() {
            coordinator.notify_processing_finished();
        }
    }
}

fn record_stop(app: AppHandle) {
    // Off the actor thread: transcription can take seconds.
    std::thread::spawn(move || {
        tray::set_state(&app, TrayState::Transcribing);
        overlay::show_transcribing(&app);
        audio_feedback::play_stop_sound(&app);

        let audio = app.state::<Arc<AudioRecordingManager>>();
        let samples = audio.stop_recording();

        if samples.is_empty() {
            log::info!("record_stop: no audio captured");
        } else {
            let transcription = app.state::<Arc<TranscriptionManager>>();
            match transcription.transcribe(samples) {
                Ok(text) if !text.trim().is_empty() => {
                    // `clipboard::paste` self-marshals to the main thread and
                    // blocks; do NOT wrap it in run_on_main_thread here.
                    if let Err(e) = clipboard::paste(text, app.clone()) {
                        log::error!("paste failed: {e}");
                    }
                }
                Ok(_) => log::info!("record_stop: transcription empty, nothing to paste"),
                Err(e) => log::error!("transcription failed: {e}"),
            }
        }

        overlay::hide_overlay(&app);
        tray::set_state(&app, TrayState::Idle);

        if let Some(coordinator) = app.try_state::<Arc<Coordinator>>() {
            coordinator.notify_processing_finished();
        }
    });
}

fn cancel(app: &AppHandle) {
    let audio = app.state::<Arc<AudioRecordingManager>>();
    audio.cancel_recording();
    overlay::hide_overlay(app);
    tray::set_state(app, TrayState::Idle);
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
