//! `/rewind` acceptance (plan todo 5): ClientMsg::Rewind restores a
//! austringer-snapshots restore point, truncates the persisted session log
//! to the restore head (search never sees un-happened events), and
//! broadcasts a System "restored to <label>" event on the normal
//! stream. Beyond-oldest is a typed error naming the oldest snapshot.

mod common;

use futures_util::{SinkExt, StreamExt};

use austringer_core::event::{ContentBlock, Event};
use austringer_sessions::SessionStore;
use austringer_sessions::lease::LeaseOwner;
use austringer_snapshots::ids::{SessionId, Turn};
use austringer_snapshots::{SnapshotOptions, SnapshotStore};

use common::{spawn_daemon, ws_hello};

const SID: &str = "rewind-e2e";

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

/// The persisted session log the daemon will truncate: three turns,
/// each a user event followed by an `Event::Snapshot` restore marker.
fn fixture_events() -> Vec<Event> {
    vec![
        user("alpha turn one"),
        Event::Snapshot {
            label: "turn-1".into(),
        },
        user("beta turn two"),
        Event::Snapshot {
            label: "turn-2".into(),
        },
        user("gamma turn three"),
        Event::Snapshot {
            label: "turn-3".into(),
        },
    ]
}

struct RewindFixture {
    daemon: common::Spawned,
    notes: std::path::PathBuf,
}

/// Spawn the daemon, create the live session, persist its log in the
/// daemon-rooted sessions store, and take three no-git snapshots of
/// the session-scoped workspace (notes.txt at v1/v2/v3 per turn).
async fn fixture(sid: &str) -> RewindFixture {
    let daemon = spawn_daemon(|c| c)
        .await
        .unwrap_or_else(|e| panic!("spawn daemon: {e}"));
    let _host = daemon.state.session(sid);
    let session_dir = daemon.state.config.data_dir.join("sessions").join(sid);
    assert!(
        session_dir.is_dir(),
        "session factory materializes the session dir"
    );

    {
        let store = SessionStore::open(&daemon.state.config.data_dir).unwrap();
        store.create(sid, "rewind e2e").unwrap();
        {
            let handle = store
                .open_session_rw(sid, &LeaseOwner::new("fixture"))
                .unwrap();
            handle.append_batch(&fixture_events()).unwrap();
        }
    }

    let notes = session_dir.join("notes.txt");
    let snapshots = SnapshotStore::open(&session_dir).unwrap();
    let session = SessionId::new(sid).unwrap();
    for (turn, content) in [(1u32, "v1"), (2, "v2"), (3, "v3")] {
        std::fs::write(&notes, content).unwrap();
        snapshots
            .snapshot(
                &session,
                Turn::new(turn).unwrap(),
                &SnapshotOptions::default(),
            )
            .unwrap();
    }
    drop(snapshots);

    RewindFixture { daemon, notes }
}

/// Attach, send `ClientMsg::Rewind { steps }`, and collect frames until
/// one matches `stop` (or the socket closes / the frame cap trips).
async fn send_rewind(
    daemon: &common::Spawned,
    sid: &str,
    steps: u64,
    client: &str,
    stop: impl Fn(&str) -> bool,
) -> Vec<String> {
    let url = format!("ws://{}/s/{sid}/attach?token={}", daemon.base, daemon.token);
    let mut socket = ws_hello(&url, &daemon.token, client, None)
        .await
        .unwrap_or_else(|e| panic!("attach: {e}"));
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({"type": "rewind", "steps": steps})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let mut frames = Vec::new();
    while let Some(Ok(msg)) = socket.next().await {
        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
            let text = text.to_string();
            let matched = stop(&text);
            frames.push(text);
            if matched || frames.len() > 64 {
                break;
            }
        }
    }
    frames
}

#[tokio::test]
async fn rewind_restores_snapshot_truncates_log_and_emits_system_event() {
    let fixture = fixture(SID).await;

    // Sanity: the fixture state at the latest snapshot.
    assert_eq!(std::fs::read_to_string(&fixture.notes).unwrap(), "v3");

    let frames = send_rewind(&fixture.daemon, SID, 2, "tui", |frame| {
        frame.contains("restored to turn 2") || frame.contains("\"error\"")
    })
    .await;
    let restored = frames
        .iter()
        .find(|frame| frame.contains("restored to turn 2"))
        .unwrap_or_else(|| panic!("no restored event in {frames:?}"));
    assert!(restored.contains("\"recorded\""), "rides the event stream");

    // Workspace restored to the turn-2 snapshot (roundtrip: the file
    // matches the fixture content snapshotted at turn 2).
    assert_eq!(
        std::fs::read_to_string(&fixture.notes).unwrap(),
        "v2",
        "workspace file must match the turn-2 snapshot fixture"
    );

    // Persisted log truncated to the restore head (turn 2 ends at seq
    // 4): counters reset, search never returns turn-3 text.
    let store = SessionStore::open(&fixture.daemon.state.config.data_dir).unwrap();
    let meta = store.open_session(SID).unwrap().meta().clone();
    assert_eq!(meta.last_seq, 4);
    assert_eq!(meta.event_count, 4);
    assert_eq!(meta.turn_count, 2);
    assert!(
        store.search_events("gamma", 10).unwrap().is_empty(),
        "un-happened events must not be searchable"
    );
    assert_eq!(store.turn_end_seq(SID, 2).unwrap(), Some(4));
}

#[tokio::test]
async fn rewind_beyond_oldest_snapshot_is_typed_error_naming_oldest() {
    let fixture = fixture("rewind-fail").await;

    let frames = send_rewind(&fixture.daemon, "rewind-fail", 99, "tui", |frame| {
        frame.contains("\"error\"")
    })
    .await;
    let error = frames
        .iter()
        .find(|frame| frame.contains("\"error\""))
        .unwrap_or_else(|| panic!("no error frame in {frames:?}"));
    assert!(
        error.contains("oldest"),
        "the refusal must name the oldest snapshot: {error}"
    );
    assert!(
        error.contains("turn 1"),
        "the refusal must name the oldest available label: {error}"
    );

    // Nothing happened: the workspace and the log are untouched.
    assert_eq!(std::fs::read_to_string(&fixture.notes).unwrap(), "v3");
    let store = SessionStore::open(&fixture.daemon.state.config.data_dir).unwrap();
    assert_eq!(
        store.open_session("rewind-fail").unwrap().meta().last_seq,
        6
    );
}
