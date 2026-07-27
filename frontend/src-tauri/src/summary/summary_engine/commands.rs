// Tauri commands for built-in AI model management
// Exposes model download, status, and management functionality to frontend

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::Mutex;

use crate::audio::hardware_detector::{GpuType, HardwareProfile};
use super::model_manager::{DownloadProgress, ModelInfo, ModelManager};

const QWEN35_4B_RECOMMENDED_RAM_GB: u64 = 14;
/// VRAM (GB) above which the larger model is recommended even on a machine
/// with modest RAM, because GPU offload makes it fit comfortably.
///
/// `#[allow(dead_code)]`: intended for the GPU-offload recommendation branch
/// (sibling to `QWEN35_4B_RECOMMENDED_RAM_GB`), not yet wired in. Keep until
/// the hardware-aware recommendation path lands.
#[allow(dead_code)]
const QWEN35_4B_RECOMMENDED_VRAM_GB: f32 = 6.0;

pub(crate) fn summary_model_priority(model_name: &str) -> u8 {
    match model_name {
        "qwen3.5:4b" => 5,
        // RuadaptQwen3-4B sits just below base Qwen 3.5 4B until A/B testing
        // confirms it should become the default; bump to 6 if it wins.
        "ruadapt-qwen3:4b" => 4,
        "qwen3.5:2b" => 3,
        "gemma3:1b" => 1,
        _ => 0,
    }
}

pub fn recommend_summary_model(_is_macos: bool, system_ram_gb: u64) -> &'static str {
    if system_ram_gb >= QWEN35_4B_RECOMMENDED_RAM_GB {
        "qwen3.5:4b"
    } else {
        "qwen3.5:2b"
    }
}

// ============================================================================
// Inference decision (pure, unit-testable)
// ============================================================================

/// User-facing inference plan derived from hardware + user preference.
/// Returned by `api_get_hardware_profile` and consumed by the onboarding UI.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InferencePlan {
    /// "CPU" | "GPU (Vulkan)" | "GPU (Metal)" | "GPU (CUDA)"
    pub inference_mode: String,
    /// True when the sidecar will run with n_gpu_layers=0.
    pub force_cpu: bool,
    /// Short human-readable reason ("GPU detected, user preference enabled").
    pub reason: String,
}

/// Pure decision function: given a hardware profile and the user's GPU-toggle
/// preference, return the inference plan. No I/O — fully unit-testable.
///
/// Rules:
/// - No GPU detected → CPU, force_cpu=true.
/// - GPU detected but user disabled → CPU, force_cpu=true.
/// - GPU detected and enabled → GPU mode, force_cpu=false.
pub fn decide_inference(profile: &HardwareProfile, use_gpu: bool) -> InferencePlan {
    if !profile.has_gpu_acceleration {
        return InferencePlan {
            inference_mode: "CPU".to_string(),
            force_cpu: true,
            reason: "GPU не обнаружен — инференс на CPU".to_string(),
        };
    }
    if !use_gpu {
        return InferencePlan {
            inference_mode: "CPU".to_string(),
            force_cpu: true,
            reason: "GPU отключён пользователем".to_string(),
        };
    }
    let mode = match profile.gpu_type {
        GpuType::Metal => "GPU (Metal)",
        GpuType::Cuda => "GPU (CUDA)",
        GpuType::Vulkan => "GPU (Vulkan)",
        GpuType::OpenCL => "GPU (OpenCL)",
        GpuType::None => "CPU",
    };
    InferencePlan {
        inference_mode: mode.to_string(),
        force_cpu: false,
        reason: format!("{} обнаружен, GPU включён", mode),
    }
}


pub(crate) fn get_recommended_summary_model_for_current_system() -> Result<&'static str, String> {
    let system_ram_gb = get_system_ram_gb()?;
    let is_macos = cfg!(target_os = "macos");

    log::info!(
        "System RAM detected: {} GB, Platform: {}",
        system_ram_gb,
        if is_macos { "macOS" } else { "other" }
    );

    Ok(recommend_summary_model(is_macos, system_ram_gb))
}

