interface ScoreBarProps {
  label: string;
  /** 1-5 */
  score: number;
}

/** Five-pip rating used for accuracy / speed. */
export function ScoreBar({ label, score }: ScoreBarProps) {
  return (
    <div className="flex items-center gap-1.5">
      <span className="w-14 text-xs text-black/45">{label}</span>
      <div className="flex gap-0.5">
        {Array.from({ length: 5 }).map((_, i) => (
          <span
            key={i}
            className={`h-1.5 w-3 rounded-full ${
              i < score ? "bg-blue-500" : "bg-black/10"
            }`}
          />
        ))}
      </div>
    </div>
  );
}
