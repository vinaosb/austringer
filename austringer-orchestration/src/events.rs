//! Team events: the daemon-event surface for subagent launches
//! (tmux-grid/team visualization). The serialized vocabulary rides the
//! daemon's frozen protocol v1 `SessionEvent::Notification { kind,
//! message }` escape hatch — new team-event kinds are new `kind` strings,
//! never protocol changes.

use std::sync::Mutex;

use crate::write_set::Collision;

/// One team event.
#[derive(Debug, Clone, PartialEq)]
pub enum TeamEvent {
    /// A child session was spawned.
    Spawned {
        /// Child id.
        child: String,
        /// Role name.
        role: String,
        /// Resolved model id.
        model: String,
        /// Resolved effort.
        effort: String,
    },
    /// A child completed.
    Completed {
        /// Child id.
        child: String,
    },
    /// A child failed.
    Failed {
        /// Child id.
        child: String,
        /// Failure class wire name.
        class: String,
    },
    /// A child touched a file.
    FileTouched {
        /// Child id.
        child: String,
        /// Path.
        path: String,
        /// Operation wire name.
        op: String,
    },
    /// Siblings collided on a path (both sides named).
    Collision(Collision),
    /// An escalation was decided.
    Escalation {
        /// Child id.
        child: String,
        /// Granted?
        granted: bool,
    },
    /// Spawning stopped on a budget/pacing reason.
    BudgetStopped {
        /// Human-readable reason.
        reason: String,
    },
    /// A wave started.
    WaveStarted {
        /// Wave index.
        index: u32,
        /// Child ids in the wave.
        children: Vec<String>,
    },
    /// The model family changed mid-run (surfaced, never silent).
    FamilySwitch {
        /// Family left.
        from: String,
        /// Family entered.
        to: String,
    },
    /// Best-of-N selected a winner.
    BestOfNSelected {
        /// Winner child id.
        winner: String,
        /// Winner score.
        score: u32,
    },
    /// A /boost phase transition.
    PhaseTransition {
        /// Phase name.
        phase: String,
    },
    /// The spec mutated mid-run; the launch aborted.
    SpecStale {
        /// Fingerprint at launch.
        expected: String,
        /// Fingerprint now.
        actual: String,
    },
    /// The launch finished.
    LaunchFinished {
        /// Children run.
        children: usize,
        /// Completed without early stop?
        complete: bool,
    },
}

/// A daemon-notification mapping (kind + message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Notification kind (the frozen-protocol string channel).
    pub kind: String,
    /// Human-readable payload.
    pub message: String,
}

impl TeamEvent {
    /// Maps the event onto the daemon's Notification surface.
    pub fn notification(&self) -> Notification {
        let (kind, message) = match self {
            Self::Spawned {
                child,
                role,
                model,
                effort,
            } => (
                "subagent_spawned",
                format!("{child} spawned as {role} on {model} at effort {effort}"),
            ),
            Self::Completed { child } => ("subagent_completed", format!("{child} completed")),
            Self::Failed { child, class } => {
                ("subagent_failed", format!("{child} failed: {class}"))
            }
            Self::FileTouched { child, path, op } => {
                ("file_touched", format!("{child} {op} {path}"))
            }
            Self::Collision(c) => (
                "file_touch_collision",
                format!("{} and {} both touch {}", c.a, c.b, c.path),
            ),
            Self::Escalation { child, granted } => (
                "escalation",
                format!(
                    "{child} escalation {}",
                    if *granted { "granted" } else { "refused" }
                ),
            ),
            Self::BudgetStopped { reason } => {
                ("budget_stopped", format!("spawning stopped: {reason}"))
            }
            Self::WaveStarted { index, children } => (
                "wave_started",
                format!("wave {index}: {}", children.join(", ")),
            ),
            Self::FamilySwitch { from, to } => (
                "family_switch",
                format!(
                    "model family changed mid-run: {from} -> {to} (context continuity surfaced)"
                ),
            ),
            Self::BestOfNSelected { winner, score } => (
                "best_of_n_selected",
                format!("{winner} won with score {score}"),
            ),
            Self::PhaseTransition { phase } => {
                ("phase_transition", format!("entering {phase} phase"))
            }
            Self::SpecStale { expected, actual } => (
                "spec_stale",
                format!("spec mutated mid-run: {expected} -> {actual}; aborting"),
            ),
            Self::LaunchFinished { children, complete } => (
                "launch_finished",
                format!("{children} children; complete={complete}"),
            ),
        };
        Notification {
            kind: kind.to_string(),
            message,
        }
    }
}

/// Event sink trait (implementations forward to the daemon's notification
/// surface, a UI, or a recorder).
pub trait TeamEventSink: Send + Sync {
    /// Consume one event.
    fn emit(&self, event: TeamEvent);
}

/// In-memory sink for tests and recorders.
#[derive(Debug, Default)]
pub struct MemoryEventSink {
    events: Mutex<Vec<TeamEvent>>,
}

impl MemoryEventSink {
    /// Empty sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// All events emitted so far, in order.
    pub fn events(&self) -> Vec<TeamEvent> {
        self.events.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

impl TeamEventSink for MemoryEventSink {
    fn emit(&self, event: TeamEvent) {
        if let Ok(mut queue) = self.events.lock() {
            queue.push(event);
        }
    }
}

/// Renders the tmux-grid team view: one pane line per active child.
pub fn render_grid(active: &[(&str, &str, &str)]) -> String {
    let mut out = String::from("+-- team grid\n");
    for (child, role, model) in active {
        out.push_str(&format!("+-- {child} ({role}/{model})\n"));
    }
    out
}
