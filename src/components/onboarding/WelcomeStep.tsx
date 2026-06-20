import { SparkleIcon } from "@/components/icons";
import { StepShell } from "./StepShell";

/** Intro step: one-line pitch and the two trigger gestures. */
export function WelcomeStep() {
  return (
    <StepShell
      icon={<SparkleIcon />}
      title="Welcome to Stenographer"
      description="Offline push-to-talk dictation that types wherever you are — nothing leaves your Mac."
    >
      <div className="space-y-2 rounded-xl border border-black/10 bg-black/[0.02] p-4 text-left">
        <div className="flex items-center justify-between gap-3 text-sm">
          <span className="text-black/60">Hold</span>
          <kbd className="rounded-md border border-black/15 bg-white px-2 py-0.5 font-mono text-xs shadow-sm">
            Fn
          </kbd>
        </div>
        <div className="h-px bg-black/[0.06]" />
        <div className="flex items-center justify-between gap-3 text-sm">
          <span className="text-black/60">Toggle hands-free</span>
          <span className="flex items-center gap-1">
            <kbd className="rounded-md border border-black/15 bg-white px-2 py-0.5 font-mono text-xs shadow-sm">
              Fn
            </kbd>
            <span className="text-black/30">+</span>
            <kbd className="rounded-md border border-black/15 bg-white px-2 py-0.5 font-mono text-xs shadow-sm">
              Space
            </kbd>
          </span>
        </div>
      </div>
      <p className="mt-4 text-xs text-black/40">
        A quick setup grants the permissions it needs and downloads a speech
        model.
      </p>
    </StepShell>
  );
}
