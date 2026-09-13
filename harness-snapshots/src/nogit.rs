//! No-git fallback: non-repo projects snapshot via content-addressed file
//! snapshots under `<workdir>/.harness/snapshots/` (the T21 CoW machinery:
//! `reflink_or_copy` with per-file fallback — FICLONE / APFS clonefile /
//! ReFS block cloning where the filesystem allows, plain copy elsewhere).
//! Rewind and dirty-state restore degrade gracefully; git-only features
//! (refs, provenance trailers) surface as disabled capability flags.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ids::{SessionId, Turn};
use crate::manifest::{
    SnapshotMeta, SnapshotOptions, SnapshotOutcome, StatCacheStats, SubmoduleRecord,
};
use crate::provenance::DetachedSignature;
use crate::walk;
use crate::{SnapshotError, util};

pub(crate) struct NoGitStore {
    pub root: PathBuf,
}

const SNAPSHOTS: &str = "snapshots";
const MANIFESTS: &str = "manifests";
const OBJECTS: &str = "objects";

impl NoGitStore {
    pub(crate) fn new(workdir: &Path) -> Self {
        Self {
            root: workdir.join(".harness").join(SNAPSHOTS),
        }
    }

    fn session_dir(&self, session: &SessionId) -> PathBuf {
        self.root.join(session.as_str())
    }

    fn manifest_path(&self, session: &SessionId, turn: Turn) -> PathBuf {
        self.session_dir(session)
            .join(MANIFESTS)
            .join(format!("{}.json", turn.slot()))
    }

    fn object_path(&self, session: &SessionId, hash: &str) -> PathBuf {
        self.session_dir(session)
            .join(OBJECTS)
            .join(&hash[0..2])
            .join(hash)
    }

