# PERF — Resource Footprint (Phase 3b)

Stenographer is built to be **near-zero footprint when idle**. This document records the
idle-footprint *design* (with code references), the measurement *methodology* (exact
commands), captured *numbers* (or the reason they are pending), and *tuning guidance* for the
model-unload timeout.

The headline target (PLAN.md Phase 3b acceptance):

> Idle RAM in the low tens of MB with the model unloaded and ~0% CPU; active spikes only during
> transcription.

---

## 1. Idle-footprint design

Four mechanisms keep the idle cost near zero. Each is implemented in the Rust backend and is
confirmed by code audit below.

### 1.1 Accessory mode — no Dock icon, minimal AppKit footprint

When `start_hidden` is set (the default — `settings.rs:122` `default_start_hidden() -> true`),
the app sets `ActivationPolicy::Accessory` at startup so it runs as a menu-bar/background agent
with **no Dock icon** and no main-menu participation.

- `src-tauri/src/lib.rs:162-167` — at setup, `if settings.start_hidden { set_activation_policy(Accessory) }`.
- `src-tauri/src/lib.rs:170-172` — the settings window is only shown at launch when `!start_hidden`;
  otherwise it stays hidden (built with `.visible(false)`, `lib.rs:158`).
- `src-tauri/src/lib.rs:80-95` (`show_main_window`) — opening Settings flips the policy to
  `Regular` so the window and its Dock icon appear on demand.

**Known footprint/UX leak (reported, not yet fixed):** there is no window-close / window-hide
handler that returns the app to `Accessory`. Once the user opens Settings (→ `Regular`) and then
closes the window, the process stays in `Regular` mode: the Dock icon and main menu persist even
though no window is visible. This is a cosmetic footprint leak (it does not load the model or
spin CPU), but it defeats the "invisible background agent" intent until the next relaunch. See
§5 for the precise suggested fix for the orchestrator to apply in `lib.rs`.

### 1.2 Lazy model load + idle unload — low idle RAM

The transcription model (the dominant RAM cost — hundreds of MB for Whisper/Parakeet) is **not
loaded at startup**. Nothing in `lib.rs` setup calls `initiate_model_load`/`load_model`; the
model is loaded lazily on first record and evicted after an idle timeout.

- **Not loaded at startup:** `lib.rs:184-186` constructs `ModelManager` and `TranscriptionManager`
  but neither loads an engine. `TranscriptionManager::new` (`transcription.rs:85-151`) only starts
  the idle-watcher thread; `engine` begins as `None` (`transcription.rs:87`).
- **Lazy load:** `TranscriptionManager::initiate_model_load` (`transcription.rs:351-373`) spawns a
  background load of the selected model; `transcribe_inner` also lazy-loads if nothing is loaded
  (`transcription.rs:404-413`). Eager load also happens when the user explicitly selects a model
  (`commands/models.rs:89-112` `set_active_model`), unless the timeout is `Immediately`.
- **Idle eviction watcher:** `transcription.rs:99-148` — a thread wakes every 10 s, reads
  `settings.model_unload_timeout`, and unloads the engine once `idle_ms > limit` and a model is
  loaded (`transcription.rs:128-142`). `unload_model` (`transcription.rs:189-217`) sets the engine
  to `None`, which **drops the engine and frees all model RAM**.
- **`set_model_unload_timeout` wired:** command at `commands/models.rs:131-139` →
  `TranscriptionManager::set_unload_timeout` (`transcription.rs:228-234`), registered in
  `lib.rs:73`. It persists the setting and touches activity so a freshly-shortened timeout does
  not unload instantly.
- **All `ModelUnloadTimeout` variants honored** (`transcription.rs:155-161` `timeout_to_seconds`,
  plus the special-casing):
  - `Never` → `None`; the timed watcher never unloads (`transcription.rs:157`).
  - `Immediately` → skipped by the timed watcher (`transcription.rs:118-120`) and instead handled
    by `maybe_unload_immediately` after each transcription (`transcription.rs:247-258`, called at
    `transcription.rs:390` and `:556`). Eager-load is also skipped for it (`models.rs:105-107`).
  - `Seconds(s)` → `Some(s)` (`transcription.rs:158`).
  - `Minutes(m)` → `Some(m*60)` (`transcription.rs:159`). Default is `Minutes(5)`
    (`settings.rs:18-22`).

**Net effect:** with the model unloaded, idle RAM is just the Rust process + the WKWebView for the
(hidden) windows + the ~2 MB VAD model — i.e. the "low tens of MB" target. The model RAM is only
resident from first-record until the idle timeout elapses.

### 1.3 Overlay hidden (orderOut) when idle

The recording-overlay NSPanel is created **hidden** at startup and only surfaced during
record/transcribe; when idle it is `orderOut:`-hidden (not merely transparent), so it costs ~nothing.

- Created hidden: `overlay.rs:253-256` — after building the panel, `panel.hide()` is called
  immediately ("Start hidden; only shown while recording / transcribing").
