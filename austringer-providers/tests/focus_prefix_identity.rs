//! Focus-block placement (the context layout law): the focus chain is the
//! volatile region of every provider request — its `status: N open / M
//! total` line changes on every todo transition — so it must ride the
//! prompt TAIL, after the entire history region (`context/builder.rs`:
//! "History must carry only the events preceding the volatile tail; the
//! tail is the last user message alone"). A todo transition must leave the
//! history region byte-identical so the provider's prefix cache survives
//! the turn (research SS24.6 engineered prefix discipline); head placement
//! invalidates the whole prompt at byte ~22.

mod common;

use austringer_core::agent_loop::{CancelToken, FocusChain, Inference, InferenceRequest, TodoId};
use austringer_test_support::sse::SseScript;
use common::{Plan, inferkit};

#[tokio::test(start_paused = true)]
async fn todo_transition_keeps_the_history_prefix_byte_identical() {
    let (bridge, mut infer) = inferkit::adapter(vec![
        Plan::ok(SseScript::single_text("ack")),
        Plan::ok(SseScript::single_text("ack")),
    ]);
    let cancel = CancelToken::new();

    // Identical history in both requests; only the todo state differs.
    let events = inferkit::context();
    let mut before = FocusChain::new();
    before.upsert(TodoId(1), "read the map");
    before.upsert(TodoId(2), "ring the bell");
    let mut after = before.clone();
    assert!(
        after.mark_done(TodoId(1)),
        "todo 1 transitions open -> done"
    );
    let focus_before = before.render();
    let focus_after = after.render();
    assert_ne!(
        focus_before, focus_after,
        "the volatile region must actually change between the turns"
    );

    for focus in [focus_before.as_str(), focus_after.as_str()] {
        infer
            .infer(
                &InferenceRequest {
                    events: &events,
                    focus_block: focus,
                    system: "",
                    tools: &[],
                },
                &cancel,
            )
            .await
            .expect("completes");
    }
    let prompts: Vec<String> = bridge
        .served()
        .iter()
        .map(|request| request.prompt.clone())
        .collect();
    assert_eq!(prompts.len(), 2);

    // The history region is everything before the trailing focus block.
    let history_bytes = prompts[0].len() - focus_before.len() - 1; // minus the join \n
    let shared = prompts[0]
        .as_bytes()
        .iter()
        .zip(prompts[1].as_bytes())
        .take_while(|(a, b)| a == b)
        .count();
    assert!(
        shared >= history_bytes,
        "a todo transition must not invalidate the history prefix: \
         divergence at byte {shared}, history region ends at byte {history_bytes}"
    );
    assert!(
        prompts[0].ends_with(&focus_before) && prompts[1].ends_with(&focus_after),
        "the volatile focus block rides the tail, never the head"
    );
    assert!(
        prompts[0].starts_with("user: "),
        "the history region is the head of the prompt"
    );
}
