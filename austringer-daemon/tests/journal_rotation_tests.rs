//! Storage-hygiene wave 1: size-capped rotation for the three
//! append-only journals (`{turn}.jsonl` turn/child journals,
//! `runs.jsonl`, `scheduler-journal.jsonl`). Every test drives the
//! REAL append paths against tempdir-backed files; caps are chosen
//! smaller than one record so rotation timing is deterministic
//! (rotate happens before a record write, never mid-record).

use std::path::Path;
use std::sync::{Arc, Mutex};

use austringer_daemon::automations::{AutomationDefinition, NetworkPolicy};
use austringer_daemon::bridge::RunSink;
use austringer_daemon::durable::{Journal, JournalStep, activity_id, turn_journal_path};
use austringer_daemon::gate::{CompletionGate, CriteriaGate};
use austringer_daemon::kickoff::{KickError, KickOff, RunKicker};
use austringer_daemon::notify::{MemorySink, Notifier};
use austringer_daemon::run_history::{RunHistory, RunRecord};
use austringer_daemon::runner::CronRunner;

/// Cap smaller than any serialized record line: the first append
/// always lands in a fresh file, the second always rotates first.
const TINY_CAP: u64 = 8;

fn generation(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().expect("file name").to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

fn started_step(n: u64) -> JournalStep {
    JournalStep::ActivityStarted {
        id: activity_id("t", &serde_json::json!(n)).0,
        name: "t".into(),
        input: serde_json::json!(n),
    }
}

fn record(session: &str, outcome: &str) -> RunRecord {
    RunRecord {
        automation_id: "nightly".into(),
        session_id: session.into(),
        fired_at_minute: 600,
        outcome: outcome.into(),
        verdict: None,
        network: NetworkPolicy::Online,
    }
}

// --- turn / child journals (Journal) -------------------------------

#[test]
fn turn_journal_rotates_at_cap_and_appends_continue() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = turn_journal_path(dir.path(), "s1", 1);
    let turn_started = JournalStep::TurnStarted {
        turn: 1,
        prompt: "first".into(),
    };
    let second = started_step(2);
    // Cap sized so the first two records fit and the third rotates
    // exactly once (newline bytes included) — deterministic without
    // pinning serialized byte counts.
    let cap = (serde_json::to_vec(&turn_started)?.len() + serde_json::to_vec(&second)?.len() + 2)
        as u64
        - 1;
    let mut journal = Journal::open_capped(&path, cap)?;
    journal.record(turn_started)?;
    journal.record(second)?;
    // Third record: the current file now exceeds the cap, so it
    // rotates to `.1` and the record lands in a fresh current file.
    journal.record(started_step(3))?;
    journal.record(JournalStep::TurnFinished {
        outcome: "done".into(),
    })?;

    let rotated = generation(&path);
    assert!(rotated.exists(), "no .1 generation after rotation");
    assert!(path.exists(), "fresh current file missing");

    // Appends survived: the newest step is the last complete line of
    // the CURRENT file; the oldest steps moved to `.1`.
    let current = std::fs::read_to_string(&path)?;
    let forensic = std::fs::read_to_string(&rotated)?;
    assert!(
        forensic.contains("turn_started"),
        "oldest history must move to .1: {forensic}"
    );
    assert!(
        !current.contains("turn_started"),
        "current must hold only post-rotation records: {current}"
    );
    let last_line = current.lines().last().unwrap_or_default();
    let parsed: JournalStep = serde_json::from_str(last_line)?;
    assert_eq!(
        parsed,
        JournalStep::TurnFinished {
            outcome: "done".into()
        }
    );
    // Appends continued into the fresh current file (two records).
    assert_eq!(Journal::open(&path)?.steps().len(), 2);
    Ok(())
}

