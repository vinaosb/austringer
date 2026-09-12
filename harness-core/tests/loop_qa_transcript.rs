//! Manual-QA transcript generators (todo 5): run explicitly with
//! `cargo nextest run --run-ignored=only -p harness-core` and paste the
//! captured stdout into the evidence file. Ignored in normal runs by
//! design — they exist to print, not to assert.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::TempSession;
use common::loopkit::{FakeApiInference, ScriptedInference, call_turn};
use common::stages_kit::{CancelAwareExecution, stages};
use harness_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome, StageSet};
use harness_core::event::{ContentBlock, Event, RecordedEvent};
use harness_test_support::provider::{FakeApi, RequestMatcher};
use harness_test_support::sse::SseScript;

fn block_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image(_) => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn dump(events: &[RecordedEvent]) {
    for recorded in events {
        let line = match &recorded.event {
            Event::User { blocks } => format!("user: {}", block_text(blocks)),
            Event::Assistant { blocks } => format!("assistant: {}", block_text(blocks)),
            Event::ToolCall {
                call_id,
                tool,
                arguments,
            } => format!("tool-call {call_id} {tool} {arguments}"),
            Event::ToolResult { call_id, blocks } => {
                format!("tool-result {call_id}: {}", block_text(blocks))
            }
            Event::System { text } => format!("system: {text}"),
            Event::Condensation { summary, .. } => format!("condensation: {summary}"),
            Event::Snapshot { label } => format!("snapshot: {label}"),
        };
        println!("  seq {:>2}  {line}", recorded.seq.0);
    }
}

#[tokio::test]
#[ignore = "manual QA transcript: happy 3-turn scripted conversation"]
async fn transcript_happy_three_turn_conversation() {
    let dir = TempSession::new("qa-happy");
    let api = Arc::new(FakeApi::new());
    api.on(RequestMatcher::Any)
        .reply_stream(SseScript::single_text("first answer"))
        .reply_stream(SseScript::single_text("second answer"))
        .reply_stream(SseScript::single_text("third answer"));
    let inference = FakeApiInference::new(api, "fake-model");
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(inference.clone()),
    )
    .expect("open session");
    session.focus_mut().upsert(1.into(), "ride along stably");

    println!("=== HAPPY: 3-turn scripted conversation through the full pipeline ===");
    for question in ["q1", "q2", "q3"] {
        session.submit(question);
        let outcome = session.run().await;
        println!(
            "run({question}) -> {outcome:?}  status={:?}",
            session.status()
        );
    }
    let seen = inference.seen();
    println!(
        "focus blocks byte-identical across turns: {}",
        seen[0].focus_block == seen[1].focus_block && seen[1].focus_block == seen[2].focus_block
    );
    println!("--- event log ({} events) ---", session.events().len());
    dump(session.events());
    let disk = harness_core::log::replay(dir.path()).expect("replay");
    println!(
        "disk replay: {} events, anomalies: {}",
        disk.events.len(),
        disk.anomalies.len()
    );
}

#[tokio::test(start_paused = true)]
#[ignore = "manual QA transcript: cancel mid ToolExecution leaves no half-state"]
async fn transcript_cancel_mid_tool_execution() {
    let dir = TempSession::new("qa-cancel");
    let stages = stages(|s| s.execution = Box::new(CancelAwareExecution));
    let inference =
        ScriptedInference::new(vec![call_turn("c1", "long_tool", serde_json::Value::Null)]);
    let config = LoopConfig {
        cancel_grace: Duration::from_secs(60),
        ..LoopConfig::default()
    };
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("run the tool");

    println!("=== FAILURE: cancel mid-ToolExecution ===");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });
    println!("outcome: {outcome:?}");
    println!("status: {:?}", session.status());
    if let RunOutcome::Cancelled(record) = &outcome {
        println!(
            "high_water={} operation={} forced={}",
            record.high_water.0,
            record.operation.name(),
            record.forced
        );
    }
    println!("--- event log tail at high-water (clean, mark last) ---");
    dump(session.events());
    drop(session);

    let mut resumed = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(ScriptedInference::new(vec![common::loopkit::text_turn(
            "resumed and done",
        )])),
    )
    .expect("reopen");
    println!(
        "resume: status={:?} (orphan tool call rebuilt from log)",
        resumed.status()
    );
    resumed.submit("finish");
    let outcome = resumed.run().await;
    println!("resume run -> {outcome:?}  status={:?}", resumed.status());
    println!(
        "tool c1 executed exactly once on resume: {}",
        resumed
            .events()
            .iter()
            .filter(|r| matches!(&r.event, Event::ToolResult { call_id, .. } if call_id == "c1"))
            .count()
            == 1
    );
    println!("--- full event log after resume ---");
    dump(resumed.events());
}
