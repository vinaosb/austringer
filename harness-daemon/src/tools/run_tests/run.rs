//! The run_tests execution lane (todo 17): the executor-#3 pattern —
//! the todo-4 per-command safety gate applied VERBATIM (policy
//! validation, deny-all egress verdict, requested-vs-enforced report)
//! with the policy's env scrub enforced at this spawn seam. Spawn
//! mechanics mirror the harness-media tool lane (bounded-output
//! headless capture with a kernel kill-tree domain) rather than the
//! PTY screen-delta lane, because the parser needs the runner's FULL
//! stream: a screen grid is bounded by construction (head+tail
//! transcript, 75-line window law) and would silently lose per-test
//! lines on any real suite.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::super::safety::SafetyVerdict;

/// Parser-input cap per stream (media-lane precedent: bounded capture,
/// never an unbounded buffer).
const STREAM_CAP_BYTES: usize = 1024 * 1024;
/// stderr head embedded in typed failure messages (characters; the
/// lossy decode may already differ from raw bytes).
const STDERR_HEAD_CHARS: usize = 4000;
/// Wait-poll granularity for the deadline loop.
const WAIT_POLL: Duration = Duration::from_millis(10);

/// One completed (or killed) run's captured output.
pub(super) struct RunOutcome {
    /// The child's exit code (`None` = abnormal termination with no
    /// code, e.g. a signal death).
    pub(super) exit_code: Option<i32>,
    /// Total wall-clock duration of the run.
    pub(super) elapsed: Duration,
    /// Bounded captured stdout (lossy-decoded).
    pub(super) stdout: String,
    /// Bounded captured stderr (lossy-decoded).
    pub(super) stderr: String,
}

/// Why a run could not produce an outcome.
#[derive(Debug, thiserror::Error)]
pub(super) enum RunError {
    /// The program could not be spawned.
    #[error("spawn `{program}` failed: {detail}")]
    Spawn {
        /// The program that failed.
        program: String,
        /// The underlying error.
        detail: String,
    },
    /// The timeout ceiling fired and the tree was killed.
    #[error("timed out after {timeout:?}")]
    TimedOut {
        /// The ceiling that fired.
        timeout: Duration,
    },
}

/// Runs `program args` in `cwd` under the safety verdict: the policy's
/// env scrub drops KEY/SECRET/TOKEN variables from the inherited
/// environment at this spawn seam, output is bounded-captured on both
/// streams, and the child lives inside a kernel kill-tree domain
/// (Windows Job Object / Unix process group) so a timeout — or a daemon
/// crash — never leaves orphaned test binaries behind.
pub(super) fn run_gated(
    verdict: &SafetyVerdict,
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<RunOutcome, RunError> {
    let ambient: Vec<(String, String)> = std::env::vars().collect();
    let scrubbed = harness_safety::scrub::scrub_env(&ambient, &verdict.policy.env_scrub);
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .envs(scrubbed.kept)
        // CI runners colorize NON-tty runner output, and the output
        // parsers contract on plain text — force color off at this
        // seam rather than trusting the runner's tty detection (the
        // parser additionally strips ANSI as defense-in-depth).
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let started = Instant::now();
    let mut child = command.spawn().map_err(|error| RunError::Spawn {
        program: program.to_owned(),
        detail: error.to_string(),
    })?;
    let out_thread = reader_thread(child.stdout.take());
    let err_thread = reader_thread(child.stderr.take());
    let mut guard = TreeGuard::attach(&child).map_err(|error| RunError::Spawn {
        program: program.to_owned(),
        detail: format!("kill-tree domain: {error}"),
    })?;
    let deadline = started + timeout;
    let exit = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                guard.kill();
                let _ = child.wait();
                return Err(RunError::Spawn {
                    program: program.to_owned(),
                    detail: error.to_string(),
                });
            }
        }
        if Instant::now() >= deadline {
            // Kernel tree kill first, then reap: no orphans, and the
            // closed pipes release the bounded readers.
            guard.kill();
            let _ = child.wait();
            return Err(RunError::TimedOut { timeout });
        }
        std::thread::sleep(WAIT_POLL);
    };
    drop(guard);
    let stdout = String::from_utf8_lossy(&out_thread.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_thread.join().unwrap_or_default()).into_owned();
    Ok(RunOutcome {
        exit_code: exit.code(),
        elapsed: started.elapsed(),
        stdout,
        stderr,
    })
}

/// Bounded reader that DRAINS past the cap (a take()-only reader would
/// block a chatty child on a full pipe and masquerade as a hang).
fn reader_thread<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut scratch = [0u8; 8192];
        let Some(mut pipe) = pipe else {
            return buf;
        };
        loop {
            match pipe.read(&mut scratch) {
                Ok(0) | Err(_) => return buf,
                Ok(n) => {
                    let keep = buf.len();
                    if keep < STREAM_CAP_BYTES {
                        let room = STREAM_CAP_BYTES - keep;
                        buf.extend_from_slice(&scratch[..n.min(room)]);
                    }
                }
            }
        }
    })
}

/// Owns the kill-tree domain for one spawned child (the todo-19
/// TreeGuard pattern re-implemented for a `std::process::Child` — the
/// exec crate's guard takes a PTY child; the media lane owns the same
/// thin copy).
struct TreeGuard {
    #[cfg(unix)]
    pid: u32,
    #[cfg(windows)]
    job: Option<win32job::Job>,
}

impl TreeGuard {
    fn attach(child: &std::process::Child) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let job = win32job::Job::create().map_err(std::io::Error::other)?;
            let mut info = job
                .query_extended_limit_info()
                .map_err(std::io::Error::other)?;
            info.limit_kill_on_job_close();
            job.set_extended_limit_info(&info)
                .map_err(std::io::Error::other)?;
            job.assign_process(child.as_raw_handle() as isize)
                .map_err(std::io::Error::other)?;
            Ok(Self { job: Some(job) })
        }
        #[cfg(unix)]
        {
            // Process-group leader since spawn (process_group(0)), so
            // pid == pgid and killpg reaches the whole tree.
            Ok(Self { pid: child.id() })
        }
    }

    fn kill(&mut self) {
        #[cfg(windows)]
        {
            // Closing the last job handle is the kernel termination
            // signal (KILL_ON_JOB_CLOSE).
            self.job = None;
        }
        #[cfg(unix)]
        {
            // SAFETY: killpg(2) on our own descendant group; ESRCH once
            // dead is irrelevant here.
            unsafe {
                libc::killpg(self.pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
}

impl Drop for TreeGuard {
    fn drop(&mut self) {
        self.kill();
    }
}

/// The stderr head for typed failure messages.
pub(super) fn stderr_head(stderr: &str) -> String {
    stderr
        .chars()
        .take(STDERR_HEAD_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}
