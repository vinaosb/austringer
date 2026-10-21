//! The completion/notification bridge: watches a kicked-off session and
//! translates daemon events into the plan's notification channels —
//! `PermissionAsk` → approval-needed, `TurnFinished` → completion gate
//! review + verdict + turn-complete notification.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

use crate::attach::SessionHost;
use crate::automations::{AutomationDefinition, NetworkPolicy};
use crate::gate::{
    CompletionGate, EvidenceBundle, EvidenceLink, GateVerdict, TestRunEvidence,
};
use crate::notify::{Notification, NotificationKind, Notifier};
use crate::protocol::{Frame, ServerMsg, SessionEvent, VerdictKind};
use crate::run_history::{RunHistory, RunHistoryError, RunRecord};
use harness_core::event::Event;

/// In-flight run context the gate needs at completion.
#[derive(Debug, Clone)]
pub struct PendingRun {
    /// Owning automation.
    pub automation_id: String,
    /// Fire minute.
    pub minute: u64,
    /// Criteria accepted completion requires.
    pub criteria: Vec<String>,
    /// Goal text.
    pub goal: Option<String>,
    /// Effective network policy.
    pub network: NetworkPolicy,
}

/// Shared completion sink: history rows + gate reviews + notifications.
pub struct RunSink {
    history: RunHistory,
    gate: Arc<dyn CompletionGate>,
    notifier: Arc<Notifier>,
    pending: Mutex<HashMap<String, PendingRun>>,
}

impl RunSink {
    /// Sink over `{root}/runs.jsonl`.
    ///
    /// # Errors
    /// history open failure.
    pub fn open(
        root: &Path,
        gate: Arc<dyn CompletionGate>,
        notifier: Arc<Notifier>,
    ) -> Result<Self, RunHistoryError> {
        Ok(Self {
            history: RunHistory::open(root)?,
            gate,
            notifier,
            pending: Mutex::new(HashMap::new()),
        })
    }

    /// The run history (read side).
    #[must_use]
    pub fn history(&self) -> &RunHistory {
        &self.history
    }

    /// Runs awaiting completion.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Record a kickoff: history row + ScheduledFired notification.
    /// `network` is the EFFECTIVE (intersected) policy of the run.
    pub fn run_started(
        &self,
        def: &AutomationDefinition,
        minute: u64,
        session_id: &str,
        network: NetworkPolicy,
    ) {
        let pending = PendingRun {
            automation_id: def.id.clone(),
            minute,
            criteria: def.criteria.clone(),
            goal: def.goal.clone(),
            network,
        };
        if let Err(e) = self.history.append(&RunRecord {
            automation_id: def.id.clone(),
            session_id: session_id.to_owned(),
            fired_at_minute: minute,
            outcome: "started".into(),
            verdict: None,
            network: pending.network,
        }) {
            tracing::warn!("run history append failed: {e}");
        }
        if let Ok(mut pending_map) = self.pending.lock() {
            pending_map.insert(session_id.to_owned(), pending);
        }
        self.emit(
            NotificationKind::ScheduledFired,
            Some(session_id),
            format!("automation {} fired; run kicked off", def.id),
        );
    }

