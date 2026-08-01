//! An offline astrology workspace — turn a recorded birth-chart reading into a single
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
//! stage owns it. [`derive`] is the same kind of module for behaviour rather
//! than data: the longitude arithmetic every renderer would otherwise
//! re-derive, computed once so its results can ride on `ChartData` fields.
//! [`plate`] is the third of that kind: the engraved wheel's geometry, stated
//! once for the three renditions that draw it.

pub mod chart;
pub mod contract;
pub mod derive;
pub mod emit;
#[cfg(any(test, feature = "testing"))]
pub mod fixtures;
pub mod geo;
pub mod i18n;
pub mod pdf;
pub mod plate;
pub mod route;
pub mod transcribe;

/// Where a reading's transcript comes from. `Audio` encodes the
/// "a recording needs a model" invariant in the type; [`TranscriptSource::classify`]
/// owns the decision for *free-form* string inputs (the desktop's text fields),
/// so those frontends never re-state it. The CLI builds this from typed clap
/// flags instead, sharing only the model-required error ([`ClassifyError`]).
pub enum TranscriptSource {
    None,
    /// A transcript file: plain text or timestamped JSONL.
    File(std::path::PathBuf),
    /// A WAV recording to transcribe with a ggml whisper model. Only present
    /// in builds with the `transcribe` feature (off on mobile).
    #[cfg(feature = "transcribe")]
    Audio { wav: std::path::PathBuf, model: std::path::PathBuf },
}

/// Why a transcript/model pair cannot be classified — structured so form
/// frontends can attach each failure to the right field.
#[derive(Debug, PartialEq)]
pub enum ClassifyError {
    NoTranscriptFile(String),
    ModelRequired,
    NoModelFile(String),
    /// This build has no on-device transcription (e.g. mobile) yet was handed
    /// an audio file — the caller should offer a text/JSONL transcript instead.
    TranscriptionUnavailable,
}

impl std::fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassifyError::NoTranscriptFile(p) => write!(f, "no file at {p}"),
            ClassifyError::ModelRequired => {
                write!(f, "an audio transcript needs a ggml whisper model")
            }
            ClassifyError::NoModelFile(p) => write!(f, "no model file at {p}"),
            ClassifyError::TranscriptionUnavailable => {
                write!(f, "audio transcription is not available in this build")
            }
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
        // From here down the input is audio, which only the `transcribe` build
        // can consume. Without the feature, refuse rather than mis-file a WAV
        // as a text transcript.
        #[cfg(not(feature = "transcribe"))]
        {
            let _ = model;
            Err(ClassifyError::TranscriptionUnavailable)
        }
        #[cfg(feature = "transcribe")]
        {
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
}

/// Assemble a [`chart::BirthInput`] from a gazetteer place — the one home
/// for the blank-name default and the Place field mapping.
pub fn birth_at_place(
    name: &str,
    date: chrono::NaiveDate,
    time: chrono::NaiveTime,
    place: &geo::Place,
    locale: i18n::Locale,
    house_system: xalen_houses::HouseSystem,
    ayanamsa: Option<xalen_ayanamsa::Ayanamsa>,
) -> chart::BirthInput {
    let name = name.trim();
    chart::BirthInput {
        name: if name.is_empty() { locale.anonymous().into() } else { name.into() },
        date,
        time,
        lat: place.lat,
        lon: place.lon,
        tz: place.tz,
        place: place.label(),
        locale,
        house_system,
        ayanamsa,
    }
}

/// The words a build routed from, in the form they should be archived: a
/// filename following the source's kind (`transcript.jsonl` for a recording,
/// `transcript.{ext}` for a file) and their verbatim contents.
///
/// A library that saves a reading has to save these too — a passage is verbatim
/// by definition, and without the transcript its span points at nothing, so
/// provenance can no longer be re-checked. [`build_reading`] therefore hands the
/// transcript back rather than consuming it: the desktop used to fork the whole
/// transcript-acquisition match for want of this one value.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchivedTranscript {
    pub filename: String,
    pub contents: String,
}

/// What a full build produced besides the chart: the span count the router
/// emitted before gating, any non-fatal warnings (a DST-ambiguous birth
/// time, Verify-gate rejections) for the caller to surface, and the transcript
/// it routed from, for callers that persist the reading.
#[derive(Debug, Default)]
pub struct BuildReport {
    pub n_routed: usize,
    pub warnings: Vec<String>,
    /// `None` when the build had no transcript at all.
    pub transcript: Option<ArchivedTranscript>,
}

/// The whole pipeline in one call: obtain the transcript (reading a file, or
/// transcribing audio while reporting whole-percent `progress`), compute the
/// chart, route + verify passages into `excerpts`. Returns the chart and a
/// [`BuildReport`]. This is the single entry point every frontend shares.
pub fn build_reading(
    input: &chart::BirthInput,
    source: TranscriptSource,
    progress: impl FnMut(i32) + Send + 'static,
) -> Result<(contract::ChartData, BuildReport), String> {
    // `progress` is only consumed by the audio arm; without the feature that
    // arm is gone, so drop the callback to keep it from reading as unused.
    #[cfg(not(feature = "transcribe"))]
    let _ = progress;
    let (transcript, archived) = match source {
        TranscriptSource::None => (None, None),
        TranscriptSource::File(path) => {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("txt");
            let archived =
                ArchivedTranscript { filename: format!("transcript.{ext}"), contents };
            (Some(route::Transcript::load(&archived.contents)), Some(archived))
        }
        #[cfg(feature = "transcribe")]
        TranscriptSource::Audio { wav, model } => {
            let lang = input.locale.whisper_lang();
            let segments = transcribe::transcribe(&wav, &model, Some(lang), progress)?;
            let archived = ArchivedTranscript {
                filename: "transcript.jsonl".to_string(),
                contents: transcribe::to_jsonl(&segments),
            };
            // Straight from the segments, skipping the JSONL round trip.
            (Some(route::Transcript::from_segments(segments)), Some(archived))
        }
    };
    let (chart, mut report) = route_into_chart(input, transcript)?;
    report.transcript = archived;
    Ok((chart, report))
}

fn route_into_chart(
    input: &chart::BirthInput,
    transcript: Option<route::Transcript>,
) -> Result<(contract::ChartData, BuildReport), String> {
    let (mut chart, mut warnings) = chart::compute_chart_reporting(input)?;
    let mut n_routed = 0;
    if let Some(transcript) = transcript {
        let report = route::route_into(&mut chart, &transcript, route::Filing::Fresh);
        n_routed = report.n_routed;
        warnings.extend(report.warnings);
    }
    Ok((chart, BuildReport { n_routed, warnings, transcript: None }))
}
