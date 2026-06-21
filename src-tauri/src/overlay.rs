//! Recording overlay NSPanel — the "pill" shown while recording / transcribing.
//!
//! This is the app's headline differentiator: a non-activating `NSPanel` that
//! behaves correctly under the **Aerospace** tiling window manager. macOS-only.
//!
//! ## Aerospace correctness
//! Tiling WMs like Aerospace aggressively re-arrange windows and switch Spaces
//! out from under apps. To stay visible on the *currently active* Space without
//! ever stealing focus we:
//!   * Build a **non-activating floating panel** (`is_floating_panel: true`) at
//!     `PanelLevel::Status`. The panel may become key (`can_become_key_window:
//!     true`) so the editable transcript can be clicked into and typed — focus
//!     stays safe because showing uses `orderFrontRegardless:` (never make-key)
//!     and the `NonactivatingPanel` mask keeps the app from activating on click.
//!   * Give it a collection behavior of
//!     `canJoinAllSpaces | fullScreenAuxiliary | stationary | ignoresCycle`,
//!     so it is present on every Space (hence always on the active one), floats
//!     over full-screen apps, and is ignored by the WM's tiling/cycling. NOTE:
//!     these flags are picked to respect macOS's mutually-exclusive collection-
//!     behavior groups (see `overlay_collection_behavior`).
//!   * On **every** show, *re-apply* the collection behavior (re-homing it to the
//!     active Space), reposition it to the monitor under the cursor, and surface
//!     it via `orderFrontRegardless:` — **never** `makeKeyAndOrderFront:`, never
//!     activating the app, never touching the activation policy.
//!
//! ## Public API (CONTRACTS.md)
//! ```ignore
//! fn create_overlay(app: &AppHandle);      // build hidden panel at startup
//! fn show_recording(app: &AppHandle);
//! fn show_transcribing(app: &AppHandle);
//! fn hide_overlay(app: &AppHandle);
//! ```
//! Plus [`emit_mic_level`] for streaming the waveform amplitude.

#![allow(dead_code)]

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

#[cfg(target_os = "macos")]
use tauri::WebviewUrl;

#[cfg(target_os = "macos")]
use tauri_nspanel::objc2_app_kit::NSWindowStyleMask;
#[cfg(target_os = "macos")]
use tauri_nspanel::{tauri_panel, CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel};

// --- UI / layout constants ---------------------------------------------------

/// Webview entry for the overlay (second Vite entry point).
const OVERLAY_URL: &str = "src/overlay/index.html";
/// Window label used to retrieve the panel/webview later.
const OVERLAY_LABEL: &str = "recording_overlay";

/// Overlay pill dimensions, in logical points.
const OVERLAY_WIDTH: f64 = 440.0;
const OVERLAY_HEIGHT: f64 = 132.0;
/// Gap between the pill and the bottom edge of the monitor, in points.
const OVERLAY_BOTTOM_OFFSET: f64 = 56.0;

// --- Frontend event names (centralized strings) ------------------------------

/// Emitted to the overlay webview with a state payload (`recording` /
/// `transcribing`) to show the pill and select its visual state.
const EVENT_SHOW: &str = "show-overlay";
/// Emitted to the overlay webview to fade it out before it is hidden.
const EVENT_HIDE: &str = "hide-overlay";
/// Emitted to the overlay webview with an `f32` 0..1 amplitude for the waveform.
const EVENT_MIC_LEVEL: &str = "mic-level";

/// Overlay visual states sent as the `show-overlay` payload.
const STATE_RECORDING: &str = "recording";
const STATE_TRANSCRIBING: &str = "transcribing";

