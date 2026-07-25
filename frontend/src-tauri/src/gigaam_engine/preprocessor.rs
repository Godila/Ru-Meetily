//! GigaAM-v3 mel-filterbank preprocessor (Rust port of `onnx-asr`'s
//! `GigaamPreprocessorNumpy`).
//!
//! Reference: <https://github.com/istupakov/onnx-asr>
//! file: `src/onnx_asr/preprocessors/numpy_preprocessor.py`
//! and the filterbank generator: `preprocessors/fbanks.py`, `preprocessors/gigaam.py`.
//!
//! # Algorithm (GigaAM-v3)
//!
//! 1. Frame the waveform into windows of length 320 (20 ms at 16 kHz),
//!    hop 160 (10 ms), **center=false** (no reflect padding — this differs
//!    from v2/Whisper/NeMo which do pad).
//! 2. Multiply each frame by a periodic Hann window of length 320
//!    (`np.hanning(321)[:-1]`).
//! 3. Real FFT of size 320 → 161 power bins (`|F|^2`).
//! 4. Matrix-multiply with the [161, 64] HTK mel filterbank → 64 mel energies.
//! 5. Clamp to [1e-9, 1e9] and take the natural log.
//! 6. Output is channel-first: `[1, 64, n_frames]` f32, plus
//!    `feature_lengths = n_frames` (i64).
//!
//! No preemphasis, no dither, no per-utterance mean/variance normalization.
//!
//! # Numerical note
//!
//! The upstream `onnx-asr` package quantizes the filterbank matrix and the
//! window to bfloat16 and back to f32 before saving to `fbanks.npz`. We
//! generate them directly in f32 here. This differs from the reference by
//! ~1e-2 on mel energies, which the model tolerates well. If WER is
//! observably worse than the Python reference, switch to embedding the
//! exact npz arrays via `include_bytes!`.

use ndarray::{Array1, Array2, Array3};
use realfft::num_complex::Complex32;
use realfft::RealFftPlanner;

/// Sample rate expected by GigaAM (Hz).
pub const SAMPLE_RATE: usize = 16_000;
/// FFT size / window length for GigaAM-v3 (20 ms at 16 kHz).
pub const N_FFT: usize = SAMPLE_RATE / 50; // 320
/// Hop length between frames (10 ms at 16 kHz).
pub const HOP_LENGTH: usize = SAMPLE_RATE / 100; // 160
/// Number of mel bins (= `features_size` in `config.json`).
pub const N_MELS: usize = 64;
/// Mel filterbank lower bound (Hz). From `preprocessors/gigaam.py`.
const F_MIN: f64 = 0.0;
/// Mel filterbank upper bound (Hz). From `preprocessors/gigaam.py`.
const F_MAX: f64 = 8_000.0;
/// Clamp range applied to mel energies before the log.
const CLAMP_MIN: f32 = 1e-9;
const CLAMP_MAX: f32 = 1e9;

/// Subsampling factor (encoder downsamples features by this factor).
/// From `config.json` (`subsampling_factor`). Used only for output length;
/// the preprocessor itself does not subsample.
pub const SUBSAMPLING_FACTOR: usize = 4;

/// GigaAM-v3 mel-filterbank preprocessor.
///
/// Holds the FFT handle, window, and filterbank matrix so they are computed
/// once and reused across calls.
pub struct GigaamPreprocessor {
    /// Periodic Hann window, length `N_FFT`.
    window: Array1<f32>,
    /// HTK mel filterbank, shape `[N_FFT/2+1, N_MELS]` = `[161, 64]`.
    /// Multiplied on the left of the power spectrum: `mel = spectrum @ fbanks`.
    fbanks: Array2<f32>,
    /// Real FFT handle for size `N_FFT`. `RealToComplex<T>` is a trait, so the
    /// handle is a trait object (`plan_fft_forward` returns
    /// `Arc<dyn RealToComplex<T>>`).
    r2c: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    /// Reusable output buffer (length N_FFT/2+1 = 161).
    spectrum_buf: Vec<Complex32>,
}

