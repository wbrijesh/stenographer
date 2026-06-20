import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import { commands, type AppSettings } from "@/bindings";

interface SettingsState {
  settings: AppSettings | null;
  loading: boolean;
  /** Load the current settings from the backend. */
  initialize: () => Promise<void>;
  /** Persist a new paste delay (proves a numeric setter round-trip). */
  setPasteDelayMs: (ms: number) => Promise<void>;
  /** Persist the overlay-enabled toggle (proves a bool setter round-trip). */
  setOverlayEnabled: (enabled: boolean) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>()(
  immer((set) => ({
    settings: null,
    loading: false,

    initialize: async () => {
      set((s) => {
        s.loading = true;
      });
      const settings = await commands.getAppSettings();
      set((s) => {
        s.settings = settings;
        s.loading = false;
      });
    },

    setPasteDelayMs: async (ms) => {
      // Optimistic update with rollback on failure.
      let previous: number | undefined;
      set((s) => {
        if (s.settings) {
          previous = s.settings.paste_delay_ms;
          s.settings.paste_delay_ms = ms;
        }
      });
      try {
        await commands.changePasteDelayMs(ms);
      } catch (e) {
        set((s) => {
          if (s.settings && previous !== undefined) {
            s.settings.paste_delay_ms = previous;
          }
        });
        throw e;
      }
    },

    setOverlayEnabled: async (enabled) => {
      let previous: boolean | undefined;
      set((s) => {
        if (s.settings) {
          previous = s.settings.overlay_enabled;
          s.settings.overlay_enabled = enabled;
        }
      });
      try {
        await commands.changeOverlayEnabled(enabled);
      } catch (e) {
        set((s) => {
          if (s.settings && previous !== undefined) {
            s.settings.overlay_enabled = previous;
          }
        });
        throw e;
      }
    },
  })),
);
