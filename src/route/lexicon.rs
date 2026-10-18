//! The deterministic keyword router: one lexicon entry per planet, sign and
//! house in the global vocabulary, plus the five aspect kinds.
//!
//! A sentence's tags are whatever its words name. An aspect *kind* fires on
//! its own word — "there's a trine here" is about a trine whether or not the
//! speaker says which bodies — and an aspect *pair* fires when a kind word and
//! two planet names appear together, which is the sentence shape that means
//! "these two are in aspect".
//!
//! The router no longer takes a chart. It used to build its aspect rules from
//! `chart.aspects`, so it could only ever file a pair the computed sky already
//! had; a reading that discussed an aspect the chart did not contain routed it
//! to nothing. See [`crate::vocab`] for why the pair tag outlived that.

use super::{RawNote, Router, Transcript};
use crate::i18n::Locale;
use crate::vocab::{self, PAIRS};

pub struct LexiconRouter {
    /// (tag-id, lowercase match terms) for every `planet:`, `sign:` and
    /// `house:` tag — the categories a single word can name outright.
    terms: Vec<(String, Vec<String>)>,
    /// (kind, the words that mean it in this locale).
    kinds: Vec<(&'static str, &'static [&'static str])>,
}

impl LexiconRouter {
    /// Build a router whose match terms come from `loc`'s vocabulary. The
    /// tag-ids (`planet:sun`, …) are language-neutral; only the words we scan
    /// for change with the locale.
    pub fn new(loc: Locale) -> LexiconRouter {
        let own = |ts: &[&str]| ts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut terms = Vec::new();
        for tag in vocab::ALL.iter() {
            if let Some(slug) = tag.strip_prefix("planet:") {
                terms.push((tag.clone(), own(loc.planet_terms(slug))));
            } else if let Some(slug) = tag.strip_prefix("sign:") {
                terms.push((tag.clone(), own(loc.sign_terms(slug))));
            } else if let Some(idx) =
                tag.strip_prefix("house:").and_then(|n| n.parse::<usize>().ok())
            {
                terms.push((tag.clone(), own(loc.house_terms(idx))));
            }
        }
        let kinds = vocab::KINDS.iter().map(|&k| (k, loc.aspect_match_words(k))).collect();
        LexiconRouter { terms, kinds }
    }
}

/// Word-boundary containment: `term` appears in `haystack` (both lowercase)
/// not embedded in a longer alphabetic word ("sun" must not match "sunday").
fn contains_term(haystack: &str, term: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(term) {
        let at = from + pos;
        let before_ok = at == 0
            || !haystack[..at].chars().next_back().unwrap().is_alphanumeric();
        let after = at + term.len();
        let after_ok = after >= haystack.len()
            || !haystack[after..].chars().next().unwrap().is_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = at + term.len();
    }
    false
}

impl Router for LexiconRouter {
    fn route(&self, transcript: &Transcript) -> Vec<RawNote> {
        let mut out = Vec::new();
        for (start, end) in transcript.sentences() {
            let sentence = transcript.text[start..end].to_lowercase();
            let mut tags: Vec<String> = Vec::new();
            for (tag, terms) in &self.terms {
                if terms.iter().any(|t| contains_term(&sentence, t)) {
                    tags.push(tag.clone());
                }
            }
            let spoken_kinds: Vec<&str> = self
                .kinds
                .iter()
                .filter(|(_, words)| words.iter().any(|w| contains_term(&sentence, w)))
                .map(|&(kind, _)| kind)
                .collect();
            for kind in &spoken_kinds {
                tags.push(format!("kind:{kind}"));
            }
            // A pair needs a kind word as well as two bodies: "the sun and the
            // moon are both bright" names two planets and no relation between
            // them. Which kind was said does not pick the pair — the sentence
            // "sun trine moon, and squaring venus" is about both pairs — so
            // any kind word licenses every pair the sentence names.
            if !spoken_kinds.is_empty() {
                for (a, b) in PAIRS.iter() {
                    if tags.iter().any(|t| t == &format!("planet:{a}"))
                        && tags.iter().any(|t| t == &format!("planet:{b}"))
                    {
                        tags.push(format!("aspect:{a}-{b}"));
                    }
                }
            }
            tags.sort();
            tags.dedup();
            if !tags.is_empty() {
                out.push(RawNote { span: (start, end), tags });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexicon_matches_word_boundaries_only() {
        assert!(contains_term("your sun shines", "sun"));
        assert!(!contains_term("see you sunday", "sun"));
        assert!(contains_term("mars-venus themes", "mars"));
    }

    #[test]
    fn lexicon_tags_planets_signs_houses() {
        let router = LexiconRouter::new(Locale::En);
        let t = Transcript::load("Your moon sits in cancer, in the fifth house.");
        let raw = router.route(&t);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].tags, vec!["house:5", "planet:moon", "sign:cancer"]);
    }

    #[test]
    fn aspect_pair_needs_both_planets_and_a_kind_word() {
        let router = LexiconRouter::new(Locale::En);
        let hit = Transcript::load("The sun trines your moon beautifully.");
        let tags = &router.route(&hit)[0].tags;
        assert!(tags.contains(&"aspect:sun-moon".to_string()));
        assert!(tags.contains(&"kind:trine".to_string()));

        // two bodies, no relation named between them
        let miss = Transcript::load("The sun and moon are both bright.");
        let tags = &router.route(&miss)[0].tags;
        assert!(!tags.contains(&"aspect:sun-moon".to_string()));
        assert!(!tags.iter().any(|t| t.starts_with("kind:")));
    }

    /// The router no longer asks whether *this* chart has the aspect. A
    /// reading that discusses a Mars–Pluto square files under the pair even
    /// when the sky in front of it has them nowhere near each other, which is
    /// what makes the tag mean "about these two" rather than "detected here".
    #[test]
    fn a_pair_files_without_a_chart_that_has_it() {
        let router = LexiconRouter::new(Locale::En);
        let t = Transcript::load("Your mars squares pluto, which is a whole conversation.");
        let tags = &router.route(&t)[0].tags;
        assert!(tags.contains(&"aspect:mars-pluto".to_string()));
        assert!(tags.contains(&"kind:square".to_string()));
    }

    /// The kind is the half an astrologer says out loud without naming bodies.
    #[test]
    fn a_kind_files_on_its_own_word() {
        let router = LexiconRouter::new(Locale::En);
        let t = Transcript::load("There is a grand trine in here somewhere.");
        let tags = &router.route(&t)[0].tags;
        assert_eq!(tags, &vec!["kind:trine".to_string()]);
    }

    /// Several bodies and a kind word: every pair the sentence names files.
    #[test]
    fn three_bodies_and_a_kind_word_file_every_pair() {
        let router = LexiconRouter::new(Locale::En);
        let t = Transcript::load("Sun, moon and venus are all in square here.");
        let tags = &router.route(&t)[0].tags;
        for pair in ["aspect:sun-moon", "aspect:sun-venus", "aspect:moon-venus"] {
            assert!(tags.contains(&pair.to_string()), "missing {pair} in {tags:?}");
        }
    }

    /// The Ascendant is a point, not a body: it takes part in no pair.
    #[test]
    fn the_ascendant_is_a_planet_tag_but_never_half_of_a_pair() {
        let router = LexiconRouter::new(Locale::En);
        let t = Transcript::load("The sun is conjunct the ascendant.");
        let tags = &router.route(&t)[0].tags;
        assert!(tags.contains(&"planet:ascendant".to_string()));
        assert!(tags.contains(&"kind:conjunction".to_string()));
        assert!(!tags.iter().any(|t| t.contains("ascendant-") || t.contains("-ascendant")));
    }

    #[test]
    fn russian_transcript_routes_planets_signs_houses() {
        let router = LexiconRouter::new(Locale::Ru);
        let t = Transcript::load("Ваше солнце в раке, в пятом доме.");
        let raw = router.route(&t);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].tags, vec!["house:5", "planet:sun", "sign:cancer"]);
    }

    #[test]
    fn russian_aspect_needs_both_planets_and_a_kind_word() {
        let router = LexiconRouter::new(Locale::Ru);
        let hit = Transcript::load("Солнце в тригоне с вашей луной.");
        let tags = &router.route(&hit)[0].tags;
        assert!(tags.contains(&"aspect:sun-moon".to_string()));
        assert!(tags.contains(&"kind:trine".to_string()));
        let miss = Transcript::load("Солнце и луна оба яркие.");
        assert!(!router.route(&miss)[0].tags.contains(&"aspect:sun-moon".to_string()));
    }

    /// The vocabulary is global, but the words are not: a Russian reading
    /// routes through Russian terms and an English one through English.
    #[test]
    fn the_kind_word_is_localized_even_though_the_tag_is_not() {
        let t = Transcript::load("Солнце в квадратуре с луной.");
        assert!(
            LexiconRouter::new(Locale::Ru).route(&t)[0].tags.contains(&"kind:square".to_string())
        );
        assert!(LexiconRouter::new(Locale::En).route(&t).is_empty());
    }
}
