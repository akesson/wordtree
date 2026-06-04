//! Job 2 — exact word -> expr_index lookup latency.
//!
//! wordtree vs. fst, boomphf, and the naive HashMap / sorted-Vec baselines.
//! All engines are built once from the same de-duplicated word list; only the
//! `get` call is measured. Queries mirror the existing wordtree benches
//! (a 2-char and a 14-char word per language).

use std::hint::black_box;

use comparisons::*;
use criterion::{Criterion, criterion_group, criterion_main};
use wordtree::TreeFn;

const QUERIES: [(&str, &str, &str); 2] = [
    // (lang, short word, long word) — same strings as benches/index.rs
    ("en", "on", "alphanumerical"),
    ("sv", "ut", "rekommendation"),
];

fn bench_lookup(c: &mut Criterion) {
    for (lang, short, long) in QUERIES {
        let ds = Dataset::load(lang);
        let tree = build_wordtree(&ds.rows);
        let map = build_fst_map(&ds.rows);
        let hm = build_hashmap(&ds.rows);
        let sv = build_sorted_vec(&ds.rows);
        let bp = build_boomphf(&ds.rows);

        let mut group = c.benchmark_group(format!("lookup/{lang}"));
        for (qlabel, q) in [("short", short), ("long", long)] {
            group.bench_function(format!("wordtree {qlabel} {q}"), |b| {
                b.iter(|| tree.index_of(black_box(q)))
            });
            group.bench_function(format!("fst {qlabel} {q}"), |b| {
                b.iter(|| map.get(black_box(q)))
            });
            group.bench_function(format!("boomphf {qlabel} {q}"), |b| {
                b.iter(|| bp.get(black_box(q)))
            });
            group.bench_function(format!("hashmap {qlabel} {q}"), |b| {
                b.iter(|| hm.get(black_box(q)))
            });
            group.bench_function(format!("sorted-vec {qlabel} {q}"), |b| {
                b.iter(|| sorted_vec_get(&sv, black_box(q)))
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_lookup);
criterion_main!(benches);
