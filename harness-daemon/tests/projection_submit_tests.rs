//! M10 residuals: the daemon submit path runs the projection resolver
//! (R2) and the T42 utility-route fallback (R3). Submit-with-media
//! resolves needed projections at intake — resolved projections land in
//! the derived store with provenance; unprojectable media append one
//! typed `Event::System` notice into the live turn and store NOTHING
//! (never a fabricated projection).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use common::CompleteOnlyInference;
use common::cassette::{Cassette, CassetteTransport, anthropic_text_cassette};
use harness_core::agent_loop::Inference;
use harness_core::event::derived::DerivedStore;
use harness_core::event::media::MediaStore;
use harness_core::event::{ContentBlock, ContentHash, Event, ProjectionKind};
use harness_core::media_delivery::DeliveryCapabilities;
use harness_daemon::attach::HostCommand;
use harness_daemon::config::DaemonConfig;
use harness_daemon::projection::{IntakeFactory, ResolvedMediation, compose_intake_factory, utility_route};
use harness_daemon::protocol::{ServerMsg, SessionEvent};
use harness_daemon::provider::{ProviderFamily, ProviderSelection};
use harness_daemon::server::DaemonState;

const KEY_ENV: &str = "HARNESS_M10_SUBMIT_KEY";
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n submit-path-fixture";

fn vision_caps() -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: true,
        audio_input: false,
    }
}

fn resolved_mediation(model: &str, caps: DeliveryCapabilities) -> ResolvedMediation {
    let selection = ProviderSelection {
        family: ProviderFamily::Anthropic,
        model: model.to_owned(),
        api_key_env: Some(KEY_ENV.to_owned()),
        base_url: None,
        inference_timeout: Duration::from_secs(30),
    };
    let resolved = selection
        .resolve_with(|name| (name == KEY_ENV).then(|| "submit-fake-key".to_owned()))
        .expect("resolve selection");
    ResolvedMediation {
        resolved,
        model_id: model.to_owned(),
        caps,
        timeout: Duration::from_secs(10),
    }
}

/// Daemon state with the projection factory wired exactly as the binary
/// wires it; sessions created from it carry the intake.
fn daemon_state(root: &Path, factory: IntakeFactory) -> Arc<DaemonState> {
    let config = DaemonConfig {
        data_dir: root.to_owned(),
        token: Some("proj-token".into()),
        ..DaemonConfig::default()
    };
    let inference: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(CompleteOnlyInference));
    let mut state = DaemonState::new(config, inference).expect("daemon state");
    state.projection_factory = factory;
    Arc::new(state)
}

/// Stores the fixture bytes and submits them as media through the real
/// intake path; returns the turn's recorded events.
async fn submit_png(state: &Arc<DaemonState>, sid: &str) -> (Vec<Event>, PathBuf) {
    let host = state.session(sid);
    let session_dir = state.sessions_root.join(sid);
    let reference = MediaStore::open(&session_dir)
        .expect("media store")
        .store_media(PNG_BYTES)
        .expect("store fixture bytes");
    let live = host.hub_subscribe();
    host.handle_command(HostCommand::SubmitMedia {
        text: "describe this".into(),
        media: vec![reference],
    })
    .await
    .expect("submit accepted");
    (collect_recorded(live).await, session_dir)
}

/// Collects recorded events from a subscription made BEFORE the submit
/// (no replay race), until the turn finishes or the budget lapses.
async fn collect_recorded(mut live: broadcast::Receiver<ServerMsg>) -> Vec<Event> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match tokio::time::timeout_at(deadline, live.recv()).await {
            Ok(Ok(ServerMsg::Event { frame })) => {
                let finished = matches!(frame.event, SessionEvent::TurnFinished { .. });
                if let SessionEvent::Recorded { event, .. } = frame.event {
                    events.push(event);
                }
                if finished {
                    return events;
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => return events,
        }
    }
}

fn carries_image_block(events: &[Event]) -> bool {
    events.iter().any(|event| match event {
        Event::User { blocks } => blocks.iter().any(|block| match block {
            ContentBlock::Image(_) => true,
            ContentBlock::Media(media) => media.mime.as_str().starts_with("image/"),
            ContentBlock::Text { .. } => false,
        }),
        _ => false,
    })
}