#[test]
fn journal_replay_after_rotation_reads_current_and_keeps_latest_state()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = turn_journal_path(dir.path(), "s1", 3);
    // Long prompt so the TurnStarted line alone exceeds the cap while
    // the whole activity triple stays current after one rotation.
    let turn_started = JournalStep::TurnStarted {
        turn: 3,
        prompt: "rotate me away ".repeat(24),
    };
    let cap = serde_json::to_vec(&turn_started)?.len() as u64;
    let id = activity_id("tool.x", &serde_json::json!({"a": 1}));
    {
        let mut journal = Journal::open_capped(&path, cap)?;
        journal.record(turn_started)?;
        journal.record(JournalStep::ActivityStarted {
            id: id.0.clone(),
            name: "tool.x".into(),
            input: serde_json::json!({"a": 1}),
        })?;
        journal.record(JournalStep::ActivityCompleted {
            id: id.0.clone(),
            output: serde_json::json!({"stable": "output"}),
            retried: false,
        })?;
        journal.record(JournalStep::TurnFinished {
            outcome: "ok".into(),
        })?;
    }
    // Rotation happened at a record boundary: every line in BOTH
    // generations parses (no torn mid-record writes).
    for file in [&path, &generation(&path)] {
        for line in std::fs::read_to_string(file)?.lines() {
            serde_json::from_str::<JournalStep>(line)?;
        }
    }
    // Replay reads the CURRENT file only: the pre-rotation TurnStarted
    // is forensic history in `.1`, the latest state is intact.
    let reopened = Journal::open(&path)?;
    assert!(
        !reopened
            .steps()
            .iter()
            .any(|s| matches!(s, JournalStep::TurnStarted { .. })),
        "rotated-away TurnStarted must not replay"
    );
    assert_eq!(reopened.steps().len(), 3);
    assert_eq!(reopened.open_turn_prompt(), None);
    assert_eq!(
        reopened.completed_outputs().get(&id.0),
        Some(&serde_json::json!({"stable": "output"})),
        "latest completed output must survive rotation"
    );
    Ok(())
}

#[test]
fn zero_cap_disables_rotation() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = turn_journal_path(dir.path(), "s1", 1);
    let mut journal = Journal::open_capped(&path, 0)?;
    for n in 0..10 {
        journal.record(started_step(n))?;
    }
    assert!(!generation(&path).exists(), "0 must mean unlimited");
    assert_eq!(Journal::open(&path)?.steps().len(), 10);
    Ok(())
}

#[test]
fn single_generation_retained_oldest_history_dropped() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = turn_journal_path(dir.path(), "s1", 1);
    let mut journal = Journal::open_capped(&path, TINY_CAP)?;
    journal.record(started_step(1))?; // generation A: current
    journal.record(started_step(2))?; // rotates: .1 = A, current = B
    journal.record(started_step(3))?; // rotates again: .1 = B, current = C
    drop(journal);

    let forensic = std::fs::read_to_string(generation(&path))?;
    let current = std::fs::read_to_string(&path)?;
    let has = |text: &str, n: u64| text.contains(&activity_id("t", &serde_json::json!(n)).0);
    assert!(
        has(&forensic, 2),
        "second generation must be retained: {forensic}"
    );
    assert!(
        !has(&forensic, 1),
        "oldest generation must be dropped: {forensic}"
    );
    assert!(
        has(&current, 3),
        "newest record must stay in current: {current}"
    );
    Ok(())
}

#[test]
fn under_cap_appends_stay_byte_identical() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = turn_journal_path(dir.path(), "s1", 1);
    let steps = vec![
        JournalStep::TurnStarted {
            turn: 1,
            prompt: "p".into(),
        },
        started_step(1),
        JournalStep::TurnFinished {
            outcome: "ok".into(),
        },
    ];
    let mut journal = Journal::open_capped(&path, u64::MAX)?;
    for step in &steps {
        journal.record(step.clone())?;
    }
    drop(journal);
    let mut expected = Vec::new();
    for step in &steps {
        expected.extend_from_slice(&serde_json::to_vec(step)?);
        expected.push(b'\n');
    }
    assert_eq!(
        std::fs::read(&path)?,
        expected,
        "under-cap append path changed"
    );
    assert!(!generation(&path).exists());
    Ok(())
}

// --- runs.jsonl (RunHistory / RunSink) ------------------------------

#[test]
fn runs_history_rotates_at_cap_and_latest_rows_survive() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let history = RunHistory::open_capped(dir.path(), TINY_CAP)?;
    history.append(&record("s-old", "started"))?;
    history.append(&record("s-mid", "started"))?;
    history.append(&record("s-new", "confirmed"))?;

    let path = dir.path().join("runs.jsonl");
    let rotated = generation(&path);
    assert!(rotated.exists(), "runs.jsonl never rotated");
    // Record-boundary integrity in both generations.
    for file in [&path, &rotated] {
        for line in std::fs::read_to_string(file)?.lines() {
            serde_json::from_str::<RunRecord>(line)?;
        }
    }
    // list() reads the current file only; the LATEST row survives.
    let rows = history.list()?;
    assert_eq!(rows.len(), 1, "current must hold only post-rotation rows");
    assert_eq!(rows[0].session_id, "s-new");
    assert_eq!(
        history.latest_for_session("s-new")?.map(|r| r.outcome),
        Some("confirmed".into()),
        "latest state must never be dropped"
    );
    Ok(())
}

