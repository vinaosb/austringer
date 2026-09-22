//! Media format adapter contract types for the harness media wave (plan
//! todo M1).
//!
//! This crate owns the typed vocabulary every format adapter speaks:
//! format identity ([`format::FormatId`], the 34-format CORE slate),
//! capability tables ([`descriptor::FormatDescriptor`] with per-operation
//! `Supported`/`Unsupported(reason)`), extraction results and safety
//! budgets ([`extract`]), generation specs under the DETERMINISM CONTRACT
//! ([`generate`]), and the typed error surface
//! ([`error::MediaError`]). Unsupported operations are typed errors with
//! reasons - never fakes, never silent degradation.
//!
//! # What is deliberately NOT here yet
//!
//! The tiered executor dispatch (M6) is a later todo. The probe registry
//! (M2, magic sniffing) lives in [`probe`]; the structural diff engine
//! and round-trip verifier (M5) live in [`diff`].
//!
//! # Structural diff + round-trip verification (summary)
//!
//! Every format's acceptance gate is the M5 diff engine
//! ([`diff::structural_diff`] over canonical trees, per-format tolerance
//! profiles, empty diff = semantic-equivalence PASS) composed by
//! [`diff::verify_roundtrip`]. Expected trees come from an INDEPENDENT
//! spec->tree projection ([`diff::spec_tree`] at the generic level; one per
//! format as adapters land) - never from the adapter's own extractor run
//! over its own output. See [`diff`] for the full provenance contract and
//! the pinned decisions (non-finite floats, integer/float meeting point,
//! null-vs-missing, unordered leftovers).
//!
//! # Determinism contract (summary)
//!
//! Every writer pins document timestamps to the fixed epoch
//! 2026-01-01T00:00:00Z ([`generate::DETERMINISM_EPOCH`]) unless the spec
//! overrides; byte-determinism across runs is an adapter-contract
//! property that the M25 byte-identical repetition evals rely on. See
//! [`generate`] for the full statement.
//!
//! # Extract safety (summary)
//!
//! Pure extract loops honor a caller-provided wall-clock deadline plus
//! total-size cap ([`extract::ExtractBudgets`]); tripping either is the
//! typed [`error::MediaError::ExtractTooLarge`]. Budgets are
//! fixture-scoped; pathological inputs (10k-page PDFs, 500 MB parquet)
//! must fail typed, never hang. Enforcement lands with the extractors;
//! the contract types are pinned here.
//!
//! # Feature topology
//!
//! Codec families are opt-in Cargo features, all optional, default = none:
//! `image`, `svg`, `gltf`, `pdf`, `audio`, `office`, `avif`, `georaster`,
//! `geovector`, `data`, `text`. The daemon enables a defined `media-core`
//! set: `image`, `svg`, `pdf`, `audio`, `office`, `gltf`, `georaster`,
//! `data`, `text`. `avif` (rav1e's 10-25 MB encoder) and `geovector`
//! (rusqlite's bundled sqlite3.c - the single digest-sanctioned C build)
//! are deliberately NON-DEFAULT so the default tree stays C-free and
//! slim; the M27 honesty matrix discloses feature-gated rows as such.

#![deny(missing_docs)]

pub mod descriptor;
pub mod diff;
pub mod error;
pub mod extract;
pub mod format;
pub mod generate;
pub mod probe;

pub use descriptor::{FormatDescriptor, FormatRegistry, Operation, OpSupport};
pub use diff::{
    spec_tree, structural_diff, verify_roundtrip, RoundTripVerdict, StructuralDiff, Tolerances,
};
pub use error::MediaError;
pub use extract::{ExtractBudgets, Extracted};
pub use format::FormatId;
pub use generate::GenerationSpec;
pub use probe::{probe, Confidence, ProbeResult};
