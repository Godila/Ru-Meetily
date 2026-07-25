//! GigaAM engine: model lifecycle, discovery, and download.
//!
//! Adapted from `parakeet_engine/parakeet_engine.rs`. The structure is
//! identical; only the model catalog, required files, and download URLs differ.

use crate::gigaam_engine::model::GigaamModel;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufWriter};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;

/// Quantization type for GigaAM models.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuantizationType {
    FP32,
    Int8,
}

impl Default for QuantizationType {
    fn default() -> Self {
        QuantizationType::Int8
    }
}

/// Model status for GigaAM models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelStatus {
    Available,
    Missing,
    Downloading { progress: u8 },
    Error(String),
    Corrupted { file_size: u64, expected_min_size: u64 },
}

/// Detailed download progress info (MB-based with speed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub downloaded_mb: f64,
    pub total_mb: f64,
    pub speed_mbps: f64,
    pub percent: u8,
}

impl DownloadProgress {
    pub fn new(downloaded: u64, total: u64, speed_mbps: f64) -> Self {
        let percent = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0).min(100.0) as u8
        } else {
            0
        };
        Self {
            downloaded_bytes: downloaded,
            total_bytes: total,
            downloaded_mb: downloaded as f64 / (1024.0 * 1024.0),
            total_mb: total as f64 / (1024.0 * 1024.0),
            speed_mbps,
            percent,
        }
    }
}

/// Information about a GigaAM model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub path: PathBuf,
    pub size_mb: u32,
    pub quantization: QuantizationType,
    pub speed: String,
    pub status: ModelStatus,
    pub description: String,
}

#[derive(Debug)]
pub enum GigaamEngineError {
    ModelNotLoaded,
    ModelNotFound(String),
    TranscriptionFailed(String),
    DownloadFailed(String),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for GigaamEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GigaamEngineError::ModelNotLoaded => write!(f, "No GigaAM model loaded"),
            GigaamEngineError::ModelNotFound(name) => write!(f, "Model '{}' not found", name),
            GigaamEngineError::TranscriptionFailed(err) => {
                write!(f, "Transcription failed: {}", err)
            }
            GigaamEngineError::DownloadFailed(err) => write!(f, "Download failed: {}", err),
            GigaamEngineError::IoError(err) => write!(f, "IO error: {}", err),
            GigaamEngineError::Other(err) => write!(f, "Error: {}", err),
        }
    }
}

impl std::error::Error for GigaamEngineError {}

impl From<std::io::Error> for GigaamEngineError {
    fn from(err: std::io::Error) -> Self {
        GigaamEngineError::IoError(err)
    }
}

pub struct GigaamEngine {
    models_dir: PathBuf,
    current_model: Arc<RwLock<Option<GigaamModel>>>,
    current_model_name: Arc<RwLock<Option<String>>>,
    pub(crate) available_models: Arc<RwLock<HashMap<String, ModelInfo>>>,
    cancel_download_flag: Arc<RwLock<Option<String>>>,
    pub(crate) active_downloads: Arc<RwLock<HashSet<String>>>,
}

impl GigaamEngine {
    /// Create a new GigaAM engine with optional custom models directory.
    pub fn new_with_models_dir(models_dir: Option<PathBuf>) -> Result<Self> {
        let models_dir = if let Some(dir) = models_dir {
            dir.join("gigaam") // GigaAM models in their own subdirectory
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| anyhow!("Failed to get current directory: {}", e))?;

            if cfg!(debug_assertions) {
                current_dir.join("models").join("gigaam")
            } else {
                dirs::data_dir()
                    .or_else(|| dirs::home_dir())
                    .ok_or_else(|| anyhow!("Could not find system data directory"))?
                    .join("Meetily")
                    .join("models")
                    .join("gigaam")
            }
        };

        log::info!("GigaamEngine using models directory: {}", models_dir.display());

        if !models_dir.exists() {
            std::fs::create_dir_all(&models_dir)?;
        }

