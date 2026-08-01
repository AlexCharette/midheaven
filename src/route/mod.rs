//! Stage 3 — route verbatim transcript spans to chart elements.
//!
//! Whatever a router emits, the Verify gate (total) is enforced by the
//! pipeline, never skipped: a span's text must be a verbatim slice of the
//! transcript, and every tag must exist in the chart vocabulary.
//!
//! [`route_into`] is the way in; it builds the chart's own router. Routers sit
//! behind the [`Router`] trait so the deterministic lexicon matcher can be
//! substituted — by this module's tests today, which is what lets the
//! composition be checked without depending on what the lexicon happens to
//! match, and by a local-LLM closed-set classifier later.

mod coalesce;
mod lexicon;
mod transcript;
mod verify;

pub use coalesce::coalesce;
pub use lexicon::LexiconRouter;
pub use transcript::Transcript;
pub use verify::verify_gate;

use crate::contract::ChartData;
use std::collections::BTreeSet;

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

/// Where a routed transcript's passages go.
///
/// The two used to be `index_transcript` and `append_transcript`, whose bodies
/// differed by which of two lines they ran. The difference is not a pipeline,
/// it is a choice, so it is a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filing {
    /// Replace whatever the chart holds — a fresh build.
    Fresh,
    /// Append after what is there, numbering on past it. A live take files this
    /// way so earlier curation (merges, corrections) survives.
    Append,
}

/// The chart's own router: it matches in the chart's language (`meta.locale`),
/// so routing a reloaded `chart.json` re-tags in the language it was read in.
fn lexicon_for(vocab: &BTreeSet<String>, chart: &ChartData) -> LexiconRouter {
    LexiconRouter::new(vocab, &chart.aspects, crate::i18n::Locale::parse(&chart.meta.locale))
}

/// File a transcript's passages into a chart, using the chart's own router.
///
/// The whole gated path: route → Verify gate → coalesce → number → file. The
/// vocabulary is built once and shared by the router and the gate; it used to be
/// built twice per call, once by each.
pub fn route_into(chart: &mut ChartData, transcript: &Transcript, filing: Filing) -> RouteReport {
    let vocab = chart.vocab();
    let router = lexicon_for(&vocab, chart);
    file(chart, &vocab, transcript, &router, filing)
}

/// The same, with a router of your own — the seam. Used by this module's tests
/// to check the composition without depending on the lexicon's matching.
pub fn route_into_with(
    chart: &mut ChartData,
    transcript: &Transcript,
    router: &dyn Router,
    filing: Filing,
) -> RouteReport {
    let vocab = chart.vocab();
    file(chart, &vocab, transcript, router, filing)
}

fn file(
    chart: &mut ChartData,
    vocab: &BTreeSet<String>,
    transcript: &Transcript,
    router: &dyn Router,
    filing: Filing,
) -> RouteReport {
    let (mut excerpts, report) = gated(vocab, transcript, router);
    // Ids are assigned here and nowhere else: coalescing changes how many
    // passages there are, and appending changes where their numbering starts,
    // so neither the gate nor the merge can know the answer. They used to guess
    // and be overwritten — three assignments for one set of ids.
    let first = match filing {
        Filing::Fresh => 1,
        Filing::Append => next_ordinal(&chart.excerpts),
    };
    renumber(&mut excerpts, first);
    match filing {
        Filing::Fresh => chart.excerpts = excerpts,
        Filing::Append => chart.excerpts.extend(excerpts),
    }
    report
}

