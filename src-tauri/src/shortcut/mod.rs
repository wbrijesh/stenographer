//! Global trigger listener for Stenographer.
//!
//! Stenographer uses a single **configurable trigger binding** (default Right ⌘)
//! with simple toggle semantics: fire once to start recording, fire again to
//! stop. The low-level listener ([`fn_listener`]) observes raw key events via
//! `handy-keys` (a CGEventTap on macOS), passively detects the binding (combo
//! key-down, or modifier-only *tap*), and submits [`TriggerInput::Toggle`] to
//! the [`Coordinator`](crate::coordinator::Coordinator).
//!
//! The listener requires Accessibility permission, so the integration layer
//! must only call [`start_fn_listener`] *after* permission is granted.

mod fn_listener;

pub use fn_listener::{start_fn_listener, FnListenerHandle};
