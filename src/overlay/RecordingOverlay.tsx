import { emit, listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";

/**
 * The recording overlay "pill".
 *
 * Rendered inside a transparent, non-activating NSPanel (420x56 logical pts).
 * It is purely event-driven from the Rust side:
 *   - `show-overlay`       (payload: "recording" | "transcribing") -> set state.
 *                          On a fresh "recording" we also reset the transcript
 *                          so stale text from a prior session never lingers.
 *   - `hide-overlay`       -> no-op for visibility (Rust orderOut hides panel).
 *   - `mic-level`          (payload: number[] of bar levels in 0..1) -> waveform.
 *                          A bare `number` is also accepted (single center
 *                          amplitude that ripples out) for robustness.
 *   - `partial-transcript` (payload: bare string, the transcript-so-far which
 *                          grows over time) -> live text in the center.
 *
 * Layout (left -> right) at 420x56:
 *   [mic / dots]  [ live transcript text  ·  compact waveform ]  [cancel ✕]
 * Before any transcript text arrives, the center shows the full waveform as a
 * voice-activity placeholder. Once text exists, the center shows the text with
 * a compact (few-bar) waveform beside the mic so both stay visible.
 *
 * The cancel button emits `overlay-cancel` for the backend to handle.
 */

type OverlayState = "recording" | "transcribing";
type MicLevelPayload = number | number[];

// Number of waveform bars in the full (placeholder) waveform.
const BAR_COUNT = 9;
// Number of bars shown in the compact waveform (once transcript text exists).
const COMPACT_BAR_COUNT = 4;
// Smoothing factor for incoming amplitude (0..1; higher = snappier).
const SMOOTHING = 0.35;
// Keep at most this many trailing characters so the latest words stay visible.
const MAX_VISIBLE_CHARS = 160;

const clamp01 = (n: number) => Math.max(0, Math.min(1, n));

/**
 * Resample an arbitrary-length array of bar levels onto exactly `BAR_COUNT`
 * bars by nearest-neighbour sampling. Keeps rendering robust regardless of how
 * many buckets the backend sends.
 */
function fitToBars(levels: number[]): number[] {
  if (levels.length === 0) return Array(BAR_COUNT).fill(0);
  if (levels.length === BAR_COUNT) return levels.map(clamp01);
  const out: number[] = new Array(BAR_COUNT);
  for (let i = 0; i < BAR_COUNT; i++) {
    const src = Math.floor((i * levels.length) / BAR_COUNT);
    out[i] = clamp01(levels[src] ?? 0);
  }
  return out;
}

/**
 * Keep only the trailing portion of a growing transcript so the most recent
 * words remain visible on a single line. We slice on a word boundary when
 * possible and prepend an ellipsis to signal earlier text was clipped.
 */
function tailText(text: string): string {
  if (text.length <= MAX_VISIBLE_CHARS) return text;
  const sliced = text.slice(text.length - MAX_VISIBLE_CHARS);
  const space = sliced.indexOf(" ");
  const trimmed = space > 0 ? sliced.slice(space + 1) : sliced;
  return `…${trimmed}`;
}

// Centralized UI strings.
const STRINGS = {
  transcribing: "Transcribing…",
  cancelLabel: "Cancel recording",
} as const;

const RecordingOverlay: React.FC = () => {
  // Panel ordering (Rust: `order_front_regardless` / `orderOut`) is the SOLE
  // source of truth for whether the pill is on screen. The pill is therefore
  // always rendered at full opacity (see `.overlay-pill` in overlay.css) and
  // the webview never gates its own visibility. A suspended WKWebview that
  // misses a `show-overlay` event can no longer get stuck invisible.
  const [state, setState] = useState<OverlayState>("recording");
  // Per-bar heights, animated. Newest amplitude is pushed in at the center and
  // ripples outward for an organic waveform.
  const [bars, setBars] = useState<number[]>(() => Array(BAR_COUNT).fill(0));
  // Live transcript-so-far. Empty until the first `partial-transcript` arrives.
  const [transcript, setTranscript] = useState<string>("");
  const smoothedRef = useRef(0);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<OverlayState>("show-overlay", (event) => {
      const next = event.payload ?? "recording";
      // A fresh recording session: clear any stale transcript so old text from
      // the previous dictation does not linger in the new pill.
      if (next === "recording") setTranscript("");
      setState(next);
    }).then((u) => unlisteners.push(u));

    // `hide-overlay` is intentionally a no-op for visibility: Rust calls
    // `orderOut` ~300ms later to remove the panel. We must NOT set a persistent
    // invisible state here, or a later missed `show-overlay` would leave the
    // pill stuck hidden once the panel is re-ordered front.
    listen("hide-overlay", () => {
      // no-op
    }).then((u) => unlisteners.push(u));

    // Live transcript text (grows over time). Bare string payload.
    listen<string>("partial-transcript", (event) => {
      setTranscript(event.payload ?? "");
    }).then((u) => unlisteners.push(u));

    listen<MicLevelPayload>("mic-level", (event) => {
      const payload = event.payload;

      if (Array.isArray(payload)) {
        // Preferred path: backend sends one level per bar. Fit to BAR_COUNT and
        // exponentially smooth each bar to reduce jitter.
        const target = fitToBars(payload);
        setBars((prev) =>
          target.map(
            (t, i) => (prev[i] ?? 0) * (1 - SMOOTHING) + t * SMOOTHING,
          ),
        );
        return;
      }

      // Fallback: a single amplitude. Ripple it out from the center.
      const raw = typeof payload === "number" ? payload : 0;
      smoothedRef.current =
        smoothedRef.current * (1 - SMOOTHING) + clamp01(raw) * SMOOTHING;
      const level = smoothedRef.current;

      setBars((prev) => {
        const mid = Math.floor(BAR_COUNT / 2);
        const next = [...prev];
        for (let i = 0; i < mid; i++) {
          next[i] = prev[i + 1];
          next[BAR_COUNT - 1 - i] = prev[BAR_COUNT - 2 - i];
        }
        next[mid] = level;
        return next;
      });
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const onCancel = () => {
    void emit("overlay-cancel");
  };

  const hasText = transcript.trim().length > 0;
  const visibleText = hasText ? tailText(transcript) : "";

  // When text exists, the center is dominated by the text and a compact
  // waveform sits beside the mic. Otherwise the full waveform fills the center
  // as the voice-activity placeholder (recording) or we show "Transcribing…".
  const compactBars = bars.slice(bars.length - COMPACT_BAR_COUNT);

  return (
    <div className="overlay-pill">
      <div className="overlay-lead" aria-hidden>
        <span className="overlay-icon">
          {state === "recording" ? <MicGlyph /> : <DotsGlyph />}
        </span>
        {hasText && state === "recording" && (
          <span className="waveform waveform--compact">
            {compactBars.map((v, i) => (
              <span
                key={i}
                className="wave-bar"
                style={{
                  height: `${4 + Math.pow(v, 0.7) * 12}px`,
                  opacity: 0.35 + Math.min(0.65, v * 1.4),
                }}
              />
            ))}
          </span>
        )}
      </div>

      <div className="overlay-center">
        {hasText ? (
          // Latest words stay visible: rtl direction + ellipsis-at-start keeps
          // the END of the (left-to-right) text in view as it grows.
          <bdi className="overlay-transcript" dir="rtl">
            {visibleText}
          </bdi>
        ) : state === "recording" ? (
          <span className="waveform">
            {bars.map((v, i) => (
              <span
                key={i}
                className="wave-bar"
                style={{
                  height: `${4 + Math.pow(v, 0.7) * 16}px`,
                  opacity: 0.35 + Math.min(0.65, v * 1.4),
                }}
              />
            ))}
          </span>
        ) : (
          <span className="overlay-text">{STRINGS.transcribing}</span>
        )}
      </div>

      <div className="overlay-right">
        {state === "recording" && (
          <button
            type="button"
            className="cancel-btn"
            aria-label={STRINGS.cancelLabel}
            onClick={onCancel}
          >
            <CancelGlyph />
          </button>
        )}
      </div>
    </div>
  );
};

const MicGlyph: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
    <rect x="9" y="2" width="6" height="12" rx="3" fill="currentColor" />
    <path
      d="M5 11a7 7 0 0 0 14 0M12 18v3"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
    />
  </svg>
);

const DotsGlyph: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
    <circle cx="5" cy="12" r="2" />
    <circle cx="12" cy="12" r="2" />
    <circle cx="19" cy="12" r="2" />
  </svg>
);

const CancelGlyph: React.FC = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
    <path
      d="M6 6l12 12M18 6L6 18"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
    />
  </svg>
);

export default RecordingOverlay;
