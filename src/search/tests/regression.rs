//! Layer 6 — guard the behaviour we must not break (substitution/transposition
//! recall) and pin the behaviour we just fixed (mid-word indels). Recall here
//! mirrors the `comparisons` study: the target counts as recalled if it appears
//! in *any* suggestion kind (a trailing-delete typo, for instance, surfaces the
//! target as an autocomplete Extension, not a Spelling).

use super::{SV, sv_rows};

#[derive(Clone, Copy)]
enum Edit {
    Delete,
    Insert,
    Substitute,
    Transpose,
}

/// One deterministic edit at the word's midpoint (matches
/// `comparisons::apply_edit`). `None` when the edit can't be formed.
fn apply_edit(word: &str, kind: Edit) -> Option<String> {
    let cs: Vec<char> = word.chars().collect();
    let n = cs.len();
    if n < 4 {
        return None;
    }
    let mid = n / 2;
    let mut o = cs.clone();
    match kind {
        Edit::Delete => {
            o.remove(mid);
        }
        Edit::Insert => {
            o.insert(mid, 'x');
        }
        Edit::Substitute => o[mid] = if cs[mid] == 'x' { 'y' } else { 'x' },
        Edit::Transpose => {
            if cs[mid - 1] == cs[mid] {
                return None;
            }
            o.swap(mid - 1, mid);
        }
    }
    Some(o.into_iter().collect())
}

/// Top-`n` fixture words of length 5..=12, most frequent first.
fn sample_words(n: usize) -> Vec<String> {
    let mut rows: Vec<(String, u16)> = sv_rows()
        .into_iter()
        .filter(|(w, _, _)| {
            let len = w.chars().count();
            (5..=12).contains(&len) && w.chars().all(|c| c.is_ascii_lowercase())
        })
        .map(|(w, p, _)| (w, p))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.into_iter().take(n).map(|(w, _)| w).collect()
}

fn all_words(q: &str) -> Vec<String> {
    super::suggestions_of(&SV.0, q)
        .into_iter()
        .map(|(_, w, _)| w)
        .collect()
}

/// (recalled, total) for one edit kind over the sample.
fn recall(kind: Edit, words: &[String]) -> (usize, usize) {
    let mut hits = 0;
    let mut total = 0;
    for w in words {
        if let Some(typo) = apply_edit(w, kind) {
            total += 1;
            if all_words(&typo).iter().any(|g| g == w) {
                hits += 1;
            }
        }
    }
    (hits, total)
}

#[test]
fn substitution_and_transposition_recall_stays_perfect() {
    let words = sample_words(80);
    let (sh, st) = recall(Edit::Substitute, &words);
    let (th, tt) = recall(Edit::Transpose, &words);
    eprintln!("substitute {sh}/{st}  transpose {th}/{tt}");
    assert_eq!(sh, st, "substitution recall regressed: {sh}/{st}");
    assert_eq!(th, tt, "transposition recall regressed: {th}/{tt}");
}

#[test]
fn indel_recall_is_now_high() {
    // The headline fix: delete was ~6% and insert 0% before. They are now high
    // (bounded only by the top-k frequency cap, not by the old algorithmic gap).
    let words = sample_words(80);
    let (dh, dt) = recall(Edit::Delete, &words);
    let (ih, it) = recall(Edit::Insert, &words);
    eprintln!("delete {dh}/{dt}  insert {ih}/{it}");
    assert!(dh * 2 > dt, "delete recall still low: {dh}/{dt}");
    assert!(ih * 2 > it, "insert recall still low: {ih}/{it}");
}

#[test]
fn apple_example_corrects_all_four_edit_kinds() {
    // The README's own example, with the indel assertions now POSITIVE (they were
    // documented as failing before the rewrite).
    let tree = super::tree_of(&[("apple", 99, 1), ("apply", 80, 2), ("apricot", 50, 3)]);
    let words = |q: &str| -> Vec<String> { super::corrections(&tree, q) };
    assert!(words("appel").contains(&"apple".to_string()), "transpose");
    assert!(words("applr").contains(&"apple".to_string()), "substitute");
    assert!(
        words("aple").contains(&"apple".to_string()),
        "delete (was broken)"
    );
    assert!(
        words("appale").contains(&"apple".to_string()),
        "insert (was broken)"
    );
}
