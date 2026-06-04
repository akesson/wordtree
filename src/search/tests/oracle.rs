//! Layer 3 — exhaustive, deterministic checks against a brute-force Damerau-
//! Levenshtein oracle (`strsim`) over the real Swedish fixture. For every
//! single-character edit at every position of a frequency-ranked sample of
//! words we verify two things:
//!   - soundness: every correction the engine returns is genuinely within edit
//!     distance one of the typo (it never invents far matches);
//!   - recall: when the source word is the uniquely most frequent word within
//!     distance one of the typo, the engine returns it.
//!
//! No randomness is used — the sample and the edits are enumerated
//! deterministically, so failures are reproducible.

use super::{SV, SuggestionType, sv_rows};
use std::collections::HashSet;

/// Every distinct string one edit away from `word`.
fn all_single_edits(word: &str) -> Vec<String> {
    let cs: Vec<char> = word.chars().collect();
    let mut out = HashSet::new();
    for i in 0..cs.len() {
        let mut v = cs.clone();
        v.remove(i);
        out.insert(v.into_iter().collect::<String>());
    }
    for i in 0..=cs.len() {
        for f in ['x', 'a'] {
            let mut v = cs.clone();
            v.insert(i, f);
            out.insert(v.into_iter().collect());
        }
    }
    for i in 0..cs.len() {
        let mut v = cs.clone();
        v[i] = if v[i] == 'x' { 'y' } else { 'x' };
        out.insert(v.into_iter().collect());
    }
    for i in 0..cs.len().saturating_sub(1) {
        if cs[i] != cs[i + 1] {
            let mut v = cs.clone();
            v.swap(i, i + 1);
            out.insert(v.into_iter().collect());
        }
    }
    out.into_iter().collect()
}

/// Top-`n` fixture words of length 5..=12, most frequent first (deterministic).
fn sample_words(n: usize) -> Vec<(String, u16)> {
    let mut rows: Vec<(String, u16)> = sv_rows()
        .into_iter()
        .filter(|(w, _, _)| {
            let len = w.chars().count();
            (5..=12).contains(&len) && w.chars().all(|c| c.is_ascii_lowercase())
        })
        .map(|(w, p, _)| (w, p))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(n);
    rows
}

/// Engine corrections (Matching/Spelling) for `q`, as `(word, percentile)`,
/// decoded through the prebuilt fixture lookup (no per-call rebuild).
fn engine_corrections(q: &str) -> Vec<(String, u16)> {
    use crate::trie::TreeFn;
    let lookup = &SV.1;
    SV.0
        .suggestions(q, |_| true)
        .into_iter()
        .filter(|s| matches!(s.kind, SuggestionType::Matching | SuggestionType::Spelling))
        .filter_map(|s| lookup.get(&s.expr_index).map(|e| (e.path.clone(), e.percentile)))
        .collect()
}

#[test]
fn soundness_no_false_positives() {
    // Every correction the engine returns must really be within DL<=1. This must
    // hold for *every* generated typo, with no exceptions.
    let words: Vec<String> = sample_words(40).into_iter().map(|(w, _)| w).collect();
    let mut checked = 0usize;
    for w in &words {
        for typo in all_single_edits(w) {
            for (got, _) in engine_corrections(&typo) {
                let d = strsim::damerau_levenshtein(&typo, &got);
                assert!(
                    d <= 1,
                    "FALSE POSITIVE: typo {typo:?} returned {got:?} at distance {d}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "suspiciously few corrections checked ({checked})");
    eprintln!("soundness: {checked} corrections checked, all within DL<=1");
}

#[test]
fn recall_returns_the_dominant_correction() {
    // When the source word is the *uniquely* most frequent word within DL<=1 of
    // a typo, it always fits in the cap (>=1) and must be returned. The strict
    // frequency-max condition avoids tie/cap flakiness.
    let rows = sv_rows();
    let sample = sample_words(40);
    let mut required = 0usize;
    let mut hits = 0usize;
    for (w, w_pct) in &sample {
        for typo in all_single_edits(w) {
            // brute-force DL<=1 neighbours (length filter is a cheap necessary
            // condition: DL<=1 implies |len difference| <= 1).
            let tl = typo.chars().count() as isize;
            let mut dominant = true;
            for (other, p, _) in &rows {
                if other == w {
                    continue;
                }
                if (other.chars().count() as isize - tl).abs() > 1 {
                    continue;
                }
                if *p >= *w_pct && strsim::damerau_levenshtein(&typo, other) <= 1 {
                    dominant = false;
                    break;
                }
            }
            if !dominant {
                continue;
            }
            required += 1;
            let returned = engine_corrections(&typo).iter().any(|(g, _)| g == w);
            assert!(returned, "RECALL MISS: typo {typo:?} did not return dominant {w:?}");
            hits += 1;
        }
    }
    assert!(required > 200, "too few dominant cases exercised ({required})");
    eprintln!("recall: {hits}/{required} dominant corrections returned");
}
