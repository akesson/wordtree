use rkyv::{Archive, Deserialize, Serialize, vec::ArchivedVec};

use crate::{Suggestion, search::NoLedger};

use super::NodeRef;
use std::{fmt, ops::Deref};

#[derive(Archive, Serialize, Deserialize)]
pub struct Tree {
    pub(crate) vec: Vec<u8>,
}

impl TreeFn for Tree {
    type V = Vec<u8>;

    fn root(&self) -> NodeRef<'_, Vec<u8>> {
        NodeRef::new(&self.vec, 0)
    }
}

impl TreeFn for ArchivedTree {
    type V = ArchivedVec<u8>;

    fn root(&self) -> NodeRef<'_, ArchivedVec<u8>> {
        NodeRef::new(&self.vec, 0)
    }
}

pub trait TreeFn {
    type V: Deref<Target = [u8]>;

    fn root(&self) -> NodeRef<'_, Self::V>;

    /// get the path of a word. Used for finding the filesystem path of a word
    /// (for reading wiktionary file)
    fn path_and_content_of(&self, prefix: &str) -> Option<(String, Vec<String>)> {
        self.root().path_and_content_of(prefix)
    }

    /// Find the path of the word. Ex: apple -> /a/p
    fn path_of(&self, word: &str) -> Option<String> {
        self.root().path_of(word)
    }

    fn index_of(&self, word: &str) -> Option<u32> {
        self.root().index_of(word)
    }

    /// Exact match, fuzzy corrections and completions merged into one ranked
    /// list — the combined as-you-type call. Use [`Self::completions`] or
    /// [`Self::corrections`] to pay for only one job.
    fn suggestions<F: Fn(u32) -> bool>(&self, search: &str, is_candidate: F) -> Vec<Suggestion> {
        self.root()
            .suggestions_with_ledger(search, is_candidate, &mut NoLedger::default())
    }

    /// Frequency-ranked completions of `prefix` (plus the exact match when
    /// `prefix` is itself a word). Completion-only: no fuzzy correction is run,
    /// so a query that is not an exact prefix of any word returns nothing. Much
    /// cheaper than [`Self::suggestions`] when only autocomplete is wanted.
    fn completions<F: Fn(u32) -> bool>(&self, prefix: &str, is_candidate: F) -> Vec<Suggestion> {
        self.root()
            .completions_with_ledger(prefix, is_candidate, &mut NoLedger::default())
    }

    /// Fuzzy spelling corrections of `search` within the configured edit
    /// distance (plus the exact match when `search` is itself a word).
    /// Correction-only: no completions are appended.
    fn corrections<F: Fn(u32) -> bool>(&self, search: &str, is_candidate: F) -> Vec<Suggestion> {
        self.root()
            .corrections_with_ledger(search, is_candidate, &mut NoLedger::default())
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut vec: Vec<String> = Vec::new();
        tree_dump("", 0, self.root(), &mut vec);
        write!(f, "{}", vec.join("\n"))
    }
}

fn tree_dump(path: &str, level: usize, childcursor: NodeRef<Vec<u8>>, tree: &mut Vec<String>) {
    let indent: String = " ".repeat(level);
    let mut childcursor = childcursor.clone();
    loop {
        if childcursor.is_folder() {
            tree.push(format!("{}{}:", indent, childcursor.char()));
        }
        let path = crate::add_to_path(path, childcursor.char());
        if let Some(expr_index) = childcursor.expr_index() {
            tree.push(format!(
                "{}{}    (idx: {}, {}% & max: {}%)",
                indent,
                path,
                expr_index,
                childcursor.percentile(),
                childcursor.max_child_percentile(),
            ));
        }
        let level = level + if childcursor.is_folder() { 1 } else { 0 };
        if let Some(children) = childcursor.children() {
            tree_dump(&path, level, children, tree);
        }
        if !childcursor.move_to_next_sibling() {
            break;
        }
    }
}
