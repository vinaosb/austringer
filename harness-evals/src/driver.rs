//! The run_sweep driver assembly (todo 26): glues the committed
//! scenario pack to the sweep machinery — [`crate::sweep::execute`]
//! over the scripted fake subject, standard guards, pin audit, and
//! the report artifacts. HERMETIC by construction: no network, no
//! live model, no process spawns (the git source probe lives in the
//! BIN, not the library — this crate performs I/O only on the
//! explicit paths it is handed).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::guard::GuardSet;
use crate::report::{HarnessCard, SourceFingerprint, SweepReport};
use crate::runner::SweepPair;
use crate::scenarios::{self, ConformanceFinding, ScenarioError};
use crate::sweep::{self, SweepConfig, SweepMode, Trigger};

/// The model id of the scripted fake provider (the only subject the
/// conformance sweep ever runs).
pub const FAKE_MODEL: &str = "fake-scripted";

/// The one locked profile of the conformance regression run.
pub const LOCKED_PROFILE: &str = "conformance-hermetic";

/// The matched-baseline reference the pack's rows carry (evals.adoc:
/// harness-matched rows only — the committed pins + dataset are the
/// reference this artifact is matched against).
pub const BASELINE_REF: &str = "hermetic-scenario-pack-v1";

/// A validated driver request.
#[derive(Debug, Clone)]
pub struct DriverArgs {
    /// Trigger semantics recorded in the artifact (scheduled or
    /// manual — there is no PR-gate trigger by policy).
    pub trigger: Trigger,
    /// The scenario-pack directory.
    pub fixtures: PathBuf,
    /// Where the JSON report artifact is written (the rendered table
    /// lands beside it as `<stem>.table.txt`).
    pub report: PathBuf,
}

/// Everything the driver can fail with before a verdict exists
/// (config-class failures: exit 3).
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// The scenario pack failed to load (corrupted cassette, pin-set
    /// mismatch, ... — the detail names the file).
    #[error("{0}")]
    Pack(#[from] ScenarioError),
    /// The sweep itself refused to run (invalid config).
    #[error("sweep rejected: {0}")]
    Sweep(String),
    /// An artifact write failed.
    #[error("report write failed: {0}")]
    Io(String),
}

impl DriverError {
    /// Config-class failures all exit 3 (perf-gate convention).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        3
    }
}

/// The finished sweep + its exit verdict.
#[derive(Debug)]
pub struct SweepVerdict {
    /// The report (also serialized to `args.report`).
    pub report: SweepReport,
    /// The rendered ranking table (also written to
    /// `<report>.table.txt` and printed by the bin).
    pub table: String,
    /// Named golden-prefix drift findings (empty when conformant).
    pub drift: Vec<ConformanceFinding>,
}

impl SweepVerdict {
    /// Exit code: 0 when every pin conforms and no unpinned
    /// regression fired; 2 (breach) otherwise. Known failures pinned
    /// in the pack's expected-failures file stay annotated, not
    /// fatal.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if self.drift.is_empty() && self.report.pins.unpinned.is_empty() {
            0
        } else {
            2
        }
    }
}

/// Parses the `--trigger` value. The vocabulary is the policy:
/// `scheduled` and `manual` only — a PR-gate trigger does not parse
/// because none exists.
///
/// # Errors
/// a bounded message naming the rejected value.
pub fn parse_trigger(value: &str) -> Result<Trigger, String> {
    match value {
        "scheduled" => Ok(Trigger::Scheduled),
        "manual" => Ok(Trigger::Manual),
        other => Err(format!(
            "unknown trigger {other:?}: expected scheduled or manual (there is no PR-gate trigger by policy)"
        )),
    }
}

fn harness_card(source: &SourceFingerprint) -> HarnessCard {
    HarnessCard {
        model: FAKE_MODEL.to_owned(),
        profile: LOCKED_PROFILE.to_owned(),
        settings: BTreeMap::from([("subject".to_owned(), "scripted-cassette".to_owned())]),
        source: source.clone(),
        baseline_ref: Some(BASELINE_REF.to_owned()),
    }
}

/// Runs the hermetic conformance sweep: loads the pack, checks every
/// golden-prefix pin, executes the one-pair regression sweep under
/// the scripted fake provider, applies the pin audit, and writes the
/// JSON + table artifacts.
///
/// # Errors
/// typed [`DriverError`] (config class, exit 3).
pub fn run(args: &DriverArgs, source: &SourceFingerprint) -> Result<SweepVerdict, DriverError> {
    let pack = scenarios::load(&args.fixtures)?;
    let drift = pack.conformance_findings();
    let mut subject = pack.build_subject();
    let pair = SweepPair {
        model: FAKE_MODEL.to_owned(),
        profile: LOCKED_PROFILE.to_owned(),
    };
    let config = SweepConfig {
        trigger: args.trigger,
        mode: SweepMode::Regression {
            locked_profile: LOCKED_PROFILE.to_owned(),
        },
        pairs: vec![pair.clone()],
        reps: 1,
    };
    let mut subjects: sweep::Subjects = BTreeMap::new();
    subjects.insert(FAKE_MODEL.to_owned(), &mut subject);
    let cards: sweep::Cards = BTreeMap::from([(pair.label(), harness_card(source))]);
    let guards = GuardSet::standard();
    let mut report = sweep::execute(&config, &pack.dataset, &mut subjects, &cards, &guards)
        .map_err(|e| DriverError::Sweep(e.to_string()))?;
    report.apply_pins(&pack.expected_failures);
    let table = report.render_table();
    report
        .write_artifact(&args.report)
        .map_err(DriverError::Io)?;
    std::fs::write(args.report.with_extension("table.txt"), &table)
        .map_err(|e| DriverError::Io(e.to_string()))?;
    Ok(SweepVerdict {
        report,
        table,
        drift,
    })
}
