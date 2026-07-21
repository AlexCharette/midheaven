//! Stage 1 — local, cross-platform speech-to-text via whisper.cpp
//! (`whisper-rs`, CPU everywhere, Metal on macOS). Models are user-supplied
//! ggml files; nothing is downloaded at runtime.
//!
//! Input is WAV only (any rate/channels; downmixed and resampled here) — a
//! deliberate license boundary: pure-Rust compressed-audio decoding would
//! pull in MPL-2.0 code, which the brief's permissive-only rule excludes.
//! For m4a/mp3 recordings: `ffmpeg -i call.m4a -ar 16000 -ac 1 call.wav`.

use crate::contract::Segment;
use std::path::Path;
#[cfg(feature = "transcribe")]
use std::path::PathBuf;
#[cfg(feature = "transcribe")]
use std::sync::Mutex;
#[cfg(feature = "transcribe")]
use std::time::SystemTime;
#[cfg(feature = "transcribe")]
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper's fixed input sample rate.
#[cfg(feature = "transcribe")]
const WHISPER_RATE: u32 = 16_000;

/// Does this file look like audio we can transcribe? Decided by content
/// (RIFF/WAVE magic), with the extension as a fallback for unreadable paths —
/// this module owns the definition of "audio", not its callers.
pub fn is_audio(path: &Path) -> bool {
    match std::fs::File::open(path) {
        // readable content decides definitively
        Ok(mut f) => {
            let mut header = [0u8; 12];
            std::io::Read::read_exact(&mut f, &mut header).is_ok()
                && &header[..4] == b"RIFF"
                && &header[8..] == b"WAVE"
        }
        // unreadable paths fall back to the extension
        Err(_) => path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav") || e.eq_ignore_ascii_case("wave")),
    }
}

/// (audio path, model path, audio mtime) — identifies one transcription run.
#[cfg(feature = "transcribe")]
type CacheKey = (PathBuf, PathBuf, Option<SystemTime>);

/// Single-slot result cache: transcription is by far the most expensive step
/// (minutes for an hour of audio), and "tweak a birth field, resubmit the
/// same recording" is a common loop in the desktop app.
#[cfg(feature = "transcribe")]
static LAST_RUN: Mutex<Option<(CacheKey, Vec<Segment>)>> = Mutex::new(None);

#[cfg(feature = "transcribe")]
fn cache_key(audio: &Path, model: &Path) -> CacheKey {
    let mtime = std::fs::metadata(audio).and_then(|m| m.modified()).ok();
    (audio.to_path_buf(), model.to_path_buf(), mtime)
}

/// Transcribe a WAV file with a ggml whisper model. `progress` receives
/// whole percentages from whisper.cpp as inference advances.
#[cfg(feature = "transcribe")]
pub fn transcribe(
    audio: &Path,
    model: &Path,
    progress: impl FnMut(i32) + Send + 'static,
) -> Result<Vec<Segment>, String> {
    if !model.exists() {
        return Err(format!(
            "no model at {} — download a ggml whisper model (see README)",
            model.display()
        ));
    }
    let key = cache_key(audio, model);
    if let Some((last_key, segments)) = &*LAST_RUN.lock().unwrap()
        && *last_key == key
    {
        return Ok(segments.clone());
    }

    let samples = load_wav_mono_16k(audio)?;

    whisper_rs::install_logging_hooks(); // keep whisper.cpp chatter off stderr
    let ctx = WhisperContext::new_with_params(model, WhisperContextParameters::default())
        .map_err(|e| format!("cannot load model {}: {e}", model.display()))?;
    let mut state = ctx.create_state().map_err(|e| format!("whisper state: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_translate(false);
    params.set_language(Some("auto"));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get() as i32);
    params.set_n_threads(threads);
    params.set_progress_callback_safe(progress);

    state
        .full(params, &samples)
        .map_err(|e| format!("transcription failed: {e}"))?;

    let mut segments = Vec::new();
    for i in 0..state.full_n_segments() {
        let Some(seg) = state.get_segment(i) else { continue };
        let text = seg.to_str_lossy().map_err(|e| format!("segment {i}: {e}"))?;
        let text = text.trim().to_string();
        if !text.is_empty() {
            // start_timestamp is in centiseconds
            segments.push(Segment { start: seg.start_timestamp() as f64 / 100.0, text });
        }
    }
    *LAST_RUN.lock().unwrap() = Some((key, segments.clone()));
    Ok(segments)
}

/// The JSONL form of a transcript — exactly what `route::Transcript::load`
/// parses back.
pub fn to_jsonl(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| serde_json::to_string(s).expect("segment serializes") + "\n")
        .collect()
}

