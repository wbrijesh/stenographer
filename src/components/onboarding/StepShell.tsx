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
        className="flex h-14 w-14 items-center justify-center rounded-[14px] transition-colors"
        style={{
          background: done
            ? "color-mix(in srgb, var(--green) 14%, transparent)"
            : "color-mix(in srgb, var(--accent) 12%, transparent)",
          color: done ? "var(--green)" : "var(--accent)",
        }}
      >
        <span className="h-7 w-7">{done ? <CheckIcon /> : icon}</span>
      </div>
      <h2 className="text-label mt-4 text-[20px] font-semibold tracking-tight">
        {title}
      </h2>
      <div className="text-secondary mt-2 max-w-sm text-[13px] leading-relaxed">
        {description}
      </div>
      {children && <div className="mt-6 w-full max-w-sm">{children}</div>}
    </div>
  );
}

/** Inline "Granted" pill used to confirm a permission is satisfied. */
export function GrantedBadge({ label = "Granted" }: { label?: string }) {
  return (
    <span
      className="text-green inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-[13px] font-medium"
      style={{ background: "color-mix(in srgb, var(--green) 14%, transparent)" }}
    >
      <span className="h-4 w-4">
        <CheckIcon />
      </span>
      {label}
    </span>
  );
}
