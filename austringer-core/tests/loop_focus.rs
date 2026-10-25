//! Focus-chain anchor (todo 5): serde-stable byte-identical rendering,
//! typed completion blocking, and the plan.md hard-block gate.

mod common;

use austringer_core::agent_loop::{CompletionBlock, FocusChain, PlanGate, TodoId};

fn chain_a() -> FocusChain {
    let mut chain = FocusChain::new();
    chain.upsert(TodoId(2), "second todo");
    chain.upsert(TodoId(1), "first todo");
    chain
}

fn chain_b() -> FocusChain {
    // Same logical state, opposite insertion order.
    let mut chain = FocusChain::new();
    chain.upsert(TodoId(1), "first todo");
    chain.upsert(TodoId(2), "second todo");
    chain
}

#[test]
fn render_is_byte_identical_for_identical_state_across_insertion_orders() {
    assert_eq!(chain_a().render(), chain_b().render());
    assert_eq!(chain_a().render(), chain_b().render());
}

#[test]
fn render_is_deterministic_and_lists_todos_sorted_by_id() {
    let rendered = chain_a().render();
    assert!(rendered.contains("<focus-chain>"));
    assert!(rendered.contains("</focus-chain>"));
    let first = rendered.find("first todo").expect("first todo listed");
    let second = rendered.find("second todo").expect("second todo listed");
    assert!(first < second, "todos must render sorted by id");
    assert!(
        rendered.contains("2 open / 2 total"),
        "counts derived: {rendered}"
    );
}

#[test]
fn closing_a_todo_changes_the_rendered_block_and_counts() {
    let mut chain = chain_a();
    let before = chain.render();
    assert_eq!(chain.open_count(), 2);
    assert!(chain.mark_done(TodoId(1)));
    let after = chain.render();
    assert_ne!(before, after);
    assert_eq!(chain.open_count(), 1);
    assert!(after.contains("1 open / 2 total"));
    assert!(!chain.mark_done(TodoId(1)), "already closed");
    assert!(!chain.mark_done(TodoId(99)), "unknown id rejected");
}

#[test]
fn completion_block_is_typed_and_lists_only_open_todos() {
    let mut chain = chain_a();
    assert!(chain.mark_done(TodoId(2)));
    let CompletionBlock::OpenTodos {
        open_count,
        rendered,
    } = chain
        .completion_block()
        .expect("blocked while todos remain open")
    else {
        panic!("expected OpenTodos variant");
    };
    assert_eq!(open_count, 1);
    assert!(rendered.contains("first todo"));
    assert!(!rendered.contains("second todo"));
    // All closed => no block at all.
    assert!(chain.mark_done(TodoId(1)));
    assert_eq!(chain.completion_block(), None);
}

#[test]
fn plan_gate_hard_block_is_a_wired_handle() {
    let gate = PlanGate::new();
    assert!(!gate.is_hard_blocked());
    gate.hard_block();
    assert!(gate.is_hard_blocked());
    gate.clear();
    assert!(!gate.is_hard_blocked());
    // Handle semantics: clones observe the same gate.
    let twin = gate.clone();
    twin.hard_block();
    assert!(gate.is_hard_blocked());
}
