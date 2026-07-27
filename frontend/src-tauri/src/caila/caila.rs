use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tauri::command;

/// Caila model information returned to frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CailaModel {
    pub id: String,
    pub owned_by: Option<String>,
}

/// API response model from Caila (OpenAI-compatible format)
#[derive(Debug, Deserialize)]
struct CailaApiModel {
    id: String,
    owned_by: Option<String>,
    #[allow(dead_code)]
    object: Option<String>,
}

/// API response wrapper from Caila
#[derive(Debug, Deserialize)]
struct CailaApiResponse {
    data: Vec<CailaApiModel>,
}

/// Cache entry for models
struct CacheEntry {
    models: Vec<CailaModel>,
    fetched_at: Instant,
}

/// Global cache for Caila models (5 minute TTL)
static MODELS_CACHE: RwLock<Option<CacheEntry>> = RwLock::new(None);

/// Cache TTL in seconds
const CACHE_TTL_SECS: u64 = 300;

/// Caila OpenAI adapter base URL (hardcoded, not user-editable)
const CAILA_BASE_URL: &str = "https://caila.io/api/adapters/openai";

/// Fallback models when API fetch fails or no key is provided.
/// These are known-good model IDs verified against the live Caila API.
const FALLBACK_MODELS: &[&str] = &[
    "just-ai/deepseek-deepseek/deepseek/deepseek-v4-flash",
    "just-ai/deepseek-deepseek/deepseek/deepseek-v4-pro",
];

/// Get fallback models as CailaModel vec
fn get_fallback_models() -> Vec<CailaModel> {
    FALLBACK_MODELS
        .iter()
        .map(|id| CailaModel {
            id: id.to_string(),
            owned_by: None,
        })
        .collect()
}

/// Check if model is a chat-capable model (filter out non-chat services).
///
/// Caila exposes a heterogeneous catalog (chat LLMs, embeddings, TTS, ASR, etc.).
/// For summary generation we only want chat-completion models.
fn is_chat_model(model_id: &str) -> bool {
    let id = model_id.to_lowercase();
    // Exclude well-known non-chat model families by keyword
    !id.contains("whisper")
        && !id.contains("embed")
        && !id.contains("guard")
        && !id.contains("tool-use")
        && !id.contains("tts")
        && !id.contains("asr")
        && !id.contains("speech")
        && !id.contains("vision-encoder")
}

/// Fetch Caila models from API.
///
/// # Arguments
/// * `api_key` - Caila API key (passed as a raw Authorization header, WITHOUT
///   the "Bearer " prefix — this is the key difference from other OpenAI-
///   compatible providers)
///
/// # Returns
/// Vector of available models, or fallback models on error / missing key
#[command]
pub async fn get_caila_models(api_key: Option<String>) -> Result<Vec<CailaModel>, String> {
    // Return fallback if no API key provided
    let api_key = match api_key {
        Some(key) if !key.trim().is_empty() => key.trim().to_string(),
        _ => {
            log::info!("No Caila API key provided, returning fallback models");
            return Ok(get_fallback_models());
        }
    };

    // Check cache first
    {
        let cache = MODELS_CACHE.read().map_err(|e| e.to_string())?;
        if let Some(entry) = cache.as_ref() {
            if entry.fetched_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
                log::info!("Returning cached Caila models ({} models)", entry.models.len());
                return Ok(entry.models.clone());
            }
        }
    }

    // Fetch from API. Note: Caila expects the raw API key in Authorization,
    // WITHOUT the "Bearer " prefix used by standard OpenAI-compatible APIs.
    log::info!("Fetching Caila models from API...");
    let client = reqwest::Client::new();

    let response = match client
        .get(format!("{}/models", CAILA_BASE_URL))
        .header("Authorization", &api_key)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            log::warn!("Failed to fetch Caila models: {}. Using fallback.", e);
            return Ok(get_fallback_models());
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        log::warn!("Caila API returned status {}. Using fallback models.", status);
        return Ok(get_fallback_models());
    }

    let api_response: CailaApiResponse = match response.json().await {
        Ok(data) => data,
        Err(e) => {
            log::warn!("Failed to parse Caila response: {}. Using fallback.", e);
            return Ok(get_fallback_models());
        }
    };

    // Filter to only chat models and map to our struct
    let models: Vec<CailaModel> = api_response
        .data
        .into_iter()
        .filter(|m| is_chat_model(&m.id))
        .map(|m| CailaModel {
            id: m.id,
            owned_by: m.owned_by,
        })
        .collect();

    // If no models returned, use fallback
    if models.is_empty() {
        log::warn!("No chat models returned from Caila API. Using fallback.");
        return Ok(get_fallback_models());
    }

    log::info!("Fetched {} Caila models from API", models.len());

    // Update cache
    {
        let mut cache = MODELS_CACHE.write().map_err(|e| e.to_string())?;
        *cache = Some(CacheEntry {
            models: models.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(models)
}

/// Clear the models cache (useful when API key changes)
pub fn clear_cache() {
    if let Ok(mut cache) = MODELS_CACHE.write() {
        *cache = None;
        log::info!("Caila models cache cleared");
    }
}
