//! Thin command wrappers over `tauri_plugin_macos_permissions` for the
//! onboarding UI.
//!
//! The underlying plugin already exposes async functions; we re-wrap them as
//! our own `#[tauri::command]`s so they participate in the tauri-specta binding
//! generation (giving the frontend typed `commands.checkAccessibilityPermission`
//! etc.) and so we can normalise return types.
//!
//! All real work is macOS-only. On other platforms we return sensible defaults
//! (permission granted / `Ok`) so the crate still compiles cross-platform, even
//! though Stenographer only ships on macOS.

/// Whether the app currently has macOS Accessibility (AX) permission.
///
/// Required for the global Fn key listener and synthetic keystroke injection.
/// Pure check — does NOT prompt the user.
#[tauri::command]
#[specta::specta]
pub async fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        tauri_plugin_macos_permissions::check_accessibility_permission().await
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request macOS Accessibility permission.
///
/// On macOS this triggers the system "grant Accessibility" prompt and opens the
/// relevant System Settings pane. The grant is asynchronous and user-driven, so
/// callers should poll [`check_accessibility_permission`] afterwards rather than
/// assume success on return.
#[tauri::command]
#[specta::specta]
pub async fn request_accessibility_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tauri_plugin_macos_permissions::request_accessibility_permission().await;
    }
    Ok(())
}

/// Whether the app currently has macOS Microphone permission.
///
/// Pure check — does NOT prompt the user.
#[tauri::command]
#[specta::specta]
pub async fn check_microphone_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        tauri_plugin_macos_permissions::check_microphone_permission().await
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request macOS Microphone permission.
///
/// On macOS this calls `AVCaptureDevice requestAccessForMediaType`, which shows
/// the system microphone prompt the first time it is called for this app. The
/// result is delivered asynchronously to the user, so callers should poll
/// [`check_microphone_permission`] afterwards rather than assume success on
/// return.
#[tauri::command]
#[specta::specta]
pub async fn request_microphone_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tauri_plugin_macos_permissions::request_microphone_permission().await?;
    }
    Ok(())
}
