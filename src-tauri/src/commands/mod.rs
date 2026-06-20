pub mod models;

use tauri::{AppHandle, Emitter};

use crate::settings::{self, get_settings, write_settings, AppSettings};

/// Return the current persisted settings.
#[tauri::command]
#[specta::specta]
pub fn get_app_settings(app: AppHandle) -> AppSettings {
    get_settings(&app)
}

/// Return the hard-coded default settings (used to reset / seed the UI).
#[tauri::command]
#[specta::specta]
pub fn get_default_settings() -> AppSettings {
    settings::get_default_settings()
}

/// Validate + persist + emit a new paste delay.
#[tauri::command]
#[specta::specta]
pub fn change_paste_delay_ms(app: AppHandle, paste_delay_ms: u64) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.paste_delay_ms = paste_delay_ms;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the overlay-enabled toggle.
#[tauri::command]
#[specta::specta]
pub fn change_overlay_enabled(app: AppHandle, overlay_enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.overlay_enabled = overlay_enabled;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Show (and focus) the main settings window, restoring the Dock icon on macOS.
#[tauri::command]
#[specta::specta]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    crate::show_main_window(&app);
    Ok(())
}
