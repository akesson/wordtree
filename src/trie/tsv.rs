use crate::{Builder, Tree};
use derive_more::Constructor;
use serde::Deserialize;
use std::fs::File;
use std::io;
use std::path::Path;

impl Tree {
    pub fn read_tsv<P>(file: P) -> Result<Tree, io::Error>
    where
        P: AsRef<Path>,
    {
        Self::from_tsv_reader(
            csv::ReaderBuilder::new()
                .has_headers(false)
                .delimiter(b'\t')
                .from_path(&file)?,
        )
    }

    fn from_tsv_reader(mut rdr: csv::Reader<File>) -> Result<Tree, io::Error> {
        let mut generator = Builder::new();
        for result in rdr.deserialize() {
            let record: Entry = result?;
            generator.add_word(&record.txt, record.percentile, record.expr_index);
        }
        generator.organize_into_folders(100);
        Ok(generator.to_tree())
    }

    pub fn from_tsv(lines: &[Entry]) -> Tree {
        let mut generator = Builder::new();
        for entry in lines {
            generator.add_word(&entry.txt, entry.percentile, entry.expr_index);
        }

        generator.organize_into_folders(100);
        generator.to_tree()
    }
}

#[derive(Debug, Deserialize, Constructor)]
pub struct Entry {
    txt: String,
    percentile: u16,
    expr_index: u32,
}
