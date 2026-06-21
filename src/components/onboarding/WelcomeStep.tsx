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
      <div className="mac-card space-y-2 p-4 text-left">
        <div className="flex items-center justify-between gap-3 text-[13px]">
          <span className="text-secondary">Hold</span>
          <kbd className="mac-kbd px-2 py-0.5 text-[12px]">Fn</kbd>
        </div>
        <div className="mac-divider !ml-0" />
        <div className="flex items-center justify-between gap-3 text-[13px]">
          <span className="text-secondary">Toggle hands-free</span>
          <span className="flex items-center gap-1">
            <kbd className="mac-kbd px-2 py-0.5 text-[12px]">Fn</kbd>
            <span className="text-tertiary">+</span>
            <kbd className="mac-kbd px-2 py-0.5 text-[12px]">Space</kbd>
          </span>
        </div>
      </div>
      <p className="text-tertiary mt-4 text-[11px]">
        A quick setup grants the permissions it needs and downloads a speech
        model.
      </p>
    </StepShell>
  );
}
