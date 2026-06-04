mod data;

use criterion::{Criterion, criterion_group, criterion_main};
use data::load;
use std::hint::black_box;
use wordtree::TreeFn;

pub fn sv_word2index(c: &mut Criterion) {
    let index = load("benches/data/sv.tsv.zst");
    c.bench_function("[sv] Index of (2 chars) ut", |b| {
        b.iter(|| index.index_of(black_box("ut")))
    });
    c.bench_function("[sv] Index of (14 chars) rekommendation", |b| {
        b.iter(|| index.index_of(black_box("rekommendation")))
    });
}

pub fn en_word2index(c: &mut Criterion) {
    let index = load("benches/data/en.tsv.zst");
    c.bench_function("[en] Index of (2 chars) on", |b| {
        b.iter(|| index.index_of(black_box("on")))
    });
    c.bench_function("[en] Index of (14 chars) alphanumerical", |b| {
        b.iter(|| index.index_of(black_box("alphanumerical")))
    });
}

criterion_group!(benches, sv_word2index, en_word2index,);
criterion_main!(benches);
