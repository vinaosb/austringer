//! The MCP schema disk cache (todo 29, jcode pattern): the last-seen
//! tool schemas of each server live on disk keyed by the server's CONFIG
//! FINGERPRINT, are ADVERTISED AT SPAWN (a warm spawn hands out the disk
//! set without waiting on a fresh `tools/list` round-trip — zero
//! first-snapshot cache miss), and are RECONCILED against the live
//! `tools/list` (drift updates the cache and surfaces a typed notice).

use std::path::{Path, PathBuf};

use crate::client::{McpClient, ToolDescriptor};
use crate::config::Fingerprint;
use crate::error::McpClientError;

/// Cached-vs-live reconciliation outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaDrift {
    /// The disk cache matches the live tool set.
    InSync,
    /// The sets differ; lists what changed (name-level).
    Drifted {
        /// Live tools the cache lacked.
        added: Vec<String>,
        /// Cached tools the live set no longer has.
        removed: Vec<String>,
    },
}

/// The result of one spawn-with-cache cycle.
#[derive(Debug, Clone)]
pub struct SpawnSchemas {
    /// What was advertised BEFORE the live list returned (`None` on a
    /// cold cache — the first spawn must fetch).
    pub advertised: Option<Vec<ToolDescriptor>>,
    /// The reconciled (live) set; persisted to disk.
    pub tools: Vec<ToolDescriptor>,
    /// The typed drift notice for this spawn.
    pub drift: SchemaDrift,
}

/// File-backed cache: one JSON file per fingerprint under a directory.
#[derive(Debug, Clone)]
pub struct SchemaDiskCache {
    dir: PathBuf,
}

impl SchemaDiskCache {
    /// Opens (creates) the cache directory.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    fn path_for(&self, fingerprint: &Fingerprint) -> PathBuf {
        self.dir.join(format!("{}.json", fingerprint.0))
    }

    /// Loads the cached tool set for a fingerprint; corrupt or missing
    /// entries degrade to `None` (never a panic, never a stale surprise).
    #[must_use]
    pub fn load(&self, fingerprint: &Fingerprint) -> Option<Vec<ToolDescriptor>> {
        let bytes = std::fs::read(self.path_for(fingerprint)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Persists the tool set for a fingerprint (tmp + rename: a torn
    /// write can never corrupt the previous entry).
    pub fn store(
        &self,
        fingerprint: &Fingerprint,
        tools: &[ToolDescriptor],
    ) -> std::io::Result<()> {
        let final_path = self.path_for(fingerprint);
        let tmp_path = self.dir.join(format!(
            "{}.tmp",
            final_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
        let bytes = serde_json::to_vec(tools).map_err(|error| {
            std::io::Error::other(format!("schema serialization failed: {error}"))
        })?;
        std::fs::write(&tmp_path, bytes)?;
        std::fs::rename(&tmp_path, &final_path)
    }

    /// Computes drift between a cached set and the live set.
    #[must_use]
    pub fn reconcile_sets(
        cached: Option<Vec<ToolDescriptor>>,
        live: &[ToolDescriptor],
    ) -> SchemaDrift {
        let Some(cached) = cached else {
            return SchemaDrift::InSync;
        };
        let mut added = Vec::new();
        let mut removed = Vec::new();
        for tool in live {
            if !cached.iter().any(|candidate| candidate.name == tool.name) {
                added.push(tool.name.clone());
            }
        }
        for tool in &cached {
            if !live.iter().any(|candidate| candidate.name == tool.name) {
                removed.push(tool.name.clone());
            }
        }
        if added.is_empty() && removed.is_empty() {
            SchemaDrift::InSync
        } else {
            SchemaDrift::Drifted { added, removed }
        }
    }
}

/// The spawn-with-cache cycle: advertise the disk set immediately, then
/// reconcile against the live `tools/list` (one round-trip), update the
/// cache, and return the typed drift notice. Cache-write failures are
/// logged (tracing) but never fail the spawn — a read-only cache
/// directory degrades to cold spawns.
pub async fn spawn_advertised(
    cache: &SchemaDiskCache,
    client: &McpClient,
) -> Result<SpawnSchemas, McpClientError> {
    let fingerprint = client.entry().fingerprint();
    let cached = cache.load(&fingerprint);
    let advertised = cached.clone();
    let live = client.list_tools().await?;
    let drift = SchemaDiskCache::reconcile_sets(cached, &live);
    if let Err(error) = cache.store(&fingerprint, &live) {
        tracing::warn!(
            target: "austringer_mcp::schema_cache",
            fingerprint = %fingerprint,
            %error,
            "schema cache write failed (degrading to cold spawns)"
        );
    }
    Ok(SpawnSchemas {
        advertised,
        tools: live,
        drift,
    })
}
