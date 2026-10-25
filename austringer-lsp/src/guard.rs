//! RSS-sampled bounded LSP (forge §26): LSP processes are the memory hogs
//! of coding harnesses, so a cheap always-on guard samples each spawned
//! server's resident set and kills runaways; the normal crash machinery
//! then restarts with backoff and degrades when the budget is spent.
//!
//! # Cost
//! One guard tick costs, per live spawned server, ONE targeted
//! single-PID refresh through `sysinfo` (a handful of syscalls — e.g. a
//! single `NtQuerySystemInformation`-family call on Windows), never a
//! process-table scan. At the default 30 s interval with ≤6 servers this
//! is unmeasurable noise — cheap enough to run always, by design.
//!
//! Attached (in-process/bridge) servers have no PID and are skipped: they
//! are not the austringer's processes to kill.

use std::sync::Arc;

use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::sync::mpsc::Sender;

use crate::config::{Language, RssGuardConfig};
use crate::lock;
use crate::router::RouterEvent;

/// Samples one process's resident set size in bytes.
pub trait RssProbe: Send + Sync {
    /// `None` when the process no longer exists.
    fn rss_bytes(&self, pid: u32) -> Option<u64>;
}

/// Production probe over [`sysinfo`] (pure-Rust, cross-platform — chosen
/// over hand-rolled Win32 PSAPI bindings: same data, no unsafe, one
/// dependency). Reuses one `System`, refreshing only the targeted PID.
pub struct SysinfoProbe {
    system: std::sync::Mutex<System>,
}

impl SysinfoProbe {
    /// Creates the probe.
    #[must_use]
    pub fn new() -> Self {
        Self {
            system: std::sync::Mutex::new(System::new()),
        }
    }
}

impl Default for SysinfoProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl RssProbe for SysinfoProbe {
    fn rss_bytes(&self, pid: u32) -> Option<u64> {
        let mut system = lock(&self.system);
        let target = Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[target]), false);
        system.process(target).map(|process| process.memory())
    }
}

/// One guard-observable server: its language, its OS pid (spawned servers
/// only), and how to kill it.
pub struct GuardTarget {
    language: Language,
    pid: Option<u32>,
    kill: Arc<dyn Fn() + Send + Sync>,
}

impl GuardTarget {
    /// Builds a target; `kill` must be idempotent.
    pub fn new(
        language: Language,
        pid: Option<u32>,
        kill: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            language,
            pid,
            kill: Arc::new(kill),
        }
    }

    /// The target's language.
    #[must_use]
    pub fn language(&self) -> Language {
        self.language
    }

    /// The target's pid, when it owns a process.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Kills the underlying server (idempotent).
    pub fn kill(&self) {
        (self.kill)();
    }
}

/// One audit pass: sample every target with a pid, kill the ones at or
/// over `cap_bytes`, emitting [`RouterEvent::RssCapped`] per kill. The
/// subsequent death bookkeeping (crash accounting, backoff restart,
/// degradation) flows through the normal slot machinery.
pub async fn enforce_once(
    targets: &[GuardTarget],
    probe: &dyn RssProbe,
    cap_bytes: u64,
    events: &Sender<RouterEvent>,
) {
    for target in targets {
        let Some(pid) = target.pid else { continue };
        let Some(rss) = probe.rss_bytes(pid) else {
            continue;
        };
        if rss >= cap_bytes {
            let _ = events.try_send(RouterEvent::RssCapped {
                language: target.language,
                rss_bytes: rss,
                cap_bytes,
            });
            target.kill();
        }
    }
}

/// The always-on loop: tick, collect targets, [`enforce_once`]. `provide`
/// is re-polled every tick so the guard sees the current set of live
/// servers. Runs until aborted (the router drops its handle).
pub async fn run_guard_loop<P>(
    provide: P,
    probe: Arc<dyn RssProbe>,
    config: RssGuardConfig,
    events: Sender<RouterEvent>,
) where
    P: Fn() -> Vec<GuardTarget>,
{
    let mut ticker = tokio::time::interval(config.sample_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if !config.enabled {
            continue;
        }
        let targets = provide();
        enforce_once(&targets, probe.as_ref(), config.rss_cap_bytes, &events).await;
    }
}
