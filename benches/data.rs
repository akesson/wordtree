use std::fs::File;
use std::io;
use wordtree::Tree;
use wordtree::TsvEntry;

pub fn read_csv(file: &str) -> std::io::Result<Vec<TsvEntry>> {
    let mut f = File::open(file)?;

    let dec = zstd::Decoder::new(&mut f)?;
    let input = io::BufReader::new(dec);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .quoting(false) // words may contain a bare `"`; don't treat it as a field quote
        .from_reader(input);
    let mut vec = Vec::new();

    for result in rdr.deserialize() {
        let record: TsvEntry = result?;
        vec.push(record);
    }
    Ok(vec)
}

#[allow(dead_code)]
pub fn load(file: &str) -> Tree {
    let csv = read_csv(file).unwrap();
    Tree::from_tsv(&csv)
}
