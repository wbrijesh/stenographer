import { useCallback, useEffect, useState } from "react";
import { commands } from "@/bindings";
import { Button } from "@/components/ui";
import { GlobeIcon } from "@/components/icons";
import { GrantedBadge, StepShell } from "./StepShell";

interface GlobeKeyStepProps {
  /** Called when the user moves on (whether resolved or skipped). */
  onResolved: () => void;
}

/**
 * Step 4 — Globe (🌐 / Fn) key behavior. macOS can hijack the Fn key (input
 * source switch, emoji picker, dictation) before our event tap sees it. We read
 * `checkFnKeyBehavior` and, when it's not "Do Nothing", show guidance plus a
 * recheck. This step is non-fatal: it can always be skipped.
 */
export function GlobeKeyStep({ onResolved }: GlobeKeyStepProps) {
  const [ok, setOk] = useState<boolean | null>(null);
  const [checking, setChecking] = useState(false);

  const recheck = useCallback(async () => {
    setChecking(true);
    const result = await commands.checkFnKeyBehavior();
    setChecking(false);
    setOk(result.ok);
  }, []);

  useEffect(() => {
    void recheck();
  }, [recheck]);

  return (
    <StepShell
      icon={<GlobeIcon />}
      title="Globe key behavior"
      description="So the Fn key can trigger dictation, macOS must not repurpose it for anything else."
      done={ok === true}
    >
      {ok === true ? (
        <div className="flex justify-center">
          <GrantedBadge label="All set" />
        </div>
      ) : (
        <div className="space-y-4">
          <div className="space-y-2 rounded-xl border border-black/10 bg-black/[0.02] p-4 text-left text-sm leading-relaxed text-black/65">
            <p className="font-medium text-black/75">
              Set “Press 🌐 to…” → “Do Nothing”
            </p>
            <ol className="list-decimal space-y-1 pl-4 text-xs text-black/55">
              <li>Open System Settings → Keyboard</li>
              <li>Find “Press 🌐 key to”</li>
              <li>Choose “Do Nothing”</li>
            </ol>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="primary"
              className="flex-1"
              disabled={checking}
              onClick={() => void recheck()}
            >
              {checking ? "Checking…" : "Re-check"}
            </Button>
            <Button variant="ghost" className="flex-1" onClick={onResolved}>
              Skip for now
            </Button>
          </div>
        </div>
      )}
    </StepShell>
  );
}
