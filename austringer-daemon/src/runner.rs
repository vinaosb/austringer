//! The scheduler loop composition: cron automations over an injected
//! clock, missed-fire coalescing with deterministic jitter, invocation
//! caps, durable fire activities (replay never re-kicks), and session
//! kickoff through the [`RunKicker`] seam.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crate::activity::{ActivityExecutor, JournalingExecutor};
use crate::automations::{AutomationDefinition, NetworkPolicy};
use crate::bridge::RunSink;
use crate::journal::Journal;
use crate::kickoff::{FIRE_ACTIVITY, KickOff, KickoffExecutor, RunKicker, SKIP_ACTIVITY};
use crate::ledger::InvocationLedger;
use crate::scheduler::{CronExpr, MAX_CATCHUP_MINUTES, SchedulerClock};

/// Default jitter window for coalesced catch-up fires (seconds).
pub const DEFAULT_JITTER_WINDOW_SECS: u64 = 30;

/// Shipped scheduler loop period (60 s). Cron resolution is
/// minute-granular and `pump` coalesces missed minutes (cursor
/// high-water marks + [`MAX_CATCHUP_MINUTES`]), so this period costs
/// nothing in fire accuracy: a due automation fires on the first tick
/// at or after `effective`, i.e. with up to one tick of latency — the
/// jitter window already treats sub-minute spread as non-essential.
/// A 5 s period woke the daemon 12x/min (12x idle wakeups, plus a
/// whole-list defs deep-clone per tick) for zero resolution gain;
/// perf-as-culture receipt, research §23.6.
pub const SCHEDULER_TICK_PERIOD: std::time::Duration = std::time::Duration::from_secs(60);

/// Default rotation cap for the scheduler journal (8 MiB); matches
/// [`crate::config::DaemonConfig::scheduler_journal_max_bytes`].
pub const DEFAULT_SCHEDULER_JOURNAL_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Deterministic jitter in `[0, window)`: blake3 of job + minute, so a
/// coalesced catch-up burst spreads across the window reproducibly.
#[must_use]
pub fn jitter_secs(job_id: &str, minute: u64, window: u64) -> u64 {
    if window == 0 {
        return 0;
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(job_id.as_bytes());
    hasher.update(&minute.to_le_bytes());
    let digest = hasher.finalize();
    let bytes: [u8; 8] = digest.as_bytes()[..8].try_into().unwrap_or([0; 8]);
    u64::from_le_bytes(bytes) % window
}

/// Loop failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    /// Journal/store I/O failure.
    #[error("scheduler io: {0}")]
    Io(String),
    /// Ledger persistence failure.
    #[error("scheduler ledger: {0}")]
    Ledger(String),
    /// A definition failed cron re-parse.
    #[error("automation {def}: bad cron: {source}")]
    BadCron {
        /// Offending definition.
        def: String,
        /// Parse failure.
        source: crate::scheduler::CronError,
    },
    /// The fire activity failed (kick refused).
    #[error("fire activity: {message}")]
    Fire {
        /// Failure cause.
        message: String,
    },
}

/// One pump's observable effects.
#[derive(Debug, Default, PartialEq)]
pub struct PumpReport {
    /// Kickoffs this pump performed (or replayed).
    pub kicked: Vec<KickOff>,
    /// Fires suppressed by caps: `(job, reason)`.
    pub skipped: Vec<(String, &'static str)>,
}

/// The scheduler loop: deterministic under the injected clock, durable
/// across restarts (ledger cursors + fire journal).
pub struct CronRunner {
    defs: Vec<AutomationDefinition>,
    crons: BTreeMap<String, CronExpr>,
    cursors: BTreeMap<String, u64>,
    exec: JournalingExecutor,
    ledger: InvocationLedger,
    kicker: Arc<dyn RunKicker>,
    sink: Arc<RunSink>,
    jitter_window: u64,
    network_baseline: NetworkPolicy,
    root: std::path::PathBuf,
}

impl CronRunner {
    /// Open the runner over `root` with the default scheduler-journal
    /// rotation cap ([`DEFAULT_SCHEDULER_JOURNAL_MAX_BYTES`]): loads
    /// the invocation ledger (restart cursors) and the durable fire
    /// journal.
    ///
    /// # Errors
    /// see [`RunnerError`].
    pub fn open(
        root: &Path,
        defs: Vec<AutomationDefinition>,
        kicker: Arc<dyn RunKicker>,
        sink: Arc<RunSink>,
        jitter_window: u64,
        network_baseline: NetworkPolicy,
    ) -> Result<Self, RunnerError> {
        Self::open_capped(
            root,
            defs,
            kicker,
            sink,
            jitter_window,
            network_baseline,
            DEFAULT_SCHEDULER_JOURNAL_MAX_BYTES,
        )
    }

