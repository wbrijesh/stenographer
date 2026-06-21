import { emit, listen } from "@tauri-apps/api/event";
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";

/**
 * The recording overlay panel.
 *
 * Rendered inside a transparent, non-activating NSPanel (440x132 logical pts).
 * It is a RECTANGLE: a scrollable live-transcript area on top and a control row
 * (mic indicator / waveform / cancel) along the bottom.
 *
 * It is purely event-driven from the Rust side:
 *   - `show-overlay`       (payload: "recording" | "transcribing") -> set state.
 *                          On a fresh "recording" we also reset the transcript
 *                          so stale text from a prior session never lingers.
 *   - `hide-overlay`       -> no-op for visibility (Rust orderOut hides panel).
 *   - `mic-level`          (payload: number[] of bar levels in 0..1) -> waveform.
 *                          A bare `number` is also accepted (single center
 *                          amplitude that ripples out) for robustness.
 *   - `partial-transcript` (payload: bare string, the transcript-so-far which
 *                          grows over time) -> live text, auto-scrolled to bottom.
 *
 * Layout (top -> bottom) at 440x132:
 *   [  scrollable live transcript (auto-scrolled to newest)  ]
 *   [ mic(indicator)  compact waveform           cancel ✕    ]
 *
 * The mic glyph is a static recording/transcribing indicator (not a button).
 * The cancel button emits `overlay-cancel` for the backend to handle.
 */

type OverlayState = "recording" | "transcribing";
type MicLevelPayload = number | number[];

// Number of bars shown in the compact bottom-row waveform.
const COMPACT_BAR_COUNT = 5;
// Smoothing factor for incoming amplitude (0..1; higher = snappier).
const SMOOTHING = 0.35;

const clamp01 = (n: number): number => Math.max(0, Math.min(1, n));

/**
 * Resample an arbitrary-length array of bar levels onto exactly `count` bars by
 * nearest-neighbour sampling. Keeps rendering robust regardless of how many
 * buckets the backend sends.
 */
function fitToBars(levels: number[], count: number): number[] {
  if (levels.length === 0) return Array(count).fill(0);
  if (levels.length === count) return levels.map(clamp01);
  const out: number[] = new Array(count);
  for (let i = 0; i < count; i++) {
    const src = Math.floor((i * levels.length) / count);
    out[i] = clamp01(levels[src] ?? 0);
  }
  return out;
}

// Centralized UI strings.
const STRINGS = {
  listening: "Listening…",
  transcribing: "Transcribing…",
  cancelLabel: "Cancel recording",
  recordingLabel: "Recording",
} as const;

const RecordingOverlay: React.FC = () => {
  // Panel ordering (Rust: `order_front_regardless` / `orderOut`) is the SOLE
  // source of truth for whether the panel is on screen. The panel is therefore
  // always rendered at full opacity (see `.overlay-panel` in overlay.css) and
  // the webview never gates its own visibility. A suspended WKWebview that
  // misses a `show-overlay` event can no longer get stuck invisible.
  const [state, setState] = useState<OverlayState>("recording");
  // Per-bar heights for the compact waveform, animated. Newest amplitude is
  // pushed in at the center and ripples outward for an organic waveform.
  const [bars, setBars] = useState<number[]>(() =>
    Array(COMPACT_BAR_COUNT).fill(0),
  );
  // Live transcript-so-far. Empty until the first `partial-transcript` arrives.
  const [transcript, setTranscript] = useState<string>("");
  const smoothedRef = useRef(0);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<OverlayState>("show-overlay", (event) => {
      const next = event.payload ?? "recording";
      // A fresh recording session: clear any stale transcript so old text from
      // the previous dictation does not linger.
      if (next === "recording") {
        setTranscript("");
      }
      setState(next);
    }).then((u) => unlisteners.push(u));

    // `hide-overlay` is intentionally a no-op for visibility: Rust calls
    // `orderOut` ~300ms later to remove the panel. We must NOT set a persistent
    // invisible state here, or a later missed `show-overlay` would leave the
    // panel stuck hidden once it is re-ordered front.
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
        // Preferred path: backend sends one level per bar. Fit to the compact
        // bar count and exponentially smooth each bar to reduce jitter.
        const target = fitToBars(payload, COMPACT_BAR_COUNT);
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
        const mid = Math.floor(COMPACT_BAR_COUNT / 2);
        const next = [...prev];
        for (let i = 0; i < mid; i++) {
          next[i] = prev[i + 1];
          next[COMPACT_BAR_COUNT - 1 - i] = prev[COMPACT_BAR_COUNT - 2 - i];
        }
        next[mid] = level;
        return next;
      });
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  // Auto-scroll the transcript area to the bottom whenever new text arrives so
  // the latest words are always visible. useLayoutEffect runs before paint to
  // avoid a visible jump.
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [transcript]);

  const onCancel = (): void => {
    void emit("overlay-cancel");
  };

  const hasText = transcript.trim().length > 0;

  // Empty-state placeholder depends on phase: while recording we are listening,
  // while transcribing we are finishing up.
  const placeholder =
    state === "recording" ? STRINGS.listening : STRINGS.transcribing;

  return (
    <div className="overlay-panel">
      <div className="overlay-scroll" ref={scrollRef}>
        {hasText ? (
          <p className="overlay-transcript">{transcript}</p>
        ) : (
          <p className="overlay-placeholder">{placeholder}</p>
        )}
      </div>

      <div className="overlay-controls">
        <div className="overlay-controls-left">
          <span
            className="mic-indicator"
            role="img"
            aria-label={STRINGS.recordingLabel}
            title={STRINGS.recordingLabel}
          >
            <MicGlyph />
          </span>

          <span className="waveform" aria-hidden>
            {bars.map((v, i) => (
              <span
                key={i}
                className="wave-bar"
                style={{
                  height: `${4 + Math.pow(v, 0.7) * 14}px`,
                  opacity: 0.35 + Math.min(0.65, v * 1.4),
                }}
              />
            ))}
          </span>
        </div>

        <button
          type="button"
          className="cancel-btn"
          aria-label={STRINGS.cancelLabel}
          title={STRINGS.cancelLabel}
          onClick={onCancel}
        >
          <CancelGlyph />
        </button>
      </div>
    </div>
  );
};

const MicGlyph: React.FC = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
    <rect x="9" y="2" width="6" height="12" rx="3" fill="currentColor" />
    <path
      d="M5 11a7 7 0 0 0 14 0M12 18v3"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
    />
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
