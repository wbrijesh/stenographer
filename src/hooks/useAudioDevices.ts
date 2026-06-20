import { useEffect, useState } from "react";
import type { SelectOption } from "@/components/ui";
import { commands, type DeviceInfo } from "@/bindings";

/**
 * Audio device enumeration backed by the real backend commands
 * (`get_available_microphones` / `get_available_output_devices`).
 *
 * The pickers always offer a "System default" option (value `""`) which maps to
 * `null` when persisted. Any saved device that is no longer present is still
 * surfaced so the user sees their stored choice.
 */

export type DeviceKind = "input" | "output";

const SYSTEM_DEFAULT: SelectOption = {
  value: "",
  label: "System default",
};

/** Module-level cache so both pickers share a single enumeration per kind. */
const cache: Record<DeviceKind, DeviceInfo[] | undefined> = {
  input: undefined,
  output: undefined,
};

async function fetchDevices(kind: DeviceKind): Promise<DeviceInfo[]> {
  return kind === "input"
    ? commands.getAvailableMicrophones()
    : commands.getAvailableOutputDevices();
}

/**
 * Enumerate the available devices for a given kind. Returns the raw
 * `DeviceInfo[]`; an empty list while loading (or on failure).
 */
export function useAudioDevices(kind: DeviceKind): DeviceInfo[] {
  const [devices, setDevices] = useState<DeviceInfo[]>(cache[kind] ?? []);

  useEffect(() => {
    let cancelled = false;
    void fetchDevices(kind)
      .then((list) => {
        cache[kind] = list;
        if (!cancelled) setDevices(list);
      })
      .catch(() => {
        if (!cancelled) setDevices([]);
      });
    return () => {
      cancelled = true;
    };
  }, [kind]);

  return devices;
}

/**
 * Build select options for a device picker. Always includes "System default"
 * (value `""`), followed by every enumerated device. `current` is the persisted
 * value; if set but not present in the enumerated list, it is surfaced so the
 * user still sees their saved choice.
 */
export function useDeviceOptions(
  current: string | null,
  kind: DeviceKind,
): SelectOption[] {
  const devices = useAudioDevices(kind);

  const options: SelectOption[] = [SYSTEM_DEFAULT];
  const seen = new Set<string>();

  for (const device of devices) {
    if (device.name.length === 0 || seen.has(device.name)) continue;
    seen.add(device.name);
    options.push({
      value: device.name,
      label: device.is_default ? `${device.name} (default)` : device.name,
    });
  }

  if (current && current.length > 0 && !seen.has(current)) {
    options.push({ value: current, label: current });
  }

  return options;
}
