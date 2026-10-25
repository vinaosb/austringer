//! The compaction engine orchestration (plan todo 13). The run's
//! boundary types live in [`super::seam`]; this module owns the phase
//! order.
//!
//! One run: validate the stream (contiguous, monotonic — corrupt
//! input is typed, never folded) → emit the PreCompact hook (a System
//! marker; a visible runtime notice) → run the doctrine selection →
//! condense through the SAME-MODEL seam when one is registered and the
//! cause is not an overflow, else land on [`basic_compact`] (the
//! Cline rule: recovery never depends on another successful LLM call;
//! a condenser refusal — model mismatch or seam failure — lands there
//! too) → emit the PostCompact hook (a Condensation event carrying
//! the summary and `through_seq`) → persist segments + state → append
//! the telemetry record. The T11 transition rides the outcome for the
//! epoch ledger; the [`SealGuard`] rides it for postcompact
//! accountability. Cancellation polls at phase boundaries: a cancel
//! discards all artifacts cleanly and records the `canceled` outcome —
//! the run restarts from scratch.

use super::condense::{DoctrineCondenseRequest, effective_prompt};
use super::doctrine::{self, compacted_from};
use super::handoff::{HandoffBundle, latest_milestone};
use super::overflow::{ContextOverflow, basic_compact};
use super::telemetry::{SealGuard, SealStatus, TelemetryOutcome, TelemetryRecord, ThresholdBasis};
use super::{CompactedSession, CompactionConfig, CompactionError, validate_stream};
use crate::context::EpochTransition;
use crate::event::{Event, RecordedEvent};
use crate::eviction::events_tokens;

pub use super::seam::{
    CompactionCause, CompactionDeps, CompactionInput, CompactionResult, CompactionSink, HookError,
};

/// The compaction engine.
pub struct CompactionEngine {
    config: CompactionConfig,
}

impl CompactionEngine {
    /// An engine under `config`.
    #[must_use]
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// The engine's config.
    #[must_use]
    pub fn config(&self) -> &CompactionConfig {
        &self.config
    }

    /// Whether the configured threshold has fired for this usage.
    #[must_use]
    pub fn should_compact(&self, used_tokens: u64, window_tokens: u64) -> Option<ThresholdBasis> {
        self.config.threshold.fired(used_tokens, window_tokens)
    }

    /// Runs one compaction. See the module docs for the phase order.
    pub async fn compact(
        &self,
        input: CompactionInput<'_>,
        cause: CompactionCause,
        deps: &mut CompactionDeps<'_>,
    ) -> Result<CompactionResult, CompactionError> {
        validate_stream(input.events)?;
        let compaction_id = deps.segments.next_ordinal()?;
        let basis = self.basis_of(&cause, &input);
        let outcome = match cause {
            CompactionCause::Manual => TelemetryOutcome::Manual,
            CompactionCause::Auto | CompactionCause::Overflow(_) => TelemetryOutcome::Auto,
        };
        let before_tokens = events_tokens(input.events);
        let cause_name = match cause {
            CompactionCause::Manual => "manual",
            CompactionCause::Auto => "auto",
            CompactionCause::Overflow(_) => "overflow",
        };
        deps.sink
            .append(Event::System {
                text: format!(
                    "<pre_compact cause=\"{cause_name}\" basis=\"{}\" used=\"{before_tokens}\"/>",
                    basis.render()
                ),
            })
            .map_err(|error| CompactionError::Hook(error.to_string()))?;
        if deps.cancel.is_cancelling() {
            return Ok(self.canceled(compaction_id, basis, before_tokens, deps));
        }
        let compacted = self.compacted_session(&input, cause, deps).await;
        if deps.cancel.is_cancelling() {
            return Ok(self.canceled(compaction_id, basis, before_tokens, deps));
        }
        let through = compacted.through_seq;
        deps.sink
            .append(Event::Condensation {
                summary: compacted.summary.clone(),
                through_seq: through,
            })
            .map_err(|error| CompactionError::Hook(error.to_string()))?;
        let written =
            deps.segments
                .write_compaction(&compacted, input.events, self.config.segment_detail);
        let segments = match written {
            Ok(written) => written.files,
            Err(error) => {
                let failed = self.telemetry(
                    compaction_id,
                    TelemetryOutcome::Failed,
                    basis,
                    before_tokens,
                    0,
                    compacted.fallback_used,
                );
                let _ = deps.sink.append(failed.event());
                return Err(error.into());
            }
        };
        let telemetry = self.telemetry(
            compaction_id,
            outcome,
            basis,
            before_tokens,
            compacted.after_tokens,
            compacted.fallback_used,
        );
        deps.sink
            .append(telemetry.event())
            .map_err(|error| CompactionError::Hook(error.to_string()))?;
        let handoff = self.handoff(&input, deps, &compacted).await;
        Ok(CompactionResult {
            seal_guard: Some(SealGuard::new(compaction_id, through)),
            milestone: latest_milestone(input.events),
            epoch_transition: Some(EpochTransition::CompactionCompleted {
                through_seq: through,
            }),
            compacted: Some(compacted),
            telemetry,
            handoff,
            segments,
        })
    }

