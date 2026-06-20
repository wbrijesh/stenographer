//! Tauri commands for the model-management UI.
//!
//! Per CONTRACTS §5. Each command is `#[tauri::command] #[specta::specta]` and
//! takes the Tauri-managed `Arc<ModelManager>` / `Arc<TranscriptionManager>`
//! state. Registration in `lib.rs` is performed by the orchestrator.

#![allow(dead_code)]

use crate::managers::model::{ModelInfo, ModelManager};
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// Full catalog with `is_downloaded` / `is_downloading` reflecting disk state.
#[tauri::command]
#[specta::specta]
pub async fn get_available_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelInfo>, String> {
    Ok(model_manager.get_available_models())
}

/// Stream a model download. Emits `model-download-progress` while running and
/// `model-download-complete` / `model-download-failed` on settle.
#[tauri::command]
#[specta::specta]
pub async fn download_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    let result = model_manager
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string());

    if let Err(ref error) = result {
        let _ = app_handle.emit(
            "model-download-failed",
            serde_json::json!({ "id": &model_id, "error": error }),
        );
    }

    result
}

/// Request cancellation of an in-flight download. The partial file is kept for
/// later resume. Emits `model-download-cancelled`.
#[tauri::command]
#[specta::specta]
pub async fn cancel_download(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .cancel_download(&model_id)
        .map_err(|e| e.to_string())
}

/// Delete a downloaded model. If it's the active model, unload it and clear the
/// persisted selection first.
#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    let settings = get_settings(&app_handle);
    if settings.selected_model.as_deref() == Some(model_id.as_str()) {
        transcription_manager
            .unload_model()
            .map_err(|e| format!("Failed to unload model: {}", e))?;

        let mut settings = settings;
        settings.selected_model = None;
        write_settings(&app_handle, &settings);
    }

    model_manager.delete_model(&model_id).map_err(|e| e.to_string())
}

/// Set the active model and eagerly load it (unless the unload timeout is
/// `Immediately`, in which case it loads on the next transcription).
#[tauri::command]
#[specta::specta]
pub async fn set_active_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    // Claim the loading slot up front to prevent concurrent switches.
    let _loading_guard = transcription_manager
        .try_start_loading()
        .ok_or_else(|| "Model load already in progress".to_string())?;

    // Validate + persist the selection (resets language if unsupported).
    model_manager.set_active(&model_id)?;

    // Eager-load unless the timeout is `Immediately`.
    let settings = get_settings(&app_handle);
    if settings.model_unload_timeout == ModelUnloadTimeout::Immediately {
        return Ok(());
    }

    transcription_manager
        .load_model(&model_id)
        .map_err(|e| e.to_string())
}

/// The persisted active model id (empty string when none selected).
#[tauri::command]
#[specta::specta]
pub async fn get_current_model(app_handle: AppHandle) -> Result<String, String> {
    Ok(get_settings(&app_handle).selected_model.unwrap_or_default())
}

/// Whether a model load is currently in progress (no model loaded yet).
#[tauri::command]
#[specta::specta]
pub async fn is_model_loading(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<bool, String> {
    Ok(transcription_manager.get_current_model().is_none())
}

/// Persist a new model-unload timeout (idle watcher picks it up next tick).
#[tauri::command]
#[specta::specta]
pub async fn set_model_unload_timeout(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    timeout: ModelUnloadTimeout,
) -> Result<(), String> {
    transcription_manager.set_unload_timeout(timeout);
    Ok(())
}

/// Manually unload the loaded model (frees memory until next transcription).
#[tauri::command]
#[specta::specta]
pub async fn unload_model_manually(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<(), String> {
    transcription_manager
        .unload_model()
        .map_err(|e| e.to_string())
}