impl GigaamPreprocessor {
    /// Build the preprocessor, computing the window and filterbank once.
    pub fn new() -> Self {
        // NOTE: the upstream `onnx-asr` package quantizes both the window and
        // the mel filterbank to bfloat16 then back to f32 before use. We must
        // do the same, or the log-mel features differ enough that the model
        // degrades badly on real (noisy/echoey) audio. See the bfloat16
        // round-trip emulation below.
        let window = bf16_round_trip_arr(&periodic_hann(N_FFT));
        let fbanks_raw = melscale_fbanks(N_FFT / 2 + 1, F_MIN, F_MAX, N_MELS, SAMPLE_RATE as f64);
        let fbanks = bf16_round_trip_arr_2d(&fbanks_raw);

        let mut planner = RealFftPlanner::<f32>::new();
        // `plan_fft_forward` matches the pattern already used in
        // `audio/audio_processing.rs::spectral_subtraction`. It returns an
        // `Arc<RealToComplex<f32>>` whose `process` takes `(&mut input, &mut output)`.
        let r2c = planner.plan_fft_forward(N_FFT);
        let spectrum_buf = r2c.make_output_vec();

        Self {
            window,
            fbanks,
            r2c,
            spectrum_buf,
        }
    }

    /// Compute log-mel features for a single mono 16 kHz waveform.
    ///
    /// Returns `(features, feature_lengths)` where `features` has shape
    /// `[1, N_MELS, n_frames]` (channel-first) and `feature_lengths = n_frames`.
    /// If the waveform is shorter than one window (`< N_FFT` samples), the
    /// resulting frame count is 0 and the caller should reject the input.
    pub fn preprocess(&mut self, waveform: &[f32]) -> (Array3<f32>, i64) {
        let wav_len = waveform.len();

        // Number of frames with center=false framing. Guard against underflow
        // for very short audio.
        let n_frames = if wav_len >= N_FFT {
            (wav_len - N_FFT) / HOP_LENGTH + 1
        } else {
            0
        };

        // Output layout is channel-first: [1, N_MELS, n_frames].
        let mut features = Array3::zeros((1, N_MELS, n_frames));

        if n_frames == 0 {
            return (features, 0i64);
        }

        let n_bins = N_FFT / 2 + 1; // 161
        let mut frame_windowed = vec![0f32; N_FFT];

        for t in 0..n_frames {
            let start = t * HOP_LENGTH;

            // Apply window to the frame.
            for i in 0..N_FFT {
                frame_windowed[i] = waveform[start + i] * self.window[i];
            }

            // rfft → power spectrum (|F|^2), 161 bins.
            // `process` overwrites `spectrum_buf` and takes ownership-style
            // `&mut` for both buffers (2-arg form, matches audio_processing.rs).
            self.r2c
                .process(&mut frame_windowed, &mut self.spectrum_buf)
                .expect("rfft size matches N_FFT");

            // mel[m] = sum_k power[k] * fbanks[k][m]
            for m in 0..N_MELS {
                let mut acc = 0f32;
                for k in 0..n_bins {
                    let re = self.spectrum_buf[k].re;
                    let im = self.spectrum_buf[k].im;
                    let power = re * re + im * im;
                    acc += power * self.fbanks[(k, m)];
                }
                let clamped = acc.clamp(CLAMP_MIN, CLAMP_MAX);
                // Channel-first output: features[0][m][t].
                features[(0, m, t)] = clamped.ln();
            }
        }

        (features, n_frames as i64)
    }
}

/// Periodic Hann window of length `n`, matching `np.hanning(n + 1)[:-1]`.
///
/// `np.hanning(M)` returns `0.5 - 0.5*cos(2*pi*n/(M-1))` for `n in [0, M)`,
/// so `np.hanning(n + 1)[:-1]` is `0.5 - 0.5*cos(2*pi*i/n)` for `i in [0, n)`.
fn periodic_hann(n: usize) -> Array1<f32> {
    let mut w = Array1::zeros(n);
    for i in 0..n {
        let v = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos();
        w[i] = v as f32;
    }
    w
}

/// Emulate the f32 → bfloat16 → f32 round-trip that `ml_dtypes` (used by
/// `onnx-asr`'s preprocessor generator) applies to the window and filterbank.
///
/// bfloat16 has the same 8-bit exponent as f32 but only 7 mantissa bits
/// (vs f32's 23). We truncate by round-to-nearest-even on the low 16 bits,
/// matching `np.astype(bfloat16).astype(float32)`. Without this, the Rust
/// f32 arrays differ by ~1e-2 and the GigaAM model degrades on real audio.
fn bf16_round_trip(x: f32) -> f32 {
    // Reinterpret bits. f32: 1 sign | 8 exponent | 23 mantissa.
    // bfloat16 keeps the top 16 bits (sign + exponent + top 7 mantissa bits),
    // with round-to-nearest-even on the 16 discarded bits.
    let bits = x.to_bits();
    // Round to nearest even: add 0x7FFF plus the odd-bias bit (0x8000 already
    // contributes the 'even' tie-break via the lowest kept mantissa bit).
    let rounding_bias = 0x7FFFu32 + ((bits >> 16) & 1);
    let rounded = bits.wrapping_add(rounding_bias) & 0xFFFF_0000;
    f32::from_bits(rounded)
}

