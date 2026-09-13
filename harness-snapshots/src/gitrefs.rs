//! Git ref/tree READ accessors for the snapshot namespace: turn
//! enumeration, manifest loading, snapshot-tree flattening, the
//! user-commits-after-snapshot walk, and blob reads.

use std::collections::BTreeMap;

use crate::SnapshotError;
use crate::gitstore::GitStore;
use crate::ids::{SessionId, Turn, manifest_ref, snapshot_ref};
use crate::manifest::SnapshotMeta;
use crate::tree::{parse_oid, tree_entries};

use gix::bstr::ByteSlice as _;

impl GitStore {
    /// All turn refs for a session (snapshot commits, no `.json` manifests).
    pub(crate) fn turns(&self, session: &SessionId) -> Result<Vec<Turn>, SnapshotError> {
        let prefix = format!("refs/harness/snapshots/{}/", session.as_str());
        let mut turns = Vec::new();
        let refs = self
            .repo
            .references()
            .map_err(|source| SnapshotError::git("list refs", source))?;
        let iter = refs
            .all()
            .map_err(|source| SnapshotError::git("iterate refs", source))?;
        for reference in iter {
            let reference =
                reference.map_err(|source| SnapshotError::git_boxed("ref entry", source))?;
            let name = reference.name().as_bstr().to_str_lossy().into_owned();
            let Some(slot) = name.strip_prefix(&prefix) else {
                continue;
            };
            let Ok(turn) = slot.parse::<u32>() else {
                continue;
            };
            turns
                .push(Turn::new(turn).map_err(|e| {
                    SnapshotError::io("turn", std::io::Error::other(e.to_string()))
                })?);
        }
        turns.sort_unstable();
        Ok(turns)
    }

    pub(crate) fn latest_turn(&self, session: &SessionId) -> Result<Option<Turn>, SnapshotError> {
        Ok(self.turns(session)?.pop())
    }

    /// Load the manifest sidecar; corrupt JSON is typed.
    pub(crate) fn meta(&self, session: &SessionId, turn: Turn) -> Option<SnapshotMeta> {
        let name = manifest_ref(session, turn);
        let reference = self.repo.find_reference(name.as_str()).ok()?;
        let oid = reference.try_id()?.detach();
        let blob = self.repo.find_blob(oid).ok()?;
        serde_json::from_slice(&blob.data).ok()
    }

    pub(crate) fn meta_required(
        &self,
        session: &SessionId,
        turn: Turn,
    ) -> Result<SnapshotMeta, SnapshotError> {
        self.meta(session, turn)
            .ok_or_else(|| SnapshotError::MissingSnapshot {
                session: session.as_str().to_owned(),
                turn: turn.get(),
            })
    }

    pub(crate) fn snapshot_tree(
        &self,
        session: &SessionId,
        turn: Turn,
    ) -> Result<BTreeMap<String, (String, u32)>, SnapshotError> {
        let reference = self
            .repo
            .find_reference(snapshot_ref(session, turn).as_str())
            .map_err(|_| SnapshotError::MissingSnapshot {
                session: session.as_str().to_owned(),
                turn: turn.get(),
            })?;
        let commit = self
            .repo
            .find_commit(self.ref_target(&reference)?)
            .map_err(|source| SnapshotError::git("snapshot commit", source))?;
        let mut files = BTreeMap::new();
        tree_entries(
            &self.repo,
            commit
                .tree_id()
                .map_err(|source| SnapshotError::git("tree id", source))?
                .detach(),
            "",
            &mut files,
        )?;
        Ok(files)
    }

    /// REFUSE restore while USER commits sit on the branch after the
    /// snapshot (Cline 4.1.17): walk HEAD's first-parent chain back to the
    /// snapshot base; every non-agent commit found is named.
    pub(crate) fn user_commits_after(
        &self,
        base_hex: Option<&str>,
    ) -> Result<Vec<String>, SnapshotError> {
        let Ok(head) = self.repo.head_commit() else {
            return Ok(Vec::new());
        };
        let mut user = Vec::new();
        let mut cursor = head.id;
        for _ in 0..1000 {
            let hex = cursor.to_hex().to_string();
            if Some(hex.as_str()) == base_hex {
                return Ok(user);
            }
            let commit = self
                .repo
                .find_commit(cursor)
                .map_err(|source| SnapshotError::git("walk commit", source))?;
            let message = commit
                .message_raw()
                .map_err(|source| SnapshotError::git("commit message", source))?
                .to_str_lossy()
                .into_owned();
            if !crate::provenance::is_agent_commit(&message) {
                user.push(hex);
            }
            let Some(parent) = commit.parent_ids().next() else {
                // The chain bottomed out without reaching the snapshot
                // base. An agent-only history below an unborn-HEAD snapshot
                // is fine; a real divergence from a known base is not.
                if base_hex.is_some() && user.is_empty() {
                    user.push(head.id.to_hex().to_string());
                }
                return Ok(user);
            };
            cursor = parent.detach();
        }
        Ok(user)
    }

    fn ref_target(&self, reference: &gix::Reference<'_>) -> Result<gix::ObjectId, SnapshotError> {
        reference
            .try_id()
            .map(|id| id.detach())
            .ok_or_else(|| SnapshotError::git("ref target", std::io::Error::other("symbolic ref")))
    }

    pub(crate) fn read_blob(&self, oid_hex: &str) -> Result<Vec<u8>, SnapshotError> {
        let blob = self
            .repo
            .find_blob(parse_oid(oid_hex))
            .map_err(|source| SnapshotError::git("read blob", source))?;
        Ok(blob.data.to_vec())
    }

    pub(crate) fn delete_ref(&self, name: &str) -> Result<(), SnapshotError> {
        match self.repo.find_reference(name) {
            Ok(reference) => {
                reference
                    .delete()
                    .map_err(|source| SnapshotError::git("delete ref", source))?;
                Ok(())
            }
            Err(_) => Ok(()),
        }
    }
}
