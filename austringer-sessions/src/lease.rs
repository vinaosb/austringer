//! Fail-closed writer leases (Qwen §18): a session opened for writing is
//! guarded by a lease carrying a liveness proof — machine boot id plus
//! PID. A foreign live lease blocks writes with a typed error; a stale
//! lease (boot id changed or PID gone) is taken over with a typed
//! [`TakeoverReport`] that surfaces exactly what happened. There is no
//! force-unlock API and no silent takeover.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension};
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::error::SessionError;

/// Lease record as persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseInfo {
    /// Writer instance label (stable across handles in one process).
    pub owner_id: String,
    /// Liveness anchor: machine boot identity at acquire time.
    pub boot_id: String,
    /// OS process id of the acquiring writer.
    pub pid: u32,
    /// When the lease was acquired (epoch seconds, logical clock).
    pub acquired_at: u64,
    /// Last heartbeat (epoch seconds).
    pub heartbeat_at: u64,
}

impl std::fmt::Display for LeaseInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (pid {}, boot {}, acquired at epoch-seconds {})",
            self.owner_id, self.pid, self.boot_id, self.acquired_at
        )
    }
}

/// Why a previous lease was considered dead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleReason {
    /// The machine rebooted since the lease was written.
    BootIdChanged {
        /// Boot id recorded in the lease.
        recorded: String,
        /// Boot id observed now.
        current: String,
    },
    /// The boot matches but the writer's process no longer exists.
    PidDead {
        /// The dead writer's PID.
        pid: u32,
    },
}

/// The typed takeover record surfaced to the caller — never silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeoverReport {
    /// The lease that was found stale.
    pub previous: LeaseInfo,
    /// Why it was considered dead.
    pub reason: StaleReason,
    /// When the takeover happened.
    pub taken_at: u64,
}

/// Liveness probe: boot identity + PID aliveness. Injectable so tests
/// simulate reboots and dead writers deterministically.
pub trait LeaseProbe: Send + Sync {
    /// Identity of the current machine boot.
    fn boot_id(&self) -> String;
    /// Whether `pid` names a live process on this boot.
    fn pid_alive(&self, pid: u32) -> bool;
}

/// Production probe: `sysinfo` boot-time anchor (changes across reboots)
/// plus targeted single-PID refresh.
pub struct SysinfoLeaseProbe {
    boot_id: String,
    system: Mutex<System>,
}

impl SysinfoLeaseProbe {
    /// Captures the boot anchor for this process's lifetime.
    #[must_use]
    pub fn new() -> Self {
        Self {
            boot_id: System::boot_time().to_string(),
            system: Mutex::new(System::new()),
        }
    }
}

impl Default for SysinfoLeaseProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl LeaseProbe for SysinfoLeaseProbe {
    fn boot_id(&self) -> String {
        self.boot_id.clone()
    }

    fn pid_alive(&self, pid: u32) -> bool {
        let Ok(mut system) = self.system.lock() else {
            return false;
        };
        let target = Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[target]), true);
        system.process(target).is_some()
    }
}

/// Simulated probe for tests: mutable boot id, configurable dead PIDs.
#[derive(Debug, Default)]
pub struct FakeLeaseProbe {
    boot: Mutex<String>,
    dead_pids: Mutex<HashSet<u32>>,
}

impl FakeLeaseProbe {
    /// A probe whose boot id starts as `boot`.
    #[must_use]
    pub fn new(boot: &str) -> Self {
        Self {
            boot: Mutex::new(boot.to_owned()),
            dead_pids: Mutex::new(HashSet::new()),
        }
    }

    /// Simulates a machine reboot: the boot identity changes.
    pub fn reboot(&self, new_boot: &str) {
        *self.boot.lock().unwrap_or_else(|p| p.into_inner()) = new_boot.to_owned();
    }

    /// Marks `pid` as dead.
    pub fn kill(&self, pid: u32) {
        self.dead_pids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(pid);
    }
}

impl LeaseProbe for FakeLeaseProbe {
    fn boot_id(&self) -> String {
        self.boot.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn pid_alive(&self, pid: u32) -> bool {
        !self
            .dead_pids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&pid)
    }
}

/// Identifies the writer acquiring a lease.
#[derive(Debug, Clone)]
pub struct LeaseOwner {
    /// Stable writer label.
    pub owner_id: String,
    /// Writer's PID.
    pub pid: u32,
}

