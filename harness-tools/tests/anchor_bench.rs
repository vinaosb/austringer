//! Anchor-scheme benchmark harness (plan todo 16, clause 9 - Grok):
//! read-amplification + shifted-anchor recovery of competing anchor
//! schemes, with a #[ignore] evidence run and deterministic ordering
//! assertions.

#[path = "anchor_bench/bench_schemes.rs"]
mod bench_schemes;

use bench_schemes::{CheckpointChain, ChunkFingerprint, ContentHash, Scheme};
use harness_tools::LineTable;

const BENCH_LINES: usize = 2000;
const TARGET: usize = 1000;

fn bench_source(lines: usize) -> String {
    (1..=lines)
        .map(|n| format!("fn l{n}() {{ let v = {n}; v + {n} * 3 }}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn shifted(base: &str, insertions: usize) -> String {
    // Insertion sits at line 500 — far above the target's ±2 signature
    // window, so the target block itself moves down intact (the plan's
    // "edit above target shifts lines" scenario).
    let noise: Vec<String> = (1..=insertions).map(|n| format!("// noise {n}")).collect();
    let mut lines: Vec<&str> = base.lines().collect();
    let mut spliced: Vec<String> = Vec::with_capacity(lines.len() + insertions);
    for (index, line) in lines.drain(..).enumerate() {
        if index == 499 {
            spliced.extend(noise.iter().cloned());
        }
        spliced.push(line.to_owned());
    }
    spliced.join("\n") + "\n"
}

#[test]
fn anchor_scheme_ordering_is_data_stable() {
    let base = bench_source(BENCH_LINES);
    let base_table = LineTable::from_string(base.clone());
    let schemes: [&dyn Scheme; 3] = [&ContentHash, &CheckpointChain, &ChunkFingerprint];
    for shift in [0usize, 3, 20] {
        let drifted = LineTable::from_string(shifted(&base, shift));
        let mut reads: Vec<(usize, &'static str)> = Vec::new();
        for scheme in schemes {
            let (anchor, _anchor_read) = scheme.anchor(&base_table, TARGET);
            let recovered = scheme.recover(&anchor, &drifted);
            let (line, read) = recovered
                .unwrap_or_else(|| panic!("{} must recover at shift {shift}", scheme.name()));
            assert_eq!(line, TARGET + shift, "{} at shift {shift}", scheme.name());
            reads.push((read, scheme.name()));
        }
        reads.sort();
        assert_eq!(
            reads[0].1, "content-hash",
            "content-hash must be cheapest at shift {shift}: {reads:?}"
        );
    }
}

#[test]
fn large_shift_content_hash_rejects_typed_chunk_pays_full_scan() {
    let base = bench_source(BENCH_LINES);
    let base_table = LineTable::from_string(base.clone());
    let drifted = LineTable::from_string(shifted(&base, 200));
    // Bounded-radius schemes must typed-reject — never first-match.
    for scheme in [&ContentHash as &dyn Scheme, &CheckpointChain] {
        let (anchor, _) = scheme.anchor(&base_table, TARGET);
        assert!(
            scheme.recover(&anchor, &drifted).is_none(),
            "{} must typed-reject a 200-line shift",
            scheme.name()
        );
    }
    // chunk-fingerprint CAN recover (whole-file scan) — only by paying
    // massive read amplification, which is exactly the recorded tradeoff.
    let (anchor, _) = ChunkFingerprint.anchor(&base_table, TARGET);
    let (line, read) = ChunkFingerprint
        .recover(&anchor, &drifted)
        .unwrap_or_else(|| panic!("chunk-fingerprint scans whole file"));
    assert_eq!(line, TARGET + 200);
    assert!(read > 20_000, "recovery must have paid the scan: {read}");
}

#[test]
#[ignore = "benchmark: run with --ignored --nocapture for evidence numbers"]
fn anchor_scheme_benchmark_harness() {
    let base = bench_source(BENCH_LINES);
    let base_table = LineTable::from_string(base.clone());
    println!("scheme             shift  recovered_at  lines_read  us");
    for shift in [0usize, 3, 20, 200] {
        let drifted = LineTable::from_string(shifted(&base, shift));
        for scheme in [
            &ContentHash as &dyn Scheme,
            &CheckpointChain,
            &ChunkFingerprint,
        ] {
            let start = std::time::Instant::now();
            let (anchor, anchor_read) = scheme.anchor(&base_table, TARGET);
            let outcome = scheme.recover(&anchor, &drifted);
            let elapsed = start.elapsed();
            match outcome {
                Some((line, read)) => println!(
                    "{:<18} {shift:>5}  {line:>12}  {read:>10}  {}",
                    scheme.name(),
                    elapsed.as_micros() + anchor_read as u128,
                ),
                None => println!(
                    "{:<18} {shift:>5}  {:>12}  {:>10}  {}",
                    scheme.name(),
                    "rejected",
                    0,
                    elapsed.as_micros() + anchor_read as u128,
                ),
            }
        }
    }
}
