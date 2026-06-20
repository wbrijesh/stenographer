import type { ModelUnloadTimeout } from "@/bindings";
import type { SelectOption } from "@/components/ui";

/**
 * `ModelUnloadTimeout` is a union: "never" | "immediately" | {seconds} | {minutes}.
 * We expose a fixed set of presets in the UI and serialize them to/from a stable
 * string key for the <Select>.
 */

interface Preset {
  key: string;
  label: string;
  value: ModelUnloadTimeout;
}

export const UNLOAD_PRESETS: Preset[] = [
  { key: "immediately", label: "Immediately", value: "immediately" },
  { key: "30s", label: "After 30 seconds", value: { seconds: 30 } },
  { key: "1m", label: "After 1 minute", value: { minutes: 1 } },
  { key: "5m", label: "After 5 minutes", value: { minutes: 5 } },
  { key: "15m", label: "After 15 minutes", value: { minutes: 15 } },
  { key: "never", label: "Never (keep loaded)", value: "never" },
];

export const UNLOAD_OPTIONS: SelectOption[] = UNLOAD_PRESETS.map((p) => ({
  value: p.key,
  label: p.label,
}));

/** Map a `ModelUnloadTimeout` value to its preset key (defaults to "never"). */
export function timeoutToKey(value: ModelUnloadTimeout | undefined): string {
  if (value === undefined) return "never";
  if (value === "never" || value === "immediately") return value;
  if ("seconds" in value) {
    const match = UNLOAD_PRESETS.find(
      (p) =>
        typeof p.value === "object" &&
        "seconds" in p.value &&
        p.value.seconds === value.seconds,
    );
    return match?.key ?? "30s";
  }
  if ("minutes" in value) {
    const match = UNLOAD_PRESETS.find(
      (p) =>
        typeof p.value === "object" &&
        "minutes" in p.value &&
        p.value.minutes === value.minutes,
    );
    return match?.key ?? "5m";
  }
  return "never";
}

export function keyToTimeout(key: string): ModelUnloadTimeout {
  const match = UNLOAD_PRESETS.find((p) => p.key === key);
  return match?.value ?? "never";
}
