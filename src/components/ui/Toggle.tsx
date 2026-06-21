interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  "aria-label"?: string;
}

/** macOS-style switch (pill track, white knob, green when on). */
export function Toggle({
  checked,
  onChange,
  disabled = false,
  ...rest
}: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={rest["aria-label"]}
      disabled={disabled}
      data-on={checked}
      onClick={() => onChange(!checked)}
      className="mac-switch"
    >
      <span className="mac-switch-knob" />
    </button>
  );
}
