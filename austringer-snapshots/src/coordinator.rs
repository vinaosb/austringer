//! Periodic workspace checkpoint coordinator (Warp): a safe-boundary
//! gated five-state machine (`Idle → Arming → Snapshotting → Armed →
//! Restoring`), the cheap dirty-check that auto-updates checkpoints on
//! repo change, and end-of-run snapshots.
//!
//! The safe-boundary gate is "no tool mid-flight": transitions that touch
//! the workspace only happen when the caller reports a quiet boundary.
//! Restores serialize through the machine — a rewind during a rewind is
//! [`SnapshotError::Busy`], never interleaved file writes. The host is
//! additionally expected to kill live terminals at restore time for
//! coherence (host-side wiring, later todo).

use std::path::Path;

use crate::checkpoint::{CheckpointOrigin, CheckpointRecord, CheckpointRecorder};
use crate::ids::{SessionId, Turn};
use crate::manifest::{ExternallyModifiedPolicy, RestoreMode, SnapshotOptions, SnapshotOutcome};
use crate::scratch::{RETENTION, reap_scratch};
use crate::store::SnapshotStore;
use crate::walk;
use crate::{RestoreReport, SnapshotError, util};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorState {
    Idle,
    Arming,
    Snapshotting,
    Armed,
    Restoring,
}

/// The cheap dirty-check: blake3 over every visible file's
/// `(rel, len, mtime)` — no content reads. Best-effort: an unwalkable
/// root still yields a stable, distinguishable value.
pub struct DirtyCheck;

impl DirtyCheck {
    pub fn fingerprint(root: &Path) -> u64 {
        let mut hasher = blake3::Hasher::new();
        match walk::walk(root, &[]) {
            Ok(output) => {
                for rel in output.files.keys() {
                    hasher.update(rel.as_bytes());
                    hasher.update(&[0]);
                    match util::file_stat(&output.files[rel]) {
                        Ok((secs, nanos, size)) => {
                            hasher.update(&secs.to_be_bytes());
                            hasher.update(&nanos.to_be_bytes());
                            hasher.update(&size.to_be_bytes());
                        }
                        Err(error) => {
                            hasher.update(error.to_string().as_bytes());
                        }
                    }
                }
            }
            Err(error) => {
                hasher.update(error.to_string().as_bytes());
            }
        }
        let digest = hasher.finalize();
        let bytes = digest.as_bytes();
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    }
}

pub struct CheckpointCoordinator {
    store: SnapshotStore,
    recorder: Box<dyn CheckpointRecorder>,
    state: CoordinatorState,
    last_fingerprint: Option<u64>,
}

impl CheckpointCoordinator {
    pub fn new(store: SnapshotStore, recorder: Box<dyn CheckpointRecorder>) -> Self {
        Self {
            store,
            recorder,
            state: CoordinatorState::Idle,
            last_fingerprint: None,
        }
    }

    #[must_use]
    pub fn state(&self) -> CoordinatorState {
        self.state
    }

    /// Idle→Arming only on a quiet boundary (no tool mid-flight); any
    /// other state is a no-op observation.
    pub fn on_safe_boundary(&mut self, quiet: bool) -> Result<(), SnapshotError> {
        if self.state == CoordinatorState::Idle && quiet {
            self.state = CoordinatorState::Arming;
        }
        Ok(())
    }

    /// Auto-update on repo change: the cheap fingerprint suppresses
    /// redundant snapshots; a change takes one.
    pub fn maybe_checkpoint(
        &mut self,
        session_root: &Path,
        session: &SessionId,
        turn: Turn,
        opts: &SnapshotOptions,
    ) -> Result<Option<SnapshotOutcome>, SnapshotError> {
        if self.state == CoordinatorState::Idle {
            return Ok(None);
        }
        self.reap_scratch_dirs();
        let fingerprint = DirtyCheck::fingerprint(self.store.workdir());
        if self.last_fingerprint == Some(fingerprint) {
            return Ok(None);
        }
        self.state = CoordinatorState::Snapshotting;
        let outcome = self.store.snapshot(session, turn, opts)?;
        self.record(&outcome, session_root);
        self.last_fingerprint = Some(fingerprint);
        self.state = CoordinatorState::Armed;
        Ok(Some(outcome))
    }

    /// End-of-run snapshots always fire, fingerprint be damned.
    pub fn end_of_run(
        &mut self,
        session_root: &Path,
        session: &SessionId,
        turn: Turn,
        opts: &SnapshotOptions,
    ) -> Result<SnapshotOutcome, SnapshotError> {
        self.reap_scratch_dirs();
        self.state = CoordinatorState::Snapshotting;
        let outcome = self.store.snapshot(session, turn, opts)?;
        self.record(&outcome, session_root);
        self.last_fingerprint = Some(DirtyCheck::fingerprint(self.store.workdir()));
        self.state = CoordinatorState::Armed;
        Ok(outcome)
    }

    /// Armed (or Idle) → Restoring; a rewind already in flight refuses.
    pub fn begin_restore(&mut self) -> Result<(), SnapshotError> {
        if self.state == CoordinatorState::Restoring {
            return Err(SnapshotError::Busy { state: "Restoring" });
        }
        self.state = CoordinatorState::Restoring;
        Ok(())
    }

    pub fn finish_restore(&mut self) {
        if self.state == CoordinatorState::Restoring {
            self.state = CoordinatorState::Armed;
        }
    }

    pub fn rewind(
        &mut self,
        session_root: &Path,
        session: &SessionId,
        turn: Turn,
        mode: RestoreMode,
        policy: ExternallyModifiedPolicy,
    ) -> Result<RestoreReport, SnapshotError> {
        let plan = self
            .store
            .rewind_plan(session_root, session, turn, mode, policy)?;
        let report = self.store.execute_rewind(plan, None, None)?;
        self.last_fingerprint = None;
        Ok(report)
    }

    fn record(&mut self, outcome: &SnapshotOutcome, _session_root: &Path) {
        let target = outcome
            .ref_name
            .clone()
            .or_else(|| {
                outcome
                    .manifest_path
                    .as_ref()
                    .map(|path| path.display().to_string())
            })
            .unwrap_or_default();
        let record = CheckpointRecord {
            session: outcome.session.clone(),
            turn: outcome.turn,
            origin: CheckpointOrigin::Native,
            target,
        };
        if let Err(error) = self.recorder.record(&record) {
            tracing::warn!(%error, "checkpoint thread record failed");
        }
    }

    fn reap_scratch_dirs(&self) {
        let scratch = self.store.workdir().join(".austringer").join("scratch");
        if let Err(error) = reap_scratch(&scratch, std::time::SystemTime::now(), RETENTION) {
            tracing::warn!(%error, "scratch reap failed");
        }
    }
}
