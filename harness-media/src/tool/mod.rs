//! # External tool executor (plan M15, subprocess tier)
//!
//! The sandboxed subprocess lane behind every
//! [`OpBinding::Subprocess`](crate::executor::OpBinding) entry: opt-in
//! manifests for the seven plan tools (blender, ffmpeg+ffprobe,
//! whisper-cpp, libreoffice, pandoc, freecadcmd, gdal - h5dump dropped,
//! HDF5 deferred by digest section 6), ALL disabled by default, a
//! SHA-256-verified install gate behind an explicit opt-in flag, and a
//! spawn lane with env redaction, a harness-safety sandbox policy gate,
//! timeout, and kernel kill-tree enforcement.
//!
//! ## Where each guarantee is enforced (honesty rule)
//!
//! * **Env redaction** - at THIS command seam: the child environment is
//!   cleared and rebuilt from exactly the
//!   [`redact_env`](harness_safety::supply::redact_env)-filtered caller
//!   map. This is the exec-layer seam the T26 Windows report names for
//!   scrub enforcement: LowBox (AppContainer) spawns on this Windows
//!   build reject custom environments (os error 203), so an
//!   AppContainer spawn CANNOT honor the never-pass-unredacted-env
//!   standing constraint - redaction wins, and this executor never
//!   claims kernel FS/network isolation it did not establish.
//! * **Sandbox policy** - the session-media-temp profile is declared and
//!   fail-closed validated through harness-safety's
//!   [`SandboxPolicy`](harness_safety::policy::decl::SandboxPolicy)
//!   vocabulary; an unsanctionable policy is the typed
//!   [`ToolError::SandboxDenied`] carrying a
//!   [`ViolationEvent`](harness_safety::policy::decl::ViolationEvent)
//!   audit record, and nothing spawns.
//! * **Kill-tree + timeout** - kernel-enforced: a Job Object with
//!   `KILL_ON_JOB_CLOSE` on Windows (the todo-19 TreeGuard pattern),
//!   a process-group kill on Unix; the deadline is enforced by a
//!   wait-poll that kills the tree on overrun - no orphans.
//! * **Bounded output** - stdout is capped, stderr is kept as a bounded
//!   head only; unbounded pipes never reach caller context.
//!
//! ## Hermetic pattern
//!
//! [`ToolOverride`] substitutes the program (and whole argv) per tool -
//! the `mocktool` binary in harness-test-support (T40 mock-child
//! precedent) is the stand-in whose canned outputs are GOLDEN FILES
//! captured from one real tool run (see
//! `harness-test-support/golden/mocktool/capture.md` for the capture
//! commands and the pending-capture honesty table). The M16-M21
//! adapters build their per-op invocations on
//! [`ToolCall`](executor::ToolCall) and report through
//! [`ToolRun::producer`](executor::ToolRun::producer).

pub mod executor;
pub mod guard;
pub mod manifest;
pub(crate) mod spawn;
mod types;

pub use executor::{ToolCall, ToolConfig, ToolExecutor, ToolOverride};
pub use manifest::{ExternalToolSpec, OP_VERSION_CHECK, ToolOpTemplate, manifests};
pub use types::{ToolAvailability, ToolError, ToolRun};

// Shared error vocabulary re-exported so adapter consumers import the
// whole subprocess surface from this module.
pub use crate::error::InstallFailure;
pub use crate::error::MediaError;
