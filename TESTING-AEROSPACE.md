# TESTING-AEROSPACE.md — The Headline Manual Test

This is the **headline differentiator test** for Stenographer: prove that the
recording overlay behaves correctly under the [Aerospace](https://github.com/nikitabobko/AeroSpace)
tiling window manager. It **cannot be reproduced in CI** — it requires real
hardware running Aerospace with multiple workspaces.

The exact bug we are guarding against (PLAN.md risk #2): a panel that is bound
to its *launch* Space plus an order-front that *raises the app*. The symptom is:
you trigger a recording while working on workspace 5, and macOS yanks you back to
workspace 2 (where Stenographer launched), or the overlay steals keyboard focus
from your editor.

---

## 0. Prerequisites

- macOS 13+ with **Aerospace** installed and running.
- A **debug or release build** of Stenographer (`cd src-tauri && cargo build`,
  then run the produced binary, or `bun tauri build` and launch the `.app`).
  Do **not** run `tauri dev` for this test — use a real launched app.
- At least **3 Aerospace workspaces** you can switch between (default
  `alt-1` … `alt-5` bindings). Have a focusable text app (TextEdit, your editor)
  open in each test workspace so you can observe focus.
- Accessibility + Microphone permissions granted to Stenographer (so the Fn
  trigger and recording actually fire).
- **"Press 🌐 to…" set to "Do Nothing"** (System Settings → Keyboard). See §3.

> Tip: keep Aerospace's workspace indicator visible (menu bar / sketchybar) so a
> Space switch is immediately obvious.

---

## 1. The headline test — overlay stays on the active workspace

Run this for **several** workspace pairs, not just one.

1. Launch Stenographer **while focused on Aerospace workspace 2**.
   - It is an Accessory app (no Dock icon); confirm it started (menu-bar icon).
2. Switch to **workspace 5** (`alt-5`). Click into the text app there so a
   **caret is blinking** in workspace 5 — this is your focus witness.
3. **Trigger a recording**: hold the **Fn (🌐)** key (push-to-talk) — or
   `Fn+Space` for a toggle session — and speak a word or two.
4. **Observe while recording:**
   - [ ] The overlay pill appears **bottom-center of the monitor under the
         cursor**, on **workspace 5**.
   - [ ] The view does **NOT** switch back to workspace 2 (no Space flip).
   - [ ] The blinking caret **stays in the workspace-5 text app** — focus was
         not stolen. Keep typing/observe: keystrokes still go to that app.
5. **Release Fn** (or tap Fn to stop the toggle). The text is transcribed and
   pasted into the **workspace-5** app. The overlay fades out.
   - [ ] Still **no** Space switch on hide. Still on workspace 5.

### Repeat matrix

Repeat steps 1–5 launching on one workspace and recording on another. Suggested
runs (launch → record):

| Launch WS | Record WS | Space stayed put? | Focus kept? | Overlay on record WS? |
|-----------|-----------|-------------------|-------------|-----------------------|
| 2         | 5         |                   |             |                       |
| 2         | 1         |                   |             |                       |
| 5         | 3         |                   |             |                       |
| 1         | 4         |                   |             |                       |
| 3         | 3 (same)  |                   |             |                       |

All rows must be PASS. A single Space-switch or focus-steal is a **FAIL**.

---

## 2. Verify the panel never steals focus (independent checks)

Beyond "the caret stayed put", confirm the non-activating contract directly:

1. **Keystroke test:** start typing in the workspace-5 app, trigger a recording
   *mid-sentence*, and keep typing. Every keystroke must still land in that app —
   the overlay must never receive keyboard input.
   - [ ] PASS
2. **No app activation:** while the overlay is up, Stenographer must **not**
   appear as the frontmost/active app and **no Dock icon** should appear.
   (Optional, from a terminal while recording:)
   ```sh
   osascript -e 'tell application "System Events" to get name of first process whose frontmost is true'
   ```
   - [ ] It reports your editor / the workspace-5 app — **not** "Stenographer".
3. **Multi-monitor (if available):** move the cursor to a second display, then
   trigger. The overlay should appear **bottom-center of the display under the
   cursor**, still without a Space switch or focus change.
   - [ ] PASS

---

## 3. "Press 🌐 to…" (Globe/Fn) setting check

macOS intercepts the Fn (🌐) key before our event tap unless this is set to
"Do Nothing". Stenographer detects this and can nudge the user.

1. Open **System Settings → Keyboard → "Press 🌐 to…"**. Confirm it is set to
   **"Do Nothing"**. If it is anything else (Change Input Source, Show Emoji &
   Symbols, Start Dictation, …) Fn push-to-talk will misbehave.
2. **Verify the in-app detection** matches the OS:
   - The frontend calls the `check_fn_key_behavior` command
     (`commands.checkFnKeyBehavior()` in `src/bindings.ts`). It returns
     `{ ok: boolean, current: number }`.
   - `ok: true` ⇒ safe (setting is "Do Nothing", `current == 0`, **or** the key
     could not be read, `current == -1` — we fail open and do not nag).
   - `ok: false` ⇒ show the one-time nudge to change the setting.
3. **Cross-check against `defaults`** (the source the command reads):
   ```sh
   defaults read com.apple.HIToolbox AppleFnUsageType
   ```
   - `0` ⇒ "Do Nothing" ⇒ command returns `ok: true`.
   - non-zero (e.g. `3`) ⇒ command returns `ok: false` ⇒ nudge should show.
   - key absent (`defaults` errors) ⇒ command returns `ok: true, current: -1`
     (no nudge).
   - [ ] The command's `ok`/`current` agree with the OS setting + `defaults`.

---

## 4. PASS criteria mapped to the hard requirements

The headline test passes only if **all** of these hold (each maps to an
overlay.rs Aerospace contract requirement):

- [ ] **R1 — Non-activating panel.** Overlay never becomes key / never receives
      keystrokes (built with `can_become_key_window: false`,
      `is_floating_panel: true`, `PanelLevel::Status`). Verified in §1.4 / §2.1.
- [ ] **R2 — Collection behavior re-applied on every show.** Overlay appears on
      whatever workspace is active *at record time*, run after run, even after
      switching Spaces (`canJoinAllSpaces | fullScreenAuxiliary | transient |
      stationary | moveToActiveSpace`). Verified by the §1 repeat matrix.
- [ ] **R3 — Re-home + reposition + order-front on every show.** Overlay shows
      bottom-center on the cursor's monitor, on the active Space, surfaced via
      `orderFrontRegardless:`. Verified in §1.4 and §2.3.
- [ ] **R4 — No forbidden activation on the record path.** No app activation, no
      Dock icon, no focus steal, no Space switch (the record path never calls
      `set_activation_policy(Regular)`, `app.activate()`, `set_focus()`,
      `make_key_and_order_front`, `show_and_make_key`). Verified in §1.4–§1.5
      and §2.2.
- [ ] **R5 — No Space switch, ever.** Across the whole §1 matrix, the active
      workspace never changed when the overlay showed or hid.
- [ ] **Globe setting.** "Press 🌐 to…" is "Do Nothing" and
      `check_fn_key_behavior` agrees with the OS. Verified in §3.

If every box is checked across multiple workspace pairs, the Aerospace headline
differentiator is **proven on this hardware**.
