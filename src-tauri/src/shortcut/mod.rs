//! Global trigger listener for Stenographer.
//!
//! Stenographer uses a single **Fn (Globe) key** state machine rather than a
//! configurable hotkey. The low-level listener ([`fn_listener`]) observes raw
//! Fn/Space key events via `handy-keys` (a CGEventTap on macOS), *classifies*
//! them against the hold/tap threshold, and submits [`TriggerInput`]s to the
//! [`Coordinator`](crate::coordinator::Coordinator).
//!
//! The listener requires Accessibility permission, so the integration layer
//! must only call [`start_fn_listener`] *after* permission is granted.

mod fn_listener;

pub use fn_listener::{start_fn_listener, FnListenerHandle};
