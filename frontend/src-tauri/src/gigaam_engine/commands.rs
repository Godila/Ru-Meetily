//! Tauri commands for the GigaAM engine. Adapted from
//! `parakeet_engine/commands.rs` — same structure, `gigaam_*` names and
//! `gigaam-model-*` events.

use crate::gigaam_engine::{GigaamEngine, ModelInfo, ModelStatus, DownloadProgress};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Emitter, Manager, Runtime};

/// Global GigaAM engine singleton.
pub static GIGAAM_ENGINE: Mutex<Option<Arc<GigaamEngine>>> = Mutex::new(None);

/// Global models directory path (set during app initialization).
static MODELS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Initialize the models directory path using app_data_dir.
/// Should be called during app setup before `gigaam_init`.
pub fn set_models_directory<R: Runtime>(app: &AppHandle<R>) {
    let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
    let models_dir = app_data_dir.join("models");
    if !models_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&models_dir) {
            log::error!("Failed to create models directory: {}", e);
            return;
        }
    }
    log::info!("GigaAM models directory set to: {}", models_dir.display());
    let mut guard = MODELS_DIR.lock().unwrap();
    *guard = Some(models_dir);
}

/// Get the configured models directory.
fn get_models_directory() -> Option<PathBuf> {
    MODELS_DIR.lock().unwrap().clone()
}

#[command]
pub async fn gigaam_init() -> Result<(), String> {
    let mut guard = GIGAAM_ENGINE.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }
    let models_dir = get_models_directory();
    let engine = GigaamEngine::new_with_models_dir(models_dir)
        .map_err(|e| format!("Failed to initialize GigaAM engine: {}", e))?;
    *guard = Some(Arc::new(engine));
    Ok(())
}

#[command]
pub async fn gigaam_get_available_models() -> Result<Vec<ModelInfo>, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        engine
            .discover_models()
            .await
            .map_err(|e| format!("Failed to discover GigaAM models: {}", e))
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_load_model<R: Runtime>(
    app_handle: AppHandle<R>,
    model_name: String,
) -> Result<(), String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        let _ = app_handle.emit(
            "gigaam-model-loading-started",
            serde_json::json!({ "modelName": model_name }),
        );

        let result = engine
            .load_model(&model_name)
            .await
            .map_err(|e| format!("Failed to load GigaAM model: {}", e));

        if result.is_ok() {
            let _ = app_handle.emit(
                "gigaam-model-loading-completed",
                serde_json::json!({ "modelName": model_name }),
            );
        } else if let Err(ref error) = result {
            let _ = app_handle.emit(
                "gigaam-model-loading-failed",
                serde_json::json!({ "modelName": model_name, "error": error }),
            );
        }
        result
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_get_current_model() -> Result<Option<String>, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        Ok(engine.get_current_model().await)
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_is_model_loaded() -> Result<bool, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        Ok(engine.is_model_loaded().await)
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_has_available_models() -> Result<bool, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        let models = engine
            .discover_models()
            .await
            .map_err(|e| format!("Failed to discover GigaAM models: {}", e))?;
        let available: Vec<_> = models
            .iter()
            .filter(|m| matches!(m.status, ModelStatus::Available))
            .collect();
        Ok(!available.is_empty())
    } else {
        Ok(false)
    }
}

#[command]
pub async fn gigaam_validate_model_ready() -> Result<String, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        if engine.is_model_loaded().await {
            if let Some(current_model) = engine.get_current_model().await {
                return Ok(current_model);
            }
        }
        let models = engine
            .discover_models()
            .await
            .map_err(|e| format!("Failed to discover GigaAM models: {}", e))?;
        let available: Vec<_> = models
            .iter()
            .filter(|m| matches!(m.status, ModelStatus::Available))
            .collect();
        if available.is_empty() {
            return Err(
                "No GigaAM models are available. Please download a model to enable Russian transcription."
                    .to_string(),
            );
        }
        let first = available
            .iter()
            .find(|m| m.quantization == crate::gigaam_engine::QuantizationType::Int8)
            .or_else(|| available.first())
            .unwrap();
        engine
            .load_model(&first.name)
            .await
            .map_err(|e| format!("Failed to load GigaAM model {}: {}", first.name, e))?;
        Ok(first.name.clone())
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

