//! PTY pipeline, process supervision, and pre-flight checks (plan todo 19).
//!
//! # Canonical output: the screen-delta model
//!
//! Commands run inside a [`portable_pty`] pseudo-terminal and their byte
//! stream is fed through a vt100-family grid parser: the model never sees
//! raw bytes, only the terminal *screen* those bytes paint — ANSI/SGR
//! styling is gone by construction, carriage-return rewrites collapse to
//! the final line, full-screen TUI redraws surface as the final frame, and
//! every line that scrolls off the top is captured into a bounded
//! transcript (head + tail + count, never the whole scrollback).
//!
//! # Windowed output law
//!
//! Model-facing text is bounded by [`window::window_lines`]: more than 75
//! lines renders as the first 25 + an omission marker + the last 50.
//! Oversized raw streams spill to the content-addressed
//! [`harness_core::tools::SpillStore`] on disk and the outcome carries a
//! recoverable pointer; the raw stream itself is structurally absent from
//! [`spec::ExecOutcome`].
//!
//! # Kill trees and orphan reaping
//!
//! Every spawned command lives inside a kernel-enforced tree kill domain:
//! a Windows Job Object with `KILL_ON_JOB_CLOSE` (closing the job handle —
//! deliberately, on timeout, or because our process died — terminates the
//! whole tree), or on Unix a dedicated process group plus
//! `PR_SET_PDEATHSIG(SIGKILL)` set between `fork` and `exec` so the kernel
//! reaps children even if this process crashes. See [`kill`].
//!
//! # Deferred batch execution
//!
//! [`pending::PendingCommandSet`] accumulates commands for atomic
//! apply-on-approval alongside file edits (Plandex pattern) without ever
//! replacing live execution via [`runner::PtyRunner`].

pub mod kill;
pub mod pending;
pub mod runner;
pub mod screen;
pub mod spawn;
pub mod spec;
pub mod spill;
pub mod transcript;
pub mod window;

pub use pending::{ApplyReport, CommandRunner, PendingCommandSet};
pub use runner::{CancelToken, PtyRunner};
pub use screen::{ScreenModel, ScreenView};
pub use spec::{ExecError, ExecOutcome, ExecSpec, ExecStatus, ScreenSize};
pub use spill::SpilledOutput;
pub use window::Windowed;
