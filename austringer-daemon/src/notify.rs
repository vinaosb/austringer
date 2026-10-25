//! Typed notification emitter: one [`NotificationSink`] trait every
//! consumer implements (tracing log now; OSC 777 escape-sequence
//! emission and system notifiers land with todo 41 on this same trait).

use std::sync::Arc;

/// Typed notification vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    /// A turn workflow completed.
    TurnComplete,
    /// A permission ask is waiting (attention needed).
    ApprovalNeeded,
    /// A scheduled job fired.
    ScheduledFired,
    /// A remote worker vanished mid-operation.
    WorkerLost,
}

impl NotificationKind {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TurnComplete => "turn_complete",
            Self::ApprovalNeeded => "approval_needed",
            Self::ScheduledFired => "scheduled_fired",
            Self::WorkerLost => "worker_lost",
        }
    }

    /// Todo 24 kind→status mapping: the terminal program status this
    /// kind represents on the OSC emission lanes. Every current kind
    /// maps; a future kind without a terminal status returns `None`
    /// and the OSC sink emits NOTHING for it (fail silent — never a
    /// garbage sequence). The exhaustive match forces that decision at
    /// compile time.
    #[must_use]
    pub const fn program_status(self) -> Option<ProgramStatus> {
        match self {
            Self::ScheduledFired => Some(ProgramStatus::Working),
            Self::ApprovalNeeded => Some(ProgramStatus::Blocked),
            Self::TurnComplete => Some(ProgramStatus::Done),
            Self::WorkerLost => Some(ProgramStatus::Error),
        }
    }
}

/// Terminal program status (todo 24): the four-state lifecycle a
/// listening terminal renders — working / blocked (attention needed:
/// permission or input) / done / error. Receipts: OpenCode #55172,
/// Antigravity v1.4.0, Warp OSC-777 lifecycle, Warp issue #16407.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStatus {
    /// Running (Warp lifecycle `prompt_submit`; OSC 7501 `working`).
    Working,
    /// Cannot continue until the user acts (Warp `permission_request`;
    /// OSC 7501 `blocked` with `kind=permission` — today's only blocked
    /// source is the permission ask; question/auth arrive when their
    /// notification kinds do).
    Blocked,
    /// Finished (Warp `stop`; OSC 7501 `done`).
    Done,
    /// Failed and stopped (Warp `stop_failure`; OSC 7501 `error`).
    Error,
}

impl ProgramStatus {
    /// Warp's structured OSC 777 cli-agent lifecycle event name (the
    /// vocabulary Warp's listener parses, event/v1.rs).
    #[must_use]
    pub const fn warp_event(self) -> &'static str {
        match self {
            Self::Working => "prompt_submit",
            Self::Blocked => "permission_request",
            Self::Done => "stop",
            Self::Error => "stop_failure",
        }
    }

    /// OSC 7501 `state` token (spec: idle | working | done | blocked |
    /// error; the daemon never reports idle — a finished run stays
    /// `done` until replaced).
    #[must_use]
    pub const fn osc7501_state(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Error => "error",
        }
    }
}

/// One emitted notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Vocabulary kind.
    pub kind: NotificationKind,
    /// Owning session, when scoped to one.
    pub session_id: Option<String>,
    /// Human-facing message (bounded by the producer).
    pub message: String,
}

/// Delivery failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("notification delivery failed: {message}")]
pub struct NotifyError {
    /// Cause.
    pub message: String,
}

/// A notification consumer. Synchronous by design: sinks must not block
/// (buffer internally, forward over channels, or write a line).
pub trait NotificationSink: Send + Sync {
    /// Consume one notification.
    fn deliver(&self, notification: &Notification) -> Result<(), NotifyError>;
}

/// Fan-out emitter over registered sinks.
#[derive(Default)]
pub struct Notifier {
    sinks: Vec<Arc<dyn NotificationSink>>,
}

impl Notifier {
    /// Emitter with no sinks.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a sink.
    pub fn add(&mut self, sink: Arc<dyn NotificationSink>) {
        self.sinks.push(sink);
    }

    /// Deliver to every sink; per-sink failures are collected, never
    /// abort the fan-out.
    pub fn emit(&self, notification: &Notification) -> Vec<NotifyError> {
        self.sinks
            .iter()
            .filter_map(|sink| sink.deliver(notification).err())
            .collect()
    }

