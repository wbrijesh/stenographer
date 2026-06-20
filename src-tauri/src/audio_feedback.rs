//! Start/stop audio feedback "pop" sounds. macOS-only port of Handy's
//! `audio_feedback.rs`, simplified to Stenographer's settings (no `SoundTheme`).
//!
//! The sound WAVs are **embedded** via `include_bytes!` (like Handy's tray
//! icons) so they are always available without any bundling configuration. The
//! `pop` theme is the default; the `marimba` assets are embedded too for a
//! potential future setting but are currently unused.
//!
//! Playback honours the `audio_feedback` (bool) / `audio_feedback_volume` (f32)
//! settings and routes to the configured `selected_output_device`, falling back
//! to the system default when unset or missing.

#![allow(dead_code)]

use std::io::Cursor;
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait};
use log::{debug, error, warn};
use rodio::OutputStreamBuilder;
use tauri::AppHandle;

use crate::settings::{self, AppSettings};

/// Which feedback sound to play.
#[derive(Clone, Copy, Debug)]
pub enum SoundType {
    Start,
    Stop,
}

// Embedded sound assets (compiled into the binary).
const POP_START: &[u8] = include_bytes!("../resources/pop_start.wav");
const POP_STOP: &[u8] = include_bytes!("../resources/pop_stop.wav");
// Marimba theme — embedded for a future "sound theme" setting; unused today.
const MARIMBA_START: &[u8] = include_bytes!("../resources/marimba_start.wav");
const MARIMBA_STOP: &[u8] = include_bytes!("../resources/marimba_stop.wav");

fn sound_bytes(sound_type: SoundType) -> &'static [u8] {
    match sound_type {
        SoundType::Start => POP_START,
        SoundType::Stop => POP_STOP,
    }
}

/// Play the recording-start sound (async, fire-and-forget). No-op if
/// `audio_feedback` is disabled in settings.
pub fn play_start_sound(app: &AppHandle) {
    play_feedback_sound(app, SoundType::Start);
}

/// Play the recording-stop sound (async, fire-and-forget). No-op if
/// `audio_feedback` is disabled in settings.
pub fn play_stop_sound(app: &AppHandle) {
    play_feedback_sound(app, SoundType::Stop);
}

/// Play the start sound regardless of the `audio_feedback` setting, blocking
/// until it finishes. Used by the `play_test_sound` command so the user can
/// preview the volume even while feedback is toggled off.
pub fn play_test_sound(app: &AppHandle) {
    let settings = settings::get_settings(app);
    if let Err(e) = play_audio_bytes(sound_bytes(SoundType::Start), &settings) {
        error!("Failed to play test sound: {e}");
    }
}

fn play_feedback_sound(app: &AppHandle, sound_type: SoundType) {
    let settings = settings::get_settings(app);
    if !settings.audio_feedback {
        return;
    }
    play_sound_async(settings, sound_type);
}

fn play_sound_async(settings: AppSettings, sound_type: SoundType) {
    thread::spawn(move || {
        if let Err(e) = play_audio_bytes(sound_bytes(sound_type), &settings) {
            error!("Failed to play feedback sound: {e}");
        }
    });
}

fn play_audio_bytes(
    bytes: &'static [u8],
    settings: &AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    let selected_device = settings.selected_output_device.clone();
    let volume = settings.audio_feedback_volume;

    let stream_builder = match selected_device {
        Some(device_name) if device_name != "Default" => {
            let host = crate::audio_toolkit::get_cpal_host();
            let mut found_device = None;
            for device in host.output_devices()? {
                if device.name().map(|n| n == device_name).unwrap_or(false) {
                    found_device = Some(device);
                    break;
                }
            }
            match found_device {
                Some(device) => OutputStreamBuilder::from_device(device)?,
                None => {
                    warn!("Output device '{device_name}' not found, using default");
                    OutputStreamBuilder::from_default_device()?
                }
            }
        }
        _ => {
            debug!("Using default output device for feedback sound");
            OutputStreamBuilder::from_default_device()?
        }
    };

    let stream_handle = stream_builder.open_stream()?;
    let mixer = stream_handle.mixer();

    // Decode from the embedded bytes (a Cursor implements Read + Seek).
    let cursor = Cursor::new(bytes);
    let sink = rodio::play(mixer, cursor)?;
    sink.set_volume(volume);
    sink.sleep_until_end();

    Ok(())
}
