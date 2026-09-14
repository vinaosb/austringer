# Committed perf artifacts (plan todo 43)

- `../budgets.json` — budgets of record (todo 43 figures). Editable via
  `perf-gate --budgets <file>`; validation is strict (typed errors).
- `gate_baseline.json` — machine-independent deterministic counters
  (repomap tokens, eviction projection size, prompt prefix bytes, PTY
  observation bytes, session page bytes). Wall-clock numbers are
  deliberately NOT baselined: they are load-bound, so they live in the
  absolute budgets with min-of-N sampling and wide headroom instead.
- `heap_profile.json` — archived dhat heap profile: per-scenario
  allocation totals (bytes + counts) recorded under the `dhat` feature
  (global allocator build). These counts are the >10% allocation
  regression gate.

## Re-recording procedure

After a DELIBERATE change that legitimately moves these numbers:

```
cargo run --release -p harness-bench --bin perf-gate -- \
  --record-baseline --baseline harness-bench/baselines/gate_baseline.json
cargo run --release -p harness-bench --bin heap-profile --features dhat -- \
  --record harness-bench/baselines/heap_profile.json
```

Cross-platform note: allocation counts can drift across rustc versions
or OS allocators even when the harness code is unchanged; if a fresh
platform/OS trips the gate without a code cause, re-record from that
platform and note it in the commit message.