// --- Panel definition (macOS) ------------------------------------------------

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(RecordingOverlayPanel {
        config: {
            // The panel CAN become key so the user can click into the editable
            // transcript and type corrections. Focus stays safe because:
            //   * showing the panel uses `order_front_regardless()` (NOT make-key),
            //     so appearing the overlay never steals focus from the editor;
            //   * the `NonactivatingPanel` style mask (applied after the swizzle)
            //     means even when the user CLICKS the overlay to edit, the owning
            //     app is NOT activated and (with the Aerospace float rule) no
            //     workspace switch occurs. The panel only becomes key on an
            //     explicit click into the text field.
            // `is_floating_panel` keeps it a floating utility panel that does not
            // participate in the tiling layout.
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

/// Collection behavior that makes the panel a well-behaved, space-following
/// overlay under Aerospace: it joins every Space, floats over full-screen apps,
/// is stationary (the WM won't move it) and is kept out of the window cycle.
///
/// ## macOS mutual-exclusivity groups (DO NOT REINTRODUCE conflicts)
/// `NSWindowCollectionBehavior` enforces mutually-exclusive GROUPS; you may set
/// AT MOST ONE flag from each group. Setting two flags from the same group
/// triggers `*** Assertion failure in -[NSWindow _validateCollectionBehavior:]`
/// → an Obj-C exception that unwinds through tao's non-unwinding
/// `applicationDidFinishLaunching:` → `abort()` (a hard launch crash).
///
/// The relevant groups are:
///   * **Spaces:** `canJoinAllSpaces` XOR `moveToActiveSpace` — pick ONE.
///   * **Exposé/cycle (managed/transient/stationary):** at most one of
///     `managed`, `transient`, `stationary`.
///   * `ignoresCycle` / `participatesInCycle` are a SEPARATE group (window
///     cycle), so `ignores_cycle()` is safe alongside the above.
///
/// We choose `can_join_all_spaces()` (the panel is present on ALL Spaces, so it
/// is always on whatever Space is active — no Space switch needed, which is why
/// `move_to_active_space` is both unnecessary AND a conflicting flag) and
/// `stationary()` (the WM/Spaces/Exposé won't move it; `transient` would
/// conflict with `stationary`).
#[cfg(target_os = "macos")]
fn overlay_collection_behavior() -> CollectionBehavior {
    CollectionBehavior::new()
        .can_join_all_spaces()
        .full_screen_auxiliary()
        .stationary()
        .ignores_cycle()
}

// --- Cursor lookup -----------------------------------------------------------

/// Returns the current global cursor position in **top-left-origin logical
/// points** (matching Tauri's monitor coordinate convention), or `None` if it
/// can't be determined.
///
/// AppKit's `NSEvent.mouseLocation` reports the cursor in *bottom-left-origin*
/// screen coordinates, so we flip Y against the primary screen's height. The
/// primary screen is the one with origin `(0, 0)`; its frame height is the
/// reference for the global flip.
#[cfg(target_os = "macos")]
fn get_cursor_position(_app: &AppHandle) -> Option<(i32, i32)> {
    use tauri_nspanel::objc2::MainThreadMarker;
    use tauri_nspanel::objc2_app_kit::{NSEvent, NSScreen};

    // NSScreen access requires the main thread. If we're not on it, bail and let
    // the caller fall back to the primary monitor.
    let mtm = MainThreadMarker::new()?;

    let cursor = NSEvent::mouseLocation(); // bottom-left origin, logical points

    // Find the primary screen (origin at 0,0) to get the global flip height.
    let screens = NSScreen::screens(mtm);
    let mut primary_height: Option<f64> = None;
    for screen in screens.iter() {
        let frame = screen.frame();
        if frame.origin.x == 0.0 && frame.origin.y == 0.0 {
            primary_height = Some(frame.size.height);
            break;
        }
    }
    let primary_height = primary_height
        .or_else(|| NSScreen::mainScreen(mtm).map(|s| s.frame().size.height))?;

    let x = cursor.x as i32;
    let y = (primary_height - cursor.y) as i32;
    Some((x, y))
}

#[cfg(not(target_os = "macos"))]
fn get_cursor_position(_app: &AppHandle) -> Option<(i32, i32)> {
    None
}

// --- Monitor / positioning ---------------------------------------------------

/// Returns the monitor the cursor currently sits on, falling back to the
/// primary monitor. Position is computed from the *cursor's* screen so the
/// overlay appears on the display the user is actively working on.
fn get_monitor_with_cursor(app: &AppHandle) -> Option<tauri::Monitor> {
    if let Some(mouse_location) = get_cursor_position(app) {
        if let Ok(monitors) = app.available_monitors() {
            for monitor in monitors {
                // Tauri reports monitor geometry in physical pixels, while the
                // cursor position is logical. Normalize to logical points by
                // dividing by the per-monitor scale factor before comparing.
                let scale = monitor.scale_factor();
                let pos = PhysicalPosition::new(
                    (monitor.position().x as f64 / scale) as i32,
                    (monitor.position().y as f64 / scale) as i32,
                );
                let size = PhysicalSize::new(
                    (monitor.size().width as f64 / scale) as u32,
                    (monitor.size().height as f64 / scale) as u32,
                );
                if is_mouse_within_monitor(mouse_location, &pos, &size) {
                    return Some(monitor);
                }
            }
        }
    }

    app.primary_monitor().ok().flatten()
}

fn is_mouse_within_monitor(
    mouse_pos: (i32, i32),
    monitor_pos: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
) -> bool {
    let (mouse_x, mouse_y) = mouse_pos;
    let PhysicalPosition {
        x: monitor_x,
        y: monitor_y,
    } = *monitor_pos;
    let PhysicalSize {
        width: monitor_width,
        height: monitor_height,
    } = *monitor_size;

    mouse_x >= monitor_x
        && mouse_x < (monitor_x + monitor_width as i32)
        && mouse_y >= monitor_y
        && mouse_y < (monitor_y + monitor_height as i32)
}

/// Computes the **bottom-center** overlay position on the monitor under the
/// cursor, in logical points.
///
/// We deliberately use logical coordinates (not physical) because Tauri/tao
/// converts `PhysicalPosition` using the scale factor of the monitor the window
/// is *currently* on, which is wrong when moving the panel across monitors with
/// different scale factors.
fn calculate_overlay_position(app: &AppHandle) -> Option<(f64, f64)> {
    let monitor = get_monitor_with_cursor(app)?;
    let scale = monitor.scale_factor();
    let monitor_x = monitor.position().x as f64 / scale;
    let monitor_y = monitor.position().y as f64 / scale;
    let monitor_width = monitor.size().width as f64 / scale;
    let monitor_height = monitor.size().height as f64 / scale;

    let x = monitor_x + (monitor_width - OVERLAY_WIDTH) / 2.0;
    let y = monitor_y + monitor_height - OVERLAY_HEIGHT - OVERLAY_BOTTOM_OFFSET;

    Some((x, y))
}

// --- Public API: create -------------------------------------------------------

/// Builds the recording overlay panel at startup and keeps it hidden.
///
/// The `PanelBuilder` creates a Tauri window then converts it into an
/// `NSPanel`; the window stays registered, so `get_webview_window(LABEL)` and
/// `get_webview_panel(LABEL)` both keep working afterwards.
#[cfg(target_os = "macos")]
pub fn create_overlay(app: &AppHandle) {
    // Bail out cleanly if we already built it (idempotent startup).
    if app.get_webview_window(OVERLAY_LABEL).is_some() {
        return;
    }

    // Seed an initial position; it is recomputed on every show.
    let (x, y) = calculate_overlay_position(app).unwrap_or((0.0, 0.0));

    match PanelBuilder::<_, RecordingOverlayPanel>::new(app, OVERLAY_LABEL)
        .url(WebviewUrl::App(OVERLAY_URL.into()))
        .title("Recording")
        .position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
        .size(tauri::Size::Logical(tauri::LogicalSize {
            width: OVERLAY_WIDTH,
            height: OVERLAY_HEIGHT,
        }))
        // Status level floats above normal/floating windows (and most WM chrome).
        .level(PanelLevel::Status)
        // Non-activating: showing the panel must never bring the app forward.
        .no_activate(true)
        .has_shadow(false)
        .transparent(true)
        .corner_radius(0.0)
        // Create the underlying Tauri window HIDDEN. Tauri builds the overlay as a
        // *normal* window before swizzling it into an NSPanel; if that window is
        // visible during that gap, Aerospace detects a normal window, assigns it to
        // the launch Space, and later follows the panel back to that Space (the
        // exact workspace-switch regression). Building hidden avoids that flash.
        .with_window(|w| w.decorations(false).transparent(true).visible(false))
        .collection_behavior(overlay_collection_behavior())
        .build()
    {
        Ok(panel) => {
            // Force the non-activating panel style mask AFTER the swizzle. This is
            // what makes tiling WMs (Aerospace) treat the window as a utility panel
            // they should ignore, and guarantees showing it never activates the app.
            // `NonactivatingPanel` is itself a borderless mask (Borderless == 0), so
            // this keeps the window borderless/transparent.
            panel.set_style_mask(
                NSWindowStyleMask::NonactivatingPanel | NSWindowStyleMask::Borderless,
            );

            // Don't let AppKit hide the panel when the (never-active) app
            // "deactivates" — it must stay put on the active Space.
            panel.set_hides_on_deactivate(false);

            // Start hidden; only shown while recording / transcribing.
            panel.hide();
            log::info!("Recording overlay panel created (hidden)");
        }
        Err(e) => {
            log::error!("Failed to create recording overlay panel: {}", e);
        }
    }
}

/// Non-macOS stub so the rest of the app links on other platforms.
#[cfg(not(target_os = "macos"))]
pub fn create_overlay(_app: &AppHandle) {}

// --- Public API: show / hide --------------------------------------------------

/// Shows the overlay in the `recording` visual state.
pub fn show_recording(app: &AppHandle) {
    show_overlay_state(app, STATE_RECORDING);
}

/// Shows the overlay in the `transcribing` visual state.
pub fn show_transcribing(app: &AppHandle) {
    show_overlay_state(app, STATE_TRANSCRIBING);
}

/// Shared show path: reposition, re-home to the active Space, order front
/// (without activating), and tell the webview which state to render.
#[cfg(target_os = "macos")]
fn show_overlay_state(app: &AppHandle, state: &str) {
    // Reposition to the monitor under the cursor first, so the panel surfaces in
    // the right place even when the active monitor changed since last time.
    reposition_overlay(app);

    if let Ok(panel) = app.get_webview_panel(OVERLAY_LABEL) {
        // Aerospace re-homing (HARD requirement #2 + #3): re-apply the collection
        // behavior on EVERY show. Combined with `moveToActiveSpace`, this rebinds
        // the panel to whatever Space is currently active — Aerospace may have
        // switched Spaces since the panel was last shown, and a panel left bound
        // to its launch Space is the exact root cause of PLAN.md risk #2 (the app
        // snapping back to the workspace it launched on). This MUST run before
        // ordering the panel front.
        panel.set_collection_behavior(overlay_collection_behavior().value());

        // Surface it WITHOUT stealing focus or activating the app (HARD
        // requirement #3 + #4 + #5). `order_front_regardless()` maps to
        // `orderFrontRegardless:`, which brings the *panel* forward on the active
        // Space without making it key and without raising the owning application.
        //
        // Do NOT replace this with any of:
        //   * `make_key_and_order_front` / `show_and_make_key` (makes the panel
        //     key → steals focus from the user's editor),
        //   * `app.activate()` / `set_focus()` (raises the app → Space switch),
        //   * `set_activation_policy(Regular)` (shows a Dock icon, activates app).
        // Any of those re-introduces the focus-steal / Space-switch bug.
        panel.order_front_regardless();
        log::info!("overlay show: state={state}, panel ordered front");
    } else {
        log::warn!("overlay show: panel not found");
    }

    // Drive the webview's visual state + fade-in. Emit via the window handle AND
    // the labeled target so the state change reliably reaches the panel webview.
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.emit(EVENT_SHOW, state);
    }
    let _ = app.emit_to(OVERLAY_LABEL, EVENT_SHOW, state);
}

#[cfg(not(target_os = "macos"))]
fn show_overlay_state(app: &AppHandle, state: &str) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.emit(EVENT_SHOW, state);
    }
    let _ = app.emit_to(OVERLAY_LABEL, EVENT_SHOW, state);
}

