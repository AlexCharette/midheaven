//! Auto-merge: consecutive passages that share a tag read as one thought, so
//! they become one passage. Grouping is decided pairwise on each passage's
//! ORIGINAL tags — comparing against a group's accumulated union would chain
//! nearly everything together through ubiquitous tags like `planet:sun`.

use super::Transcript;
use crate::contract::Excerpt;

/// Merge runs of consecutive excerpts whose neighboring tag sets intersect.
/// A merged excerpt is one verbatim transcript slice from the first span's
/// start to the last span's end (untagged connective sentences included), its
/// tags the sorted union, its time anchor the first passage's. Ids renumber.
pub fn coalesce(excerpts: Vec<Excerpt>, transcript: &Transcript) -> Vec<Excerpt> {
    let mut out: Vec<Excerpt> = Vec::new();
    let mut prev_tags: Vec<String> = Vec::new();
    for ex in excerpts {
        let shares_with_prev =
            !out.is_empty() && ex.tags.iter().any(|t| prev_tags.contains(t));
        prev_tags = ex.tags.clone();
        if shares_with_prev {
            out.last_mut().unwrap().absorb(ex);
        } else {
            out.push(ex);
        }
    }
    // Text is derived from the (possibly extended) span in one pass — the
    // gate guaranteed every input text was a verbatim slice, so this is a
    // no-op for unmerged passages.
    for ex in &mut out {
        ex.text = transcript.text[ex.span[0]..ex.span[1]].to_string();
    }
    super::renumber(&mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(id: usize, span: [usize; 2], tags: &[&str]) -> Excerpt {
        Excerpt {
            id: format!("x{id}"),
            time: String::new(),
            span,
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
        assert_eq!(merged[0].span, [0, 51]);
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
        // ids renumber
        assert_eq!(merged[1].id, "x2");
    }

    #[test]
    fn disjoint_passages_pass_through() {
        let t = Transcript::load("Aaaa. Bbbb.");
        let out = coalesce(
            vec![ex(1, [0, 5], &["planet:sun"]), ex(2, [6, 11], &["planet:moon"])],
            &t,
        );
        assert_eq!(out.len(), 2);
    }
}
