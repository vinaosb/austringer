//! Folding, content pointers, return codes, the single volatile error
//! slot, and the injected seams (plan todo 12).
//!
//! Tier2 folding replaces a tool result's payload with a parseable
//! `<folded_tool_output …/>` marker that always carries the call's
//! return code and — for hard-clears — a blake3 content pointer to the
//! original bytes (the T4/T6 hash discipline). Folding REFUSES (typed)
//! when the authoritative return-code table lacks a call's code: codes
//! are never dropped. The cheap-model condensation seam, the salience
//! filter seam, and the memory-flush hook are injected traits; the
//! latest error REPLACES the previous in the projection (errors never
//! stack — house doctrine).

use std::collections::BTreeMap;

use async_trait::async_trait;

use super::EvictionConfig;
use super::tiers::Projection;
use crate::event::content::ContentBlock;
use crate::event::{ContentHash, Event, RecordedEvent, Sequence};

/// A tool call's authoritative return code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReturnCode(pub i32);

/// The authoritative `call_id → ReturnCode` table supplied by the caller
/// (the loop knows every executed call's exit status).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolReturnTable(pub BTreeMap<String, ReturnCode>);

/// Reasons a `<folded_tool_output>` marker fails to parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FoldParseError {
    /// Missing the `<folded_tool_output ` prefix or `/>` suffix.
    #[error("malformed folded-tool-output marker")]
    Malformed,
    /// A required attribute is absent.
    #[error("missing attribute: {0}")]
    MissingAttr(&'static str),
    /// `return_code` is not an integer.
    #[error("return_code is not an integer")]
    BadCode,
    /// `content` is not a blake3 hash reference.
    #[error("content pointer is not a valid content hash")]
    BadHash,
}

/// The typed folded-tool-output marker: parseable, deterministic, and it
/// NEVER drops the return code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldedToolOutput {
    /// The folded call's correlation id.
    pub call_id: String,
    /// The tool that was called.
    pub tool: String,
    /// Blake3 pointer to the original payload bytes (hard-clears only;
    /// masks discard bulk without a pointer).
    pub content_hash: Option<ContentHash>,
    /// The call's return code — always present.
    pub return_code: ReturnCode,
    /// Byte length of the original payload.
    pub original_bytes: u64,
}

impl FoldedToolOutput {
    /// The deterministic marker form.
    #[must_use]
    pub fn render(&self) -> String {
        let pointer = self.content_hash.as_ref().map_or_else(String::new, |hash| {
            format!(" content=\"{}\"", hash.to_ref())
        });
        format!(
            "<folded_tool_output call_id=\"{}\" tool=\"{}\"{pointer} return_code=\"{}\" bytes=\"{}\"/>",
            self.call_id, self.tool, self.return_code.0, self.original_bytes
        )
    }

    /// Parses a marker back (parseability is the marker contract).
    pub fn parse(marker: &str) -> Result<Self, FoldParseError> {
        let body = marker
            .strip_prefix("<folded_tool_output ")
            .and_then(|rest| rest.strip_suffix("/>"))
            .ok_or(FoldParseError::Malformed)?;
        let mut attrs = BTreeMap::new();
        for pair in body.split(' ') {
            if let Some((key, value)) = pair.split_once('=') {
                attrs.insert(key.to_owned(), value.trim_matches('"').to_owned());
            }
        }
        let attr = |key: &'static str| {
            attrs
                .get(key)
                .cloned()
                .ok_or(FoldParseError::MissingAttr(key))
        };
        Ok(Self {
            call_id: attr("call_id")?,
            tool: attr("tool")?,
            content_hash: match attrs.get("content") {
                Some(value) => {
                    Some(ContentHash::parse(value).map_err(|_| FoldParseError::BadHash)?)
                }
                None => None,
            },
            return_code: ReturnCode(
                attr("return_code")?
                    .parse()
                    .map_err(|_| FoldParseError::BadCode)?,
            ),
            original_bytes: attr("bytes")?
                .parse()
                .map_err(|_| FoldParseError::BadCode)?,
        })
    }
}

/// Condensation output for one middle-zone span: an `<active_plan>`-style
/// typed block replacing the span in the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CondensateBlock {
    /// The last sequence number folded into this condensate.
    pub through_seq: Sequence,
    /// The condensed summary (cheap-model output).
    pub summary: String,
    /// Blake3 hash of the folded span's serialized bytes.
    pub source_hash: ContentHash,
}

impl CondensateBlock {
    /// The deterministic tagged render.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "<active_plan through_seq=\"{}\" hash=\"{}\">\n{}\n</active_plan>",
            self.through_seq.0,
            self.source_hash.to_ref(),
            self.summary
        )
    }
}

/// One condensation request to the cheap-model seam.
pub struct CondenseRequest<'a> {
    /// The middle-zone span to condense.
    pub span: &'a [RecordedEvent],
    /// Serialized byte size of the span.
    pub span_bytes: u64,
}

