//! Typed event stream hub: monotonic ids, bounded replay ring, and
//! divergence-proof resync. One hub per session; every transport reads
//! from the same hub so attached clients see identical frames.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::protocol::{MAX_SNAPSHOT_FRAMES, ServerMsg};

/// Daemon-emitted session events on the typed event stream. `recorded`
/// wraps the engine's own durable event; the rest are daemon-level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// An engine event, sequence-stamped.
    Recorded {
        /// Engine sequence number (1-based).
        seq: u64,
        /// The durable event.
        event: harness_core::event::Event,
    },
    /// A turn workflow started.
    TurnStarted {
        /// Turn number.
        turn: u64,
    },
    /// A turn workflow finished.
    TurnFinished {
        /// Turn number.
        turn: u64,
        /// Run outcome summary.
        outcome: String,
    },
    /// A semantic diff surfaced for review UIs.
    Diff {
        /// Changed path (relative, forward slashes).
        path: String,
        /// blake3 of the before content (hex, empty for adds).
        before_hash: String,
        /// blake3 of the after content (hex, empty for deletes).
        after_hash: String,
        /// Rendered hunks.
        hunks: Vec<String>,
    },
    /// A notification crossed the daemon bus.
    Notification {
        /// Notification kind (typed vocabulary).
        kind: String,
        /// Human-facing message.
        message: String,
    },
    /// A launcher child session started (todo 8 team contract; v2
    /// wires carry it typed, v1 connections receive the downgraded
    /// `Notification` form from [`crate::compat`]).
    ChildStarted {
        /// Child session id (`{launch}-child-{task}`).
        child: String,
        /// Task label (the spec's task key).
        task: String,
        /// Role name (explorer/worker/deep).
        role: String,
        /// Resolved model id.
        model: String,
    },
    /// A launcher child's phase changed (file touches, escalations,
    /// collisions, an ask blocking the child, resumption). Payload
    /// fields feed a status aggregator: child id plus a stable phase
    /// name plus context.
    ChildProgress {
        /// Child session id.
        child: String,
        /// Stable phase name (`file_touched`, `collision`,
        /// `escalation`, `blocked` on a surfaced ask, back to
        /// `running` when it resolves).
        phase: String,
        /// Phase context (touched path, contested path, the ask
        /// summary, decision).
        detail: String,
    },
    /// A launcher child finished (or was quarantined before launch).
    ChildFinished {
        /// Child session id.
        child: String,
        /// Task label.
        task: String,
        /// Outcome state: `success`, `failed`, or `quarantined`.
        state: String,
        /// Budget/collision context (summary, failure class, or the
        /// overlapping glob that quarantined the child).
        detail: String,
    },
    /// Session spend crossed the warning threshold (80% of the session
    /// cost cap; todo 33). The badge-state payload HUD surfaces
    /// consume (todos 19/20); downgraded for v1 wires to a
    /// `cost_warning` notification.
    CostWarning {
        /// Cumulative priced spend, micro-USD. u64 on the wire: serde's
        /// internally-tagged Content buffering cannot deserialize u128,
        /// so u128 fields made every client reject the frame. The
        /// internal ledger stays u128; the boundary conversion
        /// saturates at u64::MAX.
        consumed_micros: u64,
        /// The session cost cap, micro-USD (u64 on the wire; see
        /// `consumed_micros`).
        cap_micros: u64,
    },
    /// Session spend reached 100% of the cap: the typed terminal cost
    /// stop. Further turns are refused naming the cap; no model
    /// substitution ever happens (the ask-first contract stands above
    /// the halt).
    CostStop {
        /// Cumulative priced spend, micro-USD (may overshoot on the
        /// crossing record; u64 on the wire, saturating conversion).
        consumed_micros: u64,
        /// The session cost cap, micro-USD (u64 on the wire).
        cap_micros: u64,
    },
    /// The per-parent delegation budget after a spawn authorization or
    /// a recorded spend (todo 15 team surfacing; the
    /// [`crate::task::DelegationSnapshot`] fields). v1 wires receive
    /// the downgraded `team_budget` notification.
    TeamBudget {
        /// Task children spawned this session.
        spawned: u32,
        /// Tokens spent across those children.
        tokens_spent: u64,
        /// The effective fan-out cap.
        fan_out_cap: u32,
        /// The per-parent token budget.
        token_budget: u64,
    },
    /// The per-parent token budget is exhausted: further task spawns
    /// are refused (todo 15; the typed halt event alongside the
    /// [`crate::task::TaskRefusal::Tokens`] refusal).
    TeamBudgetExhausted {
        /// Tokens spent (at or over the budget).
        spent: u64,
        /// The exhausted budget.
        budget: u64,
    },
    /// Interactive plan mode entered (todo 16): `plan` names the
    /// pending plan every gated write will carry. Downgraded for v1
    /// wires to a `plan_mode_entered` notification.
    PlanModeEntered {
        /// The pending plan id.
        plan: u64,
    },
    /// Interactive plan mode exited without approval (the `/mode off`
    /// path; approval itself surfaces [`SessionEvent::PlanApproved`]).
    /// v1 wires receive the `plan_mode_exited` notification.
    PlanModeExited {
        /// The plan that was abandoned.
        plan: u64,
    },
    /// A write tool was blocked by the pending plan gate (todo 16):
    /// the block carries the plan id so frontends can offer the
    /// approve/edit/reject surface. v1 wires receive the
    /// `plan_blocked` notification.
    PlanBlocked {
        /// The pending plan id.
        plan: u64,
        /// The write tool that was blocked.
        tool: String,
        /// The call's target (path or command), when one was carried.
        target: Option<String>,
    },
    /// The plan artifact was edited (todo 16; the
    /// plan-as-editable-artifact receipt): the revision bumps and
    /// writes re-gate. v1 wires receive the `plan_updated`
    /// notification.
    PlanUpdated {
        /// The pending plan id.
        plan: u64,
        /// The new artifact revision.
        revision: u64,
    },
    /// The plan was approved (todo 16): plan mode ends and autonomy
    /// flips back per the matrix. v1 wires receive the `plan_approved`
    /// notification.
    PlanApproved {
        /// The approved plan id.
        plan: u64,
    },
    /// The plan was rejected (todo 16): the plan is refused but plan
    /// mode STAYS active and writes stay gated. v1 wires receive the
    /// `plan_rejected` notification.
    PlanRejected {
        /// The rejected plan id.
        plan: u64,
    },
}

