import { emit, listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";

/**
 * The recording overlay "pill".
 *
 * Rendered inside a transparent, non-activating NSPanel. It is purely
 * event-driven from the Rust side:
 *   - `show-overlay`  (payload: "recording" | "transcribing") -> show + set state
 *   - `hide-overlay`  -> fade out
 *   - `mic-level`     (payload: f32 in 0..1) -> drive the waveform amplitude
 *
 * The cancel button emits `overlay-cancel` for the backend to handle; the
 * overlay does not import the typed bindings so it stays self-contained.
 */

type OverlayState = "recording" | "transcribing";

// Number of waveform bars in the pill.
const BAR_COUNT = 9;
// Smoothing factor for incoming amplitude (0..1; higher = snappier).
const SMOOTHING = 0.35;

// Centralized UI strings.
const STRINGS = {
  transcribing: "Transcribing…",
  cancelLabel: "Cancel recording",
} as const;

const RecordingOverlay: React.FC = () => {
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  // Per-bar heights, animated. Newest amplitude is pushed in at the center and
  // ripples outward for an organic waveform.
  const [bars, setBars] = useState<number[]>(() => Array(BAR_COUNT).fill(0));
  const smoothedRef = useRef(0);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<OverlayState>("show-overlay", (event) => {
      setState(event.payload ?? "recording");
      setIsVisible(true);
    }).then((u) => unlisteners.push(u));

    listen("hide-overlay", () => {
      setIsVisible(false);
    }).then((u) => unlisteners.push(u));

    listen<number>("mic-level", (event) => {
      const raw = typeof event.payload === "number" ? event.payload : 0;
      const clamped = Math.max(0, Math.min(1, raw));
      // Exponential smoothing to reduce jitter.
      smoothedRef.current =
        smoothedRef.current * (1 - SMOOTHING) + clamped * SMOOTHING;
      const level = smoothedRef.current;

      // Shift bars outward from the center for a symmetric ripple.
      setBars((prev) => {
        const mid = Math.floor(BAR_COUNT / 2);
        const next = [...prev];
        // ripple: each bar inherits its inner neighbor, center gets new level.
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

  return (
    <div className={`overlay-pill ${isVisible ? "is-visible" : ""}`}>
      <div className="overlay-icon" aria-hidden>
        {state === "recording" ? <MicGlyph /> : <DotsGlyph />}
      </div>

      <div className="overlay-center">
        {state === "recording" ? (
          <div className="waveform">
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
          </div>
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
