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

/// The whole pipeline in one call: compute the chart, then (when a transcript
/// is given) route + verify its passages into `excerpts`. Returns the chart
/// and the number of spans the router emitted before gating. This is the
/// single entry point the CLI, the TUI, and tests share.
pub fn build_reading(
    input: &chart::BirthInput,
    transcript: Option<&std::path::Path>,
) -> Result<(contract::ChartData, usize), String> {
    let mut chart = chart::compute_chart(input)?;
    let mut n_routed = 0;
    if let Some(path) = transcript {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let transcript = route::Transcript::load(&raw);
        let router = route::LexiconRouter::new(&chart.vocab(), &chart.aspects);
        n_routed = route::index_transcript(&mut chart, &transcript, &router);
    }
    Ok((chart, n_routed))
}
