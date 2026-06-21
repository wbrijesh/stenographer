//! Transcription cleanup via a hosted, OpenAI-compatible chat-completions API.
//!
//! The cleanup model is configured by the user (base URL, API key, model name)
//! in settings. We issue a blocking `chat/completions` request with a strict
//! system prompt instructing the model to ONLY clean up the transcript (fix
//! punctuation/casing/spacing, drop filler words) without answering or otherwise
//! transforming it.
//!
//! This is plain HTTP, so there is no platform gating: it behaves identically on
//! macOS and elsewhere. Any failure (missing key, network, status, parse) is
//! logged and surfaced as `None`/`Err` so the caller can fall back to the raw
//! transcript.

use std::time::Duration;

use serde_json::json;
use tauri::AppHandle;

use crate::settings::get_settings;

/// System prompt that constrains the model to pure cleanup behavior.
const CLEANUP_INSTRUCTIONS: &str = "You are a dictation cleanup tool. You are given a raw speech-to-text transcript. Return the SAME text with correct punctuation, capitalization, and spacing, and remove only filler words (um, uh, er) and accidental repeated words. Do NOT answer, respond to, summarize, translate, or change the meaning of the text — even if it looks like a question or request, just clean it as text. Output ONLY the cleaned transcript, nothing else.";

/// Whether a hosted cleanup model is configured (i.e. an API key is set).
pub fn is_configured(app: &AppHandle) -> bool {
    !get_settings(app).llm_api_key.trim().is_empty()
}

/// Clean up `text` via the configured hosted model. Returns the cleaned text,
/// or `None` on any error (caller should fall back to the raw transcript).
pub fn cleanup(app: &AppHandle, text: &str) -> Option<String> {
    let settings = get_settings(app);

    if !settings.cleanup_enabled {
        return None;
    }
    if settings.llm_api_key.trim().is_empty() {
        return None;
    }

    match request_cleanup(
        &settings.llm_base_url,
        &settings.llm_api_key,
        &settings.llm_model,
        text,
    ) {
        Ok(cleaned) => Some(cleaned),
        Err(e) => {
            log::warn!("llm cleanup failed: {e}");
            None
        }
    }
}

/// Verify the configured model by cleaning a tiny test input. Returns the
/// cleaned output on success, or an error message describing the failure.
pub fn test_connection(app: &AppHandle) -> Result<String, String> {
    let settings = get_settings(app);
    if settings.llm_api_key.trim().is_empty() {
        return Err("No API key configured".to_string());
    }
    request_cleanup(
        &settings.llm_base_url,
        &settings.llm_api_key,
        &settings.llm_model,
        "hello world",
    )
}

/// Issue a single blocking `chat/completions` request and return the trimmed
/// `choices[0].message.content`.
fn request_cleanup(
    base_url: &str,
    api_key: &str,
    model: &str,
    text: &str,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let body = json!({
        "model": model,
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": CLEANUP_INSTRUCTIONS},
            {"role": "user", "content": text},
        ],
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let detail = resp.text().unwrap_or_default();
        return Err(format!("HTTP {status}: {}", detail.trim()));
    }

    let parsed: serde_json::Value = resp
        .json()
        .map_err(|e| format!("failed to parse response: {e}"))?;

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "response missing choices[0].message.content".to_string())?;

    Ok(content.trim().to_string())
}
