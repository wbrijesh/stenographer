//! Menu bar / tray icon for Stenographer (macOS).
//!
//! Ported from Handy (MIT) `src-tauri/src/tray.rs`, trimmed to the Stenographer
//! contract: English-only strings, a state-aware (Idle/Recording/Transcribing)
//! template tray icon, and a menu exposing Settings, a model submenu, Cancel,
//! and Quit.
//!
//! ## Wiring
//! `create_tray` is called once from the Tauri `setup` hook in `lib.rs`. It
//! builds the [`TrayIcon`], installs the menu-event handler, and `manage`s the
//! returned [`TrayIcon`] so [`set_state`] can later fetch it via
//! `app.state::<TrayIcon>()`.
//!
//! Menu callbacks are dispatched in [`on_menu_event`]:
//! * **Settings** -> calls [`crate::show_main_window`] directly.
//! * **Cancel** -> emits the [`TRAY_CANCEL_EVENT`] event for the pipeline
//!   coordinator to handle (it owns the in-flight recording/transcription).
//! * **Model select** -> records the chosen model id, emits
//!   [`TRAY_MODEL_SELECT_EVENT`] with the model id payload, and rebuilds the
//!   menu so the checkmark moves. The integration layer listens for this event
//!   to actually switch the active model.
//! * **Quit** -> `app.exit(0)`.
//!
//! ## Model submenu population
//! The tray does not depend on the (concurrently authored) `ModelManager`.
//! Instead the integration layer pushes the model list and the active model id
//! into a process-global registry via [`set_models`] / [`set_active_model`],
//! then calls [`refresh_menu`] (or relies on [`set_state`], which rebuilds the
//! menu) to repaint. Until populated, the submenu shows a single disabled
//! "No models" placeholder.

#![allow(dead_code)]

use std::sync::{Mutex, OnceLock};

use log::{error, info, warn};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager};

/// Event emitted when the user clicks "Cancel" in the tray while a recording or
/// transcription is in flight. The pipeline coordinator should listen for this.
pub const TRAY_CANCEL_EVENT: &str = "tray-cancel";

/// Event emitted when the user picks a model from the tray submenu. The payload
/// is the selected model id (`String`). The integration layer listens for this
/// to switch the active transcription model.
pub const TRAY_MODEL_SELECT_EVENT: &str = "tray-model-select";

/// Prefix used for per-model menu item ids, e.g. `model_select:whisper-small`.
const MODEL_SELECT_PREFIX: &str = "model_select:";

/// Embedded template tray icons (compiled into the binary so they are always
/// available regardless of resource bundling). These are 64x64 RGBA template
/// PNGs; on macOS the OS tints them to match the menu-bar appearance.
const ICON_IDLE: &[u8] = include_bytes!("../icons/tray/idle.png");
const ICON_RECORDING: &[u8] = include_bytes!("../icons/tray/recording.png");
const ICON_TRANSCRIBING: &[u8] = include_bytes!("../icons/tray/transcribing.png");

/// Current visual state of the tray icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayState {
    Idle,
    Recording,
    Transcribing,
}

/// A single entry shown in the tray model submenu.
#[derive(Clone, Debug)]
pub struct TrayModel {
    pub id: String,
    pub name: String,
}

/// Process-global tray model registry, populated by the integration layer.
#[derive(Default)]
struct ModelRegistry {
    models: Vec<TrayModel>,
    active: Option<String>,
}

fn registry() -> &'static Mutex<ModelRegistry> {
    static REGISTRY: OnceLock<Mutex<ModelRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(ModelRegistry::default()))
}

/// Replace the list of models shown in the tray submenu.
///
/// Call [`refresh_menu`] (or [`set_state`]) afterwards to repaint the tray.
pub fn set_models(models: Vec<TrayModel>) {
    if let Ok(mut reg) = registry().lock() {
        reg.models = models;
    }
}

