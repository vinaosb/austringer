//! The plugin handle: call dispatch, hot-reload swap semantics, and the
//! polling watcher seam.
//!
//! Swap semantics: a call first *snapshots* the current compiled plugin
//! (an `Arc` clone) and then runs outside the lock - so a concurrent
//! reload only affects calls that have not yet started. In-flight calls
//! always finish on the instance they started with. A FAILED reload keeps
//! the old plugin serving (the swap happens only after a fully successful
//! re-load).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::WasmError;
use crate::hostfns::{PluginGrant, ResolvedScope};
use crate::limits::PluginLimits;
use crate::loader::{self, LoadedPlugin};
use crate::runtime::{self, CallReport, WasmEngine};

struct HandleInner {
    engine: Arc<WasmEngine>,
    grant: PluginGrant,
    limits: PluginLimits,
    state: Mutex<PluginState>,
}

struct PluginState {
    loaded: Arc<LoadedPlugin>,
}

/// A loaded, reloadable plugin.
#[derive(Clone)]
pub struct PluginHandle {
    inner: Arc<HandleInner>,
}

/// A pinned view of the plugin exactly as one in-flight call sees it.
pub struct PluginSnapshot {
    loaded: Arc<LoadedPlugin>,
    scopes: Vec<ResolvedScope>,
    limits: PluginLimits,
}

/// Result of one watcher poll.
#[derive(Debug)]
pub enum ReloadOutcome {
    /// File fingerprint unchanged since load.
    Unchanged,
    /// A new plugin was loaded; new calls use it.
    Swapped,
    /// The file changed but re-loading failed; the old plugin keeps serving.
    Failed(WasmError),
}

/// Events emitted by the background watcher.
#[derive(Debug)]
pub enum ReloadEvent {
    /// The plugin was swapped successfully.
    Reloaded {
        /// Post-swap plugin name.
        plugin: String,
    },
    /// A swap attempt failed; the old plugin keeps serving.
    Failed {
        /// Serving (old) plugin name.
        plugin: String,
        /// Why the reload failed.
        error: WasmError,
    },
}

impl PluginHandle {
    /// Load a plugin file (`.wat` or `.wasm`) under an explicit grant and
    /// limits. Fails typed for every load-time violation.
    pub fn load(path: &Path, grant: &PluginGrant, limits: PluginLimits) -> Result<Self, WasmError> {
        let engine = Arc::new(WasmEngine::new()?);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "plugin".into());
        let loaded = Arc::new(loader::load(&engine, &stem, path, grant, &limits)?);
        Ok(Self {
            inner: Arc::new(HandleInner {
                engine,
                grant: grant.clone(),
                limits,
                state: Mutex::new(PluginState { loaded }),
            }),
        })
    }

    fn state(&self) -> MutexGuard<'_, PluginState> {
        // Lock sections in this crate never panic, so poisoning is not a
        // reachable state; recover the data regardless.
        self.inner.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The currently-serving manifest.
    pub fn manifest(&self) -> crate::manifest::PluginManifest {
        self.state().loaded.manifest.clone()
    }

    /// Pin the current instance (what an in-flight call runs on).
    pub fn snapshot(&self) -> PluginSnapshot {
        let state = self.state();
        PluginSnapshot {
            loaded: state.loaded.clone(),
            scopes: self.inner.grant.scopes.clone(),
            limits: self.inner.limits,
        }
    }

    /// Call a tool by manifest name with raw input bytes. The report
    /// carries the typed outcome, fuel consumed, logs, and host events.
    pub fn call_raw(&self, tool: &str, input: &[u8]) -> CallReport {
        let loaded = self.state().loaded.clone();
        invoke_loaded(
            &loaded,
            tool,
            input,
            &self.inner.grant.scopes,
            &self.inner.limits,
        )
    }

    /// Manually reload from the plugin path, regardless of fingerprint.
    pub fn reload(&self) -> Result<(), WasmError> {
        let mut state = self.state();
        let stem = state
            .loaded
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "plugin".into());
        let fresh = loader::load(
            &self.inner.engine,
            &stem,
            &state.loaded.path,
            &self.inner.grant,
            &self.inner.limits,
        )?;
        state.loaded = Arc::new(fresh);
        Ok(())
    }

    /// Poll the plugin path once; reload if the fingerprint changed.
    pub fn poll_once(&self) -> ReloadOutcome {
        let (path, current) = {
            let state = self.state();
            (state.loaded.path.clone(), state.loaded.fingerprint)
        };
        let seen = match loader::fingerprint(&path) {
            Ok(seen) => seen,
            Err(e) => return ReloadOutcome::Failed(e),
        };
        if seen == current {
            return ReloadOutcome::Unchanged;
        }
        match self.reload() {
            Ok(()) => ReloadOutcome::Swapped,
            Err(e) => ReloadOutcome::Failed(e),
        }
    }
}

impl PluginSnapshot {
    /// Call a tool on the pinned instance (old semantics, post-swap).
    pub fn call_raw(&self, tool: &str, input: &[u8]) -> CallReport {
        invoke_loaded(&self.loaded, tool, input, &self.scopes, &self.limits)
    }
}

fn invoke_loaded(
    loaded: &LoadedPlugin,
    tool: &str,
    input: &[u8],
    scopes: &[ResolvedScope],
    limits: &PluginLimits,
) -> CallReport {
    let Some(tool_id) = loaded.manifest.tool_id(tool) else {
        return CallReport {
            output: Err(WasmError::ToolNotDeclared {
                plugin: loaded.manifest.name.clone(),
                tool: tool.into(),
            }),
            fuel_consumed: 0,
            logs: Vec::new(),
            events: Vec::new(),
        };
    };
    runtime::invoke(
        &loaded.module,
        &loaded.manifest.name,
        tool_id,
        tool,
        input,
        scopes.to_vec(),
        limits,
    )
}

/// Spawn the polling watcher thread for one plugin. The guard stops and
/// joins the thread on drop; events arrive on the receiver.
pub struct PluginWatcher;

impl PluginWatcher {
    /// Start watching `handle` every `period`. Reload events (success or
    /// failure) are sent on the returned channel.
    pub fn spawn(handle: PluginHandle, period: Duration) -> Result<WatcherGuard, WasmError> {
        let (tx, rx) = std::sync::mpsc::channel::<ReloadEvent>();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("austringer-wasm-watcher".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    let plugin = handle.manifest().name;
                    match handle.poll_once() {
                        ReloadOutcome::Swapped => {
                            if tx.send(ReloadEvent::Reloaded { plugin }).is_err() {
                                break;
                            }
                        }
                        ReloadOutcome::Failed(error) => {
                            if tx.send(ReloadEvent::Failed { plugin, error }).is_err() {
                                break;
                            }
                        }
                        ReloadOutcome::Unchanged => {}
                    }
                    std::thread::sleep(period);
                }
            })
            .map_err(|e| WasmError::WatcherSpawn {
                detail: e.to_string(),
            })?;
        Ok(WatcherGuard {
            stop,
            handle: Some(worker),
            rx,
        })
    }
}

/// Owns a watcher thread; dropping it stops and joins the thread.
pub struct WatcherGuard {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    rx: Receiver<ReloadEvent>,
}

impl WatcherGuard {
    /// The reload-event stream.
    pub fn events(&self) -> &Receiver<ReloadEvent> {
        &self.rx
    }
}

impl Drop for WatcherGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.handle.take() {
            let _ = worker.join();
        }
    }
}
