//! GigaAM-v3 E2E RNN-T ONNX model wrapper and greedy transducer decode.
//!
//! This is the Rust analog of `parakeet_engine/model.rs`, adapted for the
//! GigaAM-v3 E2E RNN-T variant (`v3_e2e_rnnt`). The RNN-T model produces text
//! **with punctuation and true-case**, unlike the CTC variant.
//!
//! # Architecture
//!
//! Three ONNX sessions (mirroring the istupakov/onnx-asr `GigaamV3Rnnt` class):
//!
//! 1. **Encoder** (`v3_e2e_rnnt_encoder.int8.onnx`): Conformer. Inputs
//!    `audio_signal` f32 `[1, 64, T_feat]` (log-mel, channel-first) and
//!    `length` i64 `[1]`. Outputs `encoded` f32 `[1, hidden, T_enc]` (needs
//!    transpose to `[1, T_enc, hidden]`) and `encoded_len` `[1]`.
//! 2. **Decoder/Predictor** (`v3_e2e_rnnt_decoder.int8.onnx`): single-layer
//!    LSTM, `pred_hidden = 320`. Inputs `x` (last token id, scalar), `h.1` and
//!    `c.1` (LSTM state, each `[1, 1, 320]`). Outputs `dec` (predictor
//!    embedding), new `h`, new `c`.
//! 3. **Joint** (`v3_e2e_rnnt_joint.int8.onnx`): fuses one encoder frame and
//!    one predictor embedding into logits over the 1025-token BPE vocab.
//!    Inputs `enc` `[1, hidden, 1]`, `dec` `[1, pred_hidden, 1]`. Output
//!    `joint` `[1, vocab, 1]` → squeeze → `[vocab]`.
//!
//! # Decoding
//!
//! Standard RNN-T greedy decode (see `onnx-asr` `_AsrWithTransducerDecoding`):
//! iterate encoder frames; at each frame run predictor + joint, argmax the
//! logits; on a non-blank token commit the LSTM state, emit the token, and
//! stay on the same frame (up to `MAX_TOKENS_PER_STEP = 3` emits); on blank
//! (or after the cap) advance to the next frame.
//!
//! Reference implementation: `onnx-asr`
//! `models/gigaam.py:GigaamV3Rnnt` and `asr.py:_AsrWithTransducerDecoding`.

use crate::gigaam_engine::preprocessor::{GigaamPreprocessor, SUBSAMPLING_FACTOR};
use ndarray::{Array, Array2, Array3, ArrayD, ArrayViewD, IxDyn};
use once_cell::sync::Lazy;
use ort::execution_providers::CPUExecutionProvider;
use ort::inputs;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::TensorRef;
use regex::Regex;

use std::fs;
use std::path::Path;

/// Time (seconds) covered by one encoder output frame.
/// `window_step (0.01s) * subsampling_factor (4) = 0.04s`.
const OUTPUT_FRAME_SECONDS: f32 = 0.01 * SUBSAMPLING_FACTOR as f32;

/// Maximum non-blank tokens emitted at a single encoder frame before forcing
/// a frame advance (runaway-repetition guard). From `config.json`.
const MAX_TOKENS_PER_STEP: usize = 3;

static DECODE_SPACE_RE: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"\A\s|\s\B|(\s)\b"));

