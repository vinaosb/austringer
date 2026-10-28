//! Cold-restart replay: every existing restart/resume proof is
//! same-process (eviction_tests reopens a host inside ONE
//! DaemonState) or a live-attach resync (cli e2e, stub provider).
//! These tests pin the missing proof — a FRESH `DaemonState` opened
//! on an existing `data_dir` replays persisted history exactly: no
//! lost events, no duplicated events, no republish storm, and the
//! two durable transcripts (engine `events.jsonl`, sessions store)
//! agree after the restart and after a post-restart turn.
//!
//! Hermetic: scripted inference over a SHARED step queue (script
//! consumption survives the DaemonState re-creation, the
//! eviction_tests pattern), zero network, zero keys.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{InferenceError, InferenceRequest};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::{ContentBlock, Event, RecordedEvent, Sequence};
use austringer_daemon::attach::{HostCommand, SessionHost};
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::events::ReplayKind;
use austringer_daemon::journal::Journal;
use austringer_daemon::protocol::{ServerMsg, SessionEvent};
use austringer_daemon::server::DaemonState;
use austringer_daemon::usage::{ComposedInference, InferenceFactory};

/// Shared scripted steps + the history slice each inference call saw
/// (the replay oracle: `InferenceRequest::events` IS the model context
/// the restarted daemon would send — persisted history must ride it
/// whole, or the restart silently lost state).
type Steps = Arc<Mutex<VecDeque<AssistantTurn>>>;
type Seen = Arc<Mutex<Vec<Vec<RecordedEvent>>>>;
/// One submit's publication: (TurnStarted label, recorded (seq,
/// event) pairs framed before TurnFinished, outcome summary).
type Batch = (Option<u64>, Vec<(u64, Event)>, Option<String>);

fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// (shared step queue, shared capture) — script consumption survives
/// the DaemonState re-creation.
struct RestartScript(Steps, Seen);

#[async_trait]
impl Inference for RestartScript {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        locked(&self.1).push(request.events.to_vec());
        locked(&self.0)
            .pop_front()
            .ok_or_else(|| InferenceError::Failure("exhausted".into()))
    }
}

/// One factory per DaemonState lifetime, both over the same shared
/// queue and capture — a cold restart pops the NEXT scripted step.
fn factory(steps: &Steps, seen: &Seen) -> InferenceFactory {
    let (steps, seen) = (Arc::clone(steps), Arc::clone(seen));
    Arc::new(move || {
        ComposedInference::plain(Box::new(RestartScript(
            Arc::clone(&steps),
            Arc::clone(&seen),
        )))
    })
}

fn script(texts: &[&str]) -> Steps {
    let turns = texts.iter().copied().map(|t| AssistantTurn {
        blocks: vec![ContentBlock::Text { text: t.to_owned() }],
        tool_calls: Vec::new(),
        usage: None,
    });
    Arc::new(Mutex::new(turns.collect()))
}

fn daemon_at(dir: &Path, inference: InferenceFactory) -> Arc<DaemonState> {
    let config = DaemonConfig {
        data_dir: dir.to_owned(),
        token: Some("restart-token".into()),
        ..DaemonConfig::default()
    };
    Arc::new(DaemonState::new(config, inference).expect("daemon state"))
}

