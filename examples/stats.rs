//! Regenerate the figures in the README "Information about the data" section
//! from the bundled benchmark word lists.
//!
//! The numbers in the README were taken from the original private dataset; this
//! walks the tree built from `benches/data/*.tsv.zst` and prints the equivalent
//! statistics for the data that actually ships in this repo.
//!
//! Run with:
//!
//! ```sh
//! cargo run --release --example stats
//! ```

use std::fs::File;
use std::io;

use wordtree::{NodeRef, Tree, TreeFn, TsvEntry};

/// Child-count histogram buckets: index `i` counts nodes with exactly `i`
/// children, except the last bucket which is "this many or more".
const CHILD_BUCKETS: usize = 40;
/// Source-count histogram buckets of width 10 (`0-9`, `10-19`, …). The field is
/// capped at 1000 (see `src/trie/data.rs`), so 101 buckets cover every value.
const SOURCE_BUCKETS: usize = 101;

#[derive(Default)]
struct Stats {
    total_nodes: u64,
    total_exprs: u64,
    sum_children: u64,
    max_children: usize,
    max_rel_child_pos: usize,
    max_source_count: u16,
    max_depth: usize,
    child_count_hist: Vec<u64>,
    source_count_hist: Vec<u64>,
}

impl Stats {
    fn new() -> Self {
        Self {
            child_count_hist: vec![0; CHILD_BUCKETS],
            source_count_hist: vec![0; SOURCE_BUCKETS],
            ..Default::default()
        }
    }
}

fn load(path: &str) -> io::Result<Tree> {
    let mut f = File::open(path)?;
    let dec = zstd::Decoder::new(&mut f)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .quoting(false) // words may contain a bare `"`; don't treat it as a field quote
        .from_reader(io::BufReader::new(dec));

    let mut entries = Vec::new();
    for result in rdr.deserialize() {
        let record: TsvEntry = result?;
        entries.push(record);
    }
    Ok(Tree::from_tsv(&entries))
}

/// Number of nodes in a sibling chain (its parent's child count).
fn chain_len(mut node: NodeRef<'_, Vec<u8>>) -> usize {
    let mut n = 1;
    while node.move_to_next_sibling() {
        n += 1;
    }
    n
}

/// Walk every node, depth-first via an explicit stack (the trie can be ~200
/// levels deep, so native recursion is avoided). Each stack entry is the first
/// node of a sibling chain plus its depth.
fn walk(root: NodeRef<'_, Vec<u8>>, stats: &mut Stats) {
    let mut stack = vec![(root, 1usize)];
    while let Some((start, depth)) = stack.pop() {
        stats.max_depth = stats.max_depth.max(depth);
        let mut node = start;
        loop {
            stats.total_nodes += 1;
            if node.expr_index().is_some() {
                stats.total_exprs += 1;
            }

            let source = node.percentile();
            stats.max_source_count = stats.max_source_count.max(source);
            let bucket = (source as usize / 10).min(SOURCE_BUCKETS - 1);
            stats.source_count_hist[bucket] += 1;

            let children = match node.children() {
                Some(first_child) => {
                    let rel = node.first_child_node_pos() as usize - node.pos();
                    stats.max_rel_child_pos = stats.max_rel_child_pos.max(rel);
                    let count = chain_len(first_child.clone());
                    stack.push((first_child, depth + 1));
                    count
                }
                None => 0,
            };
            stats.sum_children += children as u64;
            stats.max_children = stats.max_children.max(children);
            stats.child_count_hist[children.min(CHILD_BUCKETS - 1)] += 1;

            if !node.move_to_next_sibling() {
                break;
            }
        }
    }
}

/// Format a histogram, dropping the run of trailing zeros for readability.
fn histogram(hist: &[u64]) -> String {
    let end = hist.iter().rposition(|&n| n != 0).map_or(0, |i| i + 1);
    let body = hist[..end]
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

fn report(label: &str, path: &str, stats: &Stats) {
    let avg = stats.sum_children as f64 / stats.total_nodes as f64;
    println!("=== {label} ({path}) ===");
    println!("Total nodes:        {}", stats.total_nodes);
    println!("Total exprs:        {}", stats.total_exprs);
    println!(
        "Child count:        avg: {avg:.4}, max: {}",
        stats.max_children
    );
    println!("max_rel_child_pos:  {}", stats.max_rel_child_pos);
    println!("max_source_count:   {}", stats.max_source_count);
    println!("max_depth:          {}", stats.max_depth);
    println!();
    println!(
        "Node child count distribution (index = children, last bucket = {}+):",
        CHILD_BUCKETS - 1
    );
    println!("{}", histogram(&stats.child_count_hist));
    println!();
    println!("Source count distribution (bucket i = source count i*10 .. i*10+9):");
    println!("{}", histogram(&stats.source_count_hist));
    println!();
}

fn main() -> io::Result<()> {
    for (label, path) in [
        ("English", "benches/data/en.tsv.zst"),
        ("Swedish", "benches/data/sv.tsv.zst"),
    ] {
        let tree = load(path)?;
        let mut stats = Stats::new();
        walk(tree.root(), &mut stats);
        report(label, path, &stats);
    }
    Ok(())
}
