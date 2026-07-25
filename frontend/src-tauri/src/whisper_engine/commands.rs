//! Whisper engine commands — STUB. See `mod.rs` for why the real engine is
//! gone. All commands report that Whisper is unavailable in this build.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Manager, Runtime};

/// Error returned by every stubbed Whisper operation.
const UNAVAILABLE: &str =
    "Whisper engine is not available in this build. Please use GigaAM (Russian) or Parakeet (English) instead.";

/// Model status — kept identical to the original enum so downstream pattern
/// matches (`ModelStatus::Available`, etc.) still compile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelStatus {
    Available,
    Missing,
    Downloading { progress: u8 },
    Error(String),
    Corrupted { file_size: u64, expected_min_size: u64 },
}

/// Model info — same shape as the original.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub path: PathBuf,
    pub size_mb: u32,
    pub accuracy: String,
    pub speed: String,
    pub status: ModelStatus,
    pub description: String,
}

/// Stub engine. Holds only the models directory so `get_models_directory`
/// and `set_models_directory` behave sensibly; no model is ever loaded.
pub struct WhisperEngine {
    models_dir: PathBuf,
}

impl WhisperEngine {
    pub fn new(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }

    pub async fn is_model_loaded(&self) -> bool {
        false
    }

    pub async fn get_current_model(&self) -> Option<String> {
        None
    }

    pub async fn load_model(&self, _name: &str) -> Result<(), String> {
        Err(UNAVAILABLE.to_string())
    }

    pub async fn unload_model(&self) -> bool {
        false
    }

    pub async fn discover_models(&self) -> Result<Vec<ModelInfo>, String> {
        Ok(Vec::new())
    }

    pub async fn get_models_directory(&self) -> PathBuf {
        self.models_dir.clone()
    }

    /// Whisper is unavailable in this build; transcription callers that hit
    /// this path get a runtime error (so they can fall back to GigaAM/Parakeet
    /// via the provider selection logic).
    pub async fn transcribe_audio_with_confidence(
        &self,
        _audio: Vec<f32>,
        _language: Option<String>,
    ) -> Result<(String, f32, bool), String> {
        Err(UNAVAILABLE.to_string())
    }

    pub async fn transcribe_audio(
        &self,
        _audio: Vec<f32>,
        _language: Option<String>,
    ) -> Result<String, String> {
        Err(UNAVAILABLE.to_string())
    }
}

/// Global engine singleton. Always `None` (nothing is ever loaded), but kept
/// so existing code paths that lock and inspect it still compile and run.
pub static WHISPER_ENGINE: Mutex<Option<Arc<WhisperEngine>>> = Mutex::new(None);

/// Models directory path, set during app setup (mirrors the parakeet/gigaam
/// pattern so the tray/UI code that opens the folder still works).
static MODELS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Initialize the models directory path using app_data_dir.
pub fn set_models_directory<R: Runtime>(app: &AppHandle<R>) {
    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let models_dir = app_data_dir.join("models");
        if !models_dir.exists() {
            let _ = std::fs::create_dir_all(&models_dir);
        }
        let mut guard = MODELS_DIR.lock().unwrap();
        *guard = Some(models_dir);
    }
}

fn get_models_directory() -> Option<PathBuf> {
    MODELS_DIR.lock().unwrap().clone()
}

#[command]
pub async fn whisper_init() -> Result<(), String> {
    // Nothing to initialize; report success so app startup doesn't abort.
    Ok(())
}

#[command]
pub async fn whisper_get_available_models() -> Result<Vec<ModelInfo>, String> {
    Ok(Vec::new())
}

#[command]
pub async fn whisper_load_model(_app_handle: AppHandle, _model_name: String) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn whisper_get_current_model() -> Result<Option<String>, String> {
    Ok(None)
}

#[command]
pub async fn whisper_is_model_loaded() -> Result<bool, String> {
    Ok(false)
}

#[command]
pub async fn whisper_has_available_models() -> Result<bool, String> {
    Ok(false)
}

#[command]
pub async fn whisper_validate_model_ready() -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}

pub async fn whisper_validate_model_ready_with_config<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
) -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn whisper_transcribe_audio(_audio_data: Vec<f32>) -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn whisper_get_models_directory() -> Result<String, String> {
    match get_models_directory() {
        Some(p) => Ok(p.to_string_lossy().to_string()),
        None => Err("Models directory not initialized".to_string()),
    }
}

#[command]
pub async fn whisper_download_model(
    _app_handle: AppHandle,
    _model_name: String,
) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn whisper_cancel_download(_model_name: String) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn whisper_delete_corrupted_model(_model_name: String) -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}

/// Open the models folder in the system file explorer. Functional (not
/// Whisper-specific) so it's kept working.
#[command]
pub async fn open_models_folder() -> Result<(), String> {
    let models_dir = get_models_directory()
        .ok_or_else(|| "Models directory not initialized".to_string())?
        .join("whisper");

    if !models_dir.exists() {
        std::fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create directory: {}", e))?;
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
    Ok(())
}
