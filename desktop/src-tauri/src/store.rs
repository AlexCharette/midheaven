//! Where everything the app holds lives: four sibling folders of one-file
//! records, under the folder the astrologer chose.
//!
//! ```text
//! <root>/
//!   notes/    <id>.json   a note, and what it was written about
//!   charts/   <id>.json   a sky, said in a way that can be recomputed
//!   people/   <id>.json   someone a chart is of
//!   views/    <id>.json   a saved question  (added by the query stage)
//! ```
//!
//! This generalizes `journal`, which was already this shape for notes alone
//! and had already paid for the parts that are easy to get wrong: one file per
//! record so a corrupt file costs one record rather than the folder, write as
//! create-and-amend so there is no read-modify-write of a shared log, and
//! every id checked before it becomes a filename because the webview hands
//! back ids it was given.
//!
//! What is actually new is identity. The journal embedded a *copy* of the
//! chart and the person in every note, and said so in its own doc comment:
//! "a name corrected on the reading does not reach notes already written about
//! it." Charts and people are records here, and a note names them. Correcting
//! someone's name is one file, and every note about them reads the new one.
//!
//! The asymmetry between the two is deliberate and worth stating, because it
//! is the thing that decides what an edit does:
//!
//! * **A person's name propagates.** It is a label on an identity, and the
//!   identity does not change when the label is corrected.
//! * **A chart's birth moment does not.** Change it and you are not looking at
//!   the same sky, so you get a new chart. Notes filed under the old one stay
//!   filed under the old one, because that is what they were written about.
//!
//! Like the library and the journal before it, the root is handed in rather
//! than fetched from an `AppHandle`, and the clock is handed in rather than
//! read — which is what makes any of this testable.
//!
//! Nothing calls this yet. It lands reviewed and tested on its own so that
//! moving the commands onto it is a rewiring rather than a rewrite.
#![allow(dead_code)]

use crate::ids;
use astro::contract::{BirthSeed, Note};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

/// Something the store keeps one file per.
///
/// `DIR` is the folder name and the only thing distinguishing the four record
/// types at the filesystem, which is why it lives on the type rather than
/// being passed at every call: a caller that could name the folder could put a
/// chart in `people/`.
pub trait Record: Serialize + DeserializeOwned {
    const DIR: &'static str;
    fn id(&self) -> &str;
}

/// Someone a chart is of.
///
/// Separate from the chart because one person has many: a natal chart, the
/// transits over it, a progression. Correcting the spelling of a name should
/// reach all of them, and it does, because they all name this record rather
/// than carrying a copy of it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub name: String,
    /// Birth date as it reads on screen. A display string, not the seed — the
    /// seed lives on the chart, because that is the thing it recomputes.
    pub born: String,
}

impl Record for Person {
    const DIR: &'static str = "people";
    fn id(&self) -> &str {
        &self.id
    }
}

/// A sky, said in a way that can be recomputed.
///
/// The computed chart is not stored. A note-anchored moment would otherwise
/// carry a full copy of every body, house and aspect, and ten notes about one
/// transit would be ten copies of one sky with no shared identity — so
/// "everything I wrote about this moment" could not be asked. The seed plus
/// the calculation codes is the same chart in a few dozen bytes, and the id
/// is what makes the question answerable.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    pub id: String,
    /// What kind of chart this is. `"moment"` and `"natal"` are what exist; a
    /// string rather than an enum because progressions, returns and synastry
    /// are charts this app cannot yet compute, and an unknown kind read from
    /// disk should survive being read rather than fail the file.
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
    /// Whose chart this is, when it is anyone's. A moment the astrologer was
    /// only looking at belongs to nobody.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub person: Option<String>,
}

impl Record for Chart {
    const DIR: &'static str = "charts";
    fn id(&self) -> &str {
        &self.id
    }
}

