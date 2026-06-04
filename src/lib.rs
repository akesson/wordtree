mod builder;
mod editdist;
mod expression;
mod search;
mod trie;

pub use builder::Builder;
pub use expression::Expression;
pub use search::Suggestion;
pub use trie::tsv::Entry as TsvEntry;
pub use trie::{ArchivedTree, NodeRef, Tree, TreeFn};

pub const NUL: char = '\u{0}';
pub use search::{StateLedger, ledger};
pub use trie::tree_as;

fn add_to_path(path: &str, node_char: char) -> String {
    let mut path = path.to_string();
    if node_char != crate::NUL {
        path.push(node_char);
    }
    path
}
