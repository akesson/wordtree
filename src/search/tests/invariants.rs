//! Layer 5 — invariants that must hold for the whole fixture, checked over a
//! deterministic stride sample (no oracle needed, except a cheap distance check
//! for the soundness gate). These are fast and always-on.

use super::{SV, SuggestionType, suggestions_of, sv_rows};
use crate::trie::TreeFn;

/// A deterministic, well-spread sample of fixture words.
fn sample() -> Vec<(String, u16, u32)> {
    let mut rows = sv_rows();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().step_by(200).collect()
}

/// `word` with its middle character removed (a mid-word deletion typo).
fn mid_delete(word: &str) -> String {
    let mut cs: Vec<char> = word.chars().collect();
    if cs.len() > 1 {
        cs.remove(cs.len() / 2);
    }
    cs.into_iter().collect()
}

#[test]
fn never_returns_a_distance_two_correction() {
    let mut checked = 0;
    for (w, _, _) in sample() {
        for q in [w.clone(), mid_delete(&w)] {
            for (kind, got, _) in suggestions_of(&SV.0, &q) {
                if matches!(kind, SuggestionType::Matching | SuggestionType::Spelling) {
                    let d = strsim::damerau_levenshtein(&q, &got);
                    assert!(d <= 1, "typo {q:?} returned {got:?} at distance {d}");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 0);
}

#[test]
fn results_are_deterministic() {
    for (w, _, _) in sample() {
        for q in [w.clone(), mid_delete(&w)] {
            let a = SV.0.suggestions(&q, |_| true);
            let b = SV.0.suggestions(&q, |_| true);
            assert_eq!(a, b, "non-deterministic result for {q:?}");
        }
    }
}

#[test]
fn caps_and_bounds_are_respected() {
    for (w, _, _) in sample() {
        for q in [w.clone(), mid_delete(&w)] {
            let s = suggestions_of(&SV.0, &q);
            let matching = s
                .iter()
                .filter(|(k, _, _)| *k == SuggestionType::Matching)
                .count();
            let spelling = s
                .iter()
                .filter(|(k, _, _)| *k == SuggestionType::Spelling)
                .count();
            assert!(matching <= 1, "more than one Matching for {q:?}");
            assert!(spelling <= 3, "more than 3 Spellings for {q:?}: {s:?}");
            assert!(
                s.len() <= 6,
                "more than 6 suggestions for {q:?}: {}",
                s.len()
            );
        }
    }
}

#[test]
fn every_word_matches_itself() {
    for (w, _, idx) in sample() {
        let matched =
            SV.0.suggestions(&w, |_| true)
                .into_iter()
                .any(|s| s.kind == SuggestionType::Matching && s.expr_index == idx);
        assert!(matched, "word {w:?} (idx {idx}) did not match itself");
    }
}

#[test]
fn walk_prunes_the_vast_majority_of_the_tree() {
    use crate::ledger::LineType;
    // The DL walk should descend at most ~one level past the query length, so it
    // touches a tiny fraction of the ~20k-node tree. A generous bound (the
    // prototype touched ~600) catches a catastrophic prune regression without
    // being flaky.
    for q in ["allmanhet", "alla", "afrikanska"] {
        let mut ledger = crate::StateLedger::default();
        let _ =
            SV.0.root()
                .suggestions_with_ledger(q, |_| true, &mut ledger);
        let visited = ledger
            .0
            .iter()
            .filter(|l| matches!(l.line, LineType::Dist { .. }))
            .count();
        assert!(
            visited < 3000,
            "walk visited {visited} nodes for {q:?} (prune regressed?)"
        );
    }
}