/// A note, and what it was written about.
///
/// Flat: the note's own fields are the record's fields rather than a nested
/// `note` object, because there is no longer anything else a note could be
/// wrapped in. `journal::JournalNote` was `{ writtenAt, anchor, note }` with
/// the anchor inline; the anchor is a `chart` id now, and the nesting went
/// with it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct StoredNote {
    pub id: String,
    /// When it was written, RFC 3339.
    ///
    /// A string rather than an integer because this file sits in the user's
    /// own folder and is meant to still make sense in a decade —
    /// `2026-08-04T09:12:00+02:00` says what it is to anyone who opens it, and
    /// a millisecond count says nothing. Sorting parses it; a value that will
    /// not parse sinks rather than throwing the note away.
    pub written_at: String,
    /// The sky it was written over.
    pub chart: String,
    /// "HH:MM:SS" into the recording it was spoken in, if it was spoken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub at: Option<String>,
    pub text: String,
    pub tags: Vec<String>,
}

impl Record for StoredNote {
    const DIR: &'static str = "notes";
    fn id(&self) -> &str {
        &self.id
    }
}

impl StoredNote {
    /// The contract note inside this record — what every renderer takes.
    pub fn note(&self) -> Note {
        Note {
            id: self.id.clone(),
            at: self.at.clone(),
            text: self.text.clone(),
            tags: self.tags.clone(),
        }
    }

    /// This note's written-at as milliseconds since the epoch, for ordering
    /// only. Unparseable is `0`, which sorts last under `Reverse`.
    fn written_at_ms(&self) -> i64 {
        chrono::DateTime::parse_from_rfc3339(&self.written_at)
            .map(|d| d.timestamp_millis())
            .unwrap_or(0)
    }
}

/// A note's id, checked rather than trusted — the webview hands back ids it
/// was given, and here an id becomes a filename.
fn safe_id(id: &str) -> Result<&str, String> {
    if ids::is_safe(id) { Ok(id) } else { Err(format!("malformed id {id:?}")) }
}

/// Four folders under one root.
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// The store inside the configured folder, or `None` when none is set.
    ///
    /// `None` is a normal state: nothing is saved, and the caller's job is to
    /// ask for a folder rather than to report an error. No directory is
    /// created here — a store that is only ever read should not leave empty
    /// folders behind.
    pub fn configured(dir: Option<&str>) -> Option<Store> {
        let root = dir.map(str::trim).filter(|r| !r.is_empty())?;
        Some(Store { root: PathBuf::from(root) })
    }

    fn dir<T: Record>(&self) -> PathBuf {
        self.root.join(T::DIR)
    }

    fn path<T: Record>(&self, id: &str) -> PathBuf {
        self.dir::<T>().join(format!("{id}.json"))
    }

    /// A free id for a record made at `now_ms`. What counts as taken is a file
    /// already being there, so two records inside one millisecond cannot
    /// collide.
    pub fn mint_id<T: Record>(&self, now_ms: u64) -> String {
        ids::mint(now_ms, |id| self.path::<T>(id).exists())
    }

    /// Write a record, creating its folder if this is the first one.
    ///
    /// The same call creates and amends: a record is one file, so replacing it
    /// is a write rather than a read-modify-write of a shared log. That is
    /// most of why the store is shaped this way.
    pub fn put<T: Record>(&self, rec: &T) -> Result<(), String> {
        safe_id(rec.id())?;
        let dir = self.dir::<T>();
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let path = self.path::<T>(rec.id());
        let json = serde_json::to_string_pretty(rec).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// One record by id, or `None` if it is not there or will not parse.
    pub fn get<T: Record>(&self, id: &str) -> Result<Option<T>, String> {
        let path = self.path::<T>(safe_id(id)?);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Ok(None);
        };
        Ok(serde_json::from_str(&raw).ok())
    }

    /// Every record in a folder, in id order — which is written order, because
    /// an id is base-36 of the millisecond it was minted.
    ///
    /// A file that will not parse is skipped rather than reported. The store
    /// lives in a folder the person also puts things in, and one bad file must
    /// not cost them the other two hundred. That a corrupt file loses exactly
    /// one record is the other half of why each record is its own file.
    pub fn all<T: Record>(&self) -> Vec<T> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(self.dir::<T>())
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json"))
                    .collect()
            })
            .unwrap_or_default();
        paths.sort();
        paths
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .filter_map(|raw| serde_json::from_str(&raw).ok())
            .collect()
    }

    /// Remove a record. Final, like removing a reading was.
    pub fn remove<T: Record>(&self, id: &str) -> Result<(), String> {
        let path = self.path::<T>(safe_id(id)?);
        if !path.is_file() {
            return Err(format!("no {} {id}", T::DIR));
        }
        std::fs::remove_file(&path).map_err(|e| format!("cannot remove {}: {e}", path.display()))
    }

    /// Every note, newest first.
    ///
    /// The one ordering the store states itself, because it is the one the
    /// journal already got right: an unparseable timestamp sorts as the epoch,
    /// so it sinks to the end instead of taking the top or dropping the note.
    pub fn notes_newest_first(&self) -> Vec<StoredNote> {
        let mut out = self.all::<StoredNote>();
        out.sort_by_key(|n| std::cmp::Reverse(n.written_at_ms()));
        out
    }

    /// Whether this root holds anything at all — the question "is there
    /// something here to open".
    pub fn is_empty(&self) -> bool {
        [StoredNote::DIR, Chart::DIR, Person::DIR]
            .iter()
            .all(|d| !dir_has_json(&self.root.join(d)))
    }
}

