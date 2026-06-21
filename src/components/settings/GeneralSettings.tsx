import { Row, Section, Slider, Toggle } from "@/components/ui";
import type { AppSettings } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { useCleanupAvailable } from "@/hooks/useCleanupAvailable";
import { ShortcutRecorder } from "@/components/ShortcutRecorder";

interface Props {
  settings: AppSettings;
}

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
  } = useSettingsStore();

  const audioFeedback = settings.audio_feedback ?? false;
  const cleanupAvailable = useCleanupAvailable();

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

      {cleanupAvailable && (
        <Section
          title="Transcription"
          description="How transcribed text is processed before pasting."
        >
          <Row
            title="Clean up transcription with AI"
            description="Uses Apple's on-device model to fix punctuation, capitalization, and filler words before pasting. Runs entirely on-device."
          >
            <Toggle
              aria-label="Clean up transcription with AI"
              checked={settings.cleanup_enabled ?? true}
              onChange={(v) => void setCleanupEnabled(v)}
            />
          </Row>
        </Section>
      )}

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
