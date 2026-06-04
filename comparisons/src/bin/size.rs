//! Size + RAM metric. Builds ONE engine per process (so the global counting
//! allocator is never contaminated) and reports:
//!   - live heap: bytes the structure holds after construction
//!   - peak build: high-water mark during construction (above the loaded list)
//!   - serialized: on-disk / mmappable bytes, where the engine has one
//!
//! Usage: `size <lang> <engine>`  (engine: wordtree|fst|boomphf|hashmap|sorted-vec|symspell|pruning-trie)
//! With no args, prints a markdown table by invoking itself once per engine.

use std::process::Command;

use comparisons::*;

#[global_allocator]
static ALLOC: alloc_counter::Counting = alloc_counter::Counting;

const ENGINES: [&str; 7] = [
    "wordtree",
    "fst",
    "boomphf",
    "hashmap",
    "sorted-vec",
    "symspell",
    "pruning-trie",
];

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match (args.get(1), args.get(2)) {
        (Some(lang), Some(engine)) => measure_one(lang, engine),
        _ => print_table(),
    }
}

/// Spawn `size <lang> <engine>` for every combination and assemble a table.
fn print_table() {
    let exe = std::env::current_exe().expect("current exe");
    println!(
        "| lang | engine | unique words | live heap (MiB) | peak build (MiB) | serialized (MiB) |"
    );
    println!(
        "| ---- | ------ | -----------: | --------------: | ---------------: | ---------------: |"
    );
    for lang in LANGS {
        for engine in ENGINES {
            let out = Command::new(&exe)
                .arg(lang)
                .arg(engine)
                .output()
                .expect("spawn size child");
            if out.status.success() {
                print!("{}", String::from_utf8_lossy(&out.stdout));
            } else {
                eprintln!(
                    "child {lang}/{engine} failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
    }
}

/// Measure a single engine and print one markdown table row.
fn measure_one(lang: &str, engine: &str) {
    let ds = Dataset::load(lang);
    let n = ds.len();

    // Baseline: rows + lookup maps are already allocated; reset the peak so we
    // only capture the build's own high-water mark above that baseline.
    alloc_counter::reset_peak();
    let before = alloc_counter::live();

    // Each arm builds its engine, pins it live with black_box, snapshots the
    // allocator, optionally computes a serialized size, and prints one row.
    match engine {
        "wordtree" => {
            let tree = build_wordtree(&ds.rows);
            std::hint::black_box(&tree);
            let (live, peak) = snapshot(before);
            // rkyv serialized payload == the mmappable bytes.
            let ser = rkyv::to_bytes::<rkyv::rancor::Error>(&tree)
                .expect("rkyv")
                .len();
            row(lang, engine, n, live, peak, Some(ser));
            std::hint::black_box(&tree);
        }
        "fst" => {
            let map = build_fst_map(&ds.rows);
            std::hint::black_box(&map);
            let (live, peak) = snapshot(before);
            // An fst is identical in memory and on disk.
            let ser = map.as_fst().as_bytes().len();
            row(lang, engine, n, live, peak, Some(ser));
            std::hint::black_box(&map);
        }
        "boomphf" => {
            let bp = build_boomphf(&ds.rows);
            std::hint::black_box(&bp);
            let (live, peak) = snapshot(before);
            // BoomHashMap has no compact serialization enabled here; report its
            // live heap (now including the stored keys) only.
            row(lang, engine, n, live, peak, None);
            std::hint::black_box(&bp);
        }
        "hashmap" => {
            let hm = build_hashmap(&ds.rows);
            std::hint::black_box(&hm);
            let (live, peak) = snapshot(before);
            row(lang, engine, n, live, peak, None);
            std::hint::black_box(&hm);
        }
        "sorted-vec" => {
            let sv = build_sorted_vec(&ds.rows);
            std::hint::black_box(&sv);
            let (live, peak) = snapshot(before);
            row(lang, engine, n, live, peak, None);
            std::hint::black_box(&sv);
        }
        "symspell" => {
            let sym = build_symspell(&ds.rows);
            std::hint::black_box(&sym);
            let (live, peak) = snapshot(before);
            row(lang, engine, n, live, peak, None);
            std::hint::black_box(&sym);
        }
        "pruning-trie" => {
            let trie = build_pruning_trie(&ds.rows);
            std::hint::black_box(&trie);
            let (live, peak) = snapshot(before);
            row(lang, engine, n, live, peak, None);
            std::hint::black_box(&trie);
        }
        other => {
            eprintln!("unknown engine {other}");
            std::process::exit(2);
        }
    }
}

fn snapshot(before: usize) -> (usize, usize) {
    (
        alloc_counter::live().saturating_sub(before),
        alloc_counter::peak().saturating_sub(before),
    )
}

fn row(lang: &str, engine: &str, n: usize, live: usize, peak: usize, serialized: Option<usize>) {
    let ser = match serialized {
        Some(b) => format!("{:.2}", mib(b)),
        None => "-".to_string(),
    };
    println!(
        "| {lang} | {engine} | {n} | {:.2} | {:.2} | {ser} |",
        mib(live),
        mib(peak),
    );
}