fn system_notices(events: &[Event]) -> Vec<&String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::System { text } => Some(text),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn submit_with_media_to_a_vision_session_model_delivers_natively_no_projection_queued() {
    let root = tempfile::tempdir().expect("root");
    let transport = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        "claude-submit-vision",
        "a red square on white",
    ))]);
    let state = daemon_state(
        root.path(),
        compose_intake_factory(
            transport.clone(),
            Some(resolved_mediation("claude-submit-vision", vision_caps())),
            None,
        ),
    );
    let (events, session_dir) = submit_png(&state, "s-proj-session").await;

    assert!(
        carries_image_block(&events),
        "the user event keeps the image block: {events:?}"
    );
    assert!(
        system_notices(&events).is_empty(),
        "native delivery queues no projection and no notice: {events:?}"
    );
    // The M8 planner contract: image + vision caps -> NativeImage, no
    // needed projection. Zero model calls, zero derived writes — the
    // session model is never billed a projection it does not need.
    assert!(
        transport.requests().is_empty(),
        "no projection call for a natively-deliverable image"
    );
    let derived = DerivedStore::open(&session_dir).expect("derived store");
    let source = ContentHash::from_bytes(PNG_BYTES);
    assert!(
        derived
            .load_derived(&source, "delivery", ProjectionKind::SceneDescription)
            .expect("load derived")
            .is_none(),
        "nothing needlessly computed or stored"
    );
}

#[tokio::test]
async fn submit_with_media_without_any_projector_appends_the_typed_notice_and_stores_nothing() {
    let root = tempfile::tempdir().expect("root");
    let transport = CassetteTransport::new(vec![]);
    let state = daemon_state(root.path(), compose_intake_factory(transport.clone(), None, None));
    let (events, session_dir) = submit_png(&state, "s-proj-none").await;

    let notices = system_notices(&events);
    assert_eq!(notices.len(), 1, "exactly one notice: {events:?}");
    assert!(
        notices[0].contains("media projection unavailable"),
        "{}",
        notices[0]
    );
    assert!(notices[0].contains("scene_description"), "{}", notices[0]);
    assert!(
        carries_image_block(&events),
        "the marker survives: {events:?}"
    );
    assert!(transport.requests().is_empty(), "no model is ever called");
    let derived = DerivedStore::open(&session_dir).expect("derived store");
    let source = ContentHash::from_bytes(PNG_BYTES);
    assert!(
        derived
            .load_derived(&source, "delivery", ProjectionKind::SceneDescription)
            .expect("load derived")
            .is_none(),
        "nothing stored — no fabrication"
    );
}

#[tokio::test]
async fn submit_with_media_falls_back_to_the_capable_utility_route_when_the_session_model_is_text_only()
{
    let root = tempfile::tempdir().expect("root");
    let transport = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        "claude-opus-4-6",
        "utility route projection",
    ))]);
    let state = daemon_state(
        root.path(),
        compose_intake_factory(
            transport.clone(),
            Some(resolved_mediation(
                "text-only-session-model",
                DeliveryCapabilities::default(),
            )),
            Some(resolved_mediation("claude-opus-4-6", vision_caps())),
        ),
    );
    let (events, session_dir) = submit_png(&state, "s-proj-utility").await;

    assert!(
        system_notices(&events).is_empty(),
        "the utility route covered the gap: {events:?}"
    );
    assert_eq!(
        transport.requests().len(),
        1,
        "the utility model projected once"
    );
    let derived = DerivedStore::open(&session_dir).expect("derived store");
    let source = ContentHash::from_bytes(PNG_BYTES);
    assert!(
        derived
            .load_derived(
                &source,
                "model:claude-opus-4-6:v1",
                ProjectionKind::SceneDescription
            )
            .expect("load derived")
            .is_some(),
        "utility provenance recorded"
    );
}

#[test]
fn utility_route_resolves_the_summarizer_role_from_routing_json() {
    let json = serde_json::json!({
        "provider": "anthropic",
        "cheap_pool": ["claude-opus-4-6"],
        "roles": [{ "role": "summarizer", "model": "claude-opus-4-6" }],
    })
    .to_string();
    let route = utility_route(&json, &harness_intel::catalog::Catalog::seeded()).expect("route");
    assert_eq!(route.model, "claude-opus-4-6");
    assert!(route.caps.vision);
    assert!(!route.caps.audio_input);
}

#[test]
fn utility_route_is_a_typed_error_when_unconfigured_or_off_the_cheap_pool() {
    let catalog = harness_intel::catalog::Catalog::seeded();
    let unconfigured =
        serde_json::json!({ "provider": "anthropic", "cheap_pool": ["claude-opus-4-6"] }).to_string();
    assert!(
        utility_route(&unconfigured, &catalog).is_err(),
        "no summarizer route configured"
    );
    let off_cheap = serde_json::json!({
        "provider": "anthropic",
        "cheap_pool": ["claude-sonnet-4-6"],
        "roles": [{ "role": "summarizer", "model": "claude-opus-4-6" }],
    })
    .to_string();
    assert!(
        utility_route(&off_cheap, &catalog).is_err(),
        "utility roles stay on the cheap pool (fail-closed)"
    );
}
