//! Criterion bench: one full tiered-eviction pass over the seeded
//! synthetic history (200 turns, 20k-token window — the gate scenario
//! shape). Input construction stays outside the measured region.

use std::hint::black_box;

use austringer_bench::scenarios::fixtures::{
    FixedCondenser, NoSalience, Rng, bloated_tool_history, tool_returns,
};
use austringer_core::eviction::{
    ActivePlan, DefaultEvictionEngine, EvictionConfig, EvictionDeps, EvictionEngine, EvictionInput,
    VecMemoryHook,
};
use criterion::{Criterion, criterion_group, criterion_main};

pub const PAIRS: u64 = 260;
pub const WINDOW_TOKENS: u64 = 80_000;
pub const SEED: u64 = 2026_0943;

fn evict_pass(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime for bench");
    let mut rng = Rng::new(SEED);
    let history = bloated_tool_history(&mut rng, PAIRS);
    let config = EvictionConfig::default().with_window(WINDOW_TOKENS);
    let input = EvictionInput::new(history.clone())
        .with_plan(ActivePlan {
            text: "1. land perf gate 2. record baselines".to_owned(),
        })
        .with_tool_returns(tool_returns(&history))
        .with_overhead(0);
    let engine = DefaultEvictionEngine::new();
    c.bench_function("eviction/evict_200turns_20k_window", |b| {
        b.iter(|| {
            let mut condenser = FixedCondenser;
            let mut salience = NoSalience;
            let mut memory = VecMemoryHook::default();
            runtime.block_on(engine.evict(
                black_box(&input),
                black_box(&config),
                EvictionDeps {
                    condenser: &mut condenser,
                    salience: &mut salience,
                    memory: &mut memory,
                },
            ))
        })
    });
}

criterion_group!(benches, evict_pass);
criterion_main!(benches);
