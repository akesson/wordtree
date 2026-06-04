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
