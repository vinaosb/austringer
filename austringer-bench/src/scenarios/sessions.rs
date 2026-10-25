//! Session paging scenario: build a synthetic session in a temp store,
//! page in 100 turns through the todo-24 lazy index. Paged bytes is the
//! deterministic gate metric; page-in wall is advisory (the crate's own
//! perf suite gates the strict 5 ms budget).

use std::time::Instant;

use austringer_core::event::{ContentBlock, Event};
use austringer_sessions::{LeaseOwner, SessionStore};
use tempfile::TempDir;

use super::fixtures::Rng;
use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;
use crate::stats::min_of;

pub const TURNS: usize = 300;
pub const BODY_BYTES: usize = 700;

pub fn build(store: &SessionStore, id: &str) -> Result<(), ScenarioError> {
    store
        .create(id, "bench")
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let owner = LeaseOwner::new("austringer-bench");
    let handle = store
        .open_session_rw(id, &owner)
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let mut rng = Rng::new(2409);
    for turn in 1..=TURNS {
        let batch = [
            Event::User {
                blocks: vec![ContentBlock::Text {
                    text: format!("turn {turn}: {}", rng.text(8)),
                }],
            },
            Event::Assistant {
                blocks: vec![ContentBlock::Text {
                    text: "y".repeat(BODY_BYTES),
                }],
            },
            Event::ToolCall {
                call_id: format!("c-{turn}"),
                tool: "view_window".to_owned(),
                arguments: serde_json::json!({"path": "src/lib.rs"}),
            },
        ];
        handle
            .append_batch(&batch)
            .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    }
    handle
        .release()
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    Ok(())
}

fn page_bytes(store: &SessionStore, id: &str) -> Result<f64, ScenarioError> {
    let handle = store
        .open_session(id)
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let pages = handle
        .turns_range(150, 249)
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let mut bytes = 0usize;
    for page in &pages {
        for event in &page.events {
            let encoded =
                serde_json::to_vec(event).map_err(|e| ScenarioError::Failed(e.to_string()))?;
            bytes += encoded.len();
        }
    }
    drop(handle);
    Ok(bytes as f64)
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let dir = TempDir::new().map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let store = SessionStore::open(dir.path()).map_err(|e| ScenarioError::Failed(e.to_string()))?;
    build(&store, "bench")?;
    let mut out = ScenarioOutput::new();
    let mut walls = Vec::new();
    let mut first: Option<f64> = None;
    #[cfg(feature = "dhat")]
    let heap_before = super::heap::snapshot();
    for _ in 0..ctx.wall_runs.max(1) {
        let started = Instant::now();
        let bytes = page_bytes(&store, "bench")?;
        walls.push(started.elapsed());
        if let Some(seen) = first
            && (seen - bytes).abs() > f64::EPSILON
        {
            return Err(ScenarioError::Failed(format!(
                "paged bytes not deterministic across runs: {seen} vs {bytes}"
            )));
        }
        first = Some(bytes);
    }
    let bytes = first.ok_or_else(|| ScenarioError::Failed("no runs".into()))?;
    out.push("sessions.page_bytes", bytes, "bytes", Policy::Regression);
    out.push(
        "sessions.page_ms",
        min_of(&walls)
            .map_err(|e| ScenarioError::Failed(e.to_string()))?
            .as_secs_f64()
            * 1000.0,
        "ms",
        Policy::Advisory,
    );
    #[cfg(feature = "dhat")]
    super::heap::record(&mut out, "sessions", heap_before);
    Ok(out)
}
