/**
 * Helpers for displaying handy-keys trigger binding strings (e.g. `"CmdRight"`,
 * `"Ctrl+Shift+R"`) as friendly, macOS-native labels.
 */

/** The hard-coded default trigger binding (Right ⌘). */
export const DEFAULT_TRIGGER_BINDING = "CmdRight";

/** Map of a single handy-keys part → its friendly label. */
const PART_LABELS: Record<string, string> = {
  CmdRight: "Right ⌘",
  CmdLeft: "Left ⌘",
  Cmd: "⌘",
  OptRight: "Right ⌥",
  OptLeft: "Left ⌥",
  Opt: "⌥",
  CtrlRight: "Right ⌃",
  CtrlLeft: "Left ⌃",
  Ctrl: "⌃",
  ShiftRight: "Right ⇧",
  ShiftLeft: "Left ⇧",
  Shift: "⇧",
  Fn: "🌐 Fn",
  Space: "Space",
  Return: "⏎",
};

/** Friendly label for one handy-keys part; unknown parts are shown as-is. */
function labelForPart(part: string): string {
  return PART_LABELS[part] ?? part;
}

/**
 * Turn a handy-keys binding string into a friendly label.
 *
 * Combos joined with `+` (e.g. `"Ctrl+Shift+R"`) have each part mapped and are
 * re-joined with `" + "`. Empty / falsy input yields an em dash.
 */
export function formatBinding(binding: string): string {
  if (!binding) return "—";
  return binding
    .split("+")
    .map((part) => labelForPart(part.trim()))
    .join(" + ");
}