/// The cheap-model condensation seam (async by design; tests inject a
/// deterministic mock, the real wiring arrives with todo 13's
/// compactor).
#[async_trait]
pub trait Condenser: Send + Sync {
    /// Condenses `request.span` into a summary.
    async fn condense(&self, request: CondenseRequest<'_>) -> Result<String, CondenseError>;
}

/// A condensation-seam failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("condense seam failed: {0}")]
pub struct CondenseError(pub String);

/// The salience seam deciding which facts survive a pre-compact memory
/// flush (mock in tests; the real filter arrives with todo 24's memory
/// subsystem).
pub trait SalienceFilter: Send + Sync {
    /// Salient facts carried by one event (empty when none).
    fn salient(&self, event: &RecordedEvent) -> Vec<String>;
}

/// The memory-store hook receiving pre-compact flushes.
pub trait MemoryHook: Send {
    /// Persists `facts` to the memory store.
    fn flush(&mut self, facts: Vec<String>);
}

/// In-memory [`MemoryHook`] collecting flushes (tests, SIM, embedders).
#[derive(Debug, Default)]
pub struct VecMemoryHook {
    /// Every flush received, in order.
    pub flushed: Vec<Vec<String>>,
}

impl MemoryHook for VecMemoryHook {
    fn flush(&mut self, facts: Vec<String>) {
        self.flushed.push(facts);
    }
}

/// One pre-compact memory flush: the salient facts distilled out of the
/// session N tokens before the compact threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryFlush {
    /// The flushed facts.
    pub facts: Vec<String>,
}

/// One error observation (from inference/tool failures upstream).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorObservation {
    /// Sequence number of the failing turn.
    pub seq: Sequence,
    /// Machine-readable error code.
    pub code: u16,
    /// Human/model-readable message.
    pub message: String,
}

/// The SINGLE volatile error slot: the latest error replaces the
/// previous — errors never stack in the LLM-visible projection.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ErrorSlot(Option<ErrorObservation>);

impl ErrorSlot {
    /// An empty slot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Observes a new error, REPLACING any previous one.
    pub fn observe(&mut self, error: ErrorObservation) {
        self.0 = Some(error);
    }

    /// The latest observation, when any.
    #[must_use]
    pub fn latest(&self) -> Option<&ErrorObservation> {
        self.0.as_ref()
    }

    /// The single typed error block (empty string when no error).
    #[must_use]
    pub fn render(&self) -> String {
        match &self.0 {
            None => String::new(),
            Some(error) => format!(
                "<volatile_error seq=\"{}\" code=\"{}\">{}</volatile_error>",
                error.seq.0, error.code, error.message
            ),
        }
    }
}

/// Concatenated text blocks of a tool result (the foldable payload).
pub(crate) fn result_text(recorded: &RecordedEvent) -> String {
    match &recorded.event {
        Event::ToolResult { blocks, .. } => blocks
            .iter()
            .map(ContentBlock::text_marker)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Builds the folded replacement event for one tool result. `pointer`
/// selects hard-clear (blake3 pointer) vs mask (bulk discarded, code
/// kept). Refuses typed when the return code is unknown.
pub(crate) fn replace_result(
    recorded: &RecordedEvent,
    tool: &str,
    code: ReturnCode,
    pointer: bool,
) -> RecordedEvent {
    let payload = result_text(recorded);
    let folded = FoldedToolOutput {
        call_id: call_id_of(recorded),
        tool: tool.to_owned(),
        content_hash: pointer.then(|| ContentHash::from_bytes(payload.as_bytes())),
        return_code: code,
        original_bytes: payload.len() as u64,
    };
    RecordedEvent {
        seq: recorded.seq,
        event: Event::ToolResult {
            call_id: folded.call_id.clone(),
            blocks: vec![ContentBlock::Text {
                text: folded.render(),
            }],
        },
    }
}

fn call_id_of(recorded: &RecordedEvent) -> String {
    match &recorded.event {
        Event::ToolResult { call_id, .. } | Event::ToolCall { call_id, .. } => call_id.clone(),
        _ => String::new(),
    }
}

/// `call_id → tool name` index over a history.
pub(crate) fn tool_index(history: &[RecordedEvent]) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for recorded in history {
        if let Event::ToolCall { call_id, tool, .. } = &recorded.event {
            index.insert(call_id.clone(), tool.clone());
        }
    }
    index
}

/// Hard-clears every middle-zone tool result to pointer markers
/// (protected head/tail untouched). The foldHistory contract: every
/// marker carries the authoritative return code or folding refuses.
pub fn fold_history(
    history: &[RecordedEvent],
    config: &EvictionConfig,
    table: &ToolReturnTable,
) -> Result<Vec<RecordedEvent>, super::EvictionError> {
    let projection = Projection::new(history, config);
    let tools = tool_index(history);
    let mut folded = Vec::with_capacity(history.len());
    for (index, recorded) in history.iter().enumerate() {
        let keep =
            !matches!(&recorded.event, Event::ToolResult { .. }) || !projection.is_evictable(index);
        if keep {
            folded.push(recorded.clone());
            continue;
        }
        let call_id = call_id_of(recorded);
        let code =
            table
                .0
                .get(&call_id)
                .ok_or_else(|| super::EvictionError::MissingReturnCode {
                    call_id: call_id.clone(),
                })?;
        let tool = tools.get(&call_id).map(String::as_str).unwrap_or("unknown");
        folded.push(replace_result(recorded, tool, *code, true));
    }
    Ok(folded)
}
