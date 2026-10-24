//! Platform spawn: PTY creation + guaranteed-tree child spawn.
//!
//! Windows spawns through portable-pty's ConPTY slave ([`CommandBuilder`])
//! and immediately wraps the child in the kill-tree Job Object. Unix owns
//! the `fork`/`exec` path itself (std `Command` over the pty slave
//! device) because the PDEATHSIG guarantee must be installed between
//! fork and exec — portable-pty's `CommandBuilder` has no `pre_exec`
//! seam, so its openpty pair is used for the master side and the child
//! is spawned directly with [`crate::kill::pdeathsig_setup`].
//!
//! Todo 20: WSL-routed specs spawn `wsl.exe --exec <program> <args…>`
//! (resolved in [`crate::wsl::resolve_routing`]) — the Job-Object tree
//! domain then covers wsl.exe and, through WSL's relay semantics, the
//! Linux child it supervises.

use std::io::{self, Read, Write};
use std::path::PathBuf;

#[cfg(windows)]
use portable_pty::CommandBuilder;
use portable_pty::{MasterPty, PtySize};

use crate::kill::TreeGuard;
use crate::spec::{ExecError, ExecSpec};

/// Everything a run needs from one spawned command.
pub(crate) struct SpawnedPty {
    /// Master-side reader (duplicate handle; safe to move to a thread).
    pub reader: Box<dyn Read + Send>,
    /// Master-side writer (terminal-response channel; shared with
    /// teardown so the stdin pipe can be closed while the reader lives).
    pub writer: Box<dyn Write + Send>,
    /// The pty master (closed by the todo-20 ordered teardown).
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

/// The concrete argv one run executes: routing collapsed to words.
pub(crate) struct Command {
    pub program: String,
    pub args: Vec<String>,
}

pub(crate) fn resolve_command(
    spec: &ExecSpec,
    routing: Option<&(PathBuf, Vec<String>)>,
) -> Command {
    match routing {
        Some((exe, argv)) => Command {
            program: exe.to_string_lossy().into_owned(),
            // argv[0] is "--exec": the child words follow it.
            args: argv[1..].to_vec(),
        },
        None => Command {
            program: spec.program.clone(),
            args: spec.args.clone(),
        },
    }
}

fn spawn_error(program: &str, source: impl Into<io::Error>) -> ExecError {
    ExecError::Spawn {
        program: program.to_string(),
        source: source.into(),
    }
}

/// The variables the spec's scrub drops from the child environment:
/// the additions AND the inherited ambient set (the CommandBuilder
/// base is the full ambient), filtered through the policy. Empty when
/// no scrub is attached.
fn scrubbed_names(spec: &ExecSpec) -> Vec<String> {
    let Some(scrub) = &spec.env_scrub else {
        return Vec::new();
    };
    spec.env
        .keys()
        .cloned()
        .chain(std::env::vars().map(|(name, _)| name))
        .filter(|name| scrub.drops(name))
        .collect()
}

/// Windows: ConPTY spawn + Job-Object tree domain.
#[cfg(windows)]
pub(crate) fn spawn_pty(
    spec: &ExecSpec,
    routing: Option<&(PathBuf, Vec<String>)>,
) -> Result<SpawnedPty, ExecError> {
    let command = resolve_command(spec, routing);
    let pair = portable_pty::native_pty_system()
        .openpty(pty_size(spec))
        .map_err(|error| spawn_error(&command.program, io::Error::other(error.to_string())))?;
    let mut builder = CommandBuilder::new(&command.program);
    builder.args(&command.args);
    for (key, value) in &spec.env {
        builder.env(key, value);
    }
    for name in scrubbed_names(spec) {
        builder.env_remove(name);
    }
    if let Some(cwd) = &spec.cwd {
        builder.cwd(cwd);
    }
    let child = pair
        .slave
        .spawn_command(builder)
        .map_err(|error| spawn_error(&command.program, io::Error::other(error.to_string())))?;
    let guard = TreeGuard::attach_child(child.as_ref()).map_err(ExecError::Io)?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| spawn_error(&command.program, io::Error::other(error.to_string())))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| spawn_error(&command.program, io::Error::other(error.to_string())))?;
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
pub(crate) fn spawn_pty(
    spec: &ExecSpec,
    routing: Option<&(PathBuf, Vec<String>)>,
) -> Result<SpawnedPty, ExecError> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::process::CommandExt;

    let command = resolve_command(spec, routing);
    let pair = portable_pty::native_pty_system()
        .openpty(pty_size(spec))
        .map_err(|error| spawn_error(&command.program, io::Error::other(error.to_string())))?;
    let slave_path = pair
        .master
        .tty_name()
        .ok_or_else(|| spawn_error(&command.program, io::Error::other("pty has no slave path")))?;
    let slave = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_path)
        .map_err(|error| spawn_error(&command.program, error))?;
    let mut process = std::process::Command::new(&command.program);
    process.args(&command.args);
    process.stdin(
        slave
            .try_clone()
            .map_err(|error| spawn_error(&command.program, error))?,
    );
    process.stdout(
        slave
            .try_clone()
            .map_err(|error| spawn_error(&command.program, error))?,
    );
    process.stderr(slave);
    for (key, value) in &spec.env {
        process.env(key, value);
    }
    for name in scrubbed_names(spec) {
        process.env_remove(name);
    }
    if let Some(cwd) = &spec.cwd {
        process.current_dir(cwd);
    }
    // SAFETY: the hook uses only async-signal-safe raw syscalls.
    unsafe {
        process.pre_exec(crate::kill::pdeathsig_setup());
    }
    let child = process
        .spawn()
        .map_err(|error| spawn_error(&command.program, error))?;
    let guard = TreeGuard::for_pid(child.id());
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| spawn_error(&command.program, io::Error::other(error.to_string())))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| spawn_error(&command.program, io::Error::other(error.to_string())))?;
    Ok(SpawnedPty {
        reader,
        writer,
        master: pair.master,
        child: PlatformChild::Native(child),
        guard,
    })
}
