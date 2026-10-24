//! Evals + conformance (plan todo 44).
//!
//! A fully hermetic evaluation harness: it scores recorded
//! `harness run --output-json` NDJSON streams (the todo-39 eval-adapter
//! contract) against deterministic graders, guards the scoring surface
//! against gaming, and feeds the todo-42 benchmark routing table. No
//! code path in this crate performs I/O beyond explicit file arguments,
//! and NOTHING in it ever talks to a network or a live model: live-model
//! conformance sweeps are SCHEDULED ONLY (the daemon's todo-41 cron
//! drives them) and are never PR gates — the [`sweep::Trigger`] enum has
//! no PR variant by design.
//!
//! Surfaces:
//! - [`wire`] parses the CLI NDJSON contract (frozen daemon protocol v1
//!   shapes).
//! - [`dataset`] / [`grader`] / [`guard`] — datasets, deterministic
//!   graders, and the integrity guards (each guard is test-pinned).
//! - [`runner`] / [`sweep`] / [`report`] — pair execution, the
//!   model×profile sweep grid, and report artifacts.
//! - [`metrics`] — Pass^k floors (not pass@1), Mean@k, tokens/solved,
//!   failure fingerprints, guard trigger-coverage and leave-one-out
//!   ablation.
//! - [`pinning`] — expected-failures YAML pinning (strict owned subset).
//! - [`benchmark`] — routing-table feeder rows + retraction-in-place.
//! - [`conformance`] — TSS golden-prefix drift gate.
//! - [`retrace`] — replay-from-traces re-scoring.
//! - [`mining`] — commit-to-request eval mining.
//! - [`skill_ab`] — skill A/B mini-benchmarks (todo 34's consumer).
//! - [`adapter`] — the CLI `run --output-json` argv contract.
//! - [`scenarios`] — the committed hermetic scenario pack (todo 26):
//!   mechanism receipts loaded from `harness-evals/scenarios/`.
//! - [`driver`] — the run_sweep conformance-sweep assembly (todo 26).

pub mod adapter;
pub mod benchmark;
pub mod conformance;
pub mod dataset;
pub mod driver;
pub mod grader;
pub mod guard;
pub mod metrics;
pub mod mining;
pub mod pinning;
pub mod report;
pub mod retrace;
pub mod runner;
pub mod scenarios;
pub mod skill_ab;
pub mod sweep;
pub mod wire;