        Ok(Self {
            models_dir,
            current_model: Arc::new(RwLock::new(None)),
            current_model_name: Arc::new(RwLock::new(None)),
            available_models: Arc::new(RwLock::new(HashMap::new())),
            cancel_download_flag: Arc::new(RwLock::new(None)),
            active_downloads: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Discover available GigaAM models (single CTC variant for now).
    pub async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let models_dir = &self.models_dir;
        let mut models = Vec::new();

        // Single model configuration. Sizes match the actual download:
        //   v3_e2e_rnnt_encoder.int8.onnx ~225 MB
        //   v3_e2e_rnnt_decoder.int8.onnx ~1.2 MB
        //   v3_e2e_rnnt_joint.int8.onnx   ~0.7 MB
        //   v3_e2e_rnnt_vocab.txt         ~13 kB
        //   config.json                   ~135 B
        // The E2E RNN-T variant produces text WITH punctuation and true-case.
        let model_configs = [(
            "gigaam-v3-rnnt-int8",
            227,
            QuantizationType::Int8,
            "Fast (RU)",
            "Лучшее распознавание русского, RNN-T с пунктуацией, int8 (~227 MB)",
        )];

        let active_downloads = self.active_downloads.read().await;

        for (name, size_mb, quantization, speed, description) in model_configs {
            let model_path = models_dir.join(name);

            let status = if active_downloads.contains(name) {
                ModelStatus::Downloading { progress: 0 }
            } else if model_path.exists() {
                // E2E RNN-T needs: encoder, decoder, joint, vocab, and config.
                let required_files = match quantization {
                    QuantizationType::Int8 => vec![
                        "v3_e2e_rnnt_encoder.int8.onnx",
                        "v3_e2e_rnnt_decoder.int8.onnx",
                        "v3_e2e_rnnt_joint.int8.onnx",
                        "v3_e2e_rnnt_vocab.txt",
                        "config.json",
                    ],
                    QuantizationType::FP32 => vec![
                        "v3_e2e_rnnt_encoder.onnx",
                        "v3_e2e_rnnt_decoder.onnx",
                        "v3_e2e_rnnt_joint.onnx",
                        "v3_e2e_rnnt_vocab.txt",
                        "config.json",
                    ],
                };

                let all_files_exist = required_files
                    .iter()
                    .all(|file| model_path.join(file).exists());

                if all_files_exist {
                    match self.validate_model_directory(&model_path).await {
                        Ok(_) => ModelStatus::Available,
                        Err(_) => {
                            log::warn!("Model directory {} appears corrupted", name);
                            let mut total_size = 0u64;
                            for file in required_files {
                                if let Ok(metadata) = std::fs::metadata(model_path.join(file)) {
                                    total_size += metadata.len();
                                }
                            }
                            ModelStatus::Corrupted {
                                file_size: total_size,
                                expected_min_size: (size_mb as u64) * 1024 * 1024,
                            }
                        }
                    }
                } else {
                    ModelStatus::Missing
                }
            } else {
                ModelStatus::Missing
            };

            let model_info = ModelInfo {
                name: name.to_string(),
                path: model_path,
                size_mb: size_mb as u32,
                quantization: quantization.clone(),
                speed: speed.to_string(),
                status,
                description: description.to_string(),
            };

            models.push(model_info);
        }

        // Update internal cache.
        let mut available_models = self.available_models.write().await;
        available_models.clear();
        for model in &models {
            available_models.insert(model.name.clone(), model.clone());
        }

        Ok(models)
    }

    /// Validate the model directory by checking the onnx files are large enough.
    async fn validate_model_directory(&self, model_dir: &PathBuf) -> Result<()> {
        let vocab_path = model_dir.join("v3_e2e_rnnt_vocab.txt");
        if !vocab_path.exists() {
            return Err(anyhow!("v3_e2e_rnnt_vocab.txt not found"));
        }

        let is_int8 = model_dir.join("v3_e2e_rnnt_encoder.int8.onnx").exists();
        let is_fp32 = model_dir.join("v3_e2e_rnnt_encoder.onnx").exists();

        if !is_int8 && !is_fp32 {
            return Err(anyhow!("No ONNX model files found"));
        }

        // Minimum sizes (~90% of expected). The int8 encoder is ~225 MB,
        // decoder ~1.2 MB, joint ~0.7 MB.
        let expected_sizes: Vec<(&str, u64)> = if is_int8 {
            vec![
                ("v3_e2e_rnnt_encoder.int8.onnx", 200_000_000), // ~225 MB
                ("v3_e2e_rnnt_decoder.int8.onnx", 1_000_000),   // ~1.2 MB
                ("v3_e2e_rnnt_joint.int8.onnx", 600_000),        // ~0.7 MB
                ("v3_e2e_rnnt_vocab.txt", 10_000),               // ~13 kB
                ("config.json", 50),                             // 135 B
            ]
        } else {
            vec![
                ("v3_e2e_rnnt_encoder.onnx", 800_000_000), // ~885 MB
                ("v3_e2e_rnnt_decoder.onnx", 4_000_000),   // ~4.6 MB
                ("v3_e2e_rnnt_joint.onnx", 2_400_000),     // ~2.7 MB
                ("v3_e2e_rnnt_vocab.txt", 10_000),
                ("config.json", 50),
            ]
        };

        for (filename, min_size) in expected_sizes {
            let file_path = model_dir.join(filename);
            if !file_path.exists() {
                return Err(anyhow!("{} not found", filename));
            }
            match std::fs::metadata(&file_path) {
                Ok(metadata) => {
                    let actual_size = metadata.len();
                    if actual_size < min_size {
                        return Err(anyhow!(
                            "{} is incomplete: {} bytes (expected at least {} bytes)",
                            filename,
                            actual_size,
                            min_size
                        ));
                    }
                }
                Err(e) => return Err(anyhow!("Failed to read {} metadata: {}", filename, e)),
            }
        }

        Ok(())
    }

    /// Clean incomplete model directory before download.
    async fn clean_incomplete_model_directory(&self, model_dir: &PathBuf) -> Result<()> {
        if !model_dir.exists() {
            return Ok(());
        }

        match self.validate_model_directory(model_dir).await {
            Ok(_) => {
                log::info!("Model directory is valid, no cleanup needed");
                Ok(())
            }
            Err(validation_error) => {
                log::warn!(
                    "Model directory exists but is invalid: {}. Cleaning up...",
                    validation_error
                );

                let mut entries = fs::read_dir(model_dir)
                    .await
                    .map_err(|e| anyhow!("Failed to read model directory: {}", e))?;

                let mut removed_count = 0;
                while let Some(entry) = entries
                    .next_entry()
                    .await
                    .map_err(|e| anyhow!("Failed to read directory entry: {}", e))?
                {
                    let path = entry.path();
                    if path.is_file() {
                        match fs::remove_file(&path).await {
                            Ok(_) => {
                                log::info!("Removed incomplete file: {:?}", path.file_name());
                                removed_count += 1;
                            }
                            Err(e) => log::warn!("Failed to remove file {:?}: {}", path, e),
                        }
                    }
                }

                log::info!("Cleaned {} incomplete files from model directory", removed_count);
                Ok(())
            }
        }
    }

    /// Load a GigaAM model by name.
    pub async fn load_model(&self, model_name: &str) -> Result<()> {
        let models = self.available_models.read().await;
        let model_info = models
            .get(model_name)
            .ok_or_else(|| anyhow!("Model {} not found", model_name))?;

        match model_info.status {
            ModelStatus::Available => {
                if let Some(current_model) = self.current_model_name.read().await.as_ref() {
                    if current_model == model_name {
                        log::info!("GigaAM model {} is already loaded, skipping reload", model_name);
                        return Ok(());
                    }
                    log::info!(
                        "Unloading current GigaAM model '{}' before loading '{}'",
                        current_model,
                        model_name
                    );
                    self.unload_model().await;
                }

                log::info!("Loading GigaAM model: {}", model_name);
                let quantized = model_info.quantization == QuantizationType::Int8;
                let model = GigaamModel::new(&model_info.path, quantized)
                    .map_err(|e| anyhow!("Failed to load GigaAM model {}: {}", model_name, e))?;

                *self.current_model.write().await = Some(model);
                *self.current_model_name.write().await = Some(model_name.to_string());

                log::info!(
                    "Successfully loaded GigaAM model: {} ({})",
                    model_name,
                    if quantized { "Int8 quantized" } else { "FP32" }
                );
                Ok(())
            }
            ModelStatus::Missing => Err(anyhow!("GigaAM model {} is not downloaded", model_name)),
            ModelStatus::Downloading { .. } => {
                Err(anyhow!("GigaAM model {} is currently downloading", model_name))
            }
            ModelStatus::Error(ref err) => {
                Err(anyhow!("GigaAM model {} has error: {}", model_name, err))
            }
            ModelStatus::Corrupted { .. } => {
                Err(anyhow!("GigaAM model {} is corrupted and cannot be loaded", model_name))
            }
        }
    }

    /// Unload the current model.
    pub async fn unload_model(&self) -> bool {
        let mut model_guard = self.current_model.write().await;
        let unloaded = model_guard.take().is_some();
        if unloaded {
            log::info!("GigaAM model unloaded");
        }
        let mut model_name_guard = self.current_model_name.write().await;
        model_name_guard.take();
        unloaded
    }

    /// Get the currently loaded model name.
    pub async fn get_current_model(&self) -> Option<String> {
        self.current_model_name.read().await.clone()
    }

    /// Check if a model is loaded.
    pub async fn is_model_loaded(&self) -> bool {
        self.current_model.read().await.is_some()
    }

    /// Transcribe audio samples using the loaded GigaAM model.
    pub async fn transcribe_audio(&self, audio_data: Vec<f32>) -> Result<String> {
        let mut model_guard = self.current_model.write().await;
        let model = model_guard
            .as_mut()
            .ok_or_else(|| anyhow!("No GigaAM model loaded. Please load a model first."))?;

        let duration_seconds = audio_data.len() as f64 / 16000.0;
        log::debug!(
            "GigaAM transcribing {} samples ({:.1}s duration)",
            audio_data.len(),
            duration_seconds
        );

        let result = model
            .transcribe_samples(audio_data)
            .map_err(|e| anyhow!("GigaAM transcription failed: {}", e))?;

        log::debug!("GigaAM transcription result: '{}'", result.text);
        Ok(result.text)
    }

    /// Get the models directory path.
    pub async fn get_models_directory(&self) -> PathBuf {
        self.models_dir.clone()
    }

    /// Delete a corrupted model.
    pub async fn delete_model(&self, model_name: &str) -> Result<String> {
        log::info!("Attempting to delete GigaAM model: {}", model_name);
        let model_info = {
            let models = self.available_models.read().await;
            models.get(model_name).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow!("GigaAM model '{}' not found", model_name))?;

        log::info!("GigaAM model '{}' has status: {:?}", model_name, model_info.status);

        match &model_info.status {
            ModelStatus::Corrupted { .. } | ModelStatus::Available => {
                if model_info.path.exists() {
                    fs::remove_dir_all(&model_info.path)
                        .await
                        .map_err(|e| {
                            anyhow!("Failed to delete directory '{}': {}", model_info.path.display(), e)
                        })?;
                    log::info!(
                        "Successfully deleted GigaAM model directory: {}",
                        model_info.path.display()
                    );
                } else {
                    log::warn!(
                        "Directory '{}' does not exist, nothing to delete",
                        model_info.path.display()
                    );
                }

                {
                    let mut models = self.available_models.write().await;
                    if let Some(model) = models.get_mut(model_name) {
                        model.status = ModelStatus::Missing;
                    }
                }

                Ok(format!("Successfully deleted GigaAM model '{}'", model_name))
            }
            _ => Err(anyhow!(
                "Can only delete corrupted or available GigaAM models. Model '{}' has status: {:?}",
                model_name,
                model_info.status
            )),
        }
    }

    /// Download a GigaAM model from HuggingFace.
    pub async fn download_model(
        &self,
        model_name: &str,
        progress_callback: Option<Box<dyn Fn(u8) + Send>>,
    ) -> Result<()> {
        let detailed_callback: Option<Box<dyn Fn(DownloadProgress) + Send>> =
            progress_callback.map(|cb| {
                Box::new(move |p: DownloadProgress| cb(p.percent))
                    as Box<dyn Fn(DownloadProgress) + Send>
            });
        self.download_model_detailed(model_name, detailed_callback).await
    }

    /// Download a GigaAM model with detailed progress (resume support).
    pub async fn download_model_detailed(
        &self,
        model_name: &str,
        progress_callback: Option<Box<dyn Fn(DownloadProgress) + Send>>,
    ) -> Result<()> {
        log::info!("Starting download for GigaAM model: {}", model_name);

        // Prevent concurrent downloads of the same model.
        {
            let active = self.active_downloads.read().await;
            if active.contains(model_name) {
                log::warn!("Download already in progress for GigaAM model: {}", model_name);
                return Err(anyhow!("Download already in progress for model: {}", model_name));
            }
        }
        {
            let mut active = self.active_downloads.write().await;
            active.insert(model_name.to_string());
        }
        {
            let mut cancel_flag = self.cancel_download_flag.write().await;
            *cancel_flag = None;
        }

        let model_info = {
            let models = self.available_models.read().await;
            match models.get(model_name).cloned() {
                Some(info) => info,
                None => {
                    let mut active = self.active_downloads.write().await;
                    active.remove(model_name);
                    return Err(anyhow!("Model {} not found", model_name));
                }
            }
        };

        // Mark downloading.
        {
            let mut models = self.available_models.write().await;
            if let Some(model) = models.get_mut(model_name) {
                model.status = ModelStatus::Downloading { progress: 0 };
            }
        }

        // HuggingFace base URL for GigaAM-v3 ONNX models.
        let base_url = "https://huggingface.co/istupakov/gigaam-v3-onnx/resolve/main";

        // Files to download for the int8 E2E RNN-T variant.
        let files_to_download = match model_info.quantization {
            QuantizationType::Int8 => vec![
                "v3_e2e_rnnt_encoder.int8.onnx",
                "v3_e2e_rnnt_decoder.int8.onnx",
                "v3_e2e_rnnt_joint.int8.onnx",
                "v3_e2e_rnnt_vocab.txt",
                "config.json",
            ],
            QuantizationType::FP32 => vec![
                "v3_e2e_rnnt_encoder.onnx",
                "v3_e2e_rnnt_decoder.onnx",
                "v3_e2e_rnnt_joint.onnx",
                "v3_e2e_rnnt_vocab.txt",
                "config.json",
            ],
        };

        let model_dir = &model_info.path;
        if !model_dir.exists() {
            if let Err(e) = fs::create_dir_all(model_dir).await {
                let mut active = self.active_downloads.write().await;
                active.remove(model_name);
                return Err(anyhow!("Failed to create model directory: {}", e));
            }
        }

        log::info!("Checking for incomplete model files to clean up...");
        if let Err(e) = self.clean_incomplete_model_directory(model_dir).await {
            log::warn!("Failed to clean incomplete model directory: {}", e);
        }

        let client = reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(1)
            .timeout(Duration::from_secs(3600))
            .connect_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        let total_files = files_to_download.len();

        // Approximate per-file sizes for weighted progress.
        let file_sizes: std::collections::HashMap<&str, u64> = match model_info.quantization {
            QuantizationType::Int8 => [
                ("v3_e2e_rnnt_encoder.int8.onnx", 225_000_000u64),
                ("v3_e2e_rnnt_decoder.int8.onnx", 1_200_000u64),
                ("v3_e2e_rnnt_joint.int8.onnx", 700_000u64),
                ("v3_e2e_rnnt_vocab.txt", 13_400u64),
                ("config.json", 135u64),
            ]
            .iter()
            .cloned()
            .collect(),
            QuantizationType::FP32 => [
                ("v3_e2e_rnnt_encoder.onnx", 885_000_000u64),
                ("v3_e2e_rnnt_decoder.onnx", 4_600_000u64),
                ("v3_e2e_rnnt_joint.onnx", 2_710_000u64),
                ("v3_e2e_rnnt_vocab.txt", 13_400u64),
                ("config.json", 135u64),
            ]
            .iter()
            .cloned()
            .collect(),
        };

        let total_size_bytes: u64 = files_to_download
            .iter()
            .filter_map(|f| file_sizes.get(*f))
            .copied()
            .sum();

        // Account for already-downloaded bytes (resume).
        let mut already_downloaded: u64 = 0;
        for filename in &files_to_download {
            let file_path = model_dir.join(filename);
            if file_path.exists() {
                if let Ok(metadata) = fs::metadata(&file_path).await {
                    let file_size = metadata.len();
                    let expected_size = file_sizes.get(*filename).copied().unwrap_or(0);
                    already_downloaded += file_size.min(expected_size);
                }
            }
        }

        let mut total_downloaded: u64 = already_downloaded;
        let download_start_time = Instant::now();
        let mut last_report_time = Instant::now();
        let mut bytes_since_last_report: u64 = 0;
        let mut last_reported_progress: u8 = 0;

        log::info!(
            "Starting weighted download for {} files, total size: {:.2} MB (already downloaded: {:.2} MB)",
            total_files,
            total_size_bytes as f64 / 1_048_576.0,
            already_downloaded as f64 / 1_048_576.0
        );

        for (index, filename) in files_to_download.iter().enumerate() {
            let file_url = format!("{}/{}", base_url, filename);
            let file_path = model_dir.join(filename);

            let existing_size: u64 = if file_path.exists() {
                fs::metadata(&file_path).await.map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };
            let expected_size = file_sizes.get(*filename).copied().unwrap_or(0);

            // Skip if file is already complete (1% tolerance).
            let size_tolerance = (expected_size as f64 * 0.99) as u64;
            if existing_size >= size_tolerance && expected_size > 0 {
                log::info!(
                    "Skipping complete file: {} ({:.2} MB, expected: {:.2} MB)",
                    filename,
                    existing_size as f64 / 1_048_576.0,
                    expected_size as f64 / 1_048_576.0
                );
                continue;
            }

            log::info!(
                "Downloading file {}/{}: {} (resuming from {} bytes)",
                index + 1,
                total_files,
                filename,
                existing_size
            );

            let mut request = client.get(&file_url);
            if existing_size > 0 {
                request = request.header("Range", format!("bytes={}-", existing_size));
                log::info!("Resuming download from byte {}", existing_size);
            }

            let mut response = request.send().await.map_err(|e| {
                anyhow!("Failed to start download for {}: {}", filename, e)
            })?;

            let (file_total_size, resuming) = if response.status()
                == reqwest::StatusCode::PARTIAL_CONTENT
            {
                let remaining = response.content_length().unwrap_or(0);
                log::info!("Server supports resume, remaining: {} bytes", remaining);
                (existing_size + remaining, true)
            } else if response.status().is_success() {
                if existing_size > 0 {
                    log::warn!(
                        "Server doesn't support resume for {}, starting fresh download",
                        filename
                    );
                }
                (response.content_length().unwrap_or(0), false)
            } else if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                log::warn!("Server returned 416 Range Not Satisfiable for {}", filename);
                let size_tolerance = (expected_size as f64 * 0.99) as u64;
                if existing_size >= size_tolerance && expected_size > 0 {
                    log::info!(
                        "File {} complete ({} bytes). Skipping.",
                        filename,
                        existing_size
                    );
                    continue;
                } else {
                    log::warn!(
                        "File {} incomplete ({}/{} bytes). Deleting and retrying.",
                        filename,
                        existing_size,
                        expected_size
                    );
                    if let Err(e) = fs::remove_file(&file_path).await {
                        let mut active = self.active_downloads.write().await;
                        active.remove(model_name);
                        return Err(anyhow!(
                            "Failed to delete incomplete file {}: {}",
                            filename,
                            e
                        ));
                    }
                    log::info!("Retrying {} without resume", filename);
                    response = client
                        .get(&file_url)
                        .send()
                        .await
                        .map_err(|e| anyhow!("Retry failed for {}: {}", filename, e))?;
                    if !response.status().is_success() {
                        let mut active = self.active_downloads.write().await;
                        active.remove(model_name);
                        return Err(anyhow!(
                            "Retry failed for {} with status: {}",
                            filename,
                            response.status()
                        ));
                    }
                    (response.content_length().unwrap_or(0), false)
                }
            } else {
                let mut active = self.active_downloads.write().await;
                active.remove(model_name);
                return Err(anyhow!(
                    "Download failed for {} with status: {}",
                    filename,
                    response.status()
                ));
            };

            let file = if resuming {
                fs::OpenOptions::new()
                    .append(true)
                    .open(&file_path)
                    .await
                    .map_err(|e| anyhow!("Failed to open file for resume {}: {}", filename, e))?
            } else {
                fs::File::create(&file_path)
                    .await
                    .map_err(|e| anyhow!("Failed to create file {}: {}", filename, e))?
            };

            let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);

            use futures_util::StreamExt;
            let mut stream = response.bytes_stream();
            let mut file_downloaded = if resuming { existing_size } else { 0u64 };

            loop {
                // Check for cancellation.
                {
                    let cancel_flag = self.cancel_download_flag.read().await;
                    if cancel_flag.as_ref() == Some(&model_name.to_string()) {
                        log::info!("Download cancelled for {}", model_name);
                        let _ = writer.flush().await;
                        drop(writer);
                        let mut active = self.active_downloads.write().await;
                        active.remove(model_name);
                        return Err(anyhow!("Download cancelled by user"));
                    }
                }

                let next_result = timeout(Duration::from_secs(30), stream.next()).await;
                let chunk = match next_result {
                    Err(_) => {
                        log::warn!(
                            "Download timeout for {}: no data received for 30 seconds",
                            model_name
                        );
                        let _ = writer.flush().await;
                        {
                            let mut active = self.active_downloads.write().await;
                            active.remove(model_name);
                        }
                        {
                            let mut models = self.available_models.write().await;
                            if let Some(model) = models.get_mut(model_name) {
                                model.status = ModelStatus::Missing;
                            }
                        }
                        return Err(anyhow!("Download timeout - No data received for 30 seconds"));
                    }
                    Ok(None) => break,
                    Ok(Some(chunk_result)) => match chunk_result {
                        Ok(c) => c,
                        Err(e) => {
                            log::error!("Download error for {}: {:?}", model_name, e);
                            let _ = writer.flush().await;
                            {
                                let mut active = self.active_downloads.write().await;
                                active.remove(model_name);
                            }
                            {
                                let mut models = self.available_models.write().await;
                                if let Some(model) = models.get_mut(model_name) {
                                    model.status = ModelStatus::Missing;
                                }
                            }
                            let error_msg = if e.is_timeout() {
                                "Connection timeout - Check your internet"
                            } else if e.is_connect() {
                                "Connection failed - Check your internet"
                            } else if e.is_body() {
                                "Stream interrupted - Network unstable"
                            } else {
                                "Download error"
                            };
                            return Err(anyhow!("{}: {}", error_msg, e));
                        }
                    },
                };

                if let Err(e) = writer.write_all(&chunk).await {
                    {
                        let mut active = self.active_downloads.write().await;
                        active.remove(model_name);
                    }
                    {
                        let mut models = self.available_models.write().await;
                        if let Some(model) = models.get_mut(model_name) {
                            model.status = ModelStatus::Missing;
                        }
                    }
                    return Err(anyhow!("Failed to write chunk to file: {}", e));
                }

                let chunk_len = chunk.len() as u64;
                file_downloaded += chunk_len;
                total_downloaded += chunk_len;
                bytes_since_last_report += chunk_len;

                let overall_progress = if total_size_bytes > 0 {
                    ((total_downloaded as f64 / total_size_bytes as f64) * 100.0).min(99.0) as u8
                } else {
                    ((index as f64 + (file_downloaded as f64 / file_total_size.max(1) as f64))
                        / total_files as f64
                        * 100.0) as u8
                };

                let elapsed_since_report = last_report_time.elapsed();
                let progress_changed = overall_progress > last_reported_progress;
                let time_threshold = elapsed_since_report >= Duration::from_millis(500);
                let is_complete = file_downloaded >= file_total_size;
                let should_report = progress_changed || time_threshold || is_complete;

                if should_report {
                    let speed_mbps = if elapsed_since_report.as_secs_f64() >= 0.1 {
                        (bytes_since_last_report as f64 / (1024.0 * 1024.0))
                            / elapsed_since_report.as_secs_f64()
                    } else {
                        let total_elapsed = download_start_time.elapsed().as_secs_f64();
                        if total_elapsed > 0.0 {
                            ((total_downloaded - already_downloaded) as f64 / (1024.0 * 1024.0))
                                / total_elapsed
                        } else {
                            0.0
                        }
                    };

                    last_reported_progress = overall_progress;
                    last_report_time = Instant::now();
                    bytes_since_last_report = 0;

                    let progress = DownloadProgress::new(total_downloaded, total_size_bytes, speed_mbps);
                    if let Some(ref callback) = progress_callback {
                        callback(progress);
                    }
                    {
                        let mut models = self.available_models.write().await;
                        if let Some(model) = models.get_mut(model_name) {
                            model.status = ModelStatus::Downloading { progress: overall_progress };
                        }
                    }
                }
            }

            if let Err(e) = writer.flush().await {
                {
                    let mut active = self.active_downloads.write().await;
                    active.remove(model_name);
                }
                {
                    let mut models = self.available_models.write().await;
                    if let Some(model) = models.get_mut(model_name) {
                        model.status = ModelStatus::Missing;
                    }
                }
                return Err(anyhow!("Failed to flush file {}: {}", filename, e));
            }

            log::info!(
                "Completed download: {} ({:.2} MB, overall progress: {:.1}%)",
                filename,
                file_downloaded as f64 / 1_048_576.0,
                (total_downloaded as f64 / total_size_bytes as f64) * 100.0
            );
        }

