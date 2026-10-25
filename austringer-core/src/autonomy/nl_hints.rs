//! NL permission hints (plan todo 8): a deterministic mini-grammar
//! turning natural-language allow/deny/ask phrases into typed permission
//! rules (and forever-scoped store grants).
//!
//! Grammar (case-insensitive, ordered):
//!
//! - `always allow <subject> [forever]` → an allow rule for the subject
//!   (command-prefix when the subject is command-like, path glob when it
//!   looks like a path). The bare phrase is SESSION-scoped config; the
//!   word `forever` additionally records a durable grant in the store —
//!   that answers the plan's "session-forever?" question.
//! - `never touch|read|access <path>` → a deny rule over the path glob.
//! - `never run|execute <command>` → a deny rule over the command prefix.
//! - `ask before <activity>` → ask rules (`deleting` expands to the
//!   rm/del/rd/Remove-Item family; `editing`/`writing` to `edit_lines`).
//!
//! Global subjects ("everything", "all", "any tool") and bypass phrases
//! ("bypass permissions", "yolo") are TYPED REFUSALS — never a global
//! allow, never silently ignored.

use crate::inspectors::permission::{Effect, PermissionConfig, Rule};
use crate::inspectors::store::{Grant, GrantScope, PermissionStore, StoreError};

/// Subjects that would turn a phrase into a global rule.
const GLOBAL_SUBJECTS: &[&str] = &[
    "everything",
    "all",
    "anything",
    "anything at all",
    "every tool",
    "all tools",
    "every command",
    "all commands",
    "*",
];

/// Bare bypass phrases (whole-phrase matches).
const BYPASS_PHRASES: &[&str] = &[
    "bypass permissions",
    "bypass permission checks",
    "yolo",
    "yolo mode",
    "allow everything",
    "allow all",
    "allow anything",
    "allow all tools",
];

/// One parsed, typed hint.
#[derive(Debug, Clone)]
pub enum ParsedHint {
    /// An allow rule (session-scoped config).
    AllowRule(Rule),
    /// A deny rule.
    DenyRule(Rule),
    /// An ask rule.
    AskRule(Rule),
    /// A durable forever grant (the `forever` word).
    Grant(Grant),
}

/// Why a phrase was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintRefusal {
    /// The phrase asked for a global allow/bypass — never applied.
    BypassAttempt {
        /// What the phrase asked for.
        detail: String,
    },
    /// The phrase is not part of the grammar (typed feedback, never a
    /// silent ignore).
    Unrecognized {
        /// The original phrase.
        phrase: String,
    },
}

/// The parse outcome.
#[derive(Debug, Clone)]
pub enum HintParse {
    /// Every hint the phrase produced, in application order.
    Applied {
        /// The typed hints.
        hints: Vec<ParsedHint>,
    },
    /// The phrase was refused.
    Refused {
        /// Why.
        reason: HintRefusal,
    },
}

fn is_global(subject: &str) -> bool {
    GLOBAL_SUBJECTS.contains(&subject)
}

fn is_path_like(subject: &str) -> bool {
    subject.starts_with('.') || subject.starts_with('~') || subject.contains(['/', '\\'])
}

fn subject_glob(subject: &str) -> String {
    format!("{subject}*")
}

/// Parses one phrase into typed hints (pure: no store, no config).
#[must_use]
pub fn parse_hint(phrase: &str) -> HintParse {
    let normalized = phrase.trim().to_lowercase();
    if normalized.is_empty() {
        return refused_unrecognized(phrase);
    }
    if BYPASS_PHRASES.contains(&normalized.as_str()) {
        return HintParse::Refused {
            reason: HintRefusal::BypassAttempt { detail: normalized },
        };
    }
    if let Some(rest) = normalized.strip_prefix("always allow ") {
        return parse_always_allow(rest);
    }
    for verb in ["never touch ", "never read ", "never access "] {
        if let Some(rest) = normalized.strip_prefix(verb) {
            return parse_never_path(rest);
        }
    }
    for verb in ["never run ", "never execute "] {
        if let Some(rest) = normalized.strip_prefix(verb) {
            return parse_never_command(rest);
        }
    }
    if let Some(rest) = normalized.strip_prefix("ask before ") {
        return parse_ask_before(rest);
    }
    refused_unrecognized(phrase)
}

fn refused_unrecognized(phrase: &str) -> HintParse {
    HintParse::Refused {
        reason: HintRefusal::Unrecognized {
            phrase: phrase.trim().to_owned(),
        },
    }
}

