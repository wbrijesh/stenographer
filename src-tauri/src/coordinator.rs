//! Pipeline coordinator state machine (Phase 1: trigger).
//!
//! The [`Coordinator`] owns the pipeline [`Stage`] and serializes all trigger
//! inputs through a single actor thread (mpsc), so keyboard events, signals, and
//! the (future) async transcribe/paste pipeline can never race.
//!
//! ## Phase-1 scope
//!
//! The coordinator does NOT call audio/transcription/overlay/tray directly yet.
//! On each meaningful transition it invokes an abstract [`PipelineAction`] hook
//! AND logs the transition (`HOLD-START`, `HOLD-STOP`, `TOGGLE-START`,
//! `TOGGLE-STOP`, `CANCEL`). The actions are *injected* via [`PipelineActions`]
//! (a struct of `Box<dyn Fn>` callbacks). The default actions only log and emit
//! a `coordinator-action` Tauri event; the Phase-2 integration layer replaces
//! them with real pipeline calls via [`Coordinator::set_actions`].

use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Debounce window for rapid-fire / repeated start inputs.
const DEBOUNCE: Duration = Duration::from_millis(30);

/// Classified trigger inputs fed to the coordinator by the Fn listener (or any
/// other producer, e.g. a tray menu item or a test).
///
/// Classification happens *before* this point — the listener applies the
/// hold/tap threshold and decides which variant to submit. See
/// [`crate::shortcut::fn_listener`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerInput {
    /// Push-to-talk began (Fn pressed). Provisional recording starts.
    HoldStart,
    /// Push-to-talk ended past the threshold (Fn released, held long enough).
    HoldStop,
    /// A hands-free toggle session began (Fn+Space).
    ToggleStart,
    /// The active toggle session ended (Fn tapped while toggling).
    ToggleStop,
    /// Abort whatever is in flight (e.g. Escape) without producing output.
    Cancel,
}

/// Pipeline lifecycle stage, owned exclusively by the coordinator thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Stage {
    /// Nothing happening.
    Idle,
    /// Capturing audio (either push-to-talk hold or a toggle session).
    Recording,
    /// Transcribing / pasting; new starts are ignored until finished.
    Processing,
}

/// Abstract action emitted by the coordinator on a state transition.
///
/// Phase-2 maps these onto real audio/transcription/overlay calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PipelineAction {
    /// Begin capturing audio.
    RecordStart,
    /// Stop capturing and run the transcribe/paste pipeline.
    RecordStop,
    /// Discard the in-flight capture without producing output.
    Cancel,
}

impl PipelineAction {
    fn as_str(self) -> &'static str {
        match self {
            PipelineAction::RecordStart => "RecordStart",
            PipelineAction::RecordStop => "RecordStop",
            PipelineAction::Cancel => "Cancel",
        }
    }
}

/// Injectable pipeline callbacks.
///
/// Each closure is invoked from the coordinator's actor thread, so it must be
/// `Send`. The default set (see [`PipelineActions::logging`]) only logs and
/// emits a `coordinator-action` Tauri event; the integration layer swaps in
/// real implementations via [`Coordinator::set_actions`].
pub struct PipelineActions {
    pub record_start: Box<dyn Fn() + Send>,
    pub record_stop: Box<dyn Fn() + Send>,
    pub cancel: Box<dyn Fn() + Send>,
}

impl PipelineActions {
    /// Default logging/event-emitting actions used until Phase-2 wires the real
    /// pipeline. Emits a `coordinator-action` Tauri event with a
    /// [`PipelineAction`] payload for each action.
    pub fn logging(app: AppHandle) -> Self {
        let emit = move |action: PipelineAction| {
            log::info!("coordinator action: {}", action.as_str());
            if let Err(e) = app.emit("coordinator-action", action) {
                log::warn!("failed to emit coordinator-action: {e}");
            }
        };
        let e1 = emit.clone();
        let e2 = emit.clone();
        let e3 = emit;
        Self {
            record_start: Box::new(move || e1(PipelineAction::RecordStart)),
            record_stop: Box::new(move || e2(PipelineAction::RecordStop)),
            cancel: Box::new(move || e3(PipelineAction::Cancel)),
        }
    }

    fn fire(&self, action: PipelineAction) {
        match action {
            PipelineAction::RecordStart => (self.record_start)(),
            PipelineAction::RecordStop => (self.record_stop)(),
            PipelineAction::Cancel => (self.cancel)(),
        }
    }
}

/// Commands processed sequentially by the coordinator thread.
enum Command {
    Input(TriggerInput),
    /// Called by the (future) pipeline when transcribe/paste finishes, to
    /// release the `Processing` stage back to `Idle`.
    ProcessingFinished,
    /// Replace the injected pipeline actions (integration wiring).
    SetActions(PipelineActions),
}

/// Resets `Stage` to `Idle` if the actor thread unwinds, so a panic in a
/// callback can't strand the pipeline in `Recording`/`Processing`.
struct StageGuard<'a> {
    stage: &'a Mutex<Stage>,
    armed: bool,
}

impl Drop for StageGuard<'_> {
    fn drop(&mut self) {
        if self.armed && thread::panicking() {
            if let Ok(mut s) = self.stage.lock() {
                *s = Stage::Idle;
            }
            log::error!("coordinator actor panicked; Stage reset to Idle");
        }
    }
}

/// Single-owner pipeline actor. Managed in Tauri state.
pub struct Coordinator {
    tx: Sender<Command>,
    /// Mirror of the actor's current stage, readable from any thread.
    stage: std::sync::Arc<Mutex<Stage>>,
}

