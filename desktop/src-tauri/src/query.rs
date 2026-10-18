//! What you can ask of the notes, and how to save the asking.
//!
//! A closed set of questions rather than a search string with a syntax. Six
//! dimensions — tags, who it is about, which chart, what kind of thing it is
//! anchored to, when it was written, and the words — because those are what
//! is true of every note, and a question outside them would be one the store
//! cannot answer without reading something it does not hold.
//!
//! Closed is the point. Each field narrows, they combine with AND, and an
//! empty [`Query`] matches everything; so a view is always a refinement of
//! "all my notes" and there is no way to write one that means nothing. The
//! tag predicate is `focus.ts`'s `matching` lifted off a single chart — the
//! app already had this rule, stated once for the reading view.
//!
//! **There is no index.** The plan this came from called for an in-memory one
//! built from the note folder, and it is deliberately not here: the store's
//! read is already the only source of truth, and a cache beside it would add
//! a synchronisation problem to buy a speed-up nobody has measured. When the
//! read does become slow, the thing to add is a cache that can be thrown away
//! and rebuilt from the files — never one that can be the only place a fact
//! lives, because then a corrupt index is data loss rather than a rebuild.

use crate::store::{Anchor, AnchorKind, Chart, Person, Store, StoredNote};
use serde::{Deserialize, Serialize};

/// How a multi-tag selection narrows: `any` = the note touches one of the
/// tags, `all` = it touches every one. The same two the reading view offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Any,
    All,
}

/// A question about notes. Every field narrows; an empty query is every note.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase", default)]
pub struct Query {
    /// Vocabulary tags the note must carry.
    pub tags: Vec<String>,
    pub mode: Mode,
    /// Who it is about — **both** senses, deliberately.
    ///
    /// A note reaches a person two ways now: by being anchored to one
    /// outright, or by being said in a reading of one of their charts.
    /// "Everything about Mira" is the question people actually ask, and
    /// splitting it would make the common case take two queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub person: Option<String>,
    /// What *kind* of thing it is anchored to, without saying which one.
    ///
    /// This is how "my standing notes on Leo" is asked, and it cannot be
    /// inferred from the fields above: leaving `person` and `chart` unset
    /// already means "do not filter by those", not "has neither".
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub anchor: Option<AnchorKind>,
    /// Which chart exactly — one sky rather than one person's several.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub chart: Option<String>,
    /// Written on or after this date, `YYYY-MM-DD`. Compared as text against
    /// the RFC 3339 `writtenAt`, which sorts correctly for exactly that
    /// reason — and a note whose timestamp will not parse is not silently
    /// included in every range.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub since: Option<String>,
    /// Written before the end of this date, `YYYY-MM-DD`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub until: Option<String>,
    /// Words in the note itself, case-insensitively.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub text: Option<String>,
}

/// A saved question, by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "generated/"))]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub id: String,
    /// What the astrologer called it.
    pub name: String,
    pub query: Query,
}

impl crate::store::Record for View {
    const DIR: &'static str = "views";
    fn id(&self) -> &str {
        &self.id
    }
}

/// Which chart a note is about, for the dimensions that are properties of the
/// chart rather than of the note.
///
/// Handed in rather than looked up per note: answering "whose is this" for
/// four hundred notes would otherwise be four hundred reads of the same
/// handful of files.
pub struct Charts(std::collections::HashMap<String, Chart>);

impl Charts {
    pub fn of(store: &Store) -> Charts {
        Charts(store.all::<Chart>().into_iter().map(|c| (c.id.clone(), c)).collect())
    }

    /// Who a note is about, either way round: the person it names outright,
    /// or the person whose chart it was said in.
    fn person_of<'a>(&'a self, note: &'a StoredNote) -> Option<&'a str> {
        match &note.anchor {
            Anchor::Person { id } => Some(id),
            Anchor::Chart { id } => self.0.get(id)?.person.as_deref(),
            Anchor::Standing => None,
        }
    }
}

