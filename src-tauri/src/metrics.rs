//! Lightweight process-global observability counters for the dictation
//! pipeline. The pipeline calls the `record_*` functions on each transition;
//! the settings window reads a [`Metrics`] snapshot via the `get_metrics`
//! command.
//!
//! Everything lives behind a single `Mutex` so updates are cheap and atomic.
//! Running averages are maintained by storing the internal totals and dividing
//! on each update, so [`snapshot`] is a trivial clone of the public fields.

use std::sync::Mutex;
use std::time::SystemTime;

use once_cell::sync::Lazy;
use serde::Serialize;

/// A serializable snapshot of the current process metrics. Returned by the
/// `get_metrics` command and consumed by the settings UI.
#[derive(Serialize, specta::Type, Clone, Default)]
pub struct Metrics {
    /// Recording sessions started.
    pub recordings: u64,
    /// Sessions transcribed + pasted successfully.
    pub completed: u64,
    /// Sessions cancelled by the user.
    pub cancelled: u64,
    /// Count of final (authoritative) transcriptions produced.
    pub transcriptions: u64,
    /// Duration of the most recent final transcription, in milliseconds.
    pub last_transcription_ms: u64,
    /// Running average final-transcription duration, in milliseconds.
    pub avg_transcription_ms: u64,
    /// Count of AI cleanups run.
    pub cleanups: u64,
    /// Duration of the most recent cleanup, in milliseconds.
    pub last_cleanup_ms: u64,
    /// Running average cleanup duration, in milliseconds.
    pub avg_cleanup_ms: u64,
    /// Total words pasted across all completed sessions.
    pub words_dictated: u64,
    /// Transcription / paste / audio errors observed.
    pub errors: u64,
    /// Process start time (Unix seconds), set on first metrics access.
    pub session_started_unix: u64,
}

/// Internal store: the public snapshot plus the totals needed to compute the
/// running averages without keeping every sample.
#[derive(Default)]
struct MetricsInner {
    metrics: Metrics,
    /// Sum of all final-transcription durations (ms) — divided by
    /// `transcriptions` for the average.
    total_transcription_ms: u64,
    /// Sum of all cleanup durations (ms) — divided by `cleanups` for the
    /// average.
    total_cleanup_ms: u64,
}

static METRICS: Lazy<Mutex<MetricsInner>> = Lazy::new(|| {
    let mut inner = MetricsInner::default();
    inner.metrics.session_started_unix = now_unix();
    Mutex::new(inner)
});

/// Current Unix time in whole seconds (0 if the clock is before the epoch).
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A recording session was started.
pub fn record_recording_started() {
    if let Ok(mut inner) = METRICS.lock() {
        inner.metrics.recordings += 1;
    }
}

/// A recording session was cancelled by the user.
pub fn record_cancelled() {
    if let Ok(mut inner) = METRICS.lock() {
        inner.metrics.cancelled += 1;
    }
}

/// A transcription / paste / audio error occurred.
pub fn record_error() {
    if let Ok(mut inner) = METRICS.lock() {
        inner.metrics.errors += 1;
    }
}

/// A session completed: it was transcribed and pasted.
///
/// - Increments `completed` and `transcriptions`, updating the last/average
///   transcription duration from `transcription_ms`.
/// - If `cleanup_ms` is `Some`, increments `cleanups` and updates the
///   last/average cleanup duration.
/// - Adds the whitespace-separated word count of `text` to `words_dictated`.
pub fn record_completed(transcription_ms: u64, cleanup_ms: Option<u64>, text: &str) {
    if let Ok(mut inner) = METRICS.lock() {
        inner.metrics.completed += 1;

        inner.metrics.transcriptions += 1;
        inner.metrics.last_transcription_ms = transcription_ms;
        inner.total_transcription_ms += transcription_ms;
        inner.metrics.avg_transcription_ms =
            inner.total_transcription_ms / inner.metrics.transcriptions;

        if let Some(cleanup_ms) = cleanup_ms {
            inner.metrics.cleanups += 1;
            inner.metrics.last_cleanup_ms = cleanup_ms;
            inner.total_cleanup_ms += cleanup_ms;
            inner.metrics.avg_cleanup_ms = inner.total_cleanup_ms / inner.metrics.cleanups;
        }

        let words = text.split_whitespace().count() as u64;
        inner.metrics.words_dictated += words;
    }
}

/// Return a clone of the current public metrics.
pub fn snapshot() -> Metrics {
    METRICS
        .lock()
        .map(|inner| inner.metrics.clone())
        .unwrap_or_default()
}
