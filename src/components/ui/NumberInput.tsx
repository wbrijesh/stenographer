import { useEffect, useState } from "react";

interface NumberInputProps {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  suffix?: string;
  "aria-label"?: string;
}

/**
 * Numeric input that commits on blur / Enter (so partial edits don't spam the
 * backend) and clamps to [min, max].
 */
export function NumberInput({
  value,
  onChange,
  min,
  max,
  step = 1,
  disabled = false,
  suffix,
  ...rest
}: NumberInputProps) {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  const commit = () => {
    let next = Number(draft);
    if (Number.isNaN(next)) {
      setDraft(String(value));
      return;
    }
    if (min !== undefined) next = Math.max(min, next);
    if (max !== undefined) next = Math.min(max, next);
    setDraft(String(next));
    if (next !== value) onChange(next);
  };

  return (
    <div className="inline-flex items-center gap-2">
      <input
        type="number"
        value={draft}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        aria-label={rest["aria-label"]}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.currentTarget.blur();
          }
        }}
        className="w-24 rounded-lg border border-black/15 bg-white px-3 py-1.5 text-sm tabular-nums shadow-sm transition-colors hover:border-black/30 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
      />
      {suffix && <span className="text-xs text-black/50">{suffix}</span>}
    </div>
  );
}
