//! The PromptBuilder-routed session prompt path: core's
//! prefix-cache-stable composition surface promoted into the daemon's
//! real spawn path. Pins the BYTE-IDENTITY contract (the routed
//! composition equals the legacy `compose_system_prompt` output —
//! same frozen const, same `<repomap>` block), the cache-violation
//! tracker's epoch semantics (stable recomposition is silent, a
//! mutated frozen input fires a typed violation), and the notice path
//! through the real session hub (`prompt_cache` notifications, never
//! silent, never fatal).
//!
//! Hermetic: scripted/unused inference, zero network, zero keys.
//! Mirrors the `repomap_prompt_tests` / `eviction_tests` fixtures.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use austringer_core::agent_loop::inference::{InferenceError, InferenceRequest};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::context::{SegmentName, ViolationKind};
use austringer_daemon::attach::SessionHost;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::engine::SESSION_SYSTEM_PROMPT;
use austringer_daemon::eviction;
use austringer_daemon::mediation::MediationPolicy;
use austringer_daemon::prompt::{
    ComposedSessionPrompt, PROMPT_CACHE_KIND, PromptEpoch, bind_system_prompt,
    compose_session_prompt,
};
use austringer_daemon::protocol::SessionEvent;
use austringer_daemon::repomap::compose_system_prompt;
use austringer_daemon::server::DaemonState;
use austringer_daemon::usage::{ComposedInference, InferenceFactory};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

fn write(dir: &tempfile::TempDir, relative: &str, bytes: &[u8]) {
    let path = dir.path().join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
    }
    fs::write(path, bytes).unwrap_or_else(|e| panic!("{e}"));
}

/// The `repomap_prompt_tests` workspace fixture: three grammar files
/// (one nested), one gitignored source file, one non-grammar file.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    write(&dir, ".gitignore", b"secret/\n");
    write(&dir, "alpha.rs", b"pub fn alpha_fn() {}\n");
    write(&dir, "beta.rs", b"pub fn beta_fn() { alpha_fn() }\n");
    write(&dir, "nested/gamma.rs", b"pub fn gamma_fn() {}\n");
    write(&dir, "secret/hidden.rs", b"pub fn secret_fn() {}\n");
    write(&dir, "notes.txt", b"plainly not source\n");
    dir
}

fn cache() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("cache dir")
}

/// Inference the composed sessions never call (no turns are submitted).
struct SilentInference;

#[async_trait]
impl Inference for SilentInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Err(InferenceError::Failure("never called".into()))
    }
}

fn silent_factory() -> InferenceFactory {
    Arc::new(|| ComposedInference::plain(Box::new(SilentInference)))
}

