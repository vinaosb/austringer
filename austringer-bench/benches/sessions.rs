//! Criterion bench: todo-24 lazy page-in of 100 turns from a synthetic
//! 300-turn session in a temp store.

use std::hint::black_box;

use austringer_sessions::SessionStore;
use criterion::{Criterion, criterion_group, criterion_main};

fn page_in(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir for bench store");
    let store = SessionStore::open(dir.path()).expect("open bench store");
    austringer_bench::scenarios::sessions::build(&store, "bench").expect("build session");
    c.bench_function("sessions/page_in_100_turns", |b| {
        b.iter(|| {
            let handle = store.open_session("bench").expect("open handle");
            handle.turns_range(black_box(150), black_box(249))
        })
    });
}

criterion_group!(benches, page_in);
criterion_main!(benches);
