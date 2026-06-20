import { useCallback, useEffect, useRef, useState } from "react";
import { commands } from "@/bindings";
import { Button } from "@/components/ui";
import { AccessibilityIcon, ExternalLinkIcon } from "@/components/icons";
import { pollPermission } from "@/hooks/useOnboarding";
import { GrantedBadge, StepShell } from "./StepShell";

interface AccessibilityStepProps {
  /** Whether the permission is already granted (seeds the ✓ state). */
  granted: boolean;
  /** Re-run the gate's prerequisite checks after a grant. */
  onGranted: () => void;
}

/**
 * Step 3 — Accessibility (AX). Requests the permission, polls until granted,
 * then initializes both the Enigo keystroke injector and the global Fn-key
 * shortcut listener (both require AX and are idempotent). If either init fails
 * we surface the error but still treat the permission as granted.
 */
export function AccessibilityStep({
  granted,
  onGranted,
}: AccessibilityStepProps) {
  const [requesting, setRequesting] = useState(false);
  const [stuck, setStuck] = useState(false);
  const [initError, setInitError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    return () => abortRef.current?.abort();
  }, []);

  const initialize = useCallback(async () => {
    const [enigo, shortcuts] = await Promise.all([
      commands.initializeEnigo(),
      commands.initializeShortcuts(),
    ]);
    if (enigo.status === "error") return enigo.error;
    if (shortcuts.status === "error") return shortcuts.error;
    return null;
  }, []);

  const request = useCallback(async () => {
    setStuck(false);
    setInitError(null);
    setRequesting(true);
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    await commands.requestAccessibilityPermission();
    const ok = await pollPermission(commands.checkAccessibilityPermission, {
      signal: controller.signal,
    });

    if (controller.signal.aborted) return;
    setRequesting(false);

    if (ok) {
      setInitError(await initialize());
      onGranted();
    } else {
      setStuck(true);
    }
  }, [initialize, onGranted]);

  return (
    <StepShell
      icon={<AccessibilityIcon />}
      title="Accessibility access"
      description="Needed so Stenographer can listen for the Fn key globally and type the transcribed text into any app."
      done={granted}
    >
      {granted ? (
        <div className="space-y-3">
          <div className="flex justify-center">
            <GrantedBadge />
          </div>
          {initError && (
            <p className="text-xs leading-relaxed text-amber-600">
              Permission granted, but setup hit a snag: {initError}
            </p>
          )}
        </div>
      ) : (
        <div className="space-y-3">
          <Button
            variant="primary"
            className="w-full"
            disabled={requesting}
            onClick={() => void request()}
          >
            {requesting
              ? "Waiting for permission…"
              : "Grant accessibility access"}
          </Button>
          {stuck && (
            <p className="text-xs leading-relaxed text-black/50">
              Still waiting. Open{" "}
              <span className="font-medium text-black/70">
                System Settings → Privacy &amp; Security → Accessibility
              </span>{" "}
              and enable Stenographer, then try again.
            </p>
          )}
          <button
            type="button"
            className="mx-auto flex items-center gap-1 text-xs text-blue-600 hover:underline"
            onClick={() => void request()}
          >
            <span className="h-3.5 w-3.5">
              <ExternalLinkIcon />
            </span>
            Re-check
          </button>
        </div>
      )}
    </StepShell>
  );
}
