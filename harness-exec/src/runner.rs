//! The live PTY runner: spawn → screen-delta capture → timeout/cancel
//! kill trees → windowed model text + spill pointer (todo 19), with the
//! todo-20 ConPTY teardown hardening.
//!
//! Each [`PtyRunner::run_with_cancel`] invocation is self-contained: a
//! fresh pseudo-terminal, a fresh [`ScreenModel`] (stale state is
//! structurally impossible), a streaming spill spool, and a
//! [`TreeAndMaster`] guard. The only text that leaves a run is the
//! windowed rendering — see [`crate::spec::ExecOutcome`], which has no
//! raw-bytes field.
//!
//! # Teardown order (the todo-20 Arc-shutdown pathology fix)
//!
//! 1. `TreeGuard::kill` — the Job Object closes (`KILL_ON_JOB_CLOSE`):
//!    the whole child tree is dead before anything else happens.
//! 2. The pty write side drops — conhost's input thread sees stdin EOF.
//! 3. The master drops — `ClosePseudoConsole` tears conhost down, which
//!    closes the pty pipes, which EOFs the (possibly blocked) reader.
//!    Because a wedged conhost could stall `ClosePseudoConsole` itself,
//!    this drop runs on a dedicated thread joined with a timeout: the
//!    run can never hang on teardown, and the Arc/Mutex teardown inside
//!    portable-pty cannot assert against a live child (the tree is
//!    already dead — that ordering is the fix).
//! 4. The waiter and reader threads join with bounded grace. A reader
//!    still blocked past that grace would be detached as a documented
//!    last resort and reported via [`crate::spec::ExecOutcome::reader_joined`].

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use harness_core::tools::{IoCap, SpillStore};

use crate::fsutil::OnedriveStatus;
use crate::query::{answer_queries, lock};
use crate::screen::ScreenModel;
use crate::spawn::spawn_pty;
use crate::spec::{ExecError, ExecOutcome, ExecSpec, ExecStatus, MAX_COMMAND_TIMEOUT};
use crate::spill::RawSpool;
use crate::window::window_lines;
use crate::wsl::{self, Routing};

const POLL: Duration = Duration::from_millis(25);
const KILL_CONFIRM_GRACE: Duration = Duration::from_secs(5);
const READER_GRACE: Duration = Duration::from_secs(3);

pub use crate::cancel::CancelToken;

enum RunExit {
    Completed(u32),
    Killed(ExecStatus),
}

type SharedWriter = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

/// RAII ownership of the kill-tree domain and the pty master, enforcing
/// the todo-20 teardown ORDER structurally — even on an early error
/// return or unwind.
struct TreeAndMaster {
    guard: Option<crate::kill::TreeGuard>,
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
}

impl TreeAndMaster {
    fn kill_tree(&mut self) {
        if let Some(mut guard) = self.guard.take() {
            guard.kill();
        }
    }

    fn close_writer(writer: &SharedWriter) {
        drop(lock(writer).take());
    }

    fn close_master(&mut self) {
        if let Some(master) = self.master.take() {
            let (done, closed) = mpsc::channel::<()>();
            thread::spawn(move || {
                drop(master);
                let _ = done.send(());
            });
            let _ = closed.recv_timeout(READER_GRACE);
        }
    }
}

impl Drop for TreeAndMaster {
    fn drop(&mut self) {
        self.kill_tree();
        self.close_master();
    }
}

/// Runs commands live through the PTY pipeline.
#[derive(Debug, Default)]
pub struct PtyRunner {
    spill_root: Option<PathBuf>,
    spill_onedrive: Option<OnedriveStatus>,
}

impl PtyRunner {
    /// A runner defaulting to a shared temp-dir spill store; embedders
    /// pass a session directory via [`PtyRunner::with_spill_root`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A runner whose oversized outputs persist under `<root>/spill/`
    /// (the harness-core [`SpillStore`] layout).
    #[must_use]
    pub fn with_spill_root(root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = root.into();
        let spill_onedrive = Some(crate::fsutil::onedrive_status(&root));
        Self {
            spill_root: Some(root),
            spill_onedrive,
        }
    }

    /// OneDrive classification of the spill root (warning surface: a
    /// synced root means AV/sync lock contention on spill files —
    /// embedders surface this once per session, not per run).
    #[must_use]
    pub fn spill_root_onedrive(&self) -> OnedriveStatus {
        self.spill_onedrive.unwrap_or(OnedriveStatus::Local)
    }

    /// Runs `spec` to completion (no cancellation).
    pub fn run(&self, spec: &ExecSpec) -> Result<ExecOutcome, ExecError> {
        self.run_with_cancel(spec, &CancelToken::new())
    }

