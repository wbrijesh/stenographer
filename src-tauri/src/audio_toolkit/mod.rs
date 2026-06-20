//! Audio toolkit: capture (cpal), channel downmix, resample (rubato) and
//! voice-activity detection (Silero ONNX). Ported from Handy, macOS-only.

pub mod audio;
pub mod constants;
pub mod utils;
pub mod vad;

#[allow(unused_imports)]
pub use audio::{
    is_microphone_access_denied, is_no_input_device_error, list_input_devices, list_output_devices,
    read_wav_samples, save_wav_file, verify_wav_file, AudioRecorder, CpalDeviceInfo,
};
pub use utils::get_cpal_host;
#[allow(unused_imports)]
pub use vad::{SileroVad, SmoothedVad, VoiceActivityDetector};
