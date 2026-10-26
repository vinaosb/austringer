//! Human-mode rendering: streaming text lines for every server
//! message and event, including the ask renderers (permission asks and
//! resolutions, fallback and plan states) — the todo-22-named
//! extraction from the render facade.

use austringer_core::event::{ContentBlock, Event};
use austringer_daemon::protocol::{AskPayload, PlanOutcome, Resolution, ServerMsg, SessionEvent};

/// Human-mode line for a server message; `None` prints nothing.
#[must_use]
pub fn human(msg: &ServerMsg) -> Option<String> {
    match msg {
        ServerMsg::Event { frame } => human_event(&frame.event),
        ServerMsg::PermissionAsk { ask } => Some(human_ask(ask)),
        ServerMsg::PermissionResolved { resolution, .. } => Some(human_resolution(resolution)),
        ServerMsg::StateResyncRequired { last_event_id } => Some(format!(
            "— resync: state diverged, replaying from {last_event_id} —"
        )),
        ServerMsg::Snapshot { .. } => Some("— state replayed —".to_owned()),
        ServerMsg::Bye { reason } => Some(format!("— daemon closed the stream: {reason} —")),
        ServerMsg::HelloAck { .. } | ServerMsg::Pong => None,
        ServerMsg::Error { code, message } => Some(format!("daemon error ({code:?}): {message}")),
        ServerMsg::MediaResult {
            producer,
            diff_summary,
            ..
        } => Some(format!("media ({producer}): {diff_summary}")),
        ServerMsg::Usage {
            model,
            input_tokens,
            output_tokens,
            ..
        } => Some(format!(
            "usage: {} — {input_tokens} in / {output_tokens} out",
            model.model
        )),
        ServerMsg::FallbackAsk { ask } => Some(format!(
            "model fallback proposed: {} -> {} — {} (answer over WS/IPC)",
            ask.current.model, ask.proposed.model, ask.reason
        )),
        ServerMsg::FallbackResolved { ask_id, chosen } => Some(format!(
            "fallback {ask_id} resolved: next model {}",
            chosen.model
        )),
        ServerMsg::Verdict {
            turn,
            verdict,
            evidence,
        } => Some(format!(
            "turn {turn} verdict: {} ({} evidence events)",
            verdict.as_str(),
            evidence.len()
        )),
        ServerMsg::PlanAsk { ask } => Some(format!(
            "plan {} (revision {}) awaiting approval — approve, edit, or reject over WS/IPC",
            ask.plan_id, ask.revision
        )),
        ServerMsg::PlanResolved {
            ask_id,
            plan_id,
            outcome,
        } => Some(plan_resolution_line(*ask_id, *plan_id, outcome)),
    }
}

fn plan_resolution_line(ask_id: u64, plan_id: u64, outcome: &PlanOutcome) -> String {
    match outcome {
        PlanOutcome::Approved { by } => {
            format!("plan ask {ask_id}: plan {plan_id} approved by {by}; writes re-armed")
        }
        PlanOutcome::Rejected { by } => {
            format!("plan ask {ask_id}: plan {plan_id} rejected by {by}; still gated")
        }
        PlanOutcome::Edited { by, .. } => format!(
            "plan ask {ask_id}: plan {plan_id} edited by {by}; re-gated on the new revision"
        ),
        PlanOutcome::Cancelled { by } => {
            format!("plan ask {ask_id}: withdrawn by {by}; plan {plan_id} stays pending")
        }
        PlanOutcome::Timeout => format!(
            "plan ask {ask_id} timed out; plan {plan_id} stays pending (never auto-approved)"
        ),
    }
}

