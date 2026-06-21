import { Children, Fragment, isValidElement, type ReactNode } from "react";

interface SectionProps {
  title: string;
  description?: string;
  children: ReactNode;
}

/**
 * Flatten so conditional `<>...</>` groups contribute their rows individually
 * (each gets its own inset hairline divider), matching macOS grouped lists.
 */
function flattenRows(children: ReactNode): ReactNode[] {
  const out: ReactNode[] = [];
  for (const child of Children.toArray(children)) {
    if (isValidElement(child) && child.type === Fragment) {
      out.push(
        ...flattenRows(
          (child.props as { children?: ReactNode }).children ?? null,
        ),
      );
    } else {
      out.push(child);
    }
  }
  return out;
}

/**
 * A macOS "grouped" settings group: a small secondary group-header label above
 * an inset rounded-rectangle card whose rows are split by inset hairlines.
 */
export function Section({ title, description, children }: SectionProps) {
  const rows = flattenRows(children);
  return (
    <section>
      <h2 className="mac-group-header">{title}</h2>
      {description && <p className="mac-group-desc mb-1.5">{description}</p>}
      <div className="mac-card">
        {rows.map((child, i) => (
          <div key={i}>
            {i > 0 && <div className="mac-divider" />}
            {child}
          </div>
        ))}
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
      <div className={`px-3.5 py-2.5 ${disabled ? "opacity-50" : ""}`}>
        <div className="mb-2">
          <h3 className="text-label text-[13px]">{title}</h3>
          {description && (
            <p className="text-secondary mt-0.5 text-[11px] leading-snug">
              {description}
            </p>
          )}
        </div>
        <div className="w-full">{children}</div>
      </div>
    );
  }

  return (
    <div
      className={`flex items-center justify-between gap-4 px-3.5 py-2.5 ${
        disabled ? "opacity-50" : ""
      }`}
    >
      <div className="min-w-0">
        <h3 className="text-label text-[13px]">{title}</h3>
        {description && (
          <p className="text-secondary mt-0.5 text-[11px] leading-snug">
            {description}
          </p>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