/// Decode a WAV file of any rate/channel count into mono 16 kHz f32 samples.
/// Decode and downmix are fused into one pass (an hour of stereo 44.1 kHz is
/// ~1.3 GB as interleaved f32 — never materialized).
#[cfg(feature = "transcribe")]
pub fn load_wav_mono_16k(path: &Path) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::open(path).map_err(|e| {
        format!(
            "cannot read {} as WAV: {e}\n(compressed audio? convert first: \
             ffmpeg -i input.m4a -ar 16000 -ac 1 output.wav)",
            path.display()
        )
    })?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let frames = reader.duration() as usize;

    let mono = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, _) => {
            fold_mono(reader.into_samples::<f32>().map(|r| r.map_err(|e| e.to_string())), channels, frames)?
        }
        (hound::SampleFormat::Int, bits) => {
            let scale = (1i64 << (bits - 1)) as f32;
            fold_mono(
                reader
                    .into_samples::<i32>()
                    .map(|r| r.map(|v| v as f32 / scale).map_err(|e| e.to_string())),
                channels,
                frames,
            )?
        }
    };
    Ok(resample_linear(mono, spec.sample_rate, WHISPER_RATE))
}

/// Average interleaved samples into mono frames, streaming.
#[cfg(feature = "transcribe")]
fn fold_mono(
    samples: impl Iterator<Item = Result<f32, String>>,
    channels: usize,
    frames: usize,
) -> Result<Vec<f32>, String> {
    let mut mono = Vec::with_capacity(frames);
    let mut acc = 0.0f32;
    let mut ch = 0;
    for sample in samples {
        acc += sample?;
        ch += 1;
        if ch == channels {
            mono.push(acc / channels as f32);
            acc = 0.0;
            ch = 0;
        }
    }
    Ok(mono)
}

/// Linear-interpolation resampling — adequate for speech into whisper.
/// The already-16k case (the documented ffmpeg recipe) is a move, not a copy.
#[cfg(feature = "transcribe")]
fn resample_linear(samples: Vec<f32>, from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() {
        return samples;
    }
    let ratio = from as f64 / to as f64;
    let out_len = ((samples.len() as f64) / ratio).floor() as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = samples[idx];
            let b = samples.get(idx + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

// These exercise the WAV loader and whisper path, so they need the feature's
// deps (hound); the `is_audio`/`to_jsonl` helpers stay covered here too.
#[cfg(all(test, feature = "transcribe"))]
mod tests {
    use super::*;
    use crate::route::Transcript;

    /// Write a WAV of `n` frames, every sample set to `value`.
    fn write_test_wav(name: &str, channels: u16, rate: u32, value: i16, n: usize) -> PathBuf {
        let dir = std::env::temp_dir().join("astro-test-wav");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..n * channels as usize {
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();
        path
    }

    #[test]
    fn resampler_preserves_constant_signal_and_length_ratio() {
        let out = resample_linear(vec![0.5f32; 44_100], 44_100, 16_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1, "len {}", out.len());
        assert!(out.iter().all(|s| (s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn fold_mono_averages_stereo_frames() {
        let interleaved = [1.0, 0.0, 0.0, 1.0, 0.5, 0.5].map(Ok);
        assert_eq!(fold_mono(interleaved.into_iter(), 2, 3).unwrap(), vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn wav_loader_handles_stereo_44k() {
        let path = write_test_wav("stereo44k.wav", 2, 44_100, 8192, 44_100); // 0.25 amplitude
        let samples = load_wav_mono_16k(&path).unwrap();
        assert!((samples.len() as i64 - 16_000).abs() <= 1, "len {}", samples.len());
        assert!(samples.iter().all(|s| (s - 0.25).abs() < 1e-3));
    }

    #[test]
    fn is_audio_decides_by_content_then_extension() {
        let wav = write_test_wav("magic.wav", 1, 16_000, 0, 16);
        assert!(is_audio(&wav));
        // real WAV content wins even with a misleading extension
        let renamed = wav.with_extension("dat");
        std::fs::copy(&wav, &renamed).unwrap();
        assert!(is_audio(&renamed));
        // text content is not audio, whatever it is called
        let fake = std::env::temp_dir().join("astro-test-wav/fake.wav");
        std::fs::write(&fake, "just words").unwrap();
        assert!(!is_audio(&fake));
        // unreadable path falls back to the extension
        assert!(is_audio(Path::new("/nonexistent/recording.WAV")));
        assert!(!is_audio(Path::new("/nonexistent/notes.txt")));
    }

    #[test]
    fn jsonl_round_trips_through_route_transcript() {
        let segments = vec![
            Segment { start: 0.0, text: "Hello there.".into() },
            Segment { start: 75.5, text: "Your sun is in cancer.".into() },
        ];
        let jsonl = to_jsonl(&segments);
        let t = Transcript::load(&jsonl);
        assert_eq!(t.text, "Hello there. Your sun is in cancer.");
        assert_eq!(t.time_at(13), "00:01:15");
    }

    /// Real inference needs a model file: set ASTRO_WHISPER_MODEL to a local
    /// ggml path and run `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a local ggml model via ASTRO_WHISPER_MODEL"]
    fn whisper_transcribes_silence_without_error() {
        let model = std::env::var("ASTRO_WHISPER_MODEL").expect("set ASTRO_WHISPER_MODEL");
        let path = write_test_wav("silence.wav", 1, 16_000, 0, 16_000);
        let segments = transcribe(&path, model.as_ref(), |_| {}).unwrap();
        // a second of silence may produce zero or hallucinated segments;
        // the contract here is only "no error"
        let _ = segments;
    }
}