    /// Registered sink count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sinks.len()
    }

    /// True when no sinks are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sinks.is_empty()
    }
}

/// Structured tracing sink (always available).
pub struct TracingSink;

impl NotificationSink for TracingSink {
    fn deliver(&self, notification: &Notification) -> Result<(), NotifyError> {
        tracing::info!(
            kind = notification.kind.as_str(),
            session = notification.session_id.as_deref().unwrap_or(""),
            "notification: {}",
            notification.message
        );
        Ok(())
    }
}

/// In-memory sink for tests and embedders.
#[derive(Default)]
pub struct MemorySink {
    seen: std::sync::Mutex<Vec<Notification>>,
}

impl MemorySink {
    /// Empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Notifications delivered so far.
    pub fn delivered(&self) -> Vec<Notification> {
        match self.seen.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl NotificationSink for MemorySink {
    fn deliver(&self, notification: &Notification) -> Result<(), NotifyError> {
        match self.seen.lock() {
            Ok(mut guard) => {
                guard.push(notification.clone());
                Ok(())
            }
            Err(poisoned) => {
                poisoned.into_inner().push(notification.clone());
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(kind: NotificationKind) -> Notification {
        Notification {
            kind,
            session_id: Some("s-1".into()),
            message: "hello".into(),
        }
    }

    struct FailingSink;

    impl NotificationSink for FailingSink {
        fn deliver(&self, _: &Notification) -> Result<(), NotifyError> {
            Err(NotifyError {
                message: "sink down".into(),
            })
        }
    }

    #[test]
    fn fan_out_reaches_all_sinks() {
        let mut notifier = Notifier::new();
        let a = Arc::new(MemorySink::new());
        let b = Arc::new(MemorySink::new());
        notifier.add(Arc::clone(&a) as Arc<dyn NotificationSink>);
        notifier.add(Arc::clone(&b) as Arc<dyn NotificationSink>);
        let errors = notifier.emit(&notification(NotificationKind::TurnComplete));
        assert!(errors.is_empty());
        assert_eq!(a.delivered().len(), 1);
        assert_eq!(b.delivered().len(), 1);
        assert_eq!(notifier.len(), 2);
    }

    #[test]
    fn sink_failure_does_not_abort_fanout() {
        let mut notifier = Notifier::new();
        let ok = Arc::new(MemorySink::new());
        notifier.add(Arc::new(FailingSink) as Arc<dyn NotificationSink>);
        notifier.add(Arc::clone(&ok) as Arc<dyn NotificationSink>);
        let errors = notifier.emit(&notification(NotificationKind::WorkerLost));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "sink down");
        assert_eq!(ok.delivered().len(), 1);
    }

    #[test]
    fn kind_vocabulary_is_stable() {
        assert_eq!(NotificationKind::TurnComplete.as_str(), "turn_complete");
        assert_eq!(NotificationKind::ApprovalNeeded.as_str(), "approval_needed");
        assert_eq!(NotificationKind::ScheduledFired.as_str(), "scheduled_fired");
        assert_eq!(NotificationKind::WorkerLost.as_str(), "worker_lost");
    }

    #[test]
    fn kind_to_status_mapping_covers_every_kind() {
        assert_eq!(
            NotificationKind::ScheduledFired.program_status(),
            Some(ProgramStatus::Working)
        );
        assert_eq!(
            NotificationKind::ApprovalNeeded.program_status(),
            Some(ProgramStatus::Blocked)
        );
        assert_eq!(
            NotificationKind::TurnComplete.program_status(),
            Some(ProgramStatus::Done)
        );
        assert_eq!(
            NotificationKind::WorkerLost.program_status(),
            Some(ProgramStatus::Error)
        );
    }

    #[test]
    fn program_status_wire_tokens_are_stable() {
        assert_eq!(ProgramStatus::Working.osc7501_state(), "working");
        assert_eq!(ProgramStatus::Blocked.osc7501_state(), "blocked");
        assert_eq!(ProgramStatus::Done.osc7501_state(), "done");
        assert_eq!(ProgramStatus::Error.osc7501_state(), "error");
        assert_eq!(ProgramStatus::Working.warp_event(), "prompt_submit");
        assert_eq!(ProgramStatus::Blocked.warp_event(), "permission_request");
        assert_eq!(ProgramStatus::Done.warp_event(), "stop");
        assert_eq!(ProgramStatus::Error.warp_event(), "stop_failure");
    }
}
