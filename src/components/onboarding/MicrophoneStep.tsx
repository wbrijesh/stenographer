import { useCallback, useEffect, useRef, useState } from "react";
import { commands } from "@/bindings";
import { Button } from "@/components/ui";
import { ExternalLinkIcon, MicIcon } from "@/components/icons";
import { pollPermission } from "@/hooks/useOnboarding";
import { GrantedBadge, StepShell } from "./StepShell";

interface MicrophoneStepProps {
  /** Whether the permission is already granted (seeds the ✓ state). */
  granted: boolean;
  /** Re-run the gate's prerequisite checks after a grant. */
  onGranted: () => void;
}

/**
 * Step 2 — Microphone. Fires the (fire-and-forget) macOS request, then polls
 * `checkMicrophonePermission` until it flips to granted. If polling exhausts
 * without a grant we surface a hint to open System Settings manually.
 */
export function MicrophoneStep({ granted, onGranted }: MicrophoneStepProps) {
  const [requesting, setRequesting] = useState(false);
  const [stuck, setStuck] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    return () => abortRef.current?.abort();
  }, []);

  const request = useCallback(async () => {
    setStuck(false);
    setRequesting(true);
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    await commands.requestMicrophonePermission();
    const ok = await pollPermission(commands.checkMicrophonePermission, {
      signal: controller.signal,
    });

    if (controller.signal.aborted) return;
    setRequesting(false);
    if (ok) {
      onGranted();
    } else {
      setStuck(true);
    }
  }, [onGranted]);

  return (
    <StepShell
      icon={<MicIcon />}
      title="Microphone access"
      description="Stenographer records your voice locally to transcribe it. Audio never leaves your Mac."
      done={granted}
    >
      {granted ? (
        <div className="flex justify-center">
          <GrantedBadge />
        </div>
      ) : (
        <div className="space-y-3">
          <Button
            variant="primary"
            className="w-full"
            disabled={requesting}
            onClick={() => void request()}
          >
            {requesting ? "Waiting for permission…" : "Grant microphone access"}
          </Button>
          {stuck && (
            <p className="text-secondary text-[12px] leading-relaxed">
              Still waiting. Open{" "}
              <span className="text-label font-medium">
                System Settings → Privacy &amp; Security → Microphone
              </span>{" "}
              and enable Stenographer, then try again.
            </p>
          )}
          <button
            type="button"
            className="text-accent mx-auto flex items-center gap-1 text-[12px] hover:underline"
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
