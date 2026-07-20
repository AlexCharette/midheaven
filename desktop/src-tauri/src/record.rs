//! Native microphone capture to a WAV file. A cpal `Stream` is `!Send`, so a
//! dedicated thread owns it end to end; the [`Recorder`] handle (a feed
//! channel + join handle) is what crosses threads. The WAV is written at the
//! device's native rate/channels — `astro::transcribe::load_wav_mono_16k`
//! normalizes at transcription time.
//!
//! The audio callback does no disk I/O: it ships each buffer over a channel
//! to the recorder thread, which owns the writer — a disk stall can no longer
//! block the callback and drop input. (A small per-callback Vec allocation
//! remains — ~100ns against the callback's multi-ms budget; a lock-free ring
//! buffer would remove it at the cost of a dependency.)

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;

enum Feed {
    Data(Vec<f32>),
    Stop,
}

pub struct Recorder {
    feed_tx: mpsc::Sender<Feed>,
    handle: JoinHandle<Result<(PathBuf, f64), String>>,
}

impl Recorder {
    /// Stop capturing, finalize the WAV, and return (path, seconds recorded).
    pub fn stop(self) -> Result<(PathBuf, f64), String> {
        let _ = self.feed_tx.send(Feed::Stop);
        self.handle.join().map_err(|_| "recording thread panicked".to_string())?
    }
}

type Writer = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

/// Begin capturing from the default input device into `out`. Returns once the
/// stream is actually running (setup errors surface here, not at stop).
pub fn start(out: PathBuf) -> Result<Recorder, String> {
    let (feed_tx, feed_rx) = mpsc::channel::<Feed>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let callback_tx = feed_tx.clone();

    let handle = std::thread::spawn(move || -> Result<(PathBuf, f64), String> {
        // The whole setup in one fallible closure → one error-reporting site.
        let setup = || -> Result<(cpal::Stream, Writer, u32, u16), String> {
            let device = cpal::default_host()
                .default_input_device()
                .ok_or("no audio input device found")?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("cannot query the input device: {e}"))?;
            let channels = config.channels();
            let sample_rate = config.sample_rate();
            let stream_config: cpal::StreamConfig = config.config();
            let writer = hound::WavWriter::create(
                &out,
                hound::WavSpec {
                    channels,
                    sample_rate,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            )
            .map_err(|e| format!("cannot create {}: {e}", out.display()))?;

            let err_fn = |e| eprintln!("recording stream error: {e}");
            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    let tx = callback_tx.clone();
                    device.build_input_stream(
                        stream_config,
                        move |data: &[f32], _| {
                            let _ = tx.send(Feed::Data(data.to_vec()));
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let tx = callback_tx.clone();
                    device.build_input_stream(
                        stream_config,
                        move |data: &[i16], _| {
                            let floats = data.iter().map(|&s| f32::from(s) / 32_768.0).collect();
                            let _ = tx.send(Feed::Data(floats));
                        },
                        err_fn,
                        None,
                    )
                }
                other => return Err(format!("unsupported input sample format: {other:?}")),
            }
            .map_err(|e| format!("cannot open the input stream: {e}"))?;
            stream.play().map_err(|e| format!("cannot start recording: {e}"))?;
            Ok((stream, writer, sample_rate, channels))
        };

        let (stream, mut writer, sample_rate, channels) = match setup() {
            Ok(ok) => ok,
            Err(e) => {
                let _ = ready_tx.send(Err(e.clone()));
                return Err(e);
            }
        };
        let _ = ready_tx.send(Ok(()));

        let mut samples: u64 = 0;
        let mut write = |writer: &mut Writer, buf: Vec<f32>| {
            for s in &buf {
                let _ = writer.write_sample(*s);
            }
            samples += buf.len() as u64;
        };
        loop {
            match feed_rx.recv() {
                Ok(Feed::Data(buf)) => write(&mut writer, buf),
                Ok(Feed::Stop) | Err(_) => break,
            }
        }
        drop(stream); // stop callbacks, then flush whatever they already sent
        while let Ok(Feed::Data(buf)) = feed_rx.try_recv() {
            write(&mut writer, buf);
        }
        drop(write);

        let secs = samples as f64 / f64::from(sample_rate) / f64::from(channels);
        writer.finalize().map_err(|e| format!("cannot finalize the recording: {e}"))?;
        Ok((out, secs))
    });

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(Recorder { feed_tx, handle }),
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
