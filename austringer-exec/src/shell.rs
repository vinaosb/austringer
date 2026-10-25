//! Windows default-shell resolution (plan todo 20).
//!
//! Resolution order — a product contract, not an implementation detail:
//!
//! 1. [`ShellKind::Pwsh`] — PowerShell 7+ (`pwsh.exe`), searched on PATH
//! 2. [`ShellKind::WindowsPowershell`] — built-in Windows PowerShell 5.1
//!    (`powershell.exe`, absolute System32 fallback)
//! 3. [`ShellKind::Cmd`] — legacy `cmd.exe`, absolute System32 fallback,
//!    the always-present floor
//!
//! An explicit `preferred` (config override) pins exactly one kind and
//! bypasses the order; a pinned-but-missing shell is a typed
//! [`ShellError::NotFound`], never a silent fallback.

use std::path::PathBuf;

/// The resolvable Windows shells, listed in probe order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    /// PowerShell 7+ — first choice.
    Pwsh,
    /// Windows PowerShell 5.1 — second choice.
    WindowsPowershell,
    /// cmd.exe — the always-present floor.
    Cmd,
}

impl ShellKind {
    fn executable(self) -> &'static str {
        match self {
            Self::Pwsh => "pwsh.exe",
            Self::WindowsPowershell => "powershell.exe",
            Self::Cmd => "cmd.exe",
        }
    }
}

/// One resolved shell: its kind and absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedShell {
    /// Which probe tier won.
    pub kind: ShellKind,
    /// Absolute path to the executable.
    pub path: PathBuf,
}

/// Typed resolution failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShellError {
    /// The requested (pinned) shell is not installed.
    #[error("shell {kind:?} not found on this machine")]
    NotFound {
        /// The pinned kind that could not be located.
        kind: ShellKind,
    },
}

/// Locates an executable by name; injectable for hermetic tests.
pub trait ExeFinder {
    /// Returns the first hit for `name` (e.g. `pwsh.exe`), or `None`.
    fn find(&self, name: &str) -> Option<PathBuf>;
}

/// Real-machine finder: PATH scan plus the absolute System32 fallbacks
/// for the shells that ship with the OS.
#[derive(Debug, Default)]
pub struct PathFinder;

impl ExeFinder for PathFinder {
    fn find(&self, name: &str) -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("PATH") {
            for dir in path.to_string_lossy().split(';') {
                let candidate = PathBuf::from(dir).join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        system32().map(|dir| dir.join(name)).filter(|p| p.is_file())
    }
}

fn system32() -> Option<PathBuf> {
    std::env::var_os("SystemRoot").map(|root| PathBuf::from(root).join("System32"))
}

/// Resolves the default shell (or the pinned one when `preferred` is set).
pub fn resolve_default_shell(
    preferred: Option<ShellKind>,
    finder: &dyn ExeFinder,
) -> Result<ResolvedShell, ShellError> {
    let probe_order = match preferred {
        Some(kind) => vec![kind],
        None => vec![
            ShellKind::Pwsh,
            ShellKind::WindowsPowershell,
            ShellKind::Cmd,
        ],
    };
    for kind in probe_order {
        if let Some(path) = finder.find(kind.executable()) {
            return Ok(ResolvedShell { kind, path });
        }
        // cmd.exe is the floor: it resolves from its absolute System32
        // location so even a stripped PATH can always run commands.
        if kind == ShellKind::Cmd
            && let Some(path) = system32().map(|dir| dir.join("cmd.exe"))
            && path.is_file()
        {
            return Ok(ResolvedShell { kind, path });
        }
    }
    Err(match preferred {
        Some(kind) => ShellError::NotFound { kind },
        None => ShellError::NotFound {
            kind: ShellKind::Cmd,
        },
    })
}
