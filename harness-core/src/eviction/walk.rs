//! The ladder-walk state machine shared by the eviction engines (plan
//! todo 12, internal).

use std::collections::BTreeMap;

use super::budget::{EvictionInput, events_tokens};
use super::fold::{
    CondensateBlock, CondenseRequest, Condenser, ReturnCode, replace_result, result_text,
    tool_index,
};
use super::ladder::{LadderStage, StageDecision, soft_trim};
use super::tiers::{Projection, Tier};
use super::{EvictionConfig, EvictionError};
use crate::event::{ContentBlock, Event, RecordedEvent};

/// Per-walk state: the working projection plus the input indexes the
/// stage operations need. Zone math is recomputed against the working
/// copy after every structural change (splices shift indexes).
pub(super) struct ReplaceCtx<'a> {
    input: &'a EvictionInput,
    config: &'a EvictionConfig,
    tools: BTreeMap<String, String>,
    /// The working projection (engine reads it for verification).
    pub(super) working: Vec<RecordedEvent>,
    /// Condensates produced so far (engine collects them).
    pub(super) condensates: Vec<CondensateBlock>,
    /// Applied-stage decisions (engine collects them).
    pub(super) decisions: Vec<StageDecision>,
}

impl<'a> ReplaceCtx<'a> {
    pub(super) fn new(input: &'a EvictionInput, config: &'a EvictionConfig) -> Self {
        Self {
            input,
            config,
            tools: tool_index(&input.history),
            working: input.history.clone(),
            condensates: Vec::new(),
            decisions: Vec::new(),
        }
    }

    pub(super) fn usage(&self) -> u64 {
        self.input.used_tokens_with(&self.working)
    }

    /// Middle-zone indexes whose event is an unfoldable tool result,
    /// oldest first.
    fn foldable(&self) -> Vec<usize> {
        let projection = Projection::new(&self.working, self.config);
        (0..self.working.len())
            .filter(|&index| {
                projection.zone(index) == Some(Tier::Two)
                    && matches!(self.working[index].event, Event::ToolResult { .. })
                    && !is_folded(&self.working[index])
            })
            .collect()
    }

    fn code_of(&self, call_id: &str) -> Result<ReturnCode, EvictionError> {
        self.input
            .tool_returns
            .0
            .get(call_id)
            .copied()
            .ok_or(EvictionError::MissingReturnCode {
                call_id: call_id.to_owned(),
            })
    }

    /// Replaces the result at `index` with its marker form (`pointer` =
    /// hard-clear vs mask), hashing the ORIGINAL payload from the input.
    fn replace_at(
        &mut self,
        index: usize,
        pointer: bool,
        decision: &mut StageDecision,
    ) -> Result<(), EvictionError> {
        let before = events_tokens(std::slice::from_ref(&self.working[index]));
        let call_id = call_of(&self.working[index]);
        let code = self.code_of(&call_id)?;
        let tool = self
            .tools
            .get(&call_id)
            .cloned()
            .unwrap_or_else(|| "unknown".to_owned());
        let source = self
            .input
            .history
            .iter()
            .find(|recorded| call_of(recorded) == call_id);
        let folded = match source {
            Some(original) => replace_result(original, &tool, code, pointer),
            None => replace_result(&self.working[index], &tool, code, pointer),
        };
        let after = events_tokens(std::slice::from_ref(&folded));
        self.working[index] = folded;
        decision.reclaimed_tokens += before.saturating_sub(after);
        decision.items += 1;
        Ok(())
    }

    pub(super) fn mask(&mut self, budget: u64) -> Result<StageDecision, EvictionError> {
        let mut decision = StageDecision {
            stage: LadderStage::Masking,
            items: 0,
            reclaimed_tokens: 0,
        };
        loop {
            if decision.reclaimed_tokens >= budget {
                return Ok(decision);
            }
            let Some(index) = self.foldable().first().copied() else {
                return Ok(decision);
            };
            self.replace_at(index, false, &mut decision)?;
        }
    }

