//! Foreign hook-config readers (compat-swallow, Grok §15): Claude's
//! settings-json shape and Cursor/legacy flat lists map onto the
//! unified [`HookSpec`] model. Unknown fields and unknown events are
//! swallowed as typed warnings; garbage never crashes — a foreign
//! config can produce specs, warnings, or nothing, never a panic.

use serde_json::Value;

use super::HookEvent;
use super::config::{DEFAULT_HOOK_TIMEOUT_SECS, HOOK_TIMEOUT_CEILING_SECS, HookKind, HookSpec};
use super::seams::HttpMethod;

/// One swallowed incompatibility (surfaced, never fatal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatWarning {
    /// An event name that maps to no trigger point; the entry is
    /// skipped.
    UnknownEvent {
        /// The unrecognized name.
        raw: String,
    },
    /// A config field this reader ignores.
    UnknownField {
        /// The ignored field name.
        field: String,
    },
    /// A hook `type` outside the four kinds; the entry is skipped.
    UnknownHookType {
        /// The unrecognized type string.
        raw: String,
    },
    /// A timeout that is not a positive number; the default applies.
    InvalidTimeout {
        /// What was found.
        raw: String,
    },
    /// A timeout above the ceiling, clamped to the ceiling.
    TimeoutCapped {
        /// The requested seconds.
        requested: u64,
    },
    /// A command-type entry carrying no command; an empty command is
    /// kept so the spec still surfaces for review.
    MissingCommand,
    /// A shape mismatch (hooks key not an object/array, group not an
    /// object); the branch is skipped.
    MalformedShape {
        /// Where the mismatch was found.
        where_: &'static str,
    },
}

/// The reader result: mapped specs plus every swallowed warning.
#[derive(Debug, Clone, Default)]
pub struct CompatReport {
    /// Specs in document order.
    pub specs: Vec<HookSpec>,
    /// Everything swallowed along the way.
    pub warnings: Vec<CompatWarning>,
}

const KNOWN_ENTRY_FIELDS: &[&str] = &[
    "type",
    "command",
    "url",
    "method",
    "timeout",
    "timeout_secs",
    "policy",
    "server",
    "tool",
];

fn read_timeout(entry: &Value, report: &mut CompatReport) -> u64 {
    let Some(raw) = entry.get("timeout").or_else(|| entry.get("timeout_secs")) else {
        return DEFAULT_HOOK_TIMEOUT_SECS;
    };
    match raw.as_u64() {
        Some(secs) if (1..=HOOK_TIMEOUT_CEILING_SECS).contains(&secs) => secs,
        Some(secs) if secs > HOOK_TIMEOUT_CEILING_SECS => {
            report
                .warnings
                .push(CompatWarning::TimeoutCapped { requested: secs });
            HOOK_TIMEOUT_CEILING_SECS
        }
        _ => {
            report.warnings.push(CompatWarning::InvalidTimeout {
                raw: raw.to_string(),
            });
            DEFAULT_HOOK_TIMEOUT_SECS
        }
    }
}

fn read_kind(entry: &Value, report: &mut CompatReport) -> Option<HookKind> {
    let kind = match entry.get("type").and_then(Value::as_str) {
        Some("command") | None => {
            let command = entry
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if command.is_empty() {
                report.warnings.push(CompatWarning::MissingCommand);
            }
            HookKind::Shell { command }
        }
        Some("http") => HookKind::Http {
            method: entry
                .get("method")
                .and_then(Value::as_str)
                .map_or(HttpMethod::Post, HttpMethod::parse),
            url: entry
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        },
        Some("mcp") => HookKind::Mcp {
            server: entry
                .get("server")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            tool: entry
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        },
        Some("prompt") => HookKind::Prompt {
            policy: entry
                .get("policy")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        },
        Some(raw) => {
            report.warnings.push(CompatWarning::UnknownHookType {
                raw: raw.to_owned(),
            });
            return None;
        }
    };
    Some(kind)
}

