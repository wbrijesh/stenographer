// Allow not-yet-wired modules during the phased build.
#![allow(dead_code)]

mod commands;
mod settings;

// Phase 1 subsystem modules (bodies implemented by delegated agents).
mod audio_feedback;
mod audio_toolkit;
mod clipboard;
mod coordinator;
mod input;
mod llm;
mod managers;
mod metrics;
mod overlay;
mod pipeline;
mod shortcut;
mod tray;

use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_log::{Builder as LogBuilder, Target, TargetKind};
use tauri_specta::{collect_commands, Builder};

use crate::settings::load_or_create_app_settings;

/// Construct the tauri-specta `Builder` with the full command surface.
///
/// This is shared between [`run`] (which mounts the handler) and the
/// `export_bindings` test (which serializes the bindings to TypeScript). New
/// commands should be added here via `collect_commands!`.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::get_app_settings,
        commands::get_default_settings,
        commands::change_paste_delay_ms,
        commands::change_overlay_enabled,
        commands::change_trigger_mode_enabled,
        commands::change_hold_tap_threshold_ms,
        commands::set_selected_microphone,
        commands::set_selected_output_device,
        commands::change_audio_feedback,
        commands::change_audio_feedback_volume,
        commands::change_mute_while_recording,
        commands::change_selected_model,
        commands::change_selected_language,
        commands::change_translate_to_english,
        commands::change_model_unload_timeout,
        commands::change_auto_submit,
        commands::change_auto_submit_key,
        commands::change_append_trailing_space,
        commands::is_cleanup_configured,
        commands::change_cleanup_enabled,
        commands::change_llm_base_url,
        commands::change_llm_api_key,
        commands::change_llm_model,
        commands::test_llm_connection,
        commands::change_start_hidden,
        commands::change_autostart_enabled,
        commands::change_show_tray_icon,
        commands::show_main_window,
        commands::open_settings,
        commands::check_fn_key_behavior,
        commands::get_metrics,
        commands::get_recent_logs,
        // --- audio devices / lifecycle / permissions ---
        commands::audio::get_available_microphones,
        commands::audio::get_available_output_devices,
        commands::audio::is_recording,
        commands::audio::play_test_sound,
        commands::audio::initialize_enigo,
        commands::audio::initialize_shortcuts,
        commands::audio::cancel_operation,
        commands::audio::get_trigger_binding,
        commands::audio::change_trigger_binding,
        commands::audio::capture_shortcut,
        // --- model management ---
        commands::models::get_available_models,
        commands::models::download_model,
        commands::models::cancel_download,
        commands::models::delete_model,
        commands::models::set_active_model,
        commands::models::get_current_model,
        commands::models::is_model_loading,
        commands::models::set_model_unload_timeout,
        commands::models::unload_model_manually,
        // --- macOS permission wrappers (onboarding UI) ---
        commands::permissions::check_accessibility_permission,
        commands::permissions::request_accessibility_permission,
        commands::permissions::check_microphone_permission,
        commands::permissions::request_microphone_permission,
    ])
}

