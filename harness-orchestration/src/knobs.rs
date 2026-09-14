//! Per-subagent model bracket params (Cursor SS19): the portable knob
//! grammar `model[effort=high,context=300k,thinking=medium]`, resolved
//! through intel's dial grammar and the SET/PROMPT/THROTTLE constitution
//! into a fully explicit [`ChildKnobs`] bundle (T34 integration).

use harness_intel::catalog::{Catalog, CatalogError, EffortValue, Family, ThinkingLevel};
use harness_intel::dial::{DialSpec, build_dial};
use harness_intel::elicitation::{TaskClass, ThinkingDirective, ThrottlePlan, elicit};
use harness_intel::model_profile::ModelProfile;
use harness_providers::identity::ModelIdentity;

/// A parsed model bracket: a model id plus portable knob overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelBracket {
    /// Model id (exact wire name).
    pub model: String,
    /// Explicit effort override.
    pub effort: Option<EffortValue>,
    /// Context-window cap in tokens (`300k` style accepted).
    pub context_tokens: Option<u64>,
    /// Thinking-level override.
    pub thinking_level: Option<ThinkingLevel>,
}

/// Typed bracket-parse failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BracketError {
    /// Key outside the grammar.
    #[error("unknown bracket key `{key}`")]
    UnknownKey {
        /// The offending key.
        key: String,
    },
    /// Effort value off the ladder.
    #[error("unknown effort `{value}`")]
    UnknownEffort {
        /// The rejected value.
        value: String,
    },
    /// Thinking value off the ladder.
    #[error("unknown thinking level `{value}`")]
    UnknownThinking {
        /// The rejected value.
        value: String,
    },
    /// Context value not a token count.
    #[error("bad context value `{value}`")]
    BadContext {
        /// The rejected value.
        value: String,
    },
    /// `[` without a closing `]`.
    #[error("unclosed bracket")]
    Unclosed,
    /// Empty model id.
    #[error("empty model id")]
    EmptyModel,
    /// Brackets inside brackets.
    #[error("nested brackets are not part of the grammar")]
    Nested,
}

impl ModelBracket {
    /// Inherent alias for [`parse_model_bracket`].
    pub fn parse(input: &str) -> Result<Self, BracketError> {
        parse_model_bracket(input)
    }
}

fn parse_effort(value: &str) -> Option<EffortValue> {
    [
        EffortValue::None,
        EffortValue::Low,
        EffortValue::Medium,
        EffortValue::High,
        EffortValue::Xhigh,
        EffortValue::Max,
    ]
    .into_iter()
    .find(|e| e.to_string() == value)
}

fn parse_thinking(value: &str) -> Option<ThinkingLevel> {
    match value {
        "minimal" => Some(ThinkingLevel::Minimal),
        "low" => Some(ThinkingLevel::Low),
        "medium" => Some(ThinkingLevel::Medium),
        "high" => Some(ThinkingLevel::High),
        _ => None,
    }
}

fn parse_context(value: &str) -> Option<u64> {
    let (digits, multiplier) = match value.char_indices().next_back() {
        Some((_, 'k') | (_, 'K')) => (&value[..value.len() - 1], 1_000u64),
        Some((_, 'm') | (_, 'M')) => (&value[..value.len() - 1], 1_000_000u64),
        _ => (value, 1),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(multiplier))
}