fn warn_unknown_fields(entry: &Value, report: &mut CompatReport) {
    let Some(object) = entry.as_object() else {
        return;
    };
    for key in object.keys() {
        if !KNOWN_ENTRY_FIELDS.contains(&key.as_str()) {
            report
                .warnings
                .push(CompatWarning::UnknownField { field: key.clone() });
        }
    }
}

fn build_spec(
    entry: &Value,
    event: HookEvent,
    matcher: Option<String>,
    index: usize,
    report: &mut CompatReport,
) {
    warn_unknown_fields(entry, report);
    let Some(kind) = read_kind(entry, report) else {
        return;
    };
    let mut spec = HookSpec::new(
        &format!("imported-{}-{index}", event.as_str()),
        &[event],
        kind,
    )
    .with_timeout_secs(read_timeout(entry, report));
    if let Some(matcher) = matcher {
        spec = spec.with_tool_matcher(&matcher);
    }
    report.specs.push(spec);
}

/// Reads Claude's settings-json shape:
/// `{"hooks": {"PreToolUse": [{"matcher": …, "hooks": [entry…]}…]…}}`.
/// Event names accept CamelCase, snake_case, and the alias table.
#[must_use]
pub fn read_claude_settings(value: &Value) -> CompatReport {
    let mut report = CompatReport::default();
    let Some(object) = value.as_object() else {
        report.warnings.push(CompatWarning::MalformedShape {
            where_: "document root is not an object",
        });
        return report;
    };
    for key in object.keys() {
        if key != "hooks" {
            report
                .warnings
                .push(CompatWarning::UnknownField { field: key.clone() });
        }
    }
    let Some(events) = value.get("hooks").and_then(Value::as_object) else {
        if object.contains_key("hooks") {
            report.warnings.push(CompatWarning::MalformedShape {
                where_: "\"hooks\" is not an object",
            });
        }
        return report;
    };
    let mut index = 0usize;
    for (raw_event, groups) in events {
        let Some(event) = HookEvent::parse(raw_event) else {
            report.warnings.push(CompatWarning::UnknownEvent {
                raw: raw_event.clone(),
            });
            continue;
        };
        let Some(groups) = groups.as_array() else {
            report.warnings.push(CompatWarning::MalformedShape {
                where_: "event group is not an array",
            });
            continue;
        };
        for group in groups {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let Some(entries) = group.get("hooks").and_then(Value::as_array) else {
                report.warnings.push(CompatWarning::MalformedShape {
                    where_: "group \"hooks\" is not an array",
                });
                continue;
            };
            for entry in entries {
                if !entry.is_object() {
                    report.warnings.push(CompatWarning::MalformedShape {
                        where_: "hook entry is not an object",
                    });
                    continue;
                }
                build_spec(entry, event, matcher.clone(), index, &mut report);
                index += 1;
            }
        }
    }
    report
}

/// Reads Cursor-style / legacy flat lists:
/// `{"hooks": [{"event": "before_tool_call", "type": …, …}…]}`.
#[must_use]
pub fn read_flat(value: &Value) -> CompatReport {
    let mut report = CompatReport::default();
    let Some(entries) = value.get("hooks").and_then(Value::as_array) else {
        if value.get("hooks").is_some() {
            report.warnings.push(CompatWarning::MalformedShape {
                where_: "\"hooks\" is not an array",
            });
        }
        return report;
    };
    for (index, entry) in entries.iter().enumerate() {
        if !entry.is_object() {
            report.warnings.push(CompatWarning::MalformedShape {
                where_: "hook entry is not an object",
            });
            continue;
        }
        let raw_event = entry
            .get("event")
            .or_else(|| entry.get("hook_event"))
            .and_then(Value::as_str);
        let Some(raw_event) = raw_event else {
            report.warnings.push(CompatWarning::UnknownEvent {
                raw: "(missing)".to_owned(),
            });
            continue;
        };
        let Some(event) = HookEvent::parse(raw_event) else {
            report.warnings.push(CompatWarning::UnknownEvent {
                raw: raw_event.to_owned(),
            });
            continue;
        };
        let matcher = entry
            .get("matcher")
            .and_then(Value::as_str)
            .map(str::to_owned);
        build_spec(entry, event, matcher, index, &mut report);
    }
    report
}
