//! Task_id-resumable children (the Kilo pattern): re-run one child
//! under its ORIGINAL identity — same child id, same resolved knobs.

use std::sync::Arc;

use crate::events::TeamEvent;
use crate::journal::JournalRecord;
use crate::launch::{ChildReport, Launcher, SubagentExecutor};
use crate::report::ResumeError;
use crate::resolve::ResolvedPlan;
use crate::telemetry::clamp_summary;

impl Launcher {
    /// Re-runs one child under its ORIGINAL identity (Kilo task_id
    /// resumability).
    pub async fn resume(
        &mut self,
        plan: &ResolvedPlan,
        executor: Arc<dyn SubagentExecutor>,
        task_id: &str,
    ) -> Result<ChildReport, ResumeError> {
        let wanted = format!("{}-child-{}", plan.launch_id, task_id);
        let job = plan
            .waves
            .iter()
            .flatten()
            .find(|j| j.child_id == wanted)
            .cloned()
            .ok_or_else(|| ResumeError::Missing(task_id.to_string()))?;
        self.announce(&job, true)?;
        match executor.run(job).await {
            Ok(mut report) => {
                report.summary = clamp_summary(&report.summary, crate::telemetry::SUMMARY_MAX);
                self.journal.record(JournalRecord::ChildCompleted {
                    child: report.child_id.clone(),
                    result: report.summary.clone(),
                })?;
                self.emit(TeamEvent::Completed {
                    child: report.child_id.clone(),
                });
                Ok(report)
            }
            Err(failure) => {
                self.journal.record(JournalRecord::ChildFailed {
                    child: failure.child_id.clone(),
                    class: failure.class.name().to_string(),
                    detail: failure.detail.clone(),
                })?;
                self.emit(TeamEvent::Failed {
                    child: failure.child_id.clone(),
                    class: failure.class.name().to_string(),
                });
                Err(ResumeError::Failure(failure))
            }
        }
    }
}