/// One numbered frame on the monotonic per-session event stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Monotonic, gap-free, 1-based event id.
    pub id: u64,
    /// The event.
    pub event: SessionEvent,
}

/// What a subscriber needs before switching to the live stream.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayKind {
    /// The ring still covers the client's position: replay these frames
    /// (ids strictly greater than the client's last seen).
    Delta {
        /// Frames to replay, oldest first.
        frames: Vec<Frame>,
    },
    /// The client fell behind the ring head: delta replay is impossible.
    /// The transport must send `StateResyncRequired` followed by a
    /// snapshot — the delta-skipping reducer.
    Resync {
        /// Highest event id at subscribe time.
        last_event_id: u64,
    },
}

/// A subscription: bounded replay plan plus the live fan-out receiver.
pub struct Subscription {
    /// Replay plan computed under the hub lock.
    pub replay: ReplayKind,
    /// Live frames from now on. Lagging here surfaces as
    /// `RecvError::Lagged`, which transports convert to a forced resync.
    pub live: broadcast::Receiver<ServerMsg>,
}

/// Per-session event hub.
pub struct EventHub {
    next_id: u64,
    ring: VecDeque<Frame>,
    ring_capacity: usize,
    tx: broadcast::Sender<ServerMsg>,
}

impl EventHub {
    /// New hub holding up to `ring_capacity` frames for replay and
    /// fanning out through a broadcast channel of `broadcast_capacity`.
    #[must_use]
    pub fn new(ring_capacity: usize, broadcast_capacity: usize) -> Self {
        Self {
            next_id: 0,
            ring: VecDeque::with_capacity(ring_capacity),
            ring_capacity,
            tx: broadcast::channel(broadcast_capacity.max(1)).0,
        }
    }

    /// Highest assigned event id.
    #[must_use]
    pub fn last_event_id(&self) -> u64 {
        self.next_id
    }

    /// Id of the oldest retained frame (0 when the ring is empty).
    #[must_use]
    pub fn ring_head_id(&self) -> u64 {
        self.ring.front().map_or(0, |f| f.id)
    }

    /// Assign the next monotonic id, retain the frame in the ring
    /// (evicting the oldest past capacity), and fan out to live
    /// subscribers.
    pub fn publish(&mut self, event: SessionEvent) -> Frame {
        self.next_id = self.next_id.saturating_add(1);
        let frame = Frame {
            id: self.next_id,
            event,
        };
        if self.ring_capacity > 0 {
            if self.ring.len() >= self.ring_capacity {
                self.ring.pop_front();
            }
            self.ring.push_back(frame.clone());
        }
        let _ = self.tx.send(ServerMsg::Event {
            frame: frame.clone(),
        });
        frame
    }

    /// Fan out one transient (non-framed) server message — e.g. a v2
    /// `MediaResult` receipt — to live subscribers without assigning an
    /// event id or retaining it in the ring: receipts are ephemeral,
    /// the durable record lives in the engine's recorded events. v1
    /// connections receive nothing on the wire (the pump downgrade
    /// withholds v2-only variants per connection).
    pub fn broadcast(&mut self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }

