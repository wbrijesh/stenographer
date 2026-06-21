import { useCallback, useState } from "react";
import { Button } from "@/components/ui";
import { useOnboarding } from "@/hooks/useOnboarding";
import { WelcomeStep } from "./WelcomeStep";
import { MicrophoneStep } from "./MicrophoneStep";
import { AccessibilityStep } from "./AccessibilityStep";
import { ShortcutStep } from "./ShortcutStep";
import { ModelStep } from "./ModelStep";
import { DoneStep } from "./DoneStep";

interface OnboardingProps {
  /**
   * Called once the user finishes the flow. Wired to the gate's `recheck` so the
   * app re-evaluates prerequisites and swaps in the main settings UI.
   */
  onComplete: () => void;
}

type StepId =
  | "welcome"
  | "microphone"
  | "accessibility"
  | "shortcut"
  | "model"
  | "done";

const ORDER: StepId[] = [
  "welcome",
  "microphone",
  "accessibility",
  "shortcut",
  "model",
  "done",
];

/**
 * First-run gate container. Walks the user through the multi-step flow with
 * next/back controls, and tracks per-step completion off the live prerequisite
 * status from {@link useOnboarding}. Steps already satisfied (e.g. a permission
 * granted in a previous run) render a ✓ and can be skipped past.
 */
export function Onboarding({ onComplete }: OnboardingProps) {
  // A local instance of the gate hook so steps can drive live status here and
  // gate the "Next" buttons on real grants. The parent's `onComplete` still
  // owns the final transition out of onboarding.
  const { status, recheck } = useOnboarding();
  const [index, setIndex] = useState(0);

  const step = ORDER[index];
  const isFirst = index === 0;

  const goNext = useCallback(() => {
    setIndex((i) => Math.min(i + 1, ORDER.length - 1));
  }, []);

  const goBack = useCallback(() => {
    setIndex((i) => Math.max(i - 1, 0));
  }, []);

  // After a step makes progress, re-check live status; advance automatically so
  // a freshly granted permission doesn't strand the user on a done step.
  const onStepProgress = useCallback(async () => {
    await recheck();
    goNext();
  }, [recheck, goNext]);

  // Whether the current step has its prerequisite satisfied (controls "Next").
  const stepSatisfied = (id: StepId): boolean => {
    switch (id) {
      case "welcome":
      case "shortcut":
      case "done":
        return true;
      case "microphone":
        return status.microphone;
      case "accessibility":
        return status.accessibility;
      case "model":
        return status.model;
    }
  };

  const renderStep = () => {
    switch (step) {
      case "welcome":
        return <WelcomeStep />;
      case "microphone":
        return (
          <MicrophoneStep
            granted={status.microphone}
            onGranted={() => void onStepProgress()}
          />
        );
      case "accessibility":
        return (
          <AccessibilityStep
            granted={status.accessibility}
            onGranted={() => void onStepProgress()}
          />
        );
      case "shortcut":
        return <ShortcutStep onContinue={goNext} />;
      case "model":
        return <ModelStep onReady={() => void onStepProgress()} />;
      case "done":
        return <DoneStep onFinish={onComplete} />;
    }
  };

  return (
    <div className="flex h-screen w-screen flex-col text-label">
      {/* Progress dots (clears the titlebar region) */}
      <div className="flex shrink-0 items-center justify-center gap-1.5 pt-9">
        {ORDER.map((id, i) => (
          <span
            key={id}
            className="h-1.5 rounded-full transition-all"
            style={{
              width: i === index ? 24 : 6,
              background:
                i === index
                  ? "var(--accent)"
                  : i < index
                    ? "color-mix(in srgb, var(--accent) 40%, transparent)"
                    : "var(--fill-strong)",
            }}
          />
        ))}
      </div>

      {/* Step body */}
      <div className="mac-scroll flex flex-1 items-center justify-center overflow-y-auto px-6">
        <div className="w-full max-w-md py-8">{renderStep()}</div>
      </div>

      {/* Footer nav (hidden on the final step — it has its own CTA) */}
      {step !== "done" && (
        <div
          className="flex shrink-0 items-center justify-between px-6 py-4"
          style={{ borderTop: "0.5px solid var(--separator)" }}
        >
          <Button
            variant="ghost"
            onClick={goBack}
            disabled={isFirst}
            className={isFirst ? "invisible" : ""}
          >
            Back
          </Button>
          <Button
            variant="primary"
            onClick={goNext}
            disabled={!stepSatisfied(step)}
          >
            {step === "welcome" ? "Get started" : "Next"}
          </Button>
        </div>
      )}
    </div>
  );
}
