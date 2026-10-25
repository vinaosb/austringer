//! Declarative automations (terraform-style): schedules are plain
//! persisted definitions, validated on registration and reload. A
//! draft-tier (or unknown) pack is NEVER schedulable — the gate fails
//! closed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::scheduler::{CronError, CronExpr, JobAction, ScheduledJob};

/// Network reachability an automation is allowed; the effective policy
/// of a run is the intersection (offline wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkPolicy {
    /// No network egress (tools run under offline grants).
    Offline,
    /// Network permitted.
    Online,
}

impl NetworkPolicy {
    /// Intersection: offline in either side wins.
    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        if matches!(self, Self::Offline) || matches!(other, Self::Offline) {
            Self::Offline
        } else {
            Self::Online
        }
    }
}

/// Trust tier of a schedulable pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackTier {
    /// Reviewed and approved for unattended scheduling.
    Approved,
    /// Draft/unvetted: never schedulable (plan guardrail).
    Draft,
}

/// Answers whether a pack is schedulable. The production
/// implementation reads a tiers file; skills/MCP integrations compose
/// through this seam.
pub trait PackGate: Send + Sync {
    /// Tier of `pack`; unknown packs must report [`PackTier::Draft`].
    fn tier(&self, pack: &str) -> PackTier;
}

/// In-memory gate (tests, embedders).
pub struct FixedPackGate {
    tiers: BTreeMap<String, PackTier>,
}

impl FixedPackGate {
    /// Gate over an explicit tier list.
    #[must_use]
    pub fn new(tiers: Vec<(String, PackTier)>) -> Self {
        Self {
            tiers: tiers.into_iter().collect(),
        }
    }
}

impl PackGate for FixedPackGate {
    fn tier(&self, pack: &str) -> PackTier {
        self.tiers.get(pack).copied().unwrap_or(PackTier::Draft)
    }
}

/// File-backed gate over `{data}/pack-tiers.json` (`name -> tier`).
/// A missing file means every pack is draft: fail closed.
pub struct FilePackGate {
    tiers: BTreeMap<String, PackTier>,
}

impl FilePackGate {
    /// Load the tiers file (absent or unreadable = all draft).
    ///
    /// # Errors
    /// malformed JSON.
    pub fn open(path: &Path) -> Result<Self, AutomationError> {
        let tiers = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Ok(Self { tiers })
    }
}

impl PackGate for FilePackGate {
    fn tier(&self, pack: &str) -> PackTier {
        self.tiers.get(pack).copied().unwrap_or(PackTier::Draft)
    }
}

/// One declarative automation (terraform-style definition persisted in
/// `automations.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationDefinition {
    /// Stable id (also the cron job id).
    pub id: String,
    /// Human label.
    pub name: String,
    /// Five-field cron expression (UTC).
    pub cron: String,
    /// Pack to run.
    pub pack: String,
    /// Goal text handed to the kicked-off session.
    pub goal: Option<String>,
    /// Goal criteria the completion gate reviews against.
    pub criteria: Vec<String>,
    /// Per-run turn budget (loop `max_turns`).
    pub budget_turns: Option<u32>,
    /// Lifetime invocation cap.
    pub max_invocations_total: Option<u64>,
    /// Per-day (UTC) invocation cap.
    pub max_invocations_per_day: Option<u64>,
    /// Network policy the automation requires.
    pub network: NetworkPolicy,
    /// Disabled definitions load but never schedule.
    pub enabled: bool,
}

/// Validation/persistence failures.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AutomationError {
    /// Malformed cron expression.
    #[error("automation {def:?}: bad cron: {source}")]
    BadCron {
        /// Offending definition id.
        def: String,
        /// Parse failure.
        source: CronError,
    },
    /// Pack is not approved-tier.
    #[error("pack {pack:?} is draft-tier; only approved packs are schedulable")]
    DraftTier {
        /// Rejected pack.
        pack: String,
    },
    /// Duplicate automation id.
    #[error("automation {id:?} already registered")]
    Duplicate {
        /// Duplicated id.
        id: String,
    },
    /// Store I/O or parse failure.
    #[error("automation store io: {0}")]
    Io(String),
}

