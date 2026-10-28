//! The backend façade: git-backed snapshots when the project is a
//! repository, content-addressed file snapshots otherwise. Capability
//! flags surface what each backend can honestly provide.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::checkpoint::CheckpointWriter;
use crate::gitstore::GitStore;
use crate::ids::{SessionId, Turn};
use crate::manifest::{
    ExternallyModifiedPolicy, RestoreMode, SnapshotCapabilities, SnapshotMeta, SnapshotOptions,
    SnapshotOutcome,
};
use crate::nogit::NoGitStore;
use crate::restore::{ContentSource, PlanInputs, RewindPlan, build_plan};
use crate::walk;
use crate::{SnapshotError, util};

use gix::bstr::ByteSlice as _;

pub(crate) enum Backend {
    Git(Box<GitStore>),
    NoGit(NoGitStore),
}

pub struct SnapshotStore {
    pub(crate) inner: Backend,
    workdir: PathBuf,
}

impl SnapshotStore {
    pub fn open(workdir: &Path) -> Result<Self, SnapshotError> {
        let inner = if gix::discover(workdir).is_ok() {
            Backend::Git(Box::new(GitStore::open(workdir)?))
        } else {
            Backend::NoGit(NoGitStore::new(workdir))
        };
        Ok(Self {
            inner,
            workdir: workdir.to_path_buf(),
        })
    }

