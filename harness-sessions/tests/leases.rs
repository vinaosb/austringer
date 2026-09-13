//! Writer-lease scenarios: foreign writers, crashed writers, reboots.

use std::sync::Arc;

use harness_core::event::{ContentBlock, Event};
use harness_sessions::{
    Clock, FakeLeaseProbe, LeaseOwner, SessionError, SessionStore, StaleReason,
};

fn clock_at(start: u64) -> Clock {
    let tick = Arc::new(std::sync::atomic::AtomicU64::new(start));
    Arc::new(move || tick.load(std::sync::atomic::Ordering::SeqCst))
}

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

#[test]
fn live_foreign_writer_is_read_only_typed_never_force_unlock() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000), probe).unwrap();
    store.create("leased", "").unwrap();

    let writer_a = LeaseOwner {
        owner_id: "writer-a".to_owned(),
        pid: 4242,
    };
    let _handle_a = store.open_session_rw("leased", &writer_a).unwrap();

    let writer_b = LeaseOwner {
        owner_id: "writer-b".to_owned(),
        pid: 4243,
    };
    let error = store.open_session_rw("leased", &writer_b).unwrap_err();
    match &error {
        SessionError::LockedByLiveWriter { lease, .. } => {
            assert_eq!(lease.owner_id, "writer-a");
            assert_eq!(lease.pid, 4242);
        }
        other => panic!("expected LockedByLiveWriter, got {other:?}"),
    }
    // The session stays readable for the foreign writer.
    let ro = store.open_session("leased").unwrap();
    assert_eq!(ro.meta().id, "leased");
}

#[test]
fn crashed_writer_pid_dead_yields_typed_takeover_report() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000), probe.clone()).unwrap();
    store.create("crashed", "").unwrap();

    let ghost = LeaseOwner {
        owner_id: "ghost-writer".to_owned(),
        pid: 9999,
    };
    {
        let h = store.open_session_rw("crashed", &ghost).unwrap();
        // The writer crashes without releasing: the handle (and its
        // drop-time release) simply disappears.
        std::mem::forget(h);
    }
    probe.kill(9999);

    let survivor = LeaseOwner {
        owner_id: "survivor".to_owned(),
        pid: std::process::id(),
    };
    let h = store.open_session_rw("crashed", &survivor).unwrap();
    let report = h
        .takeover()
        .expect("stale lease must produce a takeover report");
    assert_eq!(report.previous.owner_id, "ghost-writer");
    assert_eq!(report.reason, StaleReason::PidDead { pid: 9999 });
    // Writes proceed after the takeover.
    h.append(user("after crash recovery")).unwrap();
}

#[test]
fn lease_across_simulated_reboot_is_stale_via_boot_id() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000), probe.clone()).unwrap();
    store.create("rebooted", "").unwrap();

    let old_writer = LeaseOwner {
        owner_id: "pre-reboot".to_owned(),
        pid: 7777,
    };
    {
        let h = store.open_session_rw("rebooted", &old_writer).unwrap();
        // The writer never released: machine "crashed" instead.
        std::mem::forget(h);
    }
    probe.reboot("boot-b");

    let post_boot = LeaseOwner {
        owner_id: "post-reboot".to_owned(),
        pid: 8888,
    };
    let h = store.open_session_rw("rebooted", &post_boot).unwrap();
    let report = h
        .takeover()
        .expect("boot-id change must produce a takeover");
    assert_eq!(
        report.reason,
        StaleReason::BootIdChanged {
            recorded: "boot-a".to_owned(),
            current: "boot-b".to_owned(),
        }
    );
}
