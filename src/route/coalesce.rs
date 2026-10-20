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
/// Takes and returns notes paired with the spans they came from, because the
/// span is what makes the merge verbatim: the result is one continuous slice
/// of the transcript, *not* the merged notes' texts joined together. Those two
/// differ exactly by the untagged sentences in between, which is to say they
/// differ by whether the merged note still reads as something a person said.
pub fn coalesce(
    notes: Vec<(Note, [usize; 2])>,
    transcript: &Transcript,
) -> Vec<(Note, [usize; 2])> {
    let mut out: Vec<(Note, [usize; 2])> = Vec::new();
    let mut prev_tags: Vec<String> = Vec::new();
    for (note, span) in notes {
        let shares_with_prev =
            !out.is_empty() && note.tags.iter().any(|t| prev_tags.contains(t));
        prev_tags = note.tags.clone();
        if shares_with_prev {
            let (kept, kept_span) = out.last_mut().unwrap();
            kept.absorb(note);
            kept_span[1] = span[1];
        } else {
            out.push((note, span));
        }
    }
    // Text is re-derived from the (possibly extended) span in one pass. The
    // gate guaranteed every input text was already a verbatim slice, so this
    // is a no-op for anything that did not merge.
    for (note, [s, e]) in &mut out {
        note.text = transcript.text[*s..*e].to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(id: usize, span: [usize; 2], tags: &[&str]) -> (Note, [usize; 2]) {
        (
            Note {
                id: format!("x{id}"),
                at: None,
                text: String::new(), // set from the transcript by coalesce
                tags: tags.iter().map(|s| s.to_string()).collect(),
            },
            span,
        )
    }

    #[test]
    fn merges_verbatim_across_an_untagged_middle_sentence() {
        let t = Transcript::load("The sun is bright. It was noon. The sun set slowly.");
        let merged = coalesce(
            vec![ex(1, [0, 18], &["planet:sun"]), ex(2, [32, 51], &["planet:sun"])],
            &t,
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].0.text, "The sun is bright. It was noon. The sun set slowly.");
        assert_eq!(merged[0].1, [0, 51]);
        assert_eq!(merged[0].0.tags, vec!["planet:sun"]);
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
        assert_eq!(merged[0].0.tags, vec!["planet:sun", "sign:cancer"]);
        assert_eq!(merged[1].0.tags, vec!["sign:cancer"]);
        // Ids are not this stage's business — merging is why nothing upstream
        // can know how many notes there will be, so the filing step numbers
        // them. Whatever came in is left alone here.
        assert_eq!(merged[1].0.id, "x3");
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