    #[must_use]
    pub fn backend_name(&self) -> &'static str {
        match &self.inner {
            Backend::Git(_) => "git",
            Backend::NoGit(_) => "nogit",
        }
    }

    #[must_use]
    pub fn capabilities(&self) -> SnapshotCapabilities {
        match &self.inner {
            Backend::Git(_) => SnapshotCapabilities {
                refs: true,
                provenance: true,
            },
            Backend::NoGit(_) => SnapshotCapabilities {
                refs: false,
                provenance: false,
            },
        }
    }

    pub(crate) fn backend(&self) -> &Backend {
        &self.inner
    }

    #[must_use]
    pub(crate) fn workdir(&self) -> &Path {
        &self.workdir
    }

    /// The persistent scratch stat-cache root for this store's workdir
    /// (`<workdir>/.austringer/scratch`): the maintenance seam hosts
    /// call [`crate::scratch::reap_scratch`] on — the same root the
    /// backends write their per-session indices under.
    #[must_use]
    pub fn scratch_root(&self) -> PathBuf {
        self.workdir.join(".austringer").join("scratch")
    }

    pub fn snapshot(
        &self,
        session: &SessionId,
        turn: Turn,
        opts: &SnapshotOptions,
    ) -> Result<SnapshotOutcome, SnapshotError> {
        match &self.inner {
            Backend::Git(git) => git.snapshot(session, turn, opts),
            Backend::NoGit(nogit) => nogit.snapshot(session, turn, opts),
        }
    }

    pub fn snapshot_meta(
        &self,
        session: &SessionId,
        turn: Turn,
    ) -> Result<SnapshotMeta, SnapshotError> {
        self.meta_of(session, turn)
    }

    pub(crate) fn meta_of(
        &self,
        session: &SessionId,
        turn: Turn,
    ) -> Result<SnapshotMeta, SnapshotError> {
        match &self.inner {
            Backend::Git(git) => git.meta_required(session, turn),
            Backend::NoGit(nogit) => nogit.meta(session, turn),
        }
    }

    pub(crate) fn sessions(&self) -> Result<Vec<SessionId>, SnapshotError> {
        let mut sessions = Vec::new();
        match &self.inner {
            Backend::Git(git) => {
                let prefix = "refs/austringer/snapshots/";
                let refs = git
                    .repo
                    .references()
                    .map_err(|source| SnapshotError::git("list refs", source))?;
                let iter = refs
                    .all()
                    .map_err(|source| SnapshotError::git("iterate refs", source))?;
                for reference in iter {
                    let reference = reference
                        .map_err(|source| SnapshotError::git_boxed("ref entry", source))?;
                    let name = reference.name().as_bstr().to_str_lossy().into_owned();
                    if let Some(id) = name
                        .strip_prefix(prefix)
                        .and_then(|rest| rest.split('/').next())
                        .and_then(|session| SessionId::new(session).ok())
                        .filter(|id| !sessions.contains(id))
                    {
                        sessions.push(id);
                    }
                }
            }
            Backend::NoGit(nogit) => {
                if let Ok(children) = util::read_dir_sorted(&nogit.root) {
                    for child in children {
                        if let Some(id) = child
                            .file_name()
                            .and_then(|n| n.to_str())
                            .and_then(|name| SessionId::new(name).ok())
                        {
                            sessions.push(id);
                        }
                    }
                }
            }
        }
        Ok(sessions)
    }

    /// All turns with a recorded snapshot for `session`, ascending.
    /// The `/rewind` bridge (plan todo 5) walks this list to resolve
    /// `steps` into a target restore point.
    ///
    /// # Errors
    /// Git backends: typed ref-iteration failure.
    pub fn turns_of(&self, session: &SessionId) -> Result<Vec<Turn>, SnapshotError> {
        match &self.inner {
            Backend::Git(git) => git.turns(session),
            Backend::NoGit(nogit) => Ok(nogit.turns(session)),
        }
    }

    pub fn rewind_plan(
        &self,
        session_root: &Path,
        session: &SessionId,
        turn: Turn,
        mode: RestoreMode,
        policy: ExternallyModifiedPolicy,
    ) -> Result<RewindPlan, SnapshotError> {
        let meta = self.meta_of(session, turn)?;
        let current = self.current_hashes()?;
        let baseline = match &self.inner {
            Backend::Git(git) => git
                .latest_turn(session)?
                .and_then(|latest| git.meta(session, latest)),
            Backend::NoGit(nogit) => nogit
                .turns(session)
                .pop()
                .and_then(|latest| nogit.meta(session, latest).ok()),
        };
        let snapshot_files = self.snapshot_sources(session, turn, &meta)?;
        if let Backend::Git(git) = &self.inner {
            let user_commits = git.user_commits_after(meta.head_at_snapshot.as_deref())?;
            if !user_commits.is_empty() {
                return Err(SnapshotError::UserCommitsAfterSnapshot {
                    commits: user_commits,
                });
            }
        }
        let through = CheckpointWriter::new(session_root)
            .ok()
            .and_then(|writer| writer.through_at(turn));
        Ok(build_plan(PlanInputs {
            session,
            turn,
            mode,
            policy,
            meta: &meta,
            snapshot_files: &snapshot_files,
            current: &current,
            baseline: baseline.as_ref(),
            through,
        }))
    }

    fn snapshot_sources(
        &self,
        session: &SessionId,
        turn: Turn,
        meta: &SnapshotMeta,
    ) -> Result<BTreeMap<String, (ContentSource, bool)>, SnapshotError> {
        let mut sources = BTreeMap::new();
        match &self.inner {
            Backend::Git(git) => {
                for (rel, (oid_hex, mode)) in git.snapshot_tree(session, turn)? {
                    sources.insert(rel, (ContentSource::Blob { oid_hex }, mode == 0o100755));
                }
            }
            Backend::NoGit(_) => {
                for (rel, hash) in &meta.files {
                    sources.insert(
                        rel.clone(),
                        (
                            ContentSource::Cas {
                                hash_hex: hash.clone(),
                            },
                            meta.executables.contains(rel),
                        ),
                    );
                }
            }
        }
        Ok(sources)
    }

    fn current_hashes(&self) -> Result<BTreeMap<String, String>, SnapshotError> {
        let output = walk::walk(&self.workdir, &[])?;
        let mut hashes = BTreeMap::new();
        for (rel, path) in output.files {
            hashes.insert(
                rel,
                blake3::hash(&util::read_file(&path)?).to_hex().to_string(),
            );
        }
        Ok(hashes)
    }
}
