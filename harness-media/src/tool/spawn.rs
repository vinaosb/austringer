//! OS spawn mechanics for the M15 tool executor: the redacted child
//! environment assembly, bounded-output reader threads, the wait-poll
//! deadline with kernel kill-tree enforcement (see
//! [`crate::tool::guard`]), the command-line disclosure join, and the
//! PATH lookup.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use harness_safety::supply;

use crate::tool::guard::ToolTreeGuard;
use crate::tool::manifest::ExternalToolSpec;
use crate::tool::types::{ToolError, ToolRun};

const STDOUT_CAP_BYTES: usize = 1024 * 1024;
const STDERR_HEAD_BYTES: usize = 4096;
const WAIT_POLL: Duration = Duration::from_millis(10);

/// Drives one spawned tool child to exit under its deadline: bounded
/// reader threads collect output while a wait-poll watches the clock;
/// on overrun the kill-tree guard reaps the whole tree before the typed
/// Timeout refusal. Non-zero exits carry the bounded stderr head.
pub(crate) fn wait_bounded(
    child: &mut Child,
    tool: &str,
    timeout: Duration,
    command: &str,
    version_pin: &str,
) -> Result<ToolRun, ToolError> {
    let out_thread = reader_thread(child.stdout.take(), STDOUT_CAP_BYTES);
    let err_thread = reader_thread(child.stderr.take(), STDERR_HEAD_BYTES);
    #[cfg(windows)]
    let mut guard = attach_guard(child, tool)?;
    #[cfg(unix)]
    let mut guard = attach_guard(child)?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(detail) => {
                guard.kill();
                let _ = child.wait();
                return Err(ToolError::Spawn {
                    tool: tool.to_owned(),
                    detail: detail.to_string(),
                });
            }
        }
        if Instant::now() >= deadline {
            // Kernel tree kill first (Job Object close / killpg), then
            // reap: no orphans, and the closed pipes release the readers.
            guard.kill();
            let _ = child.wait();
            return Err(ToolError::Timeout {
                tool: tool.to_owned(),
            });
        }
        std::thread::sleep(WAIT_POLL);
    };
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    let stderr_head = String::from_utf8_lossy(&stderr).into_owned();
    let exit_code = status.code().unwrap_or(-1);
    if exit_code != 0 {
        return Err(ToolError::NonZeroExit {
            tool: tool.to_owned(),
            code: exit_code,
            stderr_head,
        });
    }
    Ok(ToolRun {
        tool: tool.to_owned(),
        version_pin: version_pin.to_owned(),
        exit_code,
        stdout,
        stderr_head,
        command: command.to_owned(),
    })
}

#[cfg(windows)]
fn attach_guard(child: &Child, tool: &str) -> Result<ToolTreeGuard, ToolError> {
    ToolTreeGuard::attach_child(child).map_err(|detail| ToolError::Spawn {
        tool: tool.to_owned(),
        detail: detail.to_string(),
    })
}

#[cfg(unix)]
fn attach_guard(child: &Child) -> Result<ToolTreeGuard, ToolError> {
    Ok(ToolTreeGuard::for_child(child))
}

fn reader_thread<R: Read + Send + 'static>(
    pipe: Option<R>,
    cap: usize,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let Some(pipe) = pipe else {
            return Vec::new();
        };
        let mut buf = Vec::new();
        let _ = pipe.take(cap as u64).read_to_end(&mut buf);
        buf
    })
}

/// The redacted child environment: the caller's map filtered through
/// `DEFAULT_REDACT_PATTERNS` plus the tool's extra denies. This IS the
/// redaction enforcement point (see the tool module docs).
pub(crate) fn redacted_child_env(
    spec: &ExternalToolSpec,
    env: Option<&BTreeMap<String, String>>,
) -> BTreeMap<String, String> {
    let empty = BTreeMap::new();
    let deny: Vec<&str> = supply::DEFAULT_REDACT_PATTERNS
        .iter()
        .chain(spec.extra_env_deny.iter())
        .copied()
        .collect();
    supply::redact_env(env.unwrap_or(&empty), &deny)
}

/// The command-line disclosure string (program + argv, space-joined).
pub(crate) fn argv_line(program: &Path, argv: &[String]) -> String {
    let mut line = program.display().to_string();
    for arg in argv {
        line.push(' ');
        line.push_str(arg);
    }
    line
}

/// Resolves a bare program name against PATH (`.exe` probe on Windows);
/// an explicit path passes through when the file exists.
pub(crate) fn find_program(name: &str) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    let path = std::env::var("PATH").ok()?;
    let extensions: &[&str] = if cfg!(windows) { &["", ".exe"] } else { &[""] };
    for dir in path.split(';') {
        for extension in extensions {
            let program = Path::new(dir).join(format!("{name}{extension}"));
            if program.is_file() {
                return Some(program);
            }
        }
    }
    None
}
