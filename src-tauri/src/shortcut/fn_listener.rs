//! Fn (Globe) key listener and trigger classifier.
//!
//! Listens to raw keyboard events through `handy-keys`' [`KeyboardListener`]
//! (a CGEventTap on macOS) and translates Fn/Space activity into classified
//! [`TriggerInput`]s for the [`Coordinator`].
//!
//! ## Classification (threshold = `hold_tap_threshold_ms`, default 250 ms)
//!
//! - **Fn down** → [`TriggerInput::HoldStart`] immediately; record the press
//!   [`Instant`]. This begins a *provisional* push-to-talk recording.
//! - **Fn up, elapsed ≥ threshold** → [`TriggerInput::HoldStop`] (push-to-talk
//!   end → transcribe+paste).
//! - **Fn up, elapsed < threshold, no active toggle** → discard the provisional
//!   recording via [`TriggerInput::Cancel`] (a too-short hold is a no-op).
//! - **Fn + Space** (Space pressed while Fn held) → [`TriggerInput::ToggleStart`]
//!   (cancels the provisional PTT for that press; begins a hands-free session).
//! - **Fn tap while a toggle session is active** → [`TriggerInput::ToggleStop`].
//!
//! On macOS the Globe key arrives as a *FlagsChanged* event: the listener sees a
//! [`KeyEvent`] with `changed_modifier == Some(Modifiers::FN)` and `is_key_down`
//! distinguishing press from release.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use handy_keys::{Key, KeyboardListener, Modifiers};
use tauri::AppHandle;

use crate::coordinator::{Coordinator, TriggerInput};
use crate::settings::get_settings;

/// Handle to a running Fn listener. Dropping it (or calling [`stop`]) signals
/// the listener thread to shut down and joins it.
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

/// Per-Fn-hold classifier state.
struct FnState {
    /// `Some(instant)` while Fn is physically held down.
    fn_down_at: Option<Instant>,
    /// Set when Space is pressed during the current Fn hold (Fn+Space chord),
    /// so the Fn-up is treated as the start of a toggle rather than a PTT end.
    toggle_started_this_hold: bool,
    /// True while a hands-free toggle session is active (between Fn+Space and
    /// the next Fn tap).
    toggle_active: bool,
}

impl FnState {
    fn new() -> Self {
        Self {
            fn_down_at: None,
            toggle_started_this_hold: false,
            toggle_active: false,
        }
    }
}

/// Start the Fn listener.
///
/// Installs the `handy-keys` CGEventTap (requires Accessibility on macOS) and
/// spawns a thread that classifies Fn/Space events into [`TriggerInput`]s,
/// submitting them to `coordinator`. Returns a [`FnListenerHandle`]; drop it or
/// call [`FnListenerHandle::stop`] to tear the listener down.
///
/// The hold/tap threshold is read from settings on each Fn-up, so changes take
/// effect without restarting the listener.
pub fn start_fn_listener(
    app: AppHandle,
    coordinator: Arc<Coordinator>,
) -> Result<FnListenerHandle, String> {
    // Construct the listener up front so permission/init failures surface to the
    // caller synchronously (the integration layer can then prompt for access).
    let listener = KeyboardListener::new().map_err(|e| {
        format!("failed to start Fn keyboard listener (Accessibility granted?): {e}")
    })?;

    let running = Arc::new(AtomicBool::new(true));
    let thread_running = running.clone();

    let thread = thread::spawn(move || {
        // Poll with a timeout so we can observe the `running` flag for shutdown.
        const RECV_TIMEOUT: Duration = Duration::from_millis(100);
        let mut state = FnState::new();

        log::info!("Fn listener started");

        while thread_running.load(Ordering::SeqCst) {
            let event = match listener.recv_timeout(RECV_TIMEOUT) {
                Ok(ev) => ev,
                Err(handy_keys::Error::Timeout) => continue,
                Err(_) => {
                    log::warn!("Fn listener: event source disconnected");
                    break;
                }
            };

            // --- Fn (Globe) press/release via FlagsChanged -------------------
            if event.changed_modifier == Some(Modifiers::FN) {
                if event.is_key_down && event.modifiers.contains(Modifiers::FN) {
                    handle_fn_down(&mut state, &coordinator);
                } else {
                    handle_fn_up(&mut state, &coordinator, &app);
                }
                continue;
            }

            // --- Space ------------------------------------------------------
            // Fn+Space chord: Space pressed while Fn is held → ToggleStart.
            if event.is_key_down
                && event.key == Some(Key::Space)
                && state.fn_down_at.is_some()
                && !state.toggle_started_this_hold
            {
                state.toggle_started_this_hold = true;
                state.toggle_active = true;
                coordinator.submit(TriggerInput::ToggleStart);
            }
        }

        log::info!("Fn listener stopped");
    });

    Ok(FnListenerHandle {
        running,
        thread: Some(thread),
    })
}

fn handle_fn_down(state: &mut FnState, coordinator: &Arc<Coordinator>) {
    // Ignore key-repeat: only act on the first down of a hold.
    if state.fn_down_at.is_some() {
        return;
    }
    state.fn_down_at = Some(Instant::now());
    state.toggle_started_this_hold = false;

    if state.toggle_active {
        // A toggle session is active and the user tapped Fn again — this hold
        // will end the toggle on Fn-up. Do not start a provisional PTT.
        return;
    }

    // Begin a provisional push-to-talk recording immediately.
    coordinator.submit(TriggerInput::HoldStart);
}

fn handle_fn_up(state: &mut FnState, coordinator: &Arc<Coordinator>, app: &AppHandle) {
    let Some(down_at) = state.fn_down_at.take() else {
        return;
    };
    let elapsed = down_at.elapsed();

    // Case 1: this hold started a toggle session (Fn+Space). The session is now
    // running hands-free; releasing Fn does nothing.
    if state.toggle_started_this_hold {
        state.toggle_started_this_hold = false;
        return;
    }

    // Case 2: a toggle session is active and this was a plain Fn tap → stop it.
    if state.toggle_active {
        state.toggle_active = false;
        coordinator.submit(TriggerInput::ToggleStop);
        return;
    }

    // Case 3: plain push-to-talk hold/tap. Threshold decides hold vs. tap.
    let threshold = Duration::from_millis(get_settings(app).hold_tap_threshold_ms);
    if elapsed >= threshold {
        // Held long enough → push-to-talk end.
        coordinator.submit(TriggerInput::HoldStop);
    } else {
        // Too short → discard the provisional recording started on Fn-down.
        coordinator.submit(TriggerInput::Cancel);
    }
}
