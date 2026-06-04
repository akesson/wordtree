use super::{NodeRef, Tree, TreeFn};
use std::{collections::HashMap, ops::Deref};

#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub path: String,
    pub is_folder: bool,
    pub expr_index: Option<u32>,
    pub percentile: u16,
    pub max_child_percentile: u16,
}

impl Tree {
    pub fn as_lookup(&self) -> HashMap<u32, TreeEntry> {
        HashMap::from_iter(
            self.as_list(usize::MAX, |_, _| true)
                .into_iter()
                .filter_map(|item| item.expr_index.map(|idx| (idx, item))),
        )
    }

    pub fn as_list<F>(&self, max: usize, filter: F) -> Vec<TreeEntry>
    where
        F: Fn(&NodeRef<Vec<u8>>, &str) -> bool,
    {
        let mut found = Vec::new();
        self.root().rec_list("", max, &mut found, &filter);
        found
    }
}

impl<V: Deref<Target = [u8]>> NodeRef<'_, V> {
    fn rec_list<F>(mut self, path: &str, max: usize, found: &mut Vec<TreeEntry>, filter: &F)
    where
        F: Fn(&NodeRef<V>, &str) -> bool,
    {
        loop {
            let path = format!("{}{}", path, &self.char());
            if filter(&self, &path) {
                found.push(TreeEntry {
                    path: path.clone(),
                    is_folder: self.is_folder(),
                    expr_index: self.expr_index(),
                    percentile: self.percentile(),
                    max_child_percentile: self.max_child_percentile(),
                })
            }

            if let Some(child) = self.children() {
                child.rec_list(&path, max, found, filter);
            }
            if !self.move_to_next_sibling() {
                break;
            }
            if found.len() >= max {
                break;
            }
        }
    }
}
