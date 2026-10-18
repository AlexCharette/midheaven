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
use std::collections::BTreeSet;
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
    /// The reading's own language (`en`, `ru`). Not shown in the library list,
    /// but a note filed under this chart reads and searches in it wherever it
    /// appears — including in the journal, which holds no chart to ask.
    pub locale: String,
    /// How many notes are filed under the chart. Named for the wire, which
    /// still spells a note `excerpt` — see `contract::ChartData::notes`.
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
/// single `Note`, and the rest of the chart is never looked at.
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
    /// Charts saved before `meta.locale` existed carry none; the contract reads
    /// those as English, and so does this.
    #[serde(default = "english")]
    locale: String,
}

fn english() -> String {
    "en".to_string()
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
    /// Saving is also where a reading gets its identity: an id is minted here
    /// and nowhere else, because *saved* is exactly when a reading becomes
    /// something another file can point at. A chart that is only previewed
    /// never acquires one it does not need.
    pub fn create(
        &self,
        stem: &str,
        chart: &mut ChartData,
        transcript: Option<&ArchivedTranscript>,
        now_ms: u64,
    ) -> Result<PathBuf, String> {
        let dir = self.root.join(stem);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        if let Some(t) = transcript {
            std::fs::write(dir.join(&t.filename), &t.contents)
                .map_err(|e| format!("cannot write {}: {e}", t.filename))?;
        }
        if chart.meta.id.is_none() {
            let taken = self.taken_ids();
            chart.meta.id = Some(crate::ids::mint(now_ms, |id| taken.contains(id)));
        }
        save_chart(&dir, chart)?;
        Ok(dir)
    }

    /// Every reading id already in the library.
    ///
    /// Read rather than assumed: an id is minted from a millisecond, and two
    /// readings saved inside one are unlikely rather than impossible. Asking
    /// costs one pass at the moment a reading is created, which is a thing a
    /// person does by hand.
    pub fn taken_ids(&self) -> BTreeSet<String> {
        #[derive(serde::Deserialize)]
        struct Identified {
            meta: IdOnly,
        }
        #[derive(serde::Deserialize)]
        struct IdOnly {
            #[serde(default)]
            id: Option<String>,
        }
        std::fs::read_dir(&self.root)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .filter_map(|p| std::fs::read_to_string(p.join(CHART)).ok())
                    .filter_map(|raw| serde_json::from_str::<Identified>(&raw).ok())
                    .filter_map(|c| c.meta.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Give every saved reading that lacks one an id, so notes can point at it.
    ///
    /// Readings saved before ids existed have none, and they are exactly the
    /// ones an existing user has. Returns how many were stamped.
    ///
    /// Idempotent by construction — a chart that already has an id is not
    /// touched, so running this twice is running it once. Interrupt-safe for
    /// the same reason: each reading is a separate file and a separate write,
    /// and a run that stops halfway leaves the ones it reached finished.
    ///
    /// A chart that will not parse, will not validate, or will not be written
    /// is skipped rather than reported. A locked or read-only library should
    /// cost a person nothing at startup, and a file this does not fully
    /// understand is not one to rewrite.
    pub fn stamp_ids(&self, now_ms: u64) -> usize {
        let mut taken = self.taken_ids();
        let mut stamped = 0;
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return 0;
        };
        for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            let Ok(raw) = std::fs::read_to_string(dir.join(CHART)) else { continue };
            let Ok(mut chart) = serde_json::from_str::<ChartData>(&raw) else { continue };
            if chart.meta.id.is_some() {
                continue;
            }
            // Derived fields before writing: a chart saved before they existed
            // deserializes with them empty, and writing that back would put an
            // empty `houseSweeps` where the file had none at all — a worse
            // state than the one we found. Filling also gives `validate`
            // something real to check.
            astro::derive::fill(&mut chart);
            if chart.validate().is_err() {
                continue;
            }
            let id = crate::ids::mint(now_ms + stamped as u64, |c| taken.contains(c));
            chart.meta.id = Some(id.clone());
            if save_chart(&dir, &chart).is_ok() {
                taken.insert(id);
                stamped += 1;
            }
        }
        stamped
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

    /// Every saved reading paired with the notes filed under it, newest first.
    ///
    /// Deliberately a second reader rather than a richer [`entries`](Self::entries):
    /// that one exists to be cheap, and its `IgnoredAny` was written precisely so
    /// listing the library never builds a single note. The journal's cross-reading
    /// view needs the opposite — every note in every chart — so it says so, and
    /// pays for it here where the cost is named.
    ///
    /// The chart is read once for both halves; going through `entries` first and
    /// then re-opening each file would read every chart twice.
    pub fn notes(&self) -> Vec<(ReadingEntry, Vec<astro::contract::Note>)> {
        let mut out: Vec<(ReadingEntry, Vec<astro::contract::Note>)> = std::fs::read_dir(&self.root)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .filter_map(|p| noted_entry(&p))
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by_key(|(e, _)| std::cmp::Reverse(e.modified_ms));
        out
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
    let raw = std::fs::read_to_string(dir.join(CHART)).ok()?;
    row(dir, &raw)
}

/// The listing row for a chart already in hand — so a caller that needs the
/// chart's contents too pays for one read rather than two.
fn row(dir: &Path, raw: &str) -> Option<ReadingEntry> {
    let chart_path = dir.join(CHART);
    let listing: Listing = serde_json::from_str(raw).ok()?;
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
        locale: listing.meta.locale,
        excerpts: listing.excerpts.len(),
        modified_ms,
    })
}