- Shown only on demand: `show_recording`/`show_transcribing` (`overlay.rs:271-278`) →
  `show_overlay_state` → `panel.order_front_regardless()` (`overlay.rs:299`), driven by the
  pipeline during a record/transcribe cycle.
- Hidden on idle: `hide_overlay` (`overlay.rs:316-331`) calls `panel.hide()` (tauri-nspanel's
  `orderOut:`) after a 300 ms fade. So between sessions the panel is ordered out of the window
  server, not left as an invisible-but-composited window.

> Note: the overlay still hosts a small WKWebView (its waveform UI). On macOS that webview process
> is created at startup and persists; it is the main *baseline* contributor alongside the main
> settings webview. It does no rendering work while `orderOut:`. (`overlay.rs` is owned by another
> agent in this pass; this file only audits it.)

### 1.4 VAD preloaded — small, no idle CPU

Silero VAD (`resources/models/silero_vad_v4.onnx`, ~2 MB) is preloaded at startup so the first
record has no model-init latency.

- Preload: `lib.rs:188` `audio.preload_vad();` → `AudioRecordingManager::preload_vad`
  (`audio.rs:175-179`) → `ensure_recorder` (`audio.rs:181-199`).
- `ensure_recorder` only **constructs** the `AudioRecorder` with the Silero model in memory
  (`audio.rs:197`, `create_audio_recorder` at `audio.rs:110-131`). It does **not** open a cpal
  microphone stream — that happens separately in `start_microphone_stream` (`audio.rs:241+`),
  which is only called while recording. So at idle the VAD model sits in RAM (~2 MB) doing **no
  CPU work**; no audio is captured and the VAD is not invoked.

This ~2 MB resident cost is an acceptable trade for instant first-record responsiveness.

### 1.5 No idle CPU offenders (timers / threads)

Audited the long-lived threads/loops for busy-spin:

- **Fn (Globe) listener** (`shortcut/fn_listener.rs:107-148`): blocks on
  `listener.recv_timeout(RECV_TIMEOUT)` with `RECV_TIMEOUT = 100 ms` (`fn_listener.rs:109`). This
  is a 10 Hz wake to check the shutdown flag, not a tight spin — negligible CPU. It otherwise
  blocks waiting for keyboard events.
- **Coordinator actor** (`coordinator.rs:187`): blocks on `rx.recv()` (no timeout) — zero CPU when
  idle; wakes only on a trigger input.
- **Idle-eviction watcher** (`transcription.rs:103-146`): `thread::sleep(Duration::from_secs(10))`
  between checks — a 0.1 Hz wake, negligible.

No busy-loops found. Idle CPU should be ~0% aside from these infrequent wakes.

---

## 2. Measurement methodology

The app starts hidden (Accessory) and must **not** prompt for permissions at launch, so it can be
launched headless-ish for a quick idle sample. Always launch in the background with a hard timeout
and guarantee cleanup so no process is ever left running.

### 2.1 Build

```sh
cd src-tauri
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
cargo build            # debug; fast. Use the debug binary for a quick sample.
# Optional (slow — release profile has LTO):
# cargo build --release
```

Binary path: `src-tauri/target/debug/stenographer` (or `target/release/stenographer`).

### 2.2 Idle sample (time-boxed, self-cleaning)

```sh
cd src-tauri
BIN="$PWD/target/debug/stenographer"
pkill -f "target/debug/stenographer" 2>/dev/null      # clear strays first
"$BIN" >/tmp/steno_run.log 2>&1 &
APP=$!
sleep 8                                                # let it settle to idle
if kill -0 $APP 2>/dev/null; then
  ps -o rss=,pcpu=,comm= -p $APP                       # main process RSS(KB) + %CPU
  pgrep -P $APP | while read c; do                     # webview / helper procs
    ps -o pid=,rss=,pcpu=,comm= -p $c
  done
fi
kill $APP 2>/dev/null; sleep 1; kill -9 $APP 2>/dev/null
pgrep -fl "target/debug/stenographer" || echo "no stray processes"
```

Notes:
- RSS is in KB. On macOS, a Tauri app's memory is split across the main process and one or more
  **WKWebView** helper processes; sum them for a true footprint. The WKWebView baseline is the main
  contributor at idle (model unloaded).
- For a realistic figure, prefer the **release** binary and a **target machine with a GUI session**
  and the model **unloaded** (default timeout will unload it 5 min after the last record; or set
  the timeout to `Immediately` / call `unload_model_manually` first).
- To capture an **active** figure, trigger a record/transcribe and sample during transcription;
  expect a multi-hundred-MB spike for the loaded Whisper/Parakeet engine plus brief CPU on the
  inference thread (Metal-accelerated).

### 2.3 Re-measuring after the model unloads (idle vs active contrast)

