//! `{{shared:name}}` interpolation at the DAEMON seam (correctness-class
//! gap): austringer-skills pins interpolation at invoke time
//! (tests/packs_lock_watch.rs), but no daemon test ever submitted a turn
//! whose skill body carries shared partials and asserted the PROMPT the
//! adapter receives. Here a fixture pack (staged like packs/ content:
//! `.austringer/skills/{pack}/SKILL.md` + `_shared/*.md`) rides the real
//! submit lane — POST /s/{sid}/cmd `/skill …` → SessionHost::route_skills
//! → SkillsRuntime::route_submit → SkillIndex::invoke (progressive
//! disclosure + resolve_shared) → render_context → engine turn → the
//! scripted adapter's InferenceRequest. An unresolvable partial must
//! refuse the turn typed (no-silent-degradation law): pinned at the HTTP
//! surface the daemon actually exposes.

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use austringer_core::agent_loop::inference::{InferenceError, PendingCall, TASK_COMPLETED_TOOL};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference, InferenceRequest};
use austringer_core::event::{ContentBlock, Event, llm_visible};
use austringer_daemon::config::DaemonConfig;

use common::spawn_daemon_with_factory;

/// The model-visible text of one inference request: every text block of
/// every llm-visible event (the sanctioned slice per the adapter
/// contract) — exactly what a provider adapter would serialize.
fn llm_prompt_text(request: &InferenceRequest<'_>) -> String {
    let mut rendered = String::new();
    for event in llm_visible(request.events.iter()) {
        let blocks: &[ContentBlock] = match event {
            Event::User { blocks } | Event::Assistant { blocks } => blocks,
            Event::ToolResult { blocks, .. } => blocks,
            Event::System { text } => {
                rendered.push_str(text);
                rendered.push('\n');
                continue;
            }
            _ => continue,
        };
        for block in blocks {
            if let ContentBlock::Text { text } = block {
                rendered.push_str(text);
                rendered.push('\n');
            }
        }
    }
    rendered
}

/// Scripted inference that records each served request's prompt text and
/// answers task_completed — one completed turn per submit.
struct CapturingInference {
    seen: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Inference for CapturingInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let rendered = llm_prompt_text(request);
        self.seen.lock().expect("capture lock").push(rendered);
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "done".into(),
            }],
            tool_calls: vec![PendingCall {
                call_id: "done".into(),
                tool: TASK_COMPLETED_TOOL.to_owned(),
                arguments: serde_json::json!({}),
            }],
            usage: None,
        })
    }
}

fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    std::fs::write(path, content).expect("write file");
}

/// Fixture data dir mirroring the packs/ install layout: two `_shared`
/// partials, one approved pack whose body references both, and one
/// approved pack referencing a partial that does not exist.
fn fixture_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join(".austringer").join("skills");
    std::fs::write(
        dir.path().join("pack-tiers.json"),
        br#"{"interp-pack": "approved", "interp-broken-pack": "approved"}"#,
    )
    .expect("pack tiers");
    write_file(
        &skills.join("_shared").join("pack-conventions.md"),
        "## Pack conventions (fixture shared partial)\n\nSHARED-CONVENTIONS-MARKER inlined at invoke time.\n",
    );
    write_file(
        &skills.join("_shared").join("review-gates.md"),
        "## Review gates (fixture shared partial)\n\nSHARED-REVIEW-GATES-MARKER second partial resolves too.\n",
    );
    write_file(
        &skills.join("interp-pack").join("SKILL.md"),
        "---\nname: interp-pack\ndescription: \"Fixture interpolation pack.\"\nversion: \"1.0.0\"\ntriggers:\n  - interpolate the widget\n---\n\n# Interp pack\n\n{{shared:pack-conventions}}\n\n{{shared:review-gates}}\n\nPACK-BODY-MARKER pack-specific body.\n",
    );
    write_file(
        &skills.join("interp-broken-pack").join("SKILL.md"),
        "---\nname: interp-broken-pack\ndescription: \"Fixture broken-partial pack.\"\nversion: \"1.0.0\"\ntriggers:\n  - break the widget\n---\n\n# Broken pack\n\n{{shared:does-not-exist}}\n\nBROKEN-PACK-BODY-MARKER.\n",
    );
    dir
}