/// Every text fragment a recorded event carries.
fn event_texts(event: &Event) -> Vec<String> {
    let (Event::User { blocks } | Event::Assistant { blocks }) = event else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Comparable transcript fingerprint — (seq, texts) per event — used
/// across the engine log, the sessions store, and the inference
/// mirror, so a persist-vs-replay divergence cannot hide anywhere.
fn fingerprint(events: &[RecordedEvent]) -> Vec<(u64, Vec<String>)> {
    events
        .iter()
        .map(|r| (r.seq.0, event_texts(&r.event)))
        .collect()
}

fn all_texts(events: &[RecordedEvent]) -> Vec<String> {
    events.iter().flat_map(|r| event_texts(&r.event)).collect()
}

/// The sessions-store rows for one session (the search/history
/// surface's persisted truth), read fresh after each mutation phase.
fn store_events(data_dir: &Path, sid: &str) -> Vec<RecordedEvent> {
    let store = austringer_sessions::SessionStore::open(data_dir).expect("sessions store");
    let handle = store.open_session(sid).expect("session row");
    handle
        .events_range(Sequence(1), Sequence(handle.meta().last_seq))
        .expect("stored events")
}

/// Seqs must be exactly 1..=len — gap-free from the head.
fn assert_gap_free(seqs: &[u64], what: &str) {
    let want: Vec<u64> = (1..=(seqs.len() as u64)).collect();
    assert_eq!(seqs, want, "{what} must replay gap-free from seq 1");
}

fn assert_ok(batch: &Batch, what: &str) {
    assert!(
        batch.2.as_deref().is_some_and(|o| !o.starts_with("failed")),
        "{what}: {:?}",
        batch.2
    );
}

async fn submit_and_collect(host: &Arc<SessionHost>, prompt: &str) -> Batch {
    let mut live = host.hub_subscribe();
    host.handle_command(HostCommand::Submit(prompt.into()))
        .await
        .expect("submit");
    tokio::time::timeout(Duration::from_secs(30), async {
        let (mut started, mut recorded) = (None, Vec::new());
        loop {
            match live.recv().await {
                Ok(ServerMsg::Event { frame }) => match frame.event {
                    SessionEvent::TurnStarted { turn } => started = Some(turn),
                    SessionEvent::Recorded { seq, event } => recorded.push((seq, event)),
                    SessionEvent::TurnFinished { outcome, .. } => {
                        return (started, recorded, Some(outcome));
                    }
                    _ => {}
                },
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    panic!("live lane lagged by {n}: republish storm?")
                }
                Err(error) => panic!("hub died: {error:?}"),
            }
        }
    })
    .await
    .expect("turn never finished")
}

/// Scenario A — cold restart replays persisted history exactly.
/// state1 runs two turns and is dropped COMPLETELY; a fresh
/// DaemonState on the same data_dir must (a) NOT re-publish the
/// replayed history on its fresh hub (a cold attach plans a resync
/// from id 0), yet serve it whole to the next inference (no loss —
/// the model context survives), (b) append the post-restart turn
/// AFTER the persisted head (contiguous engine seqs, no duplication,
/// no republish storm), and (c) leave the sessions store holding old
/// + new events exactly once, gap-free.
#[tokio::test]
async fn restart_replay_republishes_nothing_and_appends_after_persisted_history() {
    const SID: &str = "s-restart";
    let dir = tempfile::tempdir().expect("tempdir");
    let steps = script(&["first answer", "second answer", "post-restart answer"]);
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));

    let state1 = daemon_at(dir.path(), factory(&steps, &seen));
    let host1 = state1.session(SID);
    assert_ok(&submit_and_collect(&host1, "one").await, "turn one");
    assert_ok(&submit_and_collect(&host1, "two").await, "turn two");

    // Persisted before the restart: the store sees both turns' answers.
    let before = store_events(dir.path(), SID);
    let texts = all_texts(&before);
    assert!(texts.iter().any(|t| t.contains("first answer")));
    assert!(texts.iter().any(|t| t.contains("second answer")));
    let before_fp = fingerprint(&before);
    let max_seq = before.last().map(|r| r.seq.0).expect("persisted events");

    // COLD RESTART: the daemon (state + host + engine) is dropped
    // entirely; the yield lets state1's engine task observe its closed
    // command lane before state2 opens the same data_dir.
    drop((host1, state1));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let state2 = daemon_at(dir.path(), factory(&steps, &seen));
    let host2 = state2.session(SID); // AgentLoop::open replays events.jsonl here

    // (a) No republish: a fresh attach plans a resync from id 0 — the
    // hub never framed the replayed history onto the live lane.
    assert_eq!(
        host2.attach("tui", None).expect("attach").replay,
        ReplayKind::Resync { last_event_id: 0 }
    );

    // (b) The post-restart turn appends AFTER the persisted history.
    let batch3 = submit_and_collect(&host2, "three").await;
    assert_ok(&batch3, "turn three");
    let seqs: Vec<u64> = batch3.1.iter().map(|(s, _)| *s).collect();
    let leaked: Vec<String> = batch3.1.iter().flat_map(|(_, e)| event_texts(e)).collect();
    assert!(
        !seqs.is_empty()
            && seqs.iter().all(|s| *s > max_seq)
            && leaked
                .iter()
                .all(|t| !t.contains("first answer") && !t.contains("second answer")),
        "turn three must record only NEW events past head {max_seq}: {seqs:?} {leaked:?}"
    );
    assert_eq!(
        seqs,
        ((max_seq + 1)..=(max_seq + seqs.len() as u64)).collect::<Vec<_>>(),
        "engine seqs must stay contiguous past the head"
    );
    // The host turn counter restarts with the process — nothing seeds
    // it from replayed history (pinned AS IMPLEMENTED; the contiguous
    // engine seq stream above is the continuity contract that matters).
    assert_eq!(batch3.0, Some(1));

    // (a, no loss) the post-restart inference saw the FULL history:
    // the persisted prefix rides the request seq-for-seq, then the new
    // prompt — the model context survived the restart intact.
    let calls = locked(&seen).clone();
    assert_eq!(calls.len(), 3, "one provider call per turn");
    let third = &calls[2];
    assert_eq!(
        fingerprint(&third[..before.len()]),
        before_fp,
        "the replayed prefix must match the persisted store rows exactly (request len {})",
        third.len()
    );
    let tail = all_texts(&third[before.len()..]);
    assert!(tail.iter().any(|t| t.contains("three")));
    let mirror: Vec<u64> = third.iter().map(|r| r.seq.0).collect();
    assert_gap_free(&mirror, "the mirror");

    // (c) The store holds old + new exactly once, gap-free.
    let after = store_events(dir.path(), SID);
    assert_eq!(
        after.len(),
        before.len() + batch3.1.len(),
        "store rows: old + new, exactly once each"
    );
    assert_eq!(
        fingerprint(&after[..before.len()]),
        before_fp,
        "the persisted prefix must be untouched by the restart"
    );
    let after_texts = all_texts(&after);
    for marker in ["first answer", "second answer", "post-restart answer"] {
        assert_eq!(
            after_texts.iter().filter(|t| t.contains(marker)).count(),
            1,
            "{marker} exactly once: {after_texts:?}"
        );
    }
    let store_seqs: Vec<u64> = after.iter().map(|r| r.seq.0).collect();
    assert_gap_free(&store_seqs, "store rows");
}