    /// Open with an explicit scheduler-journal rotation cap; `0`
    /// disables rotation (unlimited growth).
    ///
    /// # Errors
    /// see [`RunnerError`].
    pub fn open_capped(
        root: &Path,
        defs: Vec<AutomationDefinition>,
        kicker: Arc<dyn RunKicker>,
        sink: Arc<RunSink>,
        jitter_window: u64,
        network_baseline: NetworkPolicy,
        scheduler_journal_max_bytes: u64,
    ) -> Result<Self, RunnerError> {
        let mut crons = BTreeMap::new();
        for def in &defs {
            let cron = CronExpr::parse(&def.cron).map_err(|source| RunnerError::BadCron {
                def: def.id.clone(),
                source,
            })?;
            crons.insert(def.id.clone(), cron);
        }
        let ledger = InvocationLedger::load(root);
        let cursors = defs
            .iter()
            .filter_map(|def| {
                ledger
                    .cursor(&def.id)
                    .map(|minute| (def.id.clone(), minute))
            })
            .collect();
        let journal = Journal::open_capped(
            &root.join("scheduler-journal.jsonl"),
            scheduler_journal_max_bytes,
        )
        .map_err(|e| RunnerError::Io(e.to_string()))?;
        Ok(Self {
            defs,
            crons,
            cursors,
            exec: JournalingExecutor::new(
                Arc::new(KickoffExecutor::new(Arc::clone(&kicker))),
                journal,
            ),
            ledger,
            kicker,
            sink,
            jitter_window,
            network_baseline,
            root: root.to_owned(),
        })
    }

    /// Advance to `now_epoch_secs` (injected clock): evaluate each
    /// automation's window, coalesce missed fires into the latest
    /// minute, apply jitter/caps, fire durably, kick off.
    ///
    /// # Errors
    /// see [`RunnerError`].
    pub fn pump(&mut self, now_epoch_secs: u64) -> Result<PumpReport, RunnerError> {
        let now_minute = now_epoch_secs / 60;
        let mut report = PumpReport::default();
        // Borrow, never clone: this runs every tick for the daemon's
        // lifetime, and a wholesale defs deep-clone per tick is pure
        // idle waste (the fire/skip branches take no &mut self, so
        // the borrow survives the whole loop).
        for def in &self.defs {
            if !def.enabled {
                continue;
            }
            let Some(cursor) = self.cursors.get(&def.id).copied() else {
                // First sight: establish the baseline, never catch up.
                self.cursors.insert(def.id.clone(), now_minute);
                continue;
            };
            if now_minute <= cursor {
                continue; // backward jump: high-water mark holds
            }
            let end = now_minute.min(cursor + MAX_CATCHUP_MINUTES);
            let cron = match self.crons.get(&def.id) {
                Some(cron) => cron,
                None => continue,
            };
            let mut due = None;
            let mut minute = cursor + 1;
            while minute <= end {
                if cron.matches(minute) {
                    due = Some(minute); // last match wins: coalescing
                }
                minute += 1;
            }
            let Some(fire_minute) = due else {
                self.cursors.insert(def.id.clone(), end);
                continue;
            };
            let effective =
                fire_minute * 60 + jitter_secs(&def.id, fire_minute, self.jitter_window);
            if now_epoch_secs < effective {
                // Not yet due under jitter: keep the minute pending.
                self.cursors.insert(def.id.clone(), fire_minute - 1);
                continue;
            }
            if let Some(reason) = self.cap_reason(def, fire_minute) {
                self.durable_skip(&def.id, fire_minute, reason)?;
                self.cursors.insert(def.id.clone(), fire_minute);
                report.skipped.push((def.id.clone(), reason));
                continue;
            }
            let kick = self.fire(def, fire_minute)?;
            self.ledger.record(&def.id, fire_minute);
            self.ledger.save(&self.root).map_err(RunnerError::Ledger)?;
            self.sink
                .run_started(def, fire_minute, &kick.session_id, kick.network);
            if let Some(host) = self.kicker.host_for(&kick.session_id) {
                self.sink.spawn_watcher(&host, &kick.session_id);
            }
            self.cursors.insert(def.id.clone(), fire_minute);
            report.kicked.push(kick);
        }
        Ok(report)
    }

