//! The live PTY runner (plan todo 19): spawn → screen-delta capture →
//! timeout/cancel kill trees → windowed model text + spill pointer.
//!
//! Each [`PtyRunner::run_with_cancel`] invocation is self-contained: a
//! fresh pseudo-terminal, a fresh [`ScreenModel`] (stale state is
//! structurally impossible), a streaming spill spool, and a
//! [`TreeGuard`] whose drop guarantees zero stragglers. The only text
//! that leaves a run is the windowed rendering — see
//! [`crate::spec::ExecOutcome`], which has no raw-bytes field.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use harness_core::tools::{IoCap, SpillStore};

use crate::screen::ScreenModel;
use crate::spawn::spawn_pty;
use crate::spec::{ExecError, ExecOutcome, ExecSpec, ExecStatus, MAX_COMMAND_TIMEOUT};
use crate::spill::RawSpool;
use crate::window::window_lines;

const POLL: Duration = Duration::from_millis(25);
const KILL_CONFIRM_GRACE: Duration = Duration::from_secs(5);
const READER_GRACE: Duration = Duration::from_secs(3);

/// Cooperative cancellation flag shared with a running command.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// A fresh, non-cancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Poisoning only happens after a reader panic; the shared state
    // stays consistent for reading regardless.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

enum RunExit {
    Completed(u32),
    Killed(ExecStatus),
}

/// Runs commands live through the PTY pipeline.
#[derive(Debug, Default)]
pub struct PtyRunner {
    spill_root: Option<PathBuf>,
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
        Self {
            spill_root: Some(root.into()),
        }
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

        let spawned = spawn_pty(spec)?;
        let (exit_tx, exit_rx) = mpsc::channel::<u32>();
        let waiter = {
            let mut child = spawned.child;
            thread::spawn(move || {
                // A wait() io failure still reports (as code 1) so the
                // loop can never hang on a silent waiter.
                let code = child.wait().unwrap_or(1);
                let _ = exit_tx.send(code);
            })
        };
        let (drain_tx, drain_rx) = mpsc::channel::<()>();
        let reader = {
            let screen = Arc::clone(&screen);
            let spool = Arc::clone(&spool);
            let pump = Pump {
                reader: spawned.reader,
                writer: spawned.writer,
                screen,
                spool,
            };
            thread::spawn(move || {
                let _ = pump.run();
                let _ = drain_tx.send(());
            })
        };

        let deadline = started + spec.timeout;
        let mut guard = spawned.guard;
        let exit = loop {
            if let Ok(code) = exit_rx.try_recv() {
                break RunExit::Completed(code);
            }
            match exit_rx.recv_timeout(POLL) {
                Ok(code) => break RunExit::Completed(code),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancel.is_cancelled() {
                        guard.kill();
                        break RunExit::Killed(ExecStatus::Cancelled);
                    }
                    if Instant::now() >= deadline {
                        guard.kill();
                        break RunExit::Killed(ExecStatus::TimedOut);
                    }
                }
                // The waiter died without reporting: treat the run as
                // not-completed rather than guessing an exit code.
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    guard.kill();
                    break RunExit::Killed(ExecStatus::TimedOut);
                }
            }
        };
        if matches!(exit, RunExit::Killed(_)) {
            // The tree is dead; give the wait syscall a moment to return.
            let _ = exit_rx.recv_timeout(KILL_CONFIRM_GRACE);
        }
        // Straggler reaping: a completed command leaves nothing behind.
        guard.kill();
        drop(guard);
        let _ = waiter.join();
        let reader_done = drain_rx.recv_timeout(READER_GRACE).is_ok();
        if reader_done {
            let _ = reader.join();
        }
        // A still-blocked reader is the known ConPTY hang pathology
        // (todo 20): shared state is complete without it, so detach.
        drop(spawned.master);

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
        })
    }
}

/// The reader thread's whole job, as one value: drain the master, feed
/// the screen model and spill spool, and answer terminal queries.
struct Pump {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
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
                    answer_queries(&mut carry, &mut self.writer, &self.screen);
                    let keep = carry.len().saturating_sub(5);
                    carry.drain(..keep);
                }
                Err(_) => return Ok(()),
            }
        }
    }
}

/// The headless terminal's side of the terminal query contract.
///
/// ConPTY (and terminal-aware children like vim) send queries — cursor
/// position (`ESC [ 6 n`), primary device attributes (`ESC [ c`) — and
/// BLOCK until a response arrives; on Windows conhost's handshake can
/// stall the whole child for ~60 s per unanswered query. A real terminal
/// answers; this runner is the terminal, so queries are answered
/// immediately (cursor reports come straight from the screen model).
/// This is the todo-20 "proactive injection" pathology fix, applied
/// where the byte stream is already in hand.
///
/// Answers every query found in `stream` (leaving the unconsumed tail in
/// place for split-read queries).
fn answer_queries(
    stream: &mut Vec<u8>,
    writer: &mut dyn Write,
    screen: &Mutex<Option<ScreenModel>>,
) {
    let mut consumed = 0;
    while let Some((offset, kind)) = find_query(&stream[consumed..]) {
        consumed += offset + query_len(&kind);
        let response = match kind {
            // DSR cursor position: report the model's tracked cursor.
            Query::CursorPosition => {
                let (row, col) = lock(screen).as_ref().map_or((0, 0), ScreenModel::cursor);
                format!("\x1b[{};{}R", row + 1, col + 1).into_bytes()
            }
            // DA1: "VT100 with AVO" — the minimal honest identity.
            Query::DeviceAttributes => b"\x1b[?1;2c".to_vec(),
        };
        let _ = writer.write_all(&response);
        let _ = writer.flush();
    }
    stream.drain(..consumed);
}

enum Query {
    CursorPosition,
    DeviceAttributes,
}

fn query_len(kind: &Query) -> usize {
    match kind {
        Query::CursorPosition => 4,
        Query::DeviceAttributes => 3,
    }
}

fn find_query(bytes: &[u8]) -> Option<(usize, Query)> {
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != 0x1b {
            continue;
        }
        let window = &bytes[index..];
        if window.starts_with(b"\x1b[6n") {
            return Some((index, Query::CursorPosition));
        }
        if window.starts_with(b"\x1b[c") || window.starts_with(b"\x1b[0c") {
            return Some((index, Query::DeviceAttributes));
        }
    }
    None
}
