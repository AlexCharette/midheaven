//! The deterministic keyword router: one lexicon entry per tag-id in the
//! chart vocabulary; aspect tags additionally require both planet tags to
//! have matched and a matching aspect word in the same sentence.

use super::{RawExcerpt, Router, Transcript};
use crate::contract::Aspect;
use crate::i18n::Locale;
use std::collections::BTreeSet;

pub struct LexiconRouter {
    terms: Vec<(String, Vec<String>)>, // (tag-id, lowercase match terms)
    aspects: Vec<AspectRule>,
}

/// Aspect tags fire when the aspect word and both planet tags co-occur.
struct AspectRule {
    id: String,
    words: &'static [&'static str],
    a: String, // planet tag-ids
    b: String,
}

impl LexiconRouter {
    /// Build a router whose match terms come from `loc`'s vocabulary. The
    /// tag-ids (`planet:sun`, …) are language-neutral; only the words we scan
    /// for change with the locale.
    pub fn new(vocab: &BTreeSet<String>, aspects: &[Aspect], loc: Locale) -> LexiconRouter {
        let own = |ts: &[&str]| ts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut terms = Vec::new();
        for tag in vocab {
            if let Some(slug) = tag.strip_prefix("planet:") {
                terms.push((tag.clone(), own(loc.planet_terms(slug))));
            } else if let Some(slug) = tag.strip_prefix("sign:") {
                terms.push((tag.clone(), own(loc.sign_terms(slug))));
            } else if let Some(n) = tag.strip_prefix("house:") {
                if let Ok(idx) = n.parse::<usize>() {
                    terms.push((tag.clone(), own(loc.house_terms(idx))));
                }
            }
        }
        let aspects = aspects
            .iter()
            .map(|a| AspectRule {
                id: a.id.clone(),
                words: loc.aspect_match_words(a.kind),
                a: a.a.clone(),
                b: a.b.clone(),
            })
            .collect();
        LexiconRouter { terms, aspects }
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
    fn route(&self, transcript: &Transcript) -> Vec<RawExcerpt> {
        let mut out = Vec::new();
        for (start, end) in transcript.sentences() {
            let sentence = transcript.text[start..end].to_lowercase();
            let mut tags: Vec<String> = Vec::new();
            for (tag, terms) in &self.terms {
                if terms.iter().any(|t| contains_term(&sentence, t)) {
                    tags.push(tag.clone());
                }
            }
            // The planet scans above already decided which planets appear;
            // an aspect fires when its word appears and both planets matched.
            for rule in &self.aspects {
                if rule.words.iter().any(|w| contains_term(&sentence, w))
                    && tags.contains(&rule.a)
                    && tags.contains(&rule.b)
                {
                    tags.push(rule.id.clone());
                }
            }
            tags.sort();
            tags.dedup();
            if !tags.is_empty() {
                out.push(RawExcerpt { span: (start, end), tags });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::test_vocab as vocab;

    #[test]
    fn lexicon_matches_word_boundaries_only() {
        assert!(contains_term("your sun shines", "sun"));
        assert!(!contains_term("see you sunday", "sun"));
        assert!(contains_term("mars-venus themes", "mars"));
    }

    #[test]
    fn lexicon_tags_planets_signs_houses() {
        let v = vocab(&["planet:moon", "sign:cancer", "house:5"]);
        let router = LexiconRouter::new(&v, &[], Locale::En);
        let t = Transcript::load("Your moon sits in cancer, in the fifth house.");
        let raw = router.route(&t);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].tags, vec!["house:5", "planet:moon", "sign:cancer"]);
    }

    #[test]
    fn aspect_needs_both_planets_and_word() {
        let v = vocab(&["planet:sun", "planet:moon", "aspect:sun-moon"]);
        let aspects = vec![Aspect {
            id: "aspect:sun-moon".into(),
            glyph: "△".into(),
            name: "Sun trine Moon".into(),
            a: "planet:sun".into(),
            b: "planet:moon".into(),
            nature: "harmonious".into(),
            kind: "trine",
        }];
        let router = LexiconRouter::new(&v, &aspects, Locale::En);
        let hit = Transcript::load("The sun trines your moon beautifully.");
        let tags = &router.route(&hit)[0].tags;
        assert!(tags.contains(&"aspect:sun-moon".to_string()));
        let miss = Transcript::load("The sun and moon are both bright.");
        let tags = &router.route(&miss)[0].tags;
        assert!(!tags.contains(&"aspect:sun-moon".to_string()));
    }

    #[test]
    fn russian_transcript_routes_planets_signs_houses() {
        let v = vocab(&["planet:sun", "sign:cancer", "house:5"]);
        let router = LexiconRouter::new(&v, &[], Locale::Ru);
        let t = Transcript::load("Ваше солнце в раке, в пятом доме.");
        let raw = router.route(&t);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].tags, vec!["house:5", "planet:sun", "sign:cancer"]);
    }

    #[test]
    fn russian_aspect_needs_both_planets_and_word() {
        let v = vocab(&["planet:sun", "planet:moon", "aspect:sun-moon"]);
        let aspects = vec![Aspect {
            id: "aspect:sun-moon".into(),
            glyph: "△".into(),
            name: "Солнце тригон Луна".into(),
            a: "planet:sun".into(),
            b: "planet:moon".into(),
            nature: "harmonious".into(),
            kind: "trine",
        }];
        let router = LexiconRouter::new(&v, &aspects, Locale::Ru);
        let hit = Transcript::load("Солнце в тригоне с вашей луной.");
        assert!(router.route(&hit)[0].tags.contains(&"aspect:sun-moon".to_string()));
        let miss = Transcript::load("Солнце и луна оба яркие.");
        assert!(!router.route(&miss)[0].tags.contains(&"aspect:sun-moon".to_string()));
    }
}