impl Query {
    /// Whether this note answers the question.
    pub fn matches(&self, note: &StoredNote, charts: &Charts) -> bool {
        self.tags_match(note)
            && self.person.as_ref().is_none_or(|p| charts.person_of(note) == Some(p.as_str()))
            && self.chart.as_ref().is_none_or(|c| note.anchor.chart() == Some(c.as_str()))
            && self.anchor.is_none_or(|k| note.anchor.kind() == k)
            && self.since.as_ref().is_none_or(|d| note.written_at.as_str() >= d.as_str())
            // `until` is inclusive of the whole day, and an RFC 3339 stamp
            // from that day begins with it — so the comparison is against the
            // day after, spelled as "anything that still starts with it".
            && self.until.as_ref().is_none_or(|d| {
                note.written_at.as_str() <= d.as_str() || note.written_at.starts_with(d.as_str())
            })
            && self.text.as_ref().is_none_or(|q| {
                note.text.to_lowercase().contains(q.trim().to_lowercase().as_str())
            })
    }

    fn tags_match(&self, note: &StoredNote) -> bool {
        if self.tags.is_empty() {
            return true;
        }
        let has = |t: &String| note.tags.contains(t);
        match self.mode {
            Mode::Any => self.tags.iter().any(has),
            Mode::All => self.tags.iter().all(has),
        }
    }
}

/// Run a query over the store, newest first.
pub fn run(store: &Store, q: &Query) -> Vec<StoredNote> {
    let charts = Charts::of(store);
    store.notes_newest_first().into_iter().filter(|n| q.matches(n, &charts)).collect()
}

