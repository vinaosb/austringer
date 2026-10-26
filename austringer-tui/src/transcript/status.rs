//! Status-line notices: the arm bodies that render plan-mode, cost,
//! and rewind events as transcript `Entry::Notice` rows (the exhaustive
//! `apply_frame` match stays in [`super`] by the workspace rule).

use super::{Entry, SessionView};

impl SessionView {
    /// `PlanModeEntered` arm body.
    pub(crate) fn apply_plan_entered(&mut self, plan: u64) {
        self.entries.push(Entry::Notice(format!(
            "plan mode entered (plan {plan}); writes gated until approval"
        )));
    }

    /// `PlanModeExited` arm body.
    pub(crate) fn apply_plan_exited(&mut self, plan: u64) {
        self.entries.push(Entry::Notice(format!(
            "plan mode exited without approval (plan {plan})"
        )));
    }

    /// `PlanBlocked` arm body.
    pub(crate) fn apply_plan_blocked(&mut self, plan: u64, tool: &str, target: &Option<String>) {
        let detail = match target {
            Some(target) => {
                format!("plan {plan} pending approval: write tool '{tool}' on '{target}' blocked")
            }
            None => format!("plan {plan} pending approval: write tool '{tool}' blocked"),
        };
        self.entries.push(Entry::Notice(detail));
    }

    /// `PlanUpdated` arm body.
    pub(crate) fn apply_plan_updated(&mut self, plan: u64, revision: u64) {
        self.entries.push(Entry::Notice(format!(
            "plan {plan} artifact edited (revision {revision}); writes re-gated"
        )));
    }

    /// `PlanApproved` arm body.
    pub(crate) fn apply_plan_approved(&mut self, plan: u64) {
        self.entries.push(Entry::Notice(format!(
            "plan {plan} approved; writes re-armed"
        )));
    }

    /// `PlanRejected` arm body.
    pub(crate) fn apply_plan_rejected(&mut self, plan: u64) {
        self.entries.push(Entry::Notice(format!(
            "plan {plan} rejected; plan mode stays active"
        )));
    }

    /// `CostWarning` arm body.
    pub(crate) fn apply_cost_warning(&mut self, consumed_micros: u64, cap_micros: u64) {
        self.entries.push(Entry::Notice(format!(
            "cost warning: {consumed_micros} of {cap_micros} micro-USD (80% of cap)"
        )));
    }

    /// `CostStop` arm body.
    pub(crate) fn apply_cost_stop(&mut self, consumed_micros: u64, cap_micros: u64) {
        self.entries.push(Entry::Notice(format!(
            "cost stop: cap reached ({consumed_micros} of {cap_micros} micro-USD); further turns refused"
        )));
    }

    /// `Event::Snapshot` arm body: the rewind-point marker row.
    pub(crate) fn apply_rewind_point(&mut self, label: &str) {
        self.entries.push(Entry::Notice(format!(
            "{}: {label}",
            austringer_daemon::grammar::VOCAB_REWIND_POINT
        )));
    }
}
