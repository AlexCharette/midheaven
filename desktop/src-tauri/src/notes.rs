//! The one way a note is written.
//!
//! There were two. `lib::add_in` put a note into the open chart's list and
//! `lib::write_journal_note` put one into the journal folder, and apart from
//! where they landed they were the same function written twice: trim the
//! text, refuse it when empty — with the same sentence, spelled out in three
//! places — check the tags against a vocabulary, sort and dedupe them, build
//! a `Note`.
//!
//! Two copies of one rule drift, and these had. `add_in` called `retag` when
//! the writer picked no tags, so the router filed the note from its own
//! words; `write_journal_note` did not, so a journal note written without
//! tags stayed untagged forever and never appeared under any element. Nothing
//! chose that difference — it is just what happens to the copy nobody was
//! looking at.
//!
//! One path, and it retags: a note the writer did not file should be filed by
//! the words, wherever it was written.

use crate::store::{Anchor, Store, StoredNote};
use astro::i18n::Locale;
use astro::route::retag;

/// What a note needs to be written, apart from the store it goes in.
pub struct Written<'a> {
    /// `None` mints a new note; `Some` amends the one with that id, keeping
    /// its `writtenAt` and its place in the reading.
    pub id: Option<String>,
    /// What it is about. An amendment ignores this and keeps what the note
    /// already had: moving a note between a sky, a person and nobody is a
    /// different act from correcting its words, and nothing offers it.
    pub anchor: Anchor,
    pub text: &'a str,
    /// Tags the writer picked. Empty means "file it from the words".
    pub tags: Vec<String>,
    pub locale: Locale,
    /// Milliseconds since the epoch — handed in, not read, so this is
    /// testable.
    pub now_ms: i64,
}

/// Write or amend a note.
///
/// The same call does both, because a note is one file: replacing it is a
/// write rather than a read-modify-write of a shared log.
pub fn write(store: &Store, w: Written<'_>) -> Result<StoredNote, String> {
    let text = w.text.trim();
    if text.is_empty() {
        return Err("a note needs words".to_string());
    }
    if let Some(bad) = w.tags.iter().find(|t| !astro::vocab::contains(t)) {
        return Err(format!("unknown tag {bad}"));
    }
    let mut tags = if w.tags.is_empty() { retag(text, w.locale) } else { w.tags };
    tags.sort();
    tags.dedup();

    let now = chrono::DateTime::from_timestamp_millis(w.now_ms)
        .ok_or("the clock is outside any representable date")?
        .fixed_offset();

    // An amendment keeps the note it is amending: its id, when it was first
    // written, what it is about, and the moment in the recording it points
    // at. Only the words and what they file under are the writer's to change.
    let existing = match &w.id {
        Some(id) => store.get::<StoredNote>(id)?,
        None => None,
    };
    let note = StoredNote {
        id: match w.id {
            Some(id) => id,
            None => store.mint_id::<StoredNote>(w.now_ms.max(0) as u64),
        },
        written_at: existing
            .as_ref()
            .map(|e| e.written_at.clone())
            .unwrap_or_else(|| now.to_rfc3339()),
        anchor: existing.as_ref().map(|e| e.anchor.clone()).unwrap_or(w.anchor),
        at: existing.as_ref().and_then(|e| e.at.clone()),
        text: text.to_string(),
        tags,
    };
    store.put(&note)?;
    Ok(note)
}

/// File a take's routed notes under a chart, in the order they were spoken.
///
/// Routing hands back unnumbered notes; this is where they get ids. Minting
/// one at a time against the folder is what keeps a burst inside a single
/// millisecond from colliding.
pub fn file_take(
    store: &Store,
    chart: &str,
    routed: Vec<astro::contract::Note>,
    now_ms: i64,
) -> Result<Vec<StoredNote>, String> {
    let written_at = chrono::DateTime::from_timestamp_millis(now_ms)
        .ok_or("the clock is outside any representable date")?
        .fixed_offset()
        .to_rfc3339();
    let mut out = Vec::new();
    for note in routed {
        let stored = StoredNote {
            id: store.mint_id::<StoredNote>(now_ms.max(0) as u64),
            written_at: written_at.clone(),
            anchor: Anchor::Chart { id: chart.to_string() },
            at: note.at,
            text: note.text,
            tags: note.tags,
        };
        store.put(&stored)?;
        out.push(stored);
    }
    Ok(out)
}