/// Router output through the gate and the merge, unnumbered.
fn gated(
    vocab: &BTreeSet<String>,
    transcript: &Transcript,
    router: &dyn Router,
) -> (Vec<crate::contract::Excerpt>, RouteReport) {
    let raw = router.route(transcript);
    let n_routed = raw.len();
    let (accepted, warnings) = verify_gate(transcript, raw, vocab);
    (coalesce(accepted, transcript), RouteReport { n_routed, warnings })
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
pub fn retag(chart: &ChartData, text: &str) -> Vec<String> {
    let vocab = chart.vocab();
    let router = lexicon_for(&vocab, chart);
    let transcript = Transcript::load(text);
    let (excerpts, _report) = gated(&vocab, &transcript, &router);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Excerpt;

    /// The seam's second adapter: emits exactly the spans it was given, so a
    /// test can check the composition — gate, merge, numbering, filing — without
    /// depending on what the lexicon happens to match.
    struct Fixed(Vec<RawExcerpt>);
    impl Router for Fixed {
        fn route(&self, _t: &Transcript) -> Vec<RawExcerpt> {
            self.0
                .iter()
                .map(|r| RawExcerpt { span: r.span, tags: r.tags.clone() })
                .collect()
        }
    }

    fn raw(span: (usize, usize), tags: &[&str]) -> RawExcerpt {
        RawExcerpt { span, tags: tags.iter().map(|s| s.to_string()).collect() }
    }

    /// The shared minimal chart — routing needs its vocabulary, not its
    /// astronomy.
    fn chart() -> ChartData {
        crate::fixtures::minimal_chart()
    }

    fn existing(id: &str) -> Excerpt {
        crate::fixtures::excerpt(id, "already here", &["planet:sun"])
    }

    const WORDS: &str = "Aaaa bbbb. Cccc dddd. Eeee ffff.";

    #[test]
    fn a_fresh_filing_replaces_whatever_the_chart_held() {
        let mut c = chart();
        c.excerpts = vec![existing("x1"), existing("x2")];
        let t = Transcript::load(WORDS);
        let router = Fixed(vec![raw((0, 10), &["planet:sun"])]);

        let report = route_into_with(&mut c, &t, &router, Filing::Fresh);

        assert_eq!(report.n_routed, 1);
        assert_eq!(c.excerpts.len(), 1);
        assert_eq!(c.excerpts[0].id, "x1", "a fresh filing numbers from one");
        assert_eq!(c.excerpts[0].text, "Aaaa bbbb.");
    }

    #[test]
    fn an_append_keeps_what_was_there_and_numbers_past_it() {
        let mut c = chart();
        c.excerpts = vec![existing("x1"), existing("x2")];
        let t = Transcript::load(WORDS);
        // Two spans with no shared tag, so nothing merges.
        let router = Fixed(vec![raw((0, 10), &["planet:sun"]), raw((11, 21), &["sign:s3"])]);

        route_into_with(&mut c, &t, &router, Filing::Append);

        let ids: Vec<&str> = c.excerpts.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["x1", "x2", "x3", "x4"]);
        assert_eq!(c.excerpts[0].text, "already here", "earlier curation survives");
    }

    /// Curation leaves holes; numbering from the count rather than the highest
    /// id would mint a duplicate.
    #[test]
    fn an_append_numbers_past_a_gap_left_by_curation() {
        let mut c = chart();
        c.excerpts = vec![existing("x1"), existing("x7")];
        let t = Transcript::load(WORDS);
        let router = Fixed(vec![raw((0, 10), &["planet:sun"])]);

        route_into_with(&mut c, &t, &router, Filing::Append);

        assert_eq!(c.excerpts.last().unwrap().id, "x8");
        let mut ids: Vec<&String> = c.excerpts.iter().map(|e| &e.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids must stay unique");
    }

    /// Ids are assigned once, after coalescing — which is why nothing upstream
    /// can know them: three merged spans are one passage.
    #[test]
    fn ids_are_dense_after_merging_not_before() {
        let mut c = chart();
        let t = Transcript::load(WORDS);
        // All three share a tag, so they coalesce into one passage.
        let router = Fixed(vec![
            raw((0, 10), &["planet:sun"]),
            raw((11, 21), &["planet:sun"]),
            raw((22, 32), &["planet:sun"]),
        ]);

        let report = route_into_with(&mut c, &t, &router, Filing::Fresh);

        assert_eq!(report.n_routed, 3, "the report counts what the router emitted");
        assert_eq!(c.excerpts.len(), 1, "which is not what was filed");
        assert_eq!(c.excerpts[0].id, "x1");
        assert_eq!(c.excerpts[0].text, WORDS, "one verbatim slice across all three");
    }

    /// The gate is unskippable: it sits between the router and the passages, so
    /// no router can put a tag outside the vocabulary onto a chart.
    #[test]
    fn the_gate_rejects_out_of_vocabulary_tags_and_says_so() {
        let mut c = chart();
        let t = Transcript::load(WORDS);
        let router = Fixed(vec![
            raw((0, 10), &["planet:sun"]),
            raw((11, 21), &["planet:nibiru"]),
        ]);

        let report = route_into_with(&mut c, &t, &router, Filing::Fresh);

        assert_eq!(report.n_routed, 2, "the router emitted two");
        assert_eq!(c.excerpts.len(), 1, "one of them did not pass");
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("nibiru"), "{:?}", report.warnings);
        assert!(c.validate().is_ok(), "a gated chart is always valid");
    }

    #[test]
    fn the_gate_rejects_a_span_that_is_not_a_slice_of_the_transcript() {
        let mut c = chart();
        let t = Transcript::load(WORDS);
        let router = Fixed(vec![raw((0, 9999), &["planet:sun"])]);

        let report = route_into_with(&mut c, &t, &router, Filing::Fresh);

        assert!(c.excerpts.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("out of bounds"), "{:?}", report.warnings);
    }

    #[test]
    fn a_router_that_emits_nothing_files_nothing_and_warns_about_nothing() {
        let mut c = chart();
        c.excerpts = vec![existing("x1")];
        let t = Transcript::load(WORDS);

        let report = route_into_with(&mut c, &t, &Fixed(vec![]), Filing::Append);

        assert_eq!(report.n_routed, 0);
        assert!(report.warnings.is_empty());
        assert_eq!(c.excerpts.len(), 1, "the append added nothing and removed nothing");
    }

    #[test]
    fn next_ordinal_is_one_past_the_highest_not_the_count() {
        assert_eq!(next_ordinal(&[]), 1);
        assert_eq!(next_ordinal(&[existing("x1"), existing("x2")]), 3);
        assert_eq!(next_ordinal(&[existing("x1"), existing("x7")]), 8, "gap-aware");
        // Ids that are not `x{n}` at all are ignored rather than crashing.
        assert_eq!(next_ordinal(&[existing("hand-written"), existing("x4")]), 5);
    }

    /// `retag` runs the same gated path, so a hand-typed correction can only
    /// pick up tags the chart admits.
    #[test]
    fn retag_finds_vocabulary_tags_in_free_text_and_nothing_else() {
        let c = chart();
        let tags = retag(&c, "The sun is bright today.");
        assert_eq!(tags, vec!["planet:sun"]);
        assert!(retag(&c, "Nothing here names an element.").is_empty());
    }

    #[test]
    fn routing_the_chart_s_own_language_uses_its_own_router() {
        // `route_into` builds the router from `meta.locale`, so a Russian chart
        // routes Russian words and an English one does not.
        let mut ru = chart();
        ru.meta.locale = "ru".into();
        ru.planets[0].name = "Солнце".into();
        let t = Transcript::load("Солнце сегодня яркое.");
        route_into(&mut ru, &t, Filing::Fresh);
        assert_eq!(ru.excerpts.len(), 1, "the Russian router matched");

        let mut en = chart();
        en.excerpts.clear();
        route_into(&mut en, &t, Filing::Fresh);
        assert!(en.excerpts.is_empty(), "the English router did not");
    }
}
