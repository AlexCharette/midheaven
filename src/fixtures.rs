//! The charts every test in the workspace was building for itself.
//!
//! Nine hand-built `BirthInput`s were the same birth — Berlin, 13 July 1990,
//! 14:30 local — differing only in the name, and four hand-built `ChartData`
//! literals were the same twelve-of-everything shape. Spread across two crates,
//! so a field added to `Meta` cost thirteen edits.
//!
//! Available to the whole workspace rather than only to this crate's own unit
//! tests, because `#[cfg(test)]` applies to the crate under test and nothing
//! else: `tests/pipeline.rs` sees only the public API, and `astro-desktop` is a
//! different crate. Both reach these through the `testing` feature, which each
//! turns on in its dev-dependencies and neither ships.

use crate::chart::BirthInput;
use crate::contract::{Aspect, Axes, Body, ChartData, Excerpt, HouseRef, Meta, Ref};

/// 13 July 1990, 14:30 in Berlin — tropical, Whole Sign. The birth nine tests
/// were each spelling out.
pub fn berlin() -> BirthInput {
    named("Mira Holt")
}

/// The same birth under another name, for tests that tell two readings apart.
pub fn named(name: &str) -> BirthInput {
    BirthInput {
        name: name.into(),
        date: "1990-07-13".parse().expect("a valid date"),
        time: "14:30:00".parse().expect("a valid time"),
        lat: 52.52,
        lon: 13.405,
        tz: "Europe/Berlin".parse().expect("a real timezone"),
        place: "Berlin, Germany".into(),
        locale: crate::i18n::Locale::En,
        house_system: crate::chart::systems::DEFAULT_HOUSE_SYSTEM,
        ayanamsa: None,
    }
}

/// [`berlin`], computed. Runs the ephemeris, so prefer [`minimal_chart`] when a
/// test needs a chart's *shape* rather than its astronomy.
pub fn berlin_chart() -> ChartData {
    crate::chart::compute_chart(&berlin()).expect("the fixture birth computes")
}

/// A structurally valid chart with no astronomy behind it: twelve signs, houses
/// and cusps, one body, one aspect, no passages.
///
/// Deliberately the *fullest* minimal chart — `validate` requires twelve of each
/// and a closed vocabulary, and a test that wants less can clear what it does
/// not need. Element ids are placeholders (`sign:s0`), not real slugs, so a test
/// asserting on real names is using the wrong fixture.
pub fn minimal_chart() -> ChartData {
    let mut chart = ChartData {
        meta: Meta {
            name: "T".into(),
            born: "b".into(),
            place: "p".into(),
            system: "Whole Sign".into(),
            zodiac: "Tropical".into(),
            house_system: "whole-sign".into(),
            ayanamsa: None,
            locale: "en".into(),
            astrologer: None,
            logo: None,
            birth: None,
        },
        axes: Axes { asc: 0.0, mc: 270.0 },
        house_cusps: (0..12).map(|i| i as f64 * 30.0).collect(),
        house_sweeps: Vec::new(),
        planets: vec![Body {
            id: "planet:sun".into(),
            glyph: "\u{2609}".into(),
            name: "Sun".into(),
            lon: 0.0,
            house: 1,
            ..Default::default()
        }],
        signs: (0..12)
            .map(|i| Ref {
                id: format!("sign:s{i}"),
                glyph: "x".into(),
                name: "S".into(),
                element: "fire".into(),
            })
            .collect(),
        houses: (0..12)
            .map(|i| HouseRef { id: format!("house:{}", i + 1), label: "I".into(), name: "H".into() })
            .collect(),
        aspects: vec![Aspect {
            id: "aspect:sun-moon".into(),
            glyph: "\u{25b3}".into(),
            name: "Trine".into(),
            a: "planet:sun".into(),
            b: "planet:moon".into(),
            nature: "harmonious".into(),
            orb: 0.0,
            kind: "",
        }],
        excerpts: Vec::new(),
    };
    // The same filling the compute stage does, so the fixture is a chart the
    // renderers could actually be handed.
    crate::derive::fill(&mut chart);
    chart
}

/// A passage, tagged. `span` is `[0, text.len()]`, which is the provenance
/// invariant for a passage authored rather than routed.
pub fn excerpt(id: &str, text: &str, tags: &[&str]) -> Excerpt {
    Excerpt {
        id: id.into(),
        time: String::new(),
        span: [0, text.len()],
        text: text.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixtures are only useful if what they claim is true — a chart that
    /// does not validate would send every test that uses it down a wrong path.
    #[test]
    fn the_minimal_chart_is_a_chart_the_app_would_accept() {
        let chart = minimal_chart();
        assert!(chart.validate().is_ok(), "{:?}", chart.validate());
        assert_eq!(chart.house_sweeps.len(), 12, "derive::fill ran");
        assert!(chart.excerpts.is_empty(), "a caller adds the passages it wants");
    }

    #[test]
    fn the_berlin_birth_computes_into_a_valid_chart() {
        let chart = berlin_chart();
        assert!(chart.validate().is_ok());
        assert_eq!(chart.meta.name, "Mira Holt");
        assert_eq!(chart.planets.len(), 11, "ten bodies and the ascendant point");
    }

    #[test]
    fn a_named_birth_differs_only_in_its_name() {
        let a = berlin();
        let b = named("Someone Else");
        assert_eq!((a.date, a.time, a.lat, a.lon, a.tz), (b.date, b.time, b.lat, b.lon, b.tz));
        assert_ne!(a.name, b.name);
    }

    #[test]
    fn an_excerpt_spans_its_own_text() {
        let ex = excerpt("x1", "The sun.", &["planet:sun"]);
        assert_eq!(ex.span, [0, "The sun.".len()]);
        assert_eq!(ex.tags, vec!["planet:sun"]);
    }
}
