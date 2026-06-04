use std::ops::Deref;

use super::Node;
use super::data::TwoBoolTwoU10;
use super::expr::ExprIndex;

// see node.rs for the tests

#[derive(Debug)]
pub struct NodeRef<'a, V: Deref<Target = [u8]>> {
    vec: &'a V,
    // this is the byte position
    // the node stores the node position (byte_pos = node_pos * BYTES_PER_NODE)
    byte_pos: usize,
}

impl<V: Deref<Target = [u8]>> Clone for NodeRef<'_, V> {
    fn clone(&self) -> Self {
        Self {
            vec: self.vec,
            byte_pos: self.byte_pos,
        }
    }
}

impl<'a, V: Deref<Target = [u8]>> NodeRef<'a, V> {
    pub fn new(vec: &'a V, node_pos: usize) -> Self {
        Self {
            vec,
            byte_pos: node_pos * Node::BYTES_PER_NODE,
        }
    }

    pub fn from_arr(vec: &'a V) -> Self {
        Self { vec, byte_pos: 0 }
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
    fn info(&self) -> TwoBoolTwoU10 {
        TwoBoolTwoU10::from_raw(
            self.vec[self.byte_pos + 6],
            self.vec[self.byte_pos + 7],
            self.vec[self.byte_pos + 8],
        )
    }

    #[inline]
    pub fn expr_index(&self) -> Option<u32> {
        ExprIndex::from_raw(u32::from_le_bytes([
            self.vec[self.byte_pos + 9],
            self.vec[self.byte_pos + 10],
            self.vec[self.byte_pos + 11],
            0,
        ]))
        .index()
    }

    #[inline]
    pub fn percentile(&self) -> u16 {
        self.info().get_num1()
    }

    #[inline]
    pub fn is_folder(&self) -> bool {
        self.info().get_bool1()
    }

    #[inline]
    pub fn max_child_percentile(&self) -> u16 {
        self.info().get_num2()
    }

    #[inline]
    pub fn is_last_sibling(&self) -> bool {
        self.info().get_bool2()
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
