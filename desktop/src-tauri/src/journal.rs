//! The journal: notes that belong to no saved reading.
//!
//! A note written from the reading view lives in that reading's `chart.json`,
//! where `add_in` has always put it. A note written from the home page has no
//! reading folder to live in — the sky it was written over is a moment the
//! astrologer was only looking at, not a consultation anyone recorded. Those
//! notes live here.
//!
//! Rooted at a `notes/` folder beside the saved readings rather than inside any
//! of them, for three reasons: a moment-anchored note has no reading to sit in;
//! deleting a reading must not silently delete what was written about it; and
//! handing someone a reading folder must not hand them the journal.
//!
//! `notes/` is a direct child of the library root holding no `chart.json`, so
//! [`crate::library::Library`] already ignores it and already refuses to delete
//! it. That is load-bearing rather than lucky, and
//! `the_library_does_not_mistake_the_journal_for_a_reading` holds it in place.
//!
//! Like the library, the root is handed in rather than fetched from an
//! `AppHandle`, and the clock is handed in rather than read — which is what
//! makes any of this testable.

use astro::contract::{BirthSeed, Note};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The journal's folder inside the readings library.
const DIR: &str = "notes";

/// The saved reading a note is about, snapshotted at writing time.
///
/// The id is the link; the name and birth date are a copy. Keeping the copy is
/// what lets the journal render a hundred rows without opening a hundred charts,
/// and what lets a note whose reading has since been deleted still read as
/// *about Mira Holt* rather than as a dead identifier. The cost is honest: a
/// name corrected on the reading does not reach notes already written about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    /// `Meta.id` of the reading — stable across a rename of the person or a
    /// move of the whole library.
    pub id: String,
    pub name: String,
    pub born: String,
}

/// The chart a note was written over, said in a way that can be recomputed.
///
/// The chart itself is not stored. A moment-anchored note would otherwise carry
/// a full copy of every body, house and aspect — and ten notes about the same
/// transit would be ten copies of one sky with no shared identity, so "every
/// note I wrote about this moment" could not be asked. The seed plus the
/// calculation codes is the same chart, in a few dozen bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    /// What kind of chart this is. `"moment"` and `"reading"` are what exist;
    /// the field is a string rather than an enum because progressions, returns
    /// and synastry are charts this app cannot yet compute, and an unknown kind
    /// read from disk should survive being read rather than fail the file.
    pub kind: String,
    /// Place, date and time — enough to recompute the sky.
    pub seed: BirthSeed,
    /// The place as it read on screen, so a row renders without the gazetteer.
    pub place: String,
    /// Calculation codes, so a recompute lands on the geometry the note was
    /// written over rather than on today's preference.
    pub house_system: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ayanamsa: Option<String>,
    pub locale: String,
    /// The reading this note is about, when it is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reading: Option<Subject>,
}

/// One note in the journal: the note itself, when it was written, and the chart
/// it was written over.
///
/// `note` is the *same* [`Note`] a chart carries — not a parallel type that
/// would have to be converted at every boundary. A journal note is always
/// written, so its `span` is `None` and its `time` is empty; [`Journal::write`]
/// refuses anything else rather than storing a claim it cannot support.
///
/// Its `id` is the journal's, not a chart's `x{n}`: uniqueness is the contract
/// invariant and `x{n}` is only convention, so a note has one id and the file it
/// lives in is named for it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct JournalNote {
    /// When it was written, RFC 3339.
    ///
    /// A string rather than an integer because this file sits in the user's own
    /// folder and is meant to still make sense in a decade — `2026-08-04T09:12:
    /// 00+02:00` says what it is to anyone who opens it, and a millisecond count
    /// says nothing. Sorting parses it; a value that will not parse sinks rather
    /// than throwing the note away.
    pub written_at: String,
    pub anchor: Anchor,
    pub note: Note,
}

/// Ids are minted here and handed to the webview, which hands them back to edit
/// and delete. That round trip is why the shape is checked on the way in rather
/// than trusted: an id becomes a filename, and `../../something` is a filename
/// too. Restricting it to `[a-z0-9-]` forecloses traversal, separators, and
/// Windows' reserved characters in one rule.
fn safe_id(id: &str) -> Result<&str, String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-'));
    if ok { Ok(id) } else { Err(format!("malformed note id {id:?}")) }
}

/// A folder of journal notes.
pub struct Journal {
    dir: PathBuf,
}

