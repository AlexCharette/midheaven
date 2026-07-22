//! The `ChartData` contract between pipeline stages, mirroring the TS
//! interface in `docs/natal-reading-indexer.md`. The chart stage produces it,
//! the routing stage fills `excerpts`, and the emit stage serializes it into
//! the HTML artifact as `const DATA = {...}`. No stage owns it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub meta: Meta,
    pub axes: Axes,
    #[serde(rename = "houseCusps")]
    pub house_cusps: Vec<f64>,
    pub planets: Vec<Body>,
    pub signs: Vec<Ref>,
    pub houses: Vec<HouseRef>,
    pub aspects: Vec<Aspect>,
    pub excerpts: Vec<Excerpt>,
}

impl ChartData {
    /// The closed tag vocabulary derived from the computed chart:
    /// `planet:*`, `sign:*`, `house:*`, `aspect:*`. This is the only set of
    /// tags a router may emit and the Verify gate accepts.
    pub fn vocab(&self) -> BTreeSet<String> {
        self.planets
            .iter()
            .map(|p| p.id.clone())
            .chain(self.signs.iter().map(|s| s.id.clone()))
            .chain(self.houses.iter().map(|h| h.id.clone()))
            .chain(self.aspects.iter().map(|a| a.id.clone()))
            .collect()
    }

    /// Look up a planet body (including the Ascendant point) by tag-id.
    pub fn planet(&self, id: &str) -> Option<&Body> {
        self.planets.iter().find(|p| p.id == id)
    }

    /// Structural validation for a chart that arrived from an untrusted source
    /// (the desktop `load_chart` deserializes an arbitrary `chart.json`). The
    /// compute pipeline always upholds these invariants; deserialization does
    /// not, and downstream code relies on them — the PDF/wheel layout indexes
    /// `signs`/`houses`/`house_cusps` assuming twelve of each, the router
    /// derives lexicon terms from element ids (an empty slug loops forever),
    /// and both the artifact and the webview render ids/tags as markup. Keeping
    /// ids to the closed `cat:slug` vocabulary shape also forecloses HTML
    /// injection and off-line-artifact beacons at the source.
    pub fn validate(&self) -> Result<(), String> {
        for (label, len) in [
            ("signs", self.signs.len()),
            ("houses", self.houses.len()),
            ("house cusps", self.house_cusps.len()),
        ] {
            if len != 12 {
                return Err(format!("expected 12 {label}, found {len}"));
            }
        }
        for p in &self.planets {
            check_id(&p.id, "planet")?;
            if !(1..=12).contains(&p.house) {
                return Err(format!("planet {:?} has house {} outside 1..=12", p.id, p.house));
            }
        }
        for s in &self.signs {
            check_id(&s.id, "sign")?;
        }
        for h in &self.houses {
            check_id(&h.id, "house")?;
        }
        for a in &self.aspects {
            check_id(&a.id, "aspect")?;
        }
        let vocab = self.vocab();
        for ex in &self.excerpts {
            for tag in &ex.tags {
                if !vocab.contains(tag) {
                    return Err(format!("excerpt {:?} references unknown tag {tag:?}", ex.id));
                }
            }
        }
        if self.meta.logo.as_deref().is_some_and(|l| !l.starts_with("data:")) {
            return Err("meta.logo must be an embedded data: URI".to_string());
        }
        Ok(())
    }
}

/// An element id must be exactly `{cat}:{slug}` with a non-empty slug drawn
/// from `[a-z0-9_-]` (the hyphen carries aspect ids like `aspect:sun-moon`).
/// This is the closed vocabulary the compute stage emits; enforcing it on
/// ingest keeps `"`/`<`/`>` out of ids (no markup injection) and keeps slugs
/// non-empty (no empty lexicon term).
fn check_id(id: &str, cat: &str) -> Result<(), String> {
    let ok = id
        .strip_prefix(cat)
        .and_then(|r| r.strip_prefix(':'))
        .is_some_and(|slug| {
            !slug.is_empty()
                && slug.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-'))
        });
    if ok {
        Ok(())
    } else {
        Err(format!("malformed {cat} id {id:?}"))
    }
}

/// One timestamped transcript segment — the `{"start", "text"}` JSONL wire
/// format between the transcription and routing stages; neither stage owns it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub text: String,
}

/// Filter match mode for excerpt selections, shared by every viewer
/// (the HTML template implements the same semantics in JS).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Any,
    All,
}

impl Excerpt {
    /// Merge a later passage into this one: the span extends to cover both,
    /// tags become the sorted union, this passage's time anchor is kept.
    /// The merged `text` is the caller's decision (verbatim transcript slice
    /// when one is at hand, joined parts otherwise).
    pub fn absorb(&mut self, other: Excerpt) {
        self.span[1] = other.span[1];
        let mut tags: Vec<String> = self.tags.drain(..).chain(other.tags).collect();
        tags.sort();
        tags.dedup();
        self.tags = tags;
    }

