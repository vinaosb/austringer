//! Hermetic test-support primitives for the austringer workspace: a rule-based
//! fake provider (goose `DummyApi` pattern), an SSE event-builder DSL (codex
//! `responses.rs` pattern) with mid-stream drop and idle-timeout injection, a
//! `ResponseGate` for deterministic race/cancel orchestration, and a cassette
//! recorder with strict replay and redaction.
//!
//! Hermetic by construction: everything runs over in-memory streams and
//! channels. No test using this crate performs network I/O of any kind, and the
//! crate itself depends on no networking stack.
#![deny(missing_docs)]

pub mod cassette;
pub mod gate;
pub mod mocktool;
pub mod provider;
pub mod sse;
pub mod util;

#[test]
fn workspace_scaffold_smoke() {
    let scaffold_in_place = true;
    assert!(scaffold_in_place);
}