    pub(crate) fn snapshot(
        &self,
        session: &SessionId,
        turn: Turn,
        opts: &SnapshotOptions,
    ) -> Result<SnapshotOutcome, SnapshotError> {
        let workdir = self
            .root
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| SnapshotError::NotARepository {
                workdir: self.root.clone(),
            })?
            .to_path_buf();
        let walked = walk::walk(&workdir, &opts.extra_excludes)?;
        let mut stats = StatCacheStats::default();
        let mut files = BTreeMap::new();
        for (rel, path) in &walked.files {
            let bytes = util::read_file(path)?;
            let hash = blake3::hash(&bytes).to_hex().to_string();
            let dst = self.object_path(session, &hash);
            if dst.symlink_metadata().is_ok() {
                stats.reused += 1;
            } else {
                if let Some(parent) = dst.parent() {
                    util::create_dir_all(parent)?;
                }
                match reflink_copy::reflink_or_copy(util::extended(path), util::extended(&dst)) {
                    Ok(_) => stats.hashed += 1,
                    Err(source) => return Err(SnapshotError::io("cas object", source)),
                }
            }
            files.insert(rel.clone(), hash);
        }
        let meta = SnapshotMeta {
            session: session.as_str().to_owned(),
            turn: turn.get(),
            head_at_snapshot: None,
            files,
            executables: walked.executables,
            submodules: walked
                .gitlinks
                .iter()
                .map(|g| SubmoduleRecord {
                    path: g.rel.clone(),
                    commit_hex: g.commit_hex.clone(),
                    dirty: g.dirty.clone(),
                })
                .collect(),
            filter_managed: walked.filter_managed,
            stat_fingerprint: crate::coordinator::DirtyCheck::fingerprint(&workdir),
            task: opts.task.clone(),
            signature: None::<DetachedSignature>,
            recorded_unix: util::now_unix(),
        };
        // Manifest written LAST: a crash leaves unreferenced objects, never
        // a lying manifest.
        let path = self.manifest_path(session, turn);
        if let Some(parent) = path.parent() {
            util::create_dir_all(parent)?;
        }
        util::write_file(&path, util::to_json(&meta)?.as_bytes())?;
        Ok(SnapshotOutcome {
            session: session.clone(),
            turn,
            ref_name: None,
            manifest_path: Some(path),
            stats,
        })
    }

    pub(crate) fn turns(&self, session: &SessionId) -> Vec<Turn> {
        let Ok(children) = util::read_dir_sorted(&self.session_dir(session).join(MANIFESTS)) else {
            return Vec::new();
        };
        children
            .iter()
            .filter_map(|path| {
                let stem = path.file_stem()?.to_str()?;
                let n = stem.parse::<u32>().ok()?;
                Turn::new(n).ok()
            })
            .collect()
    }

    pub(crate) fn meta(
        &self,
        session: &SessionId,
        turn: Turn,
    ) -> Result<SnapshotMeta, SnapshotError> {
        let path = self.manifest_path(session, turn);
        if !path.symlink_metadata().is_ok() {
            return Err(SnapshotError::MissingSnapshot {
                session: session.as_str().to_owned(),
                turn: turn.get(),
            });
        }
        let bytes = util::read_file(&path)?;
        let meta: SnapshotMeta = parse_tolerant(&path, &bytes)?;
        if meta.session != session.as_str() || meta.turn != turn.get() {
            return Err(SnapshotError::CorruptSnapshot { path });
        }
        Ok(meta)
    }

    pub(crate) fn read_object(
        &self,
        session: &SessionId,
        hash: &str,
    ) -> Result<Vec<u8>, SnapshotError> {
        util::read_file(&self.object_path(session, hash))
    }

    pub(crate) fn delete_manifest(
        &self,
        session: &SessionId,
        turn: Turn,
    ) -> Result<(), SnapshotError> {
        let path = self.manifest_path(session, turn);
        if path.symlink_metadata().is_ok() {
            util::remove_file(&path)?;
        }
        Ok(())
    }

    /// Objects still referenced by any remaining manifest of the session.
    pub(crate) fn live_objects(&self, session: &SessionId) -> Result<Vec<String>, SnapshotError> {
        let mut live = Vec::new();
        for turn in self.turns(session) {
            live.extend(self.meta(session, turn)?.files.into_values());
        }
        Ok(live)
    }

    pub(crate) fn sweep_objects(
        &self,
        session: &SessionId,
        live: &[String],
    ) -> Result<usize, SnapshotError> {
        let objects_dir = self.session_dir(session).join(OBJECTS);
        let Ok(buckets) = util::read_dir_sorted(&objects_dir) else {
            return Ok(0);
        };
        let mut removed = 0;
        for bucket in buckets {
            for object in util::read_dir_sorted(&bucket)? {
                let name = object
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !live.iter().any(|hash| hash == &name) {
                    util::remove_file(&object)?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    /// Dirty-state stash for the fallback: CAS objects (shared with the
    /// session store) + a manifest under `.harness/restore-transactions/
    /// <uuid>/`. No refs exist in this backend; the manifest is the
    /// transaction record.
    pub(crate) fn stash(
        &self,
        session: &SessionId,
        tx: &crate::ids::TxId,
        workdir: &Path,
    ) -> Result<String, SnapshotError> {
        let walked = walk::walk(workdir, &[])?;
        let mut files = BTreeMap::new();
        for (rel, path) in &walked.files {
            let bytes = util::read_file(path)?;
            let hash = blake3::hash(&bytes).to_hex().to_string();
            let dst = self.object_path(session, &hash);
            if !dst.symlink_metadata().is_ok() {
                if let Some(parent) = dst.parent() {
                    util::create_dir_all(parent)?;
                }
                reflink_copy::reflink_or_copy(util::extended(path), util::extended(&dst))
                    .map_err(|source| SnapshotError::io("stash object", source))?;
            }
            files.insert(rel.clone(), hash);
        }
        let rel_dir = format!(".harness/restore-transactions/{}", tx.0);
        let dir = workdir.join(&rel_dir);
        util::create_dir_all(&dir)?;
        let stash = crate::manifest::SnapshotMeta {
            session: session.as_str().to_owned(),
            recorded_unix: util::now_unix(),
            files,
            ..SnapshotMeta::default()
        };
        util::write_file(&dir.join("stash.json"), util::to_json(&stash)?.as_bytes())?;
        Ok(rel_dir)
    }
}

/// Torn (truncated write) vs corrupt (wrong shape): a serde EOF error is
/// a torn write; anything else is a well-formed wrong-shaped document.
pub(crate) fn parse_tolerant<T: serde::de::DeserializeOwned>(
    path: &Path,
    bytes: &[u8],
) -> Result<T, SnapshotError> {
    match serde_json::from_slice(bytes) {
        Ok(value) => Ok(value),
        Err(source) if source.classify() == serde_json::error::Category::Eof => {
            Err(SnapshotError::TornSnapshot {
                path: path.to_path_buf(),
            })
        }
        Err(_) => Err(SnapshotError::CorruptSnapshot {
            path: path.to_path_buf(),
        }),
    }
}
