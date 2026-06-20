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
        className="min-w-[10rem] cursor-pointer appearance-none rounded-lg border border-black/15 bg-white py-1.5 pl-3 pr-8 text-sm text-black shadow-sm transition-colors hover:border-black/30 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value} disabled={opt.disabled}>
            {opt.label}
          </option>
        ))}
      </select>
      <svg
        className="pointer-events-none absolute right-2 top-1/2 h-4 w-4 -translate-y-1/2 text-black/40"
        viewBox="0 0 20 20"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="m6 8 4 4 4-4"
        />
      </svg>
    </div>
  );
}
