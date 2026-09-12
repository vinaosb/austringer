//! PTY pipeline, process supervision, ConPTY hardening, and the WSL2
//! bridge (plan todos 19–20).
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
//! reaps children even if this process crashes. See [`kill`] and the
//! teardown-order contract in [`runner`].
//!
//! # Windows pathologies (todo 20)
//!
//! The three classic ConPTY failure shapes are structurally fixed and
//! pinned by fixtures in `tests/pathologies.rs`: the headless reader
//! hang (ordered teardown EOFs the blocked reader —
//! [`spec::ExecOutcome::reader_joined`] proves the join), the
//! Arc-shutdown assertion on mid-run teardown (kill-tree-first RAII
//! ordering), and the DA1/DSR 60 s query hang (the [`query`] responder
//! answers in milliseconds, measured).
//!
//! # WSL2 bridge
//!
//! [`wsl`] adds OPTIONAL Linux routing: `RunIn::Wsl` resolves through
//! `wsl.exe --exec` with typed in-memory `C:\` ↔ `/mnt/c` mapping, and a
//! WSL-free machine is a typed [`wsl::WslStatus::NoWsl`] — default
//! operation never probes WSL at all. Windows default shells resolve
//! pwsh → powershell → cmd ([`shell`]).
//!
//! # Deferred batch execution
//!
//! [`pending::PendingCommandSet`] accumulates commands for atomic
//! apply-on-approval alongside file edits (Plandex pattern) without ever
//! replacing live execution via [`runner::PtyRunner`].

pub mod cancel;
pub mod fsutil;
pub mod kill;
pub mod pending;
pub mod runner;
pub mod screen;
#[cfg(windows)]
pub mod shell;
pub mod spawn;
pub mod spec;
pub mod spill;
pub mod transcript;
pub mod window;
pub mod wsl;

mod query;

pub use cancel::CancelToken;
pub use pending::{ApplyReport, CommandRunner, PendingCommandSet};
pub use runner::PtyRunner;
pub use screen::{ScreenModel, ScreenView};
pub use spec::{ExecError, ExecOutcome, ExecSpec, ExecStatus, RunIn, ScreenSize};
pub use spill::SpilledOutput;
pub use window::Windowed;
