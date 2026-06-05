mod data;

use criterion::{Criterion, criterion_group, criterion_main};
use data::load;
use std::hint::black_box;
use std::time::Duration;
use wordtree::TreeFn;

#[allow(unused_variables)]
pub fn sv_text2suggestions(c: &mut Criterion) {
    let index = load("benches/data/sv.tsv.zst");
    let mut group = c.benchmark_group("suggestions");
    group.measurement_time(Duration::from_secs(7));
    group.bench_function("[sv] Suggestions (2 chars) u_", |b| {
        b.iter(|| index.suggestions(black_box("u_"), |_| true))
    });
    group.bench_function("[sv] Suggestions (14 chars) rekommendat_on", |b| {
        b.iter(|| index.suggestions(black_box("rekommendat_on"), |_| true))
    });
    // Completion-only (no fuzzy walk): clean prefixes.
    group.bench_function("[sv] Complete (2 chars) re", |b| {
        b.iter(|| index.completions(black_box("re"), |_| true))
    });
    group.bench_function("[sv] Complete (11 chars) rekommendat", |b| {
        b.iter(|| index.completions(black_box("rekommendat"), |_| true))
    });
    // Fuzzy-only (the Damerau walk): same typo queries as Suggestions above.
    group.bench_function("[sv] Corrections (2 chars) u_", |b| {
        b.iter(|| index.corrections(black_box("u_"), |_| true))
    });
    group.bench_function("[sv] Corrections (14 chars) rekommendat_on", |b| {
        b.iter(|| index.corrections(black_box("rekommendat_on"), |_| true))
    });
    group.finish();
}

#[allow(unused_variables)]
pub fn en_text2suggestions(c: &mut Criterion) {
    let index = load("benches/data/en.tsv.zst");
    let mut group = c.benchmark_group("suggestions");
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("[en] Suggestions (2 chars) o_", |b| {
        b.iter(|| index.suggestions(black_box("o_"), |_| true))
    });
    group.bench_function("[en] Suggestions (14 chars) alphanumeri_al", |b| {
        b.iter(|| index.suggestions(black_box("alphanumeri_al"), |_| true))
    });
    // Completion-only (no fuzzy walk): clean prefixes.
    group.bench_function("[en] Complete (2 chars) co", |b| {
        b.iter(|| index.completions(black_box("co"), |_| true))
    });
    group.bench_function("[en] Complete (11 chars) alphanumeri", |b| {
        b.iter(|| index.completions(black_box("alphanumeri"), |_| true))
    });
    // Fuzzy-only (the Damerau walk): same typo queries as Suggestions above.
    group.bench_function("[en] Corrections (2 chars) o_", |b| {
        b.iter(|| index.corrections(black_box("o_"), |_| true))
    });
    group.bench_function("[en] Corrections (14 chars) alphanumeri_al", |b| {
        b.iter(|| index.corrections(black_box("alphanumeri_al"), |_| true))
    });
    group.finish();
}

criterion_group!(benches, sv_text2suggestions, en_text2suggestions,);
criterion_main!(benches);
