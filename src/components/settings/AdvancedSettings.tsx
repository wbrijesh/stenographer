import {
  NumberInput,
  Row,
  Section,
  Select,
  Toggle,
  type SelectOption,
} from "@/components/ui";
import type { AppSettings, AutoSubmitKey } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { LANGUAGE_OPTIONS } from "@/lib/languages";
import {
  UNLOAD_OPTIONS,
  keyToTimeout,
  timeoutToKey,
} from "@/lib/modelUnloadTimeout";
import { useDeviceOptions } from "@/hooks/useAudioDevices";

interface Props {
  settings: AppSettings;
}

const AUTO_SUBMIT_KEY_OPTIONS: SelectOption[] = [
  { value: "enter", label: "Enter" },
  { value: "ctrl_enter", label: "Control + Enter" },
  { value: "cmd_enter", label: "Command + Enter" },
];

const LANGUAGE_SELECT_OPTIONS: SelectOption[] = LANGUAGE_OPTIONS.map((l) => ({
  value: l.value,
  label: l.label,
}));

export function AdvancedSettings({ settings }: Props) {
  const {
    setPasteDelayMs,
    setAutoSubmit,
    setAutoSubmitKey,
    setAppendTrailingSpace,
    setSelectedLanguage,
    setTranslateToEnglish,
    setModelUnloadTimeout,
    setSelectedMicrophone,
    setSelectedOutputDevice,
  } = useSettingsStore();

  const autoSubmit = settings.auto_submit ?? false;
  const micOptions = useDeviceOptions(settings.selected_microphone ?? null);
  const outputOptions = useDeviceOptions(
    settings.selected_output_device ?? null,
  );

  return (
    <div className="space-y-6">
      <Section
        title="Text insertion"
        description="How transcribed text is delivered to the focused app."
      >
        <Row
          title="Paste delay"
          description="Wait before pasting, to let the target app settle."
        >
          <NumberInput
            aria-label="Paste delay"
            min={0}
            max={5000}
            step={10}
            suffix="ms"
            value={settings.paste_delay_ms ?? 0}
            onChange={(v) => void setPasteDelayMs(v)}
          />
        </Row>
        <Row
          title="Auto-submit"
          description="Press a key after pasting to send the message."
        >
          <Toggle
            aria-label="Auto-submit"
            checked={autoSubmit}
            onChange={(v) => void setAutoSubmit(v)}
          />
        </Row>
        <Row
          title="Submit key"
          description="Which key is pressed when auto-submit is on."
          disabled={!autoSubmit}
        >
          <Select
            aria-label="Submit key"
            disabled={!autoSubmit}
            options={AUTO_SUBMIT_KEY_OPTIONS}
            value={settings.auto_submit_key ?? "enter"}
            onChange={(v) => void setAutoSubmitKey(v as AutoSubmitKey)}
          />
        </Row>
        <Row
          title="Append trailing space"
          description="Add a space after inserted text."
        >
          <Toggle
            aria-label="Append trailing space"
            checked={settings.append_trailing_space ?? false}
            onChange={(v) => void setAppendTrailingSpace(v)}
          />
        </Row>
      </Section>

      <Section title="Transcription">
        <Row
          title="Language"
          description="Spoken language hint for the model."
        >
          <Select
            aria-label="Language"
            options={LANGUAGE_SELECT_OPTIONS}
            value={settings.selected_language ?? "auto"}
            onChange={(v) => void setSelectedLanguage(v)}
          />
        </Row>
        <Row
          title="Translate to English"
          description="Output English regardless of the spoken language."
        >
          <Toggle
            aria-label="Translate to English"
            checked={settings.translate_to_english ?? false}
            onChange={(v) => void setTranslateToEnglish(v)}
          />
        </Row>
        <Row
          title="Unload model after"
          description="Free memory when the model is idle for this long."
        >
          <Select
            aria-label="Unload model after"
            options={UNLOAD_OPTIONS}
            value={timeoutToKey(settings.model_unload_timeout)}
            onChange={(v) => void setModelUnloadTimeout(keyToTimeout(v))}
          />
        </Row>
      </Section>

      <Section
        title="Audio devices"
        description="Device lists require backend support that is not wired yet — only your saved device and the system default are shown."
      >
        <Row
          title="Microphone"
          description="Input device used for recording."
        >
          <Select
            aria-label="Microphone"
            options={micOptions}
            value={settings.selected_microphone ?? ""}
            onChange={(v) =>
              void setSelectedMicrophone(v.length > 0 ? v : null)
            }
          />
        </Row>
        <Row
          title="Output device"
          description="Device used for audio feedback sounds."
        >
          <Select
            aria-label="Output device"
            options={outputOptions}
            value={settings.selected_output_device ?? ""}
            onChange={(v) =>
              void setSelectedOutputDevice(v.length > 0 ? v : null)
            }
          />
        </Row>
      </Section>
    </div>
  );
}
