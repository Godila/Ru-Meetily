//! Parallel processing commands — STUB.
//!
//! The real parallel whisper processor (`parallel_processor.rs`,
//! `system_monitor.rs`) was part of the whisper engine and is removed in this
//! build. These commands exist solely so `lib.rs`'s `generate_handler!`
//! registration still compiles. Each returns an error / empty status.

use serde::Serialize;
use tauri::{command, State};

/// Placeholder managed state. The original held a `ParallelProcessor` and a
/// `SystemMonitor`; here it is empty but still `Send + Sync + 'static` so it
/// can be registered via `app.manage(...)`.
#[derive(Default)]
pub struct ParallelProcessorState;

impl ParallelProcessorState {
    pub fn new() -> Self {
        Self
    }
}

/// Status payload returned by `get_parallel_processing_status`. Minimal shape
/// (always "not running") so the frontend renders an idle state.
#[derive(Debug, Serialize)]
pub struct ProcessingStatus {
    pub running: bool,
    pub workers: usize,
    pub processed: usize,
    pub total: usize,
}

const UNAVAILABLE: &str = "Parallel processing is not available in this build (whisper engine disabled).";

#[command]
pub async fn initialize_parallel_processor(
    _state: State<'_, ParallelProcessorState>,
    _max_workers: Option<usize>,
) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn start_parallel_processing(
    _state: State<'_, ParallelProcessorState>,
    _audio_chunks: Vec<serde_json::Value>,
) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn pause_parallel_processing(
    _state: State<'_, ParallelProcessorState>,
) -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn resume_parallel_processing(
    _state: State<'_, ParallelProcessorState>,
) -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn stop_parallel_processing(
    _state: State<'_, ParallelProcessorState>,
) -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn get_parallel_processing_status(
    _state: State<'_, ParallelProcessorState>,
) -> Result<ProcessingStatus, String> {
    Ok(ProcessingStatus {
        running: false,
        workers: 0,
        processed: 0,
        total: 0,
    })
}

#[command]
pub async fn get_system_resources(
    _state: State<'_, ParallelProcessorState>,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "available": false,
        "reason": "whisper engine disabled in this build"
    }))
}

#[command]
pub async fn check_resource_constraints(
    _state: State<'_, ParallelProcessorState>,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({ "ok": false, "reason": UNAVAILABLE }))
}

#[command]
pub async fn calculate_optimal_workers(
    _state: State<'_, ParallelProcessorState>,
) -> Result<usize, String> {
    Ok(0)
}

#[command]
pub async fn prepare_audio_chunks(
    _audio_data: Vec<f32>,
    _sample_rate: u32,
    _chunk_duration_ms: Option<u64>,
) -> Result<Vec<serde_json::Value>, String> {
    Err(UNAVAILABLE.to_string())
}

#[command]
pub async fn test_parallel_processing_setup(
    _state: State<'_, ParallelProcessorState>,
) -> Result<String, String> {
    Err(UNAVAILABLE.to_string())
}
