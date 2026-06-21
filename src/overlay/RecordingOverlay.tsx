import { emit, listen } from "@tauri-apps/api/event";
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";

import { commands } from "@/bindings";

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
 *   [  scrollable EDITABLE live transcript (auto-scrolled to newest)  ]
 *   [ mic(indicator)  compact waveform           cancel ✕    ]
 *
 * The transcript is a real `<textarea>` the user can click into and edit (e.g.
 * to fix a misheard name). While recording, `partial-transcript` events keep
 * the text in sync — BUT once the user has focused/edited the field, incoming
 * partials STOP clobbering their edits (tracked via `userEditedRef`). A fresh
 * "recording" session resets the flag and clears the text. On edit we emit
 * `transcript-edited` (the current text) so the backend can capture corrections
 * later; no backend handler is required yet.
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
  settingsLabel: "Open Settings",
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
  // Live transcript-so-far. Empty until the first `partial-transcript` arrives,
  // OR whatever the user has typed once they take over editing.
  const [transcript, setTranscript] = useState<string>("");
  const smoothedRef = useRef(0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  // True once the user has focused/edited the field. While true, incoming
  // `partial-transcript` events must NOT overwrite the user's edits. Reset to
  // false on a fresh "recording" session (new dictation). A ref (not state)
  // because the `partial-transcript` listener — registered once — reads it
  // synchronously and must always see the current value.
  const userEditedRef = useRef(false);
  // Latest transcript text, mirrored for emit-on-blur without stale closures.
  const transcriptRef = useRef("");
  transcriptRef.current = transcript;
  // Debounce timer for emitting `transcript-edited` while the user types.
  const emitDebounceRef = useRef<number | null>(null);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<OverlayState>("show-overlay", (event) => {
      const next = event.payload ?? "recording";
      // A fresh recording session: clear any stale transcript so old text from
      // the previous dictation does not linger, and let live partials drive the
      // field again (the user has not edited THIS new session yet).
      if (next === "recording") {
        userEditedRef.current = false;
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

    // Live transcript text (grows over time). Bare string payload. Once the
    // user has taken over editing, do NOT clobber their text with partials.
    listen<string>("partial-transcript", (event) => {
      if (userEditedRef.current) return;
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
  // the latest words are always visible — but ONLY while live partials drive
  // the field. Once the user is editing we must not yank their caret/scroll
  // around. useLayoutEffect runs before paint to avoid a visible jump.
  useLayoutEffect(() => {
    if (userEditedRef.current) return;
    const el = textareaRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [transcript]);

  // Flush any pending debounced emit on unmount.
  useEffect(() => {
    return () => {
      if (emitDebounceRef.current !== null) {
        window.clearTimeout(emitDebounceRef.current);
      }
    };
  }, []);

  const onCancel = (): void => {
    void emit("overlay-cancel");
  };

  // Open the main Settings window. This is a critical escape hatch: with the
  // menu-bar icon disabled, the overlay's gear is the only way in. It must NOT
  // touch recording state — just show Settings.
  const onSettings = (): void => {
    void commands.openSettings();
  };

  // The user has taken over the field: from now on, partials must not clobber
  // their text. Set on focus so even a click-with-no-typing locks the field.
  const markUserEdited = (): void => {
    userEditedRef.current = true;
  };

  const onTranscriptChange = (
    e: React.ChangeEvent<HTMLTextAreaElement>,
  ): void => {
    userEditedRef.current = true;
    const value = e.target.value;
    setTranscript(value);
    // Debounced emit so the backend can capture corrections as they happen.
    if (emitDebounceRef.current !== null) {
      window.clearTimeout(emitDebounceRef.current);
    }
    emitDebounceRef.current = window.setTimeout(() => {
      void emit("transcript-edited", value);
    }, 400);
  };

  const onTranscriptBlur = (): void => {
    // Emit the final text on blur (cancel any pending debounce first).
    if (emitDebounceRef.current !== null) {
      window.clearTimeout(emitDebounceRef.current);
      emitDebounceRef.current = null;
    }
    if (userEditedRef.current) {
      void emit("transcript-edited", transcriptRef.current);
    }
  };

  // Keep Escape working as cancel even when the textarea has focus: don't let
  // the field swallow it. Emit cancel and let it bubble.
  const onTranscriptKeyDown = (
    e: React.KeyboardEvent<HTMLTextAreaElement>,
  ): void => {
    if (e.key === "Escape") {
      void emit("overlay-cancel");
    }
  };

  const hasText = transcript.length > 0;

  // Empty-state placeholder depends on phase: while recording we are listening,
  // while transcribing we are finishing up.
  const placeholder =
    state === "recording" ? STRINGS.listening : STRINGS.transcribing;

  return (
    <div className="overlay-panel">
      <div className="overlay-scroll">
        <textarea
          ref={textareaRef}
          className={`overlay-transcript-input${
            hasText ? "" : " is-empty"
          }`}
          value={transcript}
          placeholder={placeholder}
          spellCheck={false}
          aria-label="Transcript (editable)"
          onChange={onTranscriptChange}
          onFocus={markUserEdited}
          onBlur={onTranscriptBlur}
          onKeyDown={onTranscriptKeyDown}
        />
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

        <div className="overlay-controls-right">
          <button
            type="button"
            className="settings-btn"
            aria-label={STRINGS.settingsLabel}
            title={STRINGS.settingsLabel}
            onClick={onSettings}
          >
            <SettingsGlyph />
          </button>

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

const SettingsGlyph: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
    <circle cx="12" cy="12" r="3" stroke="currentColor" strokeWidth="2" />
    <path
      d="M12 2v3M12 19v3M4.2 6.6l2.1 2.1M17.7 15.3l2.1 2.1M2 12h3M19 12h3M4.2 17.4l2.1-2.1M17.7 8.7l2.1-2.1"
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