/// Set the active (checked) model id shown in the tray submenu.
///
/// Call [`refresh_menu`] (or [`set_state`]) afterwards to repaint the tray.
pub fn set_active_model(model_id: Option<String>) {
    if let Ok(mut reg) = registry().lock() {
        reg.active = model_id;
    }
}

fn version_label() -> String {
    if cfg!(debug_assertions) {
        format!("Stenographer v{} (Dev)", env!("CARGO_PKG_VERSION"))
    } else {
        format!("Stenographer v{}", env!("CARGO_PKG_VERSION"))
    }
}

/// Resolve the [`Image`] for the given state, embedding template PNGs.
///
/// Falls back to the app's default window icon (and logs) if the embedded
/// bytes ever fail to decode, so the tray never panics at runtime.
fn icon_for_state<'a>(app: &'a AppHandle, state: TrayState) -> Option<Image<'a>> {
    let bytes = match state {
        TrayState::Idle => ICON_IDLE,
        TrayState::Recording => ICON_RECORDING,
        TrayState::Transcribing => ICON_TRANSCRIBING,
    };

    match Image::from_bytes(bytes) {
        Ok(image) => Some(image),
        Err(err) => {
            warn!(
                "Failed to decode embedded tray icon for {:?}: {}. Falling back to default icon.",
                state, err
            );
            app.default_window_icon().cloned()
        }
    }
}

/// Build the tray menu for the given state.
///
/// `Idle` shows the model submenu; `Recording`/`Transcribing` show a Cancel
/// item (so the user can abort an in-flight pipeline) instead.
fn build_menu(app: &AppHandle, state: TrayState) -> tauri::Result<Menu<tauri::Wry>> {
    // macOS accelerators.
    let settings_accelerator = Some("Cmd+,");
    let quit_accelerator = Some("Cmd+Q");

    let version_i = MenuItem::with_id(app, "version", version_label(), false, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "Settings", true, settings_accelerator)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, quit_accelerator)?;
    let separator = PredefinedMenuItem::separator(app)?;

    match state {
        TrayState::Recording | TrayState::Transcribing => {
            let cancel_i = MenuItem::with_id(app, "cancel", "Cancel", true, None::<&str>)?;
            Menu::with_items(
                app,
                &[
                    &version_i,
                    &PredefinedMenuItem::separator(app)?,
                    &cancel_i,
                    &PredefinedMenuItem::separator(app)?,
                    &settings_i,
                    &separator,
                    &quit_i,
                ],
            )
        }
        TrayState::Idle => {
            let model_submenu = build_model_submenu(app)?;
            Menu::with_items(
                app,
                &[
                    &version_i,
                    &PredefinedMenuItem::separator(app)?,
                    &model_submenu,
                    &PredefinedMenuItem::separator(app)?,
                    &settings_i,
                    &separator,
                    &quit_i,
                ],
            )
        }
    }
}

/// Build the model submenu from the process-global registry. The submenu label
/// is the active model's name (or "Model" when nothing is selected). Each model
/// is a [`CheckMenuItem`]; the active one is checked.
fn build_model_submenu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let reg = registry().lock().ok();
    let (models, active): (Vec<TrayModel>, Option<String>) = match reg {
        Some(r) => (r.models.clone(), r.active.clone()),
        None => (Vec::new(), None),
    };

    let label = active
        .as_ref()
        .and_then(|id| models.iter().find(|m| &m.id == id))
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "Model".to_string());

    let submenu = Submenu::with_id(app, "model_submenu", label, true)?;

    if models.is_empty() {
        // Placeholder until the integration layer populates the registry.
        let placeholder =
            MenuItem::with_id(app, "no_models", "No models", false, None::<&str>)?;
        submenu.append(&placeholder)?;
        return Ok(submenu);
    }

    for model in &models {
        let is_active = active.as_deref() == Some(model.id.as_str());
        let item_id = format!("{MODEL_SELECT_PREFIX}{}", model.id);
        let item =
            CheckMenuItem::with_id(app, &item_id, &model.name, true, is_active, None::<&str>)?;
        submenu.append(&item)?;
    }

    Ok(submenu)
}

