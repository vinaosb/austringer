//! Task-41 e2e: scheduled automations against a REAL `DaemonState`
//! with real engine sessions — injected clock, scripted inference.
//! Assertions are on journaled effects (history rows, journal steps,
//! delivered notifications), never on loop ticks.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use harness_daemon::automations::{
    AutomationDefinition, AutomationStore, FixedPackGate, NetworkPolicy, PackTier,
};
use harness_daemon::bridge::RunSink;
use harness_daemon::config::DaemonConfig;
use harness_daemon::gate::{CompletionGate, CriteriaGate};
use harness_daemon::kickoff::{DaemonKicker, KickError, KickOff, RunKicker};
use harness_daemon::notify::{MemorySink, NotificationKind, Notifier};
use harness_daemon::runner::{CronRunner, RunnerError};
use harness_daemon::scheduler::SchedulerClock;
use harness_daemon::server::DaemonState;

use common::CompleteOnlyInference;

fn automation(
    id: &str,
    cron: &str,
    budget: Option<u32>,
    criteria: &[&str],
) -> AutomationDefinition {
    AutomationDefinition {
        id: id.into(),
        name: format!("{id} pack run"),
        cron: cron.into(),
        pack: "review".into(),
        goal: Some("run the nightly review pack".into()),
        criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
        budget_turns: budget,
        max_invocations_total: None,
        max_invocations_per_day: None,
        network: NetworkPolicy::Online,
        enabled: true,
    }
}

fn state_in(dir: &std::path::Path) -> Arc<DaemonState> {
    let config = DaemonConfig {
        data_dir: dir.to_owned(),
        token: Some("e2e-token".into()),
        durable: false,
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn harness_core::agent_loop::Inference> + Send + Sync> =
        Arc::new(|| Box::new(CompleteOnlyInference));
    DaemonState::new(config, factory)
        .map(Arc::new)
        .unwrap_or_else(|e| panic!("state: {e}"))
}

fn sink_in(dir: &std::path::Path, memory: Arc<MemorySink>) -> Arc<RunSink> {
    let mut notifier = Notifier::new();
    notifier.add(memory);
    Arc::new(
        RunSink::open(
            dir,
            Arc::new(CriteriaGate) as Arc<dyn CompletionGate>,
            Arc::new(notifier),
        )
        .unwrap_or_else(|e| panic!("sink: {e}")),
    )
}

fn kinds_of(memory: &MemorySink, kind: NotificationKind) -> Vec<String> {
    memory
        .delivered()
        .into_iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.message)
        .collect()
}

