import type { ReactNode } from "react";
import { CheckIcon } from "@/components/icons";

interface StepShellProps {
  /** Decorative icon shown in the header badge. */
  icon: ReactNode;
  title: string;
  description: ReactNode;
  /** When true, the header badge renders a granted/done check. */
  done?: boolean;
  children?: ReactNode;
}

/**
 * Consistent vertical layout for a single onboarding step: a centered icon
 * badge, title, supporting copy, and a slot for the step's interactive body.
 */
export function StepShell({
  icon,
  title,
  description,
  done = false,
  children,
}: StepShellProps) {
  return (
    <div className="flex flex-col items-center text-center">
      <div
        className={`flex h-14 w-14 items-center justify-center rounded-2xl shadow-sm transition-colors ${
          done
            ? "bg-green-500/12 text-green-600"
            : "bg-blue-500/10 text-blue-600"
        }`}
      >
        <span className="h-7 w-7">{done ? <CheckIcon /> : icon}</span>
      </div>
      <h2 className="mt-4 text-xl font-semibold tracking-tight">{title}</h2>
      <div className="mt-2 max-w-sm text-sm leading-relaxed text-black/55">
        {description}
      </div>
      {children && <div className="mt-6 w-full max-w-sm">{children}</div>}
    </div>
  );
}

/** Inline "Granted" pill used to confirm a permission is satisfied. */
export function GrantedBadge({ label = "Granted" }: { label?: string }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full bg-green-500/12 px-3 py-1 text-sm font-medium text-green-700">
      <span className="h-4 w-4">
        <CheckIcon />
      </span>
      {label}
    </span>
  );
}