        // Final 100% report.
        let total_elapsed = download_start_time.elapsed().as_secs_f64();
        let final_speed = if total_elapsed > 0.0 {
            ((total_downloaded - already_downloaded) as f64 / (1024.0 * 1024.0)) / total_elapsed
        } else {
            0.0
        };
        let final_progress = DownloadProgress::new(total_size_bytes, total_size_bytes, final_speed);
        if let Some(ref callback) = progress_callback {
            callback(final_progress);
        }

        {
            let mut models = self.available_models.write().await;
            if let Some(model) = models.get_mut(model_name) {
                model.status = ModelStatus::Available;
                model.path = model_dir.clone();
            }
        }
        {
            let mut active = self.active_downloads.write().await;
            active.remove(model_name);
        }
        {
            let mut cancel_flag = self.cancel_download_flag.write().await;
            if cancel_flag.as_ref() == Some(&model_name.to_string()) {
                *cancel_flag = None;
            }
        }

        log::info!("Download completed for GigaAM model: {}", model_name);
        Ok(())
    }

    /// Cancel an ongoing model download.
    pub async fn cancel_download(&self, model_name: &str) -> Result<()> {
        log::info!("Cancelling download for GigaAM model: {}", model_name);
        {
            let mut cancel_flag = self.cancel_download_flag.write().await;
            *cancel_flag = Some(model_name.to_string());
        }
        {
            let mut active = self.active_downloads.write().await;
            active.remove(model_name);
        }
        {
            let mut models = self.available_models.write().await;
            if let Some(model) = models.get_mut(model_name) {
                model.status = ModelStatus::Missing;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let model_path = self.models_dir.join(model_name);
        if model_path.exists() {
            if let Err(e) = fs::remove_dir_all(&model_path).await {
                log::warn!("Failed to clean up cancelled download directory: {}", e);
            } else {
                log::info!("Cleaned up cancelled download directory: {}", model_path.display());
            }
        }
        Ok(())
    }
}
