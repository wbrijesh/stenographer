import { useSettingsStore } from "@/stores/settingsStore";

/**
 * Convenience hook returning the settings store. Section components are only
 * rendered once `settings` is loaded, so callers may treat `settings` as
 * present — but it is typed nullable to stay honest.
 */
export function useSettings() {
  return useSettingsStore();
}