/// Every note filed under one chart, in reading order.
///
/// Chart-anchored only, deliberately. Standing notes about this chart's
/// elements are shown beside the reading rather than in it — and this is the
/// function the artifact and the PDF export through, so it is also the line
/// that keeps a practitioner's own notes out of a client's keepsake.
pub fn under(store: &Store, chart: &str) -> Vec<StoredNote> {
    let mut out: Vec<StoredNote> = store
        .all::<StoredNote>()
        .into_iter()
        .filter(|n| n.anchor.chart() == Some(chart))
        .collect();
    out.sort_by(|a, b| order_key(a).cmp(&order_key(b)));
    out
}

/// How notes sit in a reading: what was said, in the order it was said, then
/// what was typed, in the order it was typed.
///
/// Position in a list used to answer this, back when the chart held one, and
/// `x{n}` ids made it look like an ordering of its own. Ids cannot answer it
/// now: two notes minted in the same millisecond are `{stem}` and
/// `{stem}-1`, and the tenth is `{stem}-10`, which sorts before `{stem}-2`.
///
/// `at` can, and means something while doing it. It is session-offset across
/// takes, so it orders every spoken note in one reading even across several
/// recordings, and it is zero-padded `HH:MM:SS`, so text order is clock
/// order.
fn order_key(n: &StoredNote) -> (bool, &str, &str, &str) {
    (n.at.is_none(), n.at.as_deref().unwrap_or(""), &n.written_at, &n.id)
}