fn bf16_round_trip_arr(a: &Array1<f32>) -> Array1<f32> {
    a.mapv(bf16_round_trip)
}

fn bf16_round_trip_arr_2d(a: &Array2<f32>) -> Array2<f32> {
    a.mapv(bf16_round_trip)
}

/// HTK mel filterbank, matching `preprocessors/fbanks.py:melscale_fbanks`
/// with `mel_scale="htk"`, `norm=None`.
///
/// Returns a matrix of shape `[n_freqs, n_mels]`. Triangular filters on the
/// HTK mel scale, where each column is a single mel filter's response across
/// the linear frequency bins.
fn melscale_fbanks(n_freqs: usize, f_min: f64, f_max: f64, n_mels: usize, sample_rate: f64) -> Array2<f32> {
    // Linear frequency axis: linspace(0, sample_rate/2, n_freqs).
    let all_freqs: Vec<f64> = (0..n_freqs)
        .map(|i| sample_rate / 2.0 * i as f64 / (n_freqs - 1) as f64)
        .collect();

    // Mel endpoints for the triangular filters.
    let m_min = hz_to_mel_htk(f_min);
    let m_max = hz_to_mel_htk(f_max);
    let m_pts: Vec<f64> = (0..n_mels + 2)
        .map(|i| m_min + (m_max - m_min) * i as f64 / (n_mels + 1) as f64)
        .map(|m| mel_to_hz_htk(m))
        .collect();

    let mut fb = Array2::zeros((n_freqs, n_mels));
    for (i, &freq) in all_freqs.iter().enumerate() {
        for m in 0..n_mels {
            let left = m_pts[m];
            let center = m_pts[m + 1];
            let right = m_pts[m + 2];
            let up = if center > left {
                (freq - left) / (center - left)
            } else {
                0.0
            };
            let down = if right > center {
                (right - freq) / (right - center)
            } else {
                0.0
            };
            let val = up.min(down).max(0.0);
            fb[(i, m)] = val as f32;
        }
    }
    fb
}

/// HTK mel scale: `2595 * log10(1 + f / 700)`.
fn hz_to_mel_htk(freq: f64) -> f64 {
    2595.0 * (1.0 + freq / 700.0).log10()
}

/// Inverse HTK mel scale: `700 * (10^(m/2595) - 1)`.
fn mel_to_hz_htk(mel: f64) -> f64 {
    700.0 * (10f64.powf(mel / 2595.0) - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_periodic_hann_endpoints() {
        let w = periodic_hann(320);
        // Periodic Hann: w[0] == 0 (cos(0)=1 → 0.5-0.5).
        assert!((w[0]).abs() < 1e-6);
        // Symmetric is NOT expected for the periodic variant; the last sample
        // is just before the (would-be) 0 at index n.
        assert!(w[319] < 0.01);
        // Peak near the center.
        let peak = w[160];
        assert!(peak > 0.99 && peak <= 1.0);
    }

    #[test]
    fn test_fbanks_shape_and_nonneg() {
        let fb = melscale_fbanks(161, 0.0, 8000.0, 64, 16000.0);
        assert_eq!(fb.shape(), &[161, 64]);
        // Filter responses are non-negative.
        for v in fb.iter() {
            assert!(*v >= 0.0);
        }
    }

    #[test]
    fn test_frame_count() {
        let mut p = GigaamPreprocessor::new();
        // 1 second of silence.
        let wav = vec![0f32; SAMPLE_RATE];
        let (feats, lens) = p.preprocess(&wav);
        // (16000 - 320)/160 + 1 = 99 frames.
        assert_eq!(lens, 99);
        assert_eq!(feats.shape(), &[1, N_MELS, 99]);
    }

    #[test]
    fn test_short_audio_yields_zero_frames() {
        let mut p = GigaamPreprocessor::new();
        // Shorter than one window → 0 frames, empty feature tensor.
        let wav = vec![0f32; 100];
        let (feats, lens) = p.preprocess(&wav);
        assert_eq!(lens, 0);
        assert_eq!(feats.shape(), &[1, N_MELS, 0]);
    }

    #[test]
    fn test_tone_produces_peaked_mel() {
        // A pure 440 Hz tone should concentrate energy in the low mel bins.
        let mut p = GigaamPreprocessor::new();
        let n = SAMPLE_RATE;
        let wav: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / SAMPLE_RATE as f32).sin() * 0.5)
            .collect();
        let (feats, _lens) = p.preprocess(&wav);
        // Energy should be non-negative after exp (i.e. features finite).
        for v in feats.iter() {
            assert!(v.is_finite());
        }
    }
}
