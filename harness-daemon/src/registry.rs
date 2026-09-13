//! Active-session registry keyed by PID with boot-id liveness proofs
//! (todo 24 lease patterns, reused): entries record the boot id they
//! were proven under; a sweep reaps entries whose PID died, whose
//! heartbeat expired, or whose boot anchor no longer matches the
//! machine's current boot (a stale process cannot vouch for anyone).

use std::collections::HashMap;
use std::sync::Arc;

use harness_sessions::lease::LeaseProbe;

/// One registered process hosting or attached to sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    /// Process id.
    pub pid: u32,
    /// Boot id under which liveness was proven.
    pub boot_id: String,
    /// Sessions the process owns.
    pub sessions: Vec<String>,
    /// Last heartbeat (epoch seconds, injected clock).
    pub heartbeat_at: u64,
}

/// Why a sweep reaped an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaped {
    /// The reaped entry.
    pub entry: RegistryEntry,
    /// Liveness proof that failed.
    pub reason: ReapReason,
}

/// Liveness failure kinds (mirrors the todo 24 vocabulary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReapReason {
    /// PID no longer alive.
    PidDead,
    /// Heartbeat TTL expired.
    HeartbeatStale,
    /// Machine rebooted since the proof was recorded.
    BootIdChanged {
        /// Boot id recorded with the entry.
        recorded: String,
        /// Current machine boot id.
        current: String,
    },
}

/// PID-keyed registry with probe-backed liveness.
pub struct SessionRegistry {
    entries: HashMap<u32, RegistryEntry>,
    probe: Arc<dyn LeaseProbe>,
    boot_id: String,
}

impl SessionRegistry {
    /// New registry proving liveness through `probe` and anchored to the
    /// current machine boot id.
    pub fn new(probe: Arc<dyn LeaseProbe>) -> Self {
        let boot_id = probe.boot_id();
        Self {
            entries: HashMap::new(),
            probe,
            boot_id,
        }
    }

    /// Register a process. Fails closed when the process is not provably
    /// alive right now.
    pub fn register(
        &mut self,
        pid: u32,
        sessions: Vec<String>,
        now: u64,
    ) -> Result<(), ReapReason> {
        if !self.probe.pid_alive(pid) {
            return Err(ReapReason::PidDead);
        }
        if self.probe.boot_id() != self.boot_id {
            return Err(ReapReason::BootIdChanged {
                recorded: self.boot_id.clone(),
                current: self.probe.boot_id(),
            });
        }
        self.entries.insert(
            pid,
            RegistryEntry {
                pid,
                boot_id: self.boot_id.clone(),
                sessions,
                heartbeat_at: now,
            },
        );
        Ok(())
    }

    /// Refresh a live entry's heartbeat.
    pub fn heartbeat(&mut self, pid: u32, now: u64) -> bool {
        match self.entries.get_mut(&pid) {
            Some(entry) => {
                entry.heartbeat_at = now;
                true
            }
            None => false,
        }
    }

    /// The session ids owned by `pid`, if registered.
    #[must_use]
    pub fn sessions_of(&self, pid: u32) -> Option<&[String]> {
        self.entries.get(&pid).map(|e| e.sessions.as_slice())
    }

    /// All live entries.
    #[must_use]
    pub fn live(&self) -> Vec<RegistryEntry> {
        self.entries.values().cloned().collect()
    }

    /// Reap dead sessions: PID-dead, stale-heartbeat (ttl), and
    /// boot-mismatched entries are removed and reported. Their sessions
    /// self-heal (return to the unowned pool / queue).
    pub fn sweep(&mut self, now: u64, heartbeat_ttl: u64) -> Vec<Reaped> {
        let current_boot = self.probe.boot_id();
        let mut reaped = Vec::new();
        let mut retain = HashMap::with_capacity(self.entries.len());
        for (_, entry) in self.entries.drain() {
            let reason = if !self.probe.pid_alive(entry.pid) {
                Some(ReapReason::PidDead)
            } else if entry.boot_id != current_boot {
                Some(ReapReason::BootIdChanged {
                    recorded: entry.boot_id.clone(),
                    current: current_boot.clone(),
                })
            } else if now.saturating_sub(entry.heartbeat_at) > heartbeat_ttl {
                Some(ReapReason::HeartbeatStale)
            } else {
                None
            };
            match reason {
                Some(reason) => reaped.push(Reaped { entry, reason }),
                None => {
                    retain.insert(entry.pid, entry);
                }
            }
        }
        self.entries = retain;
        reaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_sessions::lease::FakeLeaseProbe;

    fn registry(boot: &str) -> (SessionRegistry, Arc<FakeLeaseProbe>) {
        let probe = Arc::new(FakeLeaseProbe::new(boot));
        (
            SessionRegistry::new(Arc::clone(&probe) as Arc<dyn LeaseProbe>),
            probe,
        )
    }

    #[test]
    fn register_requires_live_pid() {
        let (mut reg, probe) = registry("boot-1");
        probe.kill(4242);
        assert_eq!(
            reg.register(4242, vec!["s1".into()], 0),
            Err(ReapReason::PidDead)
        );
        assert!(reg.live().is_empty());
    }

    #[test]
    fn sweep_reaps_dead_pids_and_reports_sessions() {
        let (mut reg, probe) = registry("boot-1");
        assert!(
            reg.register(4242, vec!["s1".into(), "s2".into()], 0)
                .is_ok()
        );
        probe.kill(4242);
        let reaped = reg.sweep(10, 100);
        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].reason, ReapReason::PidDead);
        assert_eq!(
            reaped[0].entry.sessions,
            vec!["s1".to_owned(), "s2".to_owned()]
        );
        assert!(reg.live().is_empty());
    }

    #[test]
    fn sweep_reaps_stale_heartbeats() {
        let (mut reg, _probe) = registry("boot-1");
        assert!(reg.register(4242, vec!["s1".into()], 0).is_ok());
        let reaped = reg.sweep(1000, 100);
        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].reason, ReapReason::HeartbeatStale);
    }

    #[test]
    fn heartbeat_freshness_prevents_reaping() {
        let (mut reg, _probe) = registry("boot-1");
        assert!(reg.register(4242, vec!["s1".into()], 0).is_ok());
        assert!(reg.heartbeat(4242, 950));
        assert!(reg.sweep(1000, 100).is_empty());
        assert_eq!(reg.sessions_of(4242), Some(&["s1".to_owned()][..]));
    }

    #[test]
    fn reboot_invalidates_all_proofs() {
        let (mut reg, probe) = registry("boot-1");
        assert!(reg.register(4242, vec!["s1".into()], 0).is_ok());
        probe.reboot("boot-2");
        let reaped = reg.sweep(1, 100);
        assert_eq!(
            reaped[0].reason,
            ReapReason::BootIdChanged {
                recorded: "boot-1".into(),
                current: "boot-2".into()
            }
        );
        let denied = reg.register(777, vec!["s3".into()], 1);
        assert!(matches!(denied, Err(ReapReason::BootIdChanged { .. })));
    }
}
