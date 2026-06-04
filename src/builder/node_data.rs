use nonmax::NonMaxU32;

#[derive(Debug)]
pub struct NodeData {
    pub node_char: char,
    pub expr_index: Option<NonMaxU32>,
    pub percentile: u16,
    pub max_child_percentile: u16,
    /// Node count including children
    pub node_count: u32,
    /// Word count including children
    pub word_count: u32,
    pub is_folder: bool,
}

impl NodeData {
    pub fn root() -> Self {
        Self::new(' ', 0, None)
    }

    pub fn new(node_char: char, percentile: u16, expr_index: Option<NonMaxU32>) -> Self {
        NodeData {
            node_char,
            expr_index,
            percentile,
            max_child_percentile: 0,
            node_count: 0,
            word_count: 0,
            is_folder: false,
        }
    }
}
