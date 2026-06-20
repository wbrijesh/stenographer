mod commands;
mod settings;

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
        commands::show_main_window,
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

            // Create the main settings window programmatically (hidden initially)
            // rather than declaratively in tauri.conf.json. This mirrors Handy and
            // keeps `app.windows` empty in the config.
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("/".into()))
                .title("Stenographer")
                .inner_size(680.0, 570.0)
                .min_inner_size(680.0, 570.0)
                .resizable(true)
                .maximizable(false)
                .visible(false)
                .build()?;

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
