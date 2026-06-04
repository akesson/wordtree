use super::dft_post::DftPost;
use super::{NodeData, TreeExt};
use crate::Tree as WordTree;
use crate::trie::Node;
use ego_tree::{NodeId, NodeRef, Tree};
use nonmax::NonMaxU32;
use std::cmp::Reverse;
use std::collections::VecDeque;
use unicode_normalization::UnicodeNormalization;

pub struct Builder {
    /// an alphabetically sorted tree
    tree: Tree<NodeData>,
}
impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            tree: Tree::new(NodeData::root()),
        }
    }

    // currently splits on char level (unicode scalar), but might be
    // better to split on grapheme level (visual char, might include diacritics etc)
    // https://crates.io/crates/unicode-segmentation
    pub fn add_word(&mut self, word: &str, percentile: u16, expr_index: u32) {
        let mut current = self.tree.root().id();
        let chars: Vec<char> = match unicode_normalization::is_nfc(word) {
            true => word.chars().collect(),
            false => word.nfc().collect(),
        };
        let last_idx = chars.len() - 1;
        for (idx, word_char) in chars.into_iter().enumerate() {
            current = if idx == last_idx {
                let expr_index = unsafe { NonMaxU32::new_unchecked(expr_index) };
                self.tree
                    .update_or_insert_child(current, word_char, percentile, Some(expr_index))
            } else {
                self.tree
                    .update_or_insert_child(current, word_char, 0, None)
            };
        }
    }

    pub fn add_words(&mut self, words: Vec<(&str, u16, u32)>) {
        for (word, percentile, expr_index) in words {
            self.add_word(word, percentile, expr_index);
        }
    }

    pub fn organize_into_folders(&mut self, folder_size: u32) {
        let df_ids: Vec<NodeId> = DftPost::new(self.tree.root(), |n| n.children())
            .map(|(_, n)| n.id())
            .collect();

        for id in df_ids {
            let mut child_ids = self.tree.child_ids(id);
            let mut total_words = self.set_sizes_from_children(id, &child_ids);

            child_ids.sort_by_key(|i| Reverse(self.tree.val_ref(*i).word_count));

            for child in child_ids {
                let child_word_count = self.tree.val_ref(child).word_count;

                if total_words > folder_size && child_word_count > 1 {
                    self.tree.val_mut(child).is_folder = true;
                    total_words -= child_word_count;
                    total_words += 1;
                } else {
                    self.tree.val_mut(child).is_folder = false;
                }
            }
        }
    }

    fn set_sizes_from_children(&mut self, id: NodeId, child_ids: &[NodeId]) -> u32 {
        let val = self.tree.val_ref(id);
        let mut sum: Counts = child_ids
            .iter()
            .map(|i| Counts::from(self.tree.get(*i).unwrap()))
            .sum();

        sum.word_count += if val.expr_index.is_some() { 1 } else { 0 };
        sum.max_child_percentile = sum.max_child_percentile.max(sum.max_percentile);
        let val = self.tree.val_mut(id);
        val.node_count = sum.node_count + 1;
        val.word_count = sum.word_count;

        val.max_child_percentile = sum.max_child_percentile;
        sum.word_count
    }

    pub fn to_tree(self) -> WordTree {
        let root = self.tree.root();
        let mut arr: Vec<Node> = Vec::with_capacity(root.value().node_count as usize);
        // a fifo with a child vec and an index to where the start pos should be written
        let mut to_process: VecDeque<(NodeRef<NodeData>, usize)> = VecDeque::new();
        to_process.push_back((root.first_child().unwrap(), NO_WRITE));
        while let Some((mut node, start_idx)) = to_process.pop_front() {
            // the start of this child array written to the parent nodes
            if start_idx != NO_WRITE {
                let pos = arr.len();
                arr[start_idx].set_first_child_node_pos((pos) as u32);
            }
            // how many children
            loop {
                if let Some(first_child) = node.first_child() {
                    to_process.push_back((first_child, arr.len()));
                }
                arr.push(Node::from_data(node.value(), node.next_sibling().is_none()));

                if let Some(next) = node.next_sibling() {
                    node = next;
                } else {
                    break;
                }
            }
        }
        let mut vec: Vec<u8> = arr.into_iter().flat_map(|v| v.into_inner()).collect();
        vec.shrink_to_fit();
        WordTree { vec }
    }
}

const NO_WRITE: usize = usize::MAX;

#[derive(Default)]
struct Counts {
    node_count: u32,
    word_count: u32,
    max_percentile: u16,
    max_child_percentile: u16,
}

impl Counts {
    fn from(node: NodeRef<NodeData>) -> Self {
        let val = node.value();
        Self {
            node_count: val.node_count,
            word_count: val.word_count,
            max_percentile: val.percentile,
            max_child_percentile: val.max_child_percentile,
        }
    }
    fn add(&mut self, other: &Counts) {
        self.node_count += other.node_count;
        self.word_count += other.word_count;
        self.max_percentile = self.max_percentile.max(other.max_percentile);
        self.max_child_percentile = self.max_child_percentile.max(other.max_child_percentile);
    }
}

impl std::ops::AddAssign<Counts> for Counts {
    #[inline]
    fn add_assign(&mut self, other: Counts) {
        self.add(&other);
    }
}
impl std::iter::Sum for Counts {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut aggr = Self::default();
        for cnt in iter {
            aggr.add(&cnt)
        }
        aggr
    }
}
