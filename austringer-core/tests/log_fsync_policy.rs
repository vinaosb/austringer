//! Fsync-policy tests: the writer's durability barrier must fire exactly
//! once per event (`Always`), once per snapshot marker (`OnSnapshot`), or
//! never (`NeverForTests`). Counted via an in-memory sink.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use austringer_core::event::{Event, Sequence};
use austringer_core::log::{EventLogWriter, EventSink, FsyncPolicy};

/// In-memory sink that only counts sync calls.
struct TallySink {
    syncs: Arc<AtomicUsize>,
}

impl EventSink for TallySink {
    fn append_line(&mut self, _line: &[u8]) -> io::Result<()> {
        Ok(())
    }

    fn sync(&mut self) -> io::Result<()> {
        self.syncs.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn discard_partial(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn system_event(text: &str) -> Event {
    Event::System {
        text: text.to_owned(),
    }
}

#[test]
fn fsync_policy_always_syncs_when_every_event_appended() {
    let syncs = Arc::new(AtomicUsize::new(0));
    let mut writer = EventLogWriter::with_sink(
        Box::new(TallySink {
            syncs: syncs.clone(),
        }),
        FsyncPolicy::Always,
        Sequence(0),
    );
    for index in 0..3 {
        writer
            .append(system_event(&format!("message {index}")))
            .expect("append");
    }
    writer
        .append(Event::Snapshot {
            label: "point".into(),
        })
        .expect("append snapshot");
    assert_eq!(syncs.load(Ordering::Relaxed), 4);
}

#[test]
fn fsync_policy_on_snapshot_syncs_when_only_snapshot_appended() {
    let syncs = Arc::new(AtomicUsize::new(0));
    let mut writer = EventLogWriter::with_sink(
        Box::new(TallySink {
            syncs: syncs.clone(),
        }),
        FsyncPolicy::OnSnapshot,
        Sequence(0),
    );
    for index in 0..3 {
        writer
            .append(system_event(&format!("message {index}")))
            .expect("append");
    }
    writer
        .append(Event::Snapshot {
            label: "point".into(),
        })
        .expect("append snapshot");
    assert_eq!(syncs.load(Ordering::Relaxed), 1);
}

#[test]
fn fsync_policy_never_for_tests_skips_when_events_appended() {
    let syncs = Arc::new(AtomicUsize::new(0));
    let mut writer = EventLogWriter::with_sink(
        Box::new(TallySink {
            syncs: syncs.clone(),
        }),
        FsyncPolicy::NeverForTests,
        Sequence(0),
    );
    for index in 0..4 {
        writer
            .append(system_event(&format!("message {index}")))
            .expect("append");
    }
    assert_eq!(syncs.load(Ordering::Relaxed), 0);
}
