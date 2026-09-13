//! Pack registry: verified pack distribution (todo 32).
//!
//! * **Protocol** ([`protocol`]): a typed, static-index registry format —
//!   `index.json` + content-addressed `packs/<blake3>.hpk` — behind the
//!   [`RegistryStore`] transport seam. The DEFAULT transport is
//!   [`LocalDirStore`] over a local directory: this crate contains ZERO
//!   network code, so the default install path is STRUCTURALLY offline
//!   (no phoning home). The daemon later wires an HTTP-backed store over
//!   the same layout.
//! * **Reference server** ([`store`]): [`build_registry_from_packs_dir`]
//!   deterministically builds the static registry from the shipped T33
//!   packs tree; [`add_pack`] publishes marketplace items.
//! * **Marketplace items are declarative config patches** (Roo §10): a
//!   [`PackFile`] carries files/rules/MCP entries + prompted secret
//!   DECLARATIONS. Items are never executable code; secret values
//!   resolve through the [`SecretSource`] seam and never enter config
//!   files (T29 templating discipline).
//! * **Bundles** ([`bundles`], Devin §22): skills+rules+MCP+subagents as
//!   ONE unit with pinned plugins, glob-deny, and governance (higher
//!   authority wins, forbid-`*` lockdown).
//! * **Ledger** ([`lock`], hermes §27): installed provenance as
//!   content-hash pins in `registry-lock.json` — tampering detectable
//!   ([`RegistryManager::verify`]), upgrades diffable
//!   ([`RegistryManager::diff_disk`], [`RegistryLedger::diff`]).
//! * **Signatures** ([`signatures`]): keyed-blake3 pack signatures with
//!   a [`SignatureVerifier`] seam for asymmetric upgrades in the daemon.
//!
//! Untrusted input discipline: registry indexes and pack files are
//! parsed behind size budgets and validated field-by-field (names,
//! versions, pins, paths, globs); nothing is ever evaluated.

mod bundles;
mod drift;
mod error;
mod governance;
mod install;
mod lock;
mod manager;
mod packfile;
mod protocol;
mod signatures;
mod store;
mod writes;

pub use bundles::{BundleCheck, BundleManifest, BundlePlugin, Requirement, check_bundle};
pub use drift::{ChangedEntry, Drift, FileChange, RegistryDiff};
pub use error::RegistryError;
pub use governance::{Authority, GovernancePolicy, GovernanceStack, PluginDecision, glob_match};
pub use install::{InstallOptions, InstallReceipt, MapSecretSource, Scope, SecretSource};
pub use lock::{ItemKind, LEDGER_SCHEMA, LedgerEntry, RegistryLedger};
pub use manager::{RegistryManager, RemoveReceipt};
pub use packfile::{
    MAX_PACK_BYTES, PackFile, SecretParam, validate_glob, validate_hash_hex, validate_item_name,
    validate_pin, validate_secret_name, validate_version,
};
pub use protocol::{
    INDEX_SCHEMA, MAX_INDEX_BYTES, RegistryEntry, RegistryIndex, RegistryStore, ReviewAcceptance,
    ReviewTicket, blake3_hex,
};
pub use signatures::{Blake3Verifier, SecretKey, SignatureVerifier, sign_pack};
pub use store::{LocalDirStore, add_pack, build_registry_from_packs_dir};
