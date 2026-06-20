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
        className="h-1.5 flex-grow cursor-pointer appearance-none rounded-full focus:outline-none disabled:cursor-not-allowed disabled:opacity-50 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:shadow [&::-webkit-slider-thumb]:ring-1 [&::-webkit-slider-thumb]:ring-black/10"
        style={{
          background: `linear-gradient(to right, #3b82f6 ${pct}%, rgba(0,0,0,0.12) ${pct}%)`,
        }}
      />
      {showValue && (
        <span className="w-14 text-end text-xs font-medium tabular-nums text-black/70">
          {formatValue(value)}
        </span>
      )}
    </div>
  );
}
