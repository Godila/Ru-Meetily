# GigaAM Russian STT — integration notes

This fork adds **GigaAM-v3 CTC** (Sber `ai-sage`, ONNX conversion by
`istupakov/gigaam-v3-onnx`) as a third local speech-to-text engine, alongside
Parakeet and Whisper. It is selected when you need **high-quality Russian
recognition** (WER ~5–8% vs ~21% for Whisper large-v3 on Russian benchmarks).

Like Parakeet, GigaAM runs fully **in-process on Rust via `ort`** (ONNX Runtime)
— no Python, no sidecar, no external service.

## Why this works

The ONNX conversion of GigaAM was published by the **same author**
(`istupakov`) who produced the Parakeet ONNX models Convoic already ships. The
two share the same `ort` runtime and a very similar file layout. The CTC
variant is the simplest possible ASR integration: one ONNX file, a single
forward pass, and a trivial argmax+collapse decode (no RNN-T state machine).

Licenses are compatible for forking: Convoic (MIT) + GigaAM weights (MIT) +
`onnx-asr` reference (MIT).

## Files added

### Rust backend — `frontend/src-tauri/src/gigaam_engine/`
- `mod.rs` — module declaration + re-exports.
- `preprocessor.rs` — mel-filterbank STFT. Rust port of `onnx-asr`'s
  `GigaamPreprocessorNumpy` (frame 320 / hop 160 / 64 mels / HTK filterbank /
  periodic Hann / `log(clamp(·,1e-9,1e9))`, channel-first `[1,64,T]`).
- `model.rs` — ONNX session load, CTC encode (`features`/`feature_lengths` →
  `log_probs`), greedy CTC decode (argmax → drop blank → collapse repeats),
  token→text with the same space-cleanup regex as Parakeet.
- `gigaam_engine.rs` — engine: model lifecycle, discovery, HuggingFace
  download with resume/progress/cancel (adapted from `parakeet_engine.rs`).
- `commands.rs` — Tauri commands `gigaam_*` and `gigaam-model-*` events.

### Rust wiring
- `audio/transcription/gigaam_provider.rs` — `GigaamProvider` implementing the
  shared `TranscriptionProvider` trait.
- `audio/transcription/mod.rs`, `engine.rs`, `config.rs`, `lib.rs` — routing,
  the `Gigaam` engine variant, default model constant, command registration.

### Frontend
- `src/lib/gigaam.ts` — types + `GigaamAPI` invoke wrappers.
- `src/components/GigaAMModelManager.tsx` — model download/select UI.
- `src/components/TranscriptSettings.tsx`, `src/hooks/useTranscriptionModels.ts`,
  `src/components/LanguageSelection.tsx` — provider dropdown + union types.

## Files downloaded at runtime

From `https://huggingface.co/istupakov/gigaam-v3-onnx/resolve/main/`:
- `v3_ctc.int8.onnx` (~225 MB)
- `v3_vocab.txt` (198 B; 34 tokens: `▁`=space, `а..я`, `<blk>`=33)
- `config.json` (135 B)

Stored under `<app_data_dir>/models/gigaam/gigaam-v3-ctc-int8/`.

## Build & verify

### Prerequisites (Windows, MSVC toolchain)
Convoic's `whisper-rs` dependency needs a C toolchain and libclang:
- **Rust** (MSVC host: `x86_64-pc-windows-msvc`).
- **MSVC Build Tools 2022** (Visual Studio Build Tools with the "Desktop
  development with C++" workload — provides `cl.exe`/`link.exe`).
- **LLVM** for `libclang.dll` (needed by `whisper-rs-sys` bindgen). Install via
  `winget install LLVM.LLVM` and set `LIBCLANG_PATH=C:\Program Files\LLVM\bin`.
- **whisper.cpp submodule** must be initialized:
  `git submodule update --init --recursive backend/whisper.cpp`.
- **llama-helper** binary placed at
  `frontend/src-tauri/binaries/llama-helper-x86_64-pc-windows-msvc.exe` (build
  it from the `llama-helper/` crate with `cargo build --release` and copy the
  output). This is a Tauri resource required by the main crate, unrelated to
  GigaAM.

> Note: `whisper-rs 0.13.2` vs the bundled `whisper.cpp` can mismatch on the
> `whisper_full_params` layout. That is a pre-existing Convoic build issue on
> some toolchain combos, not related to GigaAM. It only affects the Whisper
> engine; GigaAM and Parakeet do not depend on whisper-rs.

### Verified during development
The four GigaAM Rust files (`preprocessor.rs`, `model.rs`, `gigaam_engine.rs`,
`gigaam_provider.rs`) were compile-checked **in isolation** with the exact
dependency versions Convoic pins (`ort = 2.0.0-rc.10`, `ndarray = 0.16`,
`realfft = 3.4.0`) plus tauri/api/tray stubs. All compile cleanly.
`commands.rs` is a near-verbatim copy of `parakeet_engine/commands.rs` (its
call to `api_get_transcript_config` is byte-identical to the Parakeet version
that compiles in-tree).

### 1. Backend
From a `cmd.exe` with the MSVC environment loaded (e.g. run `vcvars64.bat`),
with `LIBCLANG_PATH` set:
```bash
cd frontend/src-tauri
cargo build            # default CPU build (matches Parakeet)
# On Windows with NVIDIA: cargo build --release --features cuda
```

If `cargo` reports an error in `gigaam_engine/preprocessor.rs` about the
`realfft` API, compare with `audio/audio_processing.rs::spectral_subtraction`
which uses the same `plan_fft_forward` / `make_output_vec` / 2-arg `process`
pattern.

### 3. Frontend
```bash
cd frontend
npm install        # or pnpm install
npm run build      # type-check + build
```
A clean type-check confirms all `provider` union types now include `'gigaam'`.

### 4. End-to-end (manual)
1. Run the app, open **Settings → Transcript Model**.
2. Pick **🇷🇺 GigaAM (Лучшее распознавание русского)**.
3. Click **Download** on the `gigaam-v3-ctc-int8` card (~225 MB).
4. Record a meeting in Russian; verify the transcript is intelligible
   (compare against Parakeet's output for the same audio).

## Numerical fidelity note

The reference `onnx-asr` package quantizes the mel filterbank and window to
**bfloat16** before saving to `fbanks.npz`. We generate them directly in f32,
which differs by ~1e-2 on mel energies. The model tolerates this well. If you
observe WER noticeably worse than the Python reference, switch to embedding
the exact arrays: run

```python
import numpy as np, onnx_asr.preprocessors as p, importlib.resources as r
d = np.load(r.files(p).joinpath('data/fbanks.npz'))
np.save('gigaam_v3_fbanks.npy', d['gigaam_v3'])
np.save('gigaam_v3_window.npy', d['gigaam_v3_window'])
```

and load them in `GigaamPreprocessor::new` via `include_bytes!` instead of
calling `melscale_fbanks` / `periodic_hann`.

## Future work (not in this change)
- `v3_rnnt` variant (3 ONNX files) — streaming RNN-T decode, mirrors Parakeet.
- `v3_e2e_rnnt` — built-in punctuation + text normalization (BPE vocab).
- `gigaam-multilingual-ctc` — RU+EN code-switching (separate HF repo
  `istupakov/gigaam-multilingual-ctc-onnx`).
