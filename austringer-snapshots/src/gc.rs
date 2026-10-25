//! Snapshot GC: age/count tiers with pinned turns exempt. Deleting a
//! snapshot removes its namespace refs (git) or manifest (fallback);
//! loose objects stay for the object database's own compaction — refs are
//! the ownership record.

use std::time::{Duration, SystemTime};

use crate::ids::{SessionId, Turn, manifest_ref, snapshot_ref};
use crate::{SnapshotError, store};

#[derive(Debug, Clone)]
pub struct SnapshotGcPolicy {
    /// Always-kept newest turns per session (tier 0).
    pub keep_last_per_session: usize,
    /// Hard cap on turns per session (tier 1); `None` disables.
    pub max_per_session: Option<usize>,
    /// Turns recorded before `now - max_age` fall off (tier 2); `None`
    /// disables.
    pub max_age: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GcOutcome {
    pub removed: usize,
    pub retained: usize,
    pub pinned_kept: usize,
}

impl store::SnapshotStore {
    pub fn gc(
        &self,
        policy: &SnapshotGcPolicy,
        pinned: &[(SessionId, Turn)],
    ) -> Result<GcOutcome, SnapshotError> {
        let now = SystemTime::now();
        let mut outcome = GcOutcome::default();
        for session in self.sessions()? {
            let turns = self.turns_of(&session)?;
            let newest = turns.last().copied();
            for turn in turns {
                let is_pinned = pinned.iter().any(|(s, t)| s == &session && t == &turn);
                if is_pinned {
                    outcome.pinned_kept += 1;
                    outcome.retained += 1;
                    continue;
                }
                let newest_kept = newest.is_some_and(|last| last == turn)
                    && self
                        .turn_index(&turn, newest)
                        .is_some_and(|index| index < policy.keep_last_per_session);
                let under_cap = policy
                    .max_per_session
                    .is_none_or(|cap| self.turn_index(&turn, newest).is_some_and(|i| i < cap));
                let within_age = policy.max_age.is_none_or(|max_age| {
                    self.recorded_at(&session, &turn)
                        .is_none_or(|at| age_within(now, at, max_age))
                });
                if newest_kept || (under_cap && within_age) {
                    outcome.retained += 1;
                } else {
                    self.delete_snapshot(&session, turn)?;
                    outcome.removed += 1;
                }
            }
        }
        Ok(outcome)
    }
}

fn age_within(now: SystemTime, recorded_unix: u64, max_age: Duration) -> bool {
    let Ok(age) = now.duration_since(SystemTime::UNIX_EPOCH + max_age) else {
        return true;
    };
    age.as_secs() <= recorded_unix
}

impl store::SnapshotStore {
    fn turn_index(&self, turn: &Turn, newest: Option<Turn>) -> Option<usize> {
        let newest = newest?;
        newest
            .get()
            .checked_sub(turn.get())
            .map(|distance| distance as usize)
    }

    fn recorded_at(&self, session: &SessionId, turn: &Turn) -> Option<u64> {
        self.meta_of(session, *turn)
            .ok()
            .map(|meta| meta.recorded_unix)
    }

    fn delete_snapshot(&self, session: &SessionId, turn: Turn) -> Result<(), SnapshotError> {
        match self.backend() {
            store::Backend::Git(git) => {
                git.delete_ref(&snapshot_ref(session, turn))?;
                git.delete_ref(&manifest_ref(session, turn))?;
                Ok(())
            }
            store::Backend::NoGit(nogit) => {
                nogit.delete_manifest(session, turn)?;
                let live = nogit.live_objects(session)?;
                nogit.sweep_objects(session, &live)?;
                Ok(())
            }
        }
    }
}
