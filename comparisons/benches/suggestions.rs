//! Job 3 — fuzzy spelling suggestions and prefix autocomplete latency.
//!
//! Fuzzy (edit-distance ≤1): wordtree vs. symspell, fst-Levenshtein, and a
//! brute-force strsim scan (the cost the index saves). Two query shapes per
//! language: a *substitution* typo (every engine corrects it) and a *deletion*
//! typo (wordtree short-circuits — see the correctness tests / REPORT.md), which
//! is why wordtree's suggestion latency is partly a function of the narrower
//! edit space it explores.
//!
//! Autocomplete (prefix top-k): wordtree's extensions vs. pruning_radix_trie.

use std::hint::black_box;
use std::time::Duration;

use comparisons::*;
use criterion::{Criterion, criterion_group, criterion_main};
use wordtree::TreeFn;

/// First generated typo of a given edit kind (deterministic set).
fn pick_typo(ds: &Dataset, kind: EditKind) -> FuzzyQuery {
    fuzzy_query_set(ds, 8)
        .into_iter()
        .find(|q| q.kind == kind)
        .expect("a typo of the requested kind")
}

fn bench_fuzzy(c: &mut Criterion) {
    for lang in LANGS {
        let ds = Dataset::load(lang);
        let tree = build_wordtree(&ds.rows);
        let sym = build_symspell(&ds.rows);
        let map = build_fst_map(&ds.rows);

        let sub = pick_typo(&ds, EditKind::Substitute);
        let del = pick_typo(&ds, EditKind::Delete);

        let mut group = c.benchmark_group(format!("suggest_fuzzy/{lang}"));
        group.measurement_time(Duration::from_secs(4));
        group.sample_size(20);

        for case in [("sub", &sub), ("del", &del)] {
            let (label, q) = (case.0, case.1.text.as_str());
            group.bench_function(format!("wordtree {label} {q}"), |b| {
                b.iter(|| tree.suggestions(black_box(q), |_| true))
            });
            group.bench_function(format!("wordtree-corrections {label} {q}"), |b| {
                b.iter(|| tree.corrections(black_box(q), |_| true))
            });
            group.bench_function(format!("symspell {label} {q}"), |b| {
                b.iter(|| symspell_suggest_words(&sym, black_box(q)))
            });
            group.bench_function(format!("fst-lev {label} {q}"), |b| {
                b.iter(|| fst_suggest_words(&map, &ds, black_box(q)))
            });
            group.bench_function(format!("bruteforce {label} {q}"), |b| {
                b.iter(|| ground_truth_dl1(&ds, black_box(q)))
            });
        }
        group.finish();
    }
}

fn bench_autocomplete(c: &mut Criterion) {
    for lang in LANGS {
        let ds = Dataset::load(lang);
        let tree = build_wordtree(&ds.rows);
        let trie = build_pruning_trie(&ds.rows);

        // A handful of the most common prefixes, not just one, so the
        // completions-vs-pruning ratio rests on a real sample (REPORT §3.3).
        let prefixes = prefix_query_set(&ds, 6);

        let mut group = c.benchmark_group(format!("autocomplete/{lang}"));
        group.measurement_time(Duration::from_secs(3));
        group.sample_size(30);

        for prefix in &prefixes {
            group.bench_function(format!("wordtree {prefix}"), |b| {
                b.iter(|| tree.suggestions(black_box(prefix.as_str()), |_| true))
            });
            group.bench_function(format!("wordtree-complete {prefix}"), |b| {
                b.iter(|| tree.completions(black_box(prefix.as_str()), |_| true))
            });
            group.bench_function(format!("pruning-trie {prefix}"), |b| {
                b.iter(|| pruning_prefix_words(&trie, black_box(prefix.as_str()), 5))
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_fuzzy, bench_autocomplete);
criterion_main!(benches);