async fn spawn_capturing(dir: &Path, seen: &Arc<Mutex<Vec<String>>>) -> common::Spawned {
    let capture = Arc::clone(seen);
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> = Arc::new(move || {
        Box::new(CapturingInference {
            seen: Arc::clone(&capture),
        })
    });
    let data_dir = dir.to_owned();
    spawn_daemon_with_factory(
        move |config| DaemonConfig {
            data_dir,
            durable: false,
            ..config
        },
        factory,
    )
    .await
    .expect("spawn daemon")
}

async fn post_submit(base: &str, token: &str, session: &str, text: &str) -> (u16, String) {
    let body = serde_json::json!({
        "type": "command", "command": {"type": "submit", "text": text},
    });
    let response = reqwest::Client::new()
        .post(format!("http://{base}/s/{session}/cmd"))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .expect("http");
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    (status, text)
}

#[tokio::test]
async fn skills_interpolation_resolved_partials_ride_the_inference_prompt() {
    let dir = fixture_dir();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let spawned = spawn_capturing(dir.path(), &seen).await;

    let (status, text) = post_submit(
        &spawned.base,
        &spawned.token,
        "interp",
        "/skill interp-pack ship it",
    )
    .await;
    assert_eq!(status, 202, "approved pack invocable: {text}");

    let prompt = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let found = {
                let guard = seen.lock().expect("capture lock");
                guard
                    .iter()
                    .find(|rendered| rendered.contains("PACK-BODY-MARKER"))
                    .cloned()
            };
            match found {
                Some(rendered) => return rendered,
                None => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("inference request never carried the skill body");

    assert!(
        prompt.contains("<skill name=\"interp-pack\">"),
        "render_context wrapper rides the prompt: {prompt}"
    );
    assert!(
        prompt.contains("SHARED-CONVENTIONS-MARKER"),
        "pack-conventions partial inlined: {prompt}"
    );
    assert!(
        prompt.contains("SHARED-REVIEW-GATES-MARKER"),
        "second shared partial inlined: {prompt}"
    );
    assert!(
        prompt.contains("PACK-BODY-MARKER"),
        "pack-specific body rides the prompt: {prompt}"
    );
    assert!(prompt.contains("ship it"), "caller args ride: {prompt}");
    assert!(
        !prompt.contains("{{shared:"),
        "zero literal {{{{shared: markers survive rendering: {prompt}"
    );
}

#[tokio::test]
async fn skills_interpolation_unresolvable_partial_refuses_the_turn_typed() {
    let dir = fixture_dir();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let spawned = spawn_capturing(dir.path(), &seen).await;

    // Pin the documented daemon behavior: SkillsRuntime::invoke collapses
    // the skills crate's typed SharedPartialNotFound into the Unknown
    // refusal — an honest typed error at intake (HTTP 400, turn never
    // queues), never a rendered prompt with unresolved markers.
    let (status, text) = post_submit(
        &spawned.base,
        &spawned.token,
        "broken",
        "/skill interp-broken-pack try anyway",
    )
    .await;
    assert_eq!(status, 400, "typed refusal, never acceptance: {text}");
    assert!(
        text.contains("interp-broken-pack"),
        "refusal names the pack: {text}"
    );
    assert!(
        text.contains("not found"),
        "the invoke-stage refusal surfaces typed: {text}"
    );
    assert!(
        !text.contains("{{shared:"),
        "refusal never echoes unresolved markers: {text}"
    );

    // The turn never ran: with a grace window, the adapter saw zero
    // requests — no prompt can carry the broken body in any form.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let requests = seen.lock().expect("capture lock");
    assert!(
        requests.is_empty(),
        "no inference request may carry the broken body: {requests:?}"
    );
}
