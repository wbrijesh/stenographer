# Stenographer — Interface Contracts (v1)

This is the architectural contract that lets subsystems be built in parallel. **Module owners
must not change a public signature here without orchestrator sign-off.** Reference implementation
for everything is Handy at `/tmp/Handy` (MIT). macOS-only; drop all Windows/Linux branches.

Crate name: `stenographer`. App identifier: `dev.brijesh.stenographer`.

---

## 0. Module ownership (disjoint file sets)

| Module | Owns (paths under `src-tauri/src/` unless noted) |
|---|---|
| audio | `audio_toolkit/**`, `managers/audio.rs` |
| transcription | `managers/model.rs`, `managers/transcription.rs`, `commands/models.rs` |
| paste | `clipboard.rs`, `input.rs` |
| trigger | `shortcut/**`, `coordinator.rs` |
| overlay | `overlay.rs`, frontend `src/overlay/**` |
| tray | `tray.rs` |
| ui | frontend `src/components/**`, `src/stores/**`, `src/hooks/**`, `src/i18n` (en only) |
| **integration (orchestrator-merged)** | `lib.rs`, `main.rs`, `commands/mod.rs`, `settings.rs`, `Cargo.toml`, `tauri.conf.json`, `bindings.ts` |

`lib.rs`, `Cargo.toml`, `settings.rs`, `tauri.conf.json`, `bindings.ts` are **shared** — parallel
agents append to clearly-marked sections or hand additions to the integration step; they do not
rewrite these files.

---

## 1. Settings schema (`settings.rs`)

`AppSettings` — `serde`, every field `#[serde(default = "...")]`, persisted via `tauri-plugin-store`
to `settings_store.json` under a single `"settings"` key. Provide `get_default_settings()`.

```rust
pub struct AppSettings {
  // trigger
  pub trigger_mode_enabled: bool,         // master on/off (default true)
  pub hold_tap_threshold_ms: u64,         // default 250
  // audio
  pub selected_microphone: Option<String>,
  pub selected_output_device: Option<String>,
  pub audio_feedback: bool,               // start/stop sounds, default true
  pub audio_feedback_volume: f32,         // default 1.0
  pub mute_while_recording: bool,         // default false
  // transcription / models
  pub selected_model: Option<String>,     // model id
  pub selected_language: String,          // default "auto"
  pub translate_to_english: bool,         // default false
  pub model_unload_timeout: ModelUnloadTimeout, // default Minutes(5)
  // output / paste
  pub paste_delay_ms: u64,                // default 60
  pub auto_submit: bool,                  // default false
  pub auto_submit_key: AutoSubmitKey,     // default Enter
  pub append_trailing_space: bool,        // default false
  // overlay / app
  pub overlay_enabled: bool,              // default true (bottom-center pill)
  pub start_hidden: bool,                 // default true
  pub autostart_enabled: bool,            // default false
  pub show_tray_icon: bool,               // default true
}

pub enum ModelUnloadTimeout { Never, Immediately, Seconds(u64), Minutes(u64) }
pub enum AutoSubmitKey { Enter, CtrlEnter, CmdEnter }
```

Each setting gets a dedicated setter command (validate + persist + emit), per Handy's pattern.

---

## 2. Rust module interfaces

### audio — `AudioRecordingManager` (Tauri-managed `Arc`)
```rust
fn new(app: AppHandle) -> Self;
fn preload_vad(&self);                       // load Silero ONNX, idempotent
fn start_recording(&self, binding_id: &str) -> Result<(), AudioError>;
fn stop_recording(&self) -> Vec<f32>;        // 16kHz mono, VAD-filtered, blocking drain
fn is_recording(&self) -> bool;
fn list_input_devices() -> Vec<DeviceInfo>;  // {name, is_default}
fn list_output_devices() -> Vec<DeviceInfo>;
```
Emits event `mic-level` (f32 0..1) while recording. `AudioError` variants must distinguish
`MicPermissionDenied` and `NoInputDevice` (parse cpal error strings like Handy).

### transcription — `TranscriptionManager` + `ModelManager` (Tauri-managed `Arc`)
```rust
// ModelManager
fn available_models(&self) -> Vec<ModelInfo>; // catalog (see §4), with is_downloaded/is_downloading
fn download_model(&self, id: &str) -> Result<(), String>;   // streams + emits progress events
fn cancel_download(&self, id: &str);
fn delete_model(&self, id: &str) -> Result<(), String>;
fn set_active(&self, id: &str) -> Result<(), String>;
// TranscriptionManager
fn initiate_model_load(&self);               // background load of active model
fn transcribe(&self, samples: Vec<f32>) -> Result<String, String>;
fn unload(&self);
fn set_unload_timeout(&self, t: ModelUnloadTimeout);
```
Use `transcribe-rs` with features `whisper-cpp,onnx,whisper-metal` on macOS. Models stored in
`<app_data>/models/`. Emits: `model-download-progress {id,downloaded,total,percentage}`,
`model-download-complete|failed|cancelled {id}`, `model-state-changed`.