/// Hides the overlay (fade-out, then `orderOut:`).
#[cfg(target_os = "macos")]
pub fn hide_overlay(app: &AppHandle) {
    // Ask the webview to fade out first. Emit via the window handle AND the
    // labeled target so the fade-out reliably reaches the panel webview.
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.emit(EVENT_HIDE, ());
    }
    let _ = app.emit_to(OVERLAY_LABEL, EVENT_HIDE, ());

    // Hide the panel after the fade-out completes. `panel.hide()` maps to
    // AppKit `orderOut:`, which MUST run on the main thread — calling it from a
    // background thread crashes (`-[NSWindow _doOrderWindow:]` "must only be
    // used from the main thread"). The handle being `Send` does NOT make it
    // main-thread-safe. So we wait out the fade on a background thread, then
    // hop back to the main thread via `run_on_main_thread` to do the orderOut.
    // Fetch the panel INSIDE the main-thread closure rather than moving the
    // handle across threads.
    let app_handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let inner = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            if let Ok(panel) = inner.get_webview_panel(OVERLAY_LABEL) {
                panel.hide();
            }
        });
    });
}

#[cfg(not(target_os = "macos"))]
pub fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.emit(EVENT_HIDE, ());
    }
    let _ = app.emit_to(OVERLAY_LABEL, EVENT_HIDE, ());
}

/// Recomputes and applies the overlay position (monitor under the cursor,
/// bottom-center). Safe to call while hidden.
#[cfg(target_os = "macos")]
fn reposition_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        if let Some((x, y)) = calculate_overlay_position(app) {
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
    }
}

// --- Public API: waveform feed -----------------------------------------------

/// Emits a microphone amplitude sample (`f32`, expected range 0..1) to the
/// overlay webview, which animates the waveform from it.
pub fn emit_mic_level(app: &AppHandle, level: f32) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.emit(EVENT_MIC_LEVEL, level);
    }
}
