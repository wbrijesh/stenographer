interface ProgressBarProps {
  /** 0-100 */
  percentage: number;
  className?: string;
}

export function ProgressBar({ percentage, className = "" }: ProgressBarProps) {
  const clamped = Math.max(0, Math.min(100, percentage));
  return (
    <div
      className={`h-1.5 w-full overflow-hidden rounded-full ${className}`}
      style={{ background: "var(--fill-strong)" }}
    >
      <div
        className="h-full rounded-full transition-[width] duration-150 ease-out"
        style={{ width: `${clamped}%`, background: "var(--accent)" }}
      />
    </div>
  );
}
