//! The shared hook handle: one `Arc<Mutex<HookManager>>` behind every
//! pipeline surface (entry, approval, execution, stop, compaction) so
//! journal, trust, and loop-budget state stay coherent. Locking is
//! stage-scoped — stages run sequentially in the pipeline, so no two
//! surfaces contend.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::Mutex;

use super::HookEvent;
use super::manager::HookManager;
use crate::agent_loop::stages::{EntryHook, Flow, StageCtx, StageError};

/// Cloneable shared handle over the [`HookManager`].
#[derive(Clone)]
pub struct SharedHooks {
    manager: Arc<Mutex<HookManager>>,
}

impl SharedHooks {
    /// Wraps a manager.
    #[must_use]
    pub fn new(manager: HookManager) -> Self {
        Self {
            manager: Arc::new(Mutex::new(manager)),
        }
    }

    /// Direct access for assertions and embedder-side firings.
    #[must_use]
    pub fn inner(&self) -> &Arc<Mutex<HookManager>> {
        &self.manager
    }

    /// The entry stage: SessionStart (once) + UserPromptSubmit
    /// augmentation.
    #[must_use]
    pub fn entry_stage(&self) -> HookedEntry {
        HookedEntry {
            manager: self.clone(),
        }
    }

    /// The approval stage wrapper: PreToolUse gating around the T7
    /// chain (see [`super::gates`]).
    #[must_use]
    pub fn approval_stage(
        &self,
        inner: Box<dyn crate::agent_loop::stages::ToolApproval>,
    ) -> super::gates::HookedApproval {
        super::gates::HookedApproval {
            manager: self.clone(),
            inner,
        }
    }

    /// The execution stage wrapper: PostToolUse replacement/redaction
    /// and PostToolUseFailure.
    #[must_use]
    pub fn executor_stage(
        &self,
        inner: Box<dyn crate::agent_loop::stages::ToolExecutor>,
    ) -> super::gates::HookedExecutor {
        super::gates::HookedExecutor {
            manager: self.clone(),
            inner,
        }
    }

    /// The stop stage: Stop block-until, bounded by the manager's loop
    /// budget.
    #[must_use]
    pub fn stop_stage(&self) -> super::stop::HookedStop {
        super::stop::HookedStop {
            manager: self.clone(),
        }
    }

    /// The compaction wrapper: PreCompact/PostCompact around a
    /// signaling compaction source (fires around actual compactions
    /// only; see [`super::stop::SignalingCompactor`]).
    #[must_use]
    pub fn compactor_stage(
        &self,
        inner: Box<dyn super::stop::SignalingCompactor>,
    ) -> super::stop::HookedCompactor {
        super::stop::HookedCompactor {
            manager: self.clone(),
            inner,
        }
    }

    /// Fires Notification hooks.
    pub async fn notify(&self, message: &str) {
        self.manager.lock().await.notify(message).await;
    }

    /// Fires SessionEnd hooks.
    pub async fn fire_session_end(&self) {
        self.manager.lock().await.session_end().await;
    }

    /// Fires SubagentStart hooks.
    pub async fn subagent_start(&self, agent: &str) {
        self.manager.lock().await.subagent_start(agent).await;
    }

    /// Fires SubagentStop hooks.
    pub async fn subagent_stop(&self, agent: &str) {
        self.manager.lock().await.subagent_stop(agent).await;
    }
}

/// The real EntryHook implementation: fires SessionStart on the first
/// pipeline iteration, then UserPromptSubmit hooks over each queued
/// non-command prompt, rewriting the queue entry with any (budget-
/// capped) augmentation so the Steer stage records the enriched prompt.
pub struct HookedEntry {
    manager: SharedHooks,
}

#[async_trait]
impl EntryHook for HookedEntry {
    async fn on_entry(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        let mut manager = self.manager.manager.lock().await;
        if manager.trip_start() {
            manager
                .fire(HookEvent::SessionStart, json!({"event": "session_start"}))
                .await;
        }
        let queued: Vec<String> = ctx.steer_queue.iter().cloned().collect();
        for (index, text) in queued.iter().enumerate() {
            if text.starts_with('/') {
                continue;
            }
            let payload = json!({"event": "user_prompt_submit", "prompt": text});
            let fired = manager.fire(HookEvent::UserPromptSubmit, payload).await;
            let augments: Vec<String> = fired
                .iter()
                .filter_map(|firing| match &firing.outcome {
                    super::HookOutcome::Succeeded(super::HookOpinion::Augment(text)) => {
                        Some(manager.cap_augmentation(text))
                    }
                    _ => None,
                })
                .collect();
            if !augments.is_empty() {
                ctx.steer_queue[index] = format!("{text}\n\n[hooks]\n{}", augments.join("\n"));
            }
        }
        Ok(Flow::Continue)
    }
}