/// The persisted definition set: `automations.json` under the data
/// root, revalidated on every load.
pub struct AutomationStore {
    root: PathBuf,
    defs: Vec<AutomationDefinition>,
}

impl AutomationStore {
    /// Load (and revalidate) the store; a missing file is an empty
    /// store, a present-but-invalid one is a typed error.
    ///
    /// # Errors
    /// see [`AutomationError`].
    pub fn load(root: &Path, gate: &dyn PackGate) -> Result<Self, AutomationError> {
        let path = root.join("automations.json");
        let defs: Vec<AutomationDefinition> = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| AutomationError::Io(format!("{path:?}: {e}")))?,
            Err(_) => Vec::new(),
        };
        for def in &defs {
            validate(def, gate)?;
        }
        Ok(Self {
            root: root.to_owned(),
            defs,
        })
    }

    /// Definitions under management.
    #[must_use]
    pub fn defs(&self) -> &[AutomationDefinition] {
        &self.defs
    }

    /// Validate + persist one definition (registration path).
    ///
    /// # Errors
    /// see [`AutomationError`].
    pub fn register(
        &mut self,
        def: &AutomationDefinition,
        gate: &dyn PackGate,
    ) -> Result<(), AutomationError> {
        validate(def, gate)?;
        if self.defs.iter().any(|d| d.id == def.id) {
            return Err(AutomationError::Duplicate { id: def.id.clone() });
        }
        let mut defs = self.defs.clone();
        defs.push(def.clone());
        self.persist(&defs)?;
        self.defs = defs;
        Ok(())
    }

    fn persist(&self, defs: &[AutomationDefinition]) -> Result<(), AutomationError> {
        let path = self.root.join("automations.json");
        let staged = self.root.join("automations.json.tmp");
        let bytes =
            serde_json::to_vec_pretty(defs).map_err(|e| AutomationError::Io(e.to_string()))?;
        std::fs::write(&staged, bytes)
            .and_then(|_| std::fs::rename(&staged, &path))
            .map_err(|e| AutomationError::Io(format!("{path:?}: {e}")))?;
        Ok(())
    }
}

fn validate(def: &AutomationDefinition, gate: &dyn PackGate) -> Result<(), AutomationError> {
    CronExpr::parse(&def.cron).map_err(|source| AutomationError::BadCron {
        def: def.id.clone(),
        source,
    })?;
    if gate.tier(&def.pack) != PackTier::Approved {
        return Err(AutomationError::DraftTier {
            pack: def.pack.clone(),
        });
    }
    Ok(())
}

