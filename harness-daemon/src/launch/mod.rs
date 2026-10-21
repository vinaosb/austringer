//! The orchestration launch path (todo 8): the spec-driven launcher
//! wired into the daemon. One `/launch <spec-file>` submit resolves a
//! [`harness_orchestration`] spec, quarantines overlapping declarers
//! BEFORE any child launches ([`quarantine`]), and runs each surviving
//! task as a real child session ([`SessionChildExecutor`]) whose team
//! events ride the parent session's hub as the typed `Child*` session
//! events (v2 wires) and downgraded notifications (v1 wires, via
//! [`crate::compat`]).
//!
//! The daemon scheduler and completion gate stay canonical
//! (cron/durable); nothing here reimplements them.

mod children;
mod sink;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use harness_intel::catalog::Catalog;
use harness_orchestration::ResolveError;
use harness_orchestration::authorize::SpawnAuthzClass;
use harness_orchestration::journal::TeamJournal;
use harness_orchestration::launch::{LaunchConfig, Launcher, resolve};
use harness_orchestration::report::{LaunchReport, SpecSource};
use harness_orchestration::role::RoleRegistry;
use harness_orchestration::spec::{LaunchSpec, parse_spec};

use crate::attach::SessionHost;
use crate::mediation::MediationPolicy;
use crate::protocol::SessionEvent;
use crate::tools::ExecutorFactory;
use crate::usage::InferenceFactory;

pub use children::SessionChildExecutor;
pub use sink::HubTeamSink;

/// Per-daemon services child sessions need: the same injected
/// inference and tool-execution factories the session factory uses,
/// the sessions root (children get their own per-session dir in the
/// hidden `{launch}-child-{task}` namespace), and the permission
/// mediation every write takes (the normal path — never an auto-allow
/// for declared write-set paths).
pub struct LaunchServices {
    /// Inference factory for child sessions (scripted in tests, the
    /// provider composition in the binary).
    pub inference_factory: InferenceFactory,
    /// Tool-execution stage factory for child sessions.
    pub executor_factory: ExecutorFactory,
    /// Root under which child session dirs are created.
    pub sessions_root: PathBuf,
    /// Permission mediation policy for child sessions.
    pub ask_policy: MediationPolicy,
    /// Permission ask timeout for child sessions.
    pub ask_timeout: Duration,
}

/// A `/launch` request refused before any child spawned (typed,
/// surfaced to the submitting connection; runtime child failures are
/// events, not refusals).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchRefusal {
    /// The host has no launch services bound (built outside the
    /// daemon factory).
    #[error("launch services not bound (host built outside the daemon factory)")]
    NotBound,
    /// The spec file did not parse.
    #[error("spec: {0}")]
    Spec(String),
    /// The spec did not resolve (unknown role/model, cycles).
    #[error("resolve: {0}")]
    Resolve(String),
    /// The launcher refused mid-run (spawn authorization, journal).
    #[error("launch: {0}")]
    Spawn(String),
    /// Spec-file I/O.
    #[error("io: {0}")]
    Io(String),
}

/// Task label of a `{launch}-child-{task}` id.
pub(crate) fn task_of(child_id: &str) -> &str {
    child_id.rsplit("-child-").next().unwrap_or(child_id)
}

/// Quarantines `task` (and, transitively, everything needing it)
/// BEFORE launch: collision alert plus `quarantined` finish for each,
/// stripped from the spec. The overlapping child never launches — it
/// never receives any allow, auto or otherwise.
fn quarantine(host: &SessionHost, launch_id: &str, spec: &mut LaunchSpec, task: &str, detail: String) {
    let mut doomed = vec![task.to_owned()];
    let mut index = 0;
    while index < doomed.len() {
        let current = doomed[index].clone();
        index += 1;
        for candidate in &spec.tasks {
            if candidate.needs.iter().any(|need| need == &current)
                && !doomed.contains(&candidate.id)
            {
                doomed.push(candidate.id.clone());
            }
        }
    }
    spec.tasks.retain(|candidate| !doomed.contains(&candidate.id));
    for (position, id) in doomed.iter().enumerate() {
        let child = format!("{launch_id}-child-{id}");
        if position == 0 {
            host.publish(SessionEvent::ChildProgress {
                child: child.clone(),
                phase: "collision".to_owned(),
                detail: detail.clone(),
            });
        }
        host.publish(SessionEvent::ChildFinished {
            child,
            task: id.clone(),
            state: "quarantined".to_owned(),
            detail: if position == 0 {
                detail.clone()
            } else {
                format!("depends on quarantined {task}")
            },
        });
    }
}