impl Journal {
    /// The journal inside the configured readings library, or `None` when no
    /// readings folder is set.
    ///
    /// `None` is a normal state, exactly as it is for the library: nothing is
    /// saved, and the caller's job is to ask for a folder rather than to report
    /// an error. The folder itself is not created here — a journal that is only
    /// ever read should not leave a directory behind.
    pub fn configured(readings_dir: Option<&str>) -> Option<Journal> {
        let root = readings_dir.map(str::trim).filter(|r| !r.is_empty())?;
        Some(Journal { dir: PathBuf::from(root).join(DIR) })
    }

    /// A free id for a note written at `now_ms`.
    ///
    /// Time-ordered, so a directory listing reads in the order the notes were
    /// written even before anything parses `writtenAt`. Base 36 keeps it short.
    /// A second note inside the same millisecond takes the next free suffix
    /// rather than overwriting the first — the loop terminates because each
    /// probe names a different file.
    pub fn mint_id(&self, now_ms: u64) -> String {
        let stem = to_base36(now_ms);
        if !self.path(&stem).exists() {
            return stem;
        }
        (1..).map(|n| format!("{stem}-{n}")).find(|id| !self.path(id).exists()).expect("a free id")
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Write a note, creating the journal folder if this is the first one.
    ///
    /// The same call creates and amends: a note is one file, so replacing it is
    /// a write rather than a read-modify-write of a shared log. That is most of
    /// why the store is shaped this way.
    pub fn write(&self, entry: &JournalNote) -> Result<(), String> {
        safe_id(&entry.note.id)?;
        // A journal note is written, never spoken. Storing a span here would
        // put provenance on a note that has no transcript behind it, and it
        // would come back from disk claiming to be something it is not.
        if entry.note.span.is_some() {
            return Err("a journal note is written by hand and cannot carry a span".to_string());
        }
        if entry.note.text.trim().is_empty() {
            return Err("a note needs words".to_string());
        }
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("cannot create {}: {e}", self.dir.display()))?;
        let path = self.path(&entry.note.id);
        let json = serde_json::to_string_pretty(entry).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// Every note in the journal, newest first.
    ///
    /// A file that will not parse is skipped rather than reported — the journal
    /// lives in a folder the person also puts things in, and one bad file must
    /// not cost them the other two hundred. That a corrupt file loses exactly
    /// one note is the other half of why each note is its own file.
    pub fn entries(&self) -> Vec<JournalNote> {
        let mut out: Vec<JournalNote> = std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json"))
                    .filter_map(|p| std::fs::read_to_string(&p).ok())
                    .filter_map(|raw| serde_json::from_str::<JournalNote>(&raw).ok())
                    .collect()
            })
            .unwrap_or_default();
        // Newest first. An unparseable timestamp sorts as the epoch, so it sinks
        // to the end instead of taking the top or dropping the note.
        out.sort_by_key(|e| std::cmp::Reverse(written_at_ms(e)));
        out
    }

    /// Remove a note. Final, like removing a reading.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let path = self.path(safe_id(id)?);
        if !path.is_file() {
            return Err(format!("no note {id} in the journal"));
        }
        std::fs::remove_file(&path).map_err(|e| format!("cannot remove {}: {e}", path.display()))
    }
}

/// A note's written-at timestamp as milliseconds since the epoch, for ordering
/// only. Unparseable is `0`, which sorts last under `Reverse`.
fn written_at_ms(e: &JournalNote) -> i64 {
    chrono::DateTime::parse_from_rfc3339(&e.written_at)
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}

