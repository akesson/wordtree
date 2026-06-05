use super::NoLedger;
use crate::trie::TreeFn;
use crate::{Tree, search::StateLedger, tree_as::TreeEntry, trie::tsv::Entry};
use insta::*;
use lazy_static::lazy_static;
use std::collections::HashMap;

lazy_static! {
    static ref SV: (Tree, HashMap<u32, TreeEntry>) = {
        let tree = Tree::read_tsv("src/search/tests/sv_a.tsv").unwrap();
        let lookup = tree.as_lookup();
        (tree, lookup)
    };
    static ref SV_TREE: &'static Tree = &SV.0;
}

mod complete;
mod corrections;
mod invariants;
mod oracle;
mod regression;
mod walk;

use super::suggestion::SuggestionType;

/// Build a small tree from `(word, percentile, expr_index)` triples.
fn tree_of(words: &[(&str, u16, u32)]) -> Tree {
    let mut b = crate::Builder::new();
    b.add_words(words.to_vec());
    b.organize_into_folders(100);
    b.to_tree()
}

/// Every suggestion for `q` as `(kind, word, percentile)`.
fn suggestions_of(tree: &Tree, q: &str) -> Vec<(SuggestionType, String, u16)> {
    let lookup = tree.as_lookup();
    tree.suggestions(q, |_| true)
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

/// Words returned as a correction — `Matching` (exact) or `Spelling` (within
/// edit distance). This is the set the suggestion-quality metric scores.
fn corrections(tree: &Tree, q: &str) -> Vec<String> {
    suggestions_of(tree, q)
        .into_iter()
        .filter(|(k, _, _)| matches!(k, SuggestionType::Matching | SuggestionType::Spelling))
        .map(|(_, w, _)| w)
        .collect()
}

/// Number of `Spelling` corrections returned for `q`.
fn spelling_count(tree: &Tree, q: &str) -> usize {
    suggestions_of(tree, q)
        .into_iter()
        .filter(|(k, _, _)| matches!(k, SuggestionType::Spelling))
        .count()
}

/// The SV fixture rows as `(word, percentile, expr_index)`.
fn sv_rows() -> Vec<(String, u16, u32)> {
    SV.1.iter()
        .map(|(idx, e)| (e.path.clone(), e.percentile, *idx))
        .collect()
}

#[test]
fn sv_path_alla() {
    assert_eq!(SV_TREE.path_of("alla"), Some("all/".to_string()));
}

#[test]
fn sv_a_tree() {
    assert_snapshot!(SV.0);
}

#[test]
fn sv_a() {
    assert_snapshot!(suggestions(&SV, "a"));
}

#[test]
fn sv_atl() {
    assert_snapshot!(suggestions(&SV, "atl"));
}

#[test]
fn sv_atla() {
    assert_snapshot!(suggestions(&SV, "atla"));
}

#[test]
fn sv_alla() {
    assert_snapshot!(suggestions(&SV, "alla"));
}

#[test]
fn sv_blla_wrong_first_letter() {
    assert_snapshot!(suggestions(&SV, "blla"));
}

#[test]
fn sv_allman() {
    assert_snapshot!(suggestions(&SV, "allmän"));
}

#[test]
fn sv_allmanhet() {
    assert_snapshot!(suggestions(&SV, "allmanhet"));
}

#[test]
fn sv_afrikan() {
    assert_eq!(SV_TREE.path_of("afrikan"), Some("af/".to_string()));
    assert_eq!(SV_TREE.index_of("afrikan"), Some(2801));
}

fn suggestions(lang: &(Tree, HashMap<u32, TreeEntry>), search: &str) -> String {
    let (arr, lookup) = lang;
    let mut ledger = NoLedger::default();
    arr.root()
        .suggestions_with_ledger(search, |_| true, &mut ledger)
        .iter()
        .map(|s| format!("{}    {}", s, lookup[&s.expr_index].path,))
        .collect::<Vec<String>>()
        .join("\n")
}

#[test]
fn test_pla() {
    let tree = Tree::from_tsv(&[
        Entry::new("plan".to_string(), 152, 1),
        Entry::new("plat".to_string(), 324, 2),
        Entry::new("plate".to_string(), 406, 3),
        Entry::new("plain".to_string(), 107, 4),
        Entry::new("pluck".to_string(), 258, 5),
        Entry::new("plastered".to_string(), 209, 6),
    ]);
    let lookup = tree.as_lookup();

    let mut ledger = StateLedger::default();
    let _ = tree
        .root()
        .suggestions_with_ledger("pla", |_| true, &mut ledger);
    let trace = ledger
        .0
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<String>>()
        .join("\n");
    let out = suggestions(&(tree, lookup), "pla").to_string();
    assert_snapshot!(format!("{}\n---\n{}", out, trace));
}

// ---------------------------------------------------------------------------
// Integration: spelling cap, ordering, dedup, suggestion kinds, candidate filter
// ---------------------------------------------------------------------------

#[test]
fn spellings_cap_scales_with_query_length() {
    // Each query below has more distance-1 neighbours (first-character
    // substitutions) than the cap, so the cap is what bounds the result.
    let four = tree_of(&[
        ("axxx", 900, 1),
        ("bxxx", 800, 2),
        ("cxxx", 700, 3),
        ("dxxx", 600, 4),
        ("exxx", 500, 5),
    ]);
    assert_eq!(
        spelling_count(&four, "xxxx"),
        3,
        "len 4, not found -> cap 3"
    );

    let three = tree_of(&[
        ("axx", 900, 1),
        ("bxx", 800, 2),
        ("cxx", 700, 3),
        ("dxx", 600, 4),
    ]);
    assert_eq!(
        spelling_count(&three, "xxx"),
        2,
        "len 3, not found -> cap 2"
    );

    let two = tree_of(&[("ax", 900, 1), ("bx", 800, 2), ("cx", 700, 3)]);
    assert_eq!(spelling_count(&two, "xx"), 1, "len 2 -> cap 1");

    assert_eq!(spelling_count(&two, "x"), 0, "len 1 -> cap 0");
    assert_eq!(spelling_count(&two, ""), 0, "len 0 -> cap 0");
}

#[test]
fn matching_is_always_first_then_descending_percentile() {
    let tree = tree_of(&[("alla", 500, 1), ("avla", 900, 2), ("alle", 800, 3)]);
    let got = suggestions_of(&tree, "alla");
    assert!(
        got[0].0 == SuggestionType::Matching,
        "first must be the match: {got:?}"
    );
    assert_eq!(got[0].1, "alla");
    // everything after the leading Matching is sorted by descending percentile
    let rest: Vec<u16> = got[1..].iter().map(|(_, _, p)| *p).collect();
    let mut sorted = rest.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(
        rest, sorted,
        "non-matching suggestions not sorted desc: {got:?}"
    );
}

#[test]
fn no_duplicate_expr_index() {
    // "alla" is a word with children ("allas" etc.); a child one edit away can be
    // reached both as a Spelling and an Extension — it must appear only once.
    let lookup = &SV.1;
    let seen: Vec<u32> =
        SV.0.suggestions("alla", |_| true)
            .into_iter()
            .map(|s| s.expr_index)
            .collect();
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(seen.len(), unique.len(), "duplicate expr_index in {seen:?}");
    // sanity: the words actually resolve
    assert!(seen.iter().all(|i| lookup.contains_key(i)));
}

#[test]
fn is_candidate_filter_excludes_words() {
    use crate::trie::TreeFn;
    let tree = tree_of(&[("alla", 900, 1), ("alga", 800, 2)]);
    // "alxa" is distance 1 from both alla and alga (middle-char substitution).
    let all: Vec<u32> = tree
        .suggestions("alxa", |_| true)
        .into_iter()
        .map(|s| s.expr_index)
        .collect();
    assert!(
        all.contains(&1) && all.contains(&2),
        "precondition: both candidates: {all:?}"
    );
    // Excluding expr_index 2 must drop alga.
    let filtered: Vec<u32> = tree
        .suggestions("alxa", |idx| idx != 2)
        .into_iter()
        .map(|s| s.expr_index)
        .collect();
    assert!(filtered.contains(&1), "kept candidate missing");
    assert!(
        !filtered.contains(&2),
        "filtered candidate present: {filtered:?}"
    );
}

#[test]
fn suggestion_kinds_match_scenario() {
    // (i) exact word with children -> Matching + Extension
    let kinds = suggestions_of(&SV.0, "allmän")
        .into_iter()
        .map(|(k, _, _)| k)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SuggestionType::Matching));
    assert!(kinds.contains(&SuggestionType::Extension));

    // (iv) typo correcting to a childless word -> Spelling, no Extension
    let tree = tree_of(&[("alla", 900, 1), ("zzzz", 100, 2)]);
    let kinds = suggestions_of(&tree, "alxa")
        .into_iter()
        .map(|(k, _, _)| k)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SuggestionType::Spelling), "{kinds:?}");
    assert!(!kinds.contains(&SuggestionType::Extension), "{kinds:?}");
}

