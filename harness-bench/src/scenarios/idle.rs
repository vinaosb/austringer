//! Idle scenario: the todo-36 idle proof as a gate — 100k no-op events
//! must produce ZERO additional renders (idle CPU 0.0%) in bounded wall
//! time. Deterministic render count; wall is advisory.

use std::time::Instant;

use harness_daemon::protocol::ServerMsg;
use harness_tui::app::{App, DEFAULT_CONTEXT_WINDOW, UiEvent};
use harness_tui::keys::Key;

use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;

pub const EVENTS: u64 = 100_000;

fn unbound_key() -> Key {
    Key {
        code: crossterm::event::KeyCode::F(24),
        mods: crossterm::event::KeyModifiers::NONE,
    }
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
    app.rendered(); // baseline render; anything above 1 is idle work
    let started = Instant::now();
    for _ in 0..EVENTS {
        let _ = app.step(UiEvent::Key(unbound_key()));
        let _ = app.step(UiEvent::Server(ServerMsg::Pong));
    }
    let elapsed = started.elapsed();
    let additional = app.renders() - 1;
    let mut out = ScenarioOutput::new();
    out.push(
        "idle.renders",
        additional as f64,
        "renders",
        Policy::Budget {
            budget: ctx.budgets.idle_max_renders as f64,
        },
    );
    out.push(
        "idle.drain_ms",
        elapsed.as_secs_f64() * 1000.0,
        "ms",
        Policy::Advisory,
    );
    if additional > ctx.budgets.idle_max_renders {
        return Err(ScenarioError::Failed(format!(
            "no-op events performed {additional} renders — hidden idle work"
        )));
    }
    Ok(out)
}