fn to_base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).expect("base36 digits are ascii")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A journal rooted at a temporary directory — the second adapter, and the
    /// same shape `library::tests::TempLibrary` uses. The path is derived from
    /// the test's own name so concurrent tests do not share one.
    struct TempJournal {
        journal: Journal,
        root: PathBuf,
    }

    impl TempJournal {
        fn new(name: &str) -> TempJournal {
            let root = std::env::temp_dir().join(format!("midheaven-journal-{name}"));
            std::fs::remove_dir_all(&root).ok();
            std::fs::create_dir_all(&root).expect("temp root");
            let journal = Journal::configured(root.to_str()).expect("a rooted journal");
            TempJournal { journal, root }
        }
    }

    impl Drop for TempJournal {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    fn anchor() -> Anchor {
        Anchor {
            kind: "moment".into(),
            seed: BirthSeed { place_id: 2950159, date: "2026-08-04".into(), time: "09:12".into() },
            place: "Berlin, Germany".into(),
            house_system: "whole-sign".into(),
            ayanamsa: None,
            locale: "en".into(),
            reading: None,
        }
    }

    fn entry(id: &str, written_at: &str, text: &str) -> JournalNote {
        JournalNote {
            written_at: written_at.into(),
            anchor: anchor(),
            note: Note {
                id: id.into(),
                time: String::new(),
                span: None,
                text: text.into(),
                tags: vec!["planet:sun".into()],
            },
        }
    }

    #[test]
    fn no_readings_folder_configured_is_not_a_journal() {
        assert!(Journal::configured(None).is_none());
        assert!(Journal::configured(Some("")).is_none());
        assert!(Journal::configured(Some("   ")).is_none());
        assert!(Journal::configured(Some("/somewhere")).is_some());
    }

    /// Reading a journal that was never written to must not leave a folder
    /// behind in the user's library.
    #[test]
    fn an_unwritten_journal_creates_nothing_and_lists_empty() {
        let t = TempJournal::new("unwritten");
        assert!(t.journal.entries().is_empty());
        assert!(!t.root.join(DIR).exists(), "reading the journal created its folder");
    }

    #[test]
    fn a_note_round_trips_through_its_own_file() {
        let t = TempJournal::new("roundtrip");
        t.journal.write(&entry("abc", "2026-08-04T09:12:00+02:00", "Mars is loud today.")).unwrap();

        assert!(t.root.join(DIR).join("abc.json").is_file(), "one note, one file, named for it");
        let back = t.journal.entries();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].note.text, "Mars is loud today.");
        assert_eq!(back[0].note.tags, vec!["planet:sun"]);
        assert_eq!(back[0].note.span, None, "a journal note is written, never spoken");
        assert_eq!(back[0].anchor.seed.date, "2026-08-04");
        assert_eq!(back[0].anchor.place, "Berlin, Germany");
    }

    /// The chart is not stored — only what recomputes it. A journal that kept
    /// whole charts would hold one copy of the same sky per note.
    #[test]
    fn a_note_stores_its_chart_as_a_seed_not_a_chart() {
        let t = TempJournal::new("seed");
        t.journal.write(&entry("abc", "2026-08-04T09:12:00+02:00", "t")).unwrap();
        let raw = std::fs::read_to_string(t.root.join(DIR).join("abc.json")).unwrap();
        // `BirthSeed` carries no `rename_all` — `place_id` is the shape
        // `meta.birth` has always had, and the journal reuses the type rather
        // than restating it.
        assert!(raw.contains("\"place_id\": 2950159"), "{raw}");
        assert!(raw.contains("\"houseSystem\": \"whole-sign\""), "{raw}");
        assert!(!raw.contains("\"planets\""), "no chart was stored: {raw}");
        assert!(raw.len() < 800, "a note is small: {} bytes", raw.len());
    }

    #[test]
    fn writing_the_same_id_amends_rather_than_duplicating() {
        let t = TempJournal::new("amend");
        t.journal.write(&entry("abc", "2026-08-04T09:12:00+02:00", "first thought")).unwrap();
        t.journal.write(&entry("abc", "2026-08-04T09:12:00+02:00", "second thought")).unwrap();

        let back = t.journal.entries();
        assert_eq!(back.len(), 1, "an edit is a write, not another note");
        assert_eq!(back[0].note.text, "second thought");
    }

    #[test]
    fn notes_list_newest_first_and_an_unreadable_date_sinks() {
        let t = TempJournal::new("order");
        t.journal.write(&entry("a", "2026-08-01T10:00:00Z", "oldest")).unwrap();
        t.journal.write(&entry("c", "2026-08-03T10:00:00Z", "newest")).unwrap();
        t.journal.write(&entry("b", "2026-08-02T10:00:00Z", "middle")).unwrap();
        t.journal.write(&entry("d", "whenever", "undated")).unwrap();

        let texts: Vec<String> = t.journal.entries().into_iter().map(|e| e.note.text).collect();
        assert_eq!(texts, vec!["newest", "middle", "oldest", "undated"]);
    }

    /// One unreadable file costs one note. That is the whole reason a note is
    /// its own file rather than a line in a shared log.
    #[test]
    fn a_corrupt_file_costs_one_note_and_not_the_journal() {
        let t = TempJournal::new("corrupt");
        t.journal.write(&entry("a", "2026-08-01T10:00:00Z", "kept")).unwrap();
        t.journal.write(&entry("b", "2026-08-02T10:00:00Z", "also kept")).unwrap();
        std::fs::write(t.root.join(DIR).join("bad.json"), "{ not json").unwrap();
        std::fs::write(t.root.join(DIR).join("notes.txt"), "not a note at all").unwrap();

        assert_eq!(t.journal.entries().len(), 2);
    }

    #[test]
    fn removing_a_note_takes_its_file_and_nothing_else() {
        let t = TempJournal::new("remove");
        t.journal.write(&entry("a", "2026-08-01T10:00:00Z", "one")).unwrap();
        t.journal.write(&entry("b", "2026-08-02T10:00:00Z", "two")).unwrap();

        t.journal.remove("a").unwrap();
        let texts: Vec<String> = t.journal.entries().into_iter().map(|e| e.note.text).collect();
        assert_eq!(texts, vec!["two"]);
        assert!(t.journal.remove("a").is_err(), "removing it twice is an error, not a silent no-op");
    }

    /// The webview hands back an id it was given. This is what makes trusting
    /// it unnecessary.
    #[test]
    fn an_id_that_is_not_an_id_cannot_name_a_file() {
        let t = TempJournal::new("guard");
        let victim = t.root.join("victim.json");
        std::fs::write(&victim, "precious").unwrap();

        for bad in ["../victim", "../../etc/passwd", "a/b", "a\\b", "a:b", "", "Abc", "a.b"] {
            assert!(t.journal.remove(bad).is_err(), "remove accepted {bad:?}");
            let mut e = entry("x", "2026-08-01T10:00:00Z", "t");
            e.note.id = bad.to_string();
            assert!(t.journal.write(&e).is_err(), "write accepted {bad:?}");
        }
        assert!(victim.exists(), "the guard must actually have stopped it");
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "precious");
    }

    #[test]
    fn a_journal_note_may_not_claim_provenance_it_does_not_have() {
        let t = TempJournal::new("span");
        let mut e = entry("a", "2026-08-01T10:00:00Z", "t");
        e.note.span = Some([0, 1]);
        let err = t.journal.write(&e).unwrap_err();
        assert!(err.contains("cannot carry a span"), "{err}");

        let mut e = entry("a", "2026-08-01T10:00:00Z", "   ");
        e.note.span = None;
        assert!(t.journal.write(&e).is_err(), "and a note still needs words");
    }

    #[test]
    fn minted_ids_are_free_time_ordered_and_safe() {
        let t = TempJournal::new("mint");
        let first = t.journal.mint_id(1_785_825_000_000);
        safe_id(&first).expect("a minted id is a safe id");

        // Nothing written yet, so the same millisecond mints the same id —
        // uniqueness comes from the file existing, not from the clock.
        assert_eq!(t.journal.mint_id(1_785_825_000_000), first);

        t.journal.write(&entry(&first, "2026-08-01T10:00:00Z", "one")).unwrap();
        let second = t.journal.mint_id(1_785_825_000_000);
        assert_ne!(second, first, "a taken id is not minted twice");
        safe_id(&second).unwrap();

        t.journal.write(&entry(&second, "2026-08-01T10:00:00Z", "two")).unwrap();
        let third = t.journal.mint_id(1_785_825_000_000);
        assert!(third != first && third != second);

        // Later moments sort after earlier ones as plain strings, which is what
        // makes a bare directory listing read in written order.
        assert!(t.journal.mint_id(1_785_825_000_001) > first);
    }

    #[test]
    fn base36_is_what_it_claims() {
        assert_eq!(to_base36(0), "0");
        assert_eq!(to_base36(35), "z");
        assert_eq!(to_base36(36), "10");
        assert_eq!(to_base36(1_785_825_000_000), "msea3gw0");
    }

    /// The journal sits inside the readings library, so the library must not
    /// mistake it for a reading — neither listing it nor letting it be deleted
    /// as one. Both hold because `notes/` has no `chart.json`; this is what
    /// keeps that true.
    #[test]
    fn the_library_does_not_mistake_the_journal_for_a_reading() {
        let t = TempJournal::new("coexist");
        let library = crate::library::Library::configured(t.root.to_str()).unwrap();
        let chart = astro::chart::compute_chart(&astro::fixtures::named("Mira")).unwrap();
        library.create("mira_2026-08-01", &chart, None).unwrap();
        t.journal.write(&entry("a", "2026-08-01T10:00:00Z", "a note")).unwrap();

        let entries = library.entries();
        assert_eq!(entries.len(), 1, "the journal is not a reading");
        assert_eq!(entries[0].name, "Mira");

        let err = library.remove(t.root.join(DIR).to_str().unwrap()).unwrap_err();
        assert!(err.contains("not a saved reading"), "{err}");
        assert_eq!(t.journal.entries().len(), 1, "and the journal survived");
    }
}
