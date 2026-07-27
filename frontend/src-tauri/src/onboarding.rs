use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;
use log::{info, warn, error};
use anyhow::Result;

use crate::state::AppState;
use crate::database::repositories::setting::SettingsRepository;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OnboardingStatus {
    pub version: String,
    pub completed: bool,
    pub current_step: u8,
    pub model_status: ModelStatus,
    pub last_updated: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ModelStatus {
    pub parakeet: String,  // "downloaded" | "not_downloaded" | "downloading"
    pub summary: String,   // Generic field for summary model (Qwen 3.5, Ruadapt Qwen3, or legacy Gemma variants)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_summary_model: Option<String>,
    /// Onboarding LLM-provider decision marker, mirrors the DB column of the
    /// same name: "local" | "cloud:<provider>" | "deferred" | None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_provider_choice: Option<String>,
}

/// The user's LLM-provider decision made during onboarding. Serialised as a
/// tagged union (`{"kind":"local",...}`) so the frontend's discriminated union
/// maps 1:1 and the compiler enforces exhaustive handling on both sides.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OnboardingProviderChoice {
    /// Download and use a local built-in model (auto-recommended from RAM).
    Local { model: String },
    /// Use a cloud provider with an API key. `provider` is one of the
    /// cloud LLMProvider ids (caila/openai/claude/openrouter).
    Cloud {
        provider: String,
        api_key: Option<String>,
        model: String,
    },
    /// Skip LLM selection entirely; the app will ask again on first summary.
    Skip,
}

impl Default for OnboardingStatus {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            completed: false,
            current_step: 1,
            model_status: ModelStatus {
                parakeet: "not_downloaded".to_string(),
                summary: "not_downloaded".to_string(),  // Changed from gemma
                selected_summary_model: None,
                summary_provider_choice: None,
            },
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }
}


/// Load onboarding status from store
pub async fn load_onboarding_status<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<OnboardingStatus> {
    // Try to load from Tauri store
    let store = match app.store("onboarding-status.json") {
        Ok(store) => store,
        Err(e) => {
            warn!("Failed to access onboarding store: {}, using defaults", e);
            return Ok(OnboardingStatus::default());
        }
    };

    // Try to get the status from store
    let status = if let Some(value) = store.get("status") {
        match serde_json::from_value::<OnboardingStatus>(value.clone()) {
            Ok(s) => {
                info!("Loaded onboarding status from store - Step: {}, Completed: {}",
                      s.current_step, s.completed);
                s
            }
            Err(e) => {
                warn!("Failed to deserialize onboarding status: {}, using defaults", e);
                OnboardingStatus::default()
            }
        }
    } else {
        info!("No stored onboarding status found, using defaults");
        OnboardingStatus::default()
    };

    Ok(status)
}

/// Save onboarding status to store
pub async fn save_onboarding_status<R: Runtime>(
    app: &AppHandle<R>,
    status: &OnboardingStatus,
) -> Result<()> {
    info!("Saving onboarding status: step={}, completed={}",
          status.current_step, status.completed);

    // Get or create store
    let store = app.store("onboarding-status.json")
        .map_err(|e| anyhow::anyhow!("Failed to access onboarding store: {}", e))?;

    // Update last_updated timestamp
    let mut status = status.clone();
    status.last_updated = chrono::Utc::now().to_rfc3339();

    // Serialize status to JSON value
    let status_value = serde_json::to_value(&status)
        .map_err(|e| anyhow::anyhow!("Failed to serialize onboarding status: {}", e))?;

    // Save to store
    store.set("status", status_value);

    // Persist to disk
    store.save()
        .map_err(|e| anyhow::anyhow!("Failed to save onboarding store to disk: {}", e))?;

    info!("Successfully persisted onboarding status to disk");
    Ok(())
}

/// Reset onboarding status (delete from store)
pub async fn reset_onboarding_status<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<()> {
    info!("Resetting onboarding status");

    let store = app.store("onboarding-status.json")
        .map_err(|e| anyhow::anyhow!("Failed to access onboarding store: {}", e))?;

    // Clear the status key
    store.delete("status");

    // Persist deletion to disk
    store.save()
        .map_err(|e| anyhow::anyhow!("Failed to save onboarding store after reset: {}", e))?;

    info!("Successfully reset onboarding status");
    Ok(())
}