// ============================================================================
// Global State
// ============================================================================

/// Global model manager instance
pub struct ModelManagerState(pub Arc<Mutex<Option<Arc<ModelManager>>>>);

/// Initialize the model manager
pub async fn init_model_manager<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let models_dir = app.path().app_data_dir()?.join("models").join("summary");

    let manager = ModelManager::new_with_models_dir(Some(models_dir))?;
    manager.init().await?;

    let state: State<ModelManagerState> = app.state();
    let mut manager_lock = state.0.lock().await;
    *manager_lock = Some(Arc::new(manager));

    log::info!("Built-in AI model manager initialized");
    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// List all available built-in AI models with their status
#[tauri::command]
pub async fn builtin_ai_list_models<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ModelManagerState>,
) -> Result<Vec<ModelInfo>, String> {
    let manager = {
        // Ensure manager is initialized
        {
            let manager_lock = state.0.lock().await;
            if manager_lock.is_none() {
                drop(manager_lock);
                init_model_manager(&app)
                    .await
                    .map_err(|e| format!("Failed to initialize model manager: {}", e))?;
            }
        }

        let manager_lock = state.0.lock().await;
        manager_lock
            .as_ref()
            .ok_or_else(|| "Model manager not initialized".to_string())?
            .clone()
    };

    let models = manager.list_models().await;
    Ok(models)
}

/// Get information about a specific model
#[tauri::command]
pub async fn builtin_ai_get_model_info<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ModelManagerState>,
    model_name: String,
) -> Result<Option<ModelInfo>, String> {
    let manager = {
        // Ensure manager is initialized
        {
            let manager_lock = state.0.lock().await;
            if manager_lock.is_none() {
                drop(manager_lock);
                init_model_manager(&app)
                    .await
                    .map_err(|e| format!("Failed to initialize model manager: {}", e))?;
            }
        }

        let manager_lock = state.0.lock().await;
        manager_lock
            .as_ref()
            .ok_or_else(|| "Model manager not initialized".to_string())?
            .clone()
    };

    let info = manager.get_model_info(&model_name).await;
    Ok(info)
}

/// Download a built-in AI model with progress updates
#[tauri::command]
pub async fn builtin_ai_download_model<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ModelManagerState>,
    model_name: String,
) -> Result<(), String> {
    let manager = {
        // Ensure manager is initialized
        {
            let manager_lock = state.0.lock().await;
            if manager_lock.is_none() {
                drop(manager_lock);
                init_model_manager(&app)
                    .await
                    .map_err(|e| format!("Failed to initialize model manager: {}", e))?;
            }
        }

        let manager_lock = state.0.lock().await;
        manager_lock
            .as_ref()
            .ok_or_else(|| "Model manager not initialized".to_string())?
            .clone() // Clone the Arc, not the ModelManager
    };
    // IMPORTANT: Only emit "downloading" status here, never "completed"
    // Completion event is emitted AFTER download task fully finishes (validation, etc.)
    let app_clone = app.clone();
    let model_name_clone = model_name.clone();
    let progress_callback = Box::new(move |progress: DownloadProgress| {
        let _ = app_clone.emit(
            "builtin-ai-download-progress",
            serde_json::json!({
                "model": model_name_clone,
                "progress": progress.percent,
                "downloaded_mb": progress.downloaded_mb,
                "total_mb": progress.total_mb,
                "speed_mbps": progress.speed_mbps,
                "status": "downloading"  // Always "downloading", never "completed" from progress callback
            }),
        );
    });

    match manager
        .download_model_detailed(&model_name, Some(progress_callback))
        .await
    {
        Ok(_) => {
            // Download task completed successfully (validation passed, status set to Available)
            let _ = app.emit(
                "builtin-ai-download-progress",
                serde_json::json!({
                    "model": model_name,
                    "progress": 100,
                    "downloaded_mb": 0,  // Not used by completion handler
                    "total_mb": 0,       // Not used by completion handler
                    "speed_mbps": 0,     // Not used by completion handler
                    "status": "completed"
                }),
            );
            Ok(())
        },
        Err(e) => {
            let error_msg = e.to_string();

            // Check if this is a cancellation error (marked with "CANCELLED:" prefix)
            // Don't emit error event for cancellations - cancel command already emits cancelled event
            if !error_msg.starts_with("CANCELLED:") {
                // Emit error via progress event for frontend to display (only for real errors)
                let _ = app.emit(
                    "builtin-ai-download-progress",
                    serde_json::json!({
                        "model": model_name,
                        "progress": 0,
                        "downloaded_mb": 0,
                        "total_mb": 0,
                        "speed_mbps": 0,
                        "status": "error",
                        "error": error_msg
                    }),
                );
            }
            Err(error_msg)
        }
    }
}

