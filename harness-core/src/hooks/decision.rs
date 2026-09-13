//! Hook decisions: the typed gate/observation vocabulary, and the
//! untrusted-output parse target. Hook stdout/bodies are UNTRUSTED:
//! they are parsed leniently into [`HookResponse`] (unknown fields
//! ignored) and then converted to an action with typed errors for
//! malformed combinations.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a PreToolUse hook decided about one tool call (Amp gate
/// actions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateAction {
    /// No objection (the T7 chain still gets the final word — hooks
    /// cannot bypass permissions).
    Allow,
    /// Reject-and-continue: the call is denied with `reason` as model
    /// feedback.
    Reject {
        /// Model-facing refusal reason.
        reason: String,
    },
    /// Replace the call's tool/arguments; the MODIFIED call passes back
    /// through the inspector chain before execution.
    Modify {
        /// Replacement tool name.
        tool: String,
        /// Replacement arguments.
        arguments: Value,
    },
    /// Synthesize the tool RESULT: the call never executes; `text`
    /// becomes the result (threat-scanned and budget-capped).
    Synthesize {
        /// The synthesized result text.
        text: String,
    },
}

/// What a PostToolUse hook decided about a finished tool's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostToolAction {
    /// Leave the output as-is.
    Keep,
    /// Replace (rewrite/redact) the output with `text` (threat-scanned
    /// and budget-capped before it enters context).
    Replace {
        /// The replacement text.
        text: String,
    },
}

/// What a Stop hook decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopAction {
    /// Let the turn exit.
    Proceed,
    /// Block the exit, forcing another inference round with `reason`.
    Block {
        /// Why the model must continue.
        reason: String,
    },
}

/// What one successful hook run concluded, in the vocabulary of the
/// event it fired on.
#[derive(Debug, Clone, PartialEq)]
pub enum HookOpinion {
    /// The hook ran but expressed no opinion (empty output).
    None,
    /// A tool-gate opinion (PreToolUse).
    Gate(GateAction),
    /// An output opinion (PostToolUse / PostToolUseFailure).
    PostTool(PostToolAction),
    /// A stop opinion (Stop).
    Stop(StopAction),
    /// Prompt-augmentation text (UserPromptSubmit).
    Augment(String),
}

/// Why an untrusted hook output could not become an action.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HookParseError {
    /// The output was not parseable JSON.
    #[error("hook output is not valid JSON: {detail}")]
    NotJson {
        /// What the parser said.
        detail: String,
    },
    /// The decision string is not in the vocabulary.
    #[error("unknown hook decision {raw:?}")]
    UnknownDecision {
        /// The unrecognized decision string.
        raw: String,
    },
    /// The decision requires a field the output did not carry.
    #[error("hook decision {decision:?} requires field {field:?}")]
    MissingField {
        /// The decision that needed the field.
        decision: String,
        /// The absent field name.
        field: &'static str,
    },
}

/// The lenient parse target for untrusted hook outputs (shell stdout,
/// HTTP bodies, MCP tool results). Unknown fields are ignored; missing
/// fields surface as typed errors during action conversion.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HookResponse {
    /// Decision verb: `allow`, `reject`, `modify`, `synthesize`,
    /// `keep`, `replace`, `block`, `proceed`.
    pub decision: Option<String>,
    /// Human-facing reason (rejections/blocks).
    pub reason: Option<String>,
    /// Replacement tool name (`modify`).
    pub tool: Option<String>,
    /// Replacement arguments (`modify`).
    pub arguments: Option<Value>,
    /// Replacement/ synthesized text (`synthesize`, `replace`).
    pub text: Option<String>,
    /// Extra context appended to a user prompt (`user_prompt_submit`).
    pub additional_context: Option<String>,
}

impl HookResponse {
    /// Parses untrusted text; empty/whitespace output is `None` (the
    /// hook expressed no opinion).
    pub fn parse(raw: &str) -> Result<Option<Self>, HookParseError> {
        if raw.trim().is_empty() {
            return Ok(None);
        }
        serde_json::from_str(raw)
            .map(Some)
            .map_err(|error| HookParseError::NotJson {
                detail: error.to_string(),
            })
    }

    /// Converts to a tool-gate action.
    pub fn to_gate_action(&self) -> Result<GateAction, HookParseError> {
        let decision = self.decision.as_ref().map(|d| d.to_ascii_lowercase());
        match decision.as_deref() {
            None | Some("allow") | Some("proceed") => Ok(GateAction::Allow),
            Some("reject") | Some("deny") => Ok(GateAction::Reject {
                reason: self
                    .reason
                    .clone()
                    .unwrap_or_else(|| "rejected by hook".to_owned()),
            }),
            Some("modify") => match &self.tool {
                Some(tool) => Ok(GateAction::Modify {
                    tool: tool.clone(),
                    arguments: self.arguments.clone().unwrap_or(Value::Null),
                }),
                None => Err(HookParseError::MissingField {
                    decision: "modify".to_owned(),
                    field: "tool",
                }),
            },
            Some("synthesize") => match &self.text {
                Some(text) => Ok(GateAction::Synthesize { text: text.clone() }),
                None => Err(HookParseError::MissingField {
                    decision: "synthesize".to_owned(),
                    field: "text",
                }),
            },
            Some(raw) => Err(HookParseError::UnknownDecision {
                raw: raw.to_owned(),
            }),
        }
    }

    /// Converts to a post-tool output action.
    pub fn to_post_tool_action(&self) -> Result<PostToolAction, HookParseError> {
        let decision = self.decision.as_ref().map(|d| d.to_ascii_lowercase());
        match decision.as_deref() {
            None | Some("keep") | Some("allow") => Ok(PostToolAction::Keep),
            Some("replace") => match &self.text {
                Some(text) => Ok(PostToolAction::Replace { text: text.clone() }),
                None => Err(HookParseError::MissingField {
                    decision: "replace".to_owned(),
                    field: "text",
                }),
            },
            Some(raw) => Err(HookParseError::UnknownDecision {
                raw: raw.to_owned(),
            }),
        }
    }

    /// Converts to a stop action.
    pub fn to_stop_action(&self) -> Result<StopAction, HookParseError> {
        let decision = self.decision.as_ref().map(|d| d.to_ascii_lowercase());
        match decision.as_deref() {
            None | Some("proceed") | Some("allow") => Ok(StopAction::Proceed),
            Some("block") | Some("continue") => Ok(StopAction::Block {
                reason: self
                    .reason
                    .clone()
                    .unwrap_or_else(|| "stop blocked by hook".to_owned()),
            }),
            Some(raw) => Err(HookParseError::UnknownDecision {
                raw: raw.to_owned(),
            }),
        }
    }
}
