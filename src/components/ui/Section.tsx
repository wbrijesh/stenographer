import type { ReactNode } from "react";

interface SectionProps {
  title: string;
  description?: string;
  children: ReactNode;
}

/** A titled group of settings rows, separated by hairlines. */
export function Section({ title, description, children }: SectionProps) {
  return (
    <section className="space-y-2">
      <div className="px-1">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-black/45">
          {title}
        </h2>
        {description && (
          <p className="mt-0.5 text-xs text-black/45">{description}</p>
        )}
      </div>
      <div className="overflow-hidden rounded-xl border border-black/10 bg-white/70 shadow-sm">
        <div className="divide-y divide-black/[0.07]">{children}</div>
      </div>
    </section>
  );
}

interface RowProps {
  title: string;
  description?: string;
  /** When true, stacks the control below the label (used by sliders). */
  stacked?: boolean;
  disabled?: boolean;
  children: ReactNode;
}

/** A single labelled settings row. */
export function Row({
  title,
  description,
  stacked = false,
  disabled = false,
  children,
}: RowProps) {
  if (stacked) {
    return (
      <div className={`px-4 py-3 ${disabled ? "opacity-50" : ""}`}>
        <div className="mb-2">
          <h3 className="text-sm font-medium">{title}</h3>
          {description && (
            <p className="mt-0.5 text-xs text-black/50">{description}</p>
          )}
        </div>
        <div className="w-full">{children}</div>
      </div>
    );
  }

  return (
    <div
      className={`flex items-center justify-between gap-4 px-4 py-3 ${
        disabled ? "opacity-50" : ""
      }`}
    >
      <div className="min-w-0">
        <h3 className="text-sm font-medium">{title}</h3>
        {description && (
          <p className="mt-0.5 text-xs leading-snug text-black/50">
            {description}
          </p>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
