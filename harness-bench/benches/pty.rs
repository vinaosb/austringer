//! Criterion bench: todo-19 screen-delta scrape — seeded VT stream
//! through ScreenModel + the 25/50 window law (fully synthetic, no PTY).

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use harness_bench::scenarios::fixtures::Rng;
use harness_bench::scenarios::pty::{SEED, vt_stream};
use harness_exec::screen::ScreenModel;
use harness_exec::window::window_lines;

fn scrape(c: &mut Criterion) {
    let mut rng = Rng::new(SEED);
    let stream = vt_stream(&mut rng);
    c.bench_function("pty/scrape_300lines", |b| {
        b.iter(|| {
            let mut model = ScreenModel::new(120, 40);
            for chunk in black_box(&stream).chunks(4096) {
                model.process(chunk);
            }
            let view = model.finish();
            let lines = view.transcript_lines();
            let total = view.scrolled_total + lines.len() as u64;
            window_lines(&lines, total)
        })
    });
}

criterion_group!(benches, scrape);
criterion_main!(benches);
