//! The Verify gate (total): the provenance check every router's output must
//! pass before entering the artifact.

use super::{RawNote, Transcript};
use crate::contract::Note;
use crate::vocab;

/// Reject any note whose text is not a verbatim slice of the transcript or
/// whose tags fall outside the vocabulary. Returns the accepted notes
/// (time anchors assigned, ids left empty for the filing step) with a warning
/// per rejected span
/// — the provenance chain surfaced to the caller rather than stderr, so every
/// frontend can decide how to show it.
///
/// Every note out of here is spoken, so every span is `Some` and non-empty:
/// an empty span is the wire sentinel for a *written* note, and a routed note
/// that claimed one would come back from disk as the wrong kind.
pub fn verify_gate(transcript: &Transcript, raw: Vec<RawNote>) -> (Vec<Note>, Vec<String>) {
    let mut out = Vec::new();
    let mut rejected = Vec::new();
    for r in raw {
        let (s, e) = r.span;
        if s >= e {
            rejected.push(format!("rejected span {s}..{e} (empty — that is a written note)"));
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
        out.push(Note {
            id: String::new(), // numbered once the notes have settled
            time: transcript.time_at(s),
            span: Some([s, e]),
            text: text.to_string(),
            tags: r.tags,
        });
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
        let [s, e] = ok[0].span.expect("a routed note is spoken, so it has a span");
        assert_eq!(ok[0].text, &t.text[s..e]);
        assert_eq!(ok[0].text, "The moon in cancer.");
    }

    /// An empty span is how a *written* note is spelled on the wire. A routed
    /// note that carried one would be read back from disk as authored by hand,
    /// silently losing the provenance the gate exists to establish.
    #[test]
    fn gate_rejects_an_empty_span() {
        let t = Transcript::load("The moon in cancer.");
        let raw = vec![RawNote { span: (4, 4), tags: vec!["planet:moon".into()] }];
        let (ok, rejected) = verify_gate(&t, raw);
        assert!(ok.is_empty());
        assert_eq!(rejected.len(), 1);
        assert!(rejected[0].contains("written note"), "{rejected:?}");
    }
}
