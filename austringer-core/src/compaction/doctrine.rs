//! Anthropic-doctrine selection: what a compaction MUST preserve
//! (plan todo 13).
//!
//! [`select`] is a pure structural pass over the event stream — no
//! model, no clock — extracting architectural decisions and unresolved
//! bugs verbatim (marker lines), the last few unique file paths touched
//! by tool calls, capped subagent summaries, the protected head, and
//! the retained last-N-turns window. Selection always runs, on the
//! doctrine path and on the deterministic fallback alike: a compaction
//! that only kept a model's prose would lose the doctrine; a bare cut
//! is not compaction.

use super::CompactedSession;
use crate::event::content::ContentBlock;
use crate::event::{Event, RecordedEvent, Sequence};
use crate::eviction::{estimate_tokens, events_tokens, soft_trim};

/// The doctrine knobs (Anthropic defaults: ~5 recent files, last ~10
/// turns, ~3 protected head events, subagent summaries capped in the
/// 1–2k-token band).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoctrineConfig {
    /// How many unique recent file paths survive.
    pub recent_files: usize,
    /// How many trailing user-anchored turns survive verbatim.
    pub recent_turns: usize,
    /// How many leading events are protected.
    pub head_events: usize,
    /// Subagent summary token cap (doctrine band: 1000–2000).
    pub subagent_cap_tokens: u64,
}

impl Default for DoctrineConfig {
    fn default() -> Self {
        Self {
            recent_files: 5,
            recent_turns: 10,
            head_events: 3,
            subagent_cap_tokens: 1_500,
        }
    }
}

/// The structural selection result: doctrine content carried verbatim
/// plus the retained event windows.
#[derive(Debug, Clone, PartialEq)]
pub struct DoctrineSummary {
    /// `DECISION:` lines, verbatim, in first-seen order.
    pub architectural_decisions: Vec<String>,
    /// `UNRESOLVED BUG:` lines, verbatim, in first-seen order.
    pub unresolved_bugs: Vec<String>,
    /// The last `recent_files` unique paths tool calls referenced.
    pub recent_files: Vec<String>,
    /// Subagent summaries (`<subagent_summary …>` blocks), capped.
    pub subagent_summaries: Vec<String>,
    /// The protected leading events.
    pub head: Vec<RecordedEvent>,
    /// The retained trailing turns window.
    pub retained: Vec<RecordedEvent>,
}

