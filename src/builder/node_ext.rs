use super::NodeData;
use ego_tree::{NodeId, NodeRef};
use std::cmp::Ordering::{Equal, Greater};

pub(crate) trait NodeRefExt {
    fn insertion_point_for(&self, node_char: &char) -> InsertionPoint;
}

impl NodeRefExt for NodeRef<'_, NodeData> {
    #[inline]
    fn insertion_point_for(&self, node_char: &char) -> InsertionPoint {
        for child in self.children() {
            match child.value().node_char.cmp(node_char) {
                Equal => return InsertionPoint::Match(child.id()),
                Greater => {
                    return if child.prev_sibling().is_none() {
                        InsertionPoint::First
                    } else {
                        InsertionPoint::Before(child.id())
                    };
                }
                _ => {}
            }
        }
        InsertionPoint::Last
    }
}

pub(crate) enum InsertionPoint {
    Last,
    First,
    Match(NodeId),
    Before(NodeId),
}