/// Parses `model`, or `model[k=v,...]` with keys
/// `effort|context|thinking`.
pub fn parse_model_bracket(input: &str) -> Result<ModelBracket, BracketError> {
    let input = input.trim();
    let Some(open) = input.find('[') else {
        if input.is_empty() || input.contains(']') {
            return Err(BracketError::EmptyModel);
        }
        return Ok(ModelBracket {
            model: input.to_string(),
            effort: None,
            context_tokens: None,
            thinking_level: None,
        });
    };
    let model = input[..open].trim();
    if model.is_empty() {
        return Err(BracketError::EmptyModel);
    }
    if !input.ends_with(']') {
        return Err(BracketError::Unclosed);
    }
    let inner = &input[open + 1..input.len() - 1];
    if inner.contains('[') || inner.contains(']') {
        return Err(BracketError::Nested);
    }
    let mut bracket = ModelBracket {
        model: model.to_string(),
        effort: None,
        context_tokens: None,
        thinking_level: None,
    };
    if inner.trim().is_empty() {
        return Ok(bracket);
    }
    for pair in inner.split(',') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(BracketError::UnknownKey {
                key: pair.trim().to_string(),
            });
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "effort" => {
                bracket.effort =
                    Some(
                        parse_effort(value).ok_or_else(|| BracketError::UnknownEffort {
                            value: value.to_string(),
                        })?,
                    );
            }
            "context" => {
                bracket.context_tokens =
                    Some(
                        parse_context(value).ok_or_else(|| BracketError::BadContext {
                            value: value.to_string(),
                        })?,
                    );
            }
            "thinking" => {
                bracket.thinking_level =
                    Some(
                        parse_thinking(value).ok_or_else(|| BracketError::UnknownThinking {
                            value: value.to_string(),
                        })?,
                    );
            }
            other => {
                return Err(BracketError::UnknownKey {
                    key: other.to_string(),
                });
            }
        }
    }
    Ok(bracket)
}

/// The provider string for a catalog family (build_dial takes the provider
/// as caller-owned configuration; the catalog owns the family facts).
pub fn provider_for_family(family: Family) -> &'static str {
    match family {
        Family::Anthropic => "anthropic",
        Family::OpenAi => "openai",
        Family::Google => "google",
        Family::Xai => "xai",
        Family::DeepSeek => "deepseek",
    }
}

/// Fully resolved per-child knobs: a dial position (identity + EXPLICIT
/// effort), the SET thinking directive, and the THROTTLE structural
/// budgets — nothing implicit.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildKnobs {
    /// Resolved model identity.
    pub model: ModelIdentity,
    /// Explicit effort (post elicitation scaling).
    pub effort: EffortValue,
    /// Context-window cap, if the bracket set one.
    pub context_cap: Option<u64>,
    /// Thinking directive for the SET layer.
    pub thinking: ThinkingDirective,
    /// Prompt addenda from the dial position.
    pub prompt_addenda: Vec<String>,
    /// THROTTLE budgets (concurrency/total-subtask caps).
    pub throttle: ThrottlePlan,
    /// The model's catalog family (family-switch detection).
    pub family: Family,
}

/// Knob resolution failure (catalog fail-closed).
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct KnobError(#[from] pub CatalogError);

/// Resolves a bracket into explicit child knobs: the dial grammar resolves
/// the identity and validates the effort against the model's grid; the
/// elicitation constitution applies the workload-class effort scaling and
/// produces the THROTTLE budgets.
pub fn resolve_knobs(
    bracket: &ModelBracket,
    class: TaskClass,
    catalog: &Catalog,
    profile: &ModelProfile,
) -> Result<ChildKnobs, KnobError> {
    let entry = catalog
        .entry(&bracket.model)
        .map_err(harness_intel::catalog::CatalogError::from)?;
    let provider = provider_for_family(entry.family);
    let spec = DialSpec {
        name: "child".into(),
        model: bracket.model.clone(),
        effort: bracket.effort,
        prompt_addenda: Vec::new(),
        tools: Vec::new(),
    };
    let mut positions = build_dial(provider, std::slice::from_ref(&spec), catalog)?;
    let Some(position) = positions.pop() else {
        return Err(KnobError(harness_intel::catalog::CatalogError::from(
            harness_intel::catalog::UnknownModel {
                model: bracket.model.clone(),
            },
        )));
    };
    let plan = elicit(entry, profile, class);
    Ok(ChildKnobs {
        model: position.model,
        effort: plan.set.effort,
        context_cap: bracket.context_tokens,
        thinking: plan.set.thinking,
        prompt_addenda: position.prompt_addenda,
        throttle: plan.throttle,
        family: entry.family,
    })
}
