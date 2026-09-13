//! /learn-style synthesis from external sources (URLs, directories,
//! videos): a typed [`SourceDescriptor`] plus an injected async
//! [`SourceFetcher`] seam (mocked in tests — the forge never performs
//! network I/O itself) feeding the same scanned, draft-tier proposal
//! pipeline as trajectory distillation.

use std::path::PathBuf;

use crate::forge::ForgeError;
use crate::forge::distill::{DistillModel, DraftRecord, SkillDistiller};
use crate::forge::quality::{SessionTrajectory, TaskOutcome, TrajectoryTurn, TurnRole};

/// Where synthesis content comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceDescriptor {
    /// A fetched document.
    Url(String),
    /// A directory of documents.
    Directory(PathBuf),
    /// A video (transcript fetched).
    Video(String),
}

impl SourceDescriptor {
    /// Human-readable descriptor (used in errors and audit surfaces).
    pub fn describe(&self) -> String {
        match self {
            Self::Url(url) => format!("url {url}"),
            Self::Directory(dir) => format!("directory {}", dir.display()),
            Self::Video(id) => format!("video {id}"),
        }
    }
}

/// Fetched source content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceContent {
    /// Content title.
    pub title: String,
    /// Content text (UNTRUSTED input to the forge).
    pub text: String,
}

/// Typed fetch seam. Production implements HTTP/filesystem/transcript
/// adapters behind it; tests implement deterministic mocks.
#[allow(async_fn_in_trait)]
pub trait SourceFetcher {
    /// Fetches one source. `async` because real fetches are I/O.
    async fn fetch(&self, source: &SourceDescriptor) -> Result<SourceContent, ForgeError>;
}

/// Fetches a source and distills it into a scanned, draft-tier skill
/// proposal. Quality filtering does not apply (content is not a
/// session); untrusted-output scanning does — always.
pub async fn synthesize_from_source<M: DistillModel, F: SourceFetcher>(
    fetcher: &F,
    distiller: &SkillDistiller<M>,
    source: &SourceDescriptor,
) -> Result<DraftRecord, ForgeError> {
    let content = fetcher.fetch(source).await?;
    distiller.distill_content(&content).await
}

pub(crate) fn content_trajectory(content: &SourceContent) -> SessionTrajectory {
    SessionTrajectory {
        session_id: format!("source:{}", content.title),
        turns: vec![
            TrajectoryTurn {
                role: TurnRole::User,
                text: content.title.clone(),
            },
            TrajectoryTurn {
                role: TurnRole::Assistant,
                text: content.text.clone(),
            },
        ],
        originating_prompts: vec![content.title.clone()],
        negative_prompts: Vec::new(),
        outcome: TaskOutcome::Success,
    }
}
