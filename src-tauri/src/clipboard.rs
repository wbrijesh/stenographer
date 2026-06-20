//! Clipboard paste pipeline. macOS-only port of Handy's clipboard layer.
//!
//! Flow: save current clipboard -> write transcribed text -> sleep
//! `paste_delay_ms` -> synthesize Cmd+V -> (optional auto-submit) -> restore
//! the previous clipboard contents.
//!
//! Keystroke synthesis on macOS must run on the main thread, so [`paste`]
//! marshals the whole operation onto the main thread via
//! `AppHandle::run_on_main_thread` and blocks until it completes, propagating
//! the result back to the caller.

#![allow(dead_code)]

use std::sync::mpsc;
use std::time::Duration;

use log::info;
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::input;
use crate::settings::get_settings;

/// Paste `text` into the focused application via the clipboard.
///
/// Saves the current clipboard, writes `text`, waits `paste_delay_ms`, sends
/// Cmd+V, optionally auto-submits, then restores the original clipboard. The
/// entire operation runs on the main thread (required for macOS keystroke
/// synthesis). On failure a `paste-error` event is emitted with the error
/// message and the error is also returned to the caller.
pub fn paste(text: String, app: AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let app_for_main = app.clone();

    // Marshal the paste onto the main thread. `run_on_main_thread` returns
    // immediately; we block on the channel until the closure reports back.
    let dispatch = app.run_on_main_thread(move || {
        let result = paste_on_main_thread(text, &app_for_main);
        // If the receiver is gone the caller already bailed; ignore send error.
        let _ = tx.send(result);
    });

    if let Err(e) = dispatch {
        let msg = format!("Failed to dispatch paste to main thread: {}", e);
        emit_paste_error(&app, &msg);
        return Err(msg);
    }

    // Block until the main-thread closure finishes.
    let result = rx
        .recv()
        .map_err(|e| format!("Paste main-thread task did not report a result: {}", e))?;

    if let Err(ref msg) = result {
        emit_paste_error(&app, msg);
    }

    result
}

/// The actual paste pipeline, assumed to be running on the main thread.
fn paste_on_main_thread(text: String, app: &AppHandle) -> Result<(), String> {
    let settings = get_settings(app);
    let paste_delay_ms = settings.paste_delay_ms;

    // Append a trailing space if configured.
    let text = if settings.append_trailing_space {
        format!("{} ", text)
    } else {
        text
    };

    info!("Pasting via clipboard (Cmd+V), delay: {}ms", paste_delay_ms);

    let clipboard = app.clipboard();

    // Save the current clipboard so we can restore it afterwards.
    let previous = clipboard.read_text().unwrap_or_default();

    // Write the transcribed text to the clipboard.
    clipboard
        .write_text(text.clone())
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    // Give the target app time to observe the new clipboard contents.
    std::thread::sleep(Duration::from_millis(paste_delay_ms));

    // Synthesize Cmd+V using the lazily-initialized global Enigo instance.
    let paste_result = input::with_enigo(|enigo| input::send_paste_cmd_v(enigo));

    // Brief settle before any follow-up keystrokes / restore.
    std::thread::sleep(Duration::from_millis(50));

    // Optional auto-submit (Enter / Ctrl+Enter / Cmd+Enter).
    let submit_result = if paste_result.is_ok() && settings.auto_submit {
        let key = settings.auto_submit_key;
        input::with_enigo(|enigo| input::send_return_key(enigo, key))
    } else {
        Ok(())
    };

    // Always restore the original clipboard, even if paste/submit failed.
    let _ = clipboard.write_text(previous);

    paste_result?;
    submit_result?;

    Ok(())
}

/// Emit a `paste-error` event so the frontend can surface the failure.
fn emit_paste_error(app: &AppHandle, message: &str) {
    if let Err(e) = app.emit("paste-error", message.to_string()) {
        info!("Failed to emit paste-error event: {}", e);
    }
}
