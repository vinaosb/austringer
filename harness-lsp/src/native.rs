//! One connected language server: process (or attached transport), the
//! JSON-RPC request/notification plumbing, captured stderr, and death
//! semantics.
//!
//! Two constructors:
//! * [`ServerHandle::spawn`] — a real child process with piped stdio (the
//!   native client's servers). The child is killed on drop: no orphaned
//!   servers, ever (full kill-trees are todo 19/20).
//! * [`ServerHandle::attach`] — caller-provided transport halves plus an
//!   optional stderr stream. This is the seam the in-test mock LS, the
//!   future IDE bridge, and any in-process server speak through; attached
//!   handles have no PID and are therefore outside the RSS guard's reach.

use std::collections::HashMap;
use std::io;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex as AsyncMutex, oneshot};
use tokio::time::timeout;

use crate::config::{ServerCommand, StderrHook};
use crate::error::LspError;
use crate::lock;
use crate::proto::{FrameCaps, write_frame};

/// Ring-buffer depth for retained stderr lines when no hook is configured.
pub(crate) const STDERR_RING_LINES: usize = 8;

/// Per-line stderr retention cap (chars).
pub(crate) const STDERR_LINE_CHARS: usize = 512;

/// Byte cap for one stderr line read (a hostile server cannot grow memory).
pub(crate) const STDERR_LINE_READ_CAP: usize = 4096;

/// Which position encoding the server actually speaks after `initialize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionEncoding {
    /// Server honored the UTF-8 request: columns are char offsets.
    Utf8,
    /// Server is UTF-16-only: columns arrive as code units and are
    /// converted through the file's text (see [`crate::position`]).
    Utf16,
}

type Pending = HashMap<i64, oneshot::Sender<Result<Value, LspError>>>;
type DeathHook = Box<dyn Fn() + Send + Sync>;

pub(crate) struct Inner {
    pub(crate) writer: AsyncMutex<Box<dyn AsyncWrite + Unpin + Send>>,
    pub(crate) pending: Mutex<Pending>,
    pub(crate) dead: AtomicBool,
    pub(crate) killed: tokio::sync::Notify,
    pub(crate) pid: Option<u32>,
    pub(crate) stderr_tail: Mutex<Vec<String>>,
    pub(crate) child: Mutex<Option<Child>>,
    pub(crate) on_death: Mutex<Option<DeathHook>>,
}

/// Everything needed to bring one transport online.
struct Transport {
    reader: Box<dyn AsyncRead + Unpin + Send>,
    pub(crate) writer: Box<dyn AsyncWrite + Unpin + Send>,
    stderr: Option<Box<dyn AsyncRead + Unpin + Send>>,
    stderr_hook: Option<StderrHook>,
    pub(crate) pid: Option<u32>,
    pub(crate) child: Option<Child>,
}

/// A live server connection, shared as `Arc<ServerHandle>`.
pub struct ServerHandle {
    inner: Arc<Inner>,
    next_id: AtomicI64,
}

impl ServerHandle {
    /// Spawns `command` with piped stdio and starts the reader + stderr
    /// tasks. Does NOT run the handshake — [`crate::handshake`] owns that.
    pub fn spawn(
        command: &ServerCommand,
        stderr_hook: Option<&StderrHook>,
        _caps: FrameCaps,
    ) -> io::Result<Self> {
        let mut child = Command::new(&command.program)
            .args(&command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let pid = child.id();
        let stdout = child.stdout.take();
        let stdin = child.stdin.take();
        let stderr = child.stderr.take();
        let (Some(stdout), Some(stdin)) = (stdout, stdin) else {
            return Err(io::Error::other("child pipes unavailable"));
        };
        Ok(Self::bring_online(Transport {
            reader: Box::new(tokio::io::BufReader::new(stdout)),
            writer: Box::new(stdin),
            stderr: stderr.map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>),
            stderr_hook: stderr_hook.cloned(),
            pid,
            child: Some(child),
        }))
    }

    /// Attaches to caller-provided transport halves (mock LS, IDE bridge,
    /// in-process servers). No PID, no child — outside the RSS guard.
    #[must_use]
    pub fn attach(
        reader: Box<dyn AsyncRead + Unpin + Send>,
        writer: Box<dyn AsyncWrite + Unpin + Send>,
        stderr: Option<Box<dyn AsyncRead + Unpin + Send>>,
    ) -> Self {
        Self::bring_online(Transport {
            reader,
            writer,
            stderr,
            stderr_hook: None,
            pid: None,
            child: None,
        })
    }