    fn cap_reason(&self, def: &AutomationDefinition, minute: u64) -> Option<&'static str> {
        if let Some(max) = def.max_invocations_total
            && self.ledger.count_total(&def.id) >= max
        {
            return Some("cap_total");
        }
        if let Some(max) = def.max_invocations_per_day
            && self.ledger.count_day(&def.id, minute / 1440) >= max
        {
            return Some("cap_daily");
        }
        None
    }

    fn fire(&self, def: &AutomationDefinition, minute: u64) -> Result<KickOff, RunnerError> {
        let kick = KickOff {
            automation_id: def.id.clone(),
            session_id: format!("auto-{}-{minute}", def.id),
            pack: def.pack.clone(),
            prompt: def
                .goal
                .clone()
                .unwrap_or_else(|| format!("scheduled run of pack {} ({})", def.pack, def.name)),
            budget_turns: def.budget_turns,
            network: def.network.intersect(self.network_baseline),
        };
        let input = serde_json::to_value(crate::kickoff::FireInput {
            kick: kick.clone(),
            minute,
        })
        .map_err(|e| RunnerError::Io(e.to_string()))?;
        let output = self
            .exec
            .execute(FIRE_ACTIVITY, &input)
            .map_err(|e| RunnerError::Fire { message: e.message })?;
        let session_id = output
            .get("session_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if session_id != kick.session_id {
            return Err(RunnerError::Fire {
                message: format!("journal divergence: {session_id} != {}", kick.session_id),
            });
        }
        Ok(kick)
    }

    fn durable_skip(
        &self,
        job: &str,
        minute: u64,
        reason: &'static str,
    ) -> Result<(), RunnerError> {
        let input = serde_json::json!({ "job": job, "minute": minute, "reason": reason });
        self.exec
            .execute(SKIP_ACTIVITY, &input)
            .map_err(|e| RunnerError::Io(e.message))?;
        Ok(())
    }

