//! Stage 4 — inject the assembled `ChartData` into the single-file HTML
//! template. The template embeds all CSS/JS inline and makes no network
//! calls, so the artifact runs from `file://`.

use crate::contract::{ChartData, Note};

const TEMPLATE: &str = include_str!("../templates/reading.html");
const DATA: &str = "/*__DATA__*/null";
/// The plate geometry, substituted the same way the chart is. The template used
/// to carry its own copy of the radii and tick classes, kept in step with
/// `src/pdf/wheel.rs` by a comment; now it is emitted from
/// [`crate::plate::PLATE`] by the same binary that draws the paper rendition,
/// so the two cannot disagree.
const PLATE: &str = "/*__PLATE__*/null";
/// The viewer's own furniture, in the reading's language. Substituted the same
/// way, from `i18n` — the template used to carry a `UI = { en, ru }` table
/// parallel to the Rust one, so adding a language meant editing both.
const CHROME: &str = "/*__CHROME__*/null";

/// What the artifact's `const DATA` holds: the chart's own fields, flattened,
/// plus the notes to show against it.
///
/// The notes used to be a field *on* `ChartData` — spelled `excerpts` on the
/// wire, because that name was in every saved `chart.json` and every artifact
/// already exported. Neither constraint survives: notes are not part of a
/// chart any more, and the artifact is the only thing that reads this shape.
/// `emit::tests::every_data_field_the_script_reads_is_one_the_chart_emits` is
/// what keeps this struct and the template's `DATA.x` reads in step.
#[derive(serde::Serialize)]
struct ArtifactData<'a> {
    #[serde(flatten)]
    chart: &'a ChartData,
    notes: &'a [Note],
}

pub fn emit(data: &ChartData, notes: &[Note]) -> Result<String, String> {
    let chart = serde_json::to_string(&ArtifactData { chart: data, notes })
        .map_err(|e| e.to_string())?;
    let plate = serde_json::to_string(&crate::plate::PLATE).map_err(|e| e.to_string())?;
    let loc = crate::i18n::Locale::parse(&data.meta.locale);
    // The plate title is shared with the PDF, so it lives beside the chrome
    // rather than inside it, and is flattened in for the viewer.
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Chrome<'a> {
        #[serde(flatten)]
        artifact: &'a crate::i18n::ArtifactChrome,
        plate_title: crate::i18n::PlateTitle,
    }
    let chrome = Chrome { artifact: loc.artifact(), plate_title: loc.plate_title() };
    let chrome = serde_json::to_string(&chrome).map_err(|e| e.to_string())?;
    let mut out = TEMPLATE.to_string();
    for (placeholder, json) in [(DATA, chart), (PLATE, plate), (CHROME, chrome)] {
        // `</script>` inside a JSON string would terminate the script block early.
        let json = json.replace("</", "<\\/");
        match out.matches(placeholder).count() {
            1 => out = out.replacen(placeholder, &json, 1),
            n => {
                return Err(format!(
                    "template must contain exactly one `{placeholder}` placeholder, found {n}"
                ));
            }
        }
    }
    Ok(out)
}