/// Runs one launch to its report: resolve (with pre-launch quarantine
/// of overlapping declarers through the crate's own fail-closed
/// detection), loud clamp notices, an on-disk team journal under the
/// sessions root, fresh-class spawn grants minted for the operator
/// step the `/launch` submit itself constitutes, and the spec source
/// wired for mid-run staleness checks.
///
/// # Errors
/// [`LaunchRefusal`] for every pre-run refusal; runtime child
/// failures are events on the host hub, not errors here.
pub async fn run_launch(
    host: Arc<SessionHost>,
    services: Arc<LaunchServices>,
    spec_text: &str,
    spec_path: Option<&Path>,
    launch_id: &str,
) -> Result<LaunchReport, LaunchRefusal> {
    let mut spec = parse_spec(spec_text).map_err(|e| LaunchRefusal::Spec(e.to_string()))?;
    let registry = RoleRegistry::builtin();
    let catalog = Catalog::seeded();
    let profiles = BTreeMap::new();
    let config = LaunchConfig::default();
    let plan = loop {
        match resolve(launch_id, &spec, &registry, &catalog, &profiles, &config) {
            Ok(plan) => break plan,
            // Declared sibling overlap (write_set.rs detection through
            // resolve): the second declarer is quarantined BEFORE
            // launching — it never receives any allow, auto or
            // otherwise.
            Err(ResolveError::OverlappingWrites { a, b, glob }) => {
                let detail = format!("write set `{glob}` overlaps {a}");
                let task = task_of(&b).to_owned();
                quarantine(&host, launch_id, &mut spec, &task, detail);
            }
            Err(other) => return Err(LaunchRefusal::Resolve(other.to_string())),
        }
    };
    for notice in &plan.notices {
        host.publish(SessionEvent::Notification {
            kind: "launch_notice".to_owned(),
            message: notice.clone(),
        });
    }
    let journal_path = services.sessions_root.join(format!("{launch_id}-launch.jsonl"));
    let journal = TeamJournal::open(&journal_path).map_err(|e| LaunchRefusal::Io(e.to_string()))?;
    let sink = Arc::new(HubTeamSink::new(Arc::clone(&host)));
    let mut launcher = Launcher::new(config, sink, journal);
    for job in plan.waves.iter().flatten() {
        if job.authz == SpawnAuthzClass::Fresh {
            launcher.mint_grant(&job.child_id, "launch-operator");
        }
    }
    let executor = Arc::new(SessionChildExecutor {
        services: Arc::clone(&services),
        // Todo 33: children book their spend into the PARENT session's
        // ledger (the delegation aggregation contract) — the executor
        // carries the parent host so each child's inference decorator
        // resolves it.
        parent: Arc::clone(&host),
    });
    let source = spec_path.map(|path| SpecSource::from(path.to_path_buf()));
    launcher
        .run(&plan, executor, source.as_ref())
        .await
        .map_err(|e| LaunchRefusal::Spawn(e.to_string()))
}

/// Launch id from the spec file's stem (children are resumable
/// `{launch}-child-{task}` ids).
fn sanitize_stem(path: &Path) -> String {
    let stem: String = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("launch")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let trimmed = stem.trim_matches('-');
    if trimmed.is_empty() { "launch".to_owned() } else { trimmed.to_owned() }
}

/// `/launch <spec-file>` intake: synchronous validation (typed
/// refusal to the submitting connection), then the launch runs as a
/// background task publishing team events onto the host hub.
///
/// # Errors
/// [`LaunchRefusal`] when services are unbound or the spec cannot be
/// read or parsed.
pub fn start_launch(host: &Arc<SessionHost>, argument: &str) -> Result<(), LaunchRefusal> {
    let argument = argument.trim();
    if argument.is_empty() {
        return Err(LaunchRefusal::Spec("usage: /launch <spec-file>".to_owned()));
    }
    let services = host.launcher.get().cloned().ok_or(LaunchRefusal::NotBound)?;
    let path = PathBuf::from(argument);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| LaunchRefusal::Io(format!("{}: {e}", path.display())))?;
    parse_spec(&text).map_err(|e| LaunchRefusal::Spec(e.to_string()))?;
    let launch_id = sanitize_stem(&path);
    host.publish(SessionEvent::Notification {
        kind: "launch".to_owned(),
        message: format!("launch {launch_id} starting from {}", path.display()),
    });
    let runner = Arc::clone(host);
    let reporter = Arc::clone(host);
    tokio::spawn(async move {
        if let Err(e) = run_launch(runner, services, &text, Some(&path), &launch_id).await {
            reporter.publish(SessionEvent::Notification {
                kind: "launch_error".to_owned(),
                message: e.to_string(),
            });
        }
    });
    Ok(())
}
