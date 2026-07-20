//! The `ChartData` contract between pipeline stages, mirroring the TS
//! interface in `docs/natal-reading-indexer.md`. The chart stage produces it,
//! the routing stage fills `excerpts`, and the emit stage serializes it into
//! the HTML artifact as `const DATA = {...}`. No stage owns it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub name: String,
    pub born: String,
    pub place: String,
    pub system: String,
    pub zodiac: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Axes {
    /// Ecliptic longitude in degrees; 0 = 0° Aries.
    pub asc: f64,
    pub mc: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Body {
    pub id: String,
    pub glyph: String,
    pub name: String,
    pub lon: f64,
    pub house: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ref {
    pub id: String,
    pub glyph: String,
    pub name: String,
    /// The sign's classical element (fire/earth/air/water).
    pub element: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HouseRef {
    pub id: String,
    /// Roman numeral.
    pub label: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Aspect {
    pub id: String,
    pub glyph: String,
    pub name: String,
    /// Planet ids.
    pub a: String,
    pub b: String,
    /// Aspect kind ("trine", …) for routers matching aspect words; not part
    /// of the serialized artifact contract.
    #[serde(skip)]
    pub kind: &'static str,
}

#[derive(Debug, Serialize, Clone)]
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
