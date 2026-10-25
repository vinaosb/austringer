//! Replay-performance acceptance (plan todo 4, acceptance 3): a 10k-event
//! synthetic session must write and replay with an exact count and content
//! match. Duration is measured and recorded; the assertion budget is
//! deliberately generous (CI hosts vary) while correctness is deterministic.

mod common;

use std::time::{Duration, Instant};

use austringer_core::event::Sequence;
use austringer_core::log::{EventLogWriter, FsyncPolicy, replay};
use common::{TempSession, XorShift64};

const EVENT_COUNT: usize = 10_000;
/// Generous wall-clock budget; actual duration is recorded in the evidence.
const REPLAY_BUDGET: Duration = Duration::from_secs(30);

#[test]
fn replay_recovers_10k_events_when_synthetic_session_written() {
    let session = TempSession::new("perf-10k");
    let mut rng = XorShift64::new(2026_0912);
    let events: Vec<_> = (0..EVENT_COUNT).map(|_| rng.event()).collect();

    let mut writer =
        EventLogWriter::open(session.path(), FsyncPolicy::NeverForTests).expect("open writer");
    let write_started = Instant::now();
    for event in &events {
        writer.append(event.clone()).expect("append");
    }
    let write_duration = write_started.elapsed();
    drop(writer);

    let replay_started = Instant::now();
    let report = replay(session.path()).expect("replay");
    let replay_duration = replay_started.elapsed();

    assert!(
        report.anomalies.is_empty(),
        "clean session must replay without anomalies: {:?}",
        report.anomalies
    );
    assert_eq!(report.events.len(), EVENT_COUNT, "exact count required");
    // Exact content match, not just count.
    for (index, recorded) in report.events.iter().enumerate() {
        assert_eq!(recorded.seq, Sequence(index as u64 + 1), "seq at {index}");
        assert_eq!(&recorded.event, &events[index], "content at {index}");
    }
    assert!(
        replay_duration < REPLAY_BUDGET,
        "replay exceeded budget: {replay_duration:?}"
    );
    eprintln!(
        "10k-event session: write={write_duration:?} replay={replay_duration:?} \
         (budget {REPLAY_BUDGET:?})"
    );
}