#[derive(Debug, Clone)]
pub struct TimestampedResult {
    pub text: String,
    pub timestamps: Vec<f32>,
    pub tokens: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum GigaamError {
    #[error("ORT error")]
    Ort(#[from] ort::Error),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("ndarray shape error")]
    Shape(#[from] ndarray::ShapeError),
    #[error("Model input not found: {0}")]
    InputNotFound(String),
    #[error("Model output not found: {0}")]
    OutputNotFound(String),
    #[error("Tensor shape unavailable for input: {0}")]
    TensorShape(String),
}

/// Predictor LSTM state: `(h, c)`, each `[1, 1, PRED_HIDDEN]` f32.
type PredictorState = (Array3<f32>, Array3<f32>);

pub struct GigaamModel {
    encoder: Session,
    decoder: Session,
    joint: Session,
    vocab: Vec<String>,
    blank_idx: i32,
    vocab_size: usize,
    preprocessor: GigaamPreprocessor,
}

impl Drop for GigaamModel {
    fn drop(&mut self) {
        log::debug!(
            "Dropping GigaamModel (RNN-T) with {} vocab tokens",
            self.vocab.len()
        );
    }
}

impl GigaamModel {
    /// Load the E2E RNN-T model from a model directory.
    ///
    /// Expects `v3_e2e_rnnt_encoder.int8.onnx`, `v3_e2e_rnnt_decoder.int8.onnx`,
    /// `v3_e2e_rnnt_joint.int8.onnx` (or the fp32 variants) and
    /// `v3_e2e_rnnt_vocab.txt` to be present. `quantized` selects int8.
    pub fn new<P: AsRef<Path>>(model_dir: P, quantized: bool) -> Result<Self, GigaamError> {
        let encoder = Self::init_session(&model_dir, "v3_e2e_rnnt_encoder", quantized)?;
        let decoder = Self::init_session(&model_dir, "v3_e2e_rnnt_decoder", quantized)?;
        let joint = Self::init_session(&model_dir, "v3_e2e_rnnt_joint", quantized)?;

        let (vocab, blank_idx) = Self::load_vocab(&model_dir)?;
        let vocab_size = vocab.len();
        log::info!(
            "Loaded GigaAM RNN-T vocabulary with {} tokens, blank_idx={}",
            vocab_size,
            blank_idx
        );

        let preprocessor = GigaamPreprocessor::new();

        Ok(Self {
            encoder,
            decoder,
            joint,
            vocab,
            blank_idx,
            vocab_size,
            preprocessor,
        })
    }

    /// Build an ONNX session identical to Parakeet's: CPU provider, L3
    /// optimizations, parallel execution.
    fn init_session<P: AsRef<Path>>(
        model_dir: P,
        model_name: &str,
        try_quantized: bool,
    ) -> Result<Session, GigaamError> {
        let providers = vec![CPUExecutionProvider::default().build()];

        // Prefer the quantized (int8) file when requested, fall back to fp32.
        let model_filename = if try_quantized {
            let quantized_name = format!("{}.int8.onnx", model_name);
            let quantized_path = model_dir.as_ref().join(&quantized_name);
            if quantized_path.exists() {
                log::info!("Loading quantized GigaAM model from {}...", quantized_name);
                quantized_name
            } else {
                let regular_name = format!("{}.onnx", model_name);
                log::info!(
                    "Quantized model not found, loading regular GigaAM model from {}...",
                    regular_name
                );
                regular_name
            }
        } else {
            let regular_name = format!("{}.onnx", model_name);
            log::info!("Loading GigaAM model from {}...", regular_name);
            regular_name
        };

        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_execution_providers(providers)?
            .with_parallel_execution(true)?
            .commit_from_file(model_dir.as_ref().join(&model_filename))?;

        for input in &session.inputs {
            log::info!(
                "GigaAM Model '{}' input: name={}, type={:?}",
                model_filename,
                input.name,
                input.input_type
            );
        }
        for output in &session.outputs {
            log::info!(
                "GigaAM Model '{}' output: name={}",
                model_filename,
                output.name
            );
        }

        Ok(session)
    }

    /// Parse `v3_e2e_rnnt_vocab.txt`. Format: `<token> <id>` per line, where
    /// `▁` (U+2581) is replaced by a space and `<blk>` marks the RNN-T blank.
    /// Identical parsing to Parakeet's `load_vocab`. This is a BPE vocab
    /// (~1025 tokens) — tokens may be multi-character subwords.
    fn load_vocab<P: AsRef<Path>>(model_dir: P) -> Result<(Vec<String>, i32), GigaamError> {
        let vocab_path = model_dir.as_ref().join("v3_e2e_rnnt_vocab.txt");
        let content = fs::read_to_string(vocab_path)?;

        let mut max_id = 0;
        let mut tokens_with_ids: Vec<(String, usize)> = Vec::new();
        let mut blank_idx: Option<usize> = None;

        for line in content.lines() {
            // BPE tokens may themselves contain... no: format is `token id`
            // with a single separating space, and ids are integers. Split on
            // the LAST space so a token containing a space (after ▁→space
            // replacement happens later) doesn't break parsing — but the raw
            // token uses ▁, not a literal space, so rsplit_once is safe.
            if let Some((token, id_str)) = line.trim_end().rsplit_once(' ') {
                if let Ok(id) = id_str.parse::<usize>() {
                    if token == "<blk>" {
                        blank_idx = Some(id);
                    }
                    tokens_with_ids.push((token.to_string(), id));
                    max_id = max_id.max(id);
                }
            }
        }

        // Build vocab vector with U+2581 replaced by space.
        let mut vocab = vec![String::new(); max_id + 1];
        for (token, id) in tokens_with_ids {
            vocab[id] = token.replace('\u{2581}', " ");
        }

        let blank_idx = blank_idx.ok_or_else(|| {
            GigaamError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Missing <blk> token in GigaAM vocabulary",
            ))
        })? as i32;

        Ok((vocab, blank_idx))
    }

    /// Run the Conformer encoder.
    ///
    /// Inputs (GigaAM-v3-RNNT encoder):
    ///   `audio_signal`  f32 [1, 64, T_feat]   (channel-first log-mel)
    ///   `length`        i64 [1]
    /// Outputs:
    ///   `encoded`       f32 [1, hidden, T_enc] → transposed to [1, T_enc, hidden]
    ///
    /// The encoder also emits `encoded_len` (int32), but its subsampling is a
    /// deterministic `(T_feat - 1) / 4 + 1` — we recompute it in Rust instead
    /// of extracting the i32 tensor, avoiding an element-type mismatch.
    fn encode(
        &mut self,
        features: &ArrayD<f32>,
        feature_lengths: &ArrayD<i64>,
    ) -> Result<ArrayD<f32>, GigaamError> {
        log::trace!("Running GigaAM RNN-T encoder inference...");
        let inputs = inputs![
            "audio_signal" => TensorRef::from_array_view(features.view())?,
            "length" => TensorRef::from_array_view(feature_lengths.view())?,
        ];
        let outputs = self.encoder.run(inputs)?;

        let encoded = outputs
            .get("encoded")
            .ok_or_else(|| GigaamError::OutputNotFound("encoded".to_string()))?
            .try_extract_array()?;

        // Raw encoder output is [B, hidden, T_enc]; transpose to
        // [B, T_enc, hidden] so we can index frames along axis 1.
        let encoded = encoded.permuted_axes(IxDyn(&[0, 2, 1]));

        Ok(encoded.to_owned())
    }

    /// Create the initial predictor LSTM state (all zeros).
    ///
    /// State shapes are introspected from the decoder ONNX session inputs so
    /// the code adapts if `pred_hidden` differs from the expected 320.
    fn create_predictor_state(&self) -> Result<PredictorState, GigaamError> {
        let h_shape = self
            .decoder
            .inputs
            .iter()
            .find(|i| i.name == "h.1")
            .and_then(|i| i.input_type.tensor_shape())
            .ok_or_else(|| GigaamError::TensorShape("h.1".to_string()))?;
        // Shape is symbolic [1, 1, 320]; pin batch=1 at axis 1.
        let h = Array::zeros((1, 1, h_shape[2] as usize));
        let c = Array::zeros((1, 1, h_shape[2] as usize));
        Ok((h, c))
    }

    /// One transducer step: run predictor on the last emitted token, then run
    /// the joint on one encoder frame + the predictor embedding. Returns the
    /// logits vector `[vocab_size]` and the new predictor state.
    ///
    /// Mirrors `onnx-asr` `GigaamV3Rnnt._decode`. Note we always invoke the
    /// predictor here; because the predictor is a deterministic function of
    /// (last token, h, c) and none of those change on a blank step, this is
    /// numerically identical to the Python reference's "predictor only runs on
    /// emit" caching optimization.
    fn transducer_step(
        &mut self,
        prev_token: i32,
        prev_state: &PredictorState,
        encoder_frame: &ArrayViewD<f32>, // [hidden]
    ) -> Result<(ArrayD<f32>, PredictorState), GigaamError> {
        // --- Predictor (decoder LSTM) ---
        // `x` = [[token]] int64 (shape [1,1]). The model's `x` input is dtype
        // int64, so widen the i32 token id.
        let x: Array2<i64> = Array2::from_shape_vec((1, 1), vec![prev_token as i64])?;
        let inputs = inputs![
            "x" => TensorRef::from_array_view(x.view())?,
            "h.1" => TensorRef::from_array_view(prev_state.0.view())?,
            "c.1" => TensorRef::from_array_view(prev_state.1.view())?,
        ];
        let outputs = self.decoder.run(inputs)?;

        let decoder_out = outputs
            .get("dec")
            .ok_or_else(|| GigaamError::OutputNotFound("dec".to_string()))?
            .try_extract_array::<f32>()?;
        let h_new = outputs
            .get("h")
            .ok_or_else(|| GigaamError::OutputNotFound("h".to_string()))?
            .try_extract_array::<f32>()?;
        let c_new = outputs
            .get("c")
            .ok_or_else(|| GigaamError::OutputNotFound("c".to_string()))?
            .try_extract_array::<f32>()?;

        // --- Joint ---
        // enc = encoder_frame[None, :, None] → [1, hidden, 1]
        let enc_input = encoder_frame
            .to_owned()
            .insert_axis(ndarray::Axis(0))
            .insert_axis(ndarray::Axis(2));
        // dec = decoder_out.transpose(0, 2, 1): decoder_out is [1, 1, D] → [1, D, 1]
        let dec_input = decoder_out.permuted_axes(IxDyn(&[0, 2, 1])).to_owned();

        let inputs = inputs![
            "enc" => TensorRef::from_array_view(enc_input.view())?,
            "dec" => TensorRef::from_array_view(dec_input.view())?,
        ];
        let outputs = self.joint.run(inputs)?;

        let joint = outputs
            .get("joint")
            .ok_or_else(|| GigaamError::OutputNotFound("joint".to_string()))?
            .try_extract_array::<f32>()?;

        // joint is 4D [1, 1, 1, vocab]; collapse the three leading size-1 axes
        // to get a 1-D [vocab] logits vector.
        let mut logits = joint.to_owned();
        while logits.ndim() > 1 {
            logits = logits.remove_axis(ndarray::Axis(0));
        }

        // Convert LSTM states back to typed Array3.
        let h_new_3d = h_new.to_owned().into_dimensionality::<ndarray::Ix3>()?;
        let c_new_3d = c_new.to_owned().into_dimensionality::<ndarray::Ix3>()?;

        Ok((logits, (h_new_3d, c_new_3d)))
    }

    /// Greedy RNN-T decode over the encoder output for one utterance.
    ///
    /// Reference: `onnx-asr` `_AsrWithTransducerDecoding._decoding`. The state
    /// machine: at each encoder frame, emit non-blank tokens (up to
    /// `MAX_TOKENS_PER_STEP`) while staying on the frame; advance to the next
    /// frame on blank or after the cap.
    fn decode_sequence(
        &mut self,
        encodings: &ArrayViewD<f32>, // [T_enc, hidden]
        encodings_len: usize,
    ) -> Result<(Vec<i32>, Vec<usize>), GigaamError> {
        let mut prev_state = self.create_predictor_state()?;
        let mut tokens: Vec<i32> = Vec::new();
        let mut timestamps: Vec<usize> = Vec::new();

        let mut t = 0usize;
        let mut emitted_tokens = 0usize;

        while t < encodings_len {
            // One encoder frame: encodings[t] → [hidden].
            let frame = encodings.slice(ndarray::s![t, ..]).to_owned().into_dyn();
            let last_token = tokens.last().copied().unwrap_or(self.blank_idx);

            let (logits, new_state) =
                self.transducer_step(last_token, &prev_state, &frame.view())?;

            // argmax over the vocab logits (raw joint output, no softmax).
            let logits_slice = logits
                .as_slice()
                .ok_or_else(|| {
                    GigaamError::Shape(ndarray::ShapeError::from_kind(
                        ndarray::ErrorKind::IncompatibleShape,
                    ))
                })?;
            // Only consider the first vocab_size entries (defensive: joint
            // may emit extra columns in some exports).
            let n = logits_slice.len().min(self.vocab_size);
            let token = logits_slice[..n]
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(idx, _)| idx as i32)
                .unwrap_or(self.blank_idx);

            if token != self.blank_idx {
                // Commit LSTM state and emit on non-blank.
                prev_state = new_state;
                tokens.push(token);
                timestamps.push(t);
                emitted_tokens += 1;
            }

            // Advance the frame on blank or after the per-frame emit cap.
            if token == self.blank_idx || emitted_tokens >= MAX_TOKENS_PER_STEP {
                t += 1;
                emitted_tokens = 0;
            }
        }

        if tokens.is_empty() {
            log::debug!(
                "GigaAM RNN-T decoded zero tokens (all blank) for audio with {} encoding timesteps - audio may be too short or low energy",
                encodings_len
            );
        }

        Ok((tokens, timestamps))
    }

