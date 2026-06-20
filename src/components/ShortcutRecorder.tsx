import { useCallback, useEffect, useState } from "react";
import { commands } from "@/bindings";
import { Button } from "@/components/ui";
import { useSettingsStore } from "@/stores/settingsStore";
import { DEFAULT_TRIGGER_BINDING, formatBinding } from "@/lib/shortcut";

/**
 * Reusable control for viewing and re-recording the global trigger shortcut.
 *
 * Shows the current binding as a friendly "key cap" label and a button that
 * captures the user's next key gesture via `captureShortcut()`, then applies it
 * with `changeTriggerBinding()`. Capture is a one-at-a-time interaction: the
 * button is disabled while listening, and Escape cancels (handled by the
 * backend, surfaced here as a `"cancelled"` error). Also offers a subtle
 * "reset to default" affordance.
 */
export function ShortcutRecorder() {
  const triggerBinding = useSettingsStore((s) => s.triggerBinding);
  const refreshTriggerBinding = useSettingsStore(
    (s) => s.refreshTriggerBinding,
  );

  const [listening, setListening] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  // Load the persisted binding if the store hasn't been populated yet (e.g. in
  // onboarding, where the settings store isn't initialized).
  useEffect(() => {
    if (triggerBinding === null) void refreshTriggerBinding();
  }, [triggerBinding, refreshTriggerBinding]);

  const current = triggerBinding ?? DEFAULT_TRIGGER_BINDING;
  const isDefault = current === DEFAULT_TRIGGER_BINDING;

  /** Persist + apply a binding, then refresh the displayed value. */
  const applyBinding = useCallback(
    async (binding: string): Promise<boolean> => {
      const result = await commands.changeTriggerBinding(binding);
      if (result.status === "error") {
        setHint(`Couldn't set shortcut: ${result.error}`);
        return false;
      }
      await refreshTriggerBinding();
      return true;
    },
    [refreshTriggerBinding],
  );

  const record = useCallback(async () => {
    if (listening) return;
    setHint(null);
    setListening(true);
    try {
      const captured = await commands.captureShortcut();
      if (captured.status === "error") {
        if (captured.error === "cancelled") {
          setHint("Cancelled — shortcut unchanged.");
        } else if (captured.error === "timed out") {
          setHint("No key detected — try again.");
        } else {
          setHint(`Couldn't capture shortcut: ${captured.error}`);
        }
        return;
      }
      const ok = await applyBinding(captured.data);
      if (ok) setHint(null);
    } catch (e) {
      setHint(
        `Couldn't capture shortcut: ${
          e instanceof Error ? e.message : String(e)
        }`,
      );
    } finally {
      setListening(false);
    }
  }, [listening, applyBinding]);

  const resetToDefault = useCallback(() => {
    setHint(null);
    void applyBinding(DEFAULT_TRIGGER_BINDING);
  }, [applyBinding]);

  return (
    <div className="flex flex-col items-end gap-1.5">
      <div className="flex items-center gap-2">
        {listening ? (
          <span className="inline-flex items-center gap-1.5 rounded-lg border border-blue-500/40 bg-blue-500/10 px-3 py-1.5 text-sm font-medium text-blue-700">
            <span className="h-2 w-2 animate-pulse rounded-full bg-blue-500" />
            Listening… press your shortcut (Esc to cancel)
          </span>
        ) : (
          <kbd className="inline-flex min-w-[3rem] items-center justify-center rounded-lg border border-black/15 bg-black/[0.04] px-2.5 py-1 font-mono text-sm font-semibold text-black/75 shadow-sm">
            {formatBinding(current)}
          </kbd>
        )}
        <Button
          variant="secondary"
          size="sm"
          disabled={listening}
          onClick={() => void record()}
        >
          {listening ? "Listening…" : "Change"}
        </Button>
      </div>

      {!isDefault && !listening && (
        <button
          type="button"
          className="text-xs text-black/45 underline-offset-2 transition-colors hover:text-black/70 hover:underline"
          onClick={resetToDefault}
        >
          Reset to default (Right ⌘)
        </button>
      )}

      {hint && (
        <p className="text-xs text-black/45" role="status">
          {hint}
        </p>
      )}
    </div>
  );
}
