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

/// What routing a transcript produced, apart from the passages themselves:
/// how many spans the router emitted before gating, and any provenance
/// warnings (spans/tags the Verify gate rejected) for a frontend to surface.
#[derive(Debug, Default)]
pub struct RouteReport {
    pub n_routed: usize,
    pub warnings: Vec<String>,
}

/// The default router for a chart — the one place its configuration lives.
/// The router matches in the chart's own language (`meta.locale`), so routing
/// a reloaded `chart.json` re-tags in the language it was read in.
pub fn lexicon_for(chart: &crate::contract::ChartData) -> LexiconRouter {
    let loc = crate::i18n::Locale::parse(&chart.meta.locale);
    LexiconRouter::new(&chart.vocab(), &chart.aspects, loc)
}

/// The one gated path from router output to passages: route → Verify gate →
/// coalesce. Returns the passages and a [`RouteReport`] (raw span count +
/// rejection warnings).
pub fn route_excerpts(
    chart: &crate::contract::ChartData,
    transcript: &Transcript,
    router: &dyn Router,
) -> (Vec<crate::contract::Excerpt>, RouteReport) {
    let vocab = chart.vocab();
    let raw = router.route(transcript);
    let n_routed = raw.len();
    let (accepted, warnings) = verify_gate(transcript, raw, &vocab);
    (coalesce(accepted, transcript), RouteReport { n_routed, warnings })
}

/// Route a transcript into `chart.excerpts`, replacing them. Returns the
/// [`RouteReport`] (span count + any Verify-gate warnings).
pub fn index_transcript(
    chart: &mut crate::contract::ChartData,
    transcript: &Transcript,
    router: &dyn Router,
) -> RouteReport {
    let (excerpts, report) = route_excerpts(chart, transcript, router);
    chart.excerpts = excerpts;
    report
}

/// Route an additional transcript (e.g. a live take) and append its passages
/// after the chart's existing ones, ids renumbered to stay unique.
pub fn append_transcript(
    chart: &mut crate::contract::ChartData,
    transcript: &Transcript,
    router: &dyn Router,
) -> RouteReport {
    let (mut excerpts, report) = route_excerpts(chart, transcript, router);
    renumber(&mut excerpts, next_ordinal(&chart.excerpts));
    chart.excerpts.extend(excerpts);
    report
}

/// The next free `x{n}` ordinal — gap-aware (curation merges and deletions
/// leave holes, so counting entries could mint a duplicate id).
pub fn next_ordinal(excerpts: &[crate::contract::Excerpt]) -> usize {
    excerpts
        .iter()
        .filter_map(|e| e.id.strip_prefix('x').and_then(|n| n.parse::<usize>().ok()))
        .max()
        .unwrap_or(0)
        + 1
}

/// Tags the chart's router finds in free text — curation re-tagging goes
/// through the same gated path as everything else.
pub fn retag(chart: &crate::contract::ChartData, text: &str) -> Vec<String> {
    let transcript = Transcript::load(text);
    let (excerpts, _report) = route_excerpts(chart, &transcript, &lexicon_for(chart));
    let mut tags: Vec<String> = excerpts.into_iter().flat_map(|e| e.tags).collect();
    tags.sort();
    tags.dedup();
    tags
}

/// Assign the conventional `x{n}` ids counting up from `first`.
/// Uniqueness is the contract invariant; density is convention.
pub(crate) fn renumber(excerpts: &mut [crate::contract::Excerpt], first: usize) {
    for (i, ex) in excerpts.iter_mut().enumerate() {
        ex.id = format!("x{}", first + i);
    }
}

#[cfg(test)]
pub(crate) fn test_vocab(tags: &[&str]) -> std::collections::BTreeSet<String> {
    tags.iter().map(|s| s.to_string()).collect()
}
