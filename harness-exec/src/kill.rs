//! Kill trees: kernel-enforced process-tree termination (plan todo 19).
//!
//! Every spawned command lives inside a tree-kill domain, so "no orphaned
//! tool processes after a crash, ever" is structural rather than
//! best-effort:
//!
//! * **Windows** — the child is assigned to a Job Object created with
//!   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` immediately after spawn. Closing
//!   the job handle — deliberately ([`TreeGuard::kill`]), at the end of
//!   the run, or because our own process died — terminates every process
//!   in the job: the child and all descendants. Processes inherit job
//!   membership at creation, so nothing that stays in the tree can
//!   outlive the handle. Two documented gaps: (1) the inherent
//!   microsecond race between `CreateProcessW` returning and our
//!   `AssignProcessToJobObject` call (todo 20's ConPTY supervisor closes
//!   it with suspended-then-resume assignment); (2) grandchildren spawned
//!   via `ShellExecuteEx` (e.g. .NET `Process.Start` with
//!   `UseShellExecute=true`) are created by the shell outside our job and
//!   escape tree kill — the sandbox layer (todo 26) is the enforcement
//!   point for that class, not the job.
//!
//! * **Unix** — the spawn wrapper (see [`crate::runner`]) installs a
//!   `fork`/`exec` hook that calls `prctl(PR_SET_PDEATHSIG, SIGKILL)` and
//!   `setsid()`: the kernel kills the child the moment our process (more
//!   precisely, the spawning thread) disappears, and session leadership
//!   gives timeout kills a process group to `killpg(SIGKILL)` — the whole
//!   tree, synchronously.

use std::io;

/// Owns the tree-kill domain for one spawned command. Dropping the guard
/// terminates anything still alive in the tree (deliberate straggler
/// reaping: a completed command leaves nothing behind).
#[derive(Debug)]
pub struct TreeGuard {
    pid: Option<u32>,
    #[cfg(windows)]
    job: Option<win32job::Job>,
}

impl TreeGuard {
    /// Windows: builds the job domain and attaches the freshly spawned
    /// child. Failing to establish the domain is an error, not a degraded
    /// mode — the no-orphans guarantee is the product.
    #[cfg(windows)]
    pub fn attach_child(child: &dyn portable_pty::Child) -> io::Result<Self> {
        let job = win32job::Job::create().map_err(io::Error::other)?;
        let mut info = job.query_extended_limit_info().map_err(io::Error::other)?;
        info.limit_kill_on_job_close();
        job.set_extended_limit_info(&info)
            .map_err(io::Error::other)?;
        if let Some(handle) = child.as_raw_handle() {
            job.assign_process(handle as isize)
                .map_err(io::Error::other)?;
        }
        Ok(Self {
            pid: child.process_id(),
            job: Some(job),
        })
    }

    /// Unix: the domain is the child's own session (established at spawn
    /// by [`pdeathsig_setup`]); this guard only remembers its id.
    #[cfg(unix)]
    #[must_use]
    pub fn for_pid(pid: u32) -> Self {
        Self { pid: Some(pid) }
    }

    /// The guarded child's pid, when known.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Terminates the entire process tree. Idempotent.
    pub fn kill(&mut self) {
        #[cfg(windows)]
        {
            // Closing the last job handle is the termination signal
            // (KILL_ON_JOB_CLOSE): the kernel reaps the whole tree.
            self.job = None;
        }
        #[cfg(unix)]
        if let Some(pid) = self.pid {
            // Session leader since spawn, so pid == pgid.
            // SAFETY: killpg(2) on our own descendant group; errors
            // (e.g. ESRCH once dead) are irrelevant here.
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }

    /// Pids currently inside the kill domain (empty where the platform
    /// domain has no query surface).
    #[must_use]
    pub fn pids(&self) -> Vec<u32> {
        #[cfg(windows)]
        if let Some(job) = &self.job {
            return job
                .query_process_id_list()
                .map(|pids| pids.into_iter().map(|pid| pid as u32).collect())
                .unwrap_or_default();
        }
        Vec::new()
    }
}

impl Drop for TreeGuard {
    fn drop(&mut self) {
        self.kill();
    }
}

/// The `fork`/`exec` hook giving Unix children the daemon-death guarantee:
/// `PR_SET_PDEATHSIG(SIGKILL)` (kernel reaps the child if we die) and a
/// fresh session (the child becomes a process-group leader, so a timeout
/// kill reaches the whole tree).
///
/// Runs between `fork` and `exec` — async-signal-safe raw syscalls only.
#[cfg(unix)]
pub(crate) fn pdeathsig_setup() -> impl FnMut() -> io::Result<()> + Send + Sync + 'static {
    move || {
        // SAFETY: prctl(2)/setsid(2)/ioctl(2) on the freshly duplicated
        // controlling tty; failure of any of them must not block exec.
        unsafe {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            libc::setsid();
            libc::ioctl(0, libc::TIOCSCTTY, 0);
        }
        Ok(())
    }
}
