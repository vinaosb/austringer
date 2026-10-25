//! Todo-40 orchestration: fork-join ephemeral subagents and the
//! spec-driven multi-model launcher.
//!
//! One spec decomposes into a task graph (objectives, dependencies,
//! per-task capability + model), resolves through the T34 model-profiles
//! stack (seeded catalog + dial effort-bundles + SET/PROMPT/THROTTLE),
//! launches heterogeneous children as isolated task_id-resumable sessions
//! (`{launch}-child-{task}`, hidden from the main list), and synthesizes
//! the results with a cross-consistency check.
//!
//! Guards, each test-pinned: hard budgets + absolute ceilings (no
//! 50-subagent spawns), spawn pacing (burst + refill), disjoint write
//! sets with FileTouch collision alerts, deny-only permission
//! inheritance with parent-side escalation bubbling, spawn-authorization
//! classes (standing read-only vs fresh for edits; refusals are LOUD),
//! flat depth-1 nesting by default, cache-preserving forks (verbatim
//! parent prompt + tool declarations), family-switch surfacing, and
//! best-of-N capped at four versions with the winner's diff applied.
//!
//! External-harness backends (Claude Code / Codex over their stdio
//! app-server protocols) are opt-in and disabled by default
//! (supply-chain carve-out); release assets verify against pinned
//! SHA-256 digests and child environments are redacted.
//!
//! Daemon integration is additive by construction: team events map onto
//! the frozen protocol v1 `SessionEvent::Notification { kind, message }`
//! string channel, and the journal's `child_started`/`child_completed`
//! vocabulary matches the daemon's durable-workflow step tags. The
//! launcher never shares parent context wholesale — forks carry the
//! verbatim prompt + declarations only, and the clean-context reviewer
//! reasons from the diff alone.

#![deny(missing_docs)]

pub mod authorize;
pub mod backend;
pub mod budget;
pub mod events;
pub mod fork;
pub mod job;
pub mod journal;
pub mod knobs;
pub mod launch;
pub mod phases;
pub mod plan_branch;
pub mod report;
pub mod resolve;
pub mod role;
pub mod run;
pub mod spec;
pub mod synthesis;
pub mod telemetry;
pub mod variants;
pub mod write_set;

pub use launch::LaunchError;
pub use report::{LaunchOutcome, LaunchReport, ResumeError, SpecSource, StopReason};
pub use resolve::{ResolveError, ResolvedPlan};
pub use spec::{LaunchSpec, SpecError, parse_spec};
pub mod resume;
