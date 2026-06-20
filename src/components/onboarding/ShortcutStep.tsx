import { Button } from "@/components/ui";
import { GlobeIcon } from "@/components/icons";
import { ShortcutRecorder } from "@/components/ShortcutRecorder";
import { StepShell } from "./StepShell";

interface ShortcutStepProps {
  /** Called when the user moves on (the default works, so this never gates). */
  onContinue: () => void;
}

/**
 * Trigger shortcut step. Stenographer is triggered by Right ⌘ by default —
 * press once to start dictating, again to stop. This step is non-gating: the
 * default works out of the box, so the user can change it here or just continue.
 */
export function ShortcutStep({ onContinue }: ShortcutStepProps) {
  return (
    <StepShell
      icon={<GlobeIcon />}
      title="Your trigger shortcut"
      description="Stenographer is triggered by Right ⌘ by default — press it once to start dictating, again to stop. You can change it here."
    >
      <div className="space-y-4">
        <div className="flex items-center justify-between rounded-xl border border-black/10 bg-black/[0.02] px-4 py-3 text-left">
          <span className="text-sm font-medium text-black/75">
            Trigger shortcut
          </span>
          <ShortcutRecorder />
        </div>
        <Button variant="primary" className="w-full" onClick={onContinue}>
          Continue
        </Button>
      </div>
    </StepShell>
  );
}
