use super::node_ext::InsertionPoint::{Before, First, Last, Match};
use super::{NodeData, NodeRefExt};
use ego_tree::{NodeId, Tree};
use nonmax::NonMaxU32;

pub trait TreeExt {
    /// if the char is found then updates that entry
    /// otherwise inserts a new child at alphabetically sorted position
    fn update_or_insert_child(
        &mut self,
        current: NodeId,
        node_char: char,
        percentile: u16,
        expr_index: Option<NonMaxU32>,
    ) -> NodeId;
}

impl TreeExt for Tree<NodeData> {
    fn update_or_insert_child(
        &mut self,
        id: NodeId,
        node_char: char,
        percentile: u16,
        expr_index: Option<NonMaxU32>,
    ) -> NodeId {
        match self.get_unchecked(id).insertion_point_for(&node_char) {
            First => self
                .get_unchecked_mut(id)
                .prepend(NodeData::new(node_char, percentile, expr_index))
                .id(),
            Last => self
                .get_unchecked_mut(id)
                .append(NodeData::new(node_char, percentile, expr_index))
                .id(),
            Match(child_id) => {
                if expr_index.is_some() {
                    self.val_mut(child_id).expr_index = expr_index;
                }
                child_id
            }
            Before(child_id) => self
                .get_unchecked_mut(child_id)
                .insert_before(NodeData::new(node_char, percentile, expr_index))
                .id(),
        }
    }
}
