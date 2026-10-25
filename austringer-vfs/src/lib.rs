//! Virtual filesystem and transactional shadow workspaces (plan todo 21).
//!
//! A *shadow* is an isolated mirror of a target directory the agent can
//! edit and verify inside before anything reaches the user's tree
//! (Cursor-style shadow workspaces; pre-flight lives in todo 22). The
//! transaction contract is structural:
//!
//! * [`ShadowHandle::apply`] writes only inside the shadow. No public API
//!   other than [`ShadowHandle::commit`] writes to the user tree.
//! * [`ShadowHandle::commit`] verifies the user tree still matches the
//!   shadow base (typed [`VfsError::BaseDrift`] otherwise), backs up every
//!   replace-target in memory, copies the edits up, and rolls the whole
//!   batch back if any write fails ([`VfsError::CommitFailed`] carries
//!   whether the restore completed).
//! * Dropping (or [`ShadowHandle::discard`]-ing) an active shadow is the
//!   instant rollback: the user tree was never touched. Retired shadows
//!   move to the store's reclaimed area for one [`gc::GcPolicy`] grace
//!   cycle before deletion, with reclaimed-ref bookkeeping for gix-backed
//!   ones (`refs/austringer/reclaimed/<shadow-id>/<commit>`).
//!
//! # Backings and disclosure
//!
//! [`ShadowStore::create_shadow`] picks, in order: per-file **reflink**
//! (Linux FICLONE, macOS APFS clonefile, Windows ReFS/Dev Drive block
//! cloning — copy-on-write by construction), the **gix committed-tree**
//! backing (the HEAD subtree materialized through gitoxide's object
//! database; only kept when it matches the user tree byte-for-byte, so a
//! dirty worktree or smudge-filter divergence demotes it), and finally a
//! **plain full copy**. `reflink_or_copy` degrades to a copy per file
//! silently, so the first non-CoW result aborts that backing — the handle
//! always discloses what it actually got via [`store::Backing::name`].
//! gix 0.87 has no linked-worktree *creation*, so the fallback materializes
//! the committed tree itself; it does not apply smudge filters (repos with
//! CRLF-converting attributes never select it: the self-check fails and
//! the store falls back to a full copy).
//!
//! # Edit mapping (austringer-tools, plan todo 16)
//!
//! The editing tools stay line-anchored; this layer is whole-file
//! transactional. A `austringer_tools::edit::LineEdit` batch applied through
//! `austringer_tools::edit::apply_edits` yields the post-edit
//! `austringer_tools::lines::LineTable`; send
//! [`edit::ShadowEdit::replace`]`(path, blake3(pre-edit bytes),
//! table.as_bytes())`. The expected hash is the drift contract: content
//! that moved between the tool's read and this layer's write is a typed
//! [`VfsError::HashMismatch`], never a shifted-content application.
//! `ShadowEdit::create` asserts absence and creates a new file.
//!
//! # Policies
//!
//! * Paths in edits are shadow-relative and lexically validated: absolute,
//!   UNC/verbatim (`\\?\`), and `..` forms are typed rejections. All
//!   internal I/O uses the extended-length path form, so >260-char trees
//!   work on Windows regardless of the OS long-path policy.
//! * Symlinks in the target tree are skipped everywhere (manifests,
//!   shadows, commits); edits that would land on one surface as
//!   [`VfsError::BaseDrift`] at commit time. Line endings and encodings
//!   pass through unmunged.
//! * User-tree writes are exactly the applied edits: files the user added
//!   elsewhere after shadow creation do not block a commit; anything that
//!   was modified, deleted, or appeared where the transaction writes does.

pub(crate) mod commit;
pub(crate) mod gc;
pub(crate) mod manifest;
pub(crate) mod paths;
pub(crate) mod reflink;
pub(crate) mod revise;
pub(crate) mod shadow;
pub(crate) mod store;
pub(crate) mod worktree;

pub mod edit;
pub mod error;

pub use edit::ShadowEdit;
pub use error::{DriftKind, FileHash, PathRejection, VfsError};
pub use gc::{GcPolicy, GcReport};
pub use shadow::{ApplyStats, CommitStats, ShadowHandle};
pub use store::{Backing, BackingChoice, ShadowStore};
