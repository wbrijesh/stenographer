//! Keystroke synthesis (enigo). macOS-only port of Handy's input layer.
//!
//! The `Enigo` instance is created lazily and stored in a process-global
//! `Mutex<Option<Enigo>>` (via `once_cell`). It is only created successfully
//! when macOS Accessibility permission has been granted; otherwise
//! [`init_enigo`] surfaces a clear error string. Because keystroke synthesis
//! on macOS must run on the main thread, callers should ensure the lock is
//! taken / paste is performed on the main thread (see [`crate::clipboard`]).

#![allow(dead_code)]

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use tauri::AppHandle;

use crate::settings::AutoSubmitKey;

/// Process-global, lazily-initialized Enigo instance.
///
/// Wrapped in a `Mutex` because `Enigo` requires `&mut self` for keystroke
/// synthesis. `None` until [`init_enigo`] succeeds.
static ENIGO: Lazy<Mutex<Option<Enigo>>> = Lazy::new(|| Mutex::new(None));

/// Lazily create the global `Enigo` instance.
///
/// On macOS, `Enigo::new` succeeds only when the app has been granted
/// Accessibility permission. If creation fails (typically: permission not
/// granted), a clear error string is returned and no instance is stored.
///
/// Idempotent: if the instance already exists this is a no-op.
///
/// Must be called on the main thread (Enigo construction touches the main
/// run loop on macOS).
pub fn init_enigo(_app: AppHandle) -> Result<(), String> {
    let mut guard = ENIGO
        .lock()
        .map_err(|e| format!("Failed to lock Enigo: {}", e))?;

    if guard.is_some() {
        return Ok(());
    }

    let enigo = Enigo::new(&Settings::default()).map_err(|e| {
        format!(
            "Failed to initialize Enigo (is Accessibility permission granted?): {}",
            e
        )
    })?;

    *guard = Some(enigo);
    Ok(())
}

/// Returns whether the global Enigo instance has been initialized.
pub fn is_enigo_initialized() -> bool {
    ENIGO.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Run a closure with mutable access to the global Enigo instance.
///
/// Returns an error if the instance has not been initialized via
/// [`init_enigo`], or if the closure itself fails.
///
/// Must be invoked on the main thread on macOS.
pub fn with_enigo<F>(f: F) -> Result<(), String>
where
    F: FnOnce(&mut Enigo) -> Result<(), String>,
{
    let mut guard = ENIGO
        .lock()
        .map_err(|e| format!("Failed to lock Enigo: {}", e))?;
    let enigo = guard
        .as_mut()
        .ok_or("Enigo not initialized; call init_enigo after granting Accessibility permission")?;
    f(enigo)
}

/// Sends a Cmd+V paste command using a raw virtual key code.
///
/// Uses `Key::Meta` (Cmd) held + `Key::Other(9)` (the raw macOS keycode for
/// `v`) so paste works regardless of keyboard layout (e.g. Russian, AZERTY,
/// DVORAK), exactly as Handy does.
pub fn send_paste_cmd_v(enigo: &mut Enigo) -> Result<(), String> {
    let (modifier_key, v_key_code) = (Key::Meta, Key::Other(9));

    // Press Cmd, click V, release Cmd.
    enigo
        .key(modifier_key, Direction::Press)
        .map_err(|e| format!("Failed to press modifier key: {}", e))?;
    enigo
        .key(v_key_code, Direction::Click)
        .map_err(|e| format!("Failed to click V key: {}", e))?;

    std::thread::sleep(std::time::Duration::from_millis(100));

    enigo
        .key(modifier_key, Direction::Release)
        .map_err(|e| format!("Failed to release modifier key: {}", e))?;

    Ok(())
}

/// Sends the configured auto-submit key combination.
///
/// - `Enter`     -> Return
/// - `CtrlEnter` -> Ctrl+Return
/// - `CmdEnter`  -> Cmd(Meta)+Return
pub fn send_return_key(enigo: &mut Enigo, key_type: AutoSubmitKey) -> Result<(), String> {
    match key_type {
        AutoSubmitKey::Enter => {
            enigo
                .key(Key::Return, Direction::Press)
                .map_err(|e| format!("Failed to press Return key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Release)
                .map_err(|e| format!("Failed to release Return key: {}", e))?;
        }
        AutoSubmitKey::CtrlEnter => {
            enigo
                .key(Key::Control, Direction::Press)
                .map_err(|e| format!("Failed to press Control key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Press)
                .map_err(|e| format!("Failed to press Return key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Release)
                .map_err(|e| format!("Failed to release Return key: {}", e))?;
            enigo
                .key(Key::Control, Direction::Release)
                .map_err(|e| format!("Failed to release Control key: {}", e))?;
        }
        AutoSubmitKey::CmdEnter => {
            enigo
                .key(Key::Meta, Direction::Press)
                .map_err(|e| format!("Failed to press Meta/Cmd key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Press)
                .map_err(|e| format!("Failed to press Return key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Release)
                .map_err(|e| format!("Failed to release Return key: {}", e))?;
            enigo
                .key(Key::Meta, Direction::Release)
                .map_err(|e| format!("Failed to release Meta/Cmd key: {}", e))?;
        }
    }

    Ok(())
}