/// Map definitions to scheduler jobs (disabled definitions skipped).
///
/// # Errors
/// cron re-parse failure (definitions are validated first).
pub fn to_jobs(defs: &[AutomationDefinition]) -> Result<Vec<ScheduledJob>, CronError> {
    let mut jobs = Vec::new();
    for def in defs.iter().filter(|d| d.enabled) {
        jobs.push(ScheduledJob {
            id: def.id.clone(),
            name: def.name.clone(),
            cron: CronExpr::parse(&def.cron)?,
            action: JobAction::RunPack {
                pack: def.pack.clone(),
            },
        });
    }
    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(id: &str, cron: &str, pack: &str) -> AutomationDefinition {
        AutomationDefinition {
            id: id.into(),
            name: format!("{id} label"),
            cron: cron.into(),
            pack: pack.into(),
            goal: Some("nightly review".into()),
            criteria: vec!["report written".into()],
            budget_turns: Some(8),
            max_invocations_total: Some(3),
            max_invocations_per_day: Some(2),
            network: NetworkPolicy::Online,
            enabled: true,
        }
    }

    fn approved_gate() -> FixedPackGate {
        FixedPackGate::new(vec![("review".to_owned(), PackTier::Approved)])
    }

    fn store_in(dir: &tempfile::TempDir) -> Result<AutomationStore, AutomationError> {
        AutomationStore::load(dir.path(), &approved_gate())
    }

    fn rejected(err: Result<(), AutomationError>, what: &str) -> AutomationError {
        match err {
            Err(e) => e,
            Ok(()) => panic!("{what} accepted"),
        }
    }

    #[test]
    fn register_rejects_malformed_cron() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut store = store_in(&dir)?;
        let err = rejected(
            store.register(&def("nightly", "99 * * * *", "review"), &approved_gate()),
            "bad cron",
        );
        assert!(matches!(err, AutomationError::BadCron { def, .. } if def == "nightly"));
        Ok(())
    }

    #[test]
    fn register_rejects_draft_tier_pack() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut store = store_in(&dir)?;
        let err = rejected(
            store.register(&def("nightly", "* * * * *", "unvetted"), &approved_gate()),
            "draft pack",
        );
        assert_eq!(
            err,
            AutomationError::DraftTier {
                pack: "unvetted".into()
            }
        );
        Ok(())
    }

    #[test]
    fn register_persists_and_load_roundtrips() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        {
            let mut store = store_in(&dir)?;
            store.register(&def("nightly", "0 3 * * *", "review"), &approved_gate())?;
        }
        let reloaded = store_in(&dir)?;
        assert_eq!(reloaded.defs().len(), 1);
        assert_eq!(reloaded.defs()[0].id, "nightly");
        assert_eq!(reloaded.defs()[0].cron, "0 3 * * *");
        Ok(())
    }

    #[test]
    fn duplicate_registration_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut store = store_in(&dir)?;
        store.register(&def("nightly", "0 3 * * *", "review"), &approved_gate())?;
        let err = rejected(
            store.register(&def("nightly", "0 4 * * *", "review"), &approved_gate()),
            "duplicate",
        );
        assert_eq!(
            err,
            AutomationError::Duplicate {
                id: "nightly".into()
            }
        );
        Ok(())
    }

    #[test]
    fn missing_store_file_loads_empty() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let store = store_in(&dir)?;
        assert!(store.defs().is_empty());
        assert!(!dir.path().join("automations.json").exists());
        Ok(())
    }

    #[test]
    fn load_rejects_on_disk_draft_pack() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let json = serde_json::to_vec(&vec![def("nightly", "0 3 * * *", "unvetted")])?;
        std::fs::write(dir.path().join("automations.json"), json)?;
        let err = match store_in(&dir) {
            Err(e) => e,
            Ok(_) => panic!("on-disk draft pack accepted"),
        };
        assert_eq!(
            err,
            AutomationError::DraftTier {
                pack: "unvetted".into()
            }
        );
        Ok(())
    }

    #[test]
    fn network_policy_intersection_offline_wins() {
        assert_eq!(
            NetworkPolicy::Online.intersect(NetworkPolicy::Offline),
            NetworkPolicy::Offline
        );
        assert_eq!(
            NetworkPolicy::Offline.intersect(NetworkPolicy::Online),
            NetworkPolicy::Offline
        );
        assert_eq!(
            NetworkPolicy::Online.intersect(NetworkPolicy::Online),
            NetworkPolicy::Online
        );
    }

    #[test]
    fn file_pack_gate_fails_closed_on_unknown_pack() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("pack-tiers.json");
        std::fs::write(&path, br#"{"review": "approved", "experimental": "draft"}"#)?;
        let gate = FilePackGate::open(&path)?;
        assert_eq!(gate.tier("review"), PackTier::Approved);
        assert_eq!(gate.tier("experimental"), PackTier::Draft);
        assert_eq!(gate.tier("never-heard-of"), PackTier::Draft);
        let missing = FilePackGate::open(&dir.path().join("absent.json"))?;
        assert_eq!(missing.tier("review"), PackTier::Draft);
        Ok(())
    }

    #[test]
    fn to_jobs_skips_disabled_and_parses_cron() -> Result<(), CronError> {
        let mut defs = vec![
            def("nightly", "0 3 * * *", "review"),
            def("paused", "0 4 * * *", "review"),
        ];
        defs[1].enabled = false;
        let jobs = to_jobs(&defs)?;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "nightly");
        assert_eq!(
            jobs[0].action,
            JobAction::RunPack {
                pack: "review".into()
            }
        );
        assert_eq!(jobs[0].name, "nightly label");
        Ok(())
    }
}