#[tokio::test]
async fn scheduled_automation_fires_kicks_session_and_notifies()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let gate = FixedPackGate::new(vec![("review".to_owned(), PackTier::Approved)]);
    let mut store = AutomationStore::load(dir.path(), &gate)?;
    store.register(
        &automation("nightly", "* * * * *", None, &["task_completed"]),
        &gate,
    )?;
    assert_eq!(store.defs().len(), 1);

    let state = state_in(dir.path());
    let memory = Arc::new(MemorySink::new());
    let sink = sink_in(dir.path(), Arc::clone(&memory));
    let mut runner = CronRunner::open(
        dir.path(),
        store.defs().to_vec(),
        Arc::new(DaemonKicker::new(Arc::clone(&state))),
        Arc::clone(&sink),
        0,
        NetworkPolicy::Online,
    )?;

    let report = runner.pump(0)?;
    assert!(report.kicked.is_empty(), "first pump baselines");
    let report = runner.pump(120)?;
    assert_eq!(report.kicked.len(), 1);
    assert_eq!(report.kicked[0].automation_id, "nightly");
    assert_eq!(report.kicked[0].budget_turns, None);
    assert_eq!(report.kicked[0].network, NetworkPolicy::Online);
    assert!(report.kicked[0].prompt.contains("nightly review pack"));

    assert_eq!(state.session_ids(), vec!["auto-nightly-2".to_owned()]);

    let journal = std::fs::read_to_string(dir.path().join("scheduler-journal.jsonl"))?;
    assert!(
        journal.contains("scheduled.fire"),
        "no durable fire: {journal}"
    );

    assert!(!kinds_of(&memory, NotificationKind::ScheduledFired).is_empty());

    let mut completion = None;
    for _ in 0..100 {
        let rows = sink.history().for_automation("nightly")?;
        if let Some(last) = rows.last().filter(|r| r.verdict.is_some()) {
            completion = Some(last.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let completion = completion.unwrap_or_else(|| panic!("run never completed"));
    assert_eq!(completion.outcome, "task_completed");
    assert_eq!(completion.verdict.as_deref(), Some("confirmed"));
    let turn_msgs = kinds_of(&memory, NotificationKind::TurnComplete);
    assert_eq!(turn_msgs.len(), 1);
    assert!(turn_msgs[0].contains("confirmed"), "{}", turn_msgs[0]);
    Ok(())
}

#[tokio::test]
async fn per_automation_budget_bounds_the_unattended_loop() -> Result<(), Box<dyn std::error::Error>>
{
    // budget_turns = 1 with a two-step script: the loop must stop after
    // one inference turn with a MaxTurns failure, not run to completion.
    let dir = tempfile::tempdir()?;
    let gate = FixedPackGate::new(vec![("review".to_owned(), PackTier::Approved)]);
    let mut store = AutomationStore::load(dir.path(), &gate)?;
    store.register(&automation("capped", "* * * * *", Some(1), &[]), &gate)?;

    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("e2e-token".into()),
        durable: false,
        ask_timeout: Duration::from_millis(200),
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn harness_core::agent_loop::Inference> + Send + Sync> =
        Arc::new(|| Box::new(common::TwoStepInference::new()));
    let state =
        Arc::new(DaemonState::new(config, factory).unwrap_or_else(|e| panic!("state: {e}")));

    let memory = Arc::new(MemorySink::new());
    let sink = sink_in(dir.path(), Arc::clone(&memory));
    let mut runner = CronRunner::open(
        dir.path(),
        store.defs().to_vec(),
        Arc::new(DaemonKicker::new(Arc::clone(&state))),
        Arc::clone(&sink),
        0,
        NetworkPolicy::Online,
    )?;
    runner.pump(0)?;
    assert_eq!(runner.pump(60)?.kicked.len(), 1);

    let mut outcome = None;
    for _ in 0..100 {
        let rows = sink.history().for_automation("capped")?;
        if let Some(last) = rows.last().filter(|r| r.verdict.is_some()) {
            outcome = Some(last.outcome.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let outcome = outcome.unwrap_or_else(|| panic!("capped run never finished"));
    assert!(
        outcome.contains("MaxTurns"),
        "budget not enforced, outcome: {outcome}"
    );
    Ok(())
}

#[tokio::test]
async fn restart_between_fire_and_completion_fails_closed() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let gate = FixedPackGate::new(vec![("review".to_owned(), PackTier::Approved)]);
    let mut store = AutomationStore::load(dir.path(), &gate)?;
    store.register(
        &automation("nightly", "* * * * *", None, &["task_completed"]),
        &gate,
    )?;
    let state = state_in(dir.path());
    let memory = Arc::new(MemorySink::new());
    {
        let sink = sink_in(dir.path(), Arc::clone(&memory));
        let mut runner = CronRunner::open(
            dir.path(),
            store.defs().to_vec(),
            Arc::new(DaemonKicker::new(Arc::clone(&state))),
            Arc::clone(&sink),
            0,
            NetworkPolicy::Online,
        )?;
        runner.pump(0)?;
        runner.pump(60)?;
    }
    let memory2 = Arc::new(MemorySink::new());
    let restarted_sink = sink_in(dir.path(), Arc::clone(&memory2));
    restarted_sink.on_turn_finished("auto-nightly-1", 1, "task_completed", Vec::new());
    let rows = restarted_sink.history().for_automation("nightly")?;
    let last = rows.last().cloned().unwrap_or_else(|| panic!("no rows"));
    assert_eq!(
        last.verdict.as_deref(),
        Some("inconclusive"),
        "restart must not auto-confirm"
    );
    Ok(())
}

#[tokio::test]
async fn run_kicker_errors_surface_as_retryable_activities()
-> Result<(), Box<dyn std::error::Error>> {
    struct Refusing;
    impl RunKicker for Refusing {
        fn kick_off(&self, _: &KickOff) -> Result<(), KickError> {
            Err(KickError::QueueFull)
        }
    }
    let dir = tempfile::tempdir()?;
    let gate = FixedPackGate::new(vec![("review".to_owned(), PackTier::Approved)]);
    let mut store = AutomationStore::load(dir.path(), &gate)?;
    store.register(&automation("nightly", "* * * * *", None, &[]), &gate)?;
    let sink = sink_in(dir.path(), Arc::new(MemorySink::new()));
    let mut runner = CronRunner::open(
        dir.path(),
        store.defs().to_vec(),
        Arc::new(Refusing),
        sink,
        0,
        NetworkPolicy::Online,
    )?;
    runner.pump(0)?;
    let fired = runner.pump(60);
    assert!(
        matches!(fired, Err(RunnerError::Fire { .. })),
        "kick failure must surface typed, got {fired:?}"
    );
    Ok(())
}

#[test]
fn shared_fake_clock_drives_pumps() -> Result<(), Box<dyn std::error::Error>> {
    // The clock seam is shared state in real deployments; injected
    // clocks must remain pure functions of the test's timeline.
    let now = Arc::new(AtomicU64::new(0));
    let read = Arc::clone(&now);
    let clock: SchedulerClock = Arc::new(move || read.load(Ordering::SeqCst));
    assert_eq!(clock(), 0);
    now.store(120, Ordering::SeqCst);
    assert_eq!(clock(), 120);
    Ok(())
}