/// Show (and focus) the main settings window. On macOS this restores the
/// Regular activation policy so the Dock icon reappears while the window is up.
pub fn show_main_window(app: &AppHandle) {
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.unminimize();
        let _ = main_window.show();
        let _ = main_window.set_focus();

        #[cfg(target_os = "macos")]
        {
            if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
                log::error!("Failed to set activation policy to Regular: {}", e);
            }
        }
        return;
    }
    log::error!("Main window not found");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    export_bindings(&builder);

    let invoke_handler = builder.invoke_handler();

    #[allow(unused_mut)]
    let mut tauri_builder = tauri::Builder::default()
        .plugin(
            LogBuilder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("stenographer".into()),
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ));

    #[cfg(target_os = "macos")]
    {
        tauri_builder = tauri_builder.plugin(tauri_nspanel::init());
    }

    tauri_builder
        .invoke_handler(invoke_handler)
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Load (or create) settings before any window logic.
            let settings = load_or_create_app_settings(&app_handle);

            // Reconcile OS autostart (launch agent) with the persisted setting,
            // mirroring Handy. Best-effort: log on failure, never block startup.
            {
                use tauri_plugin_autostart::ManagerExt;
                let mgr = app_handle.autolaunch();
                let result = if settings.autostart_enabled {
                    mgr.enable()
                } else {
                    mgr.disable()
                };
                if let Err(e) = result {
                    log::error!(
                        "Failed to reconcile OS autostart at startup (autostart_enabled={}): {e}",
                        settings.autostart_enabled
                    );
                }
            }

            // Create the main settings window programmatically (hidden initially)
            // rather than declaratively in tauri.conf.json. This mirrors Handy and
            // keeps `app.windows` empty in the config.
            //
            // Closing the settings window should not quit the app (the tray keeps
            // it alive, menu-bar style). Instead we hide the window and, on macOS
            // when started hidden, restore the Accessory policy so the Dock icon
            // disappears again (`show_main_window` flips it to Regular on open).
            let close_handle = app_handle.clone();
            #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
            let close_start_hidden = settings.start_hidden;
            let main_window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("/".into()),
            )
            .title("Stenographer")
            .inner_size(680.0, 570.0)
            .min_inner_size(680.0, 570.0)
            .resizable(true)
            .maximizable(false)
            .visible(false)
            // Transparent background is required for the native macOS vibrancy
            // (NSVisualEffectView) applied below to show through.
            .transparent(true)
            .build()?;

            // macOS: apply a native translucent vibrancy background to the main
            // settings window so it looks like a System-Settings-style native app.
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{
                    apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
                };
                if let Some(win) = app_handle.get_webview_window("main") {
                    if let Err(e) = apply_vibrancy(
                        &win,
                        NSVisualEffectMaterial::Sidebar,
                        Some(NSVisualEffectState::Active),
                        None,
                    ) {
                        log::error!("Failed to apply window vibrancy: {e}");
                    }
                }
            }

            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(main_window) = close_handle.get_webview_window("main") {
                        let _ = main_window.hide();
                    }

                    #[cfg(target_os = "macos")]
                    {
                        if close_start_hidden {
                            if let Err(e) = close_handle
                                .set_activation_policy(tauri::ActivationPolicy::Accessory)
                            {
                                log::error!(
                                    "Failed to set activation policy to Accessory: {}",
                                    e
                                );
                            }
                        }
                    }
                }
            });

            // macOS: when starting hidden, run as an Accessory app (no Dock icon).
            #[cfg(target_os = "macos")]
            {
                if settings.start_hidden {
                    let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }

            // Show the window immediately unless configured to start hidden.
            if !settings.start_hidden {
                show_main_window(&app_handle);
            }

            // --- Managed state: managers + coordinator -----------------------
            //
            // Construction order matters: the TranscriptionManager needs the
            // ModelManager Arc. ModelManager / TranscriptionManager return
            // Result; if either fails we log and bail out of setup (the app is
            // unusable without them).
            use crate::managers::audio::AudioRecordingManager;
            use crate::managers::model::ModelManager;
            use crate::managers::transcription::TranscriptionManager;

            let model_manager = Arc::new(ModelManager::new(&app_handle)?);
            let transcription_manager =
                Arc::new(TranscriptionManager::new(&app_handle, model_manager.clone())?);
            let audio = Arc::new(AudioRecordingManager::new(app_handle.clone()));
            audio.preload_vad();
            let coordinator = Arc::new(crate::coordinator::Coordinator::new(app_handle.clone()));

            app.manage(model_manager.clone());
            app.manage(transcription_manager.clone());
            app.manage(audio.clone());
            app.manage(coordinator.clone());
            app.manage(crate::commands::audio::FnListenerState::default());

            // --- Overlay panel (hidden until recording) ----------------------
            crate::overlay::create_overlay(&app_handle);

            // --- Tray icon + model submenu -----------------------------------
            if settings.show_tray_icon {
                match crate::tray::create_tray(&app_handle) {
                    Ok(_) => {
                        let tray_models: Vec<crate::tray::TrayModel> = model_manager
                            .get_available_models()
                            .into_iter()
                            .filter(|m| m.is_downloaded)
                            .map(|m| crate::tray::TrayModel {
                                id: m.id,
                                name: m.name,
                            })
                            .collect();
                        crate::tray::set_models(tray_models);
                        crate::tray::set_active_model(settings.selected_model.clone());
                        crate::tray::refresh_menu(&app_handle);
                    }
                    Err(e) => log::error!("Failed to create tray icon: {e}"),
                }
            }

            // --- Wire the real pipeline + control-event listeners ------------
            coordinator.set_actions(crate::pipeline::build_pipeline_actions(app_handle.clone()));
            crate::pipeline::install_event_listeners(&app_handle);

            // --- Accessibility-gated startup ---------------------------------
            //
            // If Accessibility is ALREADY granted, initialize enigo (main
            // thread) and start the Fn listener now, storing its handle in
            // managed state. If not granted, do nothing — the onboarding flow
            // (`initialize_enigo` / `initialize_shortcuts` commands) starts them
            // later. We never trigger a permission prompt here.
            #[cfg(target_os = "macos")]
            {
                let accessibility_granted = tauri::async_runtime::block_on(
                    tauri_plugin_macos_permissions::check_accessibility_permission(),
                );

                if accessibility_granted {
                    // Enigo construction must run on the main thread.
                    let enigo_app = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        if let Err(e) = crate::input::init_enigo(enigo_app) {
                            log::error!("init_enigo failed despite Accessibility granted: {e}");
                        }
                    });

                    match crate::shortcut::start_fn_listener(
                        app_handle.clone(),
                        coordinator.clone(),
                    ) {
                        Ok(handle) => {
                            if let Some(state) =
                                app_handle.try_state::<crate::commands::audio::FnListenerState>()
                            {
                                if let Ok(mut guard) = state.0.lock() {
                                    *guard = Some(handle);
                                }
                            }
                            log::info!("Fn listener started at launch (Accessibility granted)");
                        }
                        Err(e) => log::error!("Failed to start Fn listener at launch: {e}"),
                    }
                } else {
                    log::info!(
                        "Accessibility not granted; deferring enigo/Fn listener to onboarding"
                    );
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}

/// Export TypeScript bindings to `../src/bindings.ts`.
///
/// Invoked automatically from [`run`] in debug builds and from the
/// `export_bindings` test so the file can be generated without launching the
/// GUI (which is not permitted in CI).
#[cfg(debug_assertions)]
fn export_bindings(builder: &Builder<tauri::Wry>) {
    use specta_typescript::{BigIntExportBehavior, Typescript};

    builder
        .export(
            Typescript::default().bigint(BigIntExportBehavior::Number),
            "../src/bindings.ts",
        )
        .expect("Failed to export TypeScript bindings");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generates `src/bindings.ts`. Runs via `cargo test` so the bindings can be
    /// produced without launching the Tauri GUI.
    #[test]
    fn export_bindings_test() {
        export_bindings(&specta_builder());
    }
}