    /// The compacted session for this run: the deterministic fallback
    /// on overflow or condenser refusal, the doctrine path otherwise.
    async fn compacted_session(
        &self,
        input: &CompactionInput<'_>,
        cause: CompactionCause,
        deps: &mut CompactionDeps<'_>,
    ) -> CompactedSession {
        if let CompactionCause::Overflow(rejection) = cause {
            return basic_compact(
                input.events,
                input.tool_returns,
                &self.config.doctrine,
                &rejection,
            );
        }
        let overflow = ContextOverflow::new(input.used_tokens, input.window_tokens);
        let selection = doctrine::select(input.events, &self.config.doctrine);
        let span = dropped_prefix(input.events, &selection.head, &selection.retained);
        let request = DoctrineCondenseRequest {
            span: &span,
            session_model: input.model,
            prompt: effective_prompt(self.config.custom_condense_prompt.as_deref()),
        };
        if let Some(condenser) = deps.condenser.as_ref()
            && let Ok(summary) = condenser.condense(request).await
        {
            let summary = format!(
                "{}\n<model_summary>\n{summary}\n</model_summary>",
                selection.render()
            );
            let mut retained = selection.head.clone();
            retained.extend(selection.retained.clone());
            compacted_from(summary, input.events, dedup_by_seq(retained), false)
        } else {
            // No condenser, model mismatch, or seam failure: recovery
            // never depends on another successful LLM call.
            basic_compact(
                input.events,
                input.tool_returns,
                &self.config.doctrine,
                &overflow,
            )
        }
    }

    /// The optional handoff: second-model distillation when an
    /// extractor is injected, the deterministic bundle otherwise; a
    /// seam failure keeps the deterministic bundle (a handoff is a
    /// complement, never a dependency).
    async fn handoff(
        &self,
        input: &CompactionInput<'_>,
        deps: &mut CompactionDeps<'_>,
        compacted: &CompactedSession,
    ) -> Option<HandoffBundle> {
        if !self.config.produce_handoff {
            return None;
        }
        match deps.extractor.clone() {
            Some(extractor) => match extractor.extract_thread(input.events).await {
                Ok(bundle) => Some(bundle),
                Err(_) => Some(HandoffBundle::from_compaction(compacted)),
            },
            None => Some(HandoffBundle::from_compaction(compacted)),
        }
    }

    fn basis_of(&self, cause: &CompactionCause, input: &CompactionInput<'_>) -> ThresholdBasis {
        match cause {
            CompactionCause::Overflow(rejection) => {
                ThresholdBasis::TokensUsed(rejection.used_tokens)
            }
            _ => self
                .config
                .threshold
                .fired(input.used_tokens, input.window_tokens)
                .unwrap_or(ThresholdBasis::Percent(75)),
        }
    }

    fn canceled(
        &self,
        compaction_id: u64,
        basis: ThresholdBasis,
        before_tokens: u64,
        deps: &mut CompactionDeps<'_>,
    ) -> CompactionResult {
        let telemetry = self.telemetry(
            compaction_id,
            TelemetryOutcome::Canceled,
            basis,
            before_tokens,
            before_tokens,
            false,
        );
        // Best-effort: the session log may itself be why we cancel.
        let _ = deps.sink.append(telemetry.event());
        CompactionResult::canceled(telemetry)
    }

    fn telemetry(
        &self,
        compaction_id: u64,
        outcome: TelemetryOutcome,
        basis: ThresholdBasis,
        before_tokens: u64,
        after_tokens: u64,
        fallback_used: bool,
    ) -> TelemetryRecord {
        TelemetryRecord {
            compaction_id,
            outcome,
            basis,
            before_tokens,
            after_tokens,
            fallback_used,
            seal: SealStatus::Intact,
        }
    }
}

/// The events the compacted summary REPLACES: everything before the
/// retained window minus the protected head.
fn dropped_prefix(
    events: &[RecordedEvent],
    head: &[RecordedEvent],
    retained: &[RecordedEvent],
) -> Vec<RecordedEvent> {
    let first_retained = retained.first().map_or(u64::MAX, |event| event.seq.0);
    let head_end = head.last().map_or(0, |event| event.seq.0);
    events
        .iter()
        .filter(|recorded| recorded.seq.0 < first_retained && recorded.seq.0 > head_end)
        .cloned()
        .collect()
}

fn dedup_by_seq(events: Vec<RecordedEvent>) -> Vec<RecordedEvent> {
    let mut seen = std::collections::BTreeSet::new();
    events
        .into_iter()
        .filter(|recorded| seen.insert(recorded.seq))
        .collect()
}
