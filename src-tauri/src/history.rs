//! Persisted ring of the last 10 final transcriptions, newest first.
//!
//! Each completed (final, non-cancelled) transcription is appended via
//! [`record`], the list is truncated to [`MAX_ENTRIES`], and the whole thing is
//! written back to `history.json` in the app data dir. [`get`] loads the list
//! (newest first) and [`clear`] empties it.
//!
//! All IO/JSON failures are handled gracefully: nothing here panics, and
//! failures are logged at `warn`. A process-global `Mutex` serializes the
//! read-modify-write so concurrent records can't clobber each other.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Maximum number of transcriptions to retain (newest first).
const MAX_ENTRIES: usize = 10;

/// Serializes the read-modify-write of `history.json` so concurrent `record`
/// calls (e.g. overlapping recordings) can't lose entries.
static HISTORY_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// A single recorded transcription.
#[derive(Serialize, Deserialize, specta::Type, Clone)]
pub struct HistoryEntry {
    /// The final transcription text that was pasted.
    pub text: String,
    /// When it was recorded, as seconds since the Unix epoch.
    pub timestamp_unix: u64,
}

/// Resolve the path to `history.json` in the app data dir, creating the dir if
/// needed. Returns `None` if the dir can't be resolved/created.
fn history_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            log::warn!("history: failed to resolve app_data_dir: {e}");
            return None;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("history: failed to create app_data_dir {dir:?}: {e}");
        return None;
    }
    Some(dir.join("history.json"))
}

/// Load the persisted entries (newest first). Returns an empty vec if the file
/// is missing or can't be read/parsed.
fn load(path: &PathBuf) -> Vec<HistoryEntry> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        // Missing file is the normal first-run case; not worth a warning.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            log::warn!("history: failed to read {path:?}: {e}");
            return Vec::new();
        }
    };
    match serde_json::from_str(&contents) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("history: failed to parse {path:?}: {e}");
            Vec::new()
        }
    }
}

/// Persist `entries` to `path`. Best-effort: logs on failure.
fn save(path: &PathBuf, entries: &[HistoryEntry]) {
    match serde_json::to_string_pretty(entries) {
        Ok(json) => {
            if let Err(e) = std::fs::write(path, json) {
                log::warn!("history: failed to write {path:?}: {e}");
            }
        }
        Err(e) => log::warn!("history: failed to serialize history: {e}"),
    }
}

/// Record a new final transcription. Empty/whitespace-only text is ignored.
/// Prepends the entry (newest first), truncates to [`MAX_ENTRIES`], persists.
pub fn record(app: &AppHandle, text: &str) {
    if text.trim().is_empty() {
        return;
    }

    let Some(path) = history_path(app) else {
        return;
    };

    let timestamp_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let _guard = HISTORY_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut entries = load(&path);
    entries.insert(
        0,
        HistoryEntry {
            text: text.to_string(),
            timestamp_unix,
        },
    );
    entries.truncate(MAX_ENTRIES);
    save(&path, &entries);
}

/// Return the persisted entries, newest first.
pub fn get(app: &AppHandle) -> Vec<HistoryEntry> {
    let Some(path) = history_path(app) else {
        return Vec::new();
    };
    let _guard = HISTORY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    load(&path)
}

/// Empty the history list and persist the empty list.
pub fn clear(app: &AppHandle) {
    let Some(path) = history_path(app) else {
        return;
    };
    let _guard = HISTORY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    save(&path, &[]);
}
