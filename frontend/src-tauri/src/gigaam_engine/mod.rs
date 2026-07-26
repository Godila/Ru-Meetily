//! GigaAM (Sber ai-sage) speech recognition engine module.
//!
//! This module provides a Russian-focused alternative to Parakeet/Whisper
//! for speech-to-text transcription. GigaAM-v3 offers state-of-the-art Russian
//! recognition (WER ~5-8% vs ~21% for Whisper large-v3 on RU benchmarks).
//!
//! Like Parakeet, it runs fully in-process via ONNX Runtime (`ort` crate) —
//! no Python, no external services. Weights are the ONNX conversions published
//! by `istupakov/gigaam-v3-onnx` (same author as the Parakeet ONNX models
//! already used by Convoic).
//!
//! # Current scope
//!
//! - E2E RNN-T variant (Russian only), int8 quantized. Three ONNX files:
//!   `v3_e2e_rnnt_encoder.int8.onnx`, `v3_e2e_rnnt_decoder.int8.onnx`,
//!   `v3_e2e_rnnt_joint.int8.onnx`, plus a BPE vocab. Produces text WITH
//!   punctuation and true-case.
//!
//! # Module structure
//!
//! - `preprocessor`: mel-filterbank STFT (Rust port of `onnx-asr`'s
//!   `GigaamPreprocessorNumpy`).
//! - `model`: ONNX model wrapper + RNN-T greedy transducer decode.
//! - `gigaam_engine`: engine (model lifecycle, download, discovery).
//! - `commands`: Tauri command interface for frontend integration.

pub mod preprocessor;
pub mod model;
pub mod gigaam_engine;
pub mod commands;

pub use gigaam_engine::{
    GigaamEngine, GigaamEngineError, QuantizationType, ModelInfo, ModelStatus, DownloadProgress,
};
pub use model::{GigaamModel, GigaamError, TimestampedResult};
pub use commands::*;
