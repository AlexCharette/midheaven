//! Stage 4 — inject the assembled `ChartData` into the single-file HTML
//! template. The template embeds all CSS/JS inline and makes no network
//! calls, so the artifact runs from `file://`.

use crate::contract::ChartData;

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

pub fn emit(data: &ChartData) -> Result<String, String> {
    let chart = serde_json::to_string(data).map_err(|e| e.to_string())?;
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
pub fn write_artifact(data: &ChartData, path: &std::path::Path) -> Result<(), String> {
    let html = emit(data)?;
    std::fs::write(path, html).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_html_is_self_contained() {
        let data = crate::fixtures::minimal_chart();
        let html = emit(&data).unwrap();
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
}