/// Scenario B — the durable replay paths agree. Resume reads three
/// artifacts: the engine `events.jsonl` (replayed by AgentLoop::open
/// into the mirror and published_through — the documented winner:
/// "log is the durable truth"), the sessions store (the live feed the
/// search/history surfaces read), and the durable turn journals
/// (workflow forensics; their replay surface is open-turn detection).
/// After a cold restart plus a post-restart turn, the two transcripts
/// must be IDENTICAL and every turn journal must be closed (resume
/// never sees a phantom in-flight turn). Note: the host turn counter
/// restarts with the process, so the post-restart turn's workflow
/// appends into the previous run's lowest-numbered journal FILE —
/// pinned as implemented (closure below is the resume-safety part).
#[tokio::test]
async fn restart_replay_engine_log_and_sessions_store_agree_and_journals_close() {
    const SID: &str = "s-agree";
    let dir = tempfile::tempdir().expect("tempdir");
    let steps = script(&["alpha answer", "beta answer", "gamma answer"]);
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));

    let state1 = daemon_at(dir.path(), factory(&steps, &seen));
    let host1 = state1.session(SID);
    assert_ok(&submit_and_collect(&host1, "one").await, "one");
    assert_ok(&submit_and_collect(&host1, "two").await, "two");
    drop((host1, state1));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let state2 = daemon_at(dir.path(), factory(&steps, &seen));
    let host2 = state2.session(SID);
    assert_ok(&submit_and_collect(&host2, "three").await, "turn three");

    // The two durable transcripts agree: same count, same seqs, same
    // texts. A persist-vs-replay divergence (a turn that landed in the
    // engine log but not the store, or vice versa) fails here.
    let engine = austringer_core::log::replay(&dir.path().join("sessions").join(SID))
        .expect("engine log replay")
        .events;
    let store = store_events(dir.path(), SID);
    assert!(!engine.is_empty() && engine.len() == store.len());
    assert_eq!(
        fingerprint(&engine),
        fingerprint(&store),
        "the two durable transcripts must agree seq-for-seq"
    );

    // Durable journals (default ON): every turn workflow file is CLOSED
    // — no journal may leave an open turn behind a clean restart.
    let journal_dir = dir.path().join("journal").join(SID);
    let journals: Vec<std::path::PathBuf> = std::fs::read_dir(&journal_dir)
        .expect("journal dir")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        .collect();
    assert!(
        !journals.is_empty(),
        "durable default must journal turns under {journal_dir:?}"
    );
    for path in &journals {
        let journal = Journal::open(path).expect("journal open");
        assert!(
            journal.open_turn_prompt().is_none(),
            "no open turn left in {path:?}"
        );
    }
}
