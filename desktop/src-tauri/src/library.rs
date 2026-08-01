//! The readings library: a folder of saved readings on disk.
//!
//! It was already a concept in the product — CONTEXT.md says a reading is what
//! "gets built, curated, saved to the library, and exported" — and eight loose
//! helpers across five regions of `lib.rs`. Its folder convention was spelled
//! out twice, its export-name convention twice more, and three commands each
//! resolved its root from preferences for themselves.
//!
//! Here it is one thing rooted at one path. That the root is handed in rather
//! than fetched from an `AppHandle` is what makes it testable: the app passes
//! the configured readings folder, the tests pass a temporary directory.

use astro::ArchivedTranscript;
use astro::contract::ChartData;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// The file a reading's chart lives in, inside its folder.
const CHART: &str = "chart.json";

/// Filesystem-safe name stem: lowercase, runs of anything non-alphanumeric
/// collapse to one `_`.
fn slug(name: &str) -> String {
    let parts: Vec<String> = name
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();
    if parts.is_empty() { "reading".to_string() } else { parts.join("_") }
}

/// A reading's name stem: `{slug}_{YYYY-MM-DD}`. The library folder is this,
/// and so is the export name — see [`artifact_name`]. Both were written out at
/// two call sites each.
pub fn stem(name: &str, on: chrono::NaiveDate) -> String {
    format!("{}_{}", slug(name), on.format("%Y-%m-%d"))
}

/// The suggested export filename for a stem.
pub fn artifact_name(stem: &str) -> String {
    format!("{stem}.html")
}

/// Write a chart into an already-open reading's folder. Used by the session
/// through the whole reading, not only at save time — curation and takes
/// refresh it.
pub fn save_chart(dir: &Path, chart: &ChartData) -> Result<(), String> {
    let path = dir.join(CHART);
    let json = serde_json::to_string_pretty(chart).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Highest `n` among the `take-{n}.jsonl` files already in a reading folder, so
/// a take recorded after reopening never overwrites one. 0 when none exist or
/// the folder can't be read.
pub fn max_take_ordinal(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter_map(|n| {
                    n.strip_prefix("take-")
                        .and_then(|r| r.strip_suffix(".jsonl"))
                        .and_then(|d| d.parse::<usize>().ok())
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// One row of the readings library: enough to list and reopen a saved reading
/// without the frontend touching the filesystem.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct ReadingEntry {
    /// `{dir}/chart.json` — fed straight to `load_chart`.
    pub chart_path: String,
    /// The reading's folder — fed to `delete_reading`.
    pub dir: String,
    pub name: String,
    pub born: String,
    pub place: String,
    pub excerpts: usize,
    /// `chart.json`'s mtime, ms since the epoch — sort key and "saved" date.
    /// Serialized as a JSON number; ms-since-epoch stays within JS's safe
    /// integer range for millennia, so the binding is `number`, not `bigint`.
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub modified_ms: Option<u64>,
}

/// Just the fields a listing row shows.
///
/// Listing used to deserialize the *entire* `ChartData` — every body, aspect and
/// passage — to display five strings and a count, for every folder, every time
/// the panel opened. `IgnoredAny` gives the passage count without building a
/// single `Excerpt`, and the rest of the chart is never looked at.
#[derive(serde::Deserialize)]
struct Listing {
    meta: ListingMeta,
    #[serde(default)]
    excerpts: Vec<serde::de::IgnoredAny>,
}

#[derive(serde::Deserialize)]
struct ListingMeta {
    name: String,
    born: String,
    place: String,
}

/// A folder of saved readings.
pub struct Library {
    root: PathBuf,
}

impl Library {
    /// The configured library, or `None` when no readings folder is set — which
    /// is a normal state, not an error: a reading simply is not auto-saved.
    pub fn configured(root: Option<&str>) -> Option<Library> {
        let root = root.map(str::trim).filter(|r| !r.is_empty())?;
        Some(Library { root: PathBuf::from(root) })
    }

    /// Create a reading's folder and write everything it starts with: the
    /// chart, and the transcript its passages were routed from.
    ///
    /// The transcript matters as much as the chart — a passage is verbatim by
    /// definition, so without the words its span points at nothing.
    pub fn create(
        &self,
        stem: &str,
        chart: &ChartData,
        transcript: Option<&ArchivedTranscript>,
    ) -> Result<PathBuf, String> {
        let dir = self.root.join(stem);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        if let Some(t) = transcript {
            std::fs::write(dir.join(&t.filename), &t.contents)
                .map_err(|e| format!("cannot write {}: {e}", t.filename))?;
        }
        save_chart(&dir, chart)?;
        Ok(dir)
    }

    /// Every saved reading, newest first. A folder that holds no chart, or one
    /// that cannot be read, is skipped rather than reported — the library is a
    /// place a person also puts things.
    pub fn entries(&self) -> Vec<ReadingEntry> {
        let mut entries: Vec<ReadingEntry> = std::fs::read_dir(&self.root)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .filter_map(|p| entry(&p))
                    .collect()
            })
            .unwrap_or_default();
        // newest first; entries without an mtime sink to the end
        entries.sort_by_key(|e| std::cmp::Reverse(e.modified_ms));
        entries
    }

    /// Remove a reading, folder and all.
    ///
    /// The target must canonicalize to a *direct* child of the root and hold a
    /// chart, so neither a path outside the library nor a folder that is not a
    /// reading can be deleted — the frontend hands back a string it was given,
    /// and this is what makes trusting it unnecessary.
    pub fn remove(&self, dir: &str) -> Result<(), String> {
        let target = self.resolve(dir)?;
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("cannot remove {}: {e}", target.display()))
    }

    fn resolve(&self, dir: &str) -> Result<PathBuf, String> {
        let root = std::fs::canonicalize(&self.root).map_err(|e| e.to_string())?;
        let target = std::fs::canonicalize(dir).map_err(|e| format!("no folder at {dir}: {e}"))?;
        if target.parent() != Some(root.as_path()) {
            return Err("that folder is not in the readings library".to_string());
        }
        if !target.join(CHART).is_file() {
            return Err("that folder is not a saved reading".to_string());
        }
        Ok(target)
    }
}