/// Render the artifact and write it — the emit-then-write idiom shared by
/// the CLI and the desktop app.
pub fn write_artifact(
    data: &ChartData,
    notes: &[Note],
    path: &std::path::Path,
) -> Result<(), String> {
    let html = emit(data, notes)?;
    std::fs::write(path, html).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_html_is_self_contained() {
        let data = crate::fixtures::minimal_chart();
        let html = emit(&data, &[]).unwrap();
        assert!(html.contains("const DATA = {"));
        assert!(!html.contains(DATA), "the chart placeholder must be filled");
        assert!(!html.contains(PLATE), "the plate placeholder must be filled");
        assert!(!html.contains(CHROME), "the chrome placeholder must be filled");
        // The plate rides in the artifact, so the viewer needs no copy of it.
        assert!(html.contains("\"decadeLen\":12"), "the emitted plate carries its tick classes");
        assert!(html.contains("\"houseLabel\":112"), "and its radii");
        // No external references: nothing may be fetched at view time.
        // (The SVG namespace URI is an identifier, not a request.)
        for needle in ["src=", "href=", "url(", "@import", "fetch(", "XMLHttpRequest"] {
            assert!(!html.contains(needle), "external reference found: {needle}");
        }
    }

    /// A `getElementById` whose element was edited out of the markup throws
    /// at view time and kills everything after it — the artifact renders
    /// "almost empty". Catch the rot here instead.
    #[test]
    fn every_dom_id_the_script_references_exists_in_the_markup() {
        let mut rest = TEMPLATE;
        let mut checked = 0;
        while let Some(at) = rest.find("getElementById('") {
            rest = &rest[at + "getElementById('".len()..];
            let id = &rest[..rest.find('\'').expect("unterminated id")];
            // dynamic ids (template literals) are covered by their creation site
            if !id.contains("${") {
                assert!(
                    TEMPLATE.contains(&format!("id=\"{id}\"")),
                    "script references #{id} but no element carries it"
                );
                checked += 1;
            }
        }
        assert!(checked >= 8, "only {checked} ids checked — did the scan break?");
    }

    /// Every field of the plate the viewer's script reads has to be a field the
    /// plate actually serializes.
    ///
    /// A missing one is silent: the radius comes back `undefined`, the arithmetic
    /// on it is `NaN`, and SVG reads a `NaN` coordinate as zero — so the affected
    /// glyphs pile up in the top-left corner of the plate instead of failing.
    /// That is how `R.houseLbl` outlived the rename to `houseLabel` and shipped a
    /// wheel with all twelve house numerals stacked in the corner. The other two
    /// renditions cannot have this bug: `src/pdf/wheel.rs` reads the struct and
    /// the compiler checks it, and `Wheel.svelte` reads the generated
    /// `plate.ts` under `svelte-check`. Only this template reads the plate as
    /// untyped JSON, so this is that rendition's type check.
    #[test]
    fn every_plate_field_the_script_reads_is_one_the_plate_emits() {
        let plate = serde_json::to_value(crate::plate::PLATE).expect("the plate serializes");
        // The aliases the script binds the plate's three sub-objects to. They are
        // shouted precisely so this scan can find them: `t` and `c` also name a
        // DOM node and a cusp longitude elsewhere in the file.
        let scopes = [
            ("PLATE", &plate),
            ("R", &plate["radii"]),
            ("TICK", &plate["ticks"]),
            ("CROWD", &plate["crowding"]),
        ];
        let mut checked = 0;
        for (alias, object) in scopes {
            let keys = object.as_object().expect("a plate object");
            let needle = format!("{alias}.");
            for (at, _) in TEMPLATE.match_indices(&needle) {
                // `CROWD.step` is not a use of `D.`, and `prevR.x` is not one of `R.`
                let before = TEMPLATE[..at].chars().next_back();
                if before.is_some_and(|ch| ch.is_alphanumeric() || ch == '_') {
                    continue;
                }
                let rest = &TEMPLATE[at + needle.len()..];
                let end = rest.find(|ch: char| !ch.is_alphanumeric() && ch != '_').unwrap_or(0);
                let field = &rest[..end];
                assert!(
                    keys.contains_key(field),
                    "the script reads {alias}.{field}, which the plate does not emit — \
                     it has {:?}",
                    keys.keys().collect::<Vec<_>>()
                );
                checked += 1;
            }
        }
        // Every radius, every tick class, every crowding term, and the three
        // loose fields: a scan that stopped finding them would pass by default.
        assert!(checked >= 25, "only {checked} plate reads checked — did the scan break?");
    }

    /// A note's id becomes a DOM id in the artifact, so a blank or repeated
    /// one is not an error — it is a page of elements that all answer to the
    /// same name, and anchors that land on whichever came first.
    #[test]
    fn every_emitted_note_has_an_id_of_its_own() {
        let chart = crate::fixtures::minimal_chart();
        let notes = vec![
            crate::fixtures::note("x1", "one", &["planet:sun"]),
            crate::fixtures::note("x2", "two", &["planet:moon"]),
        ];
        let html = emit(&chart, &notes).unwrap();
        assert!(html.contains("\"id\":\"x1\""), "ids reach the artifact");

        // And the pipeline that feeds it gives them out.
        let (_chart, routed, _report) = crate::build_reading(
            &crate::fixtures::named("Ids"),
            crate::TranscriptSource::File(
                concat!(env!("CARGO_MANIFEST_DIR"), "/examples/transcript.jsonl").into(),
            ),
            |_| {},
        )
        .expect("build_reading");
        let ids: std::collections::BTreeSet<&str> =
            routed.iter().map(|n| n.id.as_str()).collect();
        assert!(!routed.is_empty());
        assert!(!ids.contains(""), "a blank id is a DOM id collision waiting to happen");
        assert_eq!(ids.len(), routed.len(), "and they must be distinct: {ids:?}");
    }

    /// Every tag category has somewhere to render.
    ///
    /// The viewers resolve a tag through one `lookup` table and drop anything
    /// that misses it — `if (!ref) return ''`. A category the table does not
    /// cover therefore does not error, it just silently stops appearing, which
    /// is how `kind:` behaved for exactly as long as it took to write this.
    #[test]
    fn every_vocabulary_category_reaches_the_artifacts_lookup_table() {
        let html = emit(&crate::fixtures::minimal_chart(), &[]).unwrap();
        // The table the script builds, as source — the categories it spreads in.
        let built = html
            .split_once("const lookup = {};")
            .expect("the artifact builds a lookup table")
            .1;
        let spread = &built[..built.find(".forEach").expect("the table is built from a spread")];
        for cat in crate::vocab::CATEGORIES {
            let field = match cat {
                "planet" => "planets",
                "sign" => "signs",
                "house" => "houses",
                "aspect" => "aspects",
                "kind" => "kinds",
                other => panic!("unmapped category {other}"),
            };
            assert!(
                spread.contains(&format!("DATA.{field}")),
                "{cat}: tags would resolve against nothing and vanish — \
                 the lookup table is built from {spread:?}"
            );
        }
    }

    /// The same check, one level up: every `DATA.x` the script reads has to be a
    /// key `ChartData` actually serializes.
    ///
    /// The plate had this bug and the chart can have it too, with a worse blast
    /// radius: `DATA.excerpts` coming back `undefined` throws inside `render`,
    /// which kills every listener registered after it. The artifact opens
    /// looking almost right and nothing filters.
    ///
    /// This is the test that makes the `excerpts` → `notes` wire rename safe.
    /// Renaming the Rust field alone fails here; renaming it in the template
    /// alone fails here; renaming both passes. That is the whole point — the
    /// two names are one decision and nothing else in the suite says so.
    #[test]
    fn every_data_field_the_script_reads_is_one_the_chart_emits() {
        let chart = serde_json::to_value(ArtifactData {
            chart: &crate::fixtures::minimal_chart(),
            notes: &[],
        })
        .expect("the chart serializes");
        let keys = chart.as_object().expect("a chart object");
        let mut checked = 0;
        for (at, _) in TEMPLATE.match_indices("DATA.") {
            // `__DATA__` and any `xDATA.` are not uses of `DATA.`
            let before = TEMPLATE[..at].chars().next_back();
            if before.is_some_and(|ch| ch.is_alphanumeric() || ch == '_') {
                continue;
            }
            let rest = &TEMPLATE[at + "DATA.".len()..];
            let end = rest.find(|ch: char| !ch.is_alphanumeric() && ch != '_').unwrap_or(0);
            let field = &rest[..end];
            assert!(
                keys.contains_key(field),
                "the script reads DATA.{field}, which the chart does not emit — \
                 it has {:?}",
                keys.keys().collect::<Vec<_>>()
            );
            checked += 1;
        }
        // meta, axes, houseCusps, houseSweeps, planets, signs, houses, aspects
        // and the note list: nine distinct fields, read more than once each.
        assert!(checked >= 9, "only {checked} DATA reads checked — did the scan break?");
    }
}
