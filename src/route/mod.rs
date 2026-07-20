//! Stage 3 — route verbatim transcript spans to chart elements.
//!
//! Routers are behind the [`Router`] trait so the deterministic lexicon
//! matcher can later be joined by a local-LLM closed-set classifier
//! (`route::llm`). Whatever a router emits, the Verify gate (total) is
//! enforced by the pipeline, never skipped: a span's text must be a verbatim
//! slice of the transcript, and every tag must exist in the chart vocabulary.

mod coalesce;
mod lexicon;
mod transcript;
mod verify;

pub use coalesce::coalesce;
pub use lexicon::LexiconRouter;
pub use transcript::Transcript;
pub use verify::verify_gate;

/// What a router emits: spans + tags, never text of its own.
pub struct RawExcerpt {
    pub span: (usize, usize),
    pub tags: Vec<String>,
}

pub trait Router {
    fn route(&self, transcript: &Transcript) -> Vec<RawExcerpt>;
}

/// Route a transcript into `chart.excerpts` with the Verify gate applied —
/// the one path from router output into the artifact, so the gate cannot be
/// skipped. Returns the number of spans the router emitted (before gating).
pub fn index_transcript(
    chart: &mut crate::contract::ChartData,
    transcript: &Transcript,
    router: &dyn Router,
) -> usize {
    let vocab = chart.vocab();
    let raw = router.route(transcript);
    let n_routed = raw.len();
    chart.excerpts = coalesce(verify_gate(transcript, raw, &vocab), transcript);
    n_routed
}

#[cfg(test)]
pub(crate) fn test_vocab(tags: &[&str]) -> std::collections::BTreeSet<String> {
    tags.iter().map(|s| s.to_string()).collect()
}
