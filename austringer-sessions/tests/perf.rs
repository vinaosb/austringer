//! Performance acceptance: 100 MB session opens < 50 ms; page-in
//! < 5 ms per 100 turns. Strict budgets are asserted in release
//! builds; debug builds get a 10x factor that still fails if open ever
//! scans payloads (deserializing 100 MB takes seconds in any profile).
//! Evidence timings are printed — run with `-- --nocapture`.

use std::time::Instant;

use austringer_core::event::{ContentBlock, Event};
use austringer_sessions::{LeaseOwner, Sequence, SessionStore};

fn user(text: String) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text { text }],
    }
}

fn budget_scale_ms(strict: u64) -> u64 {
    if cfg!(debug_assertions) {
        strict * 10
    } else {
        strict
    }
}

fn build_session(store: &SessionStore, id: &str, turns: usize, body_bytes: usize) {
    store.create(id, "").unwrap();
    let owner = LeaseOwner::new("perf-builder");
    let handle = store.open_session_rw(id, &owner).unwrap();
    let question = "explain the module layout and justify the layering".to_owned();
    let answer = "y".repeat(body_bytes);
    for turn in 1..=turns {
        handle
            .append_batch(&[
                user(format!("{turn}: {question}")),
                Event::Assistant {
                    blocks: vec![ContentBlock::Text {
                        text: answer.clone(),
                    }],
                },
                Event::ToolCall {
                    call_id: format!("c-{turn}"),
                    tool: "view_window".to_owned(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                },
            ])
            .unwrap();
    }
}

#[test]
fn hundred_megabyte_session_opens_under_fifty_ms() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    // 2,000 turns x (50 KB assistant answer + overhead) ≈ 100 MB.
    build_session(&store, "giant", 2_000, 50 * 1024);
    let handle = store.open_session("giant").unwrap();
    assert!(
        handle.meta().payload_bytes >= 95 * 1024 * 1024,
        "test fixture must really be ~100MB, got {} bytes",
        handle.meta().payload_bytes
    );
    // Drop: open() must be measured cold each time.
    drop(handle);

    let mut worst = std::time::Duration::ZERO;
    for _ in 0..5 {
        let start = Instant::now();
        let handle = store.open_session("giant").unwrap();
        let elapsed = start.elapsed();
        worst = worst.max(elapsed);
        drop(handle);
    }
    println!("100MB session open (worst of 5): {worst:?}");
    assert!(
        worst.as_millis() < u128::from(budget_scale_ms(50)),
        "open took {worst:?}; open() must read metadata + index bounds only"
    );
}

#[test]
fn page_in_of_one_hundred_turns_under_five_ms() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    // 2,000 realistic turns (~1 KB payloads per event).
    build_session(&store, "deep", 2_000, 700);
    let handle = store.open_session("deep").unwrap();
    assert_eq!(handle.meta().turn_count, 2_000);

    let start = Instant::now();
    let page = handle.turns_range(950, 1_049).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(page.len(), 100);
    // CONTENT, not counts: the first paged event is turn 950's user
    // message, which carries its own turn number.
    assert!(matches!(
        &page[0].events[0].event,
        Event::User { blocks } if blocks.iter().any(|b| matches!(b, ContentBlock::Text { text } if text.starts_with("950:")))
    ));
    println!("page-in of 100 turns: {elapsed:?}");
    assert!(
        elapsed.as_millis() < u128::from(budget_scale_ms(5)),
        "100-turn page-in took {elapsed:?}"
    );
}

#[test]
fn fork_of_large_session_stays_efficient() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    build_session(&store, "big", 500, 50 * 1024);
    let start = Instant::now();
    let meta = store
        .fork("big-fork", "big", Sequence(1_000), &Default::default())
        .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(meta.event_count, 1_000);
    println!("fork of 500-turn/25MB session at midpoint: {elapsed:?}");
    // CI receipt 2026-10-26 (PR #22 windows leg): a saturated 2-core
    // runner took 23.6 s against the 20 s scaled budget. The budget's
    // job is catching per-row rescan regressions (quadratic = minutes
    // at this size); 5 s strict (50 s scaled in debug) still does.
    assert!(
        elapsed.as_millis() < u128::from(budget_scale_ms(5_000)),
        "row-copy fork of 25MB took {elapsed:?} (budget: one-shot copy, not per-row rescans)"
    );
}