    /// Background loop for the binary: pump on a fixed interval.
    /// Panics never: pump errors are logged and the loop continues.
    pub fn spawn_loop(mut self, clock: SchedulerClock, period: std::time::Duration) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(period);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let now = clock();
                if let Err(e) = self.pump(now) {
                    tracing::warn!("scheduler pump failed: {e}");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::{AutomationDefinition, NetworkPolicy};
    use crate::bridge::RunSink;
    use crate::gate::{CompletionGate, CriteriaGate};
    use crate::kickoff::KickError;
    use crate::notify::{MemorySink, Notifier};
    use std::sync::{Arc, Mutex};

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

    fn runner_in(
        dir: &tempfile::TempDir,
        defs: Vec<AutomationDefinition>,
        kicker: Arc<FakeKicker>,
        jitter: u64,
    ) -> Result<CronRunner, RunnerError> {
        CronRunner::open(
            dir.path(),
            defs,
            kicker,
            sink_in(dir),
            jitter,
            NetworkPolicy::Online,
        )
    }

    #[test]
    fn jitter_is_deterministic_and_bounded() {
        assert_eq!(jitter_secs("job", 42, 30), jitter_secs("job", 42, 30));
        assert_ne!(jitter_secs("job", 42, 30), jitter_secs("job2", 42, 30));
        for minute in 0..500 {
            assert!(jitter_secs("job", minute, 30) < 30);
        }
        assert_eq!(jitter_secs("job", 42, 0), 0);
    }

    #[test]
    fn first_pump_baselines_without_firing() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        let mut runner = runner_in(
            &dir,
            vec![def("nightly", "* * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        let report = runner.pump(600)?;
        assert!(report.kicked.is_empty());
        assert!(kicks_of(&kicker).is_empty());
        Ok(())
    }

    #[test]
    fn missed_minutes_coalesce_into_one_kick() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        let mut runner = runner_in(
            &dir,
            vec![def("nightly", "* * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        runner.pump(0)?;
        let report = runner.pump(600)?;
        assert_eq!(
            report.kicked.len(),
            1,
            "missed 1..10 must coalesce to one run"
        );
        let kicks = kicks_of(&kicker);
        assert_eq!(kicks.len(), 1);
        assert!(kicks[0].session_id.ends_with("-10"), "coalesced to latest");
        assert_eq!(kicks[0].automation_id, "nightly");
        assert_eq!(kicks[0].budget_turns, Some(4));
        assert_eq!(kicks[0].network, NetworkPolicy::Online);
        Ok(())
    }

    #[test]
    fn jitter_defers_current_minute_fire_until_effective() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        let mut runner = runner_in(
            &dir,
            vec![def("nightly", "* * * * *")],
            Arc::clone(&kicker),
            30,
        )?;
        runner.pump(0)?;
        let deferred_minute = (1..=40)
            .find(|m| jitter_secs("nightly", *m, 30) > 0)
            .unwrap_or(0);
        assert_ne!(deferred_minute, 0, "no jittered minute in range");
        let boundary = deferred_minute * 60;
        let report = runner.pump(boundary)?;
        assert!(report.kicked.is_empty(), "fired before jitter elapsed");
        let report = runner.pump(boundary + 30)?;
        assert_eq!(report.kicked.len(), 1);
        Ok(())
    }

    #[test]
    fn lifetime_invocation_cap_stops_firing() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut capped = def("nightly", "* * * * *");
        capped.max_invocations_total = Some(1);
        let kicker = Arc::new(FakeKicker::default());
        let mut runner = runner_in(&dir, vec![capped], Arc::clone(&kicker), 0)?;
        runner.pump(0)?;
        assert_eq!(runner.pump(60)?.kicked.len(), 1);
        let report = runner.pump(120)?;
        assert!(report.kicked.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].0, "nightly");
        assert_eq!(report.skipped[0].1, "cap_total");
        assert_eq!(kicks_of(&kicker).len(), 1);
        Ok(())
    }

    #[test]
    fn daily_invocation_cap_bounds_same_day() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut capped = def("nightly", "* * * * *");
        capped.max_invocations_per_day = Some(1);
        let kicker = Arc::new(FakeKicker::default());
        let mut runner = runner_in(&dir, vec![capped], Arc::clone(&kicker), 0)?;
        runner.pump(0)?;
        assert_eq!(runner.pump(60)?.kicked.len(), 1);
        let report = runner.pump(120)?;
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].1, "cap_daily");
        Ok(())
    }

    #[test]
    fn restart_resumes_cursor_without_double_fire() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        {
            let mut first = runner_in(
                &dir,
                vec![def("nightly", "* * * * *")],
                Arc::clone(&kicker),
                0,
            )?;
            first.pump(0)?;
            assert_eq!(first.pump(60)?.kicked.len(), 1);
        }
        let mut second = runner_in(
            &dir,
            vec![def("nightly", "* * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        assert!(second.pump(90)?.kicked.is_empty(), "same minute re-kicked");
        assert_eq!(second.pump(120)?.kicked.len(), 1);
        assert_eq!(kicks_of(&kicker).len(), 2);
        Ok(())
    }

    #[test]
    fn journal_replay_prevents_reexecution_after_restart() -> Result<(), Box<dyn std::error::Error>>
    {
        // The fire activity is journaled; a restarted runner over the
        // same root replays it without re-kicking (cursor + journal).
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        let mut first = runner_in(
            &dir,
            vec![def("nightly", "* * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        first.pump(0)?;
        first.pump(60)?;
        let steps = std::fs::read_to_string(dir.path().join("scheduler-journal.jsonl"))?;
        assert!(steps.contains("scheduled.fire"), "no durable fire: {steps}");
        let mut second = runner_in(
            &dir,
            vec![def("nightly", "* * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        second.pump(90)?;
        assert_eq!(kicks_of(&kicker).len(), 1);
        Ok(())
    }

    #[test]
    fn backward_clock_jump_does_not_double_fire() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        let mut runner = runner_in(
            &dir,
            vec![def("hourly", "0 * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        runner.pump(0)?;
        assert_eq!(runner.pump(10 * 3600)?.kicked.len(), 1);
        assert!(runner.pump(9 * 3600 + 1800)?.kicked.is_empty());
        assert!(runner.pump(10 * 3600 + 60)?.kicked.is_empty());
        assert_eq!(kicks_of(&kicker).len(), 1);
        Ok(())
    }

    #[test]
    fn catchup_window_stays_bounded_after_long_gap() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        let mut runner = runner_in(
            &dir,
            vec![def("minutely", "* * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        runner.pump(0)?;
        let report = runner.pump(30 * 86400)?;
        assert_eq!(
            report.kicked.len(),
            1,
            "gap beyond bound must coalesce to one"
        );
        Ok(())
    }

    #[test]
    fn offline_automation_policy_intersects_to_offline() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut offline_def = def("offline", "* * * * *");
        offline_def.network = NetworkPolicy::Offline;
        let kicker = Arc::new(FakeKicker::default());
        let kicker_dyn: Arc<dyn RunKicker> = kicker.clone();
        let mut runner = CronRunner::open(
            dir.path(),
            vec![offline_def],
            kicker_dyn,
            sink_in(&dir),
            0,
            NetworkPolicy::Offline,
        )?;
        runner.pump(0)?;
        runner.pump(60)?;
        let kicks = kicks_of(&kicker);
        assert_eq!(kicks.len(), 1);
        assert_eq!(kicks[0].network, NetworkPolicy::Offline);
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn sixty_second_tick_period_fires_due_automations()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::atomic::{AtomicU64, Ordering};
        // Pins the shipped loop period AND proves it is semantically
        // sufficient: cron resolution is minute-granular and pump()
        // coalesces (cursor high-water marks + MAX_CATCHUP_MINUTES),
        // so a 60 s tick costs at most one tick of fire latency.
        assert_eq!(
            SCHEDULER_TICK_PERIOD,
            std::time::Duration::from_secs(60),
            "shipped scheduler period changed: revisit the latency receipt"
        );
        let dir = tempfile::tempdir()?;
        let kicker = Arc::new(FakeKicker::default());
        let now = Arc::new(AtomicU64::new(0));
        let read = Arc::clone(&now);
        let clock: SchedulerClock = Arc::new(move || read.load(Ordering::SeqCst));
        let runner = runner_in(
            &dir,
            vec![def("nightly", "* * * * *")],
            Arc::clone(&kicker),
            0,
        )?;
        runner.spawn_loop(clock, SCHEDULER_TICK_PERIOD);

        // The interval's first tick is immediate: it baselines only.
        tokio::task::yield_now().await;
        assert!(kicks_of(&kicker).is_empty(), "baseline tick must not fire");

        // One full period of virtual time: the due minute fires on the
        // single tick that lands after it.
        now.store(60, Ordering::SeqCst);
        tokio::time::advance(SCHEDULER_TICK_PERIOD).await;
        tokio::task::yield_now().await;
        let kicks = kicks_of(&kicker);
        assert_eq!(kicks.len(), 1, "60 s period failed to fire a due minute");
        assert_eq!(kicks[0].automation_id, "nightly");
        Ok(())
    }
}
