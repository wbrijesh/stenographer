import { useEffect, useState } from "react";
import { Button, Row, Section, Slider, Toggle } from "@/components/ui";
import { commands, type AppSettings } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { ShortcutRecorder } from "@/components/ShortcutRecorder";

interface Props {
  settings: AppSettings;
}

/** Text input that commits on blur / Enter so partial edits don't spam the backend. */
function SettingsTextInput({
  value,
  onCommit,
  placeholder,
  type = "text",
  disabled = false,
  ...rest
}: {
  value: string;
  onCommit: (value: string) => void;
  placeholder?: string;
  type?: "text" | "password";
  disabled?: boolean;
  "aria-label"?: string;
}) {
  const [draft, setDraft] = useState(value);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  const commit = () => {
    if (draft !== value) onCommit(draft);
  };

  return (
    <input
      type={type}
      value={draft}
      placeholder={placeholder}
      disabled={disabled}
      aria-label={rest["aria-label"]}
      autoComplete="off"
      autoCorrect="off"
      autoCapitalize="off"
      spellCheck={false}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
      }}
      className="w-full rounded-lg border border-black/15 bg-white px-3 py-1.5 text-sm shadow-sm transition-colors hover:border-black/30 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
    />
  );
}

type TestState =
  | { status: "idle" }
  | { status: "pending" }
  | { status: "ok"; output: string }
  | { status: "error"; message: string };