    fn bring_online(transport: Transport) -> Self {
        let inner = Arc::new(Inner {
            writer: AsyncMutex::new(transport.writer),
            pending: Mutex::new(HashMap::new()),
            dead: AtomicBool::new(false),
            killed: tokio::sync::Notify::new(),
            pid: transport.pid,
            stderr_tail: Mutex::new(Vec::new()),
            child: Mutex::new(transport.child),
            on_death: Mutex::new(None),
        });

        if let Some(stderr) = transport.stderr {
            let stderr_inner = Arc::downgrade(&inner);
            tokio::spawn(crate::stderr::capture_stderr(
                stderr,
                stderr_inner,
                transport.stderr_hook,
            ));
        }
        let reader_inner = Arc::downgrade(&inner);
        tokio::spawn(crate::wire::read_loop(transport.reader, reader_inner));

        Self {
            inner,
            next_id: AtomicI64::new(1),
        }
    }

    /// The server process id, if this handle owns a process.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.inner.pid
    }

    /// Whether the transport has died (EOF, corruption, or kill).
    #[must_use]
    pub fn is_dead(&self) -> bool {
        self.inner.dead.load(Ordering::Acquire)
    }

    /// Sets the callback invoked exactly once when the transport dies.
    /// The router uses this to mark slots crashed idempotently.
    pub fn set_on_death(&self, hook: DeathHook) {
        *lock(&self.inner.on_death) = Some(hook);
    }

    /// Sends a JSON-RPC request and awaits its response with a timeout.
    /// Dropping the future cancels cleanly: the pending entry survives
    /// until the response (or death drain) removes it, and the server is
    /// never corrupted by an abandoned request.
    pub async fn request(
        &self,
        method: &'static str,
        params: Value,
        per_request_timeout: Duration,
    ) -> Result<Value, LspError> {
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let (tx, rx) = oneshot::channel();
        lock(&self.inner.pending).insert(id, tx);

        let envelope = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if let Err(err) = self.write_json(&envelope).await {
            lock(&self.inner.pending).remove(&id);
            return Err(err);
        }

        match timeout(per_request_timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_sender_dropped)) => Err(LspError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "reader task closed the pending channel",
            ))),
            Err(_elapsed) => {
                lock(&self.inner.pending).remove(&id);
                Err(LspError::RequestTimeout {
                    method,
                    waited: per_request_timeout,
                })
            }
        }
    }

    /// Sends a JSON-RPC notification (no response expected).
    pub async fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        let envelope = json!({"jsonrpc": "2.0", "method": method, "params": params});
        self.write_json(&envelope).await
    }

    /// Kills the server now (idempotent): child kill plus dead-marking so
    /// [`ServerHandle::is_dead`] reports truth immediately. The `killed`
    /// notification interrupts a parked reader (attached transports have
    /// no child whose death would EOF the pipe), and the reader loop
    /// finishes the pending drain and fires the death hook.
    pub fn kill(&self) {
        self.inner.dead.store(true, Ordering::Release);
        self.inner.killed.notify_one();
        if let Some(child) = lock(&self.inner.child).as_mut() {
            let _ = child.start_kill();
        }
    }

    /// Retained stderr tail (bounded ring; empty when a hook consumes).
    #[must_use]
    pub fn stderr_tail(&self) -> Vec<String> {
        lock(&self.inner.stderr_tail).clone()
    }

    /// Waits until at least one stderr line is captured, or `within`
    /// elapses (diagnostic + test helper).
    pub async fn wait_stderr_tail(&self, within: Duration) -> Vec<String> {
        let deadline = tokio::time::Instant::now() + within;
        loop {
            let tail = self.stderr_tail();
            if !tail.is_empty() || tokio::time::Instant::now() >= deadline {
                return tail;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn write_json(&self, envelope: &Value) -> Result<(), LspError> {
        let body = serde_json::to_vec(envelope).map_err(|err| LspError::MalformedFrame {
            detail: err.to_string(),
        })?;
        let mut writer = self.inner.writer.lock().await;
        write_frame(&mut *writer, &body).await.map_err(LspError::Io)
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.kill();
    }
}
