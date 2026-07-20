//! Stage 4 — inject the assembled `ChartData` into the single-file HTML
//! template. The template embeds all CSS/JS inline and makes no network
//! calls, so the artifact runs from `file://`.

use crate::contract::ChartData;

const TEMPLATE: &str = include_str!("../templates/reading.html");
const PLACEHOLDER: &str = "/*__DATA__*/null";

pub fn emit(data: &ChartData) -> Result<String, String> {
    let json = serde_json::to_string(data).map_err(|e| e.to_string())?;
    // `</script>` inside a JSON string would terminate the script block early.
    let json = json.replace("</", "<\\/");
    match TEMPLATE.matches(PLACEHOLDER).count() {
        1 => Ok(TEMPLATE.replacen(PLACEHOLDER, &json, 1)),
        n => Err(format!("template must contain exactly one `{PLACEHOLDER}` placeholder, found {n}")),
    }
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
    use crate::contract::*;

    #[test]
    fn emitted_html_is_self_contained() {
        let data = ChartData {
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
            planets: vec![],
            signs: vec![],
            houses: vec![],
            aspects: vec![],
            excerpts: vec![],
        };
        let html = emit(&data).unwrap();
        assert!(html.contains("const DATA = {"));
        assert!(!html.contains(PLACEHOLDER));
        // No external references: nothing may be fetched at view time.
        // (The SVG namespace URI is an identifier, not a request.)
        for needle in ["src=", "href=", "url(", "@import", "fetch(", "XMLHttpRequest"] {
            assert!(!html.contains(needle), "external reference found: {needle}");
        }
    }
}