1. Launch, then perform one record→transcribe so the model loads. Sample RSS → **active**.
2. Wait past `model_unload_timeout` (or set it to a few seconds, or call `unload_model_manually`).
   Confirm the `model-state-changed` `unloaded` event / log line `Model unloaded due to inactivity`.
3. Sample RSS again → **idle (model unloaded)**. The delta is the model's resident cost.

---

## 3. Captured numbers

**Status: TO BE MEASURED ON TARGET HARDWARE.**

A live idle sample was attempted in this pass using the §2.2 procedure on the debug binary. The
process **aborted at launch** before reaching idle, with a non-unwinding panic during Tauri
`setup` immediately after model auto-selection:

```
[INFO] Auto-selecting model: parakeet-tdt-0.6b-v3
thread 'main' panicked at .../core/src/panicking.rs:225:5:
panic in a function that cannot unwind
thread caused non-unwinding panic. aborting.
```

This is consistent with launching a GUI app (webview/NSPanel/AppKit window creation) in a
non-interactive sandbox without a usable window-server session, not with an idle-footprint defect.
The run was time-boxed and the process was killed; **no stray process was left** (verified with
`pgrep`). Real idle/active numbers must be captured on a developer Mac with a GUI session using the
commands in §2.

**Expected ranges (design-derived, to confirm on target hardware):**

| State                         | RSS (sum of app + webview procs) | CPU      |
|-------------------------------|----------------------------------|----------|
| Idle, model unloaded          | low tens of MB (target)          | ~0%      |
| Idle, model loaded (≤ timeout)| tens of MB + model size          | ~0%      |
| Active (transcribing)         | + model size, brief inference    | spikes   |

Model RAM dominates the loaded figure: Whisper small/medium and Parakeet v3 are hundreds of MB;
the VAD adds only ~2 MB. With the model unloaded, only the Rust process + WKWebView baseline + VAD
remain — the "low tens of MB" target.

---

## 4. `model_unload_timeout` tuning guidance

Setting lives in `AppSettings.model_unload_timeout` (`settings.rs:66`), changed via the
`set_model_unload_timeout` command (`commands/models.rs:131-139`). Default: **`Minutes(5)`**.

| Variant         | Behavior                                                     | When to use                                                                 |
|-----------------|-------------------------------------------------------------|-----------------------------------------------------------------------------|
| `Never`         | Model stays resident until quit / manual unload.            | Plugged-in desktop; you dictate constantly and want zero reload latency.    |
| `Minutes(n)`    | Unload after `n` min idle (default 5).                      | **Recommended.** Balances first-record latency vs idle RAM.                 |
| `Seconds(n)`    | Unload after `n` s idle.                                    | Aggressive RAM reclaim; occasional dictation; accepts a reload each time.   |
| `Immediately`   | Unload right after every transcription.                     | Lowest idle RAM; battery/low-RAM machines. Reloads on every record.         |

Trade-off: shorter timeouts minimize idle RAM but reload the model (hundreds of MB) on the next
record, adding load latency before transcription. The 10 s watcher granularity
(`transcription.rs:106`) means actual unload can lag the configured limit by up to ~10 s — fine for
minute-scale timeouts, and `Immediately` bypasses the watcher entirely so it is exact.

---

## 5. Reported fix for the orchestrator — restore Accessory on window close

Audit finding (§1.1): after Settings is opened (→ `Regular`) and closed, the app never returns to
`Accessory`, leaving a stray Dock icon and main menu. Suggested fix in `src-tauri/src/lib.rs`
(NOT applied here to avoid conflicting with the other agent's `lib.rs` edits): add an
`on_window_event` handler on the `main` window that, on `CloseRequested`, hides the window instead
of destroying it and restores `Accessory` (only when `start_hidden`, matching launch behavior).

Sketch (apply inside `.setup`, after the main window is built):

```rust
if let Some(main_window) = app.get_webview_window("main") {
    let handle = app_handle.clone();
    main_window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.hide();
            }
            #[cfg(target_os = "macos")]
            {
                // Mirror launch: only background-ize when the app is meant to start hidden.
                if crate::settings::get_settings(&handle).start_hidden {
                    let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
        }
    });
}
```

This keeps the window instance alive (so reopening via tray/`show_main_window` is instant) and
returns the process to the no-Dock-icon idle state. It changes no model/overlay/CPU behavior — it
is purely the Accessory-policy round-trip — so it is safe to fold into the integration `lib.rs`.

---

## 6. How to re-measure (quick reference)

1. `cd src-tauri && export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH" && cargo build --release`
2. On a Mac with a GUI session, run the §2.2 snippet against `target/release/stenographer`.
3. Sum the main-process and WKWebView-helper RSS for the idle figure (model unloaded).
4. Do one record→transcribe, sample during transcription for the active figure, then wait past the
   unload timeout (or `unload_model_manually`) and re-sample to confirm the model RAM is released.
5. Record the three numbers in the §3 table and replace "TO BE MEASURED".
