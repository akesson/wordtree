use super::data::Info;
use crate::builder::NodeData;

pub struct Node([u8; Self::BYTES_PER_NODE]);

impl Node {
    pub const BYTES_PER_NODE: usize = 8;

    pub fn from_data(node: &NodeData, is_last_sibling: bool) -> Self {
        let mut slf = Self::new();
        slf.set_node_char(node.node_char);
        slf.set_info(node.is_folder, is_last_sibling, node.max_child_percentile);
        slf
    }

    pub fn new() -> Self {
        Self([0; Self::BYTES_PER_NODE])
    }
    pub fn set_first_child_node_pos(&mut self, first_child_pos: u32) {
        let bytes = check_3_bytes(first_child_pos, "first_child_pos");
        self.0[0] = bytes[0];
        self.0[1] = bytes[1];
        self.0[2] = bytes[2];
    }

    pub fn set_node_char(&mut self, node_char: char) {
        let bytes = check_3_bytes(node_char as u32, "node_char");
        self.0[3] = bytes[0];
        self.0[4] = bytes[1];
        self.0[5] = bytes[2];
    }

    pub fn set_info(&mut self, is_folder: bool, is_last_sibling: bool, max_child_percentile: u16) {
        let val = Info::new(is_folder, is_last_sibling, max_child_percentile);
        self.0[6] = val.b1;
        self.0[7] = val.b2;
    }

    #[cfg(test)]
    pub fn write_to(&self, dest: &mut Vec<u8>) {
        dest.extend(self.0)
    }

    pub fn into_inner(self) -> [u8; Self::BYTES_PER_NODE] {
        self.0
    }
}

fn check_3_bytes(val: u32, name: &'static str) -> [u8; 4] {
    let bytes = val.to_le_bytes();
    if bytes[3] != 0 {
        panic!(
            "Invalid {}, shouldn't be bigger than 24 bits, was: {}",
            name, val
        );
    }
    bytes
}

#[cfg(test)]
use super::NodeRef;

/// Reads back only the node-resident fields (own `percentile`/`expr_index` live
/// in the side `values` table, not the node), so empty side arrays suffice.
#[cfg(test)]
fn node_fields(vec: &Vec<u8>, pos: usize) -> (u32, char, bool, bool, u16) {
    let empty: Vec<u8> = Vec::new();
    let cursor = NodeRef::new(vec, &empty, &empty, &empty, pos);
    (
        cursor.first_child_node_pos(),
        cursor.char(),
        cursor.is_folder(),
        cursor.is_last_sibling(),
        cursor.max_child_percentile(),
    )
}

#[cfg(test)]
fn new_node(vals: (u32, char, bool, bool, u16)) -> Node {
    let mut node = Node::new();
    node.set_first_child_node_pos(vals.0);
    node.set_node_char(vals.1);
    node.set_info(vals.2, vals.3, vals.4);
    node
}

#[cfg(test)]
fn roundtrip(vals: (u32, char, bool, bool, u16)) {
    let node = new_node(vals);
    let mut buf = Vec::new();
    node.write_to(&mut buf);

    assert_eq!(node_fields(&buf, 0), vals)
}

#[test]
fn codec() {
    const MAX24B: u32 = 16777215; // 2^24 - 1
    const MAX_PCTL: u16 = 1000;
    // (first_child_pos, node_char, is_folder, is_last_sibling, max_child_percentile)
    roundtrip((0, ' ', false, false, 0));
    roundtrip((MAX24B, 'म', true, true, MAX_PCTL));
    roundtrip((0, ' ', true, true, MAX_PCTL));
}

#[test]
fn codec_w_offset() {
    const MAX24B: u32 = 16777215; // 2^24 - 1
    const MAX_PCTL: u16 = 1000;

    let vals = (MAX24B, '~', true, true, MAX_PCTL);
    let node = new_node(vals);
    let mut buf = vec![12; Node::BYTES_PER_NODE * 2];
    node.write_to(&mut buf);

    assert_eq!(node_fields(&buf, 2), vals)
}
