//! The Verify gate (total): the provenance check every router's output must
//! pass before entering the artifact.

use super::{RawNote, Transcript};
use crate::contract::Note;
use crate::vocab;

/// Reject any note whose text is not a verbatim slice of the transcript or
/// whose tags fall outside the vocabulary. Returns the accepted notes with the
/// span each one came from, and a warning per rejected span — the provenance
/// chain surfaced to the caller rather than stderr, so every frontend can
/// decide how to show it.
///
/// **The span is returned beside the note, not on it.** It is routing-time
/// scaffolding: `coalesce` needs it to take a verbatim slice across a merge,
/// and `route::file` drops it on the way to storage. This is the only place
/// provenance is enforced, and after this point it is a property the words
/// have rather than a field they carry.
pub fn verify_gate(
    transcript: &Transcript,
    raw: Vec<RawNote>,
) -> (Vec<(Note, [usize; 2])>, Vec<String>) {
    let mut out = Vec::new();
    let mut rejected = Vec::new();
    for r in raw {
        let (s, e) = r.span;
        if s >= e {
            rejected.push(format!("rejected span {s}..{e} (empty — nothing was said there)"));
            continue;
        }
        let Some(text) = transcript.text.get(s..e) else {
            rejected.push(format!("rejected span {s}..{e} (out of bounds / not a char boundary)"));
            continue;
        };
        let bad: Vec<&String> = r.tags.iter().filter(|t| !vocab::contains(t)).collect();
        if !bad.is_empty() {
            rejected.push(format!("rejected span {s}..{e} (tags outside vocabulary: {bad:?})"));
            continue;
        }
        if r.tags.is_empty() {
            continue;
        }
        let at = transcript.time_at(s);
        out.push((
            Note {
                id: String::new(), // numbered once the notes have settled
                at: (!at.is_empty()).then_some(at),
                text: text.to_string(),
                tags: r.tags,
            },
            [s, e],
        ));
    }
    (out, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_rejects_out_of_vocab_tags() {
        let t = Transcript::load("Your sun is strong.");
        let raw = vec![RawNote { span: (0, 19), tags: vec!["planet:vulcan".into()] }];
        let (ok, rejected) = verify_gate(&t, raw);
        assert!(ok.is_empty());
        // the rejection is surfaced as a warning, not silently dropped
        assert_eq!(rejected.len(), 1);
        assert!(rejected[0].contains("outside vocabulary"), "{rejected:?}");
    }

    #[test]
    fn gate_rejects_invalid_spans() {
        let t = Transcript::load("short");
        let raw = vec![RawNote { span: (0, 99), tags: vec!["planet:sun".into()] }];
        let (ok, rejected) = verify_gate(&t, raw);
        assert!(ok.is_empty());
        assert_eq!(rejected.len(), 1);
    }

    #[test]
    fn gate_output_is_verbatim() {
        let t = Transcript::load("The moon in cancer. Something else.");
        let raw = vec![RawNote { span: (0, 19), tags: vec!["planet:moon".into()] }];
        let (ok, rejected) = verify_gate(&t, raw);
        assert_eq!(ok.len(), 1);
        assert!(rejected.is_empty());
        let (note, [s, e]) = &ok[0];
        assert_eq!(note.text, &t.text[*s..*e]);
        assert_eq!(note.text, "The moon in cancer.");
    }

    /// An empty span selects no words, so there is nothing to have been said
    /// there and nothing to check against the transcript. It used to be
    /// rejected because `[0, 0]` was the written-note sentinel; it is rejected
    /// now because a note with no words is not a note.
    #[test]
    fn gate_rejects_an_empty_span() {
        let t = Transcript::load("The moon in cancer.");
        let raw = vec![RawNote { span: (4, 4), tags: vec!["planet:moon".into()] }];
        let (ok, rejected) = verify_gate(&t, raw);
        assert!(ok.is_empty());
        assert_eq!(rejected.len(), 1);
        assert!(rejected[0].contains("nothing was said"), "{rejected:?}");
    }
}
