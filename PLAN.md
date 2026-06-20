# Stenographer — Build Plan

A lightweight, offline, macOS-only speech-to-text app. Press a key, speak, and your
words appear in the focused app. Built to be **near-zero footprint when idle**, and to
**work correctly under the Aerospace tiling window manager** — the two things existing
tools (Wispr Flow, SuperWhisper, Handy) get wrong for this user.

> Source of truth for reuse: Handy (`/tmp/Handy`, MIT, © 2025 CJ Pais). We **port**
> proven modules from it rather than rewriting, and drop everything non-macOS.

---

## 1. Product Requirements (derived from Handy analysis)

### Must have (v1)
- **Trigger** — unified Fn (Globe) key state machine:
  - **Hold Fn** (≥250 ms) → push-to-talk: record while held, transcribe + paste on release.
  - **Fn + Space** → start a hands-free toggle session.
  - **Tap Fn** (<250 ms) while a session is active → stop + transcribe. Tap with nothing active → no-op.
  - Detect if macOS "Press 🌐 to…" is not "Do Nothing" and nudge the user to change it.
- **Audio** — cpal capture at device-native rate → resample to 16 kHz mono → Silero VAD
  silence filtering with onset/hangover smoothing.
- **Transcription** — reuse `transcribe-rs`; support the **same models Handy uses**
  (Whisper small/medium/turbo/large, Parakeet v3 default), user-selectable. Metal accel on macOS.
- **Model management** — download (resume + SHA256 verify + tar.gz extract), select, delete,
  progress UI. Reuse Handy's catalog URLs + hashes (`blob.handy.computer`).
- **Text injection** — clipboard save → write → Cmd+V (raw keycode) → restore; configurable
  paste delay; optional auto-submit (Enter).
- **Overlay** — small non-activating NSPanel pill, fixed **bottom-center**, shown only while
  recording/transcribing. **Must appear on the user's CURRENT Aerospace workspace, never
  switch spaces, never steal focus.**
- **Menu bar app** — Accessory/`LSUIElement` (no Dock icon), state-aware icon, menu
  (settings, model switch, cancel, quit).
- **Settings window** — opened on demand; model browse/download/select, mic/output device,
  trigger prefs, paste prefs, overlay toggle, model-unload-timeout.
- **Lightweight idle** — Accessory mode, model unloaded on idle, overlay hidden; measured.
- **Signing** — free Apple Development cert (Personal Team, $0) → stable identity so
  Mic/Accessibility grants persist across rebuilds. No paid program.

### Out of scope for v1 (defer / drop)
Windows + Linux • i18n/translations • LLM post-processing & providers • Apple Intelligence bridge •
custom-word fuzzy dictionary • OpenCC Chinese conversion • clamshell mic • portable mode •
multiple paste backends / external scripts / Linux typing tools • auto-updater (optional, Phase 4) •
SQLite history (optional, Phase 4).

### Tech stack (recommended — confirm before kickoff)
**Tauri 2 + React/Vite/Tailwind/Zustand frontend + Rust backend**, porting Handy modules into a
fresh macOS-only scaffold. Rationale: reuses `transcribe-rs` and Handy's proven audio/VAD/paste
code, keeps the lean ownership of a fresh repo, and parallelizes cleanly by module.
Alternative considered: fork-and-strip Handy (faster to first-run, messier to own). Native AppKit
rejected (loses transcribe-rs plumbing reuse, too much for 24 h).

---

## 2. How parallelization works

- **Phase 0 is the dependency root** — it produces the scaffold, the settings schema, the IPC
  binding pattern, and the **module interface contracts**. Nothing parallel starts until it lands.
- In parallel phases, **each agent owns a disjoint set of files/modules**. Shared contention
  points (`Cargo.toml`, `src-tauri/src/lib.rs`, `bindings.ts`) are owned by a single **integration
  step**, not edited by parallel agents.
- Recommended mechanic: parallel agents run in **git worktrees**; the integration task merges and
  wires `lib.rs` + `Cargo.toml`. Each parallel task below names its owned paths.

Legend: **[SEQ]** sequential / blocking · **[PAR]** parallelizable within the phase.

---

## Phase 0 — Decisions, scaffold & contracts  **[SEQ]**

Goal: a launchable empty Tauri app on macOS with the free signing identity, settings persistence,
typed IPC, and written interface contracts the parallel phase will build against.

Tasks (mostly one agent, sequential):
1. Confirm stack (above). Create Tauri 2 scaffold: identifier `dev.brijesh.stenographer`,
   `productName: "Stenographer"`, programmatic window creation in `.setup` (not in conf).
2. `tauri.conf.json`: `macOSPrivateApi: true`, asset protocol, `bundle.macOS.hardenedRuntime: true`,
   `minimumSystemVersion` (e.g. "13.0"), `Entitlements.plist` (microphone + audio-input),
   `Info.plist` (`NSMicrophoneUsageDescription`). Signing: ad-hoc `"-"` initially.
3. **Free signing identity**: create an "Apple Development" cert via Xcode Personal Team (any
   Apple ID, $0); document how to point `signingIdentity` at it for stable TCC grants.
