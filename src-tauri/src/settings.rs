use log::{debug, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnloadTimeout {
    Never,
    Immediately,
    Seconds(u64),
    Minutes(u64),
}

impl Default for ModelUnloadTimeout {
    fn default() -> Self {
        ModelUnloadTimeout::Minutes(5)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, Type)]
#[serde(rename_all = "snake_case")]
pub enum AutoSubmitKey {
    #[default]
    Enter,
    CtrlEnter,
    CmdEnter,
}

/// Application settings persisted via `tauri-plugin-store` to
/// `settings_store.json` under the single `"settings"` key.
///
/// Every field carries a `#[serde(default = "...")]` so that settings written
/// by an older build (missing newer fields) still deserialize cleanly.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppSettings {
    // trigger
    #[serde(default = "default_trigger_mode_enabled")]
    pub trigger_mode_enabled: bool,
    #[serde(default = "default_hold_tap_threshold_ms")]
    pub hold_tap_threshold_ms: u64,
    /// The configurable trigger binding, parsed via `handy_keys::Hotkey`.
    /// Modifier-only bindings (e.g. `"CmdRight"`) use tap detection; bindings
    /// with a key (e.g. `"Ctrl+Shift+R"`) fire on the key-down combo.
    #[serde(default = "default_trigger_binding")]
    pub trigger_binding: String,

    // audio
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub selected_output_device: Option<String>,
    #[serde(default = "default_audio_feedback")]
    pub audio_feedback: bool,
    #[serde(default = "default_audio_feedback_volume")]
    pub audio_feedback_volume: f32,
    #[serde(default = "default_mute_while_recording")]
    pub mute_while_recording: bool,

    // transcription / models
    #[serde(default)]
    pub selected_model: Option<String>,
    #[serde(default = "default_selected_language")]
    pub selected_language: String,
    #[serde(default = "default_translate_to_english")]
    pub translate_to_english: bool,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,

    // output / paste
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
    #[serde(default = "default_auto_submit")]
    pub auto_submit: bool,
    #[serde(default)]
    pub auto_submit_key: AutoSubmitKey,
    #[serde(default = "default_append_trailing_space")]
    pub append_trailing_space: bool,

    // overlay / app
    #[serde(default = "default_overlay_enabled")]
    pub overlay_enabled: bool,
    #[serde(default = "default_start_hidden")]
    pub start_hidden: bool,
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,
    #[serde(default = "default_show_tray_icon")]
    pub show_tray_icon: bool,
}

fn default_trigger_mode_enabled() -> bool {
    true
}
fn default_hold_tap_threshold_ms() -> u64 {
    250
}
/// Default trigger binding: Right Command (right ⌘).
///
/// Verified to round-trip through `handy_keys::Hotkey`:
/// `Hotkey::from_str("CmdRight").to_string() == "CmdRight"`.
fn default_trigger_binding() -> String {
    "CmdRight".to_string()
}
fn default_audio_feedback() -> bool {
    true
}
fn default_audio_feedback_volume() -> f32 {
    1.0
}
fn default_mute_while_recording() -> bool {
    false
}
fn default_selected_language() -> String {
    "auto".to_string()
}
fn default_translate_to_english() -> bool {
    false
}
fn default_paste_delay_ms() -> u64 {
    60
}
fn default_auto_submit() -> bool {
    false
}
fn default_append_trailing_space() -> bool {
    false
}
fn default_overlay_enabled() -> bool {
    true
}
fn default_start_hidden() -> bool {
    true
}
fn default_autostart_enabled() -> bool {
    false
}
fn default_show_tray_icon() -> bool {
    true
}

pub fn get_default_settings() -> AppSettings {
    AppSettings {
        trigger_mode_enabled: default_trigger_mode_enabled(),
        hold_tap_threshold_ms: default_hold_tap_threshold_ms(),
        trigger_binding: default_trigger_binding(),
        selected_microphone: None,
        selected_output_device: None,
        audio_feedback: default_audio_feedback(),
        audio_feedback_volume: default_audio_feedback_volume(),
        mute_while_recording: default_mute_while_recording(),
        selected_model: None,
        selected_language: default_selected_language(),
        translate_to_english: default_translate_to_english(),
        model_unload_timeout: ModelUnloadTimeout::default(),
        paste_delay_ms: default_paste_delay_ms(),
        auto_submit: default_auto_submit(),
        auto_submit_key: AutoSubmitKey::default(),
        append_trailing_space: default_append_trailing_space(),
        overlay_enabled: default_overlay_enabled(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        show_tray_icon: default_show_tray_icon(),
    }
}

/// Load settings from the store, creating defaults if absent and falling back
/// to defaults (and rewriting the store) if the stored value fails to parse.
pub fn load_or_create_app_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize settings store");

    if let Some(settings_value) = store.get("settings") {
        match serde_json::from_value::<AppSettings>(settings_value) {
            Ok(settings) => {
                debug!("Loaded existing settings: {:?}", settings);
                settings
            }
            Err(e) => {
                warn!("Failed to parse settings, falling back to defaults: {}", e);
                let defaults = get_default_settings();
                store.set("settings", serde_json::to_value(&defaults).unwrap());
                defaults
            }
        }
    } else {
        let defaults = get_default_settings();
        store.set("settings", serde_json::to_value(&defaults).unwrap());
        defaults
    }
}

/// Read the current settings, falling back to defaults on parse failure.
pub fn get_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize settings store");

    if let Some(settings_value) = store.get("settings") {
        serde_json::from_value::<AppSettings>(settings_value).unwrap_or_else(|_| {
            let defaults = get_default_settings();
            store.set("settings", serde_json::to_value(&defaults).unwrap());
            defaults
        })
    } else {
        let defaults = get_default_settings();
        store.set("settings", serde_json::to_value(&defaults).unwrap());
        defaults
    }
}

/// Persist the full settings object back to the store.
pub fn write_settings(app: &AppHandle, settings: &AppSettings) {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize settings store");
    store.set("settings", serde_json::to_value(settings).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_contract() {
        let s = get_default_settings();
        assert!(s.trigger_mode_enabled);
        assert_eq!(s.hold_tap_threshold_ms, 250);
        assert_eq!(s.trigger_binding, "CmdRight");
        assert!(s.audio_feedback);
        assert_eq!(s.audio_feedback_volume, 1.0);
        assert_eq!(s.paste_delay_ms, 60);
        assert_eq!(s.selected_language, "auto");
        assert_eq!(s.auto_submit_key, AutoSubmitKey::Enter);
        assert_eq!(s.model_unload_timeout, ModelUnloadTimeout::Minutes(5));
        assert!(s.overlay_enabled);
        assert!(s.start_hidden);
        assert!(!s.autostart_enabled);
        assert!(s.show_tray_icon);
    }
}
