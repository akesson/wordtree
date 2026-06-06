//! Tests for `completions()` — the completion-only searcher: the exact match
//! (when the query is a word) plus frequency-ranked prefix extensions, with no
//! fuzzy correction. A query that is not an exact prefix returns nothing — the
//! key contrast with the combined `suggestions()`, which falls back to
//! typo-tolerant completion via the distance walk's altpaths.

use super::{SV_TREE, SuggestionType, tree_of};
use crate::Tree;
use crate::trie::TreeFn;

/// Every completion for `q` as `(kind, word, percentile)`.
fn completions_of(tree: &Tree, q: &str) -> Vec<(SuggestionType, String, u16)> {
    let lookup = tree.as_lookup();
    tree.completions(q, |_| true)
        .into_iter()
        .map(|s| {
            let word = lookup
                .get(&s.expr_index)
                .map(|e| e.path.clone())
                .unwrap_or_default();
            (s.kind, word, s.percentile)
        })
        .collect()
}

#[test]
fn completes_an_exact_prefix_with_extensions() {
    let tree = tree_of(&[
        ("apple", 900, 1),
        ("apply", 800, 2),
        ("apricot", 700, 3),
        ("banana", 950, 4),
    ]);
    let got = completions_of(&tree, "app");
    let words: Vec<&str> = got.iter().map(|(_, w, _)| w.as_str()).collect();
    assert!(
        words.contains(&"apple") && words.contains(&"apply"),
        "{got:?}"
    );
    // only words under the "app" prefix — never "apricot" (ap-) or "banana"
    assert!(
        !words.iter().any(|w| *w == "apricot" || *w == "banana"),
        "{got:?}"
    );
    // completion-only -> every result is an Extension, ranked by percentile desc
    assert!(
        got.iter().all(|(k, _, _)| *k == SuggestionType::Extension),
        "{got:?}"
    );
    assert_eq!(
        words,
        vec!["apple", "apply"],
        "not frequency-ranked: {got:?}"
    );
}

#[test]
fn includes_the_exact_word_as_matching() {
    let tree = tree_of(&[("app", 950, 1), ("apple", 900, 2), ("apply", 800, 3)]);
    let got = completions_of(&tree, "app");
    assert_eq!(
        got[0].0,
        SuggestionType::Matching,
        "exact word not first: {got:?}"
    );
    assert_eq!(got[0].1, "app");
    assert!(
        got.iter()
            .any(|(k, w, _)| *k == SuggestionType::Extension && w == "apple"),
        "extensions missing: {got:?}"
    );
}

#[test]
fn low_frequency_exact_word_is_outranked_by_completions() {
    // "co" is a zero-frequency stub word; its continuations are far more common.
    // The exact word must compete by its own frequency (like the rival
    // pruning-radix-trie and the oracle), not be pinned at slot 1 — otherwise it
    // would steal a top-k slot from a real completion. With the 6-slot budget
    // full of higher-frequency words, the stub is evicted entirely.
    let tree = tree_of(&[
        ("co", 0, 1),
        ("cow", 900, 2),
        ("cover", 880, 3),
        ("count", 860, 4),
        ("cost", 840, 5),
        ("code", 820, 6),
        ("cool", 800, 7),
    ]);
    let got = completions_of(&tree, "co");
    let words: Vec<&str> = got.iter().map(|(_, w, _)| w.as_str()).collect();
    assert_eq!(
        words.first(),
        Some(&"cow"),
        "highest-frequency completion must lead, not the typed stub: {got:?}"
    );
    assert!(
        !words.contains(&"co"),
        "zero-frequency exact word should be evicted, not occupy a slot: {got:?}"
    );
    assert!(words.len() <= 6, "completions exceeded the cap: {got:?}");
}

#[test]
fn non_prefix_typo_returns_nothing() {
    // THE design pin: completions() is pure-prefix. The combined suggestions()
    // corrects "blla" -> "alla" (+ stem completions) via altpaths, but
    // completions() must not run the fuzzy walk, so it returns nothing.
    let tree = tree_of(&[("alla", 900, 1), ("alltid", 800, 2)]);
    assert!(
        !tree.suggestions("blla", |_| true).is_empty(),
        "precondition: suggestions() corrects the typo"
    );
    assert!(
        tree.completions("blla", |_| true).is_empty(),
        "completions() must not fuzzy-correct a non-prefix query"
    );
    // and on the real fixture
    assert!(
        SV_TREE.index_of("blla").is_none(),
        "precondition: blla is a typo"
    );
    assert!(SV_TREE.completions("blla", |_| true).is_empty());
}

#[test]
fn total_result_count_is_capped() {
    let tree = tree_of(&[
        ("prea", 900, 1),
        ("preb", 890, 2),
        ("prec", 880, 3),
        ("pred", 870, 4),
        ("pree", 860, 5),
        ("pref", 850, 6),
        ("preg", 840, 7),
        ("preh", 830, 8),
    ]);
    let got = tree.completions("pre", |_| true);
    assert!(
        got.len() <= 6,
        "completions exceeded the cap: {}",
        got.len()
    );
}

#[test]
fn is_candidate_filter_excludes_words() {
    let tree = tree_of(&[("apple", 900, 1), ("apply", 800, 2)]);
    let all: Vec<u32> = tree
        .completions("app", |_| true)
        .into_iter()
        .map(|s| s.expr_index)
        .collect();
    assert!(
        all.contains(&1) && all.contains(&2),
        "precondition: both completions: {all:?}"
    );
    let filtered: Vec<u32> = tree
        .completions("app", |i| i != 2)
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
        SV_TREE.completions("all", |_| true),
        SV_TREE.completions("all", |_| true)
    );
}

#[test]
fn gives_extensions_the_full_budget() {
    // suggestions() spends part of the 6-slot budget on spellings; completions()
    // gives the whole budget to extensions, so it returns at least as many.
    let comp = SV_TREE.completions("all", |_| true);
    let sugg = SV_TREE.suggestions("all", |_| true);
    let comp_ext = comp
        .iter()
        .filter(|s| s.kind == SuggestionType::Extension)
        .count();
    let sugg_ext = sugg
        .iter()
        .filter(|s| s.kind == SuggestionType::Extension)
        .count();
    assert!(
        comp_ext >= sugg_ext,
        "completions extensions {comp_ext} < suggestions extensions {sugg_ext}"
    );
}
