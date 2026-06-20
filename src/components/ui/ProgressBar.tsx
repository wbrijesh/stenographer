interface ProgressBarProps {
  /** 0-100 */
  percentage: number;
  className?: string;
}

export function ProgressBar({ percentage, className = "" }: ProgressBarProps) {
  const clamped = Math.max(0, Math.min(100, percentage));
  return (
    <div
      className={`h-1.5 w-full overflow-hidden rounded-full bg-black/10 ${className}`}
    >
      <div
        className="h-full rounded-full bg-blue-500 transition-[width] duration-150 ease-out"
        style={{ width: `${clamped}%` }}
      />
    </div>
  );
}