4. Frontend scaffold: React 18 + Vite + Tailwind 4 + Zustand + immer. Set up `tauri-specta`
   → auto-generate `src/bindings.ts`.
5. `AppSettings` Rust struct (`#[serde(default)]` per field) + `tauri-plugin-store`
   (`settings_store.json`) + `get_app_settings`/`get_default_settings`/per-setting setter pattern.
6. Register plugin set: log, store, fs, os, process, dialog, opener, clipboard-manager,
   global-shortcut, autostart, single-instance, macos-permissions, tauri-nspanel.
7. **Write `CONTRACTS.md`**: the interfaces each Phase-1 module exposes (see §"Interface contracts").

Acceptance criteria:
- `bun tauri dev` launches an empty Stenographer window on macOS 27.
- `bindings.ts` is generated from a sample command.
- A release build (`bun tauri build`) signed with the free dev cert installs and runs **with no
  paid Apple account**; Mic + Accessibility prompts appear, and **persist across a rebuild**
  (proves stable identity).
- `CONTRACTS.md` committed; settings round-trip through the store.

---

## Phase 1 — Core subsystems  **[PAR]** (up to 7 agents)

Goal: build each subsystem as a self-contained module against the Phase-0 contracts. Each is
independently testable via a small unit test or a temporary debug command.

- **1a. Audio capture + VAD** — owns `src-tauri/src/audio_toolkit/**`, `managers/audio.rs`.
  Port from Handy: cpal worker thread, F32/I16/I32 → f32, channel downmix, rubato 16 kHz resample
  + 30 ms framing, Silero VAD (`silero_vad_v4.onnx`) + onset/hangover smoothing, name-based device
  selection, short-clip padding. Expose `start()` / `stop() -> Vec<f32>` (16 kHz mono, filtered).
  *Accept:* a debug command records 3 s and returns a non-empty filtered buffer; silence is dropped.

- **1b. Transcription + model manager** — owns `managers/model.rs`, `managers/transcription.rs`,
  `commands/models.rs`. Integrate `transcribe-rs` (`whisper-metal` + `onnx` features). Port model
  catalog (reuse Handy URLs + SHA256), streaming download w/ HTTP-Range resume, SHA256 verify,
  tar.gz extract, atomic rename, cancel; engine load/unload + idle-eviction watcher + `catch_unwind`.
  *Accept:* download Parakeet v3, load it, transcribe a bundled WAV to correct text; unload-on-idle works.

- **1c. Text injection** — owns `clipboard.rs`, `input.rs`. Port macOS path only: clipboard
  save/write/sleep(delay)/Cmd+V via raw keycode/restore; auto-submit Enter/Cmd+Enter; run on main thread.
  *Accept:* a debug command pastes "hello world" into a focused TextEdit; original clipboard restored.

- **1d. Trigger: Fn state machine + coordinator** — owns `shortcut/**`, `transcription_coordinator.rs`.
  **New work.** Always-on Fn listener via `handy-keys`/`rdev` CGEventTap (captures `Modifiers::FN`).
  Record `Instant` on Fn-down; classify hold (≥250 ms) vs tap on Fn-up; handle Fn+Space toggle-start,
  tap-stop. Single-thread coordinator actor (`Idle → Recording → Processing → Idle`) with debounce +
  Drop-guard. Emits abstract record-start/stop/cancel events (no direct UI calls yet).
  *Accept:* unit/log test prints `HOLD-START/HOLD-STOP`, `TOGGLE-START`, `TOGGLE-STOP` for the right gestures; threshold configurable.

- **1e. Overlay (Aerospace-correct)** — owns `overlay.rs`, `src/overlay/**`. Non-activating NSPanel
  pill, `can_become_key_window:false`, `is_floating_panel:true`, `PanelLevel::Status`, bottom-center
  on the cursor's monitor, transparent, React waveform content. **Built from the start with the
  Aerospace fix** (see Phase 3 for the hardening/criteria). Driven by a temporary debug command for now.
  *Accept:* debug command shows/hides the pill; it never takes focus; appears bottom-center.

- **1f. Menu bar / tray** — owns `tray.rs`. Accessory app, theme-aware state icons
  (idle/recording/transcribing), menu (Settings, model submenu, Cancel, Quit). No Dock icon.
  *Accept:* menu bar icon present, no Dock icon; menu opens; icon swaps on a debug state change.

- **1g. Settings + model-download UI** — owns `src/components/**`, `src/stores/**`, `src/hooks/**`.
  Sidebar + sections (General, Models, Advanced), reusable controls, Zustand store w/ optimistic
  update + rollback, model list/`ModelCard` with progress from events. Mock backend where needed.
  *Accept:* settings window renders, toggles persist via setter commands, model list shows catalog + progress states.

Phase acceptance: every subsystem passes its own accept check in isolation; `CONTRACTS.md` honored.

---

## Phase 2 — Integration: the pipeline  **[SEQ]** (1 integration agent, small)

Goal: wire the modules into the end-to-end flow via `lib.rs` + the coordinator.