### paste — free functions (run on main thread)
```rust
fn paste(text: String, app: AppHandle) -> Result<(), String>; // clipboard save→write→Cmd+V→restore
fn init_enigo(app: AppHandle) -> Result<(), String>;          // lazy; fails if no Accessibility
```
Respects `paste_delay_ms`, `auto_submit`(+key), `append_trailing_space` from settings.
macOS Cmd+V via raw keycode (`Key::Meta` + `Key::Other(9)`), per Handy `input.rs`.

### trigger — Fn state machine + `Coordinator`
The low-level listener (handy-keys/rdev CGEventTap) detects Fn down/up + Fn+Space and feeds the
coordinator **classified** inputs. The coordinator is the single owner of pipeline state.
```rust
pub enum TriggerInput { HoldStart, HoldStop, ToggleStart, ToggleStop, Cancel }
pub enum Stage { Idle, Recording, Processing }
impl Coordinator {
  fn new(app: AppHandle) -> Self;            // managed in Tauri state
  fn submit(&self, input: TriggerInput);     // serialized via mpsc to actor thread
}
```
Classification rules (threshold = `hold_tap_threshold_ms`, default 250):
- Fn down → start provisional recording immediately; record `Instant`.
- Fn up, elapsed ≥ threshold → `HoldStop` (push-to-talk end).
- Fn up, elapsed < threshold, no active toggle → discard provisional (no-op).
- Fn+Space (Space while Fn held) → `ToggleStart` (cancels provisional PTT for that press).
- Fn tap while toggle session active → `ToggleStop`.
The actor calls the pipeline (see §3) and drives overlay+tray via the integration layer, never UI directly.

### overlay — free functions (macOS NSPanel via tauri-nspanel)
```rust
fn create_overlay(app: &AppHandle);          // build hidden panel at startup
fn show_recording(app: &AppHandle);
fn show_transcribing(app: &AppHandle);
fn hide_overlay(app: &AppHandle);
```
**Aerospace rules (hard requirements):** non-activating panel (`can_become_key_window:false`,
`is_floating_panel:true`), `PanelLevel::Status`, collection behavior
`canJoinAllSpaces | fullScreenAuxiliary | transient | stationary`, re-home to active Space + reposition
on every show, use order-front-regardless (never make key / never activate app). Bottom-center on the
cursor's monitor. Renders `src/overlay/index.html`; listens to `show-overlay`/`hide-overlay`/`mic-level`.

### tray — `tray.rs`
```rust
fn create_tray(app: &AppHandle) -> Result<TrayIcon, String>;
fn set_state(app: &AppHandle, state: TrayState);   // Idle|Recording|Transcribing → icon swap
```
Accessory app (no Dock icon). Menu: Settings, model submenu, Cancel, Quit.

---

## 3. Pipeline (integration layer wires this in `lib.rs`/`coordinator.rs`)

`record start` → parallel(`initiate_model_load`, `preload_vad`) + `tray.set_state(Recording)` +
`overlay.show_recording` + start sound + `start_recording`. `record stop` → `tray.set_state(Transcribing)`
+ stop sound + (`stop_recording` → `transcribe`) → `paste` on main thread → `overlay.hide` +
`tray.set_state(Idle)`. A Drop-guard resets `Stage` to `Idle` on panic.

---

## 4. Model catalog (reuse Handy values)

Pull the exact catalog (ids, filenames, `url` at `https://blob.handy.computer/...`, `sha256`,
`size_mb`, `engine_type`, `supported_languages`, scores) from Handy `managers/model.rs`. v1 ships
at least: `parakeet-tdt-0.6b-v3` (default, recommended), Whisper `small`/`medium`/`turbo`/`large`.
`ModelInfo` fields: `{id,name,description,filename,url,sha256,size_mb,is_downloaded,is_downloading,
partial_size,is_directory,engine_type,accuracy_score,speed_score,supports_translation,is_recommended,
supported_languages,supports_language_selection}`.

---

## 5. IPC command surface (tauri-specta → `src/bindings.ts`)

Settings: `get_app_settings`, `get_default_settings`, + one setter per field (e.g.
`change_paste_delay_ms`, `change_overlay_enabled`, `set_selected_microphone`, …).
Models: `get_available_models`, `download_model`, `cancel_download`, `delete_model`, `set_active_model`,
`get_current_model`, `is_model_loading`, `set_model_unload_timeout`, `unload_model_manually`.
Audio: `get_available_microphones`, `get_available_output_devices`, `set/get_selected_microphone`,
`set/get_selected_output_device`, `play_test_sound`, `is_recording`.
Permissions/lifecycle: `initialize_enigo`, `initialize_shortcuts`, `cancel_operation`,
`show_main_window`, paths/log helpers.

---

## 6. Events (backend → frontend)

`mic-level`, `show-overlay`, `hide-overlay`, `recording-error {error_type}`, `paste-error`,
`model-download-progress`, `model-download-complete|failed|cancelled`, `model-state-changed`.

---

## 7. Conventions

Rust 2021, `cargo fmt` + `clippy` clean. macOS-only `#[cfg(target_os="macos")]` where relevant (most
branches just removed). No `git` from sub-agents (orchestrator commits). Do not run `tauri dev`
(blocks); verify with `cargo build` (in `src-tauri`) and `bun run build`. English-only UI (no i18n
framework needed in v1, but keep strings centralized).
