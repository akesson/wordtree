pub trait Expression {
    fn has_space(&self) -> bool;
    fn source_count(&self) -> u16;
    fn create_canonical(&self) -> String;
    fn create_compatibility(&self) -> String;
    fn expr_index(&self) -> u32;
}