#[test]
fn run_sink_capped_rotation_wires_through() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let mut notifier = Notifier::new();
    notifier.add(Arc::new(MemorySink::new()));
    let sink = RunSink::open_capped(
        dir.path(),
        Arc::new(CriteriaGate) as Arc<dyn CompletionGate>,
        Arc::new(notifier),
        TINY_CAP,
    )?;
    let def = def("nightly", "* * * * *");
    sink.run_started(&def, 60, "auto-nightly-60", NetworkPolicy::Online);
    sink.run_started(&def, 120, "auto-nightly-120", NetworkPolicy::Online);
    let path = dir.path().join("runs.jsonl");
    assert!(
        generation(&path).exists(),
        "RunSink cap never rotated runs.jsonl"
    );
    let rows = sink.history().list()?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_id, "auto-nightly-120");
    Ok(())
}

// --- scheduler-journal.jsonl (CronRunner) ---------------------------

fn def(id: &str, cron: &str) -> AutomationDefinition {
    AutomationDefinition {
        id: id.into(),
        name: format!("{id} label"),
        cron: cron.into(),
        pack: "review".into(),
        goal: Some("nightly review".into()),
        criteria: vec![],
        budget_turns: Some(4),
        max_invocations_total: None,
        max_invocations_per_day: None,
        network: NetworkPolicy::Online,
        enabled: true,
    }
}

#[derive(Default)]
struct FakeKicker(Mutex<Vec<KickOff>>);

impl RunKicker for FakeKicker {
    fn kick_off(&self, run: &KickOff) -> Result<(), KickError> {
        match self.0.lock() {
            Ok(mut guard) => {
                guard.push(run.clone());
                Ok(())
            }
            Err(poisoned) => {
                poisoned.into_inner().push(run.clone());
                Ok(())
            }
        }
    }
}

fn kicks_of(kicker: &FakeKicker) -> Vec<KickOff> {
    kicker.0.lock().map(|g| g.clone()).unwrap_or_default()
}

fn sink_in(dir: &tempfile::TempDir) -> Arc<RunSink> {
    let mut notifier = Notifier::new();
    notifier.add(Arc::new(MemorySink::new()));
    Arc::new(
        RunSink::open(
            dir.path(),
            Arc::new(CriteriaGate) as Arc<dyn CompletionGate>,
            Arc::new(notifier),
        )
        .unwrap_or_else(|e| panic!("sink: {e}")),
    )
}

#[test]
fn scheduler_journal_rotates_under_cap_and_runner_keeps_firing()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let kicker = Arc::new(FakeKicker::default());
    let open = |kicker: Arc<FakeKicker>| {
        CronRunner::open_capped(
            dir.path(),
            vec![def("nightly", "* * * * *")],
            kicker,
            sink_in(&dir),
            0,
            NetworkPolicy::Online,
            TINY_CAP,
        )
    };
    let mut runner = open(Arc::clone(&kicker))?;
    runner.pump(0)?;
    assert_eq!(runner.pump(60)?.kicked.len(), 1);

    // The fire journal's first record exceeded the tiny cap, so the
    // second durable activity rotated the file mid-fire.
    let journal_path = dir.path().join("scheduler-journal.jsonl");
    assert!(
        generation(&journal_path).exists(),
        "scheduler journal never rotated"
    );
    // Record-boundary integrity across generations.
    for file in [&journal_path, &generation(&journal_path)] {
        for line in std::fs::read_to_string(file)?.lines() {
            serde_json::from_str::<JournalStep>(line)?;
        }
    }
    // Appends still succeed after rotation: the next fire goes through.
    assert_eq!(runner.pump(120)?.kicked.len(), 1);
    assert_eq!(kicks_of(&kicker).len(), 2);

    // Restart over the same root: no double fire for a covered minute
    // (latest state survives rotation; the ledger cursor plus the
    // current-file journal both hold it).
    let mut restarted = open(Arc::clone(&kicker))?;
    restarted.pump(120)?;
    assert_eq!(kicks_of(&kicker).len(), 2, "restart double-fired");
    Ok(())
}