/// Merge a note into the one before it in reading order: texts joined, tags
/// unioned, the earlier note's anchor kept.
pub fn merge_up(store: &Store, chart: &str, id: &str) -> Result<(), String> {
    let ordered = under(store, chart);
    let i = ordered.iter().position(|n| n.id == id).ok_or("no such note")?;
    if i == 0 {
        return Err("the first note has nothing above it to merge into".to_string());
    }
    let cur = &ordered[i];
    let mut into = ordered[i - 1].clone();
    into.text = format!("{} {}", into.text, cur.text);
    into.tags.extend(cur.tags.iter().cloned());
    into.tags.sort();
    into.tags.dedup();
    // Write the survivor before removing the absorbed one: an interruption
    // between the two then costs a duplicated sentence rather than a lost
    // note.
    store.put(&into)?;
    store.remove::<StoredNote>(&cur.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Chart;
    use astro::contract::BirthSeed;

    struct TempStore {
        store: Store,
        root: std::path::PathBuf,
    }

    impl TempStore {
        fn new(name: &str) -> TempStore {
            let root = std::env::temp_dir().join(format!("midheaven-notes-{name}"));
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

    const T0: i64 = 1_785_825_000_000;

    fn chart_record() -> Chart {
        Chart {
            id: "c1".into(),
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
            person: None,
        }
    }

    fn written<'a>(text: &'a str, tags: Vec<String>) -> Written<'a> {
        Written {
            id: None,
            anchor: Anchor::Chart { id: "c1".into() },
            text,
            tags,
            locale: Locale::En,
            now_ms: T0,
        }
    }

    /// The divergence this module exists to end: a note written with no tags
    /// is filed by its words, wherever it was written. The journal path used
    /// not to do this.
    #[test]
    fn a_note_with_no_tags_picked_is_filed_by_the_router() {
        let t = TempStore::new("retag");
        let n = write(&t.store, written("The sun rules this whole chart.", vec![])).unwrap();
        assert!(n.tags.contains(&"planet:sun".to_string()), "tags: {:?}", n.tags);
    }

    #[test]
    fn picked_tags_are_kept_sorted_and_deduped_and_checked() {
        let t = TempStore::new("picked");
        let n = write(
            &t.store,
            written("Anything at all.", vec!["sign:leo".into(), "planet:sun".into(), "sign:leo".into()]),
        )
        .unwrap();
        assert_eq!(n.tags, vec!["planet:sun", "sign:leo"]);

        let err = write(&t.store, written("Anything.", vec!["planet:vulcan".into()])).unwrap_err();
        assert!(err.contains("unknown tag"), "{err}");
    }

    #[test]
    fn a_note_of_pure_whitespace_is_not_a_note() {
        let t = TempStore::new("words");
        assert_eq!(write(&t.store, written("   ", vec![])).unwrap_err(), "a note needs words");
    }

    /// An amendment is the same call, and keeps what was not amended.
    #[test]
    fn amending_keeps_the_id_the_anchor_and_when_it_was_written() {
        let t = TempStore::new("amend");
        let first = write(&t.store, written("The sun.", vec![])).unwrap();

        let amended = write(
            &t.store,
            Written {
                id: Some(first.id.clone()),
                now_ms: T0 + 60_000,
                ..written("The moon, actually.", vec![])
            },
        )
        .unwrap();

        assert_eq!(amended.id, first.id);
        assert_eq!(amended.written_at, first.written_at, "when it was first written");
        assert_eq!(amended.anchor, Anchor::Chart { id: "c1".into() });
        assert_eq!(amended.text, "The moon, actually.");
        assert!(amended.tags.contains(&"planet:moon".to_string()), "retagged from the new words");
        assert_eq!(t.store.all::<StoredNote>().len(), 1, "amended, not duplicated");
    }

    /// A take's notes keep the order they were spoken in, across the burst of
    /// ids minted inside one millisecond — which is the case that id order
    /// gets wrong once there are ten of them.
    #[test]
    fn a_take_files_in_spoken_order_even_past_ten_notes() {
        let t = TempStore::new("order");
        t.store.put(&chart_record()).unwrap();
        let routed: Vec<astro::contract::Note> = (0..12)
            .map(|i| astro::contract::Note {
                id: String::new(),
                at: Some(format!("00:{:02}:00", i)),
                text: format!("sentence {i}"),
                tags: vec!["planet:sun".into()],
            })
            .collect();

        file_take(&t.store, "c1", routed, T0).unwrap();

        let texts: Vec<String> =
            under(&t.store, "c1").into_iter().map(|n| n.text).collect();
        let expected: Vec<String> = (0..12).map(|i| format!("sentence {i}")).collect();
        assert_eq!(texts, expected);
    }

    /// Spoken first, in clock order; typed after, in the order they were
    /// typed.
    #[test]
    fn typed_notes_come_after_spoken_ones() {
        let t = TempStore::new("mixed");
        file_take(
            &t.store,
            "c1",
            vec![astro::contract::Note {
                id: String::new(),
                at: Some("00:05:00".into()),
                text: "spoken".into(),
                tags: vec![],
            }],
            T0,
        )
        .unwrap();
        write(&t.store, written("typed", vec!["planet:sun".into()])).unwrap();

        let texts: Vec<String> = under(&t.store, "c1").into_iter().map(|n| n.text).collect();
        assert_eq!(texts, vec!["spoken", "typed"]);
    }

    /// Notes under one chart are not notes under another.
    #[test]
    fn under_sees_only_its_own_charts_notes() {
        let t = TempStore::new("scope");
        write(&t.store, written("mine", vec!["planet:sun".into()])).unwrap();
        write(
            &t.store,
            Written {
                anchor: Anchor::Chart { id: "c2".into() },
                now_ms: T0 + 1,
                ..written("theirs", vec!["planet:sun".into()])
            },
        )
        .unwrap();

        assert_eq!(under(&t.store, "c1").len(), 1);
        assert_eq!(under(&t.store, "c2").len(), 1);
        assert_eq!(t.store.all::<StoredNote>().len(), 2, "both are in the store");
    }

    #[test]
    fn merging_joins_the_words_and_unions_the_tags() {
        let t = TempStore::new("merge");
        file_take(
            &t.store,
            "c1",
            vec![
                astro::contract::Note {
                    id: String::new(),
                    at: Some("00:01:00".into()),
                    text: "The sun is loud.".into(),
                    tags: vec!["planet:sun".into()],
                },
                astro::contract::Note {
                    id: String::new(),
                    at: Some("00:02:00".into()),
                    text: "So is the moon.".into(),
                    tags: vec!["planet:moon".into()],
                },
            ],
            T0,
        )
        .unwrap();
        let second = under(&t.store, "c1")[1].id.clone();

        merge_up(&t.store, "c1", &second).unwrap();

        let left = under(&t.store, "c1");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].text, "The sun is loud. So is the moon.");
        assert_eq!(left[0].tags, vec!["planet:moon", "planet:sun"]);
        assert_eq!(left[0].at.as_deref(), Some("00:01:00"), "the earlier anchor is kept");
    }

    /// The export boundary, as a test.
    ///
    /// `under` is what the artifact and the PDF render through, so anything
    /// it returns leaves the building. A practitioner's standing notes and
    /// their observations about a client must not — the first are private
    /// craft, the second may be things you would never hand the person they
    /// are about.
    #[test]
    fn a_readings_notes_are_its_own_and_nothing_else() {
        let t = TempStore::new("boundary");
        write(&t.store, written("said in the reading", vec!["planet:sun".into()])).unwrap();
        write(
            &t.store,
            Written {
                anchor: Anchor::Standing,
                now_ms: T0 + 1,
                ..written("what Saturn means to me", vec!["planet:sun".into()])
            },
        )
        .unwrap();
        write(
            &t.store,
            Written {
                anchor: Anchor::Person { id: "p1".into() },
                now_ms: T0 + 2,
                ..written("she keeps deflecting", vec!["planet:sun".into()])
            },
        )
        .unwrap();

        // All three carry the same tag, so tags cannot be what separates them.
        assert_eq!(t.store.all::<StoredNote>().len(), 3);
        let exported: Vec<String> =
            under(&t.store, "c1").into_iter().map(|n| n.text).collect();
        assert_eq!(exported, vec!["said in the reading"]);
    }

    /// The premise of the whole design: having no anchor is what tells a
    /// considered note about Leo apart from a transcript sentence that
    /// happened to say the word.
    #[test]
    fn a_standing_note_is_findable_among_notes_sharing_its_tags() {
        let t = TempStore::new("standing");
        write(&t.store, written("someone said leo out loud", vec!["sign:leo".into()])).unwrap();
        write(
            &t.store,
            Written {
                anchor: Anchor::Standing,
                now_ms: T0 + 1,
                ..written("Leo wants to be seen doing the thing", vec!["sign:leo".into()])
            },
        )
        .unwrap();

        let standing: Vec<String> = t
            .store
            .all::<StoredNote>()
            .into_iter()
            .filter(|n| n.anchor.is_standing())
            .map(|n| n.text)
            .collect();
        assert_eq!(standing, vec!["Leo wants to be seen doing the thing"]);
    }

    /// Correcting the words of a note does not move it between a sky, a
    /// person and nobody. That is a different act, and nothing offers it.
    #[test]
    fn amending_cannot_change_what_a_note_is_about() {
        let t = TempStore::new("reanchor");
        let first = write(
            &t.store,
            Written { anchor: Anchor::Standing, ..written("about Leo", vec!["sign:leo".into()]) },
        )
        .unwrap();

        let amended = write(
            &t.store,
            Written {
                id: Some(first.id.clone()),
                anchor: Anchor::Chart { id: "c1".into() },
                now_ms: T0 + 60_000,
                ..written("about Leo, corrected", vec!["sign:leo".into()])
            },
        )
        .unwrap();

        assert_eq!(amended.anchor, Anchor::Standing, "the anchor it had, not the one asked for");
        assert_eq!(amended.text, "about Leo, corrected");
    }

    #[test]
    fn the_first_note_has_nothing_to_merge_into() {
        let t = TempStore::new("merge-first");
        let only = write(&t.store, written("alone", vec!["planet:sun".into()])).unwrap();
        let err = merge_up(&t.store, "c1", &only.id).unwrap_err();
        assert!(err.contains("nothing above it"), "{err}");
        assert_eq!(under(&t.store, "c1").len(), 1, "and nothing was lost trying");
    }
}