    /// Map decoded token ids to text, applying the same space-cleanup regex
    /// as Parakeet (`DECODE_SPACE_RE`).
    fn decode_tokens(&self, ids: Vec<i32>, indices: Vec<usize>) -> TimestampedResult {
        let tokens: Vec<String> = ids
            .iter()
            .filter_map(|&id| {
                let idx = id as usize;
                if idx < self.vocab.len() {
                    Some(self.vocab[idx].clone())
                } else {
                    None
                }
            })
            .collect();

        let text = match &*DECODE_SPACE_RE {
            Ok(regex) => regex
                .replace_all(&tokens.join(""), |caps: &regex::Captures| {
                    if caps.get(1).is_some() {
                        " "
                    } else {
                        ""
                    }
                })
                .to_string(),
            Err(_) => tokens.join(""),
        };

        let float_timestamps: Vec<f32> = indices
            .iter()
            .map(|&t| OUTPUT_FRAME_SECONDS * t as f32)
            .collect();

        TimestampedResult {
            text,
            timestamps: float_timestamps,
            tokens,
        }
    }

    /// Full inference: preprocess → encode → RNN-T decode → text.
    ///
    /// GigaAM (Conformer with attention) has a finite context limit: feeding an
    /// overly long segment (e.g. a >30s VAD segment from a long monologue)
    /// causes an ONNX `Mul` broadcast error and crashes the transcription. To
    /// stay safely under that limit, audio longer than `MAX_CHUNK_SAMPLES` is
    /// split into non-overlapping windows and each window transcribed
    /// independently; the texts are concatenated.
    ///
    /// Windows are non-overlapping. An earlier version used a 100 ms overlap
    /// "so words at the seam aren't lost", but never trimmed the overlap region
    /// of the following chunk, which duplicated any word straddling the seam.
    /// Overlap stitching requires transcript alignment that isn't worth the
    /// complexity here; the occasional cut word at a 24 s boundary is far less
    /// noticeable than duplicated fragments throughout the transcript.
    pub fn transcribe_samples(&mut self, samples: Vec<f32>) -> Result<TimestampedResult, GigaamError> {
        const SAMPLE_RATE: usize = 16_000;
        // ~24s window. The model's attention reliably handles sequences up to
        // ~30s; 24s leaves headroom.
        const MAX_CHUNK_SAMPLES: usize = 24 * SAMPLE_RATE;

        if samples.len() <= MAX_CHUNK_SAMPLES {
            // Short enough: single pass.
            return self.transcribe_single(samples);
        }

        // Long audio: split into non-overlapping windows and concatenate.
        log::debug!(
            "GigaAM RNN-T chunking {} samples ({:.1}s) into {}s windows",
            samples.len(),
            samples.len() as f64 / SAMPLE_RATE as f64,
            MAX_CHUNK_SAMPLES / SAMPLE_RATE
        );

        let mut combined_text = String::new();
        let mut combined_tokens: Vec<String> = Vec::new();
        let mut combined_timestamps: Vec<f32> = Vec::new();
        let mut offset_seconds: f32 = 0.0;
        let step = MAX_CHUNK_SAMPLES;
        let mut pos = 0usize;

        while pos < samples.len() {
            let end = (pos + MAX_CHUNK_SAMPLES).min(samples.len());
            let chunk = samples[pos..end].to_vec();
            let chunk_duration = chunk.len() as f32 / SAMPLE_RATE as f32;

            match self.transcribe_single(chunk) {
                Ok(r) => {
                    if !r.text.is_empty() {
                        if !combined_text.is_empty() && !combined_text.ends_with(' ') {
                            combined_text.push(' ');
                        }
                        combined_text.push_str(&r.text);
                        combined_tokens.extend(r.tokens);
                        // Shift timestamps by the chunk's start offset.
                        for &t in &r.timestamps {
                            combined_timestamps.push(t + offset_seconds);
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "GigaAM RNN-T chunk [{:.1}s..{:.1}s] failed: {} (continuing)",
                        offset_seconds,
                        offset_seconds + chunk_duration,
                        e
                    );
                }
            }

            offset_seconds += step as f32 / SAMPLE_RATE as f32;
            pos += step;
        }

        Ok(TimestampedResult {
            text: combined_text,
            timestamps: combined_timestamps,
            tokens: combined_tokens,
        })
    }

    /// Transcribe a single (short enough) chunk in one encoder pass + RNN-T decode.
    fn transcribe_single(&mut self, samples: Vec<f32>) -> Result<TimestampedResult, GigaamError> {
        // Preprocess (16 kHz mono f32 assumed; the upstream VAD pipeline
        // guarantees this contract, same as Parakeet).
        let (features, feature_lengths) = self.preprocessor.preprocess(&samples);

        if feature_lengths == 0 {
            // Audio too short to produce a single frame. Return empty.
            return Ok(TimestampedResult {
                text: String::new(),
                timestamps: Vec::new(),
                tokens: Vec::new(),
            });
        }

        // feature_lengths is a scalar; wrap as [1] for the batch dimension.
        let feature_lengths_arr =
            ndarray::Array1::from_vec(vec![feature_lengths]).into_dyn();

        // Encode. `features` from the preprocessor is `Array3`; convert to the
        // dynamic-dimension array the encoder expects.
        let features_dyn = features.into_dyn();
        let encoder_out = self.encode(&features_dyn, &feature_lengths_arr)?;

        // encoder_out is [1, T_enc, hidden]. Squeeze batch dim → [T_enc, hidden].
        let encodings = encoder_out.remove_axis(ndarray::Axis(0));

        // Encoder length is a deterministic subsampling of the feature length
        // (matches the model's `encoded_len` int32 output): T_enc = (T_feat-1)/4 + 1.
        let t_enc = encodings.shape()[0];
        let encodings_len = ((feature_lengths - 1) / SUBSAMPLING_FACTOR as i64 + 1) as usize;
        let encodings_len = encodings_len.min(t_enc);

        if encodings_len == 0 {
            return Ok(TimestampedResult {
                text: String::new(),
                timestamps: Vec::new(),
                tokens: Vec::new(),
            });
        }

        // Decode.
        let (token_ids, indices) = self.decode_sequence(&encodings.view(), encodings_len)?;

        if token_ids.is_empty() {
            log::debug!(
                "GigaAM RNN-T produced no tokens for {} encoder frames",
                encodings_len
            );
        }

        Ok(self.decode_tokens(token_ids, indices))
    }
}
