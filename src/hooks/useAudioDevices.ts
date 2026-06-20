import type { SelectOption } from "@/components/ui";

/**
 * Audio device enumeration.
 *
 * NOTE: bindings.ts exposes `setSelectedMicrophone` / `setSelectedOutputDevice`
 * (the SETTERS) but does NOT expose a command to LIST available devices, nor an
 * event announcing device changes. Until the orchestrator adds something like
 * `commands.getAudioDevices()` / `commands.getOutputDevices()`, the device
 * pickers render only a "System default" option plus whatever value is already
 * persisted in settings.
 *
 * MISSING BACKEND COMMANDS (orchestrator to add):
 *   - get_audio_devices  -> string[] (input device names)
 *   - get_output_devices -> string[] (output device names)
 */

const SYSTEM_DEFAULT: SelectOption = {
  value: "",
  label: "System default",
};

/**
 * Build select options for a device picker. `current` is the persisted value;
 * if it isn't `null`/empty and isn't already represented, it's surfaced so the
 * user still sees their saved choice.
 */
export function useDeviceOptions(current: string | null): SelectOption[] {
  const options: SelectOption[] = [SYSTEM_DEFAULT];
  if (current && current.length > 0) {
    options.push({ value: current, label: current });
  }
  return options;
}
