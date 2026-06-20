//! Audio device + lifecycle/permission commands.
//!
//! Per CONTRACTS §5. Device enumeration wraps the static
//! `AudioRecordingManager::list_*_devices` helpers; the lifecycle commands
//! drive the Accessibility-gated startup (enigo + Fn listener) that the
//! onboarding flow calls once permission has been granted.

#![allow(dead_code)]

use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use handy_keys::{Hotkey, Key, KeyboardListener};
use tauri::{AppHandle, State};

use crate::coordinator::{Coordinator, TriggerInput};
use crate::managers::audio::{AudioRecordingManager, DeviceInfo};
use crate::settings::{get_settings, write_settings};
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

/// Return the current persisted trigger binding string (e.g. `"CmdRight"`).
#[tauri::command]
#[specta::specta]
pub fn get_trigger_binding(app: AppHandle) -> String {
    get_settings(&app).trigger_binding
}

/// Validate, persist, and apply a new trigger binding.
///
/// The string must parse as a `handy_keys::Hotkey` (else `Err`). On success it
/// is persisted, then the live listener is restarted so the new binding takes
/// effect immediately. If the listener was not running (e.g. Accessibility not
/// yet granted), the binding is just persisted — it will be picked up when the
/// listener is initialized later.
#[tauri::command]
#[specta::specta]
pub fn change_trigger_binding(
    app: AppHandle,
    coordinator: State<'_, Arc<Coordinator>>,
    listener_state: State<'_, FnListenerState>,
    binding: String,
) -> Result<(), String> {
    // Validate + canonicalize (round-trip through Hotkey).
    let parsed = Hotkey::from_str(&binding).map_err(|e| format!("invalid binding: {e}"))?;
    let canonical = parsed.to_string();

    // Persist first.
    let mut settings = get_settings(&app);
    settings.trigger_binding = canonical.clone();
    write_settings(&app, &settings);

    // Restart the listener if it is running so the new binding applies now.
    let mut guard = listener_state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock trigger listener state: {e}"))?;
    if let Some(old) = guard.take() {
        old.stop();
        let handle = start_fn_listener(app.clone(), coordinator.inner().clone())?;
        *guard = Some(handle);
        log::info!("Trigger binding changed to {canonical}; listener restarted");
    } else {
        log::info!("Trigger binding changed to {canonical}; listener not running (will apply on init)");
    }

    Ok(())
}

/// Capture the user's next keyboard gesture for the "record shortcut" UI.
///
/// While capturing, the main trigger listener is temporarily stopped (so it
/// can't toggle recording during capture) and a fresh [`KeyboardListener`] is
/// installed. The capture loop runs for up to ~7s:
///
/// - a key-DOWN with a non-modifier key → returns `modifiers+key` (e.g.
///   `"Ctrl+Shift+R"`);
/// - a modifier-only press → returns the held modifiers on the modifier RELEASE
///   (e.g. `"CmdRight"`);
/// - `Escape` → `Err("cancelled")`;
/// - no gesture before the timeout → `Err("timed out")`.
///
/// The main listener (with the still-current saved binding) is always restarted
/// before returning if it was running. Runs on a blocking thread so the up-to-7s
/// wait does not block the async runtime / main thread.
#[tauri::command]
#[specta::specta]
pub async fn capture_shortcut(
    app: AppHandle,
    coordinator: State<'_, Arc<Coordinator>>,
    listener_state: State<'_, FnListenerState>,
) -> Result<String, String> {
    // (a) Temporarily take the main listener out of state so it stops toggling.
    let was_running = {
        let mut guard = listener_state
            .0
            .lock()
            .map_err(|e| format!("Failed to lock trigger listener state: {e}"))?;
        if let Some(handle) = guard.take() {
            handle.stop();
            true
        } else {
            false
        }
    };

    // (b)+(c) Capture on a blocking thread (the up-to-7s wait must not block).
    let capture_result =
        tauri::async_runtime::spawn_blocking(capture_gesture)
            .await
            .map_err(|e| format!("capture task failed: {e}"))?;

    // (d) Always restart the main listener if it was running before.
    if was_running {
        match start_fn_listener(app.clone(), coordinator.inner().clone()) {
            Ok(handle) => {
                if let Ok(mut guard) = listener_state.0.lock() {
                    *guard = Some(handle);
                }
            }
            Err(e) => log::error!("Failed to restart trigger listener after capture: {e}"),
        }
    }

    capture_result
}

/// Blocking capture loop. Mirrors handy-keys' `examples/record_hotkey.rs`.
fn capture_gesture() -> Result<String, String> {
    const OVERALL_TIMEOUT: Duration = Duration::from_secs(7);
    const RECV_TIMEOUT: Duration = Duration::from_millis(100);

    let listener = KeyboardListener::new()
        .map_err(|e| format!("failed to start capture listener (Accessibility granted?): {e}"))?;

    let deadline = Instant::now() + OVERALL_TIMEOUT;

    while Instant::now() < deadline {
        let event = match listener.recv_timeout(RECV_TIMEOUT) {
            Ok(ev) => ev,
            Err(handy_keys::Error::Timeout) => continue,
            Err(_) => return Err("capture event source disconnected".to_string()),
        };

        // Escape cancels.
        if event.is_key_down && event.key == Some(Key::Escape) {
            return Err("cancelled".to_string());
        }

        // A non-modifier key down → full combo (modifiers + key).
        if event.is_key_down {
            if let Some(key) = event.key {
                return Hotkey::new(event.modifiers, key)
                    .map(|h| h.to_string())
                    .map_err(|e| format!("invalid captured hotkey: {e}"));
            }
        }

        // Modifier-only: return on the RELEASE of a modifier, using the
        // modifiers that were held just before release (`changed_modifier`),
        // since `event.modifiers` no longer contains the released flag.
        if !event.is_key_down && event.key.is_none() {
            if let Some(changed) = event.changed_modifier {
                let held = event.modifiers | changed;
                if !held.is_empty() {
                    return Hotkey::new(held, None)
                        .map(|h| h.to_string())
                        .map_err(|e| format!("invalid captured hotkey: {e}"));
                }
            }
        }
    }

    Err("timed out".to_string())
}
