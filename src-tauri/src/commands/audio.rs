//! Audio device + lifecycle/permission commands.
//!
//! Per CONTRACTS §5. Device enumeration wraps the static
//! `AudioRecordingManager::list_*_devices` helpers; the lifecycle commands
//! drive the Accessibility-gated startup (enigo + Fn listener) that the
//! onboarding flow calls once permission has been granted.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, State};

use crate::coordinator::{Coordinator, TriggerInput};
use crate::managers::audio::{AudioRecordingManager, DeviceInfo};
use crate::shortcut::{start_fn_listener, FnListenerHandle};

/// Managed wrapper around the live Fn listener handle. Kept in Tauri state so
/// the CGEventTap stays installed for the app's lifetime (dropping the handle
/// tears the listener down).
#[derive(Default)]
pub struct FnListenerState(pub Mutex<Option<FnListenerHandle>>);

/// List available input (microphone) devices.
#[tauri::command]
#[specta::specta]
pub fn get_available_microphones() -> Vec<DeviceInfo> {
    AudioRecordingManager::list_input_devices()
}

/// List available output (speaker) devices.
#[tauri::command]
#[specta::specta]
pub fn get_available_output_devices() -> Vec<DeviceInfo> {
    AudioRecordingManager::list_output_devices()
}

/// Whether a recording is currently in progress.
#[tauri::command]
#[specta::specta]
pub fn is_recording(audio: State<'_, Arc<AudioRecordingManager>>) -> bool {
    audio.is_recording()
}

/// Play the start feedback sound so the user can preview volume / output device.
#[tauri::command]
#[specta::specta]
pub fn play_test_sound(app: AppHandle) {
    crate::audio_feedback::play_test_sound(&app);
}

/// Initialize the global Enigo instance (must run on the main thread). Fails if
/// Accessibility permission has not been granted. Idempotent.
#[tauri::command]
#[specta::specta]
pub fn initialize_enigo(app: AppHandle) -> Result<(), String> {
    crate::input::init_enigo(app)
}

/// Start the Fn-key listener and store its handle in managed state. Fails if
/// Accessibility permission has not been granted (the CGEventTap can't install).
/// Idempotent: a no-op if the listener is already running.
#[tauri::command]
#[specta::specta]
pub fn initialize_shortcuts(
    app: AppHandle,
    coordinator: State<'_, Arc<Coordinator>>,
    listener_state: State<'_, FnListenerState>,
) -> Result<(), String> {
    let mut guard = listener_state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock Fn listener state: {e}"))?;
    if guard.is_some() {
        return Ok(());
    }
    let handle = start_fn_listener(app, coordinator.inner().clone())?;
    *guard = Some(handle);
    Ok(())
}

/// Cancel any in-flight recording/transcription by submitting `Cancel` to the
/// coordinator.
#[tauri::command]
#[specta::specta]
pub fn cancel_operation(coordinator: State<'_, Arc<Coordinator>>) {
    coordinator.submit(TriggerInput::Cancel);
}
