//! On-device transcription cleanup via Apple's FoundationModels framework.
//!
//! The actual model calls live in a Swift bridge (`swift/fm_bridge.swift`),
//! compiled and linked by `build.rs`. This module is the thin Rust FFI layer
//! over its C-ABI surface. Each cleanup uses a fresh `LanguageModelSession`
//! (handled Swift-side) so no context accumulates between calls.
//!
//! On non-macOS targets the whole thing degrades to no-op stubs.

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_char, CStr, CString};

    extern "C" {
        fn fm_is_available() -> i32;
        fn fm_warm();
        fn fm_cleanup(t: *const c_char) -> *mut c_char;
        fn fm_free(p: *mut c_char);
    }

    /// Whether the on-device system language model is available right now.
    pub fn is_available() -> bool {
        // SAFETY: `fm_is_available` is a pure, side-effect-free query.
        unsafe { fm_is_available() == 1 }
    }

    /// Warm the model with a throwaway request so the first real cleanup is on
    /// the warm path. Blocks until the warmup completes (call off the main
    /// thread). Best-effort: errors are swallowed Swift-side.
    pub fn warm() {
        // SAFETY: `fm_warm` takes no args and ignores its result.
        unsafe { fm_warm() }
    }

    /// Clean up `text`, returning the corrected text, or `None` on any error.
    pub fn cleanup(text: &str) -> Option<String> {
        let input = CString::new(text).ok()?;
        // SAFETY: `input` outlives the call; `fm_cleanup` either returns null or
        // a heap C string we own and must free via `fm_free`.
        unsafe {
            let ptr = fm_cleanup(input.as_ptr());
            if ptr.is_null() {
                return None;
            }
            let cleaned = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            fm_free(ptr);
            Some(cleaned)
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn is_available() -> bool {
        false
    }
    pub fn warm() {}
    pub fn cleanup(_text: &str) -> Option<String> {
        None
    }
}

pub use imp::{cleanup, is_available, warm};
