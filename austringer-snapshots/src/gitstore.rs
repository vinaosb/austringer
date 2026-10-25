//! Git backend: gix IN-MEMORY tree/commit objects written straight to the
//! object database + namespaced refs. Nothing here touches the index, the
//! working tree, or any user branch — the only `.git` writes are refs
//! under `refs/austringer/snapshots/{session}/` and
//! `refs/austringer/restore-transactions/{uuid}`. Ref/tree READ accessors
//! live in [`crate::gitrefs`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ids::{SessionId, Turn, manifest_ref, snapshot_ref};
use crate::manifest::{
    SnapshotMeta, SnapshotOptions, SnapshotOutcome, StatCacheStats, SubmoduleRecord,
};
use crate::provenance::{provenance_message, sign_commit};
use crate::scratch::{ScratchIndex, ScratchStat};
use crate::tree::{TreeNode, austringer_signature, insert_tree, parse_oid, write_tree};
use crate::walk;
use crate::{SnapshotError, util};

pub(crate) struct GitStore {
    pub repo: gix::Repository,
    pub workdir: PathBuf,
}

impl GitStore {
    pub(crate) fn open(workdir: &Path) -> Result<Self, SnapshotError> {
        let repo =
            gix::discover(workdir).map_err(|source| SnapshotError::git("discover", source))?;
        let root = repo
            .workdir()
            .ok_or_else(|| SnapshotError::NotARepository {
                workdir: workdir.to_path_buf(),
            })?
            .to_path_buf();
        Ok(Self {
            repo: gix::open(&root).map_err(|source| SnapshotError::git("open", source))?,
            workdir: root,
        })
    }

    fn scratch_dir(&self, session: &SessionId) -> PathBuf {
        self.workdir
            .join(".austringer")
            .join("scratch")
            .join(session.as_str())
    }

    pub(crate) fn snapshot(
        &self,
        session: &SessionId,
        turn: Turn,
        opts: &SnapshotOptions,
    ) -> Result<SnapshotOutcome, SnapshotError> {
        let walked = walk::walk(&self.workdir, &opts.extra_excludes)?;
        let scratch_dir = self.scratch_dir(session);
        let mut index = ScratchIndex::open(&scratch_dir)?;
        // The latest manifest backs the stat-cache: a reused oid needs its
        // recorded blake3 hash without re-reading the file.
        let prev_meta = self
            .latest_turn(session)?
            .and_then(|t| self.meta(session, t));
        let mut stats = StatCacheStats::default();
        let mut root: BTreeMap<String, TreeNode> = BTreeMap::new();
        let mut files: BTreeMap<String, String> = BTreeMap::new();

        for (rel, path) in &walked.files {
            let (mtime_secs, mtime_nanos, size) = util::file_stat(path)?;
            let stat = ScratchStat {
                mtime_secs,
                mtime_nanos,
                size,
            };
            let cached = index.lookup(rel, &stat).filter(|_| {
                prev_meta
                    .as_ref()
                    .is_some_and(|m| m.files.contains_key(rel))
            });
            let oid_hex = match cached {
                Some(hit) => {
                    stats.reused += 1;
                    files.insert(
                        rel.clone(),
                        prev_meta
                            .as_ref()
                            .and_then(|m| m.files.get(rel))
                            .cloned()
                            .unwrap_or_default(),
                    );
                    hit
                }
                None => {
                    stats.hashed += 1;
                    let bytes = util::read_file(path)?;
                    let oid = self
                        .repo
                        .write_blob(bytes.as_slice())
                        .map_err(|source| SnapshotError::git("write blob", source))?
                        .detach();
                    let hex = oid.to_hex().to_string();
                    index.record(rel, stat, &hex);
                    files.insert(rel.clone(), blake3::hash(&bytes).to_hex().to_string());
                    hex
                }
            };
            let exec = walked.executables.iter().any(|e| e == rel);
            insert_tree(
                &mut root,
                rel,
                TreeNode::Blob {
                    oid: parse_oid(&oid_hex),
                    exec,
                },
            );
        }
        for link in &walked.gitlinks {
            insert_tree(
                &mut root,
                &link.rel,
                TreeNode::Commit(parse_oid(&link.commit_hex)),
            );
        }

        let tree_id = write_tree(&self.repo, &root)?;
        let head = self
            .repo
            .head_commit()
            .ok()
            .map(|c| c.id.to_hex().to_string());
        let commit = gix::objs::Commit {
            message: provenance_message(session, turn).into(),
            tree: tree_id,
            parents: head
                .as_ref()
                .map(|hex| parse_oid(hex))
                .into_iter()
                .collect(),
            author: austringer_signature(),
            committer: austringer_signature(),
            encoding: None,
            extra_headers: Vec::new(),
        };
        let commit_id = self
            .repo
            .write_object(&commit)
            .map_err(|source| SnapshotError::git("write commit", source))?
            .detach();
        let signature = sign_commit(opts.signer.as_ref(), &commit)?;

        let meta = SnapshotMeta {
            session: session.as_str().to_owned(),
            turn: turn.get(),
            head_at_snapshot: head,
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
            stat_fingerprint: crate::coordinator::DirtyCheck::fingerprint(&self.workdir),
            task: opts.task.clone(),
            signature,
            recorded_unix: util::now_unix(),
        };
        let meta_blob = self
            .repo
            .write_blob(util::to_json(&meta)?.as_bytes())
            .map_err(|source| SnapshotError::git("write meta blob", source))?
            .detach();
        let name = snapshot_ref(session, turn);
        self.repo
            .reference(
                name.as_str(),
                commit_id,
                gix::refs::transaction::PreviousValue::Any,
                "austringer-snapshots: workspace snapshot",
            )
            .map_err(|source| SnapshotError::git("snapshot ref", source))?;
        self.repo
            .reference(
                manifest_ref(session, turn).as_str(),
                meta_blob,
                gix::refs::transaction::PreviousValue::Any,
                "austringer-snapshots: snapshot manifest",
            )
            .map_err(|source| SnapshotError::git("manifest ref", source))?;
        index.save(&scratch_dir)?;
        Ok(SnapshotOutcome {
            session: session.clone(),
            turn,
            ref_name: Some(name),
            manifest_path: None,
            stats,
        })
    }
}