fn human_event(event: &SessionEvent) -> Option<String> {
    match event {
        SessionEvent::Recorded { event, .. } => human_core_event(event),
        SessionEvent::TurnStarted { turn } => Some(format!("— turn {turn} started —")),
        SessionEvent::TurnFinished { turn, outcome } => {
            Some(format!("— turn {turn} finished: {outcome} —"))
        }
        SessionEvent::Diff { path, .. } => Some(format!("diff: {path}")),
        SessionEvent::Notification { message, .. } => Some(format!("notice: {message}")),
        SessionEvent::ChildStarted {
            child, task, role, ..
        } => Some(format!("child started: {child} ({task}, {role})")),
        SessionEvent::ChildProgress {
            child,
            phase,
            detail,
        } => Some(format!("child {child} {phase}: {detail}")),
        SessionEvent::ChildFinished {
            child,
            task,
            state,
            detail,
        } => Some(format!("child {child} ({task}) finished {state}: {detail}")),
        SessionEvent::CostWarning {
            consumed_micros,
            cap_micros,
        } => {
            let pct = if *cap_micros == 0 {
                100
            } else {
                u32::try_from(consumed_micros.saturating_mul(100) / cap_micros).unwrap_or(100)
            };
            Some(format!(
                "cost warning: {pct}% of session cap ({consumed_micros} of {cap_micros} micro-USD)"
            ))
        }
        SessionEvent::CostStop {
            consumed_micros,
            cap_micros,
        } => Some(format!(
            "cost stop: session cap reached ({consumed_micros} of {cap_micros} micro-USD) — further turns refused"
        )),
        SessionEvent::TeamBudget {
            spawned,
            tokens_spent,
            fan_out_cap,
            token_budget,
        } => Some(format!(
            "agents budget: {spawned}/{fan_out_cap} children, {tokens_spent}/{token_budget} tokens"
        )),
        SessionEvent::TeamBudgetExhausted { spent, budget } => Some(format!(
            "agents budget exhausted: {spent} of {budget} tokens — further task spawns refused"
        )),
        SessionEvent::PlanModeEntered { plan } => Some(format!(
            "plan mode entered (plan {plan}); writes gated until approval"
        )),
        SessionEvent::PlanModeExited { plan } => {
            Some(format!("plan mode exited without approval (plan {plan})"))
        }
        SessionEvent::PlanBlocked { plan, tool, target } => Some(match target {
            Some(target) => {
                format!("plan {plan} pending approval: '{tool}' on '{target}' blocked")
            }
            None => format!("plan {plan} pending approval: '{tool}' blocked"),
        }),
        SessionEvent::PlanUpdated { plan, revision } => Some(format!(
            "plan {plan} edited (revision {revision}); writes re-gated"
        )),
        SessionEvent::PlanApproved { plan } => {
            Some(format!("plan {plan} approved; writes re-armed"))
        }
        SessionEvent::PlanRejected { plan } => {
            Some(format!("plan {plan} rejected; plan mode stays active"))
        }
    }
}

fn human_core_event(event: &Event) -> Option<String> {
    match event {
        Event::User { blocks } => Some(format!("you: {}", blocks_text(blocks))),
        Event::Assistant { blocks } => Some(format!("assistant: {}", blocks_text(blocks))),
        Event::ToolCall { call_id, tool, .. } => Some(format!("tool call {call_id}: {tool}")),
        Event::ToolResult { call_id, blocks } => {
            Some(format!("tool result {call_id}: {}", blocks_text(blocks)))
        }
        Event::System { text } => Some(format!("system: {text}")),
        Event::Condensation {
            summary,
            through_seq,
        } => Some(format!(
            "~ condensed through seq {} — {}",
            through_seq.0,
            summary.lines().next().unwrap_or_default()
        )),
        Event::Snapshot { .. } => None,
    }
}

fn blocks_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .map(ContentBlock::text_marker)
        .collect::<Vec<_>>()
        .join("\n")
}

fn human_ask(ask: &AskPayload) -> String {
    format!(
        "permission requested: {} — {} [y]allow once [s]allow session [n]deny",
        ask.tool, ask.reason
    )
}

