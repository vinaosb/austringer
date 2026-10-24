//! Team-event mapping: the launcher's [`TeamEvent`] vocabulary onto
//! the parent session's hub — child-scoped facts become the typed
//! `Child*` session events (the todo-15 contract), launch-scoped
//! facts ride the v1-visible `Notification` lane with the
//! orchestration crate's kind strings.

use std::sync::Arc;

use harness_orchestration::events::{TeamEvent, TeamEventSink};

use crate::attach::SessionHost;
use crate::protocol::SessionEvent;

use super::task_of;

/// Publishes launcher team events through one session host.
pub struct HubTeamSink {
    host: Arc<SessionHost>,
}

impl HubTeamSink {
    /// Sink publishing through `host`.
    #[must_use]
    pub fn new(host: Arc<SessionHost>) -> Self {
        Self { host }
    }
}

impl TeamEventSink for HubTeamSink {
    fn emit(&self, event: TeamEvent) {
        let host = &self.host;
        match &event {
            TeamEvent::Spawned {
                child, role, model, ..
            } => {
                host.publish(SessionEvent::ChildStarted {
                    child: child.clone(),
                    task: task_of(child).to_owned(),
                    role: role.clone(),
                    model: model.clone(),
                });
            }
            TeamEvent::Completed { child } => {
                host.publish(SessionEvent::ChildFinished {
                    child: child.clone(),
                    task: task_of(child).to_owned(),
                    state: "success".to_owned(),
                    detail: "completed".to_owned(),
                });
            }
            TeamEvent::Failed { child, class } => {
                host.publish(SessionEvent::ChildFinished {
                    child: child.clone(),
                    task: task_of(child).to_owned(),
                    state: "failed".to_owned(),
                    detail: class.clone(),
                });
            }
            TeamEvent::FileTouched { child, path, op } => {
                host.publish(SessionEvent::ChildProgress {
                    child: child.clone(),
                    phase: "file_touched".to_owned(),
                    detail: format!("{op} {path}"),
                });
            }
            TeamEvent::Collision(collision) => {
                host.publish(SessionEvent::ChildProgress {
                    child: collision.b.clone(),
                    phase: "collision".to_owned(),
                    detail: format!(
                        "{} and {} both touch {}",
                        collision.a, collision.b, collision.path
                    ),
                });
            }
            TeamEvent::Escalation { child, granted } => {
                host.publish(SessionEvent::ChildProgress {
                    child: child.clone(),
                    phase: "escalation".to_owned(),
                    detail: if *granted {
                        "escalation granted".to_owned()
                    } else {
                        "escalation refused".to_owned()
                    },
                });
            }
            TeamEvent::WaveStarted { .. }
            | TeamEvent::BudgetStopped { .. }
            | TeamEvent::FamilySwitch { .. }
            | TeamEvent::BestOfNSelected { .. }
            | TeamEvent::PhaseTransition { .. }
            | TeamEvent::SpecStale { .. }
            | TeamEvent::LaunchFinished { .. } => {
                let notification = event.notification();
                host.publish(SessionEvent::Notification {
                    kind: notification.kind,
                    message: notification.message,
                });
            }
        }
    }
}
