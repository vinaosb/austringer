//! Criterion bench: repomap full pipeline (build -> rank -> budget
//! render) over the deterministic synthetic repo. 10k files matches the
//! todo-15 acceptance precedent.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use harness_repomap::{MapOptions, repo_map, synthetic};

fn full_pipeline(c: &mut Criterion) {
    let tables_2k = synthetic::repo(2_000, 43);
    c.bench_function("repomap/repo_map_2k", |b| {
        b.iter(|| repo_map(black_box(tables_2k.clone()), &MapOptions::default()))
    });

    let tables_10k = synthetic::repo(10_000, 42);
    c.bench_function("repomap/repo_map_10k", |b| {
        b.iter(|| repo_map(black_box(tables_10k.clone()), &MapOptions::default()))
    });
}

criterion_group!(benches, full_pipeline);
criterion_main!(benches);