    /// Completion path: gate the run's evidence, record verdict +
    /// outcome, notify. A run whose criteria were lost across a
    /// restart fails closed to Inconclusive — never auto-confirmed.
    /// `test_runs` are the structured run_tests observations the
    /// watcher collected (todo 18); they bind `tests:` criteria and
    /// their event ids ride the evidence links. Returns the verdict
    /// so the caller can surface it on the wire.
    pub fn on_turn_finished(
        &self,
        session_id: &str,
        turn: u64,
        outcome: &str,
        test_runs: Vec<TestRunEvidence>,
    ) -> GateVerdict {
        let pending = match self.pending.lock() {
            Ok(mut map) => map.remove(session_id),
            Err(poisoned) => poisoned.into_inner().remove(session_id),
        };
        let prior = self.history.latest_for_session(session_id).ok().flatten();
        let (automation, minute, network) = match (&pending, &prior) {
            (Some(run), _) => (run.automation_id.clone(), run.minute, run.network),
            (None, Some(row)) => (row.automation_id.clone(), row.fired_at_minute, row.network),
            (None, None) => ("unknown".into(), 0, NetworkPolicy::Online),
        };
        let verdict = match pending {
            Some(run) => {
                let mut links = vec![EvidenceLink {
                    label: format!("session:{session_id} turn:{turn}"),
                    claims: vec![outcome.to_owned()],
                }];
                // One link per observed test run: the label carries the
                // test-run event id (verdict drilldown). A green run
                // claims its suite criterion; a red run claims nothing
                // (it contradicts the criterion, it does not cover it).
                for run in &test_runs {
                    links.push(EvidenceLink {
                        label: format!("test-run:{} suite:{}", run.event_id, run.suite),
                        claims: if run.report.summary.failed == 0 {
                            vec![format!("tests: {} green", run.suite)]
                        } else {
                            Vec::new()
                        },
                    });
                }
                let bundle = EvidenceBundle {
                    goal: run.goal.clone().unwrap_or_default(),
                    criteria: run.criteria.clone(),
                    links,
                    test_runs,
                };
                self.gate.review(&bundle)
            }
            None => GateVerdict::Inconclusive {
                reason: "run context lost across restart; criteria unavailable".into(),
                covered: vec![],
            },
        };
        if let Err(e) = self.history.append(&RunRecord {
            automation_id: automation,
            session_id: session_id.to_owned(),
            fired_at_minute: minute,
            outcome: outcome.to_owned(),
            verdict: Some(verdict.as_str().to_owned()),
            network,
        }) {
            tracing::warn!("run history append failed: {e}");
        }
        // A refuted verdict cites its reason (failing test ids for a
        // red bound suite) in the human-facing notification.
        let gate_note = match &verdict {
            GateVerdict::Refuted { reason, .. } => format!("refuted: {reason}"),
            _ => verdict.as_str().to_owned(),
        };
        self.emit(
            NotificationKind::TurnComplete,
            Some(session_id),
            format!("run {session_id} finished: {outcome} (gate: {gate_note})"),
        );
        verdict
    }

    /// Watch a kicked-off session: PermissionAsk → ApprovalNeeded;
    /// TurnFinished → [`Self::on_turn_finished`], then stop.
    /// Subscriptions are taken synchronously so no event published
    /// after this call can be missed.
    pub fn spawn_watcher(self: &Arc<Self>, host: &Arc<SessionHost>, session_id: &str) {
        let asks = host.mediator().subscribe();
        let live = host.hub_subscribe();
        let sink = Arc::clone(self);
        let host = Arc::clone(host);
        let session = session_id.to_owned();
        let (done_tx, done_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            sink.watch_turn(host, live, session, done_tx).await;
        });
        let sink = Arc::clone(self);
        let session = session_id.to_owned();
        tokio::spawn(async move {
            sink.watch_asks(session, asks, done_rx).await;
        });
    }

    async fn watch_turn(
        self: Arc<Self>,
        host: Arc<SessionHost>,
        mut live: tokio::sync::broadcast::Receiver<ServerMsg>,
        session: String,
        done: oneshot::Sender<()>,
    ) {
        // The run's evidence: every session event id observed from
        // kickoff to TurnFinished (the ids a Verdict frame cites).
        let mut evidence: Vec<u64> = Vec::new();
        // The run's structured test evidence (todo 18): run_tests
        // call ids awaiting their ToolResult, then the parsed runs.
        let mut pending_run_tests: HashSet<String> = HashSet::new();
        let mut test_runs: Vec<TestRunEvidence> = Vec::new();
        loop {
            match live.recv().await {
                Ok(ServerMsg::Event { frame }) => {
                    evidence.push(frame.id);
                    if let Some(run) = collect_test_run(&frame, &mut pending_run_tests) {
                        test_runs.push(run);
                    }
                    if let SessionEvent::TurnFinished { turn, outcome } = frame.event {
                        let verdict =
                            self.on_turn_finished(&session, turn, &outcome, test_runs);
                        let kind = match &verdict {
                            GateVerdict::Confirmed { .. } => VerdictKind::Confirmed,
                            GateVerdict::Refuted { .. } => VerdictKind::Refuted,
                            GateVerdict::Inconclusive { .. } => VerdictKind::Inconclusive,
                        };
                        host.broadcast(ServerMsg::Verdict {
                            turn,
                            verdict: kind,
                            evidence,
                        });
                        let _ = done.send(());
                        return;
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    live = host.hub_subscribe();
                }
                Err(_) => return,
            }
        }
    }

    async fn watch_asks(
        self: Arc<Self>,
        session: String,
        mut asks: tokio::sync::broadcast::Receiver<ServerMsg>,
        mut done: oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                biased;
                _ = &mut done => return,
                ask = asks.recv() => match ask {
                    Ok(ServerMsg::PermissionAsk { ask }) => {
                        self.emit(
                            NotificationKind::ApprovalNeeded,
                            Some(&session),
                            format!(
                                "approval needed for tool {} (ask {})",
                                ask.tool, ask.ask_id
                            ),
                        );
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => return,
                },
            }
        }
    }

    fn emit(&self, kind: NotificationKind, session: Option<&str>, message: String) {
        let failures = self.notifier.emit(&Notification {
            kind,
            session_id: session.map(str::to_owned),
            message,
        });
        for failure in failures {
            tracing::warn!("notification sink failed: {failure}");
        }
    }
}

