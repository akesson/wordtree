//! Tests for `corrections()` — the fuzzy-only searcher: the exact match (when
//! the query is a word) plus complete words within the edit distance, via the
//! Damerau-Levenshtein walk. No completion sweep runs, so it never returns
//! prefix extensions — the spell-check slice of the combined suggestions().

use super::{SV_TREE, SuggestionType, first_char_neighbours, tree_of};
use crate::trie::TreeFn;

#[test]
fn corrects_a_single_edit_typo() {
    // "alxa" is one substitution from "alla" (pos 2), two edits from "alle".
    let tree = tree_of(&[("alla", 900, 1), ("alle", 800, 2), ("zzzz", 100, 3)]);
    let got = tree.corrections("alxa", |_| true);
    assert!(
        got.iter()
            .any(|s| s.kind == SuggestionType::Spelling && s.expr_index == 1),
        "alla not corrected as a Spelling: {got:?}"
    );
    let idx: Vec<u32> = got.iter().map(|s| s.expr_index).collect();
    assert!(
        !idx.contains(&2),
        "alle is distance 2, must be excluded: {idx:?}"
    );
    assert!(!idx.contains(&3), "zzzz is far, must be excluded: {idx:?}");
}

#[test]
fn does_not_return_prefix_extensions() {
    // "all" is a prefix with many continuations; corrections() must surface none
    // of them as Extensions (that is completions()' job).
    let got = SV_TREE.corrections("all", |_| true);
    assert!(
        got.iter().all(|s| s.kind != SuggestionType::Extension),
        "corrections leaked prefix extensions: {got:?}"
    );
    // contrast: the combined call DOES complete the same prefix
    assert!(
        SV_TREE
            .suggestions("all", |_| true)
            .iter()
            .any(|s| s.kind == SuggestionType::Extension),
        "precondition: suggestions() completes the prefix"
    );
}

#[test]
fn respects_the_spellings_cap() {
    // 25 first-letter neighbours, all distance 1 from "xxxx"; the len-4 not-found
    // cap bounds the result — the same cap suggestions() uses by default.
    let owned = first_char_neighbours("xxxx");
    let refs: Vec<(&str, u16, u32)> = owned.iter().map(|(w, p, i)| (w.as_str(), *p, *i)).collect();
    let tree = tree_of(&refs);
    let n = tree
        .corrections("xxxx", |_| true)
        .iter()
        .filter(|s| s.kind == SuggestionType::Spelling)
        .count();
    assert_eq!(
        n,
        crate::Caps::default().not_found,
        "len-4 not-found cap should be Caps::default().not_found"
    );
}

#[test]
fn caps_let_the_caller_widen_corrections_of_a_valid_word() {
    // The "abut"/"brad" case: a valid word whose distance-1 neighbours are also
    // words. The default `found` cap keeps only a couple; `Caps::uniform` opens
    // the spell-check surface up to every Damerau-Levenshtein ≤1 neighbour.
    let mut owned = first_char_neighbours("brad"); // 25 distance-1 words
    owned.push(("brad".to_string(), 1000, 999)); // the valid word itself
    let refs: Vec<(&str, u16, u32)> = owned.iter().map(|(w, p, i)| (w.as_str(), *p, *i)).collect();
    let tree = tree_of(&refs);

    let spelling_count = |caps| {
        tree.corrections_with("brad", |_| true, caps)
            .iter()
            .filter(|s| s.kind == SuggestionType::Spelling)
            .count()
    };

    let default = tree
        .corrections("brad", |_| true)
        .iter()
        .filter(|s| s.kind == SuggestionType::Spelling)
        .count();
    assert_eq!(
        default,
        crate::Caps::default().found,
        "valid-word query uses the bounded found-cap by default"
    );
    assert_eq!(
        spelling_count(crate::Caps::uniform(64)),
        25,
        "a generous uniform cap surfaces every distance-1 neighbour"
    );
}

#[test]
fn includes_matching_when_query_is_a_word() {
    let tree = tree_of(&[("alla", 900, 1), ("alle", 800, 2)]);
    let got = tree.corrections("alla", |_| true);
    assert_eq!(
        got[0].kind,
        SuggestionType::Matching,
        "exact word not first: {got:?}"
    );
    assert_eq!(got[0].expr_index, 1);
    // "alle" is one substitution from "alla" -> a Spelling
    assert!(
        got.iter()
            .any(|s| s.kind == SuggestionType::Spelling && s.expr_index == 2),
        "neighbour not corrected: {got:?}"
    );
}

#[test]
fn is_candidate_filter_excludes_words() {
    // "alxa" is distance 1 from both "alla" and "alga" (middle substitution).
    let tree = tree_of(&[("alla", 900, 1), ("alga", 800, 2)]);
    let all: Vec<u32> = tree
        .corrections("alxa", |_| true)
        .into_iter()
        .map(|s| s.expr_index)
        .collect();
    assert!(
        all.contains(&1) && all.contains(&2),
        "precondition: both candidates: {all:?}"
    );
    let filtered: Vec<u32> = tree
        .corrections("alxa", |i| i != 2)
        .into_iter()
        .map(|s| s.expr_index)
        .collect();
    assert!(
        filtered.contains(&1) && !filtered.contains(&2),
        "filter not applied: {filtered:?}"
    );
}

#[test]
fn is_deterministic() {
    assert_eq!(
        SV_TREE.corrections("alxa", |_| true),
        SV_TREE.corrections("alxa", |_| true)
    );
}
