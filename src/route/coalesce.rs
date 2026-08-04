//! Auto-merge: consecutive notes that share a tag read as one thought, so
//! they become one note. Grouping is decided pairwise on each note's
//! ORIGINAL tags — comparing against a group's accumulated union would chain
//! nearly everything together through ubiquitous tags like `planet:sun`.

use super::Transcript;
use crate::contract::Note;

/// Merge runs of consecutive notes whose neighboring tag sets intersect.
/// A merged note is one verbatim transcript slice from the first span's
/// start to the last span's end (untagged connective sentences included), its
/// tags the sorted union, its time anchor the first note's.
///
/// Ids are left alone — merging changes how many notes there are, so only the
/// filing step downstream knows what to number them from.
///
/// Every note here is spoken: this runs over a router's output, which the
/// Verify gate has already established came from the transcript.
pub fn coalesce(notes: Vec<Note>, transcript: &Transcript) -> Vec<Note> {
    let mut out: Vec<Note> = Vec::new();
    let mut prev_tags: Vec<String> = Vec::new();
    for note in notes {
        let shares_with_prev =
            !out.is_empty() && note.tags.iter().any(|t| prev_tags.contains(t));
        prev_tags = note.tags.clone();
        if shares_with_prev {
            out.last_mut().unwrap().absorb(note);
        } else {
            out.push(note);
        }
    }
    // Text is derived from the (possibly extended) span in one pass — the
    // gate guaranteed every input text was a verbatim slice, so this is a
    // no-op for unmerged notes. A written note has no span to derive from and
    // keeps the words it was given; the gate cannot produce one, so this is a
    // guard against a future caller rather than a case that arises today.
    for note in &mut out {
        if let Some([s, e]) = note.span {
            note.text = transcript.text[s..e].to_string();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(id: usize, span: [usize; 2], tags: &[&str]) -> Note {
        Note {
            id: format!("x{id}"),
            time: String::new(),
            span: Some(span),
            text: String::new(), // set from the transcript by callers/coalesce
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn merges_verbatim_across_an_untagged_middle_sentence() {
        let t = Transcript::load("The sun is bright. It was noon. The sun set slowly.");
        let merged = coalesce(
            vec![ex(1, [0, 18], &["planet:sun"]), ex(2, [32, 51], &["planet:sun"])],
            &t,
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "The sun is bright. It was noon. The sun set slowly.");
        assert_eq!(merged[0].span, Some([0, 51]));
        assert_eq!(merged[0].tags, vec!["planet:sun"]);
    }

    #[test]
    fn grouping_is_pairwise_on_original_tags_not_the_union() {
        // A{sun, cancer} B{sun} C{cancer}: B∩C is empty, so C stays separate
        // even though the A+B union contains cancer.
        let t = Transcript::load("Aaaa. Bbbb. Cccc.");
        let merged = coalesce(
            vec![
                ex(1, [0, 5], &["planet:sun", "sign:cancer"]),
                ex(2, [6, 11], &["planet:sun"]),
                ex(3, [12, 17], &["sign:cancer"]),
            ],
            &t,
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].tags, vec!["planet:sun", "sign:cancer"]);
        assert_eq!(merged[1].tags, vec!["sign:cancer"]);
        // Ids are not this stage's business — merging is why nothing upstream
        // can know how many notes there will be, so the filing step numbers
        // them. Whatever came in is left alone here.
        assert_eq!(merged[1].id, "x3");
    }

    #[test]
    fn disjoint_notes_pass_through() {
        let t = Transcript::load("Aaaa. Bbbb.");
        let out = coalesce(
            vec![ex(1, [0, 5], &["planet:sun"]), ex(2, [6, 11], &["planet:moon"])],
            &t,
        );
        assert_eq!(out.len(), 2);
    }
}
