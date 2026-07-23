//! The Verify gate (total): the provenance check every router's output must
//! pass before entering the artifact.

use super::{RawExcerpt, Transcript};
use crate::contract::Excerpt;
use std::collections::BTreeSet;

/// Reject any excerpt whose text is not a verbatim slice of the transcript or
/// whose tags fall outside the chart vocabulary. Returns the accepted excerpts
/// (with ids and time anchors assigned) alongside a warning per rejected span
/// — the provenance chain surfaced to the caller rather than stderr, so every
/// frontend can decide how to show it.
pub fn verify_gate(
    transcript: &Transcript,
    raw: Vec<RawExcerpt>,
    vocab: &BTreeSet<String>,
) -> (Vec<Excerpt>, Vec<String>) {
    let mut out = Vec::new();
    let mut rejected = Vec::new();
    for r in raw {
        let (s, e) = r.span;
        let Some(text) = transcript.text.get(s..e) else {
            rejected.push(format!("rejected span {s}..{e} (out of bounds / not a char boundary)"));
            continue;
        };
        let bad: Vec<&String> = r.tags.iter().filter(|t| !vocab.contains(*t)).collect();
        if !bad.is_empty() {
            rejected.push(format!("rejected span {s}..{e} (tags outside vocabulary: {bad:?})"));
            continue;
        }
        if r.tags.is_empty() {
            continue;
        }
        out.push(Excerpt {
            id: format!("x{}", out.len() + 1),
            time: transcript.time_at(s),
            span: [s, e],
            text: text.to_string(),
            tags: r.tags,
        });
    }
    (out, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::test_vocab as vocab;

    #[test]
    fn gate_rejects_out_of_vocab_tags() {
        let t = Transcript::load("Your sun is strong.");
        let raw = vec![RawExcerpt { span: (0, 19), tags: vec!["planet:vulcan".into()] }];
        let (ok, rejected) = verify_gate(&t, raw, &vocab(&["planet:sun"]));
        assert!(ok.is_empty());
        // the rejection is surfaced as a warning, not silently dropped
        assert_eq!(rejected.len(), 1);
        assert!(rejected[0].contains("outside vocabulary"), "{rejected:?}");
    }

    #[test]
    fn gate_rejects_invalid_spans() {
        let t = Transcript::load("short");
        let raw = vec![RawExcerpt { span: (0, 99), tags: vec!["planet:sun".into()] }];
        let (ok, rejected) = verify_gate(&t, raw, &vocab(&["planet:sun"]));
        assert!(ok.is_empty());
        assert_eq!(rejected.len(), 1);
    }

    #[test]
    fn gate_output_is_verbatim() {
        let t = Transcript::load("The moon in cancer. Something else.");
        let raw = vec![RawExcerpt { span: (0, 19), tags: vec!["planet:moon".into()] }];
        let (ok, rejected) = verify_gate(&t, raw, &vocab(&["planet:moon"]));
        assert_eq!(ok.len(), 1);
        assert!(rejected.is_empty());
        assert_eq!(ok[0].text, &t.text[ok[0].span[0]..ok[0].span[1]]);
        assert_eq!(ok[0].text, "The moon in cancer.");
    }
}