/// Cancel an ongoing model download
#[tauri::command]
pub async fn builtin_ai_cancel_download<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ModelManagerState>,
    model_name: String,
) -> Result<(), String> {
    let manager = {
        let manager_lock = state.0.lock().await;
        manager_lock
            .as_ref()
            .ok_or_else(|| "Model manager not initialized".to_string())?
            .clone()
    };

    manager
        .cancel_download(&model_name)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit(
        "builtin-ai-download-progress",
        serde_json::json!({
            "model": model_name,
            "progress": 0,
            "status": "cancelled"
        }),
    );

    Ok(())
}

/// Delete a corrupted or available model file
#[tauri::command]
pub async fn builtin_ai_delete_model(
    state: State<'_, ModelManagerState>,
    model_name: String,
) -> Result<(), String> {
    let manager = {
        let manager_lock = state.0.lock().await;
        manager_lock
            .as_ref()
            .ok_or_else(|| "Model manager not initialized".to_string())?
            .clone()
    };

    manager
        .delete_model(&model_name)
        .await
        .map_err(|e| e.to_string())
}

/// Check if a model is ready to use
#[tauri::command]
pub async fn builtin_ai_is_model_ready<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ModelManagerState>,
    model_name: String,
    refresh: Option<bool>,  // NEW: Optional refresh parameter
) -> Result<bool, String> {
    let manager = {
        // Ensure manager is initialized
        {
            let manager_lock = state.0.lock().await;
            if manager_lock.is_none() {
                drop(manager_lock);
                init_model_manager(&app)
                    .await
                    .map_err(|e| format!("Failed to initialize model manager: {}", e))?;
            }
        }

        let manager_lock = state.0.lock().await;
        manager_lock
            .as_ref()
            .ok_or_else(|| "Model manager not initialized".to_string())?
            .clone()
    };

    let refresh_scan = refresh.unwrap_or(false);
    let ready = manager.is_model_ready(&model_name, refresh_scan).await;

    log::info!(
        "Model '{}' ready check (refresh={}): {}",
        model_name,
        refresh_scan,
        ready
    );

    Ok(ready)
}

/// Check if any summary model is available (for onboarding)
/// Returns the first available model name by priority, or None if no models exist
#[tauri::command]
pub async fn builtin_ai_get_available_summary_model<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ModelManagerState>,
) -> Result<Option<String>, String> {
    let manager = {
        // Ensure manager is initialized
        {
            let manager_lock = state.0.lock().await;
            if manager_lock.is_none() {
                drop(manager_lock);
                init_model_manager(&app)
                    .await
                    .map_err(|e| format!("Failed to initialize model manager: {}", e))?;
            }
        }

        let manager_lock = state.0.lock().await;
        manager_lock
            .as_ref()
            .ok_or_else(|| "Model manager not initialized".to_string())?
            .clone()
    };

    // Force fresh scan to ensure accurate state
    manager
        .scan_models()
        .await
        .map_err(|e| format!("Failed to scan models: {}", e))?;

    // Get all available models
    let all_models = manager.list_models().await;

    // Find first available summary model
    let available = all_models
        .iter()
        .filter(|m| matches!(m.status, crate::summary::summary_engine::model_manager::ModelStatus::Available))
        .max_by_key(|m| summary_model_priority(&m.name))
        .map(|m| m.name.clone());

    log::info!("Available summary model check: {:?}", available);
    Ok(available)
}

