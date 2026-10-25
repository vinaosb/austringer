//! The per-tool-call inspector chain (plan todo 7).
//!
//! Inspectors run sequentially over every pending tool call BEFORE
//! execution, through the todo-5 [`crate::agent_loop`] `ToolApproval`
//! stage seam. Execution happens only when the whole chain passes;
//! bypassing the chain requires constructing the execution call directly
//! (the [`InspectorChain`] implements the `ToolApproval` trait, and the
//! pipeline's execution stage only runs calls the approval stage kept).
//!
//! Verdict precedence (see [`verdict::Verdict`]):
//!
//! 1. `Deny` (and the repetition `Intervene`) always block and
//!    short-circuit the remaining inspectors — execution is refused with a
//!    typed model-feedback payload, never a crash.
//! 2. An authoritative `Allow` from the permission inspector (rule match
//!    or stored user grant) suppresses `Ask` verdicts raised by later
//!    policy inspectors (e.g. the secret-file guard's default ASK), where
//!    the policy says so.
//! 3. Any surviving `Ask` is routed to the ask policy: headless default
//!    refuses (fail toward the user, never a silent allow); `y` grants
//!    once; `a` records a grant in the permission store.
//!
//! The default chain order is: permission, secret-files, repetition,
//! optional security reviewer (off by default).

pub mod chain;
pub mod glob;
pub mod permission;
pub mod repetition;
pub mod safe_outputs;
pub mod secret_files;
pub mod security;
pub mod store;
pub mod verdict;

pub use chain::{ChainOutcome, InspectorChain, InspectorConfig};
pub use verdict::{CallInspector, Verdict};
