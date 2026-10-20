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

use crate::contract::Note;

/// What a router emits: spans + tags, never text of its own.
pub struct RawNote {
    pub span: (usize, usize),
    pub tags: Vec<String>,
}

pub trait Router {
    fn route(&self, transcript: &Transcript) -> Vec<RawNote>;
}

/// What routing a transcript produced, apart from the notes themselves:
/// how many spans the router emitted before gating, and any provenance
/// warnings (spans/tags the Verify gate rejected) for a frontend to surface.
#[derive(Debug, Default)]
pub struct RouteReport {
    pub n_routed: usize,
    pub warnings: Vec<String>,
}

/// Route a transcript into notes, in `loc`'s language.
///
/// The whole gated path: route → Verify gate → coalesce. The notes come back
/// unnumbered — `id` is empty on every one — because whoever stores them owns
/// their ids.
///
/// This used to be `route_into`, which took a chart and a [`Filing`] saying
/// whether to replace its note list or append to it. Both of those were
/// consequences of the chart owning the notes: appending existed so a second
/// take would not wipe the first, and numbering happened here because the
/// chart's list was the only thing that knew how many notes there already
/// were. A store that mints its own ids needs neither.
pub fn route(transcript: &Transcript, loc: crate::i18n::Locale) -> (Vec<Note>, RouteReport) {
    route_with(transcript, &LexiconRouter::new(loc))
}

/// The same, with a router of your own — the seam. Used by this module's tests
/// to check the composition without depending on the lexicon's matching.
///
/// This is also the persistence boundary: above it the pipeline works in
/// note-plus-span pairs because the merge needs them, and nothing below it has
/// ever read a span back.
pub fn route_with(transcript: &Transcript, router: &dyn Router) -> (Vec<Note>, RouteReport) {
    let (gated_notes, report) = gated(transcript, router);
    (gated_notes.into_iter().map(|(note, _span)| note).collect(), report)
}

/// Router output through the gate and the merge, unnumbered.
fn gated(
    transcript: &Transcript,
    router: &dyn Router,
) -> (Vec<(crate::contract::Note, [usize; 2])>, RouteReport) {
    let raw = router.route(transcript);
    let n_routed = raw.len();
    let (accepted, warnings) = verify_gate(transcript, raw);
    (coalesce(accepted, transcript), RouteReport { n_routed, warnings })
}

/// Tags the router finds in free text — curation re-tagging goes through the
/// same gated path as everything else.
pub fn retag(text: &str, loc: crate::i18n::Locale) -> Vec<String> {
    let (notes, _report) = route(&Transcript::load(text), loc);
    let mut tags: Vec<String> = notes.into_iter().flat_map(|n| n.tags).collect();
    tags.sort();
    tags.dedup();
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seam's second adapter: emits exactly the spans it was given, so a
    /// test can check the composition — gate, merge, numbering, filing — without
    /// depending on what the lexicon happens to match.
    struct Fixed(Vec<RawNote>);
    impl Router for Fixed {
        fn route(&self, _t: &Transcript) -> Vec<RawNote> {
            self.0
                .iter()
                .map(|r| RawNote { span: r.span, tags: r.tags.clone() })
                .collect()
        }
    }

    fn raw(span: (usize, usize), tags: &[&str]) -> RawNote {
        RawNote { span, tags: tags.iter().map(|s| s.to_string()).collect() }
    }

    const WORDS: &str = "Aaaa bbbb. Cccc dddd. Eeee ffff.";

    /// Numbering used to live here, and three tests covered it: a fresh
    /// filing numbered from one, an append numbered past what was there, and
    /// `next_ordinal` skipped the gaps curation left. All three were about
    /// the chart owning a note list. Routing hands back unnumbered notes now,
    /// and whoever stores them gives them ids.
    #[test]
    fn routed_notes_come_back_unnumbered() {
        let t = Transcript::load(WORDS);
        let router = Fixed(vec![raw((0, 10), &["planet:sun"])]);

        let (notes, report) = route_with(&t, &router);

        assert_eq!(report.n_routed, 1);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Aaaa bbbb.");
        assert!(notes[0].id.is_empty(), "the store names it, not the router");
    }

    /// The report counts what the router emitted, which is not what came out:
    /// three merged spans are one note.
    #[test]
    fn merging_happens_before_the_count_the_caller_sees() {
        let t = Transcript::load(WORDS);
        // All three share a tag, so they coalesce into one note.
        let router = Fixed(vec![
            raw((0, 10), &["planet:sun"]),
            raw((11, 21), &["planet:sun"]),
            raw((22, 32), &["planet:sun"]),
        ]);

        let (notes, report) = route_with(&t, &router);

        assert_eq!(report.n_routed, 3, "the report counts what the router emitted");
        assert_eq!(notes.len(), 1, "which is not what came back");
        assert_eq!(notes[0].text, WORDS, "one verbatim slice across all three");
    }

    /// The gate is unskippable: it sits between the router and the notes, so
    /// no router can produce a tag outside the vocabulary.
    #[test]
    fn the_gate_rejects_out_of_vocabulary_tags_and_says_so() {
        let t = Transcript::load(WORDS);
        let router = Fixed(vec![
            raw((0, 10), &["planet:sun"]),
            raw((11, 21), &["planet:nibiru"]),
        ]);

        let (notes, report) = route_with(&t, &router);

        assert_eq!(report.n_routed, 2, "the router emitted two");
        assert_eq!(notes.len(), 1, "one of them did not pass");
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("nibiru"), "{:?}", report.warnings);
        assert!(
            notes.iter().flat_map(|n| &n.tags).all(|t| crate::vocab::contains(t)),
            "everything that got through is in vocabulary"
        );
    }

    #[test]
    fn the_gate_rejects_a_span_that_is_not_a_slice_of_the_transcript() {
        let t = Transcript::load(WORDS);
        let router = Fixed(vec![raw((0, 9999), &["planet:sun"])]);

        let (notes, report) = route_with(&t, &router);

        assert!(notes.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("out of bounds"), "{:?}", report.warnings);
    }

    #[test]
    fn a_router_that_emits_nothing_routes_nothing_and_warns_about_nothing() {
        let t = Transcript::load(WORDS);
        let (notes, report) = route_with(&t, &Fixed(vec![]));

        assert_eq!(report.n_routed, 0);
        assert!(report.warnings.is_empty());
        assert!(notes.is_empty());
    }

    /// `retag` runs the same gated path, so a hand-typed correction can only
    /// pick up tags the vocabulary admits.
    #[test]
    fn retag_finds_vocabulary_tags_in_free_text_and_nothing_else() {
        let tags = retag("The sun is bright today.", crate::i18n::Locale::En);
        assert_eq!(tags, vec!["planet:sun"]);
        assert!(retag("Nothing here names an element.", crate::i18n::Locale::En).is_empty());
    }

    /// The vocabulary is global but the words are not, so the locale is what
    /// decides whether a sentence routes at all.
    #[test]
    fn the_locale_picks_the_words_the_router_scans_for() {
        let t = Transcript::load("Солнце сегодня яркое.");
        let (ru, _) = route(&t, crate::i18n::Locale::Ru);
        assert_eq!(ru.len(), 1, "the Russian router matched");
        let (en, _) = route(&t, crate::i18n::Locale::En);
        assert!(en.is_empty(), "the English router did not");
    }
}
