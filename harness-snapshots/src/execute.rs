//! Restore-transaction execution: stash the dirty state, apply the rewind
//! plan with in-memory backups, roll the whole batch back on any failure,
//! and emit the truncation cursor to the log seam only on a committed
//! restore.

use crate::SnapshotError;
use crate::ids::SessionId;
use crate::restore::{
    ContentSource, RestoreReport, RestoreTx, RewindLog, RewindPlan, StashRef, new_transaction,
};
use crate::store::{Backend, SnapshotStore};
use crate::tree;

impl SnapshotStore {
    fn read_source(
        &self,
        session: &SessionId,
        source: &ContentSource,
    ) -> Result<Vec<u8>, SnapshotError> {
        match (&self.inner, source) {
            (Backend::Git(git), ContentSource::Blob { oid_hex }) => git.read_blob(oid_hex),
            (Backend::NoGit(nogit), ContentSource::Cas { hash_hex }) => {
                nogit.read_object(session, hash_hex)
            }
            _ => Err(SnapshotError::Serialize(
                "backend/source mismatch".to_owned(),
            )),
        }
    }

    /// The restore transaction: stash dirty state first, apply with
    /// in-memory backups, roll the whole batch back on any failure, emit
    /// the truncation cursor to the log seam only on commit.
    pub fn execute_rewind(
        &self,
        plan: RewindPlan,
        log: Option<&mut dyn RewindLog>,
        probe: Option<&mut dyn crate::RestoreProbe>,
    ) -> Result<RestoreReport, SnapshotError> {
        let tx = new_transaction()?;
        let stash_name = match &self.inner {
            Backend::Git(git) => tree::stash_current(&git.repo, &git.workdir, &tx)?,
            Backend::NoGit(nogit) => nogit.stash(&plan.session, &tx, self.workdir())?,
        };
        let stash = StashRef {
            ref_name: stash_name,
            tx,
        };

        let mut restore = RestoreTx::new(self.workdir());
        let mut default_probe = ();
        let probe: &mut dyn crate::RestoreProbe = probe.unwrap_or(&mut default_probe);
        match self.apply_plan(&plan, &mut restore, probe) {
            Ok(()) => {
                if let Some(log) = log {
                    log.truncate(&plan.event_cursor)?;
                }
                Ok(RestoreReport {
                    restored: restore.restored,
                    deleted: restore.deleted,
                    skipped: plan.skipped.clone(),
                    stash: Some(stash),
                    task: plan.task.clone(),
                    rolled_back: false,
                })
            }
            Err(error) => {
                let (_, _, clean) = restore.rollback();
                Err(match error {
                    SnapshotError::RestoreFailed { source, .. } => SnapshotError::RestoreFailed {
                        source,
                        restored: clean,
                    },
                    other => other,
                })
            }
        }
    }

    fn apply_plan(
        &self,
        plan: &RewindPlan,
        restore: &mut RestoreTx,
        probe: &mut dyn crate::RestoreProbe,
    ) -> Result<(), SnapshotError> {
        for write in &plan.writes {
            let bytes = self.read_source(&plan.session, &write.source)?;
            restore.write(&write.rel, &bytes, write.executable, probe)?;
        }
        for rel in &plan.deletes {
            restore.delete(rel, probe)?;
        }
        Ok(())
    }
}
