//! Kill-tree domain for spawned tool processes (plan M15; the todo-19
//! TreeGuard pattern re-implemented for a `std::process::Child` - the
//! exec crate's guard takes a PTY child and safety must not depend on
//! exec, so media owns its thin copy):
//!
//! * **Windows** - the child joins a Job Object created with
//!   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` immediately after spawn.
//!   Dropping the last job handle (deliberately, at run end, or because
//!   our own process died) terminates the child and every descendant -
//!   processes inherit job membership at creation, so nothing in the
//!   tree outlives the handle.
//! * **Unix** - the child is spawned as a session leader
//!   (`Command::process_group(0)`), so pid == pgid and a kill reaches
//!   the whole group via `killpg`.

use std::process::Child;

/// Owns the tree-kill domain for one spawned tool process. Dropping the
/// guard terminates anything still alive in the tree.
#[derive(Debug)]
pub(crate) struct ToolTreeGuard {
    #[cfg(unix)]
    pid: u32,
    #[cfg(windows)]
    job: Option<win32job::Job>,
}

impl ToolTreeGuard {
    /// Windows: builds the Job Object domain and attaches the freshly
    /// spawned child. Failing to establish the domain is an error, not a
    /// degraded mode - the no-orphans guarantee is the product.
    #[cfg(windows)]
    pub(crate) fn attach_child(child: &Child) -> std::io::Result<Self> {
        use std::io;
        use std::os::windows::io::AsRawHandle;
        let job = win32job::Job::create().map_err(io::Error::other)?;
        let mut info = job.query_extended_limit_info().map_err(io::Error::other)?;
        info.limit_kill_on_job_close();
        job.set_extended_limit_info(&info)
            .map_err(io::Error::other)?;
        job.assign_process(child.as_raw_handle() as isize)
            .map_err(io::Error::other)?;
        Ok(Self { job: Some(job) })
    }

    /// Unix: the domain is the child's own process group (established at
    /// spawn via `process_group(0)`); this guard remembers its id.
    #[cfg(unix)]
    pub(crate) fn for_child(child: &Child) -> Self {
        Self { pid: child.id() }
    }

    /// Terminates the entire process tree. Idempotent.
    pub(crate) fn kill(&mut self) {
        #[cfg(windows)]
        {
            // Closing the last job handle is the kernel termination
            // signal (KILL_ON_JOB_CLOSE): the tree is reaped whole.
            self.job = None;
        }
        #[cfg(unix)]
        {
            // Session leader since spawn, so pid == pgid.
            // SAFETY: killpg(2) on our own descendant group; ESRCH once
            // dead is irrelevant here.
            unsafe {
                libc::killpg(self.pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
}

impl Drop for ToolTreeGuard {
    fn drop(&mut self) {
        self.kill();
    }
}