/// Read a folder's chart into a listing row, or `None` if it holds none.
fn entry(dir: &Path) -> Option<ReadingEntry> {
    let chart_path = dir.join(CHART);
    let raw = std::fs::read_to_string(&chart_path).ok()?;
    let listing: Listing = serde_json::from_str(&raw).ok()?;
    let modified_ms = std::fs::metadata(&chart_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    Some(ReadingEntry {
        chart_path: chart_path.to_string_lossy().into_owned(),
        dir: dir.to_string_lossy().into_owned(),
        name: listing.meta.name,
        born: listing.meta.born,
        place: listing.meta.place,
        excerpts: listing.excerpts.len(),
        modified_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library rooted at a temporary directory — the second adapter, and the
    /// reason any of this can be tested at all. The path is derived from the
    /// test's own name so concurrent tests do not share one.
    struct TempLibrary {
        lib: Library,
        root: PathBuf,
    }

    impl TempLibrary {
        fn new(name: &str) -> TempLibrary {
            let root = std::env::temp_dir().join(format!("midheaven-library-{name}"));
            std::fs::remove_dir_all(&root).ok();
            std::fs::create_dir_all(&root).expect("temp root");
            let lib = Library::configured(root.to_str()).expect("a rooted library");
            TempLibrary { lib, root }
        }
    }

    impl Drop for TempLibrary {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    fn chart(name: &str) -> ChartData {
        let input = astro::chart::BirthInput {
            name: name.into(),
            date: "1990-07-13".parse().unwrap(),
            time: "14:30:00".parse().unwrap(),
            lat: 52.52,
            lon: 13.405,
            tz: chrono_tz::Europe::Berlin,
            place: "Berlin".into(),
            locale: astro::i18n::Locale::En,
            house_system: astro::chart::systems::house_system("whole-sign"),
            ayanamsa: None,
        };
        astro::chart::compute_chart(&input).unwrap()
    }

    fn on(date: &str) -> chrono::NaiveDate {
        date.parse().unwrap()
    }

    #[test]
    fn a_stem_is_filesystem_safe_and_dated() {
        assert_eq!(stem("Mira Holt", on("2026-08-01")), "mira_holt_2026-08-01");
        assert_eq!(stem("  Ada  O'Neill-Smith ", on("2026-08-01")), "ada_o_neill_smith_2026-08-01");
        assert_eq!(stem("Мира", on("2026-08-01")), "мира_2026-08-01", "non-ascii letters survive");
        assert_eq!(stem("  Ana-María d'Été  ", on("2026-08-01")), "ana_maría_d_été_2026-08-01");
        assert_eq!(stem("···", on("2026-08-01")), "reading_2026-08-01");
        assert_eq!(stem("", on("2026-08-01")), "reading_2026-08-01");
        assert_eq!(stem("???", on("2026-08-01")), "reading_2026-08-01");
        // No separator can escape the folder.
        for name in ["../../etc", "a/b", "c\\d", "e:f"] {
            let s = stem(name, on("2026-08-01"));
            assert!(!s.contains(['/', '\\', ':']), "{name:?} produced {s:?}");
            assert!(!s.contains(".."), "{name:?} produced {s:?}");
        }
    }

    #[test]
    fn the_export_name_follows_the_folder_name() {
        assert_eq!(artifact_name("mira_holt_2026-08-01"), "mira_holt_2026-08-01.html");
    }

    #[test]
    fn no_readings_folder_configured_is_not_a_library() {
        assert!(Library::configured(None).is_none());
        assert!(Library::configured(Some("")).is_none());
        assert!(Library::configured(Some("   ")).is_none());
        assert!(Library::configured(Some("/somewhere")).is_some());
    }

    #[test]
    fn creating_a_reading_writes_its_chart_and_the_words_it_came_from() {
        let t = TempLibrary::new("create");
        let transcript = ArchivedTranscript {
            filename: "transcript.jsonl".into(),
            contents: "{\"start\":0.0,\"text\":\"The sun.\"}".into(),
        };
        let dir = t.lib.create("mira_2026-08-01", &chart("Mira"), Some(&transcript)).unwrap();

        assert!(dir.join("chart.json").is_file());
        assert_eq!(
            std::fs::read_to_string(dir.join("transcript.jsonl")).unwrap(),
            transcript.contents
        );
        assert_eq!(dir.file_name().unwrap(), "mira_2026-08-01");
    }

    #[test]
    fn a_reading_with_no_transcript_writes_only_its_chart() {
        let t = TempLibrary::new("no-transcript");
        let dir = t.lib.create("mira_2026-08-01", &chart("Mira"), None).unwrap();
        assert!(dir.join("chart.json").is_file());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn the_listing_shows_a_reading_by_the_fields_it_displays() {
        let t = TempLibrary::new("listing");
        let mut c = chart("Mira Holt");
        c.excerpts = vec![
            astro::contract::Excerpt {
                id: "x1".into(),
                time: String::new(),
                span: [0, 0],
                text: "one".into(),
                tags: vec![],
            },
            astro::contract::Excerpt {
                id: "x2".into(),
                time: String::new(),
                span: [0, 0],
                text: "two".into(),
                tags: vec![],
            },
        ];
        t.lib.create("mira_2026-08-01", &c, None).unwrap();

        let entries = t.lib.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Mira Holt");
        assert_eq!(entries[0].born, c.meta.born);
        assert_eq!(entries[0].place, c.meta.place);
        assert_eq!(entries[0].excerpts, 2, "counted without building a single passage");
        assert!(entries[0].chart_path.ends_with("chart.json"));
    }

    #[test]
    fn the_listing_skips_folders_that_are_not_readings() {
        let t = TempLibrary::new("skips");
        t.lib.create("real_2026-08-01", &chart("Real"), None).unwrap();
        std::fs::create_dir_all(t.root.join("someone-elses-folder")).unwrap();
        std::fs::write(t.root.join("someone-elses-folder/notes.txt"), "hello").unwrap();
        std::fs::write(t.root.join("loose-file.txt"), "hello").unwrap();
        std::fs::create_dir_all(t.root.join("corrupt")).unwrap();
        std::fs::write(t.root.join("corrupt/chart.json"), "{ not json").unwrap();

        let entries = t.lib.entries();
        assert_eq!(entries.len(), 1, "only the real reading, got {}", entries.len());
        assert_eq!(entries[0].name, "Real");
    }

    #[test]
    fn an_unreadable_root_lists_empty_rather_than_failing() {
        let lib = Library::configured(Some("/no/such/place")).unwrap();
        assert!(lib.entries().is_empty());
    }

    #[test]
    fn readings_list_newest_first() {
        let t = TempLibrary::new("order");
        for stem in ["a_2026-08-01", "b_2026-08-01", "c_2026-08-01"] {
            t.lib.create(stem, &chart(stem), None).unwrap();
            // mtime resolution is coarse on some filesystems; force an order.
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let entries = t.lib.entries();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["c_2026-08-01", "b_2026-08-01", "a_2026-08-01"]);
    }

    #[test]
    fn removing_a_reading_takes_its_whole_folder() {
        let t = TempLibrary::new("remove");
        let dir = t.lib.create("mira_2026-08-01", &chart("Mira"), None).unwrap();
        std::fs::write(dir.join("take-1.jsonl"), "{}").unwrap();

        t.lib.remove(dir.to_str().unwrap()).unwrap();
        assert!(!dir.exists());
        assert!(t.lib.entries().is_empty());
    }

    /// The frontend hands back a string it was given; the library trusts the
    /// filesystem rather than the string.
    #[test]
    fn nothing_outside_the_library_can_be_removed() {
        let t = TempLibrary::new("guard");
        let inside = t.lib.create("mira_2026-08-01", &chart("Mira"), None).unwrap();

        // A sibling of the root, reached by traversal from inside it.
        let outside = t.root.parent().unwrap().join("midheaven-library-guard-victim");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("chart.json"), "{}").unwrap();
        let traversal = inside.join("..").join("..").join("midheaven-library-guard-victim");
        let err = t.lib.remove(traversal.to_str().unwrap()).unwrap_err();
        assert!(err.contains("not in the readings library"), "{err}");
        assert!(outside.exists(), "the guard must actually have stopped it");
        std::fs::remove_dir_all(&outside).ok();

        // A folder inside the root that is not a reading.
        let plain = t.root.join("just-a-folder");
        std::fs::create_dir_all(&plain).unwrap();
        let err = t.lib.remove(plain.to_str().unwrap()).unwrap_err();
        assert!(err.contains("not a saved reading"), "{err}");

        // A folder that is not there at all.
        assert!(t.lib.remove(t.root.join("ghost").to_str().unwrap()).is_err());

        // And the real one still can be.
        t.lib.remove(inside.to_str().unwrap()).unwrap();
    }

    /// A nested folder is not a direct child, so it is not a reading of this
    /// library even if it looks like one.
    #[test]
    fn only_direct_children_of_the_root_are_readings() {
        let t = TempLibrary::new("nested");
        let nested = t.root.join("outer").join("inner");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("chart.json"), "{}").unwrap();
        let err = t.lib.remove(nested.to_str().unwrap()).unwrap_err();
        assert!(err.contains("not in the readings library"), "{err}");
    }

    #[test]
    fn take_ordinals_continue_past_the_files_already_there() {
        let t = TempLibrary::new("takes");
        let dir = t.lib.create("mira_2026-08-01", &chart("Mira"), None).unwrap();
        assert_eq!(max_take_ordinal(&dir), 0);

        std::fs::write(dir.join("take-1.jsonl"), "{}").unwrap();
        std::fs::write(dir.join("take-2.jsonl"), "{}").unwrap();
        assert_eq!(max_take_ordinal(&dir), 2);

        // The highest, not the count — a deleted middle take must not cause a
        // new one to overwrite an existing file.
        std::fs::remove_file(dir.join("take-1.jsonl")).unwrap();
        assert_eq!(max_take_ordinal(&dir), 2);

        // Anything not `take-{n}.jsonl` is not a take.
        std::fs::write(dir.join("take-notes.jsonl"), "{}").unwrap();
        std::fs::write(dir.join("take-3.txt"), "{}").unwrap();
        assert_eq!(max_take_ordinal(&dir), 2);

        assert_eq!(max_take_ordinal(Path::new("/no/such/place")), 0);
    }

    #[test]
    fn a_saved_chart_reloads_as_the_same_reading() {
        let t = TempLibrary::new("roundtrip");
        let mut c = chart("Mira Holt");
        c.excerpts = vec![astro::contract::Excerpt {
            id: "x1".into(),
            time: "00:00:12".into(),
            span: [0, 8],
            text: "The sun.".into(),
            tags: vec!["planet:sun".into()],
        }];
        let dir = t.lib.create("mira_2026-08-01", &c, None).unwrap();

        let raw = std::fs::read_to_string(dir.join("chart.json")).unwrap();
        let back: ChartData = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.meta.name, c.meta.name);
        assert_eq!(back.planets.len(), c.planets.len());
        assert_eq!(back.excerpts.len(), 1);
        assert_eq!(back.excerpts[0].tags, vec!["planet:sun"]);
        assert!(back.validate().is_ok(), "and it is openable");
    }
}