export function GeneralSettings({ settings }: Props) {
  const {
    setTriggerModeEnabled,
    setOverlayEnabled,
    setStartHidden,
    setAutostartEnabled,
    setShowTrayIcon,
    setAudioFeedback,
    setAudioFeedbackVolume,
    setMuteWhileRecording,
    setCleanupEnabled,
    setLlmBaseUrl,
    setLlmApiKey,
    setLlmModel,
  } = useSettingsStore();

  const audioFeedback = settings.audio_feedback ?? false;
  const cleanupEnabled = settings.cleanup_enabled ?? false;

  // Whether an API key is configured on the backend ("paste with cleanup" hint).
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [test, setTest] = useState<TestState>({ status: "idle" });

  // Re-query "is configured" whenever the locally-known key changes so the hint
  // and status stay in sync with what the backend actually has stored.
  useEffect(() => {
    let cancelled = false;
    void commands.isCleanupConfigured().then((v) => {
      if (!cancelled) setConfigured(v);
    });
    return () => {
      cancelled = true;
    };
  }, [settings.llm_api_key]);

  const runTest = async () => {
    setTest({ status: "pending" });
    try {
      const result = await commands.testLlmConnection();
      if (result.status === "ok") {
        setTest({ status: "ok", output: result.data });
      } else {
        setTest({ status: "error", message: result.error });
      }
    } catch (e) {
      setTest({
        status: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  return (
    <div className="space-y-6">
      <Section
        title="Trigger"
        description="How recording is started and stopped."
      >
        <Row
          title="Trigger shortcut"
          description="Press your trigger once to start, again to stop."
        >
          <ShortcutRecorder />
        </Row>
        <Row
          title="Enable trigger"
          description="Turn the global trigger shortcut on or off."
        >
          <Toggle
            aria-label="Enable trigger"
            checked={settings.trigger_mode_enabled ?? false}
            onChange={(v) => void setTriggerModeEnabled(v)}
          />
        </Row>
      </Section>

      <Section
        title="Transcription"
        description="How transcribed text is processed before pasting."
      >
        <Row
          title="Clean up & format transcription"
          description="Sends the transcript to a hosted model to fix punctuation, capitalization, and filler words before pasting."
        >
          <Toggle
            aria-label="Clean up & format transcription"
            checked={cleanupEnabled}
            onChange={(v) => void setCleanupEnabled(v)}
          />
        </Row>

        {cleanupEnabled && (
          <>
            <Row title="API base URL" stacked disabled={!cleanupEnabled}>
              <SettingsTextInput
                aria-label="API base URL"
                value={settings.llm_base_url ?? ""}
                placeholder="https://api.openai.com/v1"
                disabled={!cleanupEnabled}
                onCommit={(v) => void setLlmBaseUrl(v)}
              />
              <p className="mt-1.5 text-xs text-black/50">
                Any OpenAI-compatible endpoint (OpenAI, Gemini, DeepSeek, etc.).
              </p>
            </Row>

            <Row title="API key" stacked disabled={!cleanupEnabled}>
              <SettingsTextInput
                aria-label="API key"
                type="password"
                value={settings.llm_api_key ?? ""}
                placeholder="sk-…"
                disabled={!cleanupEnabled}
                onCommit={(v) => void setLlmApiKey(v)}
              />
              <p className="mt-1.5 text-xs text-black/50">
                Stored locally on this Mac.
              </p>
            </Row>

            <Row title="Model" stacked disabled={!cleanupEnabled}>
              <SettingsTextInput
                aria-label="Model"
                value={settings.llm_model ?? ""}
                placeholder="gpt-5-nano"
                disabled={!cleanupEnabled}
                onCommit={(v) => void setLlmModel(v)}
              />
            </Row>

            <Row title="Test connection" stacked disabled={!cleanupEnabled}>
              <div className="space-y-2">
                <Button
                  variant="secondary"
                  disabled={!cleanupEnabled || test.status === "pending"}
                  onClick={() => void runTest()}
                >
                  {test.status === "pending" && (
                    <span
                      aria-hidden
                      className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-black/20 border-t-black/60"
                    />
                  )}
                  {test.status === "pending"
                    ? "Testing…"
                    : "Test connection"}
                </Button>

                {test.status === "ok" && (
                  <p className="flex items-start gap-1.5 text-xs text-green-600">
                    <span aria-hidden>✓</span>
                    <span className="min-w-0 break-words">
                      Connected. Sample: “{test.output}”
                    </span>
                  </p>
                )}
                {test.status === "error" && (
                  <p className="flex items-start gap-1.5 text-xs text-red-600">
                    <span aria-hidden>✕</span>
                    <span className="min-w-0 break-words">{test.message}</span>
                  </p>
                )}

                {configured === false && (
                  <p className="text-xs text-black/45">
                    Not configured — transcripts will paste without cleanup.
                  </p>
                )}
              </div>
            </Row>
          </>
        )}
      </Section>

      <Section title="Appearance & startup">
        <Row
          title="Recording overlay"
          description="Show a floating indicator while recording."
        >
          <Toggle
            aria-label="Recording overlay"
            checked={settings.overlay_enabled ?? false}
            onChange={(v) => void setOverlayEnabled(v)}
          />
        </Row>
        <Row
          title="Start hidden"
          description="Launch without showing the settings window."
        >
          <Toggle
            aria-label="Start hidden"
            checked={settings.start_hidden ?? false}
            onChange={(v) => void setStartHidden(v)}
          />
        </Row>
        <Row
          title="Launch at login"
          description="Start Stenographer automatically when you log in."
        >
          <Toggle
            aria-label="Launch at login"
            checked={settings.autostart_enabled ?? false}
            onChange={(v) => void setAutostartEnabled(v)}
          />
        </Row>
        <Row
          title="Show menu bar icon"
          description="Display the Stenographer icon in the macOS menu bar."
        >
          <Toggle
            aria-label="Show menu bar icon"
            checked={settings.show_tray_icon ?? false}
            onChange={(v) => void setShowTrayIcon(v)}
          />
        </Row>
      </Section>

      <Section title="Sound">
        <Row
          title="Audio feedback"
          description="Play a sound when recording starts and stops."
        >
          <Toggle
            aria-label="Audio feedback"
            checked={audioFeedback}
            onChange={(v) => void setAudioFeedback(v)}
          />
        </Row>
        <Row
          title="Feedback volume"
          stacked
          disabled={!audioFeedback}
        >
          <Slider
            aria-label="Feedback volume"
            min={0}
            max={1}
            step={0.01}
            value={settings.audio_feedback_volume ?? 0.5}
            disabled={!audioFeedback}
            onChange={(v) => void setAudioFeedbackVolume(v)}
            formatValue={(v) => `${Math.round(v * 100)}%`}
          />
        </Row>
        <Row
          title="Mute while recording"
          description="Silence other audio output during recording."
        >
          <Toggle
            aria-label="Mute while recording"
            checked={settings.mute_while_recording ?? false}
            onChange={(v) => void setMuteWhileRecording(v)}
          />
        </Row>
      </Section>
    </div>
  );
}