// ============================================================================
// Startup Initialization & Utility Commands
// ============================================================================

pub async fn init_model_manager_at_startup<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), String> {
    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("models")
        .join("summary");

    let manager = ModelManager::new_with_models_dir(Some(models_dir))
        .map_err(|e| format!("Failed to create ModelManager: {}", e))?;

    manager
        .init()
        .await
        .map_err(|e| format!("Failed to initialize ModelManager: {}", e))?;

    let state: State<ModelManagerState> = app.state();
    let mut manager_lock = state.0.lock().await;
    *manager_lock = Some(Arc::new(manager));

    log::info!("ModelManager initialized at startup");
    Ok(())
}


/// Get recommended summary model based on platform and system RAM.
/// macOS → qwen3.5:4b
/// non-macOS + <8GB RAM → qwen3.5:2b
/// non-macOS + >=8GB RAM → qwen3.5:4b
#[tauri::command]
pub async fn builtin_ai_get_recommended_model() -> Result<String, String> {
    let recommended = get_recommended_summary_model_for_current_system()?;

    log::info!("Recommended summary model: {}", recommended);
    Ok(recommended.to_string())
}

// ============================================================================
// Hardware profile (onboarding + settings UI)
// ============================================================================

/// Hardware profile exposed to the frontend. Combines the cached
/// `HardwareProfile` detection with the recommended model and the inferred
/// inference plan so the onboarding/settings UI can render everything in one
/// invoke call.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileInfo {
    pub cpu_cores: u8,
    pub memory_gb: u8,
    /// "none" | "metal" | "cuda" | "vulkan" | "opencl"
    pub gpu_type: String,
    pub gpu_name: Option<String>,
    pub gpu_vram_gb: Option<f32>,
    pub vulkan_available: bool,
    /// "low" | "medium" | "high" | "ultra"
    pub performance_tier: String,
    pub recommended_model: String,
    /// User-facing inference mode ("CPU" | "GPU (Vulkan)" | ...) assuming the
    /// default preference (GPU on when available).
    pub recommended_inference_mode: String,
    pub has_gpu: bool,
}

fn gpu_type_str(t: GpuType) -> &'static str {
    match t {
        GpuType::None => "none",
        GpuType::Metal => "metal",
        GpuType::Cuda => "cuda",
        GpuType::Vulkan => "vulkan",
        GpuType::OpenCL => "opencl",
    }
}

fn tier_str(
    t: crate::audio::hardware_detector::PerformanceTier,
) -> &'static str {
    use crate::audio::hardware_detector::PerformanceTier;
    match t {
        PerformanceTier::Low => "low",
        PerformanceTier::Medium => "medium",
        PerformanceTier::High => "high",
        PerformanceTier::Ultra => "ultra",
    }
}

/// Returns the detected hardware profile + recommended model + recommended
/// inference mode. Called by the onboarding "Setup overview" step and the
/// settings GPU-toggle card.
#[tauri::command]
pub async fn api_get_hardware_profile() -> Result<HardwareProfileInfo, String> {
    let profile = HardwareProfile::detect();
    let recommended_model = get_recommended_summary_model_for_current_system()?;
    // Onboarding shows the *recommended* (default) plan: GPU on when available.
    let plan = decide_inference(profile, true);

    Ok(HardwareProfileInfo {
        cpu_cores: profile.cpu_cores,
        memory_gb: profile.memory_gb,
        gpu_type: gpu_type_str(profile.gpu_type).to_string(),
        gpu_name: profile.gpu_name.clone(),
        gpu_vram_gb: profile.gpu_vram_gb,
        vulkan_available: profile.vulkan_available,
        performance_tier: tier_str(profile.performance_tier).to_string(),
        recommended_model: recommended_model.to_string(),
        recommended_inference_mode: plan.inference_mode,
        has_gpu: profile.has_gpu_acceleration,
    })
}