/// Tauri commands for onboarding status
#[tauri::command]
pub async fn get_onboarding_status<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<OnboardingStatus>, String> {
    let status = load_onboarding_status(&app)
        .await
        .map_err(|e| format!("Failed to load onboarding status: {}", e))?;

    // Return None if it's the default (never saved before)
    // Check if we have any saved data by seeing if the store has the key
    let store = app.store("onboarding-status.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;

    if store.get("status").is_none() {
        Ok(None)
    } else {
        Ok(Some(status))
    }
}

#[tauri::command]
pub async fn save_onboarding_status_cmd<R: Runtime>(
    app: AppHandle<R>,
    status: OnboardingStatus,
) -> Result<(), String> {
    save_onboarding_status(&app, &status)
        .await
        .map_err(|e| format!("Failed to save onboarding status: {}", e))
}

#[tauri::command]
pub async fn reset_onboarding_status_cmd<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    reset_onboarding_status(&app)
        .await
        .map_err(|e| format!("Failed to reset onboarding status: {}", e))
}

#[tauri::command]
pub async fn complete_onboarding<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    choice: OnboardingProviderChoice,
) -> Result<(), String> {
    let pool = state.db_manager.pool();

    // Branch on the user's LLM-provider decision. STT (GigaAM) is always
    // saved below — this match only concerns the summary-side config.
    let (provider_for_status, choice_marker, saved_model_for_status) = match &choice {
        OnboardingProviderChoice::Local { model } => {
            info!("Completing onboarding: local built-in model = {}", model);
            if let Err(e) = SettingsRepository::save_model_config(
                pool,
                "builtin-ai",
                model,
                "large-v3",
                None,
            ).await {
                error!("Failed to save builtin-ai model config: {}", e);
                return Err(format!("Failed to save builtin-ai model config: {}", e));
            }
            ("builtin-ai".to_string(), "local".to_string(), model.clone())
        }
        OnboardingProviderChoice::Cloud { provider, api_key, model } => {
            info!("Completing onboarding: cloud provider = {}, model = {}", provider, model);
            if let Err(e) = SettingsRepository::save_model_config(
                pool,
                provider,
                model,
                "large-v3",
                None,
            ).await {
                error!("Failed to save cloud model config: {}", e);
                return Err(format!("Failed to save cloud model config: {}", e));
            }
            if let Some(key) = api_key {
                if !key.trim().is_empty() {
                    if let Err(e) = SettingsRepository::save_api_key(pool, provider, key).await {
                        // custom-openai legitimately rejects save_api_key — its
                        // key lives in customOpenAIConfig. Don't fail onboarding
                        // for that; the frontend persists it via a separate call.
                        if provider != "custom-openai" {
                            error!("Failed to save cloud api key: {}", e);
                            return Err(format!("Failed to save cloud api key: {}", e));
                        }
                    }
                }
            }
            (
                provider.clone(),
                format!("cloud:{}", provider),
                model.clone(),
            )
        }
        OnboardingProviderChoice::Skip => {
            info!("Completing onboarding: user deferred LLM provider selection");
            // Do NOT overwrite the active provider/model — leave whatever the
            // INSERT default left ("openai"/"gpt-4o..."). The frontend lazy-gate
            // will prompt the user on first summary generation.
            ("deferred".to_string(), "deferred".to_string(), "deferred".to_string())
        }
    };

    // Record the onboarding choice marker (separate column, keeps `provider`
    // NOT NULL invariant intact). Best-effort: a failure here is logged but
    // does not block completion, since the active provider is already saved.
    if let Err(e) = SettingsRepository::set_onboarding_provider_choice(pool, &choice_marker).await {
        warn!("Failed to record onboarding_provider_choice marker: {}", e);
    }

    // Seed the GPU-toggle default: ON iff a GPU was detected. Best-effort.
    let default_use_gpu =
        crate::audio::hardware_detector::HardwareProfile::detect().has_gpu_acceleration;
    if let Err(e) = SettingsRepository::set_use_gpu(pool, default_use_gpu).await {
        warn!("Failed to seed use_gpu default: {}", e);
    } else {
        info!("Seeded use_gpu default: {}", default_use_gpu);
    }

    // Save transcription model config. Convoic defaults to GigaAM (Russian STT),
    // which is the model downloaded during onboarding. Parakeet remains available
    // as an alternative the user can switch to in Settings.
    if let Err(e) = SettingsRepository::save_transcript_config(
        pool,
        "gigaam",
        crate::config::DEFAULT_GIGAAM_MODEL,
    ).await {
        error!("Failed to save transcription model config: {}", e);
        return Err(format!("Failed to save transcription model config: {}", e));
    }
    info!("Saved transcription model config: provider=gigaam, model={}", crate::config::DEFAULT_GIGAAM_MODEL);

    // Only NOW mark onboarding as complete (after DB operations succeed).
    let mut status = load_onboarding_status(&app)
        .await
        .map_err(|e| format!("Failed to load onboarding status: {}", e))?;

    status.completed = true;
    status.current_step = 4; // Max step (5 on macOS with permissions, 4 on other platforms)
    status.model_status.parakeet = "downloaded".to_string();
    // For Skip/cloud the "summary" model isn't a downloaded file; use a marker
    // that the frontend restore logic can distinguish from a half-downloaded Qwen.
    status.model_status.summary = match &choice {
        OnboardingProviderChoice::Local { .. } => "downloaded".to_string(),
        OnboardingProviderChoice::Cloud { .. } => "cloud".to_string(),
        OnboardingProviderChoice::Skip => "deferred".to_string(),
    };
    status.model_status.selected_summary_model = Some(saved_model_for_status);
    status.model_status.summary_provider_choice = Some(choice_marker);

    save_onboarding_status(&app, &status)
        .await
        .map_err(|e| format!("Failed to save completed onboarding status: {}", e))?;

    info!("Onboarding completed successfully (provider_for_status={}, marker={})", provider_for_status, status.model_status.summary_provider_choice.as_deref().unwrap_or("?"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_status_deserializes_without_selected_summary_model() {
        let status: OnboardingStatus = serde_json::from_str(
            r#"{
                "version": "1.0",
                "completed": true,
                "current_step": 4,
                "model_status": {
                    "parakeet": "downloaded",
                    "summary": "downloaded"
                },
                "last_updated": "2026-05-30T00:00:00Z"
            }"#,
        )
        .expect("old onboarding status should remain compatible");

        assert_eq!(status.model_status.selected_summary_model, None);
        // The new field must default to None on legacy payloads.
        assert_eq!(status.model_status.summary_provider_choice, None);
    }

    #[test]
    fn provider_choice_local_roundtrips() {
        let choice = OnboardingProviderChoice::Local {
            model: "qwen3.5:4b".to_string(),
        };
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains(r#""kind":"local""#), "json was: {}", json);
        let back: OnboardingProviderChoice = serde_json::from_str(&json).unwrap();
        match back {
            OnboardingProviderChoice::Local { model } => assert_eq!(model, "qwen3.5:4b"),
            other => panic!("expected Local, got {:?}", other),
        }
    }

    #[test]
    fn provider_choice_cloud_roundtrips() {
        let json = r#"{"kind":"cloud","provider":"caila","api_key":"secret","model":"just-ai/x"}"#;
        let choice: OnboardingProviderChoice = serde_json::from_str(json).unwrap();
        match choice {
            OnboardingProviderChoice::Cloud { provider, api_key, model } => {
                assert_eq!(provider, "caila");
                assert_eq!(api_key.as_deref(), Some("secret"));
                assert_eq!(model, "just-ai/x");
            }
            other => panic!("expected Cloud, got {:?}", other),
        }
    }

    #[test]
    fn provider_choice_skip_roundtrips() {
        let json = r#"{"kind":"skip"}"#;
        let choice: OnboardingProviderChoice = serde_json::from_str(json).unwrap();
        assert!(matches!(choice, OnboardingProviderChoice::Skip));
    }

    #[test]
    fn provider_choice_rejects_unknown_kind() {
        let json = r#"{"kind":"deferred"}"#;
        let result: Result<OnboardingProviderChoice, _> = serde_json::from_str(json);
        assert!(result.is_err(), "unknown kind should not deserialize");
    }
}
