use std::ops::Deref;

use super::Node;
use super::data::Info;
use super::rank;

// see node.rs for the node-byte codec tests

#[derive(Debug)]
pub struct NodeRef<'a, V: Deref<Target = [u8]>> {
    vec: &'a V,
    // The side tables (see `super::rank`): `word_bits` marks which nodes are
    // words, `rank_index` answers rank(node) -> slot, `values` holds the per-word
    // (percentile, expr_index). All carried by reference so they propagate
    // through `clone`/`children` without copying.
    word_bits: &'a V,
    rank_index: &'a V,
    values: &'a V,
    // this is the byte position
    // the node stores the node position (byte_pos = node_pos * BYTES_PER_NODE)
    byte_pos: usize,
}

impl<V: Deref<Target = [u8]>> Clone for NodeRef<'_, V> {
    fn clone(&self) -> Self {
        Self {
            vec: self.vec,
            word_bits: self.word_bits,
            rank_index: self.rank_index,
            values: self.values,
            byte_pos: self.byte_pos,
        }
    }
}

impl<'a, V: Deref<Target = [u8]>> NodeRef<'a, V> {
    pub fn new(
        vec: &'a V,
        word_bits: &'a V,
        rank_index: &'a V,
        values: &'a V,
        node_pos: usize,
    ) -> Self {
        Self {
            vec,
            word_bits,
            rank_index,
            values,
            byte_pos: node_pos * Node::BYTES_PER_NODE,
        }
    }

    #[inline]
    pub fn first_child_node_pos(&self) -> u32 {
        u32::from_le_bytes([
            self.vec[self.byte_pos],
            self.vec[self.byte_pos + 1],
            self.vec[self.byte_pos + 2],
            0,
        ])
    }

    #[inline]
    pub fn char(&self) -> char {
        let val = u32::from_le_bytes([
            self.vec[self.byte_pos + 3],
            self.vec[self.byte_pos + 4],
            self.vec[self.byte_pos + 5],
            0,
        ]);
        unsafe { char::from_u32_unchecked(val) }
    }

    #[inline]
    fn info(&self) -> Info {
        Info::from_raw(self.vec[self.byte_pos + 6], self.vec[self.byte_pos + 7])
    }

    /// `true` if this node terminates a word. A single bit probe — no rank query.
    #[inline]
    pub fn is_word(&self) -> bool {
        rank::get_bit(self.word_bits, self.pos())
    }

    /// The `(percentile, expr_index)` of this node's word, or `None` for a
    /// non-word node. Resolving the value costs one rank query, so callers on the
    /// hot path should gate this behind cheaper checks (`is_word`, the edit
    /// distance) and call it once.
    #[inline]
    pub fn word_value(&self) -> Option<(u16, u32)> {
        let i = self.pos();
        if !rank::get_bit(self.word_bits, i) {
            return None;
        }
        let slot = rank::rank(self.word_bits, self.rank_index, i);
        Some(rank::read_value(self.values, slot))
    }

    #[inline]
    pub fn expr_index(&self) -> Option<u32> {
        self.word_value().map(|(_, expr_index)| expr_index)
    }

    #[inline]
    pub fn percentile(&self) -> u16 {
        self.word_value().map_or(0, |(percentile, _)| percentile)
    }

    #[inline]
    pub fn is_folder(&self) -> bool {
        self.info().is_folder()
    }

    #[inline]
    pub fn max_child_percentile(&self) -> u16 {
        self.info().max_child_percentile()
    }

    #[inline]
    pub fn is_last_sibling(&self) -> bool {
        self.info().is_last_sibling()
    }

    #[inline]
    pub(crate) fn first_child_byte_pos(&self) -> usize {
        self.first_child_node_pos() as usize * Node::BYTES_PER_NODE
    }

    #[inline]
    pub fn has_children(&self) -> bool {
        self.first_child_node_pos() != 0
    }

    /// returns true if moved to next sibling,
    /// otherwise false without moving
    #[inline]
    pub fn move_to_next_sibling(&mut self) -> bool {
        if self.is_last_sibling() {
            false
        } else {
            self.byte_pos += Node::BYTES_PER_NODE;
            true
        }
    }

    /// If has children, then return a new NodeRef pointing
    /// to the first child, otherwise returns None
    #[inline]
    pub fn children(&self) -> Option<NodeRef<'a, V>> {
        if self.has_children() {
            Some(NodeRef {
                vec: self.vec,
                word_bits: self.word_bits,
                rank_index: self.rank_index,
                values: self.values,
                byte_pos: self.first_child_byte_pos(),
            })
        } else {
            None
        }
    }

    /// If node has children, then moves to first child
    /// and returns true, otherwise doesn't move and
    /// returns false
    #[inline]
    pub fn move_to_first_child(&mut self) -> bool {
        if self.has_children() {
            self.byte_pos = self.first_child_byte_pos();
            true
        } else {
            false
        }
    }

    /// Moves to the first sibling matching char and returns true
    /// if found, otherwise moves to last sibling and returns false
    #[inline]
    pub fn move_to_sibling_matching(&mut self, chr: char) -> bool {
        loop {
            if self.char() == chr {
                return true;
            }
            if !self.move_to_next_sibling() {
                return false;
            }
        }
    }

    pub fn pos(&self) -> usize {
        self.byte_pos / Node::BYTES_PER_NODE
    }
}

impl<V: Deref<Target = [u8]>> std::fmt::Display for NodeRef<'_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.char())
    }
}
