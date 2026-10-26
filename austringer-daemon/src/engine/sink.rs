//! The notification sink mapping the daemon's scheduled-run kinds onto
//! the owning session's event stream (the todo-2-named extraction from
//! the engine driver module).

use std::sync::Arc;

use crate::attach::SessionHost;
use crate::notify::{Notification, NotificationKind, NotificationSink, NotifyError};

/// Resolves a session id to its live host (read-only; never creates).
pub type HostResolver = Arc<dyn Fn(&str) -> Option<Arc<SessionHost>> + Send + Sync>;

/// Notification sink mirroring the daemon's scheduled-run kinds onto
/// the owning session's event stream as `SessionEvent::Notification`
/// frames — kind `sched` for a fired automation, `verdict` for a gated
/// completion. Complementary to the `Verdict` frame (todo 1): the same
/// facts on the v1-visible notification lane, beside the engine's own
/// `steer` kind. Other notification kinds keep their existing lanes
/// (asks ride `PermissionAsk`, workers their own frames).
pub struct EventStreamSink {
    resolve: HostResolver,
}

impl EventStreamSink {
    /// Sink resolving sessions through `resolve`.
    #[must_use]
    pub fn new(resolve: HostResolver) -> Self {
        Self { resolve }
    }
}

impl NotificationSink for EventStreamSink {
    fn deliver(&self, notification: &Notification) -> Result<(), NotifyError> {
        let kind = match notification.kind {
            NotificationKind::ScheduledFired => "sched",
            NotificationKind::TurnComplete => "verdict",
            NotificationKind::ApprovalNeeded | NotificationKind::WorkerLost => return Ok(()),
        };
        if let Some(session) = &notification.session_id
            && let Some(host) = (self.resolve)(session)
        {
            host.publish(crate::protocol::SessionEvent::Notification {
                kind: kind.to_owned(),
                message: notification.message.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{EventStreamSink, HostResolver, Notification, NotificationKind, NotificationSink};
    use crate::attach::SessionHost;
    use crate::mediation::MediationPolicy;
    use crate::protocol::{ServerMsg, SessionEvent};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn event_stream_sink_maps_only_sched_and_verdict() {
        let (host, _cmd_rx) = SessionHost::new(
            "s-sink",
            64,
            16,
            8,
            MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        let resolve: HostResolver = Arc::new({
            let host = Arc::clone(&host);
            move |sid: &str| (sid == "s-sink").then(|| Arc::clone(&host))
        });
        let sink = EventStreamSink::new(resolve);
        let mut hub = host.hub_subscribe();

        for kind in [
            NotificationKind::ScheduledFired,
            NotificationKind::TurnComplete,
            NotificationKind::ApprovalNeeded,
            NotificationKind::WorkerLost,
        ] {
            sink.deliver(&Notification {
                kind,
                session_id: Some("s-sink".into()),
                message: format!("m-{}", kind.as_str()),
            })
            .expect("deliver never fails");
        }
        // A session-scoped notification for an unknown session is a
        // no-op, not an error.
        sink.deliver(&Notification {
            kind: NotificationKind::ScheduledFired,
            session_id: Some("gone".into()),
            message: "nope".into(),
        })
        .expect("unknown session is a no-op");

        let mut kinds = Vec::new();
        while let Ok(msg) = hub.try_recv() {
            if let ServerMsg::Event { frame } = msg
                && let SessionEvent::Notification { kind, .. } = frame.event
            {
                kinds.push(kind);
            }
        }
        assert_eq!(kinds, vec!["sched".to_owned(), "verdict".to_owned()]);
    }
}