    /// An empty selection matches everything; Any = the excerpt touches any
    /// selected tag; All = it touches every one.
    pub fn matches(&self, selected: &BTreeSet<String>, mode: Mode) -> bool {
        if selected.is_empty() {
            return true;
        }
        match mode {
            Mode::Any => selected.iter().any(|t| self.tags.contains(t)),
            Mode::All => selected.iter().all(|t| self.tags.contains(t)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    pub born: String,
    pub place: String,
    pub system: String,
    pub zodiac: String,
    /// Practitioner branding for the artifact ("prepared by …"); absent
    /// unless a frontend stamps it, keeping unbranded output byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub astrologer: Option<String>,
    /// Practitioner logo as a `data:` URI — the artifact stays self-contained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Axes {
    /// Ecliptic longitude in degrees; 0 = 0° Aries.
    pub asc: f64,
    pub mc: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub id: String,
    pub glyph: String,
    pub name: String,
    pub lon: f64,
    pub house: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ref {
    pub id: String,
    pub glyph: String,
    pub name: String,
    /// The sign's classical element (fire/earth/air/water).
    pub element: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HouseRef {
    pub id: String,
    /// Roman numeral.
    pub label: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aspect {
    pub id: String,
    pub glyph: String,
    pub name: String,
    /// Planet ids.
    pub a: String,
    pub b: String,
    /// "harmonious" (trine, sextile), "challenging" (square, opposition),
    /// or "neutral" (conjunction) — drives the wheel's chord coloring.
    pub nature: String,
    /// Aspect kind ("trine", …) for routers matching aspect words; not part
    /// of the serialized artifact contract.
    #[serde(skip)]
    pub kind: &'static str,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Excerpt {
    /// Unique among the chart's excerpts (the `x{n}` scheme is convention;
    /// uniqueness is the invariant consumers rely on).
    pub id: String,
    /// "HH:MM:SS" anchor into the recording; empty when the transcript
    /// carries no timestamps.
    pub time: String,
    /// Byte offsets into the transcript — provenance.
    pub span: [usize; 2],
    /// VERBATIM; must equal `transcript[span.0..span.1]`.
    pub text: String,
    /// Tag-ids; each must exist in the chart vocabulary.
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal but structurally valid chart: 12 signs/houses/cusps, one
    /// planet, one aspect, one excerpt tagged from the vocabulary.
    fn valid_chart() -> ChartData {
        ChartData {
            meta: Meta {
                name: "T".into(),
                born: "b".into(),
                place: "p".into(),
                system: "Whole Sign".into(),
                zodiac: "Tropical".into(),
                astrologer: None,
                logo: None,
            },
            axes: Axes { asc: 0.0, mc: 270.0 },
            house_cusps: (0..12).map(|i| i as f64 * 30.0).collect(),
            planets: vec![Body {
                id: "planet:sun".into(),
                glyph: "☉".into(),
                name: "Sun".into(),
                lon: 0.0,
                house: 1,
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
                .map(|i| HouseRef {
                    id: format!("house:{}", i + 1),
                    label: "I".into(),
                    name: "H".into(),
                })
                .collect(),
            aspects: vec![Aspect {
                id: "aspect:sun-moon".into(),
                glyph: "△".into(),
                name: "Trine".into(),
                a: "planet:sun".into(),
                b: "planet:moon".into(),
                nature: "harmonious".into(),
                kind: "",
            }],
            excerpts: vec![Excerpt {
                id: "x1".into(),
                time: String::new(),
                span: [0, 0],
                text: "hi".into(),
                tags: vec!["planet:sun".into()],
            }],
        }
    }

    #[test]
    fn validate_accepts_a_well_formed_chart() {
        assert!(valid_chart().validate().is_ok());
        // a data: logo is allowed
        let mut c = valid_chart();
        c.meta.logo = Some("data:image/png;base64,AAAA".into());
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_rejects_injection_and_broken_structure() {
        // a quote in an id would break out of the artifact's data-cat attribute
        let mut c = valid_chart();
        c.planets[0].id = "planet:s\"x".into();
        assert!(c.validate().is_err());

        // an empty aspect slug would make the router's lexicon loop forever
        let mut c = valid_chart();
        c.aspects[0].id = "aspect:".into();
        assert!(c.validate().is_err());

        // a tag outside the vocabulary
        let mut c = valid_chart();
        c.excerpts[0].tags = vec!["planet:pluto".into()];
        assert!(c.validate().is_err());

        // fewer than twelve signs → PDF/wheel index panic
        let mut c = valid_chart();
        c.signs.pop();
        assert!(c.validate().is_err());

        // a remote logo would turn the offline artifact into a beacon
        let mut c = valid_chart();
        c.meta.logo = Some("https://evil.example/x.png".into());
        assert!(c.validate().is_err());

        // a house index outside 1..=12
        let mut c = valid_chart();
        c.planets[0].house = 0;
        assert!(c.validate().is_err());
    }
}