impl Coordinator {
    /// Create the coordinator and spawn its actor thread. Until
    /// [`set_actions`](Self::set_actions) is called, transitions use the default
    /// logging/event actions.
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        let stage = std::sync::Arc::new(Mutex::new(Stage::Idle));
        let thread_stage = stage.clone();

        thread::spawn(move || {
            // Armed for the duration of the actor loop. Disarmed on clean exit
            // so only an actual panic triggers the Idle reset.
            let mut guard = StageGuard {
                stage: &thread_stage,
                armed: true,
            };

            let mut actions = PipelineActions::logging(app);
            // Whether a toggle (hands-free) session is currently active.
            let mut toggle_active = false;
            let mut last_start: Option<Instant> = None;

            let set_stage = |s: Stage| {
                if let Ok(mut g) = thread_stage.lock() {
                    *g = s;
                }
            };

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Command::SetActions(new_actions) => {
                        actions = new_actions;
                    }
                    Command::ProcessingFinished => {
                        toggle_active = false;
                        set_stage(Stage::Idle);
                        log::info!("PROCESSING-FINISHED");
                    }
                    Command::Input(input) => {
                        let stage = *thread_stage.lock().unwrap();
                        match input {
                            TriggerInput::HoldStart => {
                                // Debounce key-repeat / double-fire starts.
                                let now = Instant::now();
                                if last_start.map_or(false, |t| now.duration_since(t) < DEBOUNCE) {
                                    log::debug!("debounced HoldStart");
                                    continue;
                                }
                                last_start = Some(now);

                                if matches!(stage, Stage::Idle) {
                                    set_stage(Stage::Recording);
                                    log::info!("HOLD-START");
                                    actions.fire(PipelineAction::RecordStart);
                                } else {
                                    log::debug!("ignoring HoldStart: stage={stage:?}");
                                }
                            }
                            TriggerInput::HoldStop => {
                                // Only meaningful for an active push-to-talk
                                // recording that is NOT a toggle session.
                                if matches!(stage, Stage::Recording) && !toggle_active {
                                    set_stage(Stage::Processing);
                                    log::info!("HOLD-STOP");
                                    actions.fire(PipelineAction::RecordStop);
                                } else {
                                    log::debug!(
                                        "ignoring HoldStop: stage={stage:?} toggle={toggle_active}"
                                    );
                                }
                            }
                            TriggerInput::ToggleStart => {
                                // Fn+Space: cancel the provisional PTT for this
                                // press and begin a hands-free session.
                                match stage {
                                    Stage::Idle | Stage::Recording => {
                                        // If a provisional PTT recording was just
                                        // started by the Fn-down, it transitions
                                        // seamlessly into the toggle session
                                        // (same RecordStart already fired).
                                        if matches!(stage, Stage::Idle) {
                                            actions.fire(PipelineAction::RecordStart);
                                        }
                                        toggle_active = true;
                                        set_stage(Stage::Recording);
                                        log::info!("TOGGLE-START");
                                    }
                                    Stage::Processing => {
                                        log::debug!("ignoring ToggleStart: pipeline busy");
                                    }
                                }
                            }
                            TriggerInput::ToggleStop => {
                                if toggle_active && matches!(stage, Stage::Recording) {
                                    toggle_active = false;
                                    set_stage(Stage::Processing);
                                    log::info!("TOGGLE-STOP");
                                    actions.fire(PipelineAction::RecordStop);
                                } else {
                                    log::debug!("ignoring ToggleStop: not in a toggle session");
                                }
                            }
                            TriggerInput::Cancel => {
                                // Don't yank the rug during Processing — let the
                                // pipeline finish (it will signal ProcessingFinished).
                                if matches!(stage, Stage::Recording) {
                                    toggle_active = false;
                                    set_stage(Stage::Idle);
                                    log::info!("CANCEL");
                                    actions.fire(PipelineAction::Cancel);
                                } else {
                                    log::debug!("ignoring Cancel: stage={stage:?}");
                                }
                            }
                        }
                    }
                }
            }

            // Channel closed: normal shutdown, disarm the panic guard so it
            // does not reset the stage.
            guard.armed = false;
            log::debug!("coordinator actor exited");
        });

        Self { tx, stage }
    }

    /// Submit a classified trigger input. Serialized through the actor thread.
    pub fn submit(&self, input: TriggerInput) {
        if self.tx.send(Command::Input(input)).is_err() {
            log::warn!("coordinator channel closed; dropping input {input:?}");
        }
    }

    /// Signal that the async transcribe/paste pipeline finished, releasing the
    /// `Processing` stage back to `Idle`. (Phase-2 hook.)
    pub fn notify_processing_finished(&self) {
        if self.tx.send(Command::ProcessingFinished).is_err() {
            log::warn!("coordinator channel closed; dropping ProcessingFinished");
        }
    }

    /// Replace the injected pipeline actions. Called by the integration layer
    /// to swap the default logging actions for real pipeline calls.
    pub fn set_actions(&self, actions: PipelineActions) {
        if self.tx.send(Command::SetActions(actions)).is_err() {
            log::warn!("coordinator channel closed; dropping SetActions");
        }
    }

    /// Current pipeline stage (snapshot; lock-free for callers).
    pub fn stage(&self) -> Stage {
        self.stage.lock().map(|g| *g).unwrap_or(Stage::Idle)
    }
}
