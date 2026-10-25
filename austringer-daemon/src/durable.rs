//! Durable turn execution (Cursor Temporal §19 *pattern*, not the
//! dependency): every turn is a journaled workflow of typed steps.
//! Replay reuses journaled activity outputs without re-invoking the
//! model (no token replay), activity retry re-runs the activity alone,
//! signals delivered mid-turn follow signal-with-start semantics, and
//! child workflows get their own journals. Overhead: one JSONL append
//! per step (fsync on turn boundaries); journals live under
//! `{data}/journal/{session}/`. Default-on via
//! [`crate::config::DaemonConfig`].

use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

pub use crate::activity::{ActivityError, ActivityExecutor, JournalingExecutor, retry_activity};
pub use crate::journal::{
    ActivityId, DurableError, Journal, JournalStep, activity_id, child_journal_path,
    turn_journal_path,
};

/// What a signal-with-start decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalOutcome {
    /// No turn was running: the signal started one.
    Started,
    /// A turn was running: the signal was journaled and queued to it.
    Delivered,
}

/// One turn's durable workflow driver over a [`Journal`].
pub struct TurnWorkflow {
    journal: Journal,
    turn: u64,
    running: bool,
}

impl TurnWorkflow {
    /// Open the workflow journal for `session`/`turn` under `root`.
    pub fn open(root: &Path, session: &str, turn: u64) -> Result<Self, DurableError> {
        Ok(Self {
            journal: Journal::open(&turn_journal_path(root, session, turn))?,
            turn,
            running: false,
        })
    }

    /// Direct construction over an existing journal (tests).
    #[must_use]
    pub fn from_journal(journal: Journal, turn: u64) -> Self {
        Self {
            journal,
            turn,
            running: false,
        }
    }

    /// Begin the turn. Refuses double-begin (a turn is a single
    /// workflow instance).
    pub fn begin(&mut self, prompt: &str) -> Result<(), DurableError> {
        if self.running {
            return Ok(());
        }
        self.running = true;
        self.journal.record(JournalStep::TurnStarted {
            turn: self.turn,
            prompt: prompt.to_owned(),
        })
    }

    /// Finish the turn and fsync its journal.
    pub fn finish(&mut self, outcome: &str) -> Result<(), DurableError> {
        if !self.running {
            return Ok(());
        }
        self.running = false;
        let out = self.journal.record(JournalStep::TurnFinished {
            outcome: outcome.to_owned(),
        });
        let synced = self.journal.sync();
        out.and(synced)
    }

    /// Whether a turn is mid-flight.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// The underlying journal.
    #[must_use]
    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    /// Signal-with-start: `running` decides delivery. The signal is
    /// journaled either way; on the delivered path it is also pushed to
    /// `steer` (the live turn's steering queue).
    pub fn signal_with_start(
        &mut self,
        text: &str,
        running: bool,
        steer: &mpsc::Sender<String>,
    ) -> Result<SignalOutcome, DurableError> {
        self.journal.record(JournalStep::Signal {
            text: text.to_owned(),
        })?;
        if running {
            let _ = steer.try_send(text.to_owned());
            Ok(SignalOutcome::Delivered)
        } else {
            self.begin(text).map(|()| SignalOutcome::Started)
        }
    }

    /// Record a child workflow spawn + return its journal path.
    pub fn spawn_child(&mut self, child: &str, spec: &str) -> Result<PathBuf, DurableError> {
        self.journal.record(JournalStep::ChildStarted {
            child: child.to_owned(),
            spec: spec.to_owned(),
        })?;
        Ok(self
            .journal
            .path()
            .with_file_name(format!("child-{child}.jsonl")))
    }

