//! Layer 2 — the Damerau-Levenshtein walk on small hand-built trees. Each edit
//! kind is exercised at the start, middle and end of a word (the matrix the old
//! `dist4_*` snapshots covered, now asserting the *correction* actually happens),
//! plus structural and unicode edge cases.

use super::{corrections, tree_of};

/// Does querying `query` against a tree of `words` return `target` as a
/// correction (Matching or Spelling)?
fn corrects(words: &[(&str, u16, u32)], query: &str, target: &str) -> bool {
    let tree = tree_of(words);
    corrections(&tree, query).iter().any(|w| w == target)
}

/// Words sharing the "all" prefix, plus a couple of others for end edits.
const WORDS: &[(&str, u16, u32)] = &[
    ("alla", 900, 1),
    ("alle", 800, 2),
    ("avla", 700, 3),
    ("alltid", 600, 4),
    ("atlas", 500, 5),
    ("ko", 400, 6),
];

#[test]
fn substitution_start_middle_end() {
    assert!(corrects(WORDS, "blla", "alla"), "substitute first char");
    assert!(corrects(WORDS, "alxa", "alla"), "substitute middle char");
    assert!(corrects(WORDS, "allx", "alla"), "substitute last char");
}

#[test]
fn deletion_start_middle_end() {
    assert!(corrects(WORDS, "lla", "alla"), "delete first char");
    assert!(
        corrects(WORDS, "ala", "alla"),
        "delete a middle char (the headline bug)"
    );
    assert!(corrects(WORDS, "all", "alla"), "delete last char");
}

#[test]
fn insertion_start_middle_end() {
    assert!(corrects(WORDS, "xalla", "alla"), "insert at start");
    assert!(
        corrects(WORDS, "alxla", "alla"),
        "insert in the middle (was 0% recall)"
    );
    assert!(corrects(WORDS, "allax", "alla"), "insert at end");
}

#[test]
fn transposition_start_middle_end() {
    assert!(corrects(WORDS, "lala", "alla"), "transpose first pair");
    assert!(corrects(WORDS, "altas", "atlas"), "transpose middle pair");
    assert!(corrects(WORDS, "atlsa", "atlas"), "transpose last pair");
}

#[test]
fn exact_match_is_returned() {
    assert!(corrects(WORDS, "alla", "alla"));
}

#[test]
fn no_match_returns_nothing() {
    let tree = tree_of(WORDS);
    assert!(corrections(&tree, "qqqq").is_empty());
}

#[test]
fn multiple_candidates_all_returned_within_cap() {
    let tree = tree_of(&[("alla", 900, 1), ("alga", 800, 2), ("alza", 700, 3)]);
    // "alxa" is distance 1 from all three; the cap for a length-4 typo is 3.
    let got = corrections(&tree, "alxa");
    for w in ["alla", "alga", "alza"] {
        assert!(got.contains(&w.to_string()), "missing {w}: {got:?}");
    }
}

#[test]
fn distance_two_is_excluded() {
    assert!(
        !corrects(WORDS, "alxx", "alla"),
        "two substitutions must not correct"
    );
    assert!(
        !corrects(WORDS, "axlxa", "alla"),
        "two insertions must not correct"
    );
}

#[test]
fn query_longer_than_every_word() {
    let tree = tree_of(&[("ab", 900, 1), ("ko", 800, 2)]);
    assert!(corrections(&tree, "abcdef").is_empty());
}

#[test]
fn query_shorter_than_words() {
    let tree = tree_of(&[("alla", 900, 1)]);
    assert!(
        !corrections(&tree, "al").iter().any(|w| w == "alla"),
        "distance 2 excluded"
    );
    assert!(
        corrections(&tree, "all").iter().any(|w| w == "alla"),
        "distance 1 included"
    );
}

#[test]
fn empty_and_single_char_queries_have_no_spellings() {
    let tree = tree_of(WORDS);
    assert!(super::spelling_count(&tree, "") == 0);
    assert!(super::spelling_count(&tree, "a") == 0);
}

#[test]
fn repeated_letters() {
    let tree = tree_of(&[("aaaa", 900, 1), ("aaab", 800, 2)]);
    assert!(
        corrections(&tree, "aaa").iter().any(|w| w == "aaaa"),
        "delete from a run"
    );
    assert!(
        corrections(&tree, "aaaaa").iter().any(|w| w == "aaaa"),
        "insert into a run"
    );
}

#[test]
fn unicode_substitution_is_one_edit() {
    let tree = tree_of(&[("allé", 900, 1), ("alla", 800, 2)]);
    let got = corrections(&tree, "allx");
    assert!(
        got.contains(&"allé".to_string()),
        "é is one substitution: {got:?}"
    );
    assert!(got.contains(&"alla".to_string()));
}

#[test]
fn unicode_deletion_is_one_edit() {
    // A 4-char query (cap 3) so the single deletion candidate is not capped out.
    let tree = tree_of(&[("förra", 900, 1)]);
    let got = corrections(&tree, "frra");
    assert!(
        got.contains(&"förra".to_string()),
        "deleting ö is one edit: {got:?}"
    );
}

#[test]
fn nfc_normalization_makes_allmanhet_a_substitution() {
    // The Builder NFC-normalizes input; "allmanhet" differs from the stored
    // "allmänhet" by a single ä->a substitution.
    let tree = tree_of(&[("allmänhet", 900, 1)]);
    assert!(
        corrections(&tree, "allmanhet")
            .iter()
            .any(|w| w == "allmänhet")
    );
}
