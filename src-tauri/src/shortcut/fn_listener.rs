//! Configurable trigger listener and toggle detector.
//!
//! Listens to raw keyboard events through `handy-keys`' [`KeyboardListener`]
//! (a CGEventTap on macOS) and submits a single [`TriggerInput::Toggle`] to the
//! [`Coordinator`] each time the user's configured trigger binding fires. The
//! coordinator's toggle semantics turn the first fire into "start recording"
//! and the next into "stop (transcribe + paste)".
//!
//! The listener is **passive**: it never suppresses or blocks the key, so the
//! key keeps its normal OS behavior. The default binding is Right ⌘
//! (`"CmdRight"`), which does nothing on its own in macOS, so passive listening
//! is safe.
//!
//! ## Binding kinds
//!
//! The binding is parsed from settings via [`Hotkey::from_str`]:
//!
//! - **Combo** (`binding.key.is_some()`, e.g. `Ctrl+Shift+R`): on a key-DOWN
//!   whose key equals the binding key and whose modifiers match the binding
//!   modifiers, submit [`TriggerInput::Toggle`].
//! - **Modifier-only** (`binding.key.is_none()`, e.g. Right ⌘): *tap detection*.
//!   Only a clean press → release of that modifier with NO other key (or
//!   different modifier) in between counts as a tap. This prevents combos like
//!   Right‑⌘+C from toggling.

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use handy_keys::{Hotkey, KeyboardListener, Modifiers};
use tauri::AppHandle;

use crate::coordinator::{Coordinator, TriggerInput};
use crate::settings::get_settings;

/// Handle to a running trigger listener. Dropping it (or calling [`stop`])
/// signals the listener thread to shut down and joins it.
///
/// [`stop`]: FnListenerHandle::stop
pub struct FnListenerHandle {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FnListenerHandle {
    /// Signal the listener to stop and join its thread.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for FnListenerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Default binding used when settings are missing or unparseable: Right ⌘.
fn fallback_binding() -> Hotkey {
    // CMD_RIGHT, modifier-only — known-valid, so unwrap is safe.
    Hotkey::new(Modifiers::CMD_RIGHT, None).expect("CMD_RIGHT is a valid hotkey")
}

/// Parse the trigger binding from settings, falling back to Right ⌘ on error.
fn read_binding(app: &AppHandle) -> Hotkey {
    let s = get_settings(app).trigger_binding;
    match Hotkey::from_str(&s) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("invalid trigger_binding {s:?} ({e}); falling back to Right Command");
            fallback_binding()
        }
    }
}

/// Tap-detection state for a modifier-only binding.
struct TapState {
    /// The single modifier flag we watch (e.g. `Modifiers::CMD_RIGHT`).
    target: Modifiers,
    /// True while the target modifier is physically held down.
    trigger_down: bool,
    /// Set if any other key-down or different modifier change occurred while
    /// the target was held — invalidates the tap.
    other_key_since_down: bool,
}

impl TapState {
    fn new(target: Modifiers) -> Self {
        Self {
            target,
            trigger_down: false,
            other_key_since_down: false,
        }
    }
}

/// Start the configurable trigger listener.
///
/// Installs the `handy-keys` CGEventTap (requires Accessibility on macOS) and
/// spawns a passive poll thread that detects the configured trigger and submits
/// [`TriggerInput::Toggle`] to `coordinator`. The binding is read from
/// `settings.trigger_binding` at startup; to change it, restart the listener
/// (see `change_trigger_binding`). Returns a [`FnListenerHandle`]; drop it or
/// call [`FnListenerHandle::stop`] to tear the listener down.
pub fn start_fn_listener(
    app: AppHandle,
    coordinator: Arc<Coordinator>,
) -> Result<FnListenerHandle, String> {
    // Construct the listener up front so permission/init failures surface to the
    // caller synchronously (the integration layer can then prompt for access).
    let listener = KeyboardListener::new().map_err(|e| {
        format!("failed to start trigger keyboard listener (Accessibility granted?): {e}")
    })?;

    let binding = read_binding(&app);
    log::info!("Trigger listener binding: {binding}");

    let running = Arc::new(AtomicBool::new(true));
    let thread_running = running.clone();

    let thread = thread::spawn(move || {
        // Poll with a timeout so we can observe the `running` flag for shutdown.
        const RECV_TIMEOUT: Duration = Duration::from_millis(100);

        log::info!("Trigger listener started");

        if let Some(key) = binding.key {
            // ---- Combo binding (modifiers + key) -----------------------------
            run_combo_loop(&listener, &thread_running, &coordinator, binding.modifiers, key, RECV_TIMEOUT);
        } else {
            // ---- Modifier-only binding (tap detection) -----------------------
            let target = binding.modifiers;
            run_tap_loop(&listener, &thread_running, &coordinator, target, RECV_TIMEOUT);
        }

        log::info!("Trigger listener stopped");
    });

    Ok(FnListenerHandle {
        running,
        thread: Some(thread),
    })
}

/// Combo-binding loop: fire on a key-down matching key + modifiers.
fn run_combo_loop(
    listener: &KeyboardListener,
    running: &AtomicBool,
    coordinator: &Arc<Coordinator>,
    mods: Modifiers,
    key: handy_keys::Key,
    timeout: Duration,
) {
    while running.load(Ordering::SeqCst) {
        let event = match listener.recv_timeout(timeout) {
            Ok(ev) => ev,
            Err(handy_keys::Error::Timeout) => continue,
            Err(_) => {
                log::warn!("trigger listener: event source disconnected");
                break;
            }
        };

        if event.is_key_down && event.key == Some(key) && mods.matches(event.modifiers) {
            log::info!("Trigger fired: Toggle");
            coordinator.submit(TriggerInput::Toggle);
        }
    }
}

/// Modifier-only loop: clean press→release tap detection.
fn run_tap_loop(
    listener: &KeyboardListener,
    running: &AtomicBool,
    coordinator: &Arc<Coordinator>,
    target: Modifiers,
    timeout: Duration,
) {
    let mut state = TapState::new(target);

    while running.load(Ordering::SeqCst) {
        let event = match listener.recv_timeout(timeout) {
            Ok(ev) => ev,
            Err(handy_keys::Error::Timeout) => continue,
            Err(_) => {
                log::warn!("trigger listener: event source disconnected");
                break;
            }
        };

        if event.changed_modifier == Some(state.target) {
            // A change of the watched modifier itself: press or release.
            if event.is_key_down {
                // Press of the target modifier → arm a potential tap.
                state.trigger_down = true;
                state.other_key_since_down = false;
            } else {
                // Release of the target modifier → a clean tap iff nothing else
                // happened while it was held.
                if state.trigger_down && !state.other_key_since_down {
                    log::info!("Trigger fired: Toggle");
                    coordinator.submit(TriggerInput::Toggle);
                }
                state.trigger_down = false;
                state.other_key_since_down = false;
            }
            continue;
        }

        // Any OTHER event while the target is held invalidates the tap:
        // a regular key-down, or a change of a different modifier.
        if state.trigger_down && (event.is_key_down || event.changed_modifier.is_some()) {
            state.other_key_since_down = true;
        }
    }
}