/// Every person who has a chart, for the people filter.
pub fn people(store: &Store) -> Vec<Person> {
    let mut out = store.all::<Person>();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Record;
    use astro::contract::BirthSeed;

    struct TempStore {
        store: Store,
        root: std::path::PathBuf,
    }

    impl TempStore {
        fn new(name: &str) -> TempStore {
            let root = std::env::temp_dir().join(format!("midheaven-query-{name}"));
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

    fn note(id: &str, chart: &str, written_at: &str, text: &str, tags: &[&str]) -> StoredNote {
        StoredNote {
            id: id.into(),
            written_at: written_at.into(),
            anchor: Anchor::Chart { id: chart.into() },
            at: None,
            text: text.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Two people, three skies, five notes — enough for every dimension to
    /// have something it excludes.
    fn seeded(name: &str) -> TempStore {
        let t = TempStore::new(name);
        t.store
            .put(&Person { id: "p1".into(), name: "Mira Holt".into(), born: "1990-07-13".into() })
            .unwrap();
        t.store
            .put(&Person { id: "p2".into(), name: "Ada Vance".into(), born: "1988-02-02".into() })
            .unwrap();
        t.store.put(&chart("c1", Some("p1"))).unwrap();
        t.store.put(&chart("c2", Some("p1"))).unwrap();
        t.store.put(&chart("c3", Some("p2"))).unwrap();
        // a sky nobody is consulting over
        t.store.put(&chart("c4", None)).unwrap();

        for n in [
            note("n1", "c1", "2026-08-01T09:00:00Z", "The sun is loud.", &["planet:sun"]),
            note("n2", "c1", "2026-08-05T09:00:00Z", "A quiet moon.", &["planet:moon"]),
            note("n3", "c2", "2026-08-09T09:00:00Z", "Sun and moon both.", &["planet:sun", "planet:moon"]),
            note("n4", "c3", "2026-08-12T09:00:00Z", "Ada's tenth house.", &["house:10"]),
            note("n5", "c4", "2026-08-15T09:00:00Z", "A trine today.", &["kind:trine"]),
        ] {
            t.store.put(&n).unwrap();
        }
        t
    }

    fn ids(notes: Vec<StoredNote>) -> Vec<String> {
        notes.into_iter().map(|n| n.id).collect()
    }

    /// An empty query is every note — which is what makes every other query a
    /// refinement of one rather than a thing that can mean nothing.
    #[test]
    fn an_empty_query_is_every_note_newest_first() {
        let t = seeded("empty");
        assert_eq!(ids(run(&t.store, &Query::default())), ["n5", "n4", "n3", "n2", "n1"]);
    }

    #[test]
    fn tags_narrow_by_any_or_by_all() {
        let t = seeded("tags");
        let any = Query {
            tags: vec!["planet:sun".into(), "planet:moon".into()],
            mode: Mode::Any,
            ..Default::default()
        };
        assert_eq!(ids(run(&t.store, &any)), ["n3", "n2", "n1"]);

        let all = Query { mode: Mode::All, ..any };
        assert_eq!(ids(run(&t.store, &all)), ["n3"]);
    }

    fn standing(id: &str, written_at: &str, text: &str, tags: &[&str]) -> StoredNote {
        StoredNote {
            id: id.into(),
            written_at: written_at.into(),
            anchor: Anchor::Standing,
            at: None,
            text: text.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn about(id: &str, person: &str, written_at: &str, text: &str, tags: &[&str]) -> StoredNote {
        StoredNote { anchor: Anchor::Person { id: person.into() }, ..standing(id, written_at, text, tags) }
    }

    /// "About Mira" is one question, not two. A note reaches her by naming
    /// her outright or by having been said in a reading of one of her
    /// charts, and the filter means both.
    #[test]
    fn a_person_gathers_both_senses() {
        let t = seeded("both-senses");
        t.store
            .put(&about("n6", "p1", "2026-08-20T09:00:00Z", "She deflects.", &["planet:sun"]))
            .unwrap();

        let q = Query { person: Some("p1".into()), ..Default::default() };
        assert_eq!(ids(run(&t.store, &q)), ["n6", "n3", "n2", "n1"]);
    }

    /// Craft knowledge belongs to nobody, so it must not fall into anyone's
    /// results — including through the chart side of the question.
    #[test]
    fn a_standing_note_belongs_to_no_person() {
        let t = seeded("standing-person");
        t.store
            .put(&standing("n6", "2026-08-20T09:00:00Z", "Saturn is slow.", &["planet:saturn"]))
            .unwrap();
        for p in ["p1", "p2"] {
            let q = Query { person: Some(p.into()), ..Default::default() };
            assert!(!ids(run(&t.store, &q)).contains(&"n6".to_string()));
        }
    }

    /// The dimension the whole design turns on: "my standing notes on Leo".
    /// It cannot be inferred from the other fields — leaving `person` and
    /// `chart` unset means "do not filter by those", not "has neither".
    #[test]
    fn the_anchor_kind_separates_craft_from_everything_else() {
        let t = seeded("kinds");
        t.store
            .put(&standing("n6", "2026-08-20T09:00:00Z", "Craft.", &["planet:sun"]))
            .unwrap();
        t.store
            .put(&about("n7", "p1", "2026-08-21T09:00:00Z", "Observation.", &["planet:sun"]))
            .unwrap();

        let only = |k| Query { anchor: Some(k), ..Default::default() };
        assert_eq!(ids(run(&t.store, &only(AnchorKind::Standing))), ["n6"]);
        assert_eq!(ids(run(&t.store, &only(AnchorKind::Person))), ["n7"]);
        assert_eq!(ids(run(&t.store, &only(AnchorKind::Chart))).len(), 5, "the seeded reading notes");

        // And it composes with the tags, which is the actual question:
        // everything I have written about the sun, as craft.
        let q = Query {
            tags: vec!["planet:sun".into()],
            anchor: Some(AnchorKind::Standing),
            ..Default::default()
        };
        assert_eq!(ids(run(&t.store, &q)), ["n6"]);
    }

    /// A person's notes are the notes under every chart of theirs, which is
    /// the whole reason a chart names a person rather than copying them.
    #[test]
    fn a_person_gathers_every_chart_of_theirs() {
        let t = seeded("person");
        let q = Query { person: Some("p1".into()), ..Default::default() };
        assert_eq!(ids(run(&t.store, &q)), ["n3", "n2", "n1"], "across c1 and c2");

        let q = Query { chart: Some("c1".into()), ..Default::default() };
        assert_eq!(ids(run(&t.store, &q)), ["n2", "n1"], "one sky, not both");
    }

    /// A sky nobody is consulting over belongs to no one, and must not fall
    /// into anybody's results.
    #[test]
    fn a_chart_with_no_person_answers_to_no_person() {
        let t = seeded("nobody");
        for p in ["p1", "p2"] {
            let q = Query { person: Some(p.into()), ..Default::default() };
            assert!(!ids(run(&t.store, &q)).contains(&"n5".to_string()));
        }
    }

    #[test]
    fn a_date_range_is_inclusive_at_both_ends() {
        let t = seeded("dates");
        let q = Query {
            since: Some("2026-08-05".into()),
            until: Some("2026-08-12".into()),
            ..Default::default()
        };
        assert_eq!(ids(run(&t.store, &q)), ["n4", "n3", "n2"]);

        // The last day counts in full: a note written at 09:00 on the `until`
        // day is before the end of it.
        let q = Query { until: Some("2026-08-01".into()), ..Default::default() };
        assert_eq!(ids(run(&t.store, &q)), ["n1"]);
    }

    #[test]
    fn free_text_ignores_case_and_surrounding_space() {
        let t = seeded("text");
        let q = Query { text: Some("  QUIET  ".into()), ..Default::default() };
        assert_eq!(ids(run(&t.store, &q)), ["n2"]);
    }

    /// Fields combine with AND, so adding one can only ever narrow.
    #[test]
    fn every_field_narrows_and_they_combine() {
        let t = seeded("combine");
        let q = Query {
            tags: vec!["planet:sun".into()],
            person: Some("p1".into()),
            since: Some("2026-08-05".into()),
            ..Default::default()
        };
        assert_eq!(ids(run(&t.store, &q)), ["n3"]);

        // One more field, and it is empty rather than larger.
        let narrower = Query { text: Some("nothing like this".into()), ..q };
        assert!(run(&t.store, &narrower).is_empty());
    }

    /// A saved view reopens by name and returns the same notes — which is the
    /// whole of what saving one is for.
    #[test]
    fn a_saved_view_reopens_and_answers_the_same() {
        let t = seeded("views");
        let q = Query {
            tags: vec!["planet:moon".into()],
            person: Some("p1".into()),
            ..Default::default()
        };
        let before = ids(run(&t.store, &q));

        t.store
            .put(&View { id: "v1".into(), name: "Mira on the moon".into(), query: q })
            .unwrap();

        let reopened = t.store.get::<View>("v1").unwrap().expect("a saved view comes back");
        assert_eq!(reopened.name, "Mira on the moon");
        assert_eq!(ids(run(&t.store, &reopened.query)), before);
    }

    /// Views are records like the rest, in a folder of their own.
    #[test]
    fn views_live_in_their_own_folder() {
        let t = seeded("folder");
        t.store
            .put(&View { id: "v1".into(), name: "All".into(), query: Query::default() })
            .unwrap();
        assert_eq!(View::DIR, "views");
        assert!(t.root.join("views/v1.json").is_file());
        assert_eq!(t.store.all::<View>().len(), 1);
    }

    /// The results are notes from the store, so deleting one changes what a
    /// saved view answers — a view is a question, not a copy of an answer.
    #[test]
    fn a_view_is_a_question_not_a_saved_answer() {
        let t = seeded("live");
        let q = Query { tags: vec!["planet:sun".into()], ..Default::default() };
        assert_eq!(ids(run(&t.store, &q)).len(), 2);
        t.store.remove::<StoredNote>("n1").unwrap();
        assert_eq!(ids(run(&t.store, &q)).len(), 1);
    }

    #[test]
    fn people_come_back_in_a_readable_order() {
        let t = seeded("people");
        let names: Vec<String> = people(&t.store).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Ada Vance", "Mira Holt"]);
    }
}
