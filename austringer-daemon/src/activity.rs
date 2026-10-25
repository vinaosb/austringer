//! Activity execution seam: replay-first journaling executor and
//! activity-level retry. The no-token-replay rule lives here — journaled
//! outputs are returned without consulting the inner executor (the
//! model is never re-invoked for replayed history).

use std::sync::{Arc, Mutex};

use crate::journal::{Journal, JournalStep, activity_id};

/// Activity execution seam.
pub trait ActivityExecutor: Send + Sync {
    /// Execute one activity.
    fn execute(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, ActivityError>;
}

/// Activity failures; `retryable` distinguishes transient activities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityError {
    /// Failure cause.
    pub message: String,
    /// Whether retrying the activity may succeed.
    pub retryable: bool,
}

/// A shared, journaling executor: replay-first, otherwise execute and
/// record.
pub struct JournalingExecutor {
    inner: Arc<dyn ActivityExecutor>,
    journal: Mutex<Journal>,
    executions: std::sync::atomic::AtomicUsize,
}

impl JournalingExecutor {
    /// Wrap `inner` with journaling.
    #[must_use]
    pub fn new(inner: Arc<dyn ActivityExecutor>, journal: Journal) -> Self {
        Self {
            inner,
            journal: Mutex::new(journal),
            executions: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// How many times the inner executor actually ran (0 on a full
    /// replay — the non-determinism guard).
    #[must_use]
    pub fn inner_executions(&self) -> usize {
        self.executions.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Journal> {
        match self.journal.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl ActivityExecutor for JournalingExecutor {
    fn execute(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, ActivityError> {
        let id = activity_id(name, input);
        let mut journal = self.lock();
        if let Some(output) = journal.completed_outputs().get(&id.0) {
            return Ok(output.clone());
        }
        journal
            .record(JournalStep::ActivityStarted {
                id: id.0.clone(),
                name: name.to_owned(),
                input: input.clone(),
            })
            .map_err(|e| ActivityError {
                message: e.to_string(),
                retryable: false,
            })?;
        let output = self.inner.execute(name, input)?;
        self.executions
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        journal
            .record(JournalStep::ActivityCompleted {
                id: id.0,
                output: output.clone(),
                retried: false,
            })
            .map_err(|e| ActivityError {
                message: e.to_string(),
                retryable: false,
            })?;
        Ok(output)
    }
}

/// Retry one activity by id: re-executes the ACTIVITY through
/// `executor` and overwrites its journaled output. The model is never
/// re-invoked — journaled inference activities replay their recorded
/// output unchanged.
pub fn retry_activity(
    journal: &mut Journal,
    executor: &dyn ActivityExecutor,
    id: &str,
) -> Result<serde_json::Value, ActivityError> {
    let target = journal.steps().iter().find_map(|step| match step {
        JournalStep::ActivityStarted {
            id: sid,
            name,
            input,
        } if sid == id => Some((name.clone(), input.clone())),
        _ => None,
    });
    let Some((name, input)) = target else {
        return Err(ActivityError {
            message: format!("unknown activity {id}"),
            retryable: false,
        });
    };
    let output = executor.execute(&name, &input)?;
    journal
        .record(JournalStep::ActivityCompleted {
            id: id.to_owned(),
            output: output.clone(),
            retried: true,
        })
        .map_err(|e| ActivityError {
            message: e.to_string(),
            retryable: false,
        })?;
    Ok(output)
}
