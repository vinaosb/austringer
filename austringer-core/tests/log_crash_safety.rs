//! Crash-safety tests for the session log reader/writer (plan todo 4,
//! acceptance 2): torn final lines, invalid lines mid-file, sequence gaps,
//! and reopen paths. All inputs are deterministic; no wall-clock dependence.

mod common;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use austringer_core::event::{Event, RecordedEvent, Sequence};
use austringer_core::log::{Anomaly, EVENTS_FILE_NAME, EventLogWriter, FsyncPolicy, replay};
use common::{TempSession, XorShift64};

fn events_from_seed(seed: u64, count: u64) -> Vec<Event> {
    let mut rng = XorShift64::new(seed);
    (0..count).map(|_| rng.event()).collect()
}

fn log_path(session: &TempSession) -> PathBuf {
    session.path().join(EVENTS_FILE_NAME)
}

fn system_event(text: &str) -> Event {
    Event::System {
        text: text.to_owned(),
    }
}

#[test]
fn replay_ignores_torn_final_line_when_crash_left_partial_write() {
    // Given: 5 committed events, then a kill-9 style half line (no newline).
    let session = TempSession::new("torn-tail");
    let originals = events_from_seed(5, 5);
    let mut writer =
        EventLogWriter::open(session.path(), FsyncPolicy::NeverForTests).expect("open writer");
    for event in &originals {
        writer.append(event.clone()).expect("append event");
    }
    drop(writer);

    let uncommitted = RecordedEvent {
        seq: Sequence(6),
        event: originals[0].clone(),
    };
    let full = serde_json::to_string(&uncommitted).expect("serialize doomed event");
    let mut file = OpenOptions::new()
        .append(true)
        .open(log_path(&session))
        .expect("open log for raw append");
    file.write_all(&full.as_bytes()[..full.len() / 2])
        .expect("write torn half line");
    file.flush().expect("flush torn half line");
    drop(file);

    // When: the session is replayed.
    let report = replay(session.path()).expect("replay session");

    // Then: all committed events recovered with exact content; the torn line
    // is ignored and reported as a recoverable anomaly — never an error.
    assert_eq!(report.events.len(), 5, "committed events must survive");
    for (index, recorded) in report.events.iter().enumerate() {
        assert_eq!(recorded.seq, Sequence(index as u64 + 1));
        assert_eq!(&recorded.event, &originals[index], "event {index} diverged");
    }
    assert_eq!(
        report.anomalies,
        vec![Anomaly::TornFinalLine {
            bytes: full.len() / 2
        }],
        "torn tail must be reported as an anomaly"
    );
}

#[test]
fn reopen_after_torn_tail_truncates_then_appends_cleanly() {
    // Given: 5 committed events plus a torn tail.
    let session = TempSession::new("reopen-torn");
    let originals = events_from_seed(6, 5);
    let mut writer =
        EventLogWriter::open(session.path(), FsyncPolicy::NeverForTests).expect("open writer");
    for event in &originals {
        writer.append(event.clone()).expect("append event");
    }
    drop(writer);
    let uncommitted = serde_json::to_string(&RecordedEvent {
        seq: Sequence(6),
        event: originals[0].clone(),
    })
    .expect("serialize");
    let mut file = OpenOptions::new()
        .append(true)
        .open(log_path(&session))
        .expect("open log");
    file.write_all(uncommitted.as_bytes())
        .expect("write partial");
    drop(file);

    // When: the writer reopens (must truncate the uncommitted tail) and
    // appends one more event.
    let mut writer =
        EventLogWriter::open(session.path(), FsyncPolicy::NeverForTests).expect("reopen writer");
    let seq = writer
        .append(system_event("after recovery"))
        .expect("append after recovery");

    // Then: the new event takes sequence 6 (the torn line never existed) and
    // a full replay is anomaly-free.
    assert_eq!(seq, Sequence(6));
    drop(writer);
    let report = replay(session.path()).expect("replay");
    assert_eq!(report.events.len(), 6);
    assert!(
        report.anomalies.is_empty(),
        "recovered session must replay cleanly: {:?}",
        report.anomalies
    );
    assert_eq!(report.events[5].seq, Sequence(6));
    assert_eq!(report.events[5].event, system_event("after recovery"));
}

#[test]
fn replay_reports_invalid_midfile_line_as_anomaly_not_error() {
    // Given: 4 committed events with line 2 corrupted in place.
    let session = TempSession::new("invalid-mid");
    let originals = events_from_seed(7, 4);
    let mut writer =
        EventLogWriter::open(session.path(), FsyncPolicy::NeverForTests).expect("open writer");
    for event in &originals {
        writer.append(event.clone()).expect("append event");
    }
    drop(writer);

    let content = fs::read_to_string(log_path(&session)).expect("read log");
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_owned).collect();
    assert_eq!(lines.len(), 4);
    lines[1] = "{torn garbage: not json\n".to_owned();
    fs::write(log_path(&session), lines.concat()).expect("rewrite log");

    // When: replaying.
    let report = replay(session.path()).expect("replay must not hard-error");

    // Then: events 1, 3, 4 recovered with exact content; the bad line and
    // the resulting sequence jump are both reported as anomalies.
    assert_eq!(report.events.len(), 3);
    assert_eq!(report.events[0].event, originals[0]);
    assert_eq!(report.events[1].event, originals[2]);
    assert_eq!(report.events[2].event, originals[3]);
    assert_eq!(report.anomalies.len(), 2);
    assert!(matches!(
        &report.anomalies[0],
        Anomaly::InvalidLine { line: 2, .. }
    ));
    assert_eq!(
        report.anomalies[1],
        Anomaly::UnexpectedSequence {
            line: 3,
            expected: Sequence(2),
            found: Sequence(3),
        }
    );
}

#[test]
fn replay_reports_sequence_gap_as_anomaly() {
    // Given: a hand-crafted log whose second line jumps from seq 1 to seq 3.
    let session = TempSession::new("seq-gap");
    fs::create_dir_all(session.path()).expect("create session dir");
    let first = RecordedEvent {
        seq: Sequence(1),
        event: system_event("one"),
    };
    let third = RecordedEvent {
        seq: Sequence(3),
        event: system_event("three"),
    };
    fs::write(
        log_path(&session),
        format!(
            "{}\n{}\n",
            serde_json::to_string(&first).expect("serialize"),
            serde_json::to_string(&third).expect("serialize")
        ),
    )
    .expect("write crafted log");

    // When: replaying.
    let report = replay(session.path()).expect("replay");

    // Then: both events recovered; the gap reported exactly.
    assert_eq!(report.events.len(), 2);
    assert_eq!(
        report.anomalies,
        vec![Anomaly::UnexpectedSequence {
            line: 2,
            expected: Sequence(2),
            found: Sequence(3),
        }]
    );
}