/// Dispatch a tray menu click. Settings is handled inline; everything else is
/// forwarded to the rest of the app via events so the tray stays decoupled from
/// the pipeline and model subsystems.
fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id.as_ref();

    match id {
        "settings" => crate::show_main_window(app),
        "cancel" => {
            if let Err(e) = app.emit(TRAY_CANCEL_EVENT, ()) {
                error!("Failed to emit {TRAY_CANCEL_EVENT}: {e}");
            }
        }
        "quit" => {
            info!("Quit requested from tray.");
            app.exit(0);
        }
        other if other.starts_with(MODEL_SELECT_PREFIX) => {
            let model_id = other[MODEL_SELECT_PREFIX.len()..].to_string();
            info!("Tray model select: {model_id}");
            set_active_model(Some(model_id.clone()));
            if let Err(e) = app.emit(TRAY_MODEL_SELECT_EVENT, model_id) {
                error!("Failed to emit {TRAY_MODEL_SELECT_EVENT}: {e}");
            }
            // Repaint so the checkmark follows the new selection.
            refresh_menu(app);
        }
        _ => {}
    }
}

/// Rebuild and reinstall the tray menu for the current Idle state. Useful after
/// the model registry changes without a state transition.
pub fn refresh_menu(app: &AppHandle) {
    if let Some(tray) = app.try_state::<TrayIcon>() {
        match build_menu(app, TrayState::Idle) {
            Ok(menu) => {
                if let Err(e) = tray.set_menu(Some(menu)) {
                    error!("Failed to refresh tray menu: {e}");
                }
            }
            Err(e) => error!("Failed to build tray menu: {e}"),
        }
    }
}

/// Create the tray icon, install the menu-event handler, and register the
/// resulting [`TrayIcon`] in Tauri's state so [`set_state`] can reach it.
///
/// Called once from the `setup` hook in `lib.rs`.
pub fn create_tray(app: &AppHandle) -> Result<TrayIcon, String> {
    let menu = build_menu(app, TrayState::Idle).map_err(|e| format!("build menu: {e}"))?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip(version_label())
        .icon_as_template(true)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu_event(app, event));

    if let Some(icon) = icon_for_state(app, TrayState::Idle) {
        builder = builder.icon(icon);
    } else {
        warn!("No tray icon available (embedded + default both failed); tray will be iconless.");
    }

    let tray = builder.build(app).map_err(|e| format!("build tray: {e}"))?;

    // Register so `set_state` / `refresh_menu` can fetch it via app state.
    // If a TrayIcon is already managed (shouldn't happen), `manage` returns
    // false; we ignore that and keep the freshly built handle for the caller.
    let _ = app.manage(tray.clone());

    info!("Tray icon created.");
    Ok(tray)
}

/// Swap the tray icon and rebuild the menu for the given [`TrayState`].
///
/// Resilient: if the icon can't be set the menu is still updated, and nothing
/// panics. No-op (with a log) if the tray hasn't been created yet.
pub fn set_state(app: &AppHandle, state: TrayState) {
    let Some(tray) = app.try_state::<TrayIcon>() else {
        warn!("set_state called before tray was created; ignoring.");
        return;
    };

    if let Some(icon) = icon_for_state(app, state) {
        if let Err(e) = tray.set_icon(Some(icon)) {
            error!("Failed to set tray icon for {state:?}: {e}");
        }
    }
    // Keep template tinting on across swaps.
    let _ = tray.set_icon_as_template(true);

    match build_menu(app, state) {
        Ok(menu) => {
            if let Err(e) = tray.set_menu(Some(menu)) {
                error!("Failed to set tray menu for {state:?}: {e}");
            }
        }
        Err(e) => error!("Failed to build tray menu for {state:?}: {e}"),
    }
}
