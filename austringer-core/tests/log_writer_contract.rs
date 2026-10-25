//! Writer-contract tests (plan todo 4, acceptance 4): the disk-full /
//! IO-error contract via test doubles injecting failures into a real backing
//! file — read-only safe mode, loud notice event, file intact — plus
//! sequence monotonicity.

mod common;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use austringer_core::event::{Event, EventClass, RecordedEvent, Sequence};
use austringer_core::log::{AppendError, EVENTS_FILE_NAME, EventLogWriter, EventSink, FsyncPolicy};
use common::TempSession;

/// Failure mode injected by [`InjectingSink`].
enum Failure {
    /// Fail before writing any bytes, from the Nth append on.
    BeforeWrite,
    /// Write half the line, then fail, from the Nth append on.
    HalfWrite,
    /// Appends succeed; every fsync fails.
    Sync,
}

/// Real-file sink that injects IO failures. Implements the sink contract the
/// way the production sink does: `committed` only advances after a fully
/// successful append, so `discard_partial` restores the valid prefix.
struct InjectingSink {
    file: File,
    committed: u64,
    appends: usize,
    trigger: usize,
    inject: Failure,
}

impl InjectingSink {
    fn open(path: &Path, inject: Failure, trigger: usize) -> io::Result<Self> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let committed = file.seek(SeekFrom::End(0))?;
        Ok(Self {
            file,
            committed,
            appends: 0,
            trigger,
            inject,
        })
    }
}

impl EventSink for InjectingSink {
    fn append_line(&mut self, line: &[u8]) -> io::Result<()> {
        self.appends += 1;
        if matches!(self.inject, Failure::BeforeWrite) && self.appends >= self.trigger {
            return Err(io::Error::other(
                "injected write failure (simulated disk full)",
            ));
        }
        let torn = matches!(self.inject, Failure::HalfWrite) && self.appends >= self.trigger;
        let payload = if torn { &line[..line.len() / 2] } else { line };
        self.file.write_all(payload)?;
        self.file.flush()?;
        if torn {
            return Err(io::Error::other("injected failure mid-write"));
        }
        self.committed += line.len() as u64;
        Ok(())
    }

    fn sync(&mut self) -> io::Result<()> {
        match self.inject {
            Failure::Sync => Err(io::Error::other("injected fsync failure")),
            _ => self.file.sync_data(),
        }
    }

    fn discard_partial(&mut self) -> io::Result<()> {
        self.file.set_len(self.committed)?;
        self.file.seek(SeekFrom::Start(self.committed))?;
        Ok(())
    }
}

fn session_file(session: &TempSession) -> PathBuf {
    session.path().join(EVENTS_FILE_NAME)
}

fn system_event(text: &str) -> Event {
    Event::System {
        text: text.to_owned(),
    }
}

fn committed_line(seq: u64, event: &Event) -> String {
    let mut line = serde_json::to_string(&RecordedEvent {
        seq: Sequence(seq),
        event: event.clone(),
    })
    .expect("serialize");
    line.push('\n');
    line
}

#[test]
fn append_degrades_to_read_only_when_sink_fails_before_write() {
    let session = TempSession::new("degrade-before-write");
    let mut writer = EventLogWriter::with_sink(
        Box::new(
            InjectingSink::open(&session_file(&session), Failure::BeforeWrite, 3).expect("sink"),
        ),
        FsyncPolicy::Always,
        Sequence(0),
    );
    writer.append(system_event("one")).expect("append 1");
    writer.append(system_event("two")).expect("append 2");

    let error = writer
        .append(system_event("three"))
        .expect_err("third append must fail");

    let AppendError::DegradedToReadOnly { notice, .. } = &error else {
        panic!("expected DegradedToReadOnly, got: {error:?}");
    };
    // Loud: a system event (visible in the live conversation) naming the mode.
    assert_eq!(notice.class(), EventClass::LlmVisible);
    let Event::System { text } = notice else {
        panic!("notice must be a system event");
    };
    assert!(text.contains("READ-ONLY"), "notice must be loud: {text}");

    // Read-only safe mode: queryable state + fail-fast on further appends.
    assert!(writer.is_read_only());
    let degraded = writer.degraded().expect("degradation state");
    assert!(degraded.cause.contains("injected write failure"));
    assert!(matches!(
        writer.append(system_event("four")),
        Err(AppendError::ReadOnly)
    ));

    // File intact: exactly the two committed lines, nothing else.
    drop(writer);
    let expected = format!(
        "{}{}",
        committed_line(1, &system_event("one")),
        committed_line(2, &system_event("two"))
    );
    assert_eq!(
        fs::read(session_file(&session)).expect("read file"),
        expected.as_bytes(),
        "file must hold its valid prefix"
    );
}

#[test]
fn append_restores_valid_prefix_when_sink_fails_mid_write() {
    let session = TempSession::new("degrade-mid-write");
    let mut writer = EventLogWriter::with_sink(
        Box::new(
            InjectingSink::open(&session_file(&session), Failure::HalfWrite, 3).expect("sink"),
        ),
        FsyncPolicy::NeverForTests,
        Sequence(0),
    );
    writer.append(system_event("one")).expect("append 1");
    writer.append(system_event("two")).expect("append 2");

    let error = writer
        .append(system_event("three"))
        .expect_err("half-written append must fail");
    assert!(matches!(error, AppendError::DegradedToReadOnly { .. }));

    // The writer's recovery truncated the torn half-line back to the prefix.
    drop(writer);
    let expected = format!(
        "{}{}",
        committed_line(1, &system_event("one")),
        committed_line(2, &system_event("two"))
    );
    assert_eq!(
        fs::read(session_file(&session)).expect("read file"),
        expected.as_bytes(),
        "torn partial line must be rolled back"
    );

    let report = austringer_core::log::replay(session.path()).expect("replay");
    assert_eq!(report.events.len(), 2);
    assert!(report.anomalies.is_empty());
}

#[test]
fn append_degrades_to_read_only_when_fsync_fails_after_commit() {
    let session = TempSession::new("degrade-fsync");
    let mut writer = EventLogWriter::with_sink(
        Box::new(
            InjectingSink::open(&session_file(&session), Failure::Sync, usize::MAX).expect("sink"),
        ),
        FsyncPolicy::Always,
        Sequence(0),
    );
    let error = writer
        .append(system_event("committed but not durable"))
        .expect_err("sync failure must degrade the session");
    assert!(matches!(error, AppendError::DegradedToReadOnly { .. }));
    assert!(writer.is_read_only());

    // The flushed line is a valid prefix (it may simply not be durable yet).
    drop(writer);
    let report = austringer_core::log::replay(session.path()).expect("replay");
    assert_eq!(report.events.len(), 1);
    assert!(
        report.anomalies.is_empty(),
        "sync failure must not corrupt the log: {:?}",
        report.anomalies
    );
}

#[test]
fn sequences_increase_monotonically_when_events_appended() {
    let session = TempSession::new("monotonic");
    let mut writer =
        EventLogWriter::open(session.path(), FsyncPolicy::NeverForTests).expect("open");
    let mut previous = Sequence(0);
    for index in 0..10 {
        let seq = writer
            .append(system_event(&format!("event {index}")))
            .expect("append");
        assert!(seq.0 > previous.0, "sequence must strictly increase");
        previous = seq;
    }
    assert_eq!(writer.last_sequence(), Sequence(10));
}
