mod data;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use data::load;
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

criterion_group!(benches, en_word2index,);
criterion_main!(benches);
