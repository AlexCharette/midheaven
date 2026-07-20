//! Natal reading indexer — turn a recorded birth-chart reading into a single
//! offline HTML artifact where the astrologer's *verbatim* words are filed
//! under the chart elements they refer to.
//!
//! Four-stage pipeline (build brief: `docs/natal-reading-indexer.md`):
//! 1. **Transcribe** — [`transcribe`]: local whisper.cpp over a WAV file
//!    (cross-platform, user-supplied ggml model); external transcripts are
//!    equally welcome as plain text or timestamped JSONL via
//!    [`route::Transcript`].
//! 2. **Compute** — [`chart::compute_chart`]: birth data → tropical Whole Sign
//!    chart, fully offline (analytic ephemeris, embedded gazetteer in [`geo`]).
//! 3. **Route** — [`route`]: a [`route::Router`] tags verbatim spans with the
//!    chart-derived vocabulary; [`route::verify_gate`] enforces provenance.
//! 4. **Emit** — [`emit`]: inject the assembled [`contract::ChartData`] into
//!    the self-contained HTML viewer.
//!
//! [`contract`] holds the `ChartData` types — the contract between stages; no
//! stage owns it.

pub mod chart;
pub mod contract;
pub mod emit;
pub mod geo;
pub mod route;
pub mod transcribe;

/// Where a reading's transcript comes from. `Audio` encodes the
/// "a recording needs a model" invariant in the type; [`TranscriptSource::classify`]
/// owns the decision procedure so no frontend re-states it.
pub enum TranscriptSource {
    None,
    /// A transcript file: plain text or timestamped JSONL.
    File(std::path::PathBuf),
    /// A WAV recording to transcribe with a ggml whisper model.
    Audio { wav: std::path::PathBuf, model: std::path::PathBuf },
}

/// Why a transcript/model pair cannot be classified — structured so form
/// frontends can attach each failure to the right field.
#[derive(Debug, PartialEq)]
pub enum ClassifyError {
    NoTranscriptFile(String),
    ModelRequired,
    NoModelFile(String),
}

impl std::fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassifyError::NoTranscriptFile(p) => write!(f, "no file at {p}"),
            ClassifyError::ModelRequired => {
                write!(f, "an audio transcript needs a ggml whisper model")
            }
            ClassifyError::NoModelFile(p) => write!(f, "no model file at {p}"),
        }
    }
}

impl TranscriptSource {
    /// Classify free-form transcript/model inputs: empty → `None`; audio
    /// (decided by content via [`transcribe::is_audio`]) requires an existing
    /// model file; anything else is a transcript file.
    pub fn classify(transcript: &str, model: &str) -> Result<TranscriptSource, ClassifyError> {
        let transcript = transcript.trim();
        if transcript.is_empty() {
            return Ok(TranscriptSource::None);
        }
        let path = std::path::Path::new(transcript);
        if !path.exists() {
            return Err(ClassifyError::NoTranscriptFile(transcript.into()));
        }
        if !transcribe::is_audio(path) {
            return Ok(TranscriptSource::File(path.into()));
        }
        let model = model.trim();
        if model.is_empty() {
            return Err(ClassifyError::ModelRequired);
        }
        let model_path = std::path::Path::new(model);
        if !model_path.exists() {
            return Err(ClassifyError::NoModelFile(model.into()));
        }
        Ok(TranscriptSource::Audio { wav: path.into(), model: model_path.into() })
    }
}

/// Assemble a [`chart::BirthInput`] from a gazetteer place — the one home
/// for the blank-name default and the Place field mapping.
pub fn birth_at_place(
    name: &str,
    date: chrono::NaiveDate,
    time: chrono::NaiveTime,
    place: &geo::Place,
) -> chart::BirthInput {
    let name = name.trim();
    chart::BirthInput {
        name: if name.is_empty() { chart::DEFAULT_NAME.into() } else { name.into() },
        date,
        time,
        lat: place.lat,
        lon: place.lon,
        tz: place.tz,
        place: place.label(),
    }
}

/// The whole pipeline in one call: obtain the transcript (reading a file, or
/// transcribing audio while reporting whole-percent `progress`), compute the
/// chart, route + verify passages into `excerpts`. Returns the chart and the
/// number of spans the router emitted before gating. This is the single
/// entry point the CLI, the TUI, and tests share.
pub fn build_reading(
    input: &chart::BirthInput,
    source: TranscriptSource,
    progress: impl FnMut(i32) + Send + 'static,
) -> Result<(contract::ChartData, usize), String> {
    let transcript = match source {
        TranscriptSource::None => None,
        TranscriptSource::File(path) => {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            Some(route::Transcript::load(&raw))
        }
        TranscriptSource::Audio { wav, model } => {
            let segments = transcribe::transcribe(&wav, &model, progress)?;
            Some(route::Transcript::from_segments(segments))
        }
    };
    route_into_chart(input, transcript)
}

fn route_into_chart(
    input: &chart::BirthInput,
    transcript: Option<route::Transcript>,
) -> Result<(contract::ChartData, usize), String> {
    let mut chart = chart::compute_chart(input)?;
    let mut n_routed = 0;
    if let Some(transcript) = transcript {
        let router = route::LexiconRouter::new(&chart.vocab(), &chart.aspects);
        n_routed = route::index_transcript(&mut chart, &transcript, &router);
    }
    Ok((chart, n_routed))
}
