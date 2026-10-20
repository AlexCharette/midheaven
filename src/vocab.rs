//! The closed tag vocabulary — every tag a note may carry, independent of any
//! chart.
//!
//! This used to be [`crate::contract::ChartData::vocab`], built per chart from
//! the elements that chart happened to contain. That made the vocabulary a
//! property of a computed sky, and so made a note untaggable without one: to
//! write "the moon in the sixth house" you first needed a chart that had a
//! moon and a sixth house. Four of the five categories never varied by chart
//! anyway — there are always the same planets, signs and houses — so the
//! per-chart vocabulary was mostly a fixed set rebuilt on every call.
//!
//! The fifth category is where it was really chart-shaped. `aspect:sun-moon`
//! existed only when *this* chart detected a Sun–Moon aspect, which meant the
//! tag said two different things depending on who was asking: to the router,
//! "these two bodies aspect each other here", and to a reader, "this note is
//! about the Sun and the Moon". Only the second survives without a chart, so
//! that is the one the vocabulary keeps: all [`PAIRS`] planet pairs exist as
//! tags, whether or not any particular sky brings them into aspect.
//!
//! What the first reading loses, [`KINDS`] gains back. `kind:trine` says the
//! thing the aspect tag used to carry implicitly, says it without naming
//! bodies, and can be filed from the spoken word alone — which is what an
//! astrologer actually says out loud.

use crate::chart::catalog::{ASPECT_TYPES, HOUSE_NAMES, PLANETS, SIGNS_ALL};
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// The five categories, in the order the viewers band them.
pub const CATEGORIES: [&str; 5] = ["planet", "sign", "house", "aspect", "kind"];

/// Planet pairs, in [`PLANETS`] order on both sides — the same order
/// `chart::detect_aspects` builds its ids from, so a detected aspect's id is
/// always one of these. The Ascendant is a chart point rather than a body and
/// takes part in no aspect, so it is a `planet:` tag and not a pair.
pub static PAIRS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    let mut out = Vec::new();
    for (i, &(_, a, ..)) in PLANETS.iter().enumerate() {
        for &(_, b, ..) in &PLANETS[i + 1..] {
            out.push((a, b));
        }
    }
    out
});

/// The five Ptolemaic aspect kinds, as spoken: trine, square, and so on.
///
/// This is `ASPECT_TYPES`' first field, the one the router already matches
/// words against (`i18n::Locale::aspect_match_words`) — not its last field,
/// which is the three-valued harmonious/challenging/neutral that colours the
/// wheel's chords. Nobody says "that's a harmonious"; they say "that's a
/// trine".
pub static KINDS: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| ASPECT_TYPES.iter().map(|&(kind, ..)| kind).collect());

/// Every tag in the vocabulary.
pub static ALL: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    let mut out = BTreeSet::new();
    for &(_, short, ..) in PLANETS {
        out.insert(format!("planet:{short}"));
    }
    // The Ascendant is a point, not a body: `chart::ascendant_point` mints it
    // separately and `PLANETS` does not list it.
    out.insert("planet:ascendant".to_string());
    for &(short, ..) in SIGNS_ALL {
        out.insert(format!("sign:{short}"));
    }
    for n in 1..=HOUSE_NAMES.len() {
        out.insert(format!("house:{n}"));
    }
    for (a, b) in PAIRS.iter() {
        out.insert(format!("aspect:{a}-{b}"));
    }
    for kind in KINDS.iter() {
        out.insert(format!("kind:{kind}"));
    }
    out
});

/// Whether `tag` is in the vocabulary. The only question the gate asks.
pub fn contains(tag: &str) -> bool {
    ALL.contains(tag)
}

/// A tag's category, or `None` if it is not a vocabulary tag. The viewers band
/// tags by this, and the PDF colours its chips by it.
pub fn category(tag: &str) -> Option<&'static str> {
    let cat = tag.split_once(':')?.0;
    CATEGORIES.into_iter().find(|&c| c == cat).filter(|_| contains(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sizes are the point: a vocabulary that silently lost a category
    /// would still be a valid `BTreeSet`, and every downstream check would
    /// still pass while the missing tags simply stopped being filable.
    #[test]
    fn the_vocabulary_is_the_five_fixed_categories() {
        let count = |cat: &str| ALL.iter().filter(|t| t.starts_with(&format!("{cat}:"))).count();
        assert_eq!(count("planet"), 11, "ten bodies and the ascendant");
        assert_eq!(count("sign"), 12);
        assert_eq!(count("house"), 12);
        assert_eq!(count("aspect"), 45, "every pair of the ten bodies, C(10,2)");
        assert_eq!(count("kind"), 5, "the Ptolemaic aspects");
        assert_eq!(ALL.len(), 85, "and nothing else");
    }

    /// Every id in here is rendered as markup by the artifact and the webview,
    /// so the charset is the injection boundary — see `contract::check_id`.
    #[test]
    fn every_tag_is_a_well_formed_id() {
        for tag in ALL.iter() {
            let (cat, slug) = tag.split_once(':').expect("every tag is cat:slug");
            assert!(CATEGORIES.contains(&cat), "unknown category in {tag:?}");
            assert!(!slug.is_empty(), "empty slug in {tag:?}");
            assert!(
                slug.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-')),
                "{tag:?} is outside the closed charset"
            );
        }
    }

    /// The aspect ids a real chart mints have to be ones the vocabulary holds,
    /// or the gate would reject the router's own output. `detect_aspects`
    /// builds `aspect:{a}-{b}` from `PLANETS` order, and so does `PAIRS`.
    #[test]
    fn a_computed_charts_aspect_ids_are_all_in_the_vocabulary() {
        let chart = crate::chart::compute_chart(&crate::fixtures::named("Vocab")).expect("chart");
        assert!(!chart.aspects.is_empty(), "the fixture detects some aspects");
        for a in &chart.aspects {
            assert!(contains(&a.id), "{:?} is not in the vocabulary", a.id);
        }
        for p in &chart.planets {
            assert!(contains(&p.id), "{:?} is not in the vocabulary", p.id);
        }
        for s in &chart.signs {
            assert!(contains(&s.id), "{:?} is not in the vocabulary", s.id);
        }
        for h in &chart.houses {
            assert!(contains(&h.id), "{:?} is not in the vocabulary", h.id);
        }
    }

    #[test]
    fn category_names_the_band_a_tag_belongs_to() {
        assert_eq!(category("planet:sun"), Some("planet"));
        assert_eq!(category("kind:trine"), Some("kind"));
        assert_eq!(category("aspect:sun-moon"), Some("aspect"));
        // not a vocabulary tag, so it has no band
        assert_eq!(category("planet:nibiru"), None);
        assert_eq!(category("colour:red"), None);
        assert_eq!(category("nonsense"), None);
    }
}
