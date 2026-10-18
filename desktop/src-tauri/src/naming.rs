//! What an exported file is called.
//!
//! This is what is left of `library`, which managed a folder of reading
//! folders: one `chart.json` and its transcripts per reading, listed by
//! reading off disk, with ids stamped into the charts so a note could point
//! at one. The store replaced all of it — a chart is a record, a note names
//! it, and nothing walks a tree of folders to find out what exists.
//!
//! The naming rule outlived it because an export still needs a filename, and
//! that filename is still the reading's name and its date.

/// A name as a filename-safe slug: lowercase, non-alphanumerics to `_`, runs
/// collapsed to one `_`.
fn slug(name: &str) -> String {
    let parts: Vec<String> = name
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();
    if parts.is_empty() { "reading".to_string() } else { parts.join("_") }
}

/// A reading's name stem: `{slug}_{YYYY-MM-DD}`.
pub fn stem(name: &str, on: chrono::NaiveDate) -> String {
    format!("{}_{}", slug(name), on.format("%Y-%m-%d"))
}

/// The suggested export filename for a stem.
pub fn artifact_name(stem: &str) -> String {
    format!("{stem}.html")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> chrono::NaiveDate {
        "2026-08-01".parse().unwrap()
    }

    #[test]
    fn a_name_becomes_a_filename_nobody_has_to_quote() {
        assert_eq!(stem("Mira Holt", on()), "mira_holt_2026-08-01");
        assert_eq!(stem("  Mira   Holt  ", on()), "mira_holt_2026-08-01");
        assert_eq!(stem("Mira/Holt", on()), "mira_holt_2026-08-01");
        assert_eq!(stem("Мира Холт", on()), "мира_холт_2026-08-01");
    }

    /// A name with nothing filename-shaped in it still has to produce a file.
    #[test]
    fn a_nameless_reading_still_gets_a_name() {
        assert_eq!(stem("", on()), "reading_2026-08-01");
        assert_eq!(stem("***", on()), "reading_2026-08-01");
    }

    #[test]
    fn the_export_name_is_the_stem_with_an_extension() {
        assert_eq!(artifact_name(&stem("Mira Holt", on())), "mira_holt_2026-08-01.html");
    }
}
