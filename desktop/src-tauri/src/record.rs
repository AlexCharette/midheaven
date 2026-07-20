//! Native microphone capture to a WAV file. A cpal `Stream` is `!Send`, so a
//! dedicated thread owns it end to end; the [`Recorder`] handle (a stop
//! channel + join handle) is what crosses threads. The WAV is written at the
//! device's native rate/channels — `astro::transcribe::load_wav_mono_16k`
//! normalizes at transcription time.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct Recorder {
    stop_tx: mpsc::Sender<()>,
    handle: JoinHandle<Result<(PathBuf, f64), String>>,
}

impl Recorder {
    /// Stop capturing, finalize the WAV, and return (path, seconds recorded).
    pub fn stop(self) -> Result<(PathBuf, f64), String> {
        let _ = self.stop_tx.send(());
        self.handle.join().map_err(|_| "recording thread panicked".to_string())?
    }
}

/// Begin capturing from the default input device into `out`. Returns once the
/// stream is actually running (setup errors surface here, not at stop).
pub fn start(out: PathBuf) -> Result<Recorder, String> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let handle = std::thread::spawn(move || -> Result<(PathBuf, f64), String> {
        let setup = || -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
            let device = cpal::default_host()
                .default_input_device()
                .ok_or("no audio input device found")?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("cannot query the input device: {e}"))?;
            Ok((device, config))
        };
        let (device, config) = match setup() {
            Ok(dc) => dc,
            Err(e) => {
                let _ = ready_tx.send(Err(e.clone()));
                return Err(e);
            }
        };

        let channels = config.channels();
        let sample_rate = config.sample_rate();
        let stream_config: cpal::StreamConfig = config.config();
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = match hound::WavWriter::create(&out, spec) {
            Ok(w) => Arc::new(Mutex::new(Some(w))),
            Err(e) => {
                let e = format!("cannot create {}: {e}", out.display());
                let _ = ready_tx.send(Err(e.clone()));
                return Err(e);
            }
        };
        let samples_written = Arc::new(AtomicU64::new(0));

        let err_fn = |e| eprintln!("recording stream error: {e}");
        let stream = {
            let writer = writer.clone();
            let counter = samples_written.clone();
            let write_f32 = move |data: &[f32]| {
                if let Some(w) = writer.lock().unwrap().as_mut() {
                    for &s in data {
                        let _ = w.write_sample(s);
                    }
                    counter.fetch_add(data.len() as u64, Ordering::Relaxed);
                }
            };
            match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    stream_config.clone(),
                    move |data: &[f32], _| write_f32(data),
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    stream_config.clone(),
                    move |data: &[i16], _| {
                        let floats: Vec<f32> =
                            data.iter().map(|&s| f32::from(s) / 32_768.0).collect();
                        write_f32(&floats);
                    },
                    err_fn,
                    None,
                ),
                other => {
                    let e = format!("unsupported input sample format: {other:?}");
                    let _ = ready_tx.send(Err(e.clone()));
                    return Err(e);
                }
            }
        }
        .map_err(|e| format!("cannot open the input stream: {e}"))
        .and_then(|s| {
            s.play().map_err(|e| format!("cannot start recording: {e}"))?;
            Ok(s)
        });
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(e.clone()));
                return Err(e);
            }
        };

        let _ = ready_tx.send(Ok(()));
        let _ = stop_rx.recv(); // park until stop (or the Recorder is dropped)
        drop(stream);

        let secs =
            samples_written.load(Ordering::Relaxed) as f64 / f64::from(sample_rate) / f64::from(channels);
        if let Some(w) = writer.lock().unwrap().take() {
            w.finalize().map_err(|e| format!("cannot finalize the recording: {e}"))?;
        }
        Ok((out, secs))
    });

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(Recorder { stop_tx, handle }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("recording thread died during setup".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a real input device — run manually with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs an audio input device"]
    fn one_second_capture_produces_a_readable_wav() {
        let out = std::env::temp_dir().join("astro-record-smoke.wav");
        let recorder = start(out.clone()).expect("recording starts");
        std::thread::sleep(std::time::Duration::from_secs(1));
        let (path, secs) = recorder.stop().expect("recording stops");
        assert!(secs > 0.5, "recorded {secs}s");
        let samples = astro::transcribe::load_wav_mono_16k(&path).expect("wav loads");
        assert!(samples.len() > 8_000, "{} samples", samples.len());
    }
}