impl DoctrineSummary {
    /// The deterministic typed render: every section lists CONTENT,
    /// never counts.
    #[must_use]
    pub fn render(&self) -> String {
        let section = |name: &str, items: &[String]| {
            let body = if items.is_empty() {
                "  (none)".to_owned()
            } else {
                items
                    .iter()
                    .map(|item| format!("- {item}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            format!("<{name}>\n{body}\n</{name}>")
        };
        format!(
            "<doctrine>\n{}\n{}\n{}\n{}\n</doctrine>",
            section("architectural_decisions", &self.architectural_decisions),
            section("unresolved_bugs", &self.unresolved_bugs),
            section("recent_files", &self.recent_files),
            section("subagent_summaries", &self.subagent_summaries),
        )
    }
}

/// Runs the structural selection over `events` under `config`.
#[must_use]
pub fn select(events: &[RecordedEvent], config: &DoctrineConfig) -> DoctrineSummary {
    let mut decisions = Vec::new();
    let mut bugs = Vec::new();
    let mut subagents = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for recorded in events {
        for text in texts_of(recorded) {
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("DECISION:") {
                    push_unique(&mut decisions, format!("DECISION:{rest}"));
                } else if let Some(rest) = line.strip_prefix("UNRESOLVED BUG:") {
                    push_unique(&mut bugs, format!("UNRESOLVED BUG:{rest}"));
                }
            }
            if text.trim_start().starts_with("<subagent_summary") {
                subagents.push(cap(text, config.subagent_cap_tokens));
            }
        }
        if let Event::ToolCall { arguments, .. } = &recorded.event
            && let Some(path) = path_of(arguments)
        {
            push_unique(&mut files, path);
        }
    }
    let retained_from = retained_from_index(events, config.recent_turns);
    let cap = config.subagent_cap_tokens;
    DoctrineSummary {
        architectural_decisions: decisions,
        unresolved_bugs: bugs,
        recent_files: files
            .into_iter()
            .rev()
            .take(config.recent_files)
            .rev()
            .collect(),
        subagent_summaries: subagents,
        head: events
            .iter()
            .take(config.head_events)
            .map(|recorded| trim_oversized(recorded, cap))
            .collect(),
        retained: events
            .iter()
            .skip(retained_from)
            .map(|recorded| trim_oversized(recorded, cap))
            .collect(),
    }
}

/// Builds the deterministic compacted session from a selection: the
/// doctrine render is the summary; the retained window survives (the
/// caller may fold it); `fallback_used` marks the zero-LLM path.
#[must_use]
pub fn compacted_from(
    summary_render: String,
    events: &[RecordedEvent],
    retained: Vec<RecordedEvent>,
    fallback_used: bool,
) -> CompactedSession {
    let through_seq = events.last().map_or(Sequence(0), |event| event.seq);
    let before = events_tokens(events);
    let compacted = CompactedSession {
        summary: summary_render,
        retained,
        through_seq,
        before_tokens: before,
        after_tokens: 0,
        fallback_used,
    };
    let after = estimate_tokens(&compacted.render_context());
    CompactedSession {
        after_tokens: after,
        ..compacted
    }
}

/// Index the retained window starts at: the (recent_turns)-th
/// user-anchored turn from the end.
fn retained_from_index(events: &[RecordedEvent], recent_turns: usize) -> usize {
    let mut turns_left = recent_turns;
    let mut start = events.len();
    for (index, recorded) in events.iter().enumerate().rev() {
        if matches!(recorded.event, Event::User { .. }) {
            if turns_left == 0 {
                break;
            }
            turns_left -= 1;
            start = index;
        }
    }
    if turns_left > 0 { 0 } else { start }
}

/// Caps a summary at `cap_tokens` through omission-marked head/tail
/// trims — never a bare cut. Reserves 64 bytes so the trim marker
/// itself cannot push the result over budget.
fn cap(text: &str, cap_tokens: u64) -> String {
    let cap_bytes = (cap_tokens * 4) as usize;
    if estimate_tokens(text) <= cap_tokens {
        return text.to_owned();
    }
    let half = (cap_bytes / 2).saturating_sub(32);
    soft_trim(text, half, half)
}

/// One retained/head event with any oversized conversation text block
/// brought under the cap (omission-marked trim — a huge single turn
/// never blows the compacted context, and the cut is always visible).
fn trim_oversized(recorded: &RecordedEvent, cap_tokens: u64) -> RecordedEvent {
    let trim_blocks = |blocks: &[ContentBlock]| -> Vec<ContentBlock> {
        blocks
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } if estimate_tokens(text) > cap_tokens => {
                    ContentBlock::Text {
                        text: cap(text, cap_tokens),
                    }
                }
                other => other.clone(),
            })
            .collect()
    };
    let event = match &recorded.event {
        Event::User { blocks } => Event::User {
            blocks: trim_blocks(blocks),
        },
        Event::Assistant { blocks } => Event::Assistant {
            blocks: trim_blocks(blocks),
        },
        Event::System { text } if estimate_tokens(text) > cap_tokens => Event::System {
            text: cap(text, cap_tokens),
        },
        other => other.clone(),
    };
    RecordedEvent {
        seq: recorded.seq,
        event,
    }
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn texts_of(recorded: &RecordedEvent) -> Vec<&str> {
    let mut out = Vec::new();
    match &recorded.event {
        Event::User { blocks } | Event::Assistant { blocks } | Event::ToolResult { blocks, .. } => {
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    out.push(text.as_str());
                }
            }
        }
        Event::System { text } => out.push(text.as_str()),
        _ => {}
    }
    out
}

fn path_of(arguments: &serde_json::Value) -> Option<String> {
    let object = arguments.as_object()?;
    for key in ["path", "file"] {
        if let Some(value) = object.get(key)
            && let Some(path) = value.as_str()
        {
            return Some(path.to_owned());
        }
    }
    None
}
