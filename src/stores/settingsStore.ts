import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import {
  commands,
  type AppSettings,
  type AutoSubmitKey,
  type ModelUnloadTimeout,
  type Result,
} from "@/bindings";

/**
 * A backend setter returning the standard `Result<null, string>` envelope.
 * `getAppSettings` / `getDefaultSettings` are NOT setters and are excluded.
 */
type SettingResult = Result<null, string>;

interface SettingsState {
  settings: AppSettings | null;
  defaults: AppSettings | null;
  loading: boolean;
  error: string | null;

  /** Load current + default settings from the backend. */
  initialize: () => Promise<void>;

  // General
  setTriggerModeEnabled: (v: boolean) => Promise<void>;
  setHoldTapThresholdMs: (v: number) => Promise<void>;
  setOverlayEnabled: (v: boolean) => Promise<void>;
  setStartHidden: (v: boolean) => Promise<void>;
  setAutostartEnabled: (v: boolean) => Promise<void>;
  setShowTrayIcon: (v: boolean) => Promise<void>;
  setAudioFeedback: (v: boolean) => Promise<void>;
  setAudioFeedbackVolume: (v: number) => Promise<void>;
  setMuteWhileRecording: (v: boolean) => Promise<void>;

  // Models
  setSelectedModel: (v: string | null) => Promise<void>;

  // Advanced
  setPasteDelayMs: (v: number) => Promise<void>;
  setAutoSubmit: (v: boolean) => Promise<void>;
  setAutoSubmitKey: (v: AutoSubmitKey) => Promise<void>;
  setAppendTrailingSpace: (v: boolean) => Promise<void>;
  setSelectedLanguage: (v: string) => Promise<void>;
  setTranslateToEnglish: (v: boolean) => Promise<void>;
  setModelUnloadTimeout: (v: ModelUnloadTimeout) => Promise<void>;
  setSelectedMicrophone: (v: string | null) => Promise<void>;
  setSelectedOutputDevice: (v: string | null) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>()(
  immer((set) => {
    /**
     * Generic optimistic-update-with-rollback helper.
     * Applies `value` to `settings[key]` immediately, calls the backend, and
     * rolls back on failure (rejected promise OR `{status:"error"}` envelope).
     */
    async function update<K extends keyof AppSettings, V extends AppSettings[K]>(
      key: K,
      value: V,
      persist: (value: V) => Promise<SettingResult>,
    ): Promise<void> {
      let previous: AppSettings[K] | undefined;
      set((s) => {
        if (s.settings) {
          previous = s.settings[key];
          s.settings[key] = value;
        }
        s.error = null;
      });

      const rollback = (message: string) => {
        set((s) => {
          if (s.settings && previous !== undefined) {
            s.settings[key] = previous;
          }
          s.error = message;
        });
      };

      try {
        const result = await persist(value);
        if (result.status === "error") {
          rollback(result.error);
        }
      } catch (e) {
        rollback(e instanceof Error ? e.message : String(e));
        throw e;
      }
    }

    return {
      settings: null,
      defaults: null,
      loading: false,
      error: null,

      initialize: async () => {
        set((s) => {
          s.loading = true;
        });
        const [settings, defaults] = await Promise.all([
          commands.getAppSettings(),
          commands.getDefaultSettings(),
        ]);
        set((s) => {
          s.settings = settings;
          s.defaults = defaults;
          s.loading = false;
        });
      },

      // General
      setTriggerModeEnabled: (v) =>
        update("trigger_mode_enabled", v, commands.changeTriggerModeEnabled),
      setHoldTapThresholdMs: (v) =>
        update("hold_tap_threshold_ms", v, commands.changeHoldTapThresholdMs),
      setOverlayEnabled: (v) =>
        update("overlay_enabled", v, commands.changeOverlayEnabled),
      setStartHidden: (v) => update("start_hidden", v, commands.changeStartHidden),
      setAutostartEnabled: (v) =>
        update("autostart_enabled", v, commands.changeAutostartEnabled),
      setShowTrayIcon: (v) =>
        update("show_tray_icon", v, commands.changeShowTrayIcon),
      setAudioFeedback: (v) =>
        update("audio_feedback", v, commands.changeAudioFeedback),
      setAudioFeedbackVolume: (v) =>
        update("audio_feedback_volume", v, commands.changeAudioFeedbackVolume),
      setMuteWhileRecording: (v) =>
        update("mute_while_recording", v, commands.changeMuteWhileRecording),

      // Models
      setSelectedModel: (v) =>
        update("selected_model", v, commands.changeSelectedModel),

      // Advanced
      setPasteDelayMs: (v) =>
        update("paste_delay_ms", v, commands.changePasteDelayMs),
      setAutoSubmit: (v) => update("auto_submit", v, commands.changeAutoSubmit),
      setAutoSubmitKey: (v) =>
        update("auto_submit_key", v, commands.changeAutoSubmitKey),
      setAppendTrailingSpace: (v) =>
        update("append_trailing_space", v, commands.changeAppendTrailingSpace),
      setSelectedLanguage: (v) =>
        update("selected_language", v, commands.changeSelectedLanguage),
      setTranslateToEnglish: (v) =>
        update("translate_to_english", v, commands.changeTranslateToEnglish),
      setModelUnloadTimeout: (v) =>
        update("model_unload_timeout", v, commands.changeModelUnloadTimeout),
      setSelectedMicrophone: (v) =>
        update("selected_microphone", v, commands.setSelectedMicrophone),
      setSelectedOutputDevice: (v) =>
        update("selected_output_device", v, commands.setSelectedOutputDevice),
    };
  }),
);
