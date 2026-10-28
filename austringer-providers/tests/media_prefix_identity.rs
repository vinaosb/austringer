//! M8 prefix-identity acceptance (plan): a 20-turn text-only session with
//! one image submitted mid-window and its cached projection arriving
//! later. Prints the per-pair report (the manual-QA artifact); pins 100%
//! identity on the pre-media prefix, a single sanctioned divergence
//! (projection arrival), >= 0.9 across the window, and deterministic
//! re-rendering of the same context.

mod common;

use std::sync::Arc;

use austringer_core::agent_loop::{CancelToken, Inference, InferenceRequest};
use austringer_core::event::Event;
use austringer_core::event::derived::{DerivedStore, ProjectionKind};
use austringer_core::media_delivery::prefix_identity;
use austringer_providers::adapter::InferenceAdapter;
use austringer_providers::media::{DELIVERY_PROJECTION_PRODUCER, MediaDelivery};
use austringer_test_support::provider::FakeApi;
use austringer_test_support::sse::SseScript;
use common::inferkit::{config, recorded, text_block};
use common::mediakit::{MediaSession, image_hash};

use common::{FakeBridge, Plan};

const TURNS: usize = 20;
const IMAGE_TURN: usize = 9;
const PROJECTION_AFTER_TURN: usize = 12;
const FOCUS: &str = "<focus-chain>stable focus block for cache identity</focus-chain>";

fn filler(turn: usize) -> String {
    format!(
        "turn {turn}: please inspect window {turn} of the trace and summarize the deltas found so far"
    )
}

fn plans(turns: usize) -> Vec<Plan> {
    (0..turns)
        .map(|_| Plan::ok(SseScript::single_text("ack")))
        .collect()
}

#[tokio::test(start_paused = true)]
async fn twenty_turn_prefix_identity_report() {
    let (session, image, _) = MediaSession::with_fixtures("m8-prefix").expect("session");
    let delivery = MediaDelivery::open(session.path()).expect("delivery");
    let derived = DerivedStore::open(session.path()).expect("derived");
    let bridge = FakeBridge::new(
        Arc::new(FakeApi::new()),
        common::inferkit::identity(),
        plans(TURNS + 1),
    );
    let mut infer =
        InferenceAdapter::new(Arc::new(bridge.clone()), config(), 7).with_media_delivery(delivery);

    let mut events = Vec::new();
    let mut seq = 0u64;
    let mut prompts: Vec<String> = Vec::new();
    let mut snapshot_at_projection_turn = Vec::new();
    for turn in 1..=TURNS {
        seq += 1;
        let mut blocks = vec![text_block(&filler(turn))];
        if turn == IMAGE_TURN {
            blocks.push(image.clone());
        }
        events.push(recorded(seq, Event::User { blocks }));
        seq += 1;
        events.push(recorded(
            seq,
            Event::Assistant {
                blocks: vec![text_block(&format!("ack {turn}"))],
            },
        ));
        if turn == PROJECTION_AFTER_TURN + 1 {
            // Events through turn 13 with the projection already cached:
            // re-rendering this exact state must reproduce turn 13's prompt.
            snapshot_at_projection_turn = events.clone();
        }
        let cancel = CancelToken::new();
        infer
            .infer(
                &InferenceRequest {
                    events: &events,
                    focus_block: FOCUS,
                    system: "",
                    tools: &[],
                },
                &cancel,
            )
            .await
            .expect("completes");
        prompts.push(bridge.served()[turn - 1].prompt.clone());
        if turn == PROJECTION_AFTER_TURN {
            // The projection lands AFTER turn 12's request was built: the
            // mid-window arrival the plan sanctions.
            derived
                .store_derived(
                    &image_hash(),
                    DELIVERY_PROJECTION_PRODUCER,
                    ProjectionKind::SceneDescription,
                    b"scene: a red square with a white border",
                )
                .expect("projection lands mid-window");
        }
    }

    println!("=== M8 20-turn prefix-identity report (text-only session) ===");
    // Layout law (context/builder.rs): the focus block rides the volatile
    // tail, so the cacheable region of a prompt is everything before it.
    // Identity is measured on that region — whole-prompt chaining was an
    // artifact of the deprecated head placement, where the constant focus
    // block made every prompt a strict prefix of the next.
    fn history_region(prompt: &str) -> &str {
        &prompt[..prompt.len() - FOCUS.len() - 1]
    }
    let mut divergent = 0usize;
    let mut identities = Vec::new();
    for pair in 0..prompts.len().saturating_sub(1) {
        let identity = prefix_identity(
            history_region(&prompts[pair]).as_bytes(),
            history_region(&prompts[pair + 1]).as_bytes(),
        );
        if identity < 1.0 {
            divergent += 1;
        }
        identities.push(identity);
        println!(
            "pair {:02}->{:02}: {:.6} (history {} B)",
            pair + 1,
            pair + 2,
            identity,
            history_region(&prompts[pair]).len()
        );
    }
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let pre_mean = mean(&identities[..IMAGE_TURN - 1]);
    let window_mean = mean(&identities);
    println!(
        "pre-media pairs (1..{}) mean: {pre_mean:.6} [required 1.0]",
        IMAGE_TURN - 1
    );
    println!("full 20-turn window mean:     {window_mean:.6} [required >= 0.9]");
    println!("divergent pairs: {divergent} [required 1: the projection arrival]");

    assert_eq!(pre_mean, 1.0, "pre-media prefix must be byte-stable (100%)");
    assert!(
        window_mean >= 0.9,
        "20-turn window identity must stay >= 0.9, got {window_mean}"
    );
    assert_eq!(divergent, 1, "exactly one sanctioned divergence");

    let cancel = CancelToken::new();
    infer
        .infer(
            &InferenceRequest {
                events: &snapshot_at_projection_turn,
                focus_block: FOCUS,
                system: "",
                tools: &[],
            },
            &cancel,
        )
        .await
        .expect("completes");
    let rerendered = &bridge.served()[TURNS].prompt;
    assert_eq!(
        rerendered, &prompts[PROJECTION_AFTER_TURN],
        "same events + same cache state re-render byte-identically"
    );
    assert!(rerendered.contains("scene: a red square with a white border"));
    assert!(rerendered.contains("[image: image/png 4x4 blake3:"));
}