    pub(super) async fn distill(
        &mut self,
        budget: u64,
        condenser: &dyn Condenser,
    ) -> Result<StageDecision, EvictionError> {
        let mut decision = StageDecision {
            stage: LadderStage::Distillation,
            items: 0,
            reclaimed_tokens: 0,
        };
        loop {
            if decision.reclaimed_tokens >= budget {
                return Ok(decision);
            }
            let Some(span) = self.next_conversation_span() else {
                return Ok(decision);
            };
            let before = events_tokens(&self.working[span.clone()]);
            let span_bytes = before * 4;
            let summary = condenser
                .condense(CondenseRequest {
                    span: &self.working[span.clone()],
                    span_bytes,
                })
                .await
                .map_err(EvictionError::Condense)?;
            let mut serialized = String::new();
            for recorded in &self.working[span.clone()] {
                serialized.push_str(&serde_json::to_string(&recorded.event).unwrap_or_default());
            }
            let through_seq = self.working[span.end - 1].seq;
            let condensate = CondensateBlock {
                through_seq,
                summary,
                source_hash: crate::event::ContentHash::from_bytes(serialized.as_bytes()),
            };
            self.working.splice(
                span,
                std::iter::once(RecordedEvent {
                    seq: through_seq,
                    event: Event::System {
                        text: condensate.render(),
                    },
                }),
            );
            self.condensates.push(condensate);
            let after = events_tokens(&self.working);
            decision.reclaimed_tokens += before.saturating_sub(after);
            decision.items += 1;
        }
    }

    /// The oldest contiguous run of middle-zone Tier1 events not already
    /// a condensate.
    fn next_conversation_span(&self) -> Option<std::ops::Range<usize>> {
        let projection = Projection::new(&self.working, self.config);
        let in_run = |index: usize| {
            projection.zone(index) == Some(Tier::One) && !is_condensate(&self.working[index])
        };
        let start = (0..self.working.len()).find(|&index| in_run(index))?;
        let end = start
            + (start..self.working.len())
                .take_while(|&index| in_run(index))
                .count();
        Some(start..end)
    }

    pub(super) fn compress(
        &mut self,
        budget: u64,
        config: &EvictionConfig,
    ) -> Result<StageDecision, EvictionError> {
        let mut decision = StageDecision {
            stage: LadderStage::Compression,
            items: 0,
            reclaimed_tokens: 0,
        };
        let mut done = Vec::new();
        loop {
            if decision.reclaimed_tokens >= budget {
                return Ok(decision);
            }
            let Some(&index) = self
                .foldable()
                .iter()
                .find(|&&index| !done.contains(&index))
            else {
                return Ok(decision);
            };
            let before = events_tokens(std::slice::from_ref(&self.working[index]));
            let trimmed = soft_trim(
                &result_text(&self.working[index]),
                config.pruning.soft_head_bytes,
                config.pruning.soft_tail_bytes,
            );
            let replacement = RecordedEvent {
                seq: self.working[index].seq,
                event: Event::ToolResult {
                    call_id: call_of(&self.working[index]),
                    blocks: vec![ContentBlock::Text { text: trimmed }],
                },
            };
            let after = events_tokens(std::slice::from_ref(&replacement));
            self.working[index] = replacement;
            decision.reclaimed_tokens += before.saturating_sub(after);
            decision.items += 1;
            done.push(index);
        }
    }

    pub(super) fn truncate(&mut self, budget: u64) -> Result<StageDecision, EvictionError> {
        let mut decision = StageDecision {
            stage: LadderStage::Truncation,
            items: 0,
            reclaimed_tokens: 0,
        };
        loop {
            if decision.reclaimed_tokens >= budget {
                return Ok(decision);
            }
            let Some(index) = self.foldable().first().copied() else {
                return Ok(decision);
            };
            self.replace_at(index, true, &mut decision)?;
        }
    }
}

fn call_of(recorded: &RecordedEvent) -> String {
    match &recorded.event {
        Event::ToolResult { call_id, .. } | Event::ToolCall { call_id, .. } => call_id.clone(),
        _ => String::new(),
    }
}

fn is_folded(recorded: &RecordedEvent) -> bool {
    single_text_of(recorded).is_some_and(|text| text.starts_with("<folded_tool_output "))
}

fn is_condensate(recorded: &RecordedEvent) -> bool {
    match &recorded.event {
        Event::System { text } => text.starts_with("<active_plan "),
        _ => false,
    }
}

fn single_text_of(recorded: &RecordedEvent) -> Option<&str> {
    match &recorded.event {
        Event::ToolResult { blocks, .. } => match blocks.as_slice() {
            [ContentBlock::Text { text }] => Some(text),
            _ => None,
        },
        _ => None,
    }
}
