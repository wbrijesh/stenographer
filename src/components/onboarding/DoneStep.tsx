import { Button } from "@/components/ui";
import { SparkleIcon } from "@/components/icons";
import { StepShell } from "./StepShell";

interface DoneStepProps {
  /** Dismiss onboarding and reveal the main settings window. */
  onFinish: () => void;
}

/** Final step — confirmation and dismissal to the main settings UI. */
export function DoneStep({ onFinish }: DoneStepProps) {
  return (
    <StepShell
      icon={<SparkleIcon />}
      title="You're all set"
      description="Hold Fn to dictate, or press Fn + Space to toggle hands-free. Your transcribed text is typed wherever your cursor is."
      done
    >
      <Button variant="primary" className="w-full" onClick={onFinish}>
        Start using Stenographer
      </Button>
    </StepShell>
  );
}
