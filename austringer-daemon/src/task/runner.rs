//! The delegation run path: guards, the task-8 child-session spawn, and
//! the budget bookkeeping around one child's turn (the todo-13
//! mechanical split of the services facade — `TaskServices` itself
//! lives in [`super`]).

use std::sync::Arc;

use austringer_intel::routing::RouteRequest;
use austringer_orchestration::budget::BudgetBreach;
use austringer_orchestration::fork::DepthGuard;

use super::{
    ChildObserved, CuratedAgent, DelegationBudget, DelegationSnapshot, SessionHost, TASK_ROLE,
    TaskRefusal, TaskRequest, TaskServices, await_child_turn, curated, depth_of,
};
use crate::attach::HostCommand;
use crate::protocol::SessionEvent;

impl TaskServices {
    /// Runs one delegation child to its outcome: guards first (depth,
    /// category route, budget), then the task-8 child-session pattern —
    /// fresh host + registry + engine, one turn over the prompt routed
    /// under the category, Child* events on the parent hub, and the
    /// token spend folded into the parent's aggregate. Failures of the
    /// child TURN are outcomes, not refusals — the caller surfaces why.
    ///
    /// # Errors
    /// [`TaskRefusal`] for every pre-spawn refusal and spawn failure.
    pub async fn run_child(
        &self,
        parent: &Arc<SessionHost>,
        request: TaskRequest,
    ) -> Result<super::TaskChildOutcome, TaskRefusal> {
        DepthGuard::default().check(depth_of(parent.session_id()) + 1)?;
        let curated = curated(&request.category);
        let resolution = self.intake.table.resolve(match &curated {
            Some(agent) => RouteRequest::Role(agent.role),
            None => RouteRequest::Category(request.category.clone()),
        })?;
        let ledger = self.ledger_for(parent.session_id());
        let authorized = ledger
            .lock()
            .map_err(|_| TaskRefusal::ParentGone)?
            .authorize(request.escalate_fan_out);
        let ordinal = match authorized {
            Ok(ordinal) => ordinal,
            // The typed halt event (todo 15): every attached client
            // sees WHY spawns stopped, not just the refusing tool
            // result.
            Err(TaskRefusal::Tokens(BudgetBreach::TokensExhausted { spent, budget })) => {
                parent.publish(SessionEvent::TeamBudgetExhausted { spent, budget });
                return Err(BudgetBreach::TokensExhausted { spent, budget }.into());
            }
            Err(other) => return Err(other),
        };
        if let Ok(budget) = ledger.lock() {
            publish_budget(parent, &budget);
        }
        let spawn = ChildSpawn {
            child_id: format!("{}-child-task{ordinal}", parent.session_id()),
            model: resolution.model.model,
            curated,
        };
        let spawned = self.spawn_and_run(parent, &request, &spawn).await;
        let finished = match &spawned {
            Ok((observed, _)) if observed.succeeded() => {
                ("success".to_owned(), observed.summary.clone())
            }
            Ok((observed, _)) => ("failed".to_owned(), observed.outcome.clone()),
            Err(refusal) => ("failed".to_owned(), refusal.to_string()),
        };
        parent.publish(SessionEvent::ChildFinished {
            child: spawn.child_id.clone(),
            task: request.category.clone(),
            state: finished.0,
            detail: finished.1,
        });
        match spawned {
            Ok((observed, child)) => {
                if let Ok(mut budget) = ledger.lock() {
                    let _ = budget.record_spend(observed.tokens);
                    publish_budget(parent, &budget);
                }
                Ok(super::TaskChildOutcome {
                    child_id: spawn.child_id,
                    outcome: observed.outcome,
                    summary: observed.summary,
                    tokens: observed.tokens,
                    usage: observed.usage,
                    child,
                })
            }
            Err(refusal) => Err(refusal),
        }
    }