    /// Record a child workflow completion.
    pub fn child_done(&mut self, child: &str, result: &str) -> Result<(), DurableError> {
        self.journal.record(JournalStep::ChildCompleted {
            child: child.to_owned(),
            result: result.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Arc;

    struct CountingExecutor {
        output: serde_json::Value,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl ActivityExecutor for CountingExecutor {
        fn execute(
            &self,
            _name: &str,
            _input: &serde_json::Value,
        ) -> Result<serde_json::Value, ActivityError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.output.clone())
        }
    }

    /// Non-deterministic executor: every call returns something new.
    struct RandomExecutor {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl Default for RandomExecutor {
        fn default() -> Self {
            Self {
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl ActivityExecutor for RandomExecutor {
        fn execute(
            &self,
            _name: &str,
            _input: &serde_json::Value,
        ) -> Result<serde_json::Value, ActivityError> {
            let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(serde_json::json!({ "dice": n, "random": true }))
        }
    }

    #[test]
    fn activity_ids_are_content_addressed() {
        let input = serde_json::json!({"cmd": "ls"});
        assert_eq!(activity_id("bash", &input), activity_id("bash", &input));
        assert_ne!(activity_id("bash", &input), activity_id("edit", &input));
        assert_ne!(
            activity_id("bash", &input),
            activity_id("bash", &serde_json::json!({"cmd": "pwd"}))
        );
    }

    #[test]
    fn journal_roundtrip_preserves_history() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = turn_journal_path(dir.path(), "s1", 1);
        let mut journal = Journal::open(&path)?;
        journal.record(JournalStep::TurnStarted {
            turn: 1,
            prompt: "hi".into(),
        })?;
        journal.record(JournalStep::ActivityStarted {
            id: activity_id("t", &serde_json::json!(1)).0,
            name: "t".into(),
            input: serde_json::json!(1),
        })?;
        drop(journal);
        let reopened = Journal::open(&path)?;
        assert_eq!(reopened.steps().len(), 2);
        Ok(())
    }

    #[test]
    fn torn_tail_lines_are_skipped_not_fatal() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = turn_journal_path(dir.path(), "s1", 1);
        {
            let mut journal = Journal::open(&path)?;
            journal.record(JournalStep::TurnStarted {
                turn: 1,
                prompt: "hi".into(),
            })?;
        }
        let mut torn = std::fs::OpenOptions::new().append(true).open(&path)?;
        torn.write_all(b"{\"type\":\"turn_sta")?;
        drop(torn);
        let reopened = Journal::open(&path)?;
        assert_eq!(reopened.steps().len(), 1);
        Ok(())
    }

    #[test]
    fn full_replay_makes_zero_executor_calls() {
        // Production-history replay: a NON-DETERMINISTIC executor must
        // not be consulted at all when the journal covers the history.
        let inner = Arc::new(RandomExecutor::default());
        let mut seed = Journal::in_memory();
        let id = activity_id("tool.x", &serde_json::json!({"a": 1}));
        seed.record(JournalStep::ActivityStarted {
            id: id.0.clone(),
            name: "tool.x".into(),
            input: serde_json::json!({"a": 1}),
        })
        .ok();
        seed.record(JournalStep::ActivityCompleted {
            id: id.0,
            output: serde_json::json!({"stable": "output"}),
            retried: false,
        })
        .ok();
        let exec = JournalingExecutor::new(Arc::clone(&inner) as Arc<dyn ActivityExecutor>, seed);
        let out = exec.execute("tool.x", &serde_json::json!({"a": 1})).ok();
        assert_eq!(out, Some(serde_json::json!({"stable": "output"})));
        assert_eq!(exec.inner_executions(), 0);
        assert_eq!(inner.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn retry_redoes_activity_not_inference() {
        let tool = Arc::new(CountingExecutor {
            output: serde_json::json!("tool-result-2"),
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let model = Arc::new(CountingExecutor {
            output: serde_json::json!("inference"),
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let mut journal = Journal::in_memory();
        let infer_id = activity_id("inference", &serde_json::json!({"turn": 1}));
        let tool_id = activity_id("tool.run", &serde_json::json!({"cmd": "x"}));
        journal
            .record(JournalStep::ActivityStarted {
                id: infer_id.0.clone(),
                name: "inference".into(),
                input: serde_json::json!({"turn": 1}),
            })
            .ok();
        journal
            .record(JournalStep::ActivityCompleted {
                id: infer_id.0,
                output: serde_json::json!("model said hi"),
                retried: false,
            })
            .ok();
        journal
            .record(JournalStep::ActivityStarted {
                id: tool_id.0.clone(),
                name: "tool.run".into(),
                input: serde_json::json!({"cmd": "x"}),
            })
            .ok();
        journal
            .record(JournalStep::ActivityCompleted {
                id: tool_id.0.clone(),
                output: serde_json::json!("tool-result-1"),
                retried: false,
            })
            .ok();

        let out = retry_activity(&mut journal, &*tool, &tool_id.0);
        assert_eq!(out.ok(), Some(serde_json::json!("tool-result-2")));
        assert_eq!(
            tool.calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "activity retried once"
        );
        assert_eq!(
            model.calls.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "inference never re-invoked"
        );
        let outputs = journal.completed_outputs();
        assert_eq!(
            outputs.get(&tool_id.0),
            Some(&serde_json::json!("tool-result-2")),
            "latest retry wins"
        );
    }

    #[tokio::test]
    async fn signal_with_start_routes_on_running_state() -> Result<(), DurableError> {
        let (steer_tx, mut steer_rx) = mpsc::channel(4);
        let mut wf = TurnWorkflow::from_journal(Journal::in_memory(), 1);
        let started = wf.signal_with_start("first message", false, &steer_tx)?;
        assert_eq!(started, SignalOutcome::Started);
        assert!(wf.is_running());
        let delivered = wf.signal_with_start("follow-up", true, &steer_tx)?;
        assert_eq!(delivered, SignalOutcome::Delivered);
        let steered = steer_rx.try_recv();
        assert_eq!(steered.ok(), Some("follow-up".to_owned()));
        wf.finish("complete")?;
        assert!(!wf.is_running());
        assert_eq!(wf.journal().steps().len(), 4);
        Ok(())
    }

    #[test]
    fn child_workflow_journals_in_its_own_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut parent = TurnWorkflow::open(dir.path(), "s1", 1)?;
        parent.begin("do the thing")?;
        let child_path = parent.spawn_child("1", "research subtask")?;
        assert!(child_path.ends_with("child-1.jsonl"));
        let mut child = Journal::open(&child_path)?;
        child
            .record(JournalStep::ActivityStarted {
                id: activity_id("step", &serde_json::json!(0)).0,
                name: "step".into(),
                input: serde_json::json!(0),
            })
            .ok();
        parent.child_done("1", "research summary")?;
        parent.finish("done")?;
        assert!(child_path.exists());
        assert_eq!(child.steps().len(), 1);
        Ok(())
    }

    #[test]
    fn open_turn_detection_drives_resume() -> Result<(), DurableError> {
        let mut journal = Journal::in_memory();
        journal
            .record(JournalStep::TurnStarted {
                turn: 3,
                prompt: "p".into(),
            })
            .ok();
        assert_eq!(journal.open_turn_prompt(), Some("p".to_owned()));
        journal
            .record(JournalStep::TurnFinished {
                outcome: "ok".into(),
            })
            .ok();
        assert_eq!(journal.open_turn_prompt(), None);
        Ok(())
    }
}
