export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface SelectProps {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  "aria-label"?: string;
}

/** Native-styled select with a custom chevron. */
export function Select({
  value,
  options,
  onChange,
  disabled = false,
  ...rest
}: SelectProps) {
  return (
    <div className="relative inline-block">
      <select
        value={value}
        disabled={disabled}
        aria-label={rest["aria-label"]}
        onChange={(e) => onChange(e.target.value)}
        className="mac-select"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value} disabled={opt.disabled}>
            {opt.label}
          </option>
        ))}
      </select>
      <svg
        className="text-secondary pointer-events-none absolute right-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2"
        viewBox="0 0 20 20"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.75}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="m6 8.5 4 4 4-4"
        />
      </svg>
    </div>
  );
}
