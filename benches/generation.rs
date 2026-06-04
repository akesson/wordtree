mod data;

use criterion::{Criterion, criterion_group, criterion_main};
use data::read_csv;
use std::time::Duration;
use wordtree::Tree;

pub fn sv_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation");
    group.measurement_time(Duration::from_secs(30));
    let csv = read_csv("benches/data/sv.tsv.zst").unwrap();
    group.bench_function("[sv] tree", |b| b.iter(|| Tree::from_tsv(&csv)));
    group.finish();
}

pub fn en_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation");
    group.measurement_time(Duration::from_secs(50));
    group.sample_size(20);
    let csv = read_csv("benches/data/en.tsv.zst").unwrap();
    group.bench_function("[en] tree", |b| b.iter(|| Tree::from_tsv(&csv)));
    group.finish();
}

criterion_group!(benches, en_tree, sv_tree);
criterion_main!(benches);