/// Correlate a recorded `run_tests` ToolCall/ToolResult pair into
/// structured test evidence: the call id is the correlation key, the
/// observation's first line IS the TestReport JSON (todo-17
/// contract), and the ToolResult's frame id is what the verdict
/// cites. Call ids are consumed on their result (one result per
/// call).
fn collect_test_run(frame: &Frame, pending: &mut HashSet<String>) -> Option<TestRunEvidence> {
    let SessionEvent::Recorded { event, .. } = &frame.event else {
        return None;
    };
    match event {
        Event::ToolCall { call_id, tool, .. } if tool == "run_tests" => {
            pending.insert(call_id.clone());
            None
        }
        Event::ToolResult { call_id, blocks } if pending.remove(call_id) => {
            TestRunEvidence::from_observation(frame.id, blocks)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::{AutomationDefinition, NetworkPolicy};
    use crate::gate::{CompletionGate, CriteriaGate, EvidenceBundle, GateVerdict};
    use crate::notify::{MemorySink, NotificationKind, Notifier};
    use std::sync::Arc;
    use std::time::Duration;

    fn def_with_criteria(criteria: &[&str]) -> AutomationDefinition {
        AutomationDefinition {
            id: "nightly".into(),
            name: "nightly".into(),
            cron: "0 3 * * *".into(),
            pack: "review".into(),
            goal: Some("nightly review".into()),
            criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
            budget_turns: None,
            max_invocations_total: None,
            max_invocations_per_day: None,
            network: NetworkPolicy::Online,
            enabled: true,
        }
    }

    fn sink_in(dir: &tempfile::TempDir, memory: Arc<MemorySink>) -> Arc<RunSink> {
        let mut notifier = Notifier::new();
        notifier.add(memory);
        Arc::new(
            RunSink::open(
                dir.path(),
                Arc::new(CriteriaGate) as Arc<dyn CompletionGate>,
                Arc::new(notifier),
            )
            .unwrap_or_else(|e| panic!("sink: {e}")),
        )
    }

    fn messages_of(memory: &MemorySink, kind: NotificationKind) -> Vec<String> {
        memory
            .delivered()
            .into_iter()
            .filter(|n| n.kind == kind)
            .map(|n| n.message)
            .collect()
    }

    #[test]
    fn run_started_records_history_and_notifies() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        sink.run_started(
            &def_with_criteria(&["task_completed"]),
            600,
            "auto-nightly-600",
            NetworkPolicy::Online,
        );
        assert_eq!(sink.pending_len(), 1);
        let rows = sink.history().for_automation("nightly")?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].outcome, "started");
        assert_eq!(rows[0].session_id, "auto-nightly-600");
        assert_eq!(rows[0].fired_at_minute, 600);
        assert!(!messages_of(&memory, NotificationKind::ScheduledFired).is_empty());
        Ok(())
    }

    #[test]
    fn turn_finish_flows_through_gate_and_notifies_verdict()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        let def = def_with_criteria(&["task_completed"]);
        sink.run_started(&def, 600, "auto-nightly-600", NetworkPolicy::Online);
        sink.on_turn_finished("auto-nightly-600", 1, "task_completed", Vec::new());
        let rows = sink.history().for_automation("nightly")?;
        assert_eq!(rows.len(), 2, "started + completion rows");
        assert_eq!(rows[1].verdict.as_deref(), Some("confirmed"));
        assert_eq!(rows[1].outcome, "task_completed");
        let turn_msgs = messages_of(&memory, NotificationKind::TurnComplete);
        assert_eq!(turn_msgs.len(), 1);
        assert!(turn_msgs[0].contains("confirmed"), "{}", turn_msgs[0]);
        assert_eq!(sink.pending_len(), 0);
        Ok(())
    }

    #[test]
    fn unmet_criteria_end_inconclusive() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        sink.run_started(
            &def_with_criteria(&["report written"]),
            600,
            "s-1",
            NetworkPolicy::Online,
        );
        sink.on_turn_finished("s-1", 1, "task_completed", Vec::new());
        let rows = sink.history().for_automation("nightly")?;
        assert_eq!(rows[1].verdict.as_deref(), Some("inconclusive"));
        Ok(())
    }

    #[test]
    fn restart_lost_pending_fails_closed_inconclusive() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        {
            let sink = sink_in(&dir, Arc::clone(&memory));
            sink.run_started(
                &def_with_criteria(&["task_completed"]),
                600,
                "s-1",
                NetworkPolicy::Online,
            );
        }
        let memory2 = Arc::new(MemorySink::new());
        let restarted = sink_in(&dir, Arc::clone(&memory2));
        restarted.on_turn_finished("s-1", 1, "task_completed", Vec::new());
        let rows = restarted.history().for_automation("nightly")?;
        assert_eq!(
            rows.last().map(|r| r.verdict.as_deref()),
            Some(Some("inconclusive")),
            "criteria lost across restart must not auto-confirm"
        );
        Ok(())
    }

    struct AlwaysRefuse;
    impl CompletionGate for AlwaysRefuse {
        fn review(&self, _: &EvidenceBundle) -> GateVerdict {
            GateVerdict::Refuted {
                reason: "adversarial reviewer".into(),
                covered: vec![],
            }
        }
    }

    #[test]
    fn refuted_verdict_is_recorded_verbatim() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let mut notifier = Notifier::new();
        notifier.add(Arc::clone(&memory) as std::sync::Arc<dyn crate::notify::NotificationSink>);
        let sink = Arc::new(RunSink::open(
            dir.path(),
            Arc::new(AlwaysRefuse),
            Arc::new(notifier),
        )?);
        sink.run_started(&def_with_criteria(&[]), 60, "s-1", NetworkPolicy::Online);
        sink.on_turn_finished("s-1", 1, "task_completed", Vec::new());
        let rows = sink.history().for_automation("nightly")?;
        assert_eq!(
            rows.last().map(|r| r.verdict.as_deref()),
            Some(Some("refuted"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn permission_ask_emits_approval_needed() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        let (host, _cmd_rx) = crate::attach::SessionHost::new(
            "s-ask",
            64,
            16,
            8,
            crate::mediation::MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        sink.spawn_watcher(&host, "s-ask");
        let mediator = host.mediator();
        let ask = tokio::spawn(async move {
            mediator
                .mediate(harness_core::inspectors::store::AskRequest {
                    tool: "edit_lines".into(),
                    target: Some("src/lib.rs".into()),
                    reason: "scheduled edit".into(),
                })
                .await
        });
        let mut seen = false;
        for _ in 0..40 {
            if !messages_of(&memory, NotificationKind::ApprovalNeeded).is_empty() {
                seen = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            seen,
            "no approval_needed notification: {:?}",
            memory.delivered()
        );
        let _ = ask.await;
        Ok(())
    }

    #[tokio::test]
    async fn watcher_gates_real_turn_finished_event() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        let (host, _cmd_rx) = crate::attach::SessionHost::new(
            "s-live",
            64,
            16,
            8,
            crate::mediation::MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        sink.run_started(
            &def_with_criteria(&["task_completed"]),
            60,
            "s-live",
            NetworkPolicy::Online,
        );
        sink.spawn_watcher(&host, "s-live");
        host.publish(crate::protocol::SessionEvent::TurnFinished {
            turn: 1,
            outcome: "task_completed".into(),
        });
        let mut done = None;
        for _ in 0..40 {
            let rows = sink.history().for_automation("nightly")?;
            if rows.len() == 2 {
                done = rows.last().cloned();
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let done = done.unwrap_or_else(|| panic!("watcher never gated the turn"));
        assert_eq!(done.verdict.as_deref(), Some("confirmed"));
        Ok(())
    }

    // --- todo 18: scheduled runs binding suite evidence ---------------

    use crate::tools::run_tests::{TestEntry, TestReport, TestSummary};
    use harness_core::event::{ContentBlock, Event};

    fn nextest_report(passing: &[&str], failing: &[&str]) -> TestReport {
        let tests: Vec<TestEntry> = passing
            .iter()
            .map(|name| TestEntry {
                name: (*name).into(),
                status: "passed".into(),
                duration_ms: Some(1),
            })
            .chain(failing.iter().map(|name| TestEntry {
                name: (*name).into(),
                status: "failed".into(),
                duration_ms: Some(2),
            }))
            .collect();
        let summary = TestSummary::of(&tests);
        TestReport {
            framework: "cargo-nextest".into(),
            command: "cargo nextest run --no-fail-fast".into(),
            exit_code: Some(if failing.is_empty() { 0 } else { 100 }),
            timed_out: false,
            suite_duration_ms: 12,
            tests,
            summary,
        }
    }

    fn recorded_tool_call(seq: u64, call_id: &str, tool: &str) -> SessionEvent {
        SessionEvent::Recorded {
            seq,
            event: Event::ToolCall {
                call_id: call_id.into(),
                tool: tool.into(),
                arguments: serde_json::json!({}),
            },
        }
    }

    fn recorded_run_tests_result(seq: u64, call_id: &str, report: &TestReport) -> SessionEvent {
        let json = serde_json::to_string(report).expect("serialize");
        SessionEvent::Recorded {
            seq,
            event: Event::ToolResult {
                call_id: call_id.into(),
                blocks: vec![ContentBlock::Text {
                    text: format!(
                        "{json}\n[safety] backend=windows-appcontainer requested=online \
                         enforced=offline"
                    ),
                }],
            },
        }
    }

    async fn await_history_verdict(
        sink: &RunSink,
        automation: &str,
    ) -> Option<crate::run_history::RunRecord> {
        for _ in 0..100 {
            if let Ok(rows) = sink.history().for_automation(automation)
                && let Some(last) = rows.last().filter(|r| r.verdict.is_some())
            {
                return Some(last.clone());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        None
    }

    async fn await_verdict_frame(
        frames: &mut tokio::sync::broadcast::Receiver<ServerMsg>,
    ) -> Option<(VerdictKind, Vec<u64>)> {
        for _ in 0..100 {
            match tokio::time::timeout(Duration::from_millis(50), frames.recv()).await {
                Ok(Ok(ServerMsg::Verdict { verdict, evidence, .. })) => {
                    return Some((verdict, evidence));
                }
                Ok(Ok(_)) => continue,
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(_)) | Err(_) => return None,
            }
        }
        None
    }

    #[tokio::test]
    async fn green_suite_run_confirms_and_verdict_cites_test_run_event()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        let (host, _cmd_rx) = crate::attach::SessionHost::new(
            "s-green",
            64,
            16,
            8,
            crate::mediation::MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        sink.run_started(
            &def_with_criteria(&["tests: cargo-nextest green"]),
            60,
            "s-green",
            NetworkPolicy::Online,
        );
        let mut frames = host.hub_subscribe();
        sink.spawn_watcher(&host, "s-green");
        host.publish(recorded_tool_call(1, "c1", "run_tests"));
        let report = nextest_report(&["rtfx_a passes", "rtfx_b passes"], &[]);
        let run_frame = host.publish(recorded_run_tests_result(2, "c1", &report));
        host.publish(SessionEvent::TurnFinished {
            turn: 1,
            outcome: "task_completed".into(),
        });
        let done =
            await_history_verdict(&sink, "nightly").await.expect("watcher never gated the turn");
        assert_eq!(done.verdict.as_deref(), Some("confirmed"));
        let (kind, evidence) =
            await_verdict_frame(&mut frames).await.expect("no Verdict frame observed");
        assert_eq!(kind, VerdictKind::Confirmed);
        assert!(
            evidence.contains(&run_frame.id),
            "verdict evidence {evidence:?} must cite the test-run event id {}",
            run_frame.id
        );
        // Drilldown: the cited frame is the run_tests observation and
        // its first line parses back to the per-test report.
        let cited = host
            .snapshot()
            .into_iter()
            .find(|f| f.id == run_frame.id)
            .expect("cited frame retained in the hub ring");
        let SessionEvent::Recorded {
            event: Event::ToolResult { blocks, .. },
            ..
        } = cited.event
        else {
            panic!("cited frame is not a tool result");
        };
        let text = blocks
            .iter()
            .find_map(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .expect("text block");
        let parsed: TestReport =
            serde_json::from_str(text.lines().next().expect("observation line")).expect("report");
        let names: Vec<&str> = parsed.tests.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["rtfx_a passes", "rtfx_b passes"]);
        Ok(())
    }

    #[tokio::test]
    async fn red_suite_run_refutes_citing_failing_test_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        let (host, _cmd_rx) = crate::attach::SessionHost::new(
            "s-red",
            64,
            16,
            8,
            crate::mediation::MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        sink.run_started(
            &def_with_criteria(&["tests: cargo-nextest green"]),
            60,
            "s-red",
            NetworkPolicy::Online,
        );
        let mut frames = host.hub_subscribe();
        sink.spawn_watcher(&host, "s-red");
        host.publish(recorded_tool_call(1, "c1", "run_tests"));
        let report = nextest_report(&["rtfx_c passes"], &["rtfx_a fails", "rtfx_b fails"]);
        host.publish(recorded_run_tests_result(2, "c1", &report));
        host.publish(SessionEvent::TurnFinished {
            turn: 1,
            outcome: "task_completed".into(),
        });
        let done =
            await_history_verdict(&sink, "nightly").await.expect("watcher never gated the turn");
        assert_eq!(done.verdict.as_deref(), Some("refuted"));
        let (kind, _) =
            await_verdict_frame(&mut frames).await.expect("no Verdict frame observed");
        assert_eq!(kind, VerdictKind::Refuted);
        let turn_msgs = messages_of(&memory, NotificationKind::TurnComplete);
        assert_eq!(turn_msgs.len(), 1);
        assert!(
            turn_msgs[0].contains("rtfx_a fails") && turn_msgs[0].contains("rtfx_b fails"),
            "refuted notification must cite the failing test ids: {}",
            turn_msgs[0]
        );
        Ok(())
    }

    #[tokio::test]
    async fn missing_test_run_stays_inconclusive_never_confirmed()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let memory = Arc::new(MemorySink::new());
        let sink = sink_in(&dir, Arc::clone(&memory));
        let (host, _cmd_rx) = crate::attach::SessionHost::new(
            "s-missing",
            64,
            16,
            8,
            crate::mediation::MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        sink.run_started(
            &def_with_criteria(&["tests: cargo-nextest green"]),
            60,
            "s-missing",
            NetworkPolicy::Online,
        );
        let mut frames = host.hub_subscribe();
        sink.spawn_watcher(&host, "s-missing");
        // A non-test tool ran and the turn completed: no run_tests
        // observation exists to bind.
        host.publish(recorded_tool_call(1, "c1", "view_window"));
        host.publish(SessionEvent::TurnFinished {
            turn: 1,
            outcome: "task_completed".into(),
        });
        let done =
            await_history_verdict(&sink, "nightly").await.expect("watcher never gated the turn");
        assert_eq!(
            done.verdict.as_deref(),
            Some("inconclusive"),
            "a suite binding without a test run must never confirm"
        );
        let (kind, _) =
            await_verdict_frame(&mut frames).await.expect("no Verdict frame observed");
        assert_eq!(kind, VerdictKind::Inconclusive);
        Ok(())
    }
}