impl LeaseOwner {
    /// A writer labeled `owner_id` running in this process.
    #[must_use]
    pub fn new(owner_id: &str) -> Self {
        Self {
            owner_id: owner_id.to_owned(),
            pid: std::process::id(),
        }
    }
}

/// Result of a lease acquisition.
pub(crate) enum Acquired {
    /// Fresh lease, no predecessor.
    Fresh,
    /// Same writer reacquired; heartbeat refreshed.
    Renewed,
    /// A stale lease was replaced; the report must surface.
    Takeover(TakeoverReport),
}

pub(crate) fn load(conn: &Connection, session_id: &str) -> Result<Option<LeaseInfo>, SessionError> {
    conn.query_row(
        "SELECT owner_id, boot_id, pid, acquired_at, heartbeat_at
         FROM leases WHERE session_id = ?1",
        [session_id],
        |row| {
            Ok(LeaseInfo {
                owner_id: row.get(0)?,
                boot_id: row.get(1)?,
                pid: u32::try_from(row.get::<_, i64>(2)?).unwrap_or(u32::MAX),
                acquired_at: row.get::<_, i64>(3)?.max(0) as u64,
                heartbeat_at: row.get::<_, i64>(4)?.max(0) as u64,
            })
        },
    )
    .optional()
    .map_err(SessionError::sqlite)
}

pub(crate) fn acquire(
    conn: &Connection,
    session_id: &str,
    owner: &LeaseOwner,
    probe: &dyn LeaseProbe,
    now: u64,
) -> Result<Acquired, SessionError> {
    let current_boot = probe.boot_id();
    if let Some(existing) = load(conn, session_id)? {
        if existing.owner_id == owner.owner_id {
            conn.execute(
                "UPDATE leases SET heartbeat_at = ?2, boot_id = ?3, pid = ?4 WHERE session_id = ?1",
                rusqlite::params![session_id, now as i64, current_boot, owner.pid],
            )
            .map_err(SessionError::sqlite)?;
            return Ok(Acquired::Renewed);
        }
        let alive = existing.boot_id == current_boot && probe.pid_alive(existing.pid);
        if alive {
            return Err(SessionError::LockedByLiveWriter {
                id: session_id.to_owned(),
                lease: existing,
            });
        }
        let reason = if existing.boot_id != current_boot {
            StaleReason::BootIdChanged {
                recorded: existing.boot_id.clone(),
                current: current_boot.clone(),
            }
        } else {
            StaleReason::PidDead { pid: existing.pid }
        };
        conn.execute(
            "INSERT INTO leases (session_id, owner_id, boot_id, pid, acquired_at, heartbeat_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(session_id) DO UPDATE SET
                owner_id = excluded.owner_id,
                boot_id = excluded.boot_id,
                pid = excluded.pid,
                acquired_at = excluded.acquired_at,
                heartbeat_at = excluded.heartbeat_at",
            rusqlite::params![
                session_id,
                owner.owner_id,
                current_boot,
                owner.pid,
                now as i64
            ],
        )
        .map_err(SessionError::sqlite)?;
        return Ok(Acquired::Takeover(TakeoverReport {
            previous: existing,
            reason,
            taken_at: now,
        }));
    }
    conn.execute(
        "INSERT INTO leases (session_id, owner_id, boot_id, pid, acquired_at, heartbeat_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![
            session_id,
            owner.owner_id,
            current_boot,
            owner.pid,
            now as i64
        ],
    )
    .map_err(SessionError::sqlite)?;
    Ok(Acquired::Fresh)
}

pub(crate) fn release(
    conn: &Connection,
    session_id: &str,
    owner_id: &str,
) -> Result<(), SessionError> {
    conn.execute(
        "DELETE FROM leases WHERE session_id = ?1 AND owner_id = ?2",
        [session_id, owner_id],
    )
    .map_err(SessionError::sqlite)?;
    Ok(())
}

/// Shared production probe handle.
pub(crate) type SharedProbe = Arc<dyn LeaseProbe>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_probe_simulates_reboot_and_death() {
        let probe = FakeLeaseProbe::new("boot-a");
        assert_eq!(probe.boot_id(), "boot-a");
        assert!(probe.pid_alive(7));
        probe.kill(7);
        assert!(!probe.pid_alive(7));
        probe.reboot("boot-b");
        assert_eq!(probe.boot_id(), "boot-b");
    }

    #[test]
    fn production_probe_sees_itself_alive() {
        let probe = SysinfoLeaseProbe::new();
        assert!(!probe.boot_id().is_empty());
        assert!(probe.pid_alive(std::process::id()));
    }
}