/// The same row, plus the notes themselves — the expensive half of [`entry`],
/// kept apart from it so the cheap read stays cheap.
fn noted_entry(dir: &Path) -> Option<(ReadingEntry, Vec<astro::contract::Note>)> {
    /// The listing fields, with the notes actually built this time.
    #[derive(serde::Deserialize)]
    struct Noted {
        #[serde(default)]
        excerpts: Vec<astro::contract::Note>,
    }
    let raw = std::fs::read_to_string(dir.join(CHART)).ok()?;
    // Two passes over one string rather than two reads of one file: the row and
    // the notes are different views of the same JSON.
    let row = row(dir, &raw)?;
    let noted: Noted = serde_json::from_str(&raw).ok()?;
    Some((row, noted.excerpts))
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
        astro::chart::compute_chart(&astro::fixtures::named(name)).unwrap()
    }

    /// A fixed clock. Every test below wants a saved reading; none of them
    /// cares what millisecond it was saved in.
    const CLOCK: u64 = 1_785_825_000_000;

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
        let dir = t.lib.create("mira_2026-08-01", &mut chart("Mira"), Some(&transcript), CLOCK).unwrap();

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
        let dir = t.lib.create("mira_2026-08-01", &mut chart("Mira"), None, CLOCK).unwrap();
        assert!(dir.join("chart.json").is_file());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn the_listing_shows_a_reading_by_the_fields_it_displays() {
        let t = TempLibrary::new("listing");
        let mut c = chart("Mira Holt");
        c.notes = vec![
            astro::contract::Note {
                id: "x1".into(),
                at: None,
                text: "one".into(),
                tags: vec![],
            },
            astro::contract::Note {
                id: "x2".into(),
                at: None,
                text: "two".into(),
                tags: vec![],
            },
        ];
        t.lib.create("mira_2026-08-01", &mut c, None, CLOCK).unwrap();

        let entries = t.lib.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Mira Holt");
        assert_eq!(entries[0].born, c.meta.born);
        assert_eq!(entries[0].place, c.meta.place);
        assert_eq!(entries[0].excerpts, 2, "counted without building a single note");
        assert!(entries[0].chart_path.ends_with("chart.json"));
    }

    #[test]
    fn the_listing_skips_folders_that_are_not_readings() {
        let t = TempLibrary::new("skips");
        t.lib.create("real_2026-08-01", &mut chart("Real"), None, CLOCK).unwrap();
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
            t.lib.create(stem, &mut chart(stem), None, CLOCK).unwrap();
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
        let dir = t.lib.create("mira_2026-08-01", &mut chart("Mira"), None, CLOCK).unwrap();
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
        let inside = t.lib.create("mira_2026-08-01", &mut chart("Mira"), None, CLOCK).unwrap();

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
        let dir = t.lib.create("mira_2026-08-01", &mut chart("Mira"), None, CLOCK).unwrap();
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

    /// A reading becomes something another file can point at at exactly the
    /// moment it is saved — which is why `create` is where the id is minted.
    #[test]
    fn saving_a_reading_gives_it_an_identity() {
        let t = TempLibrary::new("mint");
        let mut c = chart("Mira");
        assert_eq!(c.meta.id, None, "a computed chart has none");

        t.lib.create("mira_2026-08-01", &mut c, None, CLOCK).unwrap();
        let id = c.meta.id.clone().expect("saving minted one");
        assert!(crate::ids::is_safe(&id), "{id}");
        assert_eq!(t.lib.taken_ids(), BTreeSet::from([id.clone()]), "and it reached the file");

        // The caller's copy is stamped too, not only the one on disk — the
        // session and the webview hold that copy, and a note points at its id.
        let reloaded: ChartData =
            serde_json::from_str(&std::fs::read_to_string(t.root.join("mira_2026-08-01/chart.json")).unwrap())
                .unwrap();
        assert_eq!(reloaded.meta.id.as_deref(), Some(id.as_str()));
    }

    #[test]
    fn a_reading_that_already_has_an_identity_keeps_it() {
        let t = TempLibrary::new("keep-id");
        let mut c = chart("Mira");
        c.meta.id = Some("already-mine".into());
        t.lib.create("mira_2026-08-01", &mut c, None, CLOCK).unwrap();
        assert_eq!(c.meta.id.as_deref(), Some("already-mine"));
    }

    /// Two readings saved inside one millisecond is unlikely, not impossible,
    /// and an id two notes could both mean is worse than an ugly one.
    #[test]
    fn two_readings_saved_in_the_same_millisecond_get_different_ids() {
        let t = TempLibrary::new("collide");
        let (mut a, mut b) = (chart("A"), chart("B"));
        t.lib.create("a_2026-08-01", &mut a, None, CLOCK).unwrap();
        t.lib.create("b_2026-08-01", &mut b, None, CLOCK).unwrap();
        assert_ne!(a.meta.id, b.meta.id);
        assert_eq!(t.lib.taken_ids().len(), 2);
    }

    /// Readings saved before ids existed are the ones an existing user has, so
    /// the migration is the whole point rather than a tidy-up.
    #[test]
    fn the_sweep_gives_older_readings_an_identity() {
        let t = TempLibrary::new("sweep");
        for stem in ["a_2026-08-01", "b_2026-08-01", "c_2026-08-01"] {
            let mut c = chart(stem);
            t.lib.create(stem, &mut c, None, CLOCK).unwrap();
            // Put them back the way a build from before ids would have left them.
            c.meta.id = None;
            save_chart(&t.root.join(stem), &c).unwrap();
        }
        assert!(t.lib.taken_ids().is_empty(), "the fixture really is id-less");

        assert_eq!(t.lib.stamp_ids(CLOCK), 3);
        let ids = t.lib.taken_ids();
        assert_eq!(ids.len(), 3, "and each got a distinct one: {ids:?}");
        assert!(ids.iter().all(|id| crate::ids::is_safe(id)));
    }

    /// Running it twice is running it once — which is what makes an interrupted
    /// run safe to simply repeat.
    #[test]
    fn the_sweep_is_idempotent() {
        let t = TempLibrary::new("sweep-twice");
        let mut c = chart("Mira");
        t.lib.create("mira_2026-08-01", &mut c, None, CLOCK).unwrap();
        c.meta.id = None;
        save_chart(&t.root.join("mira_2026-08-01"), &c).unwrap();

        assert_eq!(t.lib.stamp_ids(CLOCK), 1);
        let first = t.lib.taken_ids();
        assert_eq!(t.lib.stamp_ids(CLOCK), 0, "nothing left to do");
        assert_eq!(t.lib.taken_ids(), first, "and nothing was re-minted");
    }

    /// The sweep rewrites files a person owns. Anything it does not fully
    /// understand it leaves exactly as it found it.
    #[test]
    fn the_sweep_does_not_touch_what_it_cannot_read() {
        let t = TempLibrary::new("sweep-skips");
        let mut good = chart("Good");
        t.lib.create("good_2026-08-01", &mut good, None, CLOCK).unwrap();
        good.meta.id = None;
        save_chart(&t.root.join("good_2026-08-01"), &good).unwrap();

        std::fs::create_dir_all(t.root.join("corrupt")).unwrap();
        std::fs::write(t.root.join("corrupt/chart.json"), "{ not json").unwrap();
        std::fs::create_dir_all(t.root.join("not-a-reading")).unwrap();
        std::fs::write(t.root.join("not-a-reading/notes.txt"), "hello").unwrap();
        // Parses as a chart but could never have been produced by one — the
        // sweep must not launder it into the library by rewriting it.
        let mut broken = chart("Broken");
        broken.signs.pop();
        broken.meta.id = None;
        std::fs::create_dir_all(t.root.join("broken")).unwrap();
        save_chart(&t.root.join("broken"), &broken).unwrap();

        assert_eq!(t.lib.stamp_ids(CLOCK), 1, "only the one it understood");
        assert_eq!(std::fs::read_to_string(t.root.join("corrupt/chart.json")).unwrap(), "{ not json");
        let after: ChartData =
            serde_json::from_str(&std::fs::read_to_string(t.root.join("broken/chart.json")).unwrap())
                .unwrap();
        assert_eq!(after.meta.id, None, "the invalid chart was left alone");
    }

    /// A chart saved before `houseSweeps` existed deserializes with it empty.
    /// Writing that back would leave `"houseSweeps": []` where the file had no
    /// such key — a worse state than the one the sweep found.
    #[test]
    fn the_sweep_writes_a_coherent_chart_not_a_hollowed_one() {
        let t = TempLibrary::new("sweep-derive");
        let mut c = chart("Mira");
        t.lib.create("mira_2026-08-01", &mut c, None, CLOCK).unwrap();
        c.meta.id = None;
        c.house_sweeps = Vec::new();
        save_chart(&t.root.join("mira_2026-08-01"), &c).unwrap();

        assert_eq!(t.lib.stamp_ids(CLOCK), 1);
        let after: ChartData =
            serde_json::from_str(&std::fs::read_to_string(t.root.join("mira_2026-08-01/chart.json")).unwrap())
                .unwrap();
        assert_eq!(after.house_sweeps.len(), 12, "derived fields were filled, not blanked");
        assert!(after.validate().is_ok());
    }

    #[test]
    fn a_saved_chart_reloads_as_the_same_reading() {
        let t = TempLibrary::new("roundtrip");
        let mut c = chart("Mira Holt");
        c.notes = vec![astro::contract::Note {
            id: "x1".into(),
            at: Some("00:00:12".into()),
            text: "The sun.".into(),
            tags: vec!["planet:sun".into()],
        }];
        let dir = t.lib.create("mira_2026-08-01", &mut c, None, CLOCK).unwrap();

        let raw = std::fs::read_to_string(dir.join("chart.json")).unwrap();
        let back: ChartData = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.meta.name, c.meta.name);
        assert_eq!(back.planets.len(), c.planets.len());
        assert_eq!(back.notes.len(), 1);
        assert_eq!(back.notes[0].tags, vec!["planet:sun"]);
        assert!(back.validate().is_ok(), "and it is openable");
    }
}
