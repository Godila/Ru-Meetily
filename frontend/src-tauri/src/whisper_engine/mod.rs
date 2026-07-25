//! Whisper engine — STUB.
//!
//! The real whisper-rs-based engine was removed from this build because
//! whisper-rs 0.13.2 is incompatible with libclang 22 on the build machine
//! (bindgen produces an opaque `whisper_full_params`). Speech-to-text in this
//! build is provided by GigaAM (Russian) and Parakeet (English), both via
//! ONNX Runtime.
//!
//! This module preserves the public API surface that the rest of the app
//! (`audio/transcription/engine.rs`, `audio/stt.rs`, `audio/import.rs`,
//! `audio/retranscription.rs`, `lib.rs`) compiles against. Every command
//! returns an error indicating Whisper is unavailable; callers should fall
//! back to GigaAM or Parakeet.

pub mod commands;
pub mod parallel_commands;

pub use commands::{WhisperEngine, ModelStatus, ModelInfo, WHISPER_ENGINE};
pub use commands::*;