// ---------------------------------------------------------------------------
// Typo-triggered autocomplete (altpaths), the only consumer of dist_search's
// altpaths (exact-prefix autocomplete bypasses it).
// ---------------------------------------------------------------------------

#[test]
fn altpath_autocompletes_from_corrected_stem() {
    use crate::trie::TreeFn;
    assert!(
        SV_TREE.index_of("blla").is_none(),
        "precondition: blla is a typo"
    );
    let got: Vec<String> = suggestions_of(&SV.0, "blla")
        .into_iter()
        .map(|(_, w, _)| w)
        .collect();
    // the direct distance-1 correction
    assert!(
        got.contains(&"alla".to_string()),
        "missing correction alla: {got:?}"
    );
    // autocompletions of the corrected "all" stem
    assert!(
        got.iter().any(|w| w.starts_with("all") && w != "alla"),
        "missing stem autocomplete: {got:?}"
    );
}

#[test]
fn altpath_does_not_flood_generic_high_frequency_words() {
    // "axel"/"april"/"augusti" are the top-frequency 'a' words but are far
    // (distance > 1) from "blla". Seeding altpaths by full-word distance (not the
    // shallow prefix distance) must keep them out.
    let got: Vec<String> = suggestions_of(&SV.0, "blla")
        .into_iter()
        .map(|(_, w, _)| w)
        .collect();
    for generic in ["axel", "april", "augusti"] {
        assert!(
            !got.contains(&generic.to_string()),
            "flooded {generic:?}: {got:?}"
        );
    }
}

#[test]
fn altpath_minitree_corrects_first_letter_then_completes() {
    let tree = tree_of(&[("alpha", 900, 1), ("alpine", 850, 2), ("zebra", 100, 3)]);
    let got: Vec<String> = suggestions_of(&tree, "blpha")
        .into_iter()
        .map(|(_, w, _)| w)
        .collect();
    assert!(
        got.iter().any(|w| w == "alpha" || w == "alpine"),
        "typo-autocomplete from corrected prefix failed: {got:?}"
    );
}

#[test]
fn dl_walk_indel_trace() {
    // A reviewable artifact: the trace for the mid-word deletion "ala" -> "alla"
    // must show the word "alla" reached at dist(1) (the bug was that the old
    // engine scored this 2 and pruned it).
    let tree = tree_of(&[("alla", 99, 1), ("alle", 50, 2)]);
    let mut ledger = StateLedger::default();
    let _ = tree
        .root()
        .suggestions_with_ledger("ala", |_| true, &mut ledger);
    let trace = ledger
        .0
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<String>>()
        .join("\n");
    assert_snapshot!(trace);
}