fn human_resolution(resolution: &Resolution) -> String {
    match resolution {
        Resolution::Allowed { by } => format!("permission allowed by {by}"),
        Resolution::Denied { by } => format!("permission denied by {by}"),
        Resolution::Cancelled { by } => format!("permission cancelled by {by}"),
        Resolution::Timeout => "permission ask timed out".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ServerMsg, human};
    use austringer_core::event::{ContentBlock, Event};
    use austringer_daemon::protocol::{
        AskPayload, Frame, PlanAskPayload, PlanOutcome, Resolution, SessionEvent,
    };

    fn user_frame(id: u64, text: &str) -> Frame {
        Frame {
            id,
            event: SessionEvent::Recorded {
                seq: id,
                event: Event::User {
                    blocks: vec![ContentBlock::Text {
                        text: text.to_owned(),
                    }],
                },
            },
        }
    }

    #[test]
    fn human_lines_render_agents_budget_and_state_replayed_copy() {
        let budget = super::human_event(&SessionEvent::TeamBudget {
            spawned: 2,
            tokens_spent: 50,
            fan_out_cap: 4,
            token_budget: 1000,
        })
        .unwrap_or_default();
        assert!(budget.starts_with("agents budget:"), "{budget}");
        let exhausted = super::human_event(&SessionEvent::TeamBudgetExhausted {
            spent: 1000,
            budget: 1000,
        })
        .unwrap_or_default();
        assert!(
            exhausted.starts_with("agents budget exhausted"),
            "{exhausted}"
        );
        let replayed = human(&ServerMsg::Snapshot { frames: Vec::new() }).unwrap_or_default();
        assert_eq!(replayed, "— state replayed —");
    }

    #[test]
    fn human_lines_render_team_budget_events() {
        let budget = super::human_event(&SessionEvent::TeamBudget {
            spawned: 2,
            tokens_spent: 50,
            fan_out_cap: 4,
            token_budget: 1000,
        })
        .unwrap_or_default();
        assert!(budget.contains("2/4"), "{budget}");
        assert!(budget.contains("50/1000"), "{budget}");
        let exhausted = super::human_event(&SessionEvent::TeamBudgetExhausted {
            spent: 1000,
            budget: 1000,
        })
        .unwrap_or_default();
        assert!(exhausted.contains("exhausted"), "{exhausted}");
        assert!(exhausted.contains("spawns"), "{exhausted}");
    }

    #[test]
    fn human_lines_carry_the_pinned_keywords() {
        let ask = human(&ServerMsg::PermissionAsk {
            ask: AskPayload {
                ask_id: 1,
                tool: "bash".to_owned(),
                target: None,
                reason: "runs a command".to_owned(),
                deadline_ms: 1000,
            },
        })
        .unwrap_or_default();
        assert!(ask.contains("permission"), "{ask}");
        let denied = human(&ServerMsg::PermissionResolved {
            ask_id: 1,
            resolution: Resolution::Denied {
                by: "cli".to_owned(),
            },
        })
        .unwrap_or_default();
        assert!(denied.contains("denied"), "{denied}");
        let finished = super::human_event(&SessionEvent::TurnFinished {
            turn: 1,
            outcome: "task_completed".to_owned(),
        })
        .unwrap_or_default();
        assert!(finished.contains("task_completed"), "{finished}");
        let user = human(&ServerMsg::Event {
            frame: user_frame(1, "say something"),
        })
        .unwrap_or_default();
        assert!(user.contains("say something"), "{user}");
    }

    #[test]
    fn condensation_renders_a_condensed_line() {
        let frame = Frame {
            id: 5,
            event: SessionEvent::Recorded {
                seq: 5,
                event: Event::Condensation {
                    summary: "early history folded into a summary".to_owned(),
                    through_seq: austringer_core::event::Sequence(4),
                },
            },
        };
        let line = human(&ServerMsg::Event { frame }).unwrap_or_default();
        assert!(line.contains("~ condensed"), "{line}");
        assert!(line.contains("seq 4"), "{line}");
        assert!(
            line.contains("early history folded"),
            "summary must be visible: {line}"
        );
    }

    #[test]
    fn human_render_shows_fallback_and_plan_approval_states() {
        let fallback: ServerMsg = serde_json::from_value(serde_json::json!({
            "type": "fallback_ask",
            "ask": {
                "ask_id": 7,
                "current": { "provider": "mock", "model": "big" },
                "proposed": { "provider": "mock", "model": "small" },
                "reason": "cap pressure",
                "deadline_ms": 5000
            }
        }))
        .unwrap();
        let line = human(&fallback).unwrap_or_default();
        assert!(line.contains("fallback"), "{line}");
        assert!(line.contains("big"), "{line}");
        assert!(line.contains("small"), "{line}");
        assert!(line.contains("cap pressure"), "{line}");
        let plan = human(&ServerMsg::PlanAsk {
            ask: PlanAskPayload {
                ask_id: 1,
                plan_id: 2,
                revision: 1,
                artifact: "- do the thing".to_owned(),
                deadline_ms: 1000,
            },
        })
        .unwrap_or_default();
        assert!(plan.contains("awaiting approval"), "{plan}");
        let resolved = human(&ServerMsg::PlanResolved {
            ask_id: 1,
            plan_id: 2,
            outcome: PlanOutcome::Approved {
                by: "tui".to_owned(),
            },
        })
        .unwrap_or_default();
        assert!(resolved.contains("approved"), "{resolved}");
    }
}
