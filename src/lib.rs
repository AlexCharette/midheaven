//! An offline astrology workspace — turn a recorded birth-chart reading into a single
//! offline HTML artifact where the astrologer's *verbatim* words are filed
//! under the chart elements they refer to.
//!
//! Four-stage pipeline (the original brief, now historical, is
//! `docs/natal-reading-indexer.md`; `CONTEXT.md` is the current account):
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
//! stage owns it. [`mod@derive`] is the same kind of module for behaviour rather
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
pub mod vocab;

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
    chart::BirthInput {
        name: locale.name_or_anonymous(name).to_string(),
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

/// What a full build produced besides the chart and its notes: the span count
/// the router emitted before gating, and any non-fatal warnings (a
/// DST-ambiguous birth time, Verify-gate rejections) for the caller to
/// surface.
///
/// It used to hand the transcript back too, as an `ArchivedTranscript` with a
/// filename, because a saved reading had to keep the words its notes' spans
/// pointed into — without them provenance could not be re-checked. Notes do
/// not carry spans and readings are not folders, so there is nothing left for
/// a stored transcript to be checked against.
#[derive(Debug, Default)]
pub struct BuildReport {
    pub n_routed: usize,
    pub warnings: Vec<String>,
}

/// The whole pipeline in one call: obtain the transcript (reading a file, or
/// transcribing audio while reporting whole-percent `progress`), compute the
/// chart, route + verify the notes. Returns the chart, the notes, and a
/// [`BuildReport`]. This is the single entry point every frontend shares.
///
/// The notes come back *beside* the chart rather than on it, numbered `x1`,
/// `x2`, … — see [`numbered`] for why this call does that when `route` does
/// not.
pub fn build_reading(
    input: &chart::BirthInput,
    source: TranscriptSource,
    progress: impl FnMut(i32) + Send + 'static,
) -> Result<(contract::ChartData, Vec<contract::Note>, BuildReport), String> {
    // `progress` is only consumed by the audio arm; without the feature that
    // arm is gone, so drop the callback to keep it from reading as unused.
    #[cfg(not(feature = "transcribe"))]
    let _ = progress;
    let transcript = match source {
        TranscriptSource::None => None,
        TranscriptSource::File(path) => {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            Some(route::Transcript::load(&contents))
        }
        #[cfg(feature = "transcribe")]
        TranscriptSource::Audio { wav, model } => {
            let lang = input.locale.whisper_lang();
            let segments = transcribe::transcribe(&wav, &model, Some(lang), progress)?;
            // Straight from the segments, skipping the JSONL round trip.
            Some(route::Transcript::from_segments(segments))
        }
    };
    compute_and_route(input, transcript)
}

/// Give unnumbered notes the conventional `x{n}` ids, in order.
///
/// `route` deliberately leaves `id` empty: a store mints its own, and
/// numbering there would be a name immediately thrown away. But
/// `build_reading` is the whole pipeline for callers that have no store — the
/// CLI emits an artifact straight from it — and the artifact uses a note's id
/// as a DOM id. An empty one is not an error anywhere; it is a page of
/// elements all sharing the id `""`.
fn numbered(mut notes: Vec<contract::Note>) -> Vec<contract::Note> {
    for (i, note) in notes.iter_mut().enumerate() {
        note.id = format!("x{}", i + 1);
    }
    notes
}

fn compute_and_route(
    input: &chart::BirthInput,
    transcript: Option<route::Transcript>,
) -> Result<(contract::ChartData, Vec<contract::Note>, BuildReport), String> {
    let (chart, mut warnings) = chart::compute_chart_reporting(input)?;
    let mut notes = Vec::new();
    let mut n_routed = 0;
    if let Some(transcript) = transcript {
        let (routed, report) = route::route(&transcript, input.locale);
        notes = routed;
        n_routed = report.n_routed;
        warnings.extend(report.warnings);
    }
    Ok((chart, numbered(notes), BuildReport { n_routed, warnings }))
}
