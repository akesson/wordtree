use super::data::TwoBoolTwoU10;
use super::expr::ExprIndex;
use crate::builder::NodeData;

pub struct Node([u8; Self::BYTES_PER_NODE]);

impl Node {
    pub const BYTES_PER_NODE: usize = 12;

    pub fn from_data(node: &NodeData, is_last_sibling: bool) -> Self {
        let mut slf = Self::new();
        slf.set_node_char(node.node_char);
        slf.set_info(
            node.is_folder,
            is_last_sibling,
            node.percentile,
            node.max_child_percentile,
        );

        slf.set_expr_index(ExprIndex::new(node.expr_index.map(|v| v.get())));
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

    pub fn set_info(
        &mut self,
        is_folder: bool,
        is_last_sibling: bool,
        percentile: u16,
        max_child_percentile: u16,
    ) {
        let val = TwoBoolTwoU10::new(is_folder, is_last_sibling, percentile, max_child_percentile);
        self.0[6] = val.b1;
        self.0[7] = val.b2;
        self.0[8] = val.b3;
    }

    pub fn set_expr_index(&mut self, expr_index: ExprIndex) {
        let bytes = check_3_bytes(*expr_index.inner(), "expr_index");
        self.0[9] = bytes[0];
        self.0[10] = bytes[1];
        self.0[11] = bytes[2];
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

#[cfg(test)]
fn new_node(vals: (u32, char, bool, u16, bool, u16, Option<u32>)) -> Node {
    let mut node = Node::new();
    node.set_first_child_node_pos(vals.0);
    node.set_node_char(vals.1);
    node.set_info(vals.2, vals.4, vals.5, vals.3);

    node.set_expr_index(ExprIndex::new(vals.6));
    node
}

#[cfg(test)]
fn node_data(vec: Vec<u8>, pos: usize) -> (u32, char, bool, u16, bool, u16, Option<u32>) {
    let slice = NodeRef::new(&vec, pos);
    (
        slice.first_child_node_pos(),
        slice.char(),
        slice.is_folder(),
        slice.percentile(),
        slice.is_last_sibling(),
        slice.max_child_percentile(),
        slice.expr_index(),
    )
}

#[cfg(test)]
fn roundtrip(vals: (u32, char, bool, u16, bool, u16, Option<u32>)) {
    let node = new_node(vals);
    let mut buf = Vec::new();
    node.write_to(&mut buf);

    assert_eq!(node_data(buf, 0), vals)
}

#[test]
fn codec() {
    const MAX_NODE: u32 = 16777215 / Node::BYTES_PER_NODE as u32; // 2^24 - 1 / BYTES PER NODE
    const MAX24B: u32 = 16777214; // 2^24 - 2
    const MAX_PCTL: u16 = 1000;
    roundtrip((0, ' ', false, 0, false, 0, None));
    roundtrip((MAX_NODE, 'म', true, MAX_PCTL, true, MAX_PCTL, Some(MAX24B)));
    roundtrip((0, ' ', true, MAX_PCTL, true, MAX_PCTL, None));
}

#[test]
fn codec_w_offset() {
    const MAX_NODE: u32 = 16777215 / Node::BYTES_PER_NODE as u32; // 2^24 - 1 / BYTES PER NODE
    const MAX24B: u32 = 16777214; // 2^24 - 2
    const MAX_PCTL: u16 = 1000;

    let vals = (MAX_NODE, '~', true, MAX_PCTL, true, MAX_PCTL, Some(MAX24B));
    let node = new_node(vals);
    let mut buf = vec![12; Node::BYTES_PER_NODE * 2];
    node.write_to(&mut buf);

    assert_eq!(node_data(buf, 2), vals)
}
