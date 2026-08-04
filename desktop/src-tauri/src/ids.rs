//! Short, stable, time-ordered ids.
//!
//! Two things need one: a journal note, whose id becomes its filename, and a
//! saved reading, whose id is what a note points at when it is about someone.
//! Both want the same properties and neither wants a dependency — so this is
//! the one place that says what an id looks like.
//!
//! Base 36 of the millisecond it was minted: short, sortable as plain text (so
//! a directory listing reads in written order before anything parses a date),
//! and drawn from a charset that cannot name a parent directory.
//!
//! The clock is a parameter rather than a call. That is what makes the
//! collision behaviour testable without waiting a millisecond for it.

/// Whether a string is one of ours: `[a-z0-9-]`, non-empty, bounded.
///
/// Checked wherever an id arrives from outside rather than trusted, because an
/// id the webview hands back becomes a filename, and `../../something` is a
/// filename too. One rule forecloses traversal, path separators, and Windows'
/// reserved characters together.
pub fn is_safe(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-'))
}

/// A free id for something made at `now_ms`, given a way to ask what is taken.
///
/// Uniqueness comes from `taken`, not from the clock: two things made inside
/// one millisecond get `{stem}` and `{stem}-1` rather than the same id twice.
/// The search terminates because each probe names a different candidate.
pub fn mint(now_ms: u64, taken: impl Fn(&str) -> bool) -> String {
    let stem = base36(now_ms);
    if !taken(&stem) {
        return stem;
    }
    (1..).map(|n| format!("{stem}-{n}")).find(|id| !taken(id)).expect("a free id")
}

fn base36(mut n: u64) -> String {
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
    use std::collections::BTreeSet;

    #[test]
    fn base36_is_what_it_claims() {
        assert_eq!(base36(0), "0");
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
        assert_eq!(base36(1_785_825_000_000), "msea3gw0");
    }

    #[test]
    fn a_minted_id_is_a_safe_id() {
        assert!(is_safe(&mint(1_785_825_000_000, |_| false)));
        assert!(is_safe(&mint(0, |_| false)));
        assert!(is_safe(&mint(u64::MAX, |_| false)));
    }

    #[test]
    fn nothing_that_could_name_a_file_elsewhere_is_safe() {
        for bad in ["../victim", "../../etc/passwd", "a/b", "a\\b", "a:b", "", "Abc", "a.b", "a b"] {
            assert!(!is_safe(bad), "{bad:?} passed");
        }
        assert!(!is_safe(&"a".repeat(65)), "an unbounded id is not safe");
        assert!(is_safe(&"a".repeat(64)));
    }

    /// Uniqueness is the caller's set, not the clock's resolution — a burst
    /// written inside one millisecond must still be a burst of distinct ids.
    #[test]
    fn one_millisecond_can_mint_many() {
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for _ in 0..5 {
            let id = mint(1_785_825_000_000, |c| taken.contains(c));
            assert!(taken.insert(id.clone()), "minted {id} twice");
        }
        assert_eq!(taken.len(), 5);
        assert!(taken.iter().all(|id| is_safe(id)));
    }

    /// Text order is written order, which is what makes a bare directory
    /// listing readable before anything parses a timestamp.
    #[test]
    fn later_moments_sort_after_earlier_ones_as_text() {
        let a = mint(1_785_825_000_000, |_| false);
        let b = mint(1_785_825_000_001, |_| false);
        let c = mint(1_785_925_000_000, |_| false);
        assert!(a < b && b < c, "{a} {b} {c}");
    }
}