    /// Runs `spec`, observing `cancel` until the child exits or the
    /// timeout ceiling fires.
    pub fn run_with_cancel(
        &self,
        spec: &ExecSpec,
        cancel: &CancelToken,
    ) -> Result<ExecOutcome, ExecError> {
        if spec.timeout.is_zero() || spec.timeout > MAX_COMMAND_TIMEOUT {
            return Err(ExecError::InvalidTimeout {
                requested: spec.timeout,
            });
        }
        let routing = match wsl::resolve_routing(spec, &wsl::SystemProbe) {
            Routing::Native => None,
            Routing::Wsl { exe, argv } => Some((exe, argv)),
            Routing::Unavailable(status) => return Err(ExecError::from(status)),
        };
        let started = Instant::now();
        let root = self
            .spill_root
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("harness-exec-spill"));
        let store = SpillStore::open(&root)?;
        let screen = Arc::new(Mutex::new(Some(ScreenModel::new(
            spec.size.cols,
            spec.size.rows,
        ))));
        let spool = Arc::new(Mutex::new(Some(RawSpool::new(store, IoCap::DEFAULT))));

        let spawned = spawn_pty(spec, routing.as_ref())?;
        let crate::spawn::SpawnedPty {
            reader,
            writer,
            master,
            child,
            guard,
        } = spawned;
        let writer: SharedWriter = Arc::new(Mutex::new(Some(writer)));
        let mut session = TreeAndMaster {
            guard: Some(guard),
            master: Some(master),
        };
        let (exit_tx, exit_rx) = mpsc::channel::<u32>();
        let waiter = {
            let mut child = child;
            thread::spawn(move || {
                // A wait() io failure still reports (as code 1) so the
                // loop can never hang on a silent waiter.
                let code = child.wait().unwrap_or(1);
                let _ = exit_tx.send(code);
            })
        };
        let (drain_tx, drain_rx) = mpsc::channel::<()>();
        let reader = {
            let pump = Pump {
                reader,
                writer: Arc::clone(&writer),
                screen: Arc::clone(&screen),
                spool: Arc::clone(&spool),
            };
            thread::spawn(move || {
                let _ = pump.run();
                let _ = drain_tx.send(());
            })
        };

        let deadline = started + spec.timeout;
        let exit = loop {
            if let Ok(code) = exit_rx.try_recv() {
                break RunExit::Completed(code);
            }
            match exit_rx.recv_timeout(POLL) {
                Ok(code) => break RunExit::Completed(code),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancel.is_cancelled() {
                        break RunExit::Killed(ExecStatus::Cancelled);
                    }
                    if Instant::now() >= deadline {
                        break RunExit::Killed(ExecStatus::TimedOut);
                    }
                }
                // The waiter died without reporting: treat the run as
                // not-completed rather than guessing an exit code.
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break RunExit::Killed(ExecStatus::TimedOut);
                }
            }
        };
        session.kill_tree();
        if matches!(exit, RunExit::Killed(_)) {
            // The tree just died; give the wait syscall a moment.
            let _ = exit_rx.recv_timeout(KILL_CONFIRM_GRACE);
        }
        TreeAndMaster::close_writer(&writer);
        session.close_master();
        let _ = waiter.join();
        let reader_joined = drain_rx.recv_timeout(READER_GRACE).is_ok();
        if reader_joined {
            let _ = reader.join();
        }

        let view = match lock(&screen).take() {
            Some(model) => model.finish(),
            // Unreachable: the model is taken exactly once, here.
            None => ScreenModel::new(spec.size.cols, spec.size.rows).finish(),
        };
        let spill = match lock(&spool).take() {
            Some(spool) => spool.finish()?,
            None => None,
        };
        let total_lines = view.scrolled_total + view.visible.len() as u64;
        let mut model_text = window_lines(&view.transcript_lines(), total_lines).text;
        if let Some(spill) = &spill {
            model_text = format!(
                "[output spilled: {} raw bytes -> spill pointer {}; full stream recoverable]\n{}",
                spill.raw_bytes,
                spill.hash.to_ref(),
                model_text
            );
        }
        let status = match exit {
            RunExit::Completed(code) => ExecStatus::Completed { exit_code: code },
            RunExit::Killed(status) => status,
        };
        Ok(ExecOutcome {
            status,
            model_text,
            spill,
            total_lines,
            elapsed: started.elapsed(),
            reader_joined,
        })
    }
}

/// The reader thread's whole job, as one value: drain the master, feed
/// the screen model and spill spool, and answer terminal queries.
struct Pump {
    reader: Box<dyn Read + Send>,
    writer: SharedWriter,
    screen: Arc<Mutex<Option<ScreenModel>>>,
    spool: Arc<Mutex<Option<RawSpool>>>,
}

impl Pump {
    fn run(mut self) -> io::Result<()> {
        let mut buffer = [0u8; 8192];
        // Longest query is 5 bytes (`ESC [ 0 c`); carry the tail so a
        // query split across reads is still seen.
        let mut carry: Vec<u8> = Vec::with_capacity(8);
        loop {
            match self.reader.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(n) => {
                    let chunk = &buffer[..n];
                    if let Some(model) = lock(&self.screen).as_mut() {
                        model.process(chunk);
                    }
                    if let Some(spool) = lock(&self.spool).as_mut() {
                        spool.write(chunk)?;
                    }
                    carry.extend_from_slice(chunk);
                    if let Some(open) = lock(&self.writer).as_mut() {
                        answer_queries(&mut carry, &mut **open, &self.screen);
                    }
                    let keep = carry.len().saturating_sub(5);
                    carry.drain(..keep);
                }
                Err(_) => return Ok(()),
            }
        }
    }
}