    /// The task-8 child-session spawn: fresh host, per-child registry,
    /// category- or curated-role-routed inference (the child's spend
    /// books into the PARENT ledger — the todo-33 aggregation
    /// contract), submit, and the bounded turn await. Curated agents
    /// (todo 14) run under the read-only approval narrowing.
    async fn spawn_and_run(
        &self,
        parent: &Arc<SessionHost>,
        request: &TaskRequest,
        spawn: &ChildSpawn,
    ) -> Result<(ChildObserved, Arc<SessionHost>), TaskRefusal> {
        let session_dir = self.launch.sessions_root.join(&spawn.child_id);
        std::fs::create_dir_all(&session_dir)
            .map_err(|e| TaskRefusal::Spawn(format!("session dir: {e}")))?;
        let (child, cmd_rx) = SessionHost::new(
            &spawn.child_id,
            super::CHILD_HUB_CAPACITY,
            super::CHILD_HUB_CAPACITY,
            2,
            self.launch.ask_policy.clone(),
            self.launch.ask_timeout,
            None,
        );
        let (executor, wire_tools) =
            (self.launch.executor_factory)(&session_dir, child.event_sink());
        let crate::usage::ComposedInference {
            inference,
            identity,
        } = (self.launch.inference_factory)();
        let route = match &spawn.curated {
            Some(agent) => RouteRequest::Role(agent.role),
            None => RouteRequest::Category(request.category.clone()),
        };
        let inference = match (identity, parent.cost_ledger()) {
            (Some(identity), Some(ledger)) => Box::new(
                crate::routing::SessionRouter::new(
                    Arc::clone(&self.intake),
                    &spawn.child_id,
                    Arc::clone(&child),
                    ledger,
                    identity,
                )
                .under_request(route),
            )
                as Box<dyn austringer_core::agent_loop::Inference>,
            // Identity-less factories (scripted stacks) or unledgered
            // parents run the factory stack unrouted — honest absence.
            _ => inference,
        };
        let stages = match &spawn.curated {
            Some(agent) => child.readonly_stages(executor, &agent.name, agent.tools.clone()),
            None => child.standard_stages(executor),
        };
        // Children ride the same wire surfaces as parents: the stable
        // system prompt plus their own registry's pinned core schemas.
        let loop_config = austringer_core::agent_loop::LoopConfig {
            system: crate::engine::SESSION_SYSTEM_PROMPT.to_owned(),
            tools: wire_tools,
            ..austringer_core::agent_loop::LoopConfig::default()
        };
        child
            .spawn_engine_with_config(session_dir, stages, inference, cmd_rx, loop_config)
            .map_err(|e| TaskRefusal::Spawn(format!("engine spawn: {e}")))?;
        parent.publish(SessionEvent::ChildStarted {
            child: spawn.child_id.clone(),
            task: request.category.clone(),
            role: spawn
                .curated
                .as_ref()
                .map_or_else(|| TASK_ROLE.to_owned(), |agent| agent.name.clone()),
            model: spawn.model.clone(),
        });
        let mut live = child.hub_subscribe();
        // Subscribed BEFORE the submit so no mid-turn ask can slip
        // between the submit and the await (todo 15's blocked-child
        // surfacing reads this lane).
        let mut control = child.mediator().subscribe();
        child
            .commands()
            .send(HostCommand::Submit(request.prompt.clone()))
            .await
            .map_err(|e| TaskRefusal::Spawn(format!("prompt submit: {e}")))?;
        let observed = await_child_turn(
            parent,
            &child,
            &mut live,
            &mut control,
            super::CHILD_TURN_TIMEOUT,
        )
        .await;
        observed.map(|observed| (observed, child))
    }
}

/// Publishes the current delegation-budget snapshot as a team event
/// (the `DelegationSnapshot` fields; todo 15's budget surfacing).
fn publish_budget(parent: &Arc<SessionHost>, budget: &DelegationBudget) {
    let DelegationSnapshot {
        spawned,
        tokens_spent,
        fan_out_cap,
        token_budget,
    } = budget.snapshot();
    parent.publish(SessionEvent::TeamBudget {
        spawned,
        tokens_spent,
        fan_out_cap,
        token_budget,
    });
}

/// One authorized child placement (the spawn plan `run_child` hands the
/// spawn path): identity plus, for curated agents (todo 14), the
/// read-only dispatch.
pub(super) struct ChildSpawn {
    /// Child session id (`{parent}-child-task{ordinal}`).
    pub(super) child_id: String,
    /// The resolved model (rides the ChildStarted event).
    pub(super) model: String,
    /// The curated-agent dispatch, when the task() name was a reserved
    /// curated key (None = a plain user category).
    pub(super) curated: Option<CuratedAgent>,
}
