//! The `ChartData` contract between pipeline stages, mirroring the TS
//! interface in `docs/natal-reading-indexer.md`. The chart stage produces it,
//! the routing stage fills `excerpts`, and the emit stage serializes it into
//! the HTML artifact as `const DATA = {...}`. No stage owns it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct ChartData {
    pub meta: Meta,
    pub axes: Axes,
    #[serde(rename = "houseCusps")]
    pub house_cusps: Vec<f64>,
    /// Derived: how wide each house is, cusp to next cusp — see
    /// [`crate::derive::sweep`]. Parallel to `house_cusps`. Absent from charts
    /// saved before the field existed; `derive::fill` restores it on load.
    #[serde(rename = "houseSweeps", default)]
    pub house_sweeps: Vec<f64>,
    pub planets: Vec<Body>,
    pub signs: Vec<Ref>,
    pub houses: Vec<HouseRef>,
    pub aspects: Vec<Aspect>,
    /// Every note filed under this chart, spoken and written alike. The wire
    /// name stays `excerpts`: it is in every saved `chart.json` and in the
    /// `const DATA` of every artifact ever exported, both of which must keep
    /// loading. The word the project uses is *note* — see CONTEXT.md.
    #[serde(rename = "excerpts")]
    pub notes: Vec<Note>,
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
            ("house sweeps", self.house_sweeps.len()),
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
            // The derived position indexes `signs` directly and is rendered as
            // text; `derive::fill` guarantees the ranges, so a violation here
            // means the fields were never filled.
            if p.sign > 11 || p.deg > 29 || p.min > 59 {
                return Err(format!(
                    "planet {:?} has an out-of-range derived position {}/{}/{}",
                    p.id, p.sign, p.deg, p.min
                ));
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
        for note in &self.notes {
            for tag in &note.tags {
                if !vocab.contains(tag) {
                    return Err(format!("note {:?} references unknown tag {tag:?}", note.id));
                }
            }
            // An empty span is the wire sentinel for a written note, so a
            // spoken one may never carry it — otherwise the two kinds become
            // indistinguishable on the next round-trip.
            if note.span.is_some_and(|[s, e]| s >= e) {
                return Err(format!("note {:?} has an empty span", note.id));
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

// The note filter mode and `Note::matches` used to live here, documented
// as "shared by every viewer". They were shared by none: both viewers are
// JavaScript and implement it themselves — `desktop/src/lib/focus.ts`'s
// `matching` for the app, and `templates/reading.html`'s `matches` for the
// artifact — and nothing in Rust ever filtered a note, because the PDF prints
// them all. A third statement of a rule, in the module whose job is to be the
// shared contract, that no caller could reach. Deleting it concentrated nothing,
// which is what made it safe to delete.

impl Note {
    /// Merge a later note into this one: the span extends to cover both,
    /// tags become the sorted union, this note's time anchor is kept.
    /// The merged `text` is the caller's decision (verbatim transcript slice
    /// when one is at hand, joined parts otherwise).
    ///
    /// A written note has no span to extend, and nothing can extend into one:
    /// coalescing runs over a router's output, where every note is spoken. A
    /// hand-merge that reaches a written note keeps whichever provenance was
    /// already there rather than inventing one.
    pub fn absorb(&mut self, other: Note) {
        if let (Some(mine), Some(theirs)) = (self.span.as_mut(), other.span) {
            mine[1] = theirs[1];
        }
        let mut tags: Vec<String> = self.tags.drain(..).chain(other.tags).collect();
        tags.sort();
        tags.dedup();
        self.tags = tags;
    }
}

/// The locale a chart falls back to when `meta.locale` is absent (older files)
/// — English, matching the app's original behavior.
fn default_locale() -> String {
    "en".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct Meta {
    /// Stable identity for this reading, minted once when it is first saved.
    ///
    /// A reading's other names are all derived from things that move: its
    /// folder is `{slug}_{date}` from a name the astrologer can correct, and
    /// its path is under a library root the user can relocate. Nothing outside
    /// the chart could point at it and still be pointing a month later.
    ///
    /// Absent on charts saved before this existed and on CLI output; a
    /// migration stamps the library's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub id: Option<String>,
    pub name: String,
    pub born: String,
    pub place: String,
    /// Localized house-system label for display ("Whole Sign", "Placidus", …).
    pub system: String,
    /// Localized zodiac label for display ("Tropical", "Sidereal · Lahiri …").
    pub zodiac: String,
    /// Machine-readable house-system code (`whole-sign`, `placidus`, …) — the
    /// stable value for JSON consumers, distinct from the display `system`.
    /// Defaulted so charts written before this field still deserialize.
    #[serde(default)]
    pub house_system: String,
    /// Machine-readable ayanamsa code (`lahiri`, …) when the chart is sidereal;
    /// absent for tropical charts, keeping tropical JSON byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ayanamsa: Option<String>,
    /// Reading language as a short code (`en`, `ru`). Drives the router's
    /// match terms and every renderer's chrome; parsed via `i18n::Locale`.
    /// Defaulted so charts written before this field still deserialize.
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Practitioner branding for the artifact ("prepared by …"); absent
    /// unless a frontend stamps it, keeping unbranded output byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub astrologer: Option<String>,
    /// Practitioner logo as a `data:` URI — the artifact stays self-contained.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub logo: Option<String>,
    /// Machine-readable seed to recompute this chart's geometry (live house-
    /// system / zodiac swaps in the reading view). Stamped by the desktop build;
    /// absent on charts saved before this existed and on CLI output, which then
    /// simply aren't reprojectable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub birth: Option<BirthSeed>,
}

/// The minimum needed to reconstruct a `chart::BirthInput` for a live
/// recalculation: the gazetteer place id (re-resolves lat/lon/tz) plus the raw
/// birth date and time. Name, locale, and the calculation codes come from the
/// rest of `Meta`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct BirthSeed {
    pub place_id: u32,
    /// Birth date, `YYYY-MM-DD`.
    pub date: String,
    /// Local birth time, `HH:MM` or `HH:MM:SS`.
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct Axes {
    /// Ecliptic longitude in degrees; 0 = 0° Aries.
    pub asc: f64,
    pub mc: f64,
}

/// A chart body. `Default` exists so the compute stage can write the measured
/// fields and leave the derived ones to `derive::fill`, its only writer:
/// `Body { id, glyph, name, lon, house, ..Default::default() }`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct Body {
    pub id: String,
    pub glyph: String,
    pub name: String,
    pub lon: f64,
    pub house: u8,
    /// Derived from `lon` by [`crate::derive::position`]: the sign it falls in
    /// (`0..=11`, indexing [`ChartData::signs`]) and how far into it. Every
    /// renderer reads these instead of re-deriving them — the PDF, the artifact
    /// and the desktop wheel each used to carry their own copy of the
    /// arithmetic and had drifted. Absent from charts saved before the fields
    /// existed; `derive::fill` restores them on load.
    #[serde(default)]
    pub sign: u8,
    /// Whole degrees into the sign, `0..=29`.
    #[serde(default)]
    pub deg: u8,
    /// Arcminutes, `0..=59`.
    #[serde(default)]
    pub min: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct Ref {
    pub id: String,
    pub glyph: String,
    pub name: String,
    /// The sign's classical element (fire/earth/air/water).
    pub element: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct HouseRef {
    pub id: String,
    /// Roman numeral.
    pub label: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
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
    /// Deviation from the aspect's exact angle, in degrees — the orb. Computed
    /// once at detect time (see `chart::detect_aspects`) so no viewer re-derives
    /// it from longitudes. Defaulted so charts written before this field load.
    #[serde(default)]
    pub orb: f64,
    /// Aspect kind ("trine", …) for routers matching aspect words; not part
    /// of the serialized artifact contract.
    #[serde(skip)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub kind: &'static str,
}

/// One note filed under the chart elements it refers to.
///
/// A note is **spoken** or **written**, and `span` is what tells them apart.
/// A spoken note carries byte offsets into the transcript and its `text` is
/// verbatim — that is provenance, not a formatting preference. A written note
/// was authored by hand, has no transcript to point at, and carries no span.
///
/// Both kinds are the same thing to every consumer: they file under the same
/// vocabulary, filter through the same rules, and render alike.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct Note {
    /// Unique among the chart's notes (the `x{n}` scheme is convention;
    /// uniqueness is the invariant consumers rely on).
    pub id: String,
    /// "HH:MM:SS" anchor into the recording; empty for a written note, and
    /// for a spoken one whose transcript carries no timestamps.
    pub time: String,
    /// Byte offsets into the transcript — provenance. `None` for a written
    /// note. See the [`span`] module for why the wire shape is unchanged.
    #[serde(with = "span")]
    #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
    pub span: Option<[usize; 2]>,
    /// VERBATIM when `span` is `Some`; must equal `transcript[span.0..span.1]`.
    pub text: String,
    /// Tag-ids; each must exist in the chart vocabulary. May be empty — a note
    /// nothing could be filed under is shown whenever no filter is active.
    pub tags: Vec<String>,
}

/// `Option<[usize; 2]>` in Rust, `[0, 0]` on the wire.
///
/// A written note has always been stored as `span: [0, 0]`, a sentinel upheld
/// by a comment at its one writer and understood nowhere else. The type now
/// says which kind a note is, and this module keeps every chart ever written —
/// and every artifact already in someone's hands — byte-identical.
///
/// The mapping is total in both directions because a real span can never be
/// `[0, 0]`: [`ChartData::validate`] and the Verify gate both reject an empty
/// one, so the sentinel is unambiguous.
mod span {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<[usize; 2]>, s: S) -> Result<S::Ok, S::Error> {
        v.unwrap_or([0, 0]).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<[usize; 2]>, D::Error> {
        let raw = <[usize; 2]>::deserialize(d)?;
        Ok(if raw[0] == raw[1] { None } else { Some(raw) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared minimal chart plus one note, so the vocabulary check has
    /// something to check. Every assertion below mutates a copy of it.
    fn valid_chart() -> ChartData {
        let mut chart = crate::fixtures::minimal_chart();
        chart.notes = vec![crate::fixtures::note("x1", "hi", &["planet:sun"])];
        chart
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
        c.notes[0].tags = vec!["planet:pluto".into()];
        assert!(c.validate().is_err());

        // an empty span is the written-note sentinel; a spoken note claiming
        // one would come back written on the next round-trip
        let mut c = valid_chart();
        c.notes[0].span = Some([7, 7]);
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

    /// The wire is what every saved chart and every exported artifact already
    /// holds. `Option<[usize; 2]>` is a statement about the Rust type only —
    /// if it leaked into the JSON, charts written by older builds and artifacts
    /// already in someone's hands would stop agreeing with this one.
    #[test]
    fn the_span_sentinel_keeps_the_wire_unchanged() {
        let written = Note {
            id: "x1".into(),
            time: String::new(),
            span: None,
            text: "authored".into(),
            tags: vec!["planet:sun".into()],
        };
        let json = serde_json::to_string(&written).unwrap();
        assert!(json.contains("\"span\":[0,0]"), "a written note still writes the sentinel: {json}");

        let spoken = Note { span: Some([4, 12]), ..written.clone() };
        let json = serde_json::to_string(&spoken).unwrap();
        assert!(json.contains("\"span\":[4,12]"), "{json}");

        // and both come back as the kind they went in as
        let back: Note = serde_json::from_str(&serde_json::to_string(&written).unwrap()).unwrap();
        assert_eq!(back.span, None, "[0,0] reads back as written, not as a span");
        let back: Note = serde_json::from_str(&serde_json::to_string(&spoken).unwrap()).unwrap();
        assert_eq!(back.span, Some([4, 12]));

        // any degenerate span means written — a real one can never be empty,
        // which is what `validate` enforces
        let degenerate: Note = serde_json::from_str(
            r#"{"id":"x1","time":"","span":[9,9],"text":"t","tags":[]}"#,
        )
        .unwrap();
        assert_eq!(degenerate.span, None);
    }

    /// The field renamed; the JSON key did not.
    #[test]
    fn notes_are_still_spelled_excerpts_on_the_wire() {
        let json = serde_json::to_string(&valid_chart()).unwrap();
        assert!(json.contains("\"excerpts\":["), "{json}");
        assert!(!json.contains("\"notes\":"), "{json}");

        let back: ChartData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.notes.len(), 1);
    }

    /// Absent on everything written before it existed, and absent from the
    /// JSON when unset — a chart without an id must stay byte-identical.
    #[test]
    fn a_reading_id_is_optional_and_omitted_when_unset() {
        let mut c = valid_chart();
        assert_eq!(c.meta.id, None, "the fixture mints none");
        // scoped to `meta` — every element in the chart carries an `id` of its own
        let meta = serde_json::to_string(&c.meta).unwrap();
        assert!(!meta.contains("\"id\":"), "no id key when unset: {meta}");

        c.meta.id = Some("r-1".into());
        let json = serde_json::to_string(&c).unwrap();
        let back: ChartData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.meta.id.as_deref(), Some("r-1"));
    }
}