fn prompt_cache_notifications(host: &Arc<SessionHost>) -> Vec<String> {
    host.snapshot()
        .into_iter()
        .filter_map(|frame| match frame.event {
            SessionEvent::Notification { kind, message } if kind == PROMPT_CACHE_KIND => {
                Some(message)
            }
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// (1) byte-identity: builder-routed == compose_system_prompt
// ---------------------------------------------------------------------------

#[test]
fn builder_routed_composition_is_byte_identical_with_the_map() {
    let dir = fixture();
    let cache = cache();
    let routed = compose_session_prompt(dir.path(), cache.path(), 1000);
    let legacy = compose_system_prompt(dir.path(), cache.path(), 1000);
    assert!(
        legacy.contains("alpha_fn"),
        "the fixture's map actually hydrates"
    );
    assert_eq!(
        routed.system(),
        legacy,
        "the routed composition must stay byte-identical to the legacy contract"
    );
    // The routed string is EXACTLY the frozen const plus the block
    // around the RENDER-EXTRACTED segment content (the derivation is
    // real, not a re-render of the raw inputs).
    let ComposedSessionPrompt::Rendered { system, rendered } = &routed else {
        panic!("the fixture must render, not refuse");
    };
    let map_text = rendered
        .segment_content(SegmentName::Repomap)
        .expect("repomap segment present");
    assert!(!map_text.is_empty());
    assert_eq!(
        system,
        &format!("{SESSION_SYSTEM_PROMPT}\n\n<repomap>\n{map_text}\n</repomap>")
    );
}

#[test]
fn builder_routed_composition_is_byte_identical_on_the_degenerate_cases() {
    // Budget 0.
    let dir = fixture();
    let zero_budget_cache = cache();
    assert_eq!(
        compose_session_prompt(dir.path(), zero_budget_cache.path(), 0).system(),
        compose_system_prompt(dir.path(), zero_budget_cache.path(), 0)
    );
    // Empty workspace.
    let empty = tempfile::TempDir::new().expect("tempdir");
    let empty_cache = cache();
    assert_eq!(
        compose_session_prompt(empty.path(), empty_cache.path(), 1000).system(),
        compose_system_prompt(empty.path(), empty_cache.path(), 1000)
    );
    assert_eq!(
        compose_session_prompt(empty.path(), empty_cache.path(), 1000).system(),
        SESSION_SYSTEM_PROMPT
    );
    // Unbuildable root.
    let missing_cache = cache();
    assert_eq!(
        compose_session_prompt(Path::new("no-such-root-zq"), missing_cache.path(), 1000).system(),
        compose_system_prompt(Path::new("no-such-root-zq"), missing_cache.path(), 1000)
    );
}

// ---------------------------------------------------------------------------
// (2) the tracker: stable recomposition is silent, mutated frozen
//     input fires a typed violation
// ---------------------------------------------------------------------------

#[test]
fn epoch_tracker_same_spec_same_hash_no_violation() {
    let dir = fixture();
    let cache = cache();
    let ComposedSessionPrompt::Rendered {
        rendered: first, ..
    } = compose_session_prompt(dir.path(), cache.path(), 1000)
    else {
        panic!("render");
    };
    let ComposedSessionPrompt::Rendered {
        rendered: second, ..
    } = compose_session_prompt(dir.path(), cache.path(), 1000)
    else {
        panic!("render");
    };
    assert_eq!(
        first.stable_prefix_hash, second.stable_prefix_hash,
        "same logical state, same bytes"
    );

    let mut epoch = PromptEpoch::new();
    assert!(
        epoch.observe(&first).is_empty(),
        "the first observation only establishes the chain"
    );
    assert!(
        epoch.observe(&second).is_empty(),
        "an unchanged epoch never fires"
    );
    assert_eq!(epoch.stable_prefix_hash(), Some(first.stable_prefix_hash));
}

#[test]
fn epoch_tracker_mutated_frozen_input_fires_a_typed_violation() {
    let dir = fixture();
    let cache = cache();
    let ComposedSessionPrompt::Rendered {
        system: original, ..
    } = compose_session_prompt(dir.path(), cache.path(), 1000)
    else {
        panic!("render");
    };
    let mut epoch = PromptEpoch::new();
    let ComposedSessionPrompt::Rendered {
        rendered: stable, ..
    } = compose_session_prompt(dir.path(), cache.path(), 1000)
    else {
        panic!("render");
    };
    epoch.observe(&stable);

    // Mutate a frozen input: the repository map changes.
    write(&dir, "alpha.rs", b"pub fn omega_fn() {}\n");
    write(&dir, "beta.rs", b"pub fn beta_fn() { omega_fn() }\n");
    let mutated = compose_session_prompt(dir.path(), cache.path(), 1000);
    let ComposedSessionPrompt::Rendered {
        system: drifted, ..
    } = &mutated
    else {
        panic!("render");
    };
    assert_ne!(
        original, *drifted,
        "the fixture mutation actually moved the composed bytes"
    );

    let violations = match &mutated {
        ComposedSessionPrompt::Rendered { rendered, .. } => epoch.observe(rendered),
        ComposedSessionPrompt::Refused { .. } => panic!("render"),
    };
    assert_eq!(violations.len(), 1, "one chain-break, got {violations:?}");
    assert_eq!(violations[0].diverged_segment, Some(SegmentName::Repomap));
    assert!(
        matches!(violations[0].kind, ViolationKind::ChainBreak { .. }),
        "the drift is a chain break: {:?}",
        violations[0].kind
    );
}

// ---------------------------------------------------------------------------
// (3) the notice path: drift surfaces as a typed notification on the
//     real session hub (the same publish lane the fallback notices use)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drift_surfaces_as_a_typed_notice_on_the_session_hub() {
    let dir = fixture();
    let cache = cache();
    let epochs = Mutex::new(HashMap::new());
    let (host, _cmd_rx) = SessionHost::new(
        "s-prompt-epoch",
        64,
        32,
        8,
        MediationPolicy::FirstResponder,
        Duration::from_millis(200),
        None,
    );
    let sid = "s-prompt-epoch";

    let (system_a, notices) = bind_system_prompt(&epochs, sid, dir.path(), cache.path(), 1000);
    assert!(notices.is_empty(), "creation is silent: {notices:?}");
    assert!(system_a.contains("<repomap>"));

    let (_, notices) = bind_system_prompt(&epochs, sid, dir.path(), cache.path(), 1000);
    assert!(notices.is_empty(), "stable recomposition is silent");

    write(&dir, "alpha.rs", b"pub fn omega_fn() {}\n");
    write(&dir, "beta.rs", b"pub fn beta_fn() { omega_fn() }\n");
    let (system_b, notices) = bind_system_prompt(&epochs, sid, dir.path(), cache.path(), 1000);
    assert_ne!(system_a, system_b);
    assert_eq!(notices.len(), 1, "drift fires exactly one notice");

    // The session-factory publication lane, verbatim.
    for message in notices {
        host.publish(SessionEvent::Notification {
            kind: PROMPT_CACHE_KIND.to_owned(),
            message,
        });
    }

    let hub_notices = prompt_cache_notifications(&host);
    assert_eq!(hub_notices.len(), 1, "exactly one hub notification");
    assert!(
        hub_notices[0].contains("chain break"),
        "the notice names the violation: {}",
        hub_notices[0]
    );
    assert!(
        hub_notices[0].contains("Repomap"),
        "the notice names the diverged segment: {}",
        hub_notices[0]
    );
}

// ---------------------------------------------------------------------------
// (4) the daemon path: an evicted-and-recreated session recomposes
//     against the surviving epoch (budget 0 keeps the test independent
//     of the daemon process's working directory)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn daemon_recomposes_the_epoch_stably_across_eviction() {
    let dir = tempfile::tempdir().unwrap();
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("prompt-token".into()),
        repomap_token_budget: 0,
        ..DaemonConfig::default()
    };
    let state = Arc::new(DaemonState::new(config, silent_factory()).expect("daemon state"));
    let sid = "s-prompt";

    let host = state.session(sid);
    assert!(
        prompt_cache_notifications(&host).is_empty(),
        "the first composition only establishes the epoch"
    );

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(
        eviction::run_once(&state, Duration::from_millis(1)),
        vec![sid.to_owned()],
        "the idle host is swept"
    );

    let reopened = state.session(sid);
    assert!(
        !Arc::ptr_eq(&host, &reopened),
        "a fresh host was created for the evicted id"
    );
    assert!(
        prompt_cache_notifications(&reopened).is_empty(),
        "a stable recomposition never fires"
    );

    let epochs = state.prompt_epochs.lock().expect("epoch map");
    assert_eq!(epochs.len(), 1, "the epoch survived the eviction");
    let epoch = epochs.get(sid).expect("epoch entry");
    assert_eq!(
        epoch.compositions(),
        2,
        "the recomposition was observed against the recorded baseline"
    );
    assert!(
        epoch.stable_prefix_hash().is_some(),
        "the baseline hash is recorded"
    );
}
