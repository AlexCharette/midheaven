//! Stage 1 — local, cross-platform speech-to-text via whisper.cpp
//! (`whisper-rs`, CPU everywhere, Metal on macOS). Models are user-supplied
//! ggml files; nothing is downloaded at runtime.
//!
//! Input is WAV only (any rate/channels; downmixed and resampled here) — a
//! deliberate license boundary: pure-Rust compressed-audio decoding would
//! pull in MPL-2.0 code, which the brief's permissive-only rule excludes.
//! For m4a/mp3 recordings: `ffmpeg -i call.m4a -ar 16000 -ac 1 call.wav`.

use serde::Serialize;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper's fixed input sample rate.
const WHISPER_RATE: u32 = 16_000;

/// One transcribed segment — serializes to the `{"start", "text"}` JSONL
/// lines the routing stage consumes.
#[derive(Debug, Serialize, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub text: String,
}

/// Transcribe a WAV file with a ggml whisper model. `progress` receives
/// whole percentages from whisper.cpp as inference advances.
pub fn transcribe(
    audio: &Path,
    model: &Path,
    progress: impl FnMut(i32) + Send + 'static,
) -> Result<Vec<Segment>, String> {
    let samples = load_wav_mono_16k(audio)?;
    if !model.exists() {
        return Err(format!(
            "no model at {} — download a ggml whisper model (see README)",
            model.display()
        ));
    }

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
    Ok(segments)
}

/// The JSONL form of a transcript — exactly what `route::Transcript::load`
/// parses back.
pub fn to_jsonl(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| serde_json::to_string(s).expect("segment serializes"))
        .fold(String::new(), |mut out, line| {
            out.push_str(&line);
            out.push('\n');
            out
        })
}

/// Decode a WAV file of any rate/channel count into mono 16 kHz f32 samples.
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

    // Normalize every encoding to f32 in [-1, 1].
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, _) => {
            reader.into_samples::<f32>().collect::<Result<_, _>>().map_err(|e| e.to_string())?
        }
        (hound::SampleFormat::Int, bits) => {
            let scale = (1i64 << (bits - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        }
    };

    let mono = downmix(&interleaved, channels);
    Ok(resample_linear(&mono, spec.sample_rate, WHISPER_RATE))
}

/// Average interleaved channels into mono.
fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear-interpolation resampling — adequate for speech into whisper.
fn resample_linear(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() {
        return samples.to_vec();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::Transcript;

    #[test]
    fn resampler_preserves_constant_signal_and_length_ratio() {
        let input = vec![0.5f32; 44_100];
        let out = resample_linear(&input, 44_100, 16_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1, "len {}", out.len());
        assert!(out.iter().all(|s| (s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn downmix_averages_stereo() {
        let interleaved = [1.0, 0.0, 0.0, 1.0, 0.5, 0.5];
        assert_eq!(downmix(&interleaved, 2), vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn wav_loader_handles_stereo_44k(){
        let dir = std::env::temp_dir().join("astro-test-wav");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stereo44k.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..44_100 {
            writer.write_sample(8192i16).unwrap(); // 0.25 in both channels
            writer.write_sample(8192i16).unwrap();
        }
        writer.finalize().unwrap();

        let samples = load_wav_mono_16k(&path).unwrap();
        assert!((samples.len() as i64 - 16_000).abs() <= 1, "len {}", samples.len());
        assert!(samples.iter().all(|s| (s - 0.25).abs() < 1e-3));
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
        let dir = std::env::temp_dir().join("astro-test-wav");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("silence.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..16_000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        let segments = transcribe(&path, model.as_ref(), |_| {}).unwrap();
        // a second of silence may produce zero or hallucinated segments;
        // the contract here is only "no error"
        let _ = segments;
    }
}
