//! Criterion bench: T11 prompt-builder render over the seeded synthetic
//! session (prefix-cache-stable composition).

use std::hint::black_box;

use austringer_bench::scenarios::fixtures::{Rng, context_spec, synthetic_history};
use austringer_core::context::PromptBuilder;
use criterion::{Criterion, criterion_group, criterion_main};

pub const HISTORY_EVENTS: u64 = 400;
pub const SEED: u64 = 1109;

fn render_prompt(c: &mut Criterion) {
    let mut rng = Rng::new(SEED);
    let spec = context_spec(synthetic_history(&mut rng, HISTORY_EVENTS));
    c.bench_function("prompt/render_400events", |b| {
        b.iter(|| PromptBuilder::render(black_box(&spec)))
    });
}

criterion_group!(benches, render_prompt);
criterion_main!(benches);
