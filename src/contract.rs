//! The `ChartData` contract between pipeline stages: a computed sky and
//! nothing else. The chart stage produces it and the emit stage serializes it
//! into the HTML artifact. No stage owns it.
//!
//! It used to carry the notes filed under it, which made the chart the thing
//! that owned them — a note could not exist without one, and "every note I
//! have written about this planet" could only ever be asked of a single
//! reading. Notes live in their own store now and are handed to the renderers
//! beside the chart rather than inside it.

use serde::{Deserialize, Serialize};

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
    /// The five aspect kinds, as the `kind:` category renders them.
    ///
    /// Global, like the signs and the houses beside it — every chart has the
    /// same five — and here for the same reason those are: a viewer needs the
    /// glyph and the localized word to draw a tag, and nothing else in the
    /// artifact carries them.
    #[serde(default)]
    pub kinds: Vec<KindRef>,
}

impl ChartData {
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
        for k in &self.kinds {
            check_id(&k.id, "kind")?;
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
    /// Every tag is in the vocabulary, and the words are not only whitespace.
    ///
    /// `ChartData::validate` used to check a note's tags, back when notes rode
    /// on the chart. They arrive from their own store now, and from the same
    /// untrusted direction a chart does, so the check moved with them.
    pub fn validate(&self) -> Result<(), String> {
        if self.text.trim().is_empty() {
            return Err(format!("note {:?} has no words", self.id));
        }
        for tag in &self.tags {
            if !crate::vocab::contains(tag) {
                return Err(format!("note {:?} references unknown tag {tag:?}", self.id));
            }
        }
        Ok(())
    }

    /// Merge a later note into this one: tags become the sorted union, this
    /// note's `at` anchor is kept, and the merged `text` is the caller's
    /// decision.
    ///
    /// Merging used to extend a span as well, which is why the caller could
    /// leave `text` alone and have it re-derived. Nothing stores a span now,
    /// so a caller that is merging two notes owns the result's words: see
    /// `route::coalesce`, which still has the transcript at hand and takes the
    /// verbatim slice across both.
    pub fn absorb(&mut self, other: Note) {
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
///
/// Compared by value, so two notes written over the same moment can be
/// recognised as being about the same sky.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// One aspect kind — `kind:trine` and the glyph and word that render it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct KindRef {
    pub id: String,
    pub glyph: String,
    pub name: String,
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

/// One note, filed under the vocabulary tags it refers to.
///
/// Notes used to come in two kinds. A **spoken** one carried byte offsets into
/// its transcript and a `text` that had to equal that slice exactly; a
/// **written** one was authored by hand and carried no offsets. The distinction
/// was real at routing time and inert everywhere after it: both kinds filed
/// under the same vocabulary, filtered through the same rules and rendered
/// alike, and nothing in the codebase ever read a stored span back.
///
/// What made it worth removing is what it cost to keep. The absent span had to
/// be spelled `[0, 0]` on the wire, the sentinel had to be unambiguous, so an
/// empty span had to be an error, so `validate` and the Verify gate both
/// carried a rule about a case that could not arise. Provenance is still
/// enforced — `route::verify_gate` is the thing that enforces it — it is just
/// no longer carried around by every note for the rest of its life.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
pub struct Note {
    /// Unique among the notes it is filed beside. The `x{n}` scheme is
    /// convention; uniqueness is the invariant consumers rely on.
    pub id: String,
    /// "HH:MM:SS" into the recording this note was spoken in.
    ///
    /// `None` for a note written by hand, and for a spoken one whose
    /// transcript carried no timestamps. It was a `String` whose emptiness
    /// meant absence — the same sentinel shape as the span's `[0, 0]`, and
    /// removed for the same reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub at: Option<String>,
    /// The words. Verbatim from the transcript for a spoken note — the Verify
    /// gate refuses anything else — and whatever was typed for a written one.
    pub text: String,
    /// Tag-ids, each of which must be in [`crate::vocab`]. May be empty: a
    /// note nothing could be filed under is shown whenever no filter is
    /// active.
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_chart() -> ChartData {
        crate::fixtures::minimal_chart()
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

    /// A note arrives from its own store, from the same untrusted direction a
    /// chart does, so it checks itself. `planet:pluto` would not do as the bad
    /// tag here: the vocabulary is global, so Pluto is real whether or not any
    /// particular chart carries the body.
    #[test]
    fn a_note_refuses_an_unknown_tag_and_empty_words() {
        let ok = crate::fixtures::note("x1", "hi", &["planet:sun", "kind:trine"]);
        assert!(ok.validate().is_ok());

        let bad = crate::fixtures::note("x1", "hi", &["planet:vulcan"]);
        assert!(bad.validate().is_err());

        let empty = crate::fixtures::note("x1", "   ", &["planet:sun"]);
        assert!(empty.validate().is_err());
    }

    /// An absent time anchor is absent from the JSON, not an empty string.
    ///
    /// This is the shape the span sentinel did not have. `span: [0, 0]` meant
    /// "no span" and `time: ""` meant "no time", and in both cases the reader
    /// had to know the convention to get the right answer back. One `Option`
    /// says it in the type, and a note with nothing to say about when simply
    /// does not mention it.
    #[test]
    fn an_absent_time_anchor_is_absent_from_the_wire() {
        let written = Note {
            id: "x1".into(),
            at: None,
            text: "authored".into(),
            tags: vec!["planet:sun".into()],
        };
        let json = serde_json::to_string(&written).unwrap();
        assert!(!json.contains("\"at\""), "no key at all when unset: {json}");
        assert_eq!(serde_json::from_str::<Note>(&json).unwrap(), written);

        let spoken = Note { at: Some("00:04:12".into()), ..written.clone() };
        let json = serde_json::to_string(&spoken).unwrap();
        assert!(json.contains("\"at\":\"00:04:12\""), "{json}");
        assert_eq!(serde_json::from_str::<Note>(&json).unwrap(), spoken);
    }

    /// A chart is a sky and nothing else. `excerpts` was the wire name of the
    /// note list it used to carry, kept for compatibility with saved files
    /// that no longer need keeping.
    #[test]
    fn a_chart_carries_no_notes_under_any_name() {
        let json = serde_json::to_string(&valid_chart()).unwrap();
        for gone in ["\"excerpts\"", "\"notes\""] {
            assert!(!json.contains(gone), "{gone} is still on the wire: {json}");
        }
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