fn bypass(rest: &str) -> HintParse {
    HintParse::Refused {
        reason: HintRefusal::BypassAttempt {
            detail: rest.to_owned(),
        },
    }
}

fn parse_always_allow(rest: &str) -> HintParse {
    let (subject, forever) = match rest.strip_suffix(" forever") {
        Some(subject) => (subject.trim(), true),
        None => (rest.trim(), false),
    };
    if subject.is_empty() || is_global(subject) {
        return bypass(rest);
    }
    let mut hints = Vec::new();
    if is_path_like(subject) {
        if let Ok(rule) = Rule::path(Effect::Allow, "*", &subject_glob(subject)) {
            hints.push(ParsedHint::AllowRule(rule));
            if forever {
                hints.push(ParsedHint::Grant(Grant {
                    tool: "view_window".to_owned(),
                    target: Some(subject.to_owned()),
                    scope: GrantScope::Forever,
                }));
            }
        }
    } else if let Ok(rule) = Rule::command(Effect::Allow, "execute_command", &subject_glob(subject))
    {
        hints.push(ParsedHint::AllowRule(rule));
        if forever {
            hints.push(ParsedHint::Grant(Grant {
                tool: "execute_command".to_owned(),
                target: Some(subject.to_owned()),
                scope: GrantScope::Forever,
            }));
        }
    }
    if hints.is_empty() {
        return refused_unrecognized(rest);
    }
    HintParse::Applied { hints }
}

fn parse_never_path(rest: &str) -> HintParse {
    let subject = rest.trim();
    if subject.is_empty() || is_global(subject) {
        return bypass(rest);
    }
    match Rule::path(Effect::Deny, "*", &subject_glob(subject)) {
        Ok(rule) => HintParse::Applied {
            hints: vec![ParsedHint::DenyRule(rule)],
        },
        Err(_) => refused_unrecognized(rest),
    }
}

fn parse_never_command(rest: &str) -> HintParse {
    let subject = rest.trim();
    if subject.is_empty() || is_global(subject) {
        return bypass(rest);
    }
    match Rule::command(Effect::Deny, "execute_command", &subject_glob(subject)) {
        Ok(rule) => HintParse::Applied {
            hints: vec![ParsedHint::DenyRule(rule)],
        },
        Err(_) => refused_unrecognized(rest),
    }
}

fn parse_ask_before(rest: &str) -> HintParse {
    let subject = rest.trim();
    let mut hints = Vec::new();
    match subject {
        "deleting" | "removing" | "delete" | "remove" => {
            for pattern in ["rm *", "del *", "rd *", "rmdir *", "Remove-Item *"] {
                if let Ok(rule) = Rule::command(Effect::Ask, "execute_command", pattern) {
                    hints.push(ParsedHint::AskRule(rule));
                }
            }
        }
        "editing" | "writing" | "edits" => {
            if let Ok(rule) = Rule::tool(Effect::Ask, "edit_lines") {
                hints.push(ParsedHint::AskRule(rule));
            }
        }
        "committing" | "commits" => {
            if let Ok(rule) = Rule::command(Effect::Ask, "execute_command", "git commit*") {
                hints.push(ParsedHint::AskRule(rule));
            }
        }
        _ if !subject.is_empty() && !is_global(subject) => {
            let pattern = subject_glob(subject);
            let rule = if is_path_like(subject) {
                Rule::path(Effect::Ask, "*", &pattern)
            } else {
                Rule::command(Effect::Ask, "execute_command", &pattern)
            };
            if let Ok(rule) = rule {
                hints.push(ParsedHint::AskRule(rule));
            }
        }
        _ => {}
    }
    if hints.is_empty() {
        return refused_unrecognized(rest);
    }
    HintParse::Applied { hints }
}

/// Parses and APPLIES one phrase: rules land in `config`, forever grants
/// land in `store`. Refusals change nothing.
///
/// # Errors
/// Typed [`StoreError`] only when a durable grant fails to persist (the
/// rules that already landed stay landed; refusal outcomes are values,
/// not errors).
pub fn apply_hint(
    config: &mut PermissionConfig,
    store: &dyn PermissionStore,
    phrase: &str,
) -> Result<HintParse, StoreError> {
    let parsed = parse_hint(phrase);
    if let HintParse::Applied { hints } = &parsed {
        for hint in hints {
            match hint {
                ParsedHint::AllowRule(rule)
                | ParsedHint::DenyRule(rule)
                | ParsedHint::AskRule(rule) => config.rules.push(rule.clone()),
                ParsedHint::Grant(grant) => store.record(grant)?,
            }
        }
    }
    Ok(parsed)
}
