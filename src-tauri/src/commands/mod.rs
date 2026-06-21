pub mod audio;
pub mod models;
pub mod permissions;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::metrics::Metrics;

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

/// Whether a hosted cleanup model is configured (i.e. an API key is set). The
/// frontend uses this to indicate whether the "Clean up transcription" toggle
/// will actually take effect.
#[tauri::command]
#[specta::specta]
pub fn is_cleanup_configured(app: AppHandle) -> bool {
    crate::llm::is_configured(&app)
}

/// Validate + persist + emit the cleanup-enabled toggle. Only effective when
/// `is_cleanup_configured()` is true.
#[tauri::command]
#[specta::specta]
pub fn change_cleanup_enabled(app: AppHandle, cleanup_enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.cleanup_enabled = cleanup_enabled;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the LLM base URL.
#[tauri::command]
#[specta::specta]
pub fn change_llm_base_url(app: AppHandle, llm_base_url: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.llm_base_url = llm_base_url;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the LLM API key.
#[tauri::command]
#[specta::specta]
pub fn change_llm_api_key(app: AppHandle, llm_api_key: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.llm_api_key = llm_api_key;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate + persist + emit the LLM model name.
#[tauri::command]
#[specta::specta]
pub fn change_llm_model(app: AppHandle, llm_model: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.llm_model = llm_model;
    write_settings(&app, &settings);
    app.emit("settings-changed", &settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Verify the configured hosted cleanup model by cleaning a tiny test input.
/// Returns the cleaned output on success, or an error message on failure.
#[tauri::command]
#[specta::specta]
pub fn test_llm_connection(app: AppHandle) -> Result<String, String> {
    crate::llm::test_connection(&app)
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

/// Validate + persist + emit the autostart-enabled toggle, then reconcile the
/// OS-level launch-agent registration to match.
///
/// Persistence always happens first. The autostart plugin call is best-effort:
/// if the OS rejects enabling/disabling the launch agent we log the error but
/// do NOT fail the command, so the user's saved preference still sticks.
#[tauri::command]
#[specta::specta]
pub fn change_autostart_enabled(app: AppHandle, autostart_enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let mut settings = get_settings(&app);
    settings.autostart_enabled = autostart_enabled;
    write_settings(&app, &settings);

    // Reconcile OS autostart (launch agent) with the new setting. Best-effort:
    // log on failure but still report success since the setting was persisted.
    let mgr = app.autolaunch();
    let result = if autostart_enabled {
        mgr.enable()
    } else {
        mgr.disable()
    };
    if let Err(e) = result {
        log::error!("Failed to {} OS autostart: {e}", if autostart_enabled { "enable" } else { "disable" });
    }

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

/// Result of inspecting the macOS "Press 🌐 to…" (Globe/Fn key) behavior.
///
/// `ok` is `true` when Fn is safe to use as a push-to-talk trigger — i.e. the
/// system setting is "Do Nothing" (`current == 0`) OR the key could not be read
/// (we fail open so the UI never nags wrongly). `current` is the raw
/// `AppleFnUsageType` value (`0` = Do Nothing, `1` = Change Input Source,
/// `2` = Show Emoji & Symbols on older macOS, `3` = Show Emoji & Symbols /
/// "Start Dictation"-style behaviors on newer macOS, etc.); `-1` means the key
/// was absent or unreadable.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FnKeyBehavior {
    /// `true` if Fn is safe to use for push-to-talk (setting is "Do Nothing" or unknown).
    pub ok: bool,
    /// Raw `AppleFnUsageType` value; `-1` if absent/unreadable.
    pub current: i64,
}

/// Detect the macOS "Press 🌐 to…" setting so the frontend can show a one-time
/// nudge when Fn won't work as a push-to-talk trigger.
///
/// macOS stores this as `AppleFnUsageType` in the `com.apple.HIToolbox` defaults
/// domain. `0` means "Do Nothing" (the value Stenographer needs); any non-zero
/// value (Change Input Source, Show Emoji & Symbols, Start Dictation, …) will
/// intercept the Fn key before our CGEventTap sees it.
///
/// Resilience: if the key is missing or unparseable we return `ok: true,
/// current: -1` so the app does not nag users whose setup we can't read.
/// Non-macOS always returns `ok: true`.
#[tauri::command]
#[specta::specta]
pub fn check_fn_key_behavior() -> FnKeyBehavior {
    #[cfg(target_os = "macos")]
    {
        match read_apple_fn_usage_type() {
            Some(value) => FnKeyBehavior {
                ok: value == 0,
                current: value,
            },
            // Key absent / unreadable → fail open (don't nag).
            None => FnKeyBehavior {
                ok: true,
                current: -1,
            },
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        FnKeyBehavior {
            ok: true,
            current: -1,
        }
    }
}

/// Reads `AppleFnUsageType` from the `com.apple.HIToolbox` defaults domain via
/// the `defaults` CLI. Returns `None` if the command fails, the key is absent,
/// or the value can't be parsed as an integer.
#[cfg(target_os = "macos")]
fn read_apple_fn_usage_type() -> Option<i64> {
    let output = std::process::Command::new("defaults")
        .args(["read", "com.apple.HIToolbox", "AppleFnUsageType"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().parse::<i64>().ok()
}

/// Return a snapshot of the process-global pipeline metrics for the settings
/// window's observability panel.
#[tauri::command]
#[specta::specta]
pub fn get_metrics() -> Metrics {
    crate::metrics::snapshot()
}

/// Return the last `lines` lines of the application log file, newest line LAST
/// (i.e. chronological order, matching how the file is written).
///
/// The log is produced by `tauri-plugin-log`'s `LogDir` target with
/// `file_name: Some("stenographer")`, so the file is `stenographer.log` inside
/// the app's log directory. We resolve it via `app.path().app_log_dir()`, and
/// fall back to the standard macOS path
/// `~/Library/Logs/dev.brijesh.stenographer/stenographer.log` if that fails or
/// the file is absent. `lines` is capped at 1000. A missing/unreadable file
/// yields an empty vec.
#[tauri::command]
#[specta::specta]
pub fn get_recent_logs(app: AppHandle, lines: u32) -> Vec<String> {
    const MAX_LINES: u32 = 1000;
    const LOG_FILE_NAME: &str = "stenographer.log";

    let cap = lines.min(MAX_LINES) as usize;
    if cap == 0 {
        return Vec::new();
    }

    // Candidate paths: the resolved app log dir first, then the standard macOS
    // location as a fallback.
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(dir) = app.path().app_log_dir() {
        candidates.push(dir.join(LOG_FILE_NAME));
    }
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            std::path::Path::new(&home)
                .join("Library/Logs/dev.brijesh.stenographer")
                .join(LOG_FILE_NAME),
        );
    }

    let contents = candidates
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok());

    let Some(contents) = contents else {
        return Vec::new();
    };

    let all: Vec<&str> = contents.lines().collect();
    let start = all.len().saturating_sub(cap);
    all[start..].iter().map(|s| s.to_string()).collect()
}
