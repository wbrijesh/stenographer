import { emit, listen } from "@tauri-apps/api/event";
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";

import { commands } from "@/bindings";
import { AdvancedIcon } from "@/components/icons";

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
 *   - `mic-level`          (payload: `f32` amplitude in 0..1) -> waveform.
 *                          Each event pushes one sample into a fixed-length
 *                          rolling buffer that scrolls right-to-left; a bare
 *                          `number[]` is also tolerated (last value is used).
 *   - `partial-transcript` (payload: bare string, the transcript-so-far which
 *                          grows over time) -> live text, auto-scrolled to bottom.
 *
 * Layout (top -> bottom) at 440x132:
 *   [  scrollable EDITABLE live transcript (auto-scrolled to newest)  ]
 *   [ mic(indicator)  compact waveform           cancel ✕    ]
 *
 * The transcript is a real `<textarea>` the user can click into and edit (e.g.
 * to fix a misheard name). While recording, `partial-transcript` events keep
 * the text in sync. Once the user has focused/edited the field we no longer
 * clobber their edits; instead we APPEND only the newly-transcribed tail of
 * each cumulative partial (the suffix after the longest common prefix with the
 * previous raw partial — see `lastRawRef`). This preserves the user's edits to
 * earlier text while still surfacing freshly-spoken words. A fresh "recording"
 * session resets the edit flag, the raw-partial baseline, and the text. On edit
 * we emit `transcript-edited` (the current text) so the backend can capture
 * corrections later; no backend handler is required yet.
 *
 * The mic glyph is a static recording/transcribing indicator (not a button).
 * The cancel button emits `overlay-cancel` for the backend to handle.
 */

type OverlayState = "recording" | "transcribing";
type MicLevelPayload = number | number[];

// --- Waveform tuning ---------------------------------------------------------
// Number of bars (and therefore samples) in the scrolling waveform. One bar per
// buffered mic-level sample; the buffer scrolls left as new samples arrive.
const WAVE_SAMPLE_COUNT = 48;
// Bar geometry, in CSS pixels. WAVE_MAX_HEIGHT is the tallest a full-amplitude
// (1.0) bar can reach; WAVE_MIN_HEIGHT is the thin baseline shown at silence so
// the waveform always reads as a flat line rather than disappearing.
const WAVE_MAX_HEIGHT = 22;
const WAVE_MIN_HEIGHT = 2;
// Perceptual easing applied to the raw 0..1 amplitude before mapping to height.
// An exponent < 1 lifts quiet speech so it is visible without making loud
// speech clip, giving a natural voice-memo response curve.
const WAVE_EASE = 0.65;

const clamp01 = (n: number): number => Math.max(0, Math.min(1, n));

/**
 * Map a raw 0..1 amplitude to a bar height in pixels, applying perceptual
 * easing and clamping into the [min, max] band so silence shows a thin baseline.
 */
function levelToHeight(level: number): number {
  const eased = Math.pow(clamp01(level), WAVE_EASE);
  return WAVE_MIN_HEIGHT + eased * (WAVE_MAX_HEIGHT - WAVE_MIN_HEIGHT);
}