/// Get total system RAM in gigabytes
fn get_system_ram_gb() -> Result<u64, String> {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_memory();

    let total_memory_bytes = sys.total_memory();
    let total_memory_gb = total_memory_bytes / (1024 * 1024 * 1024);

    Ok(total_memory_gb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_summary_model_uses_qwen2b_below_effective_16gb_floor() {
        assert_eq!(recommend_summary_model(true, 13), "qwen3.5:2b");
        assert_eq!(recommend_summary_model(false, 13), "qwen3.5:2b");
    }

    #[test]
    fn recommended_summary_model_uses_qwen4b_at_effective_16gb_floor() {
        assert_eq!(recommend_summary_model(true, 14), "qwen3.5:4b");
        assert_eq!(recommend_summary_model(false, 14), "qwen3.5:4b");
    }

    #[test]
    fn available_summary_model_priority_ranks_models_correctly() {
        // Base Qwen 3.5 4B stays top until Ruadapt proves itself in A/B testing.
        assert_eq!(summary_model_priority("qwen3.5:4b"), 5);
        assert_eq!(summary_model_priority("ruadapt-qwen3:4b"), 4);
        assert_eq!(summary_model_priority("qwen3.5:2b"), 3);
        assert_eq!(summary_model_priority("gemma3:1b"), 1);
        // Removed-from-catalog gemma3:4b falls back to 0 (graceful degradation).
        assert_eq!(summary_model_priority("gemma3:4b"), 0);
        // Ordering invariant: base 4B > ruadapt 4B > 2B > 1B.
        assert!(summary_model_priority("qwen3.5:4b") > summary_model_priority("ruadapt-qwen3:4b"));
        assert!(summary_model_priority("ruadapt-qwen3:4b") > summary_model_priority("qwen3.5:2b"));
        assert!(summary_model_priority("qwen3.5:2b") > summary_model_priority("gemma3:1b"));
    }

    // ---- decide_inference (pure) -------------------------------------------

    use crate::audio::hardware_detector::{
        GpuType, HardwareProfile, PerformanceTier,
    };

    fn profile(gpu: GpuType) -> HardwareProfile {
        HardwareProfile {
            cpu_cores: 8,
            has_gpu_acceleration: !matches!(gpu, GpuType::None),
            gpu_type: gpu,
            memory_gb: 16,
            performance_tier: PerformanceTier::High,
            gpu_name: None,
            gpu_vram_gb: None,
            vulkan_available: !matches!(gpu, GpuType::None | GpuType::Metal),
        }
    }

    #[test]
    fn decide_inference_no_gpu_forces_cpu() {
        let p = profile(GpuType::None);
        let plan = decide_inference(&p, true);
        assert_eq!(plan.inference_mode, "CPU");
        assert!(plan.force_cpu);
    }

    #[test]
    fn decide_inference_gpu_but_user_disabled_forces_cpu() {
        let p = profile(GpuType::Vulkan);
        let plan = decide_inference(&p, false);
        assert_eq!(plan.inference_mode, "CPU");
        assert!(plan.force_cpu);
    }

    #[test]
    fn decide_inference_vulkan_gpu_enabled() {
        let p = profile(GpuType::Vulkan);
        let plan = decide_inference(&p, true);
        assert_eq!(plan.inference_mode, "GPU (Vulkan)");
        assert!(!plan.force_cpu);
    }

    #[test]
    fn decide_inference_metal_gpu_enabled() {
        let p = profile(GpuType::Metal);
        let plan = decide_inference(&p, true);
        assert_eq!(plan.inference_mode, "GPU (Metal)");
        assert!(!plan.force_cpu);
    }
}
