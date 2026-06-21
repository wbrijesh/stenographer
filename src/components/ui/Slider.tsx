interface SliderProps {
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
  showValue?: boolean;
  formatValue?: (value: number) => string;
  "aria-label"?: string;
}

export function Slider({
  value,
  onChange,
  min,
  max,
  step = 1,
  disabled = false,
  showValue = true,
  formatValue = (v) => String(v),
  ...rest
}: SliderProps) {
  const pct = max > min ? ((value - min) / (max - min)) * 100 : 0;

  return (
    <div className="flex w-full items-center gap-3">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        aria-label={rest["aria-label"]}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="mac-slider flex-grow"
        style={{
          background: `linear-gradient(to right, var(--accent) ${pct}%, var(--fill-strong) ${pct}%)`,
        }}
      />
      {showValue && (
        <span className="text-secondary w-12 text-end text-[12px] font-medium tabular-nums">
          {formatValue(value)}
        </span>
      )}
    </div>
  );
}
