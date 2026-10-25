//! Scripted SSE response bodies: ordered steps with optional delays.

use std::time::Duration;

use super::event::SseEvent;

/// One step of a scripted stream: an event or an idle gap before the next step.
#[derive(Clone, Debug)]
pub enum SseStep {
    /// Emit one event.
    Event(SseEvent),
    /// Idle gap before whatever follows (virtual time under `start_paused`).
    Delay(Duration),
}

/// An ordered script of SSE steps — the scripted body of one provider reply.
#[derive(Clone, Debug, Default)]
pub struct SseScript {
    /// Steps in emission order.
    pub steps: Vec<SseStep>,
}

impl SseScript {
    /// Minimal happy-path reply: `message_start`, one text delta, `message_stop`,
    /// `done`.
    pub fn single_text(text: &str) -> Self {
        SseScriptBuilder::new()
            .event(super::event::ev_message_start())
            .event(super::event::ev_text_delta(text))
            .event(super::event::ev_message_stop())
            .event(super::event::ev_done())
            .build()
    }

    /// Script from pre-built events, with no delays.
    pub fn from_events(events: impl IntoIterator<Item = SseEvent>) -> Self {
        Self {
            steps: events.into_iter().map(SseStep::Event).collect(),
        }
    }

    /// Event-only view of the script (delays dropped); used by cassettes, which
    /// record content, not timing.
    pub fn events(&self) -> Vec<SseEvent> {
        self.steps
            .iter()
            .filter_map(|s| match s {
                SseStep::Event(ev) => Some(ev.clone()),
                SseStep::Delay(_) => None,
            })
            .collect()
    }
}

/// Builder for [`SseScript`]: append events and delays in order, then finalize.
#[derive(Clone, Debug, Default)]
pub struct SseScriptBuilder {
    steps: Vec<SseStep>,
}

impl SseScriptBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one event step.
    pub fn event(mut self, ev: SseEvent) -> Self {
        self.steps.push(SseStep::Event(ev));
        self
    }

    /// Appends one idle gap (applies before the next step's bytes are yielded).
    pub fn delay(mut self, gap: Duration) -> Self {
        self.steps.push(SseStep::Delay(gap));
        self
    }

    /// Finalizes the script, repeating the accumulated steps up to `n` times
    /// (`n = 3` with one delta step yields that delta three times). `n = 0`
    /// produces an empty script.
    pub fn up_to_n_times(self, n: u32) -> SseScript {
        SseScript {
            steps: (0..n).flat_map(|_| self.steps.iter().cloned()).collect(),
        }
    }

    /// Finalizes the script as-is.
    pub fn build(self) -> SseScript {
        SseScript { steps: self.steps }
    }
}