/** Length of the longest common prefix of two strings. */
function commonPrefixLength(a: string, b: string): number {
  const max = Math.min(a.length, b.length);
  let i = 0;
  while (i < max && a[i] === b[i]) i++;
  return i;
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
  // Live transcript-so-far. Empty until the first `partial-transcript` arrives,
  // OR whatever the user has typed once they take over editing.
  const [transcript, setTranscript] = useState<string>("");

  // --- Waveform state ---
  // Fixed-length rolling buffer of recent mic levels (0..1). New samples are
  // pushed at the end and the oldest is dropped, so index 0 is the oldest bar
  // (left) and the last index is the newest (right). Kept in a ref and rendered
  // by a single rAF loop so 50 events/sec never trigger 50 React re-renders.
  const waveBufRef = useRef<number[]>(new Array(WAVE_SAMPLE_COUNT).fill(0));
  // DOM nodes for each bar, populated on mount. The rAF loop writes heights
  // directly to these for a smooth, cheap scroll without React in the hot path.
  const waveBarRefs = useRef<(HTMLSpanElement | null)[]>([]);
  const waveRafRef = useRef<number | null>(null);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  // True once the user has focused/edited the field. While true, incoming
  // `partial-transcript` events APPEND the new tail instead of replacing. Reset
  // to false on a fresh "recording" session (new dictation). A ref (not state)
  // because the `partial-transcript` listener — registered once — reads it
  // synchronously and must always see the current value.
  const userEditedRef = useRef(false);
  // The last full raw `partial-transcript` string received (pre-edit model
  // output). Used to compute the newly-spoken delta against the next partial.
  const lastRawRef = useRef("");
  // Latest transcript text, mirrored so listeners/handlers read it without a
  // stale closure (the partial listener appends to the current value).
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
        lastRawRef.current = "";
        setTranscript("");
        // Drop any stale bars so the prior session's waveform doesn't linger.
        waveBufRef.current.fill(0);
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

    // Live transcript text. Partials are CUMULATIVE (each one is the full
    // transcript-so-far). Behaviour depends on whether the user has edited:
    //   - Not edited: mirror the raw partial directly (and remember it).
    //   - Edited: append only the newly-spoken delta — the suffix of the new
    //     raw partial after its longest common prefix with the previous raw —
    //     onto the user's CURRENT (edited) text, preserving their corrections.
    listen<string>("partial-transcript", (event) => {
      const newRaw = event.payload ?? "";

      if (!userEditedRef.current) {
        lastRawRef.current = newRaw;
        setTranscript(newRaw);
        return;
      }

      const delta = newRaw.slice(commonPrefixLength(lastRawRef.current, newRaw));
      // Always advance the baseline (even on an empty delta) so a later partial
      // diffs against the most recent raw output rather than a stale one.
      lastRawRef.current = newRaw;
      if (delta.length === 0) return;
      setTranscript(transcriptRef.current + delta);
    }).then((u) => unlisteners.push(u));

    // Mic amplitude: one `f32` (0..1) per event. Push into the rolling buffer
    // and drop the oldest; the rAF loop reads the buffer and paints. A stray
    // array payload is tolerated by taking its last value.
    listen<MicLevelPayload>("mic-level", (event) => {
      const payload = event.payload;
      const raw = Array.isArray(payload)
        ? (payload[payload.length - 1] ?? 0)
        : typeof payload === "number"
          ? payload
          : 0;
      const buf = waveBufRef.current;
      buf.shift();
      buf.push(clamp01(raw));
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  // Single rAF loop that paints the waveform from the rolling buffer. Writing
  // bar heights directly to the DOM (rather than through React state) keeps the
  // hot path off the React reconciler even at ~50 mic-level events/sec. The CSS
  // `height` transition on each bar smooths the per-frame step into a fluid,
  // right-to-left scroll. Runs for the component's whole lifetime.
  useEffect(() => {
    const paint = (): void => {
      const buf = waveBufRef.current;
      const bars = waveBarRefs.current;
      for (let i = 0; i < bars.length; i++) {
        const el = bars[i];
        if (el) el.style.height = `${levelToHeight(buf[i] ?? 0)}px`;
      }
      waveRafRef.current = requestAnimationFrame(paint);
    };
    waveRafRef.current = requestAnimationFrame(paint);
    return () => {
      if (waveRafRef.current !== null) cancelAnimationFrame(waveRafRef.current);
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
            {Array.from({ length: WAVE_SAMPLE_COUNT }, (_, i) => (
              <span
                key={i}
                ref={(el) => {
                  waveBarRefs.current[i] = el;
                }}
                className="wave-bar"
                style={{ height: `${WAVE_MIN_HEIGHT}px` }}
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
            <AdvancedIcon width={14} height={14} />
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
