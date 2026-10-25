//! The model-mediated arm of the resolve pipeline (plan M10).
//!
//! The capability check is the gate, always (MUST-NOT: a model-mediated
//! projection without one): image sources need `vision` plus a queued
//! scene description, audio sources need `audio_input` plus a queued
//! transcript — the M7 flag semantics mirrored by
//! [`austringer_core::media_delivery::DeliveryCapabilities`]. The session
//! model is considered first, then the configured capable utility route
//! (T42 wiring at the daemon edge supplies both as data).
//!
//! The projection call is one single-block user turn with a per-kind
//! focus instruction under a wall-clock timeout. A model that answers
//! with tool calls or empty text has NOT projected — that is a typed
//! failure, never a fabricated transcript.

use austringer_core::agent_loop::{AssistantTurn, CancelToken, InferenceRequest};
use austringer_core::event::{ContentBlock, Event, ProjectionKind, RecordedEvent, Sequence};

use super::PROJECTION_PROMPT_VERSION;
use super::{
    MediatedProjection, Mediation, Pending, ProjectionOutcome, SessionStores, Unprojectable,
};

pub(super) async fn mediate(
    stores: &SessionStores,
    pending: &Pending<'_>,
    mediation: &Mediation<'_>,
) -> ProjectionOutcome {
    let block = pending.block;
    let Some(candidate) = mediation
        .session
        .filter(|model| mediates(block, pending.kind, model))
        .or_else(|| {
            mediation
                .utility
                .filter(|model| mediates(block, pending.kind, model))
        })
    else {
        return pending.unprojectable(Unprojectable::NoProjector);
    };
    let producer = format!("model:{}:{PROJECTION_PROMPT_VERSION}", candidate.model_id);
    if pending.cached(&stores.derived, &producer) {
        return ProjectionOutcome::Resolved {
            source: pending.source(),
            kind: pending.kind,
            producer,
        };
    }
    let events = [RecordedEvent {
        seq: Sequence::FIRST,
        event: Event::User {
            blocks: vec![block.clone()],
        },
    }];
    let request = InferenceRequest {
        events: &events,
        focus_block: instruction(pending.kind),
    };
    let mut inference = (candidate.factory)();
    let answer = tokio::time::timeout(
        candidate.timeout,
        inference.infer(&request, &CancelToken::new()),
    )
    .await;
    let text = match answer {
        Err(_elapsed) => return pending.unprojectable(Unprojectable::Timeout),
        Ok(Err(error)) => {
            return pending.unprojectable(Unprojectable::ModelMediation(error.to_string()));
        }
        Ok(Ok(turn)) => match projection_text(&turn) {
            Some(text) => text,
            None => {
                return pending.unprojectable(Unprojectable::ModelMediation(
                    "the model returned no projection text (empty or tool-call turn)".to_owned(),
                ));
            }
        },
    };
    pending.store(&stores.derived, &producer, &text)
}

fn mediates(block: &ContentBlock, kind: ProjectionKind, model: &MediatedProjection) -> bool {
    let mime = match block {
        ContentBlock::Image(image) => image.mime.as_str(),
        ContentBlock::Media(media) => media.mime.as_str(),
        ContentBlock::Text { .. } => return false,
    };
    let major = mime.split_once('/').map_or("", |(head, _)| head);
    match (major, kind) {
        ("image", ProjectionKind::SceneDescription) => model.caps.vision,
        ("audio", ProjectionKind::Transcript) => model.caps.audio_input,
        _ => false,
    }
}

fn instruction(kind: ProjectionKind) -> &'static str {
    match kind {
        ProjectionKind::SceneDescription => {
            "Describe the attached image factually and completely for a text-only reader: \
             subject, layout, colors, counts, and any visible text. Output only the description."
        }
        ProjectionKind::Transcript => {
            "Transcribe the attached audio verbatim and completely. Output only the transcript."
        }
        ProjectionKind::TextProjection | ProjectionKind::Thumbnail => {
            "Project the attached media to bounded factual text for a text-only reader. \
             Output only the projection."
        }
    }
}

fn projection_text(turn: &AssistantTurn) -> Option<String> {
    if !turn.tool_calls.is_empty() {
        return None;
    }
    let text: String = turn
        .blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image(_) | ContentBlock::Media(_) => None,
        })
        .collect();
    (!text.trim().is_empty()).then_some(text)
}
