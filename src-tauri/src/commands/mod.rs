pub mod audio;
pub mod models;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::managers::audio::AudioRecordingManager;
use crate::settings::{
    self, get_settings, write_settings, AppSettings, AutoSubmitKey, ModelUnloadTimeout,
};

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

/// Validate + persist + emit the trigger-mode-enabled toggle.
#[tauri::command]
#[specta::specta]
pub fn change_trigger_mode_enabled(
    app: AppHandle,
    trigger_mode_enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.trigger_mode_enabled = trigger_mode_enabled;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the hold/tap threshold.
#[tauri::command]
#[specta::specta]
pub fn change_hold_tap_threshold_ms(
    app: AppHandle,
    hold_tap_threshold_ms: u64,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.hold_tap_threshold_ms = hold_tap_threshold_ms;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the selected microphone device.
#[tauri::command]
#[specta::specta]
pub fn set_selected_microphone(
    app: AppHandle,
    audio: State<'_, Arc<AudioRecordingManager>>,
    selected_microphone: Option<String>,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_microphone = selected_microphone;
    write_settings(&app, &settings);
    // Restart the (open) stream so the new device takes effect immediately.
    if let Err(e) = audio.update_selected_device() {
        log::warn!("Failed to switch microphone device: {e}");
    }
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the selected output device.
#[tauri::command]
#[specta::specta]
pub fn set_selected_output_device(
    app: AppHandle,
    selected_output_device: Option<String>,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_output_device = selected_output_device;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the audio-feedback toggle.
#[tauri::command]
#[specta::specta]
pub fn change_audio_feedback(app: AppHandle, audio_feedback: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.audio_feedback = audio_feedback;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the audio-feedback volume.
#[tauri::command]
#[specta::specta]
pub fn change_audio_feedback_volume(
    app: AppHandle,
    audio_feedback_volume: f32,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.audio_feedback_volume = audio_feedback_volume;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the mute-while-recording toggle.
#[tauri::command]
#[specta::specta]
pub fn change_mute_while_recording(
    app: AppHandle,
    mute_while_recording: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.mute_while_recording = mute_while_recording;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the selected model.
#[tauri::command]
#[specta::specta]
pub fn change_selected_model(
    app: AppHandle,
    selected_model: Option<String>,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_model = selected_model;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the selected language.
#[tauri::command]
#[specta::specta]
pub fn change_selected_language(app: AppHandle, selected_language: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_language = selected_language;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the translate-to-english toggle.
#[tauri::command]
#[specta::specta]
pub fn change_translate_to_english(
    app: AppHandle,
    translate_to_english: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.translate_to_english = translate_to_english;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the model-unload timeout.
#[tauri::command]
#[specta::specta]
pub fn change_model_unload_timeout(
    app: AppHandle,
    model_unload_timeout: ModelUnloadTimeout,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.model_unload_timeout = model_unload_timeout;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the auto-submit toggle.
#[tauri::command]
#[specta::specta]
pub fn change_auto_submit(app: AppHandle, auto_submit: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.auto_submit = auto_submit;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the auto-submit key.
#[tauri::command]
#[specta::specta]
pub fn change_auto_submit_key(app: AppHandle, auto_submit_key: AutoSubmitKey) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.auto_submit_key = auto_submit_key;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the append-trailing-space toggle.
#[tauri::command]
#[specta::specta]
pub fn change_append_trailing_space(
    app: AppHandle,
    append_trailing_space: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.append_trailing_space = append_trailing_space;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the start-hidden toggle.
#[tauri::command]
#[specta::specta]
pub fn change_start_hidden(app: AppHandle, start_hidden: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.start_hidden = start_hidden;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the autostart-enabled toggle.
#[tauri::command]
#[specta::specta]
pub fn change_autostart_enabled(app: AppHandle, autostart_enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.autostart_enabled = autostart_enabled;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the show-tray-icon toggle.
#[tauri::command]
#[specta::specta]
pub fn change_show_tray_icon(app: AppHandle, show_tray_icon: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.show_tray_icon = show_tray_icon;
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
