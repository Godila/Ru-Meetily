// audio/transcription/gigaam_provider.rs
//
// GigaAM transcription provider implementation. Mirrors
// `parakeet_provider.rs`: wraps the GigaAM engine behind the shared
// `TranscriptionProvider` trait.

use super::provider::{TranscriptionError, TranscriptionProvider, TranscriptResult};
use async_trait::async_trait;
use log::warn;
use std::sync::Arc;

/// GigaAM transcription provider (wraps GigaamEngine).
pub struct GigaamProvider {
    engine: Arc<crate::gigaam_engine::GigaamEngine>,
}

impl GigaamProvider {
    pub fn new(engine: Arc<crate::gigaam_engine::GigaamEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl TranscriptionProvider for GigaamProvider {
    async fn transcribe(
        &self,
        audio: Vec<f32>,
        language: Option<String>,
    ) -> std::result::Result<TranscriptResult, TranscriptionError> {
        // GigaAM-v3 is Russian-focused. Log any language hint but transcribe
        // regardless (the model handles its supported languages internally).
        if let Some(ref lang) = language {
            warn!(
                "GigaAM ignores language preference '{}' - transcribing with the built-in Russian model",
                lang
            );
        }

        match self.engine.transcribe_audio(audio).await {
            Ok(text) => Ok(TranscriptResult {
                text: text.trim().to_string(),
                confidence: None, // CTC doesn't expose a per-utterance confidence here
                is_partial: false,
            }),
            Err(e) => Err(TranscriptionError::EngineFailed(e.to_string())),
        }
    }

    async fn is_model_loaded(&self) -> bool {
        self.engine.is_model_loaded().await
    }

    async fn get_current_model(&self) -> Option<String> {
        self.engine.get_current_model().await
    }

    fn provider_name(&self) -> &'static str {
        "GigaAM"
    }
}
