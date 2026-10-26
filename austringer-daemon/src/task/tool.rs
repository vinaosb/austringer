//! The model-facing `task` tool: `task(category, prompt)` delegates to
//! a child session on the category's route. Refusals surface as typed
//! tool-result text (the session continues); success returns the child
//! summary CLAMPED at the delegation role's budget and labeled as
//! child output.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use austringer_core::tools::{ProviderError, ToolSchema};
use austringer_orchestration::telemetry::clamp_summary;

use super::{TaskRefusal, TaskRequest, TaskServices, session_id_of, summary_budget_for};

/// The model-facing delegation tool (one per session registry).
pub struct TaskTool {
    services: Arc<TaskServices>,
    session_id: String,
}

impl TaskTool {
    /// Binds the tool to one session's directory and the daemon's
    /// delegation services.
    #[must_use]
    pub fn new(session_dir: &Path, services: Arc<TaskServices>) -> Self {
        Self {
            services,
            session_id: session_id_of(session_dir),
        }
    }

    fn refusal_text(&self, refusal: TaskRefusal) -> String {
        format!("(task refused: {refusal})")
    }

    /// Depth is checked BEFORE the parent resolves: a child session
    /// invoking task must see the DepthExceeded vocabulary, not a
    /// missing-parent error (its host is never in the resolver's map).
    fn depth_refusal(&self) -> Option<String> {
        austringer_orchestration::fork::DepthGuard::default()
            .check(super::depth_of(&self.session_id) + 1)
            .err()
            .map(|exceeded| self.refusal_text(exceeded.into()))
    }

    async fn result_text(&self, arguments: &Value) -> Result<String, ProviderError> {
        if let Some(refusal) = self.depth_refusal() {
            return Err(ProviderError::Execution { message: refusal });
        }
        let category = crate::tools::providers::str_arg(arguments, "category")?;
        let prompt = crate::tools::providers::str_arg(arguments, "prompt")?;
        let escalate = match arguments.get("escalate_fan_out") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(flag)) => *flag,
            Some(_) => {
                return Err(ProviderError::InvalidArguments {
                    message: "'escalate_fan_out' must be a boolean".to_owned(),
                });
            }
        };
        let parent = (self.services.parent_resolver())(&self.session_id).ok_or_else(|| {
            ProviderError::Execution {
                message: self.refusal_text(TaskRefusal::ParentGone),
            }
        })?;
        let outcome = self
            .services
            .run_child(
                &parent,
                TaskRequest {
                    category: category.to_owned(),
                    prompt: prompt.to_owned(),
                    escalate_fan_out: escalate,
                },
            )
            .await
            .map_err(|refusal| ProviderError::Execution {
                message: self.refusal_text(refusal),
            })?;
        let label = format!("[task child {} category={category}]", outcome.child_id);
        if outcome.succeeded() {
            Ok(format!(
                "{label} {}",
                clamp_summary(&outcome.summary, summary_budget_for(category))
            ))
        } else {
            Ok(format!(
                "{label} child failed: {} {}",
                clamp_summary(&outcome.outcome, summary_budget_for(category)),
                clamp_summary(&outcome.summary, summary_budget_for(category))
            ))
        }
    }
}

#[async_trait]
impl austringer_core::tools::ToolProvider for TaskTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "task".into(),
            description: "Delegate one bounded subtask to a child session on a configured \
                          delegation category (routing.json categories[]: each pins the model + \
                          effort). Two curated agents ride the same vocabulary as reserved \
                          names — explore (fast read-only codebase search) and librarian \
                          (read-only docs/OSS search) — which dispatch on their routing.json \
                          ROLES entries, are pinned to the cheap pool by the utility-role \
                          validation, and run under read-only tool allowlists; user categories \
                          cannot shadow them. Returns the child's summary, clamped and labeled \
                          as child output. Fan-out is capped per session (default 4); pass \
                          escalate_fan_out=true only when the cap genuinely blocks the work. \
                          Children may not delegate further."
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Delegation category (a routing.json categories[] key)."
                    },
                    "prompt": {
                        "type": "string",
                        "minLength": 1,
                        "description": "The child's complete objective."
                    },
                    "escalate_fan_out": {
                        "type": "boolean",
                        "default": false,
                        "description": "Explicit fan-out escalation past the session cap \
                                        (hard ceiling still applies)."
                    }
                },
                "required": ["category", "prompt"]
            }),
        }
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        Ok(self.result_text(arguments).await?.into_bytes())
    }
}
