//! Platform spawn: PTY creation + guaranteed-tree child spawn (todo 19).
//!
//! Windows spawns through portable-pty's ConPTY slave ([`CommandBuilder`])
//! and immediately wraps the child in the kill-tree Job Object. Unix owns
//! the `fork`/`exec` path itself (std `Command` over the pty slave
//! device) because the PDEATHSIG guarantee must be installed between
//! fork and exec — portable-pty's `CommandBuilder` has no `pre_exec`
//! seam, so its openpty pair is used for the master side and the child
//! is spawned directly with [`crate::kill::pdeathsig_setup`].

use std::io::{self, Read, Write};

#[cfg(windows)]
use portable_pty::CommandBuilder;
use portable_pty::{MasterPty, PtySize};

use crate::kill::TreeGuard;
use crate::spec::{ExecError, ExecSpec};

/// Everything a run needs from one spawned command.
pub(crate) struct SpawnedPty {
    /// Master-side reader (duplicate handle; safe to move to a thread).
    pub reader: Box<dyn Read + Send>,
    /// Master-side writer (terminal-response channel; moved to the
    /// reader thread, which answers terminal queries).
    pub writer: Box<dyn Write + Send>,
    /// The pty master (kept alive until the reader is finished).
    pub master: Box<dyn MasterPty + Send>,
    /// The child handle.
    pub child: PlatformChild,
    /// The tree-kill domain.
    pub guard: TreeGuard,
}

/// Platform-specific child handle with a uniform wait interface.
pub(crate) enum PlatformChild {
    /// portable-pty child (Windows ConPTY spawn).
    #[cfg(windows)]
    Pty(Box<dyn portable_pty::Child + Send + Sync>),
    /// Directly spawned child (Unix PDEATHSIG path).
    #[cfg(unix)]
    Native(std::process::Child),
}

impl PlatformChild {
    /// Blocks until the child exits; returns its exit code. Signal deaths
    /// (Unix) map to 1 — they only occur for runs we already classify as
    /// killed.
    pub(crate) fn wait(&mut self) -> io::Result<u32> {
        match self {
            #[cfg(windows)]
            PlatformChild::Pty(child) => child.wait().map(|status| status.exit_code()),
            #[cfg(unix)]
            PlatformChild::Native(child) => child
                .wait()
                .map(|status| status.code().map(|code| code as u32).unwrap_or(1)),
        }
    }
}

fn pty_size(spec: &ExecSpec) -> PtySize {
    PtySize {
        rows: spec.size.rows,
        cols: spec.size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn spawn_error(program: &str, source: impl Into<io::Error>) -> ExecError {
    ExecError::Spawn {
        program: program.to_string(),
        source: source.into(),
    }
}

/// Windows: ConPTY spawn + Job-Object tree domain.
#[cfg(windows)]
pub(crate) fn spawn_pty(spec: &ExecSpec) -> Result<SpawnedPty, ExecError> {
    let pair = portable_pty::native_pty_system()
        .openpty(pty_size(spec))
        .map_err(|error| spawn_error(&spec.program, io::Error::other(error.to_string())))?;
    let mut builder = CommandBuilder::new(&spec.program);
    builder.args(&spec.args);
    for (key, value) in &spec.env {
        builder.env(key, value);
    }
    if let Some(cwd) = &spec.cwd {
        builder.cwd(cwd);
    }
    let child = pair
        .slave
        .spawn_command(builder)
        .map_err(|error| spawn_error(&spec.program, io::Error::other(error.to_string())))?;
    let guard = TreeGuard::attach_child(child.as_ref()).map_err(ExecError::Io)?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| spawn_error(&spec.program, io::Error::other(error.to_string())))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| spawn_error(&spec.program, io::Error::other(error.to_string())))?;
    Ok(SpawnedPty {
        reader,
        writer,
        master: pair.master,
        child: PlatformChild::Pty(child),
        guard,
    })
}

/// Unix: portable-pty openpty pair + direct std spawn with PDEATHSIG.
#[cfg(unix)]
pub(crate) fn spawn_pty(spec: &ExecSpec) -> Result<SpawnedPty, ExecError> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::process::CommandExt;

    let pair = portable_pty::native_pty_system()
        .openpty(pty_size(spec))
        .map_err(|error| spawn_error(&spec.program, io::Error::other(error.to_string())))?;
    let slave_path = pair
        .master
        .tty_name()
        .ok_or_else(|| spawn_error(&spec.program, io::Error::other("pty has no slave path")))?;
    let slave = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_path)
        .map_err(|error| spawn_error(&spec.program, error))?;
    let mut command = std::process::Command::new(&spec.program);
    command.args(&spec.args);
    command.stdin(
        slave
            .try_clone()
            .map_err(|error| spawn_error(&spec.program, error))?,
    );
    command.stdout(
        slave
            .try_clone()
            .map_err(|error| spawn_error(&spec.program, error))?,
    );
    command.stderr(slave);
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    // SAFETY: the hook uses only async-signal-safe raw syscalls.
    unsafe {
        command.pre_exec(crate::kill::pdeathsig_setup());
    }
    let child = command
        .spawn()
        .map_err(|error| spawn_error(&spec.program, error))?;
    let guard = TreeGuard::for_pid(child.id());
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| spawn_error(&spec.program, io::Error::other(error.to_string())))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| spawn_error(&spec.program, io::Error::other(error.to_string())))?;
    Ok(SpawnedPty {
        reader,
        writer,
        master: pair.master,
        child: PlatformChild::Native(child),
        guard,
    })
}
