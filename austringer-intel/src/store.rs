//! The local profile store: per-model records under a directory, one JSON
//! file per model, written with deterministic struct serialization. Each
//! [`ProfileStore::update`] profiles the observation window it is handed
//! and REPLACES the stored record — reloads reflect the new window, never
//! an average with stale history. The store performs local file I/O only;
//! profile data never leaves the machine (plan todo 34: "Must NOT share
//! data externally" — the crate also has no network-capable dependency).

use std::path::PathBuf;

use austringer_providers::identity::ModelIdentity;
use serde::{Deserialize, Serialize};

use super::axes::{self, BehaviorAxes};
use super::taxonomy::{self, FailureTaxonomy, TurnObservation};

/// Everything the store can fail with.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Local I/O failure.
    #[error("profile store io: {0}")]
    Io(String),
    /// A store file exists but does not deserialize.
    #[error("profile store file corrupt ({path}): {detail}")]
    Corrupt {
        /// The store file name.
        path: String,
        /// What was wrong with it.
        detail: String,
    },
}

impl StoreError {
    fn io(context: &str, err: &std::io::Error) -> Self {
        Self::Io(format!("{context}: {err}"))
    }
}

/// One stored profile record (only ever written with sufficient data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredProfile {
    /// The model the record describes.
    pub model: ModelIdentity,
    /// The taxonomy computed over the stored window.
    pub taxonomy: FailureTaxonomy,
    /// The axes computed over the stored window.
    pub axes: BehaviorAxes,
    /// Turns in the stored window.
    pub observation_turns: u32,
    /// When the record was written (caller clock, unix seconds).
    pub updated_unix: u64,
}

/// Directory-backed profile store: one `<provider>.<model>.json` per model.
#[derive(Debug, Clone)]
pub struct ProfileStore {
    root: PathBuf,
}

impl ProfileStore {
    /// A store rooted at `root` (created on first write).
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The file one model's record lives in.
    #[must_use]
    pub fn file_for(&self, identity: &ModelIdentity) -> PathBuf {
        self.root
            .join(format!("{}.{}.json", identity.provider, identity.model))
    }

    /// Profiles the observation window for `model` and persists the
    /// record. Returns `Ok(None)` — writing NOTHING — when the window is
    /// under the data floor: a thin-data record would mislead a later
    /// reader more than no record at all.
    pub fn update(
        &self,
        model: &ModelIdentity,
        observations: &[TurnObservation],
        now_unix: u64,
    ) -> Result<Option<StoredProfile>, StoreError> {
        let Some(taxonomy) = taxonomy::compute_taxonomy(observations, model) else {
            return Ok(None);
        };
        // The axes gate is the same floor over the same observations, so a
        // stored record always carries both halves.
        let Some(axes) = axes::compute_axes(observations, model) else {
            return Ok(None);
        };
        let record = StoredProfile {
            observation_turns: taxonomy.turns,
            model: model.clone(),
            taxonomy,
            axes,
            updated_unix: now_unix,
        };
        std::fs::create_dir_all(&self.root).map_err(|e| StoreError::io("create root", &e))?;
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| StoreError::Corrupt {
            path: self.file_for(model).display().to_string(),
            detail: e.to_string(),
        })?;
        std::fs::write(self.file_for(model), bytes)
            .map_err(|e| StoreError::io("write record", &e))?;
        Ok(Some(record))
    }

    /// Loads one model's record; `Ok(None)` when none is stored.
    pub fn get(&self, model: &ModelIdentity) -> Result<Option<StoredProfile>, StoreError> {
        let path = self.file_for(model);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(StoreError::io("read record", &e)),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| StoreError::Corrupt {
                path: path.display().to_string(),
                detail: e.to_string(),
            })
    }
}