fn dir_has_json(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut rd| {
        rd.any(|e| {
            e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "json"))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store rooted at a temporary directory, named for the test that owns
    /// it so concurrent tests do not share one.
    struct TempStore {
        store: Store,
        root: PathBuf,
    }

    impl TempStore {
        fn new(name: &str) -> TempStore {
            let root = std::env::temp_dir().join(format!("midheaven-store-{name}"));
            std::fs::remove_dir_all(&root).ok();
            std::fs::create_dir_all(&root).expect("temp root");
            let store = Store::configured(root.to_str()).expect("a rooted store");
            TempStore { store, root }
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    fn person(id: &str, name: &str) -> Person {
        Person { id: id.into(), name: name.into(), born: "1990-07-13".into() }
    }

    fn chart(id: &str, person: Option<&str>) -> Chart {
        Chart {
            id: id.into(),
            kind: "natal".into(),
            seed: BirthSeed {
                place_id: 2_950_159,
                date: "1990-07-13".into(),
                time: "14:30".into(),
            },
            place: "Berlin, Germany".into(),
            house_system: "whole-sign".into(),
            ayanamsa: None,
            locale: "en".into(),
            person: person.map(str::to_string),
        }
    }

    fn note(id: &str, written_at: &str, chart: &str, text: &str) -> StoredNote {
        StoredNote {
            id: id.into(),
            written_at: written_at.into(),
            chart: chart.into(),
            at: None,
            text: text.into(),
            tags: vec!["planet:sun".into()],
        }
    }

    #[test]
    fn all_four_record_types_round_trip() {
        let t = TempStore::new("round-trip");
        let p = person("p1", "Mira Holt");
        let c = chart("c1", Some("p1"));
        let n = note("n1", "2026-08-04T09:12:00+02:00", "c1", "The sun is loud.");

        t.store.put(&p).unwrap();
        t.store.put(&c).unwrap();
        t.store.put(&n).unwrap();

        assert_eq!(t.store.get::<Person>("p1").unwrap(), Some(p));
        assert_eq!(t.store.get::<Chart>("c1").unwrap(), Some(c));
        assert_eq!(t.store.get::<StoredNote>("n1").unwrap(), Some(n));
    }

    /// The four folders are four namespaces: a chart and a note may share an
    /// id without either shadowing the other.
    #[test]
    fn the_record_type_picks_the_folder() {
        let t = TempStore::new("folders");
        t.store.put(&chart("same", None)).unwrap();
        t.store.put(&note("same", "2026-08-04T09:12:00Z", "same", "words")).unwrap();

        assert!(t.root.join("charts/same.json").is_file());
        assert!(t.root.join("notes/same.json").is_file());
        assert_eq!(t.store.get::<Person>("same").unwrap(), None);
    }

    /// One call creates and amends, because a record is one file.
    #[test]
    fn writing_the_same_id_twice_amends_rather_than_duplicates() {
        let t = TempStore::new("amend");
        t.store.put(&person("p1", "Mira Holt")).unwrap();
        t.store.put(&person("p1", "Mira Holte")).unwrap();
        let all = t.store.all::<Person>();
        assert_eq!(all.len(), 1, "one id is one file");
        assert_eq!(all[0].name, "Mira Holte");
    }

    /// Correcting a name is one write, and every chart of that person reads
    /// the corrected one because none of them copied it.
    #[test]
    fn a_corrected_name_reaches_every_chart_of_that_person() {
        let t = TempStore::new("propagate");
        t.store.put(&person("p1", "Mira Hlot")).unwrap();
        t.store.put(&chart("c1", Some("p1"))).unwrap();
        t.store.put(&chart("c2", Some("p1"))).unwrap();

        t.store.put(&person("p1", "Mira Holt")).unwrap();

        for c in t.store.all::<Chart>() {
            let p = t.store.get::<Person>(c.person.as_ref().unwrap()).unwrap().unwrap();
            assert_eq!(p.name, "Mira Holt");
        }
    }

    /// The other half of the asymmetry. Nothing here enforces it — it is a
    /// rule about what callers do — but the shape has to allow it: two charts
    /// of one person, differing in the moment, each keeping its own notes.
    #[test]
    fn a_different_birth_moment_is_a_different_chart() {
        let t = TempStore::new("asymmetry");
        t.store.put(&person("p1", "Mira Holt")).unwrap();
        let mut corrected = chart("c2", Some("p1"));
        corrected.seed.time = "14:45".into();
        t.store.put(&chart("c1", Some("p1"))).unwrap();
        t.store.put(&corrected).unwrap();
        t.store.put(&note("n1", "2026-08-04T09:00:00Z", "c1", "under the first")).unwrap();
        t.store.put(&note("n2", "2026-08-04T09:01:00Z", "c2", "under the second")).unwrap();

        let charts = t.store.all::<Chart>();
        assert_eq!(charts.len(), 2, "the correction minted a chart, it did not edit one");
        let under_c1: Vec<_> =
            t.store.all::<StoredNote>().into_iter().filter(|n| n.chart == "c1").collect();
        assert_eq!(under_c1.len(), 1, "notes stay with the sky they were written about");
    }

    /// One bad file costs one record. This is why each record is its own file,
    /// and it is the behaviour the journal paid for first.
    #[test]
    fn a_corrupt_file_costs_one_record_not_the_folder() {
        let t = TempStore::new("corrupt");
        for (i, id) in ["n1", "n2", "n3"].iter().enumerate() {
            t.store.put(&note(id, &format!("2026-08-0{}T09:00:00Z", i + 1), "c1", "words")).unwrap();
        }
        std::fs::write(t.root.join("notes/n2.json"), "{ not json").unwrap();
        std::fs::write(t.root.join("notes/notes.txt"), "not even a record").unwrap();

        let all = t.store.all::<StoredNote>();
        assert_eq!(all.len(), 2, "the other two survive: {all:?}");
    }

    /// Newest first, and a timestamp that will not parse sinks rather than
    /// taking the top or dropping the note.
    #[test]
    fn notes_come_back_newest_first_with_unparseable_dates_sinking() {
        let t = TempStore::new("order");
        t.store.put(&note("n1", "2026-08-01T09:00:00Z", "c1", "oldest")).unwrap();
        t.store.put(&note("n2", "2026-08-09T09:00:00Z", "c1", "newest")).unwrap();
        t.store.put(&note("n3", "the fourth", "c1", "undated")).unwrap();

        let notes = t.store.notes_newest_first();
        let texts: Vec<&str> = notes.iter().map(|n| n.text.as_str()).collect();
        assert_eq!(texts, vec!["newest", "oldest", "undated"]);
    }

    /// An id the webview hands back becomes a filename, and `../../something`
    /// is a filename too.
    #[test]
    fn an_id_that_could_name_a_file_elsewhere_is_refused() {
        let t = TempStore::new("traversal");
        let victim = t.root.join("victim");
        std::fs::write(&victim, "precious").unwrap();

        for bad in ["../victim", "../../etc/passwd", "a/b", "", "Abc"] {
            assert!(t.store.get::<StoredNote>(bad).is_err(), "get {bad:?} passed");
            assert!(t.store.remove::<StoredNote>(bad).is_err(), "remove {bad:?} passed");
            assert!(
                t.store.put(&note(bad, "2026-08-01T09:00:00Z", "c1", "x")).is_err(),
                "put {bad:?} passed"
            );
        }
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "precious");
    }

    #[test]
    fn removing_is_final_and_says_so_when_there_was_nothing_there() {
        let t = TempStore::new("remove");
        t.store.put(&note("n1", "2026-08-01T09:00:00Z", "c1", "words")).unwrap();
        assert!(t.store.remove::<StoredNote>("n1").is_ok());
        assert!(t.store.all::<StoredNote>().is_empty());
        assert!(t.store.remove::<StoredNote>("n1").is_err(), "twice is an error, not a no-op");
    }

    /// Reading a store that was never written leaves no folders behind.
    #[test]
    fn reading_an_empty_store_creates_nothing() {
        let t = TempStore::new("empty");
        assert!(t.store.all::<StoredNote>().is_empty());
        assert!(t.store.get::<Chart>("c1").unwrap().is_none());
        assert!(t.store.is_empty());
        assert!(!t.root.join("notes").exists(), "a read must not create the folder");
    }

    #[test]
    fn no_configured_folder_is_no_store() {
        assert!(Store::configured(None).is_none());
        assert!(Store::configured(Some("")).is_none());
        assert!(Store::configured(Some("   ")).is_none());
    }

    /// Ids are minted against what is on disk, so a burst inside one
    /// millisecond cannot collide — and the folder is per type, so a note and
    /// a chart minted in the same millisecond each get the clean stem.
    #[test]
    fn minted_ids_are_free_within_their_own_folder() {
        let t = TempStore::new("mint");
        let first = t.store.mint_id::<StoredNote>(1_785_825_000_000);
        t.store.put(&note(&first, "2026-08-01T09:00:00Z", "c1", "words")).unwrap();
        let second = t.store.mint_id::<StoredNote>(1_785_825_000_000);
        assert_ne!(first, second, "the second must dodge the file the first left");

        let chart_id = t.store.mint_id::<Chart>(1_785_825_000_000);
        assert_eq!(chart_id, first, "a different folder is a different namespace");
    }

    /// The record the renderers take is the note inside, with the same id the
    /// file is named for.
    #[test]
    fn a_stored_note_yields_the_contract_note_renderers_take() {
        let stored = StoredNote {
            at: Some("00:04:12".into()),
            ..note("n1", "2026-08-01T09:00:00Z", "c1", "The sun is loud.")
        };
        let note = stored.note();
        assert_eq!(note.id, "n1");
        assert_eq!(note.text, "The sun is loud.");
        assert_eq!(note.at.as_deref(), Some("00:04:12"));
        assert_eq!(note.tags, vec!["planet:sun"]);
    }
}