    /// Subscribe with the client's last seen event id. `None` means a
    /// fresh client: full snapshot (`ReplayKind::Resync` with a snapshot
    /// to follow, matching the "attach anywhere" contract).
    pub fn subscribe(&self, after: Option<u64>) -> Subscription {
        let live = self.tx.subscribe();
        let replay = match after {
            None => ReplayKind::Resync {
                last_event_id: self.next_id,
            },
            Some(last) => {
                if last >= self.next_id {
                    ReplayKind::Delta { frames: Vec::new() }
                } else if self.covers(last) {
                    ReplayKind::Delta {
                        frames: self.ring_after(last),
                    }
                } else {
                    ReplayKind::Resync {
                        last_event_id: self.next_id,
                    }
                }
            }
        };
        Subscription { replay, live }
    }

    /// True when the ring still retains the first frame after `last`,
    /// making a gap-free delta replay possible.
    fn covers(&self, last: u64) -> bool {
        let head = self.ring_head_id();
        head != 0 && head <= last.saturating_add(1)
    }

    fn ring_after(&self, last: u64) -> Vec<Frame> {
        self.ring.iter().filter(|f| f.id > last).cloned().collect()
    }

    /// Current full-state snapshot (the retained ring), bounded by
    /// [`MAX_SNAPSHOT_FRAMES`] — newest frames win when the ring is
    /// larger than the bound.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Frame> {
        if self.ring.len() <= MAX_SNAPSHOT_FRAMES {
            self.ring.iter().cloned().collect()
        } else {
            let skip = self.ring.len() - MAX_SNAPSHOT_FRAMES;
            self.ring.iter().skip(skip).cloned().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn_started(turn: u64) -> SessionEvent {
        SessionEvent::TurnStarted { turn }
    }

    #[test]
    fn ids_are_monotonic_and_gap_free() {
        let mut hub = EventHub::new(8, 8);
        for expected in 1..=5 {
            let frame = hub.publish(turn_started(expected));
            assert_eq!(frame.id, expected);
        }
        assert_eq!(hub.last_event_id(), 5);
    }

    #[test]
    fn ring_evicts_oldest_beyond_capacity() {
        let mut hub = EventHub::new(4, 8);
        for i in 1..=10 {
            hub.publish(turn_started(i));
        }
        assert_eq!(hub.ring_head_id(), 7);
        assert_eq!(hub.snapshot().len(), 4);
        assert_eq!(hub.snapshot().last().map(|f| f.id), Some(10));
    }

    #[test]
    fn subscribe_inside_ring_gets_exact_delta() {
        let mut hub = EventHub::new(8, 8);
        for i in 1..=6 {
            hub.publish(turn_started(i));
        }
        let sub = hub.subscribe(Some(3));
        let ReplayKind::Delta { frames } = sub.replay else {
            panic!("expected delta");
        };
        let ids: Vec<u64> = frames.iter().map(|f| f.id).collect();
        assert_eq!(ids, vec![4, 5, 6]);
    }

    #[test]
    fn subscribe_behind_ring_head_forces_resync() {
        let mut hub = EventHub::new(4, 8);
        for i in 1..=10 {
            hub.publish(turn_started(i));
        }
        let sub = hub.subscribe(Some(2));
        assert_eq!(sub.replay, ReplayKind::Resync { last_event_id: 10 });
    }

    #[test]
    fn fresh_client_attaches_via_resync_snapshot() {
        let mut hub = EventHub::new(16, 8);
        for i in 1..=3 {
            hub.publish(turn_started(i));
        }
        let sub = hub.subscribe(None);
        assert_eq!(sub.replay, ReplayKind::Resync { last_event_id: 3 });
        let snap = hub.snapshot();
        let ids: Vec<u64> = snap.iter().map(|f| f.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn resynced_client_keeps_receiving_live_events() {
        let mut hub = EventHub::new(4, 8);
        for i in 1..=5 {
            hub.publish(turn_started(i));
        }
        let Subscription {
            replay, mut live, ..
        } = hub.subscribe(Some(0));
        assert!(matches!(replay, ReplayKind::Resync { .. }));
        hub.publish(turn_started(6));
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => assert_eq!(frame.id, 6),
            other => panic!("expected live frame, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn slow_live_receiver_lags_then_can_resubscribe() {
        let mut hub = EventHub::new(64, 2);
        let mut slow = hub.subscribe(None).live;
        for i in 1..=10 {
            hub.publish(turn_started(i));
        }
        match slow.recv().await {
            Err(broadcast::error::RecvError::Lagged(skipped)) => assert!(skipped >= 1),
            other => panic!("expected lag, got {other:?}"),
        }
        let recovered = hub.subscribe(Some(hub.last_event_id()));
        assert!(matches!(recovered.replay, ReplayKind::Delta { .. }));
    }

    #[test]
    fn snapshot_is_bounded() {
        let mut hub = EventHub::new(MAX_SNAPSHOT_FRAMES + 10, 8);
        for i in 0..(MAX_SNAPSHOT_FRAMES + 10) {
            hub.publish(turn_started(u64::try_from(i).unwrap_or(1)));
        }
        assert_eq!(hub.snapshot().len(), MAX_SNAPSHOT_FRAMES);
    }
}