/// Internal version that respects the user's transcript config (analogous to
/// Parakeet's `parakeet_validate_model_ready_with_config`).
pub async fn gigaam_validate_model_ready_with_config<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<String, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        if engine.is_model_loaded().await {
            if let Some(current_model) = engine.get_current_model().await {
                log::info!("GigaAM model already loaded: {}", current_model);
                return Ok(current_model);
            }
        }

        // Try the user's configured model.
        let model_to_load = match crate::api::api::api_get_transcript_config(
            app.clone(),
            app.state(),
            None,
        )
        .await
        {
            Ok(Some(config)) => {
                log::info!(
                    "Got transcript config from API - provider: {}, model: {}",
                    config.provider,
                    config.model
                );
                if config.provider == "gigaam" && !config.model.is_empty() {
                    Some(config.model)
                } else {
                    None
                }
            }
            _ => None,
        };

        let models = engine
            .discover_models()
            .await
            .map_err(|e| format!("Failed to discover GigaAM models: {}", e))?;
        let available: Vec<_> = models
            .iter()
            .filter(|m| matches!(m.status, ModelStatus::Available))
            .collect();
        if available.is_empty() {
            return Err(
                "No GigaAM models are available. Please download a model to enable Russian transcription."
                    .to_string(),
            );
        }

        let model_name = if let Some(configured) = model_to_load {
            if available.iter().any(|m| m.name == configured) {
                configured
            } else {
                log::warn!(
                    "Configured GigaAM model '{}' not found, falling back to first available",
                    configured
                );
                available.first().unwrap().name.clone()
            }
        } else {
            available.first().unwrap().name.clone()
        };

        engine
            .load_model(&model_name)
            .await
            .map_err(|e| format!("Failed to load GigaAM model {}: {}", model_name, e))?;
        Ok(model_name)
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_transcribe_audio(audio_data: Vec<f32>) -> Result<String, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        engine
            .transcribe_audio(audio_data)
            .await
            .map_err(|e| format!("GigaAM transcription failed: {}", e))
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_get_models_directory() -> Result<String, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        let path = engine.get_models_directory().await;
        Ok(path.to_string_lossy().to_string())
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_download_model<R: Runtime>(
    app_handle: AppHandle<R>,
    model_name: String,
) -> Result<(), String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        let app_handle_clone = app_handle.clone();
        let model_name_clone = model_name.clone();

        let progress_callback = Box::new(move |progress: DownloadProgress| {
            log::info!(
                "GigaAM download progress for {}: {:.1} MB / {:.1} MB ({:.1} MB/s) - {}%",
                model_name_clone,
                progress.downloaded_mb,
                progress.total_mb,
                progress.speed_mbps,
                progress.percent
            );
            let _ = app_handle_clone.emit(
                "gigaam-model-download-progress",
                serde_json::json!({
                    "modelName": model_name_clone,
                    "progress": progress.percent,
                    "downloaded_bytes": progress.downloaded_bytes,
                    "total_bytes": progress.total_bytes,
                    "downloaded_mb": progress.downloaded_mb,
                    "total_mb": progress.total_mb,
                    "speed_mbps": progress.speed_mbps,
                    "status": if progress.percent == 100 { "completed" } else { "downloading" }
                }),
            );
        });

        if let Err(e) = engine.discover_models().await {
            log::warn!("Failed to discover models before download: {}", e);
        }

        let result = engine.download_model_detailed(&model_name, Some(progress_callback)).await;

        match result {
            Ok(()) => {
                let _ = app_handle.emit(
                    "gigaam-model-download-complete",
                    serde_json::json!({ "modelName": model_name }),
                );
                log::info!("GigaAM model download complete - updating tray menu");
                crate::tray::update_tray_menu(&app_handle);
                Ok(())
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "gigaam-model-download-error",
                    serde_json::json!({ "modelName": model_name, "error": e.to_string() }),
                );
                Err(format!("Failed to download GigaAM model: {}", e))
            }
        }
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_cancel_download<R: Runtime>(
    app_handle: AppHandle<R>,
    model_name: String,
) -> Result<(), String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        engine
            .cancel_download(&model_name)
            .await
            .map_err(|e| format!("Failed to cancel GigaAM download: {}", e))?;
        let _ = app_handle.emit(
            "gigaam-model-download-progress",
            serde_json::json!({ "modelName": model_name, "progress": 0, "status": "cancelled" }),
        );
        log::info!("GigaAM download cancelled: {}", model_name);
        Ok(())
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_retry_download<R: Runtime>(
    app_handle: AppHandle<R>,
    model_name: String,
) -> Result<(), String> {
    log::info!("Retrying download for: {}", model_name);
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        {
            let mut active = engine.active_downloads.write().await;
            if active.contains(&model_name) {
                log::warn!("Retry: Model {} was still in active downloads, removing", model_name);
                active.remove(&model_name);
            }
        }
        {
            let mut models = engine.available_models.write().await;
            if let Some(model) = models.get_mut(&model_name) {
                log::info!(
                    "Retry: Resetting model {} status from {:?} to Missing",
                    model_name,
                    model.status
                );
                model.status = ModelStatus::Missing;
            }
        }
        let _ = engine.discover_models().await;
        gigaam_download_model(app_handle, model_name).await
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

#[command]
pub async fn gigaam_delete_corrupted_model(model_name: String) -> Result<String, String> {
    let engine = {
        let guard = GIGAAM_ENGINE.lock().unwrap();
        guard.as_ref().cloned()
    };
    if let Some(engine) = engine {
        engine
            .delete_model(&model_name)
            .await
            .map_err(|e| format!("Failed to delete GigaAM model: {}", e))
    } else {
        Err("GigaAM engine not initialized".to_string())
    }
}

/// Open the GigaAM models folder in the system file explorer.
#[command]
pub async fn open_gigaam_models_folder() -> Result<(), String> {
    let models_dir = get_models_directory()
        .ok_or_else(|| "GigaAM models directory not initialized".to_string())?
        .join("gigaam");

    if !models_dir.exists() {
        std::fs::create_dir_all(&models_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let folder_path = models_dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    log::info!("Opened GigaAM models folder: {}", folder_path);
    Ok(())
}