Tasks:
1. Manage `Arc<>` state (Audio, Model, Transcription) + coordinator + tray as Tauri state.
2. Connect coordinator events → pipeline: model+VAD parallel preload → record (overlay show, tray
   Recording, start sound) → stop → concurrent WAV-save + transcribe → paste on main thread →
   overlay hide, tray Idle. Drop-guard resets state on panic.
3. Lazy init: `initialize_enigo` / `initialize_shortcuts` only after permission grant (no premature dialogs).
4. Merge Phase-1 worktrees; reconcile `Cargo.toml` + `lib.rs` + `bindings.ts`.

Acceptance criteria:
- **End-to-end on macOS:** hold Fn → speak → release → correct text appears in the focused app.
- **Fn+Space** starts a hands-free session; **tap Fn** stops it and pastes.
- Tray reflects idle/recording/transcribing; start/stop sounds play; cancel works.
- No premature permission dialogs at launch.

---

## Phase 3 — Aerospace correctness + resource optimization  **[PAR]** (2 agents)

Goal: deliver the two differentiators and prove them with measurements.

- **3a. Aerospace overlay correctness** (owns `overlay.rs` + a test checklist). Implement and verify:
  - Re-home the panel to the **active Space at show time** (don't bind to launch workspace).
  - Use `orderFrontRegardless:` (never `makeKeyAndOrderFront:`); keep panel non-key.
  - Collection behavior `canJoinAllSpaces | fullScreenAuxiliary | transient | stationary`.
  - **Never** call `set_activation_policy(Regular)` / `set_focus` / `app.activate` on the record path.
  - Detect "Press 🌐 to…" ≠ "Do Nothing" → one-time nudge.
  - *Accept (the headline test):* launch in Aerospace workspace 2 → switch to workspace 5 → trigger
    record. **Overlay appears in workspace 5, focus stays in 5, NO space switch back to 2.** Repeat
    across several workspaces. (This is the exact bug the user hits with Handy.)

- **3b. Resource footprint** (owns profiling + `model_unload_timeout` wiring + docs). Confirm
  Accessory mode (no Dock), overlay/webview disposed or hidden when idle, model unloaded on idle.
  Measure and record **idle vs active** RAM/CPU; tune idle eviction.
  - *Accept:* idle RAM in the low tens of MB with model unloaded and ~0% CPU; active spikes only
    during transcription; numbers written to `PERF.md`.

---

## Phase 4 — Polish & ship  **[PAR → SEQ]** (2–3 agents, then 1)

- **[PAR] 4a. Onboarding & permissions** — accessibility + mic onboarding gate, first-run model pick.
- **[PAR] 4b. Autostart** (LaunchAgent) + start-hidden + "launch at login" toggle.
- **[PAR] 4c. (Optional) SQLite history** + retry, and/or **auto-updater** (minisign + `latest.json`).
- **[SEQ] 4d. Final build & install** — produce signed `.app`/DMG with the free dev cert; install;
  run the full acceptance suite below.

Final acceptance (the v1 definition of done):
- Builds and runs from a **free Apple account**; permissions persist across rebuilds.
- Hold-Fn PTT + Fn+Space toggle + tap-stop all work; text lands in the focused app.
- Overlay shows on the **current Aerospace workspace** with no space switching, on any workspace.
- Idle footprint is minimal (per `PERF.md`); model loads on demand, unloads on idle.
- User can download/select/delete models and change prefs in the settings window.

---

## Interface contracts (to finalize in Phase 0 `CONTRACTS.md`)

- **Audio:** `AudioRecordingManager::start(binding_id)`, `stop() -> Vec<f32>` (16 kHz mono, VAD-filtered),
  `is_recording()`, `preload_vad()`.
- **Transcription:** `TranscriptionManager::initiate_model_load()`, `transcribe(Vec<f32>) -> String`,
  unload-timeout control. `ModelManager`: catalog, `download/cancel/delete`, `set_active`, status.
- **Trigger → Coordinator:** coordinator receives classified inputs
  `{Hold(start/stop), ToggleStart, ToggleStop, Cancel}` and drives the pipeline; emits no UI directly.
- **Overlay:** `show_recording()`, `show_transcribing()`, `hide()` — all non-activating, current-Space.
- **Tray:** `set_state(Idle|Recording|Transcribing)`, menu callbacks.
- **IPC/events:** `mic-level`, `show-overlay`/`hide-overlay`, `recording-error`,
  `model-download-progress` + lifecycle, settings getters/setters.

## Key risks
1. **transcribe-rs + Metal build** on macOS 27 — validate early (Phase 1b spike) before depending on it.
2. **Aerospace space-switch** — root cause is the panel bound to its launch Space + an order-front
   that raises the app; Phase 3a is dedicated to it and gated by the headline test.
3. **Forked deps** — Handy pins forks of tauri-runtime/rdev/rodio/vad-rs. Try upstream first;
   fall back to the same forks if the nspanel/event behavior needs them.
4. **Fn capture vs OS Globe behavior** — needs Accessibility (CGEventTap) + user setting "Do Nothing".
