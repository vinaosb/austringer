//! The submit-intake gates: cost-cap refusal, skill routing, media-ref
//! resolution, and `/launch` dispatch — everything that decides whether
//! a command BECOMES a turn (the todo-6/todo-33-named extraction from
//! the hosting module; the `SessionHost` struct and hub surface live in
//! [`super`]).

use std::sync::Arc;

use austringer_core::event::{ContentBlock, MediaBlock, MediaRef, MediaStore};

use super::{CommandRejected, HostCommand, SessionHost};
use crate::protocol::ServerMsg;

impl SessionHost {
    /// Intake for transports: Steer resolves IMMEDIATELY (journal +
    /// shared queue — the loop's Steer stage drains it mid-run; the
    /// engine driver is busy inside `run()` and cannot process it),
    /// SubmitMedia resolves its refs against the session media store
    /// (typed refusal on unknown hashes, MediaResult receipts to v2
    /// wires), `/launch` starts a background orchestration launch
    /// (todo 8; the submit itself is the operator authorization),
    /// while plain Submits/SubmitBlocks/Cancel queue to the driver.
    /// Turn-starting submits pass the cost-cap gate first (todo 33):
    /// a stopped session refuses NEW spend typed, naming the cap.
    pub async fn handle_command(
        self: &Arc<Self>,
        command: HostCommand,
    ) -> Result<(), CommandRejected> {
        match &command {
            HostCommand::Steer(text) => {
                self.record_signal(text);
                if let Ok(mut queue) = self.steer_queue.lock() {
                    queue.push_back(text.clone());
                }
            }
            HostCommand::Submit(text) => {
                self.refuse_at_cost_cap()?;
                if text == "/launch" || text.starts_with("/launch ") {
                    let argument = text.strip_prefix("/launch ").unwrap_or_default();
                    return crate::launch::start_launch(self, argument)
                        .map_err(CommandRejected::LaunchRefused);
                }
                let queued = self.route_skills(text)?;
                let _ = self.cmd_tx.send(queued).await;
            }
            HostCommand::SubmitMedia { text, media } => {
                self.refuse_at_cost_cap()?;
                let blocks = self.resolve_media_blocks(text, media)?;
                // Plan M10: needed projections resolve BEFORE the turn
                // queues, so the delivery cache inlines at request-build
                // time; unprojectable media queue Event::System notices.
                if let Some(intake) = self.projection.get() {
                    intake.enqueue_notices(&blocks).await;
                }
                self.queue_blocks(blocks, media);
            }
            HostCommand::SubmitBlocks(_) => {
                self.refuse_at_cost_cap()?;
                let _ = self.cmd_tx.send(command).await;
            }
            HostCommand::Cancel => {
                let _ = self.cmd_tx.send(command).await;
            }
        }
        Ok(())
    }

    /// The cost-cap intake gate (todo 33): a session whose ledger is at
    /// or over its cap refuses turn-starting submits typed, naming the
    /// cap. Steer and Cancel stay open (steering a finishing turn and
    /// cancelling are not new spend); hosts without a ledger pass.
    fn refuse_at_cost_cap(&self) -> Result<(), CommandRejected> {
        if let Some(ledger) = self.cost_ledger.get()
            && let Err(reason) = ledger.preflight()
        {
            return Err(CommandRejected::CostCapReached(reason));
        }
        Ok(())
    }

    /// The skills submit-lane gate (todo 6): every Submit passes the
    /// trust/consent gates — `/skill` commands AND trigger matches.
    /// Refusals are typed and carry the quarantine tier; an activated
    /// skill's context rides the turn as typed blocks while untouched
    /// submits queue in their original form. A host with no runtime
    /// bound (built outside the daemon factory) has no skills surface
    /// at all: `/skill` fails closed, plain text passes untouched.
    fn route_skills(&self, text: &str) -> Result<HostCommand, CommandRejected> {
        let is_skill_command = text.starts_with("/skill ");
        let Some(runtime) = self.skills.get() else {
            return if is_skill_command {
                let name = text
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                Err(CommandRejected::SkillRefused(
                    crate::skills::SkillRefusal::NotBound { name },
                ))
            } else {
                Ok(HostCommand::Submit(text.to_owned()))
            };
        };
        let args = text
            .strip_prefix("/skill ")
            .and_then(|rest| rest.split_once(' ').map(|(_, args)| args))
            .unwrap_or_default();
        match runtime.route_submit(text) {
            crate::skills::SubmitRouting::Pass => Ok(HostCommand::Submit(text.to_owned())),
            crate::skills::SubmitRouting::Refused(refusal) => {
                Err(CommandRejected::SkillRefused(refusal))
            }
            crate::skills::SubmitRouting::Activate(skill) => {
                Ok(HostCommand::SubmitBlocks(vec![ContentBlock::Text {
                    text: crate::skills::SkillsRuntime::render_context(&skill, args),
                }]))
            }
        }
    }

    /// Resolve uploaded refs into typed media blocks: each ref must
    /// exist in the session media store (unknown hash = typed refusal);
    /// the mime + format metadata come from probing the stored bytes.
    fn resolve_media_blocks(
        &self,
        text: &str,
        media: &[MediaRef],
    ) -> Result<Vec<ContentBlock>, CommandRejected> {
        if text.is_empty() && media.is_empty() {
            return Err(CommandRejected::Empty);
        }
        let mut blocks = Vec::new();
        if !text.is_empty() {
            blocks.push(ContentBlock::Text {
                text: text.to_owned(),
            });
        }
        let Some(dir) = self.media_dir.get().cloned() else {
            return Err(match media.first() {
                Some(reference) => CommandRejected::UnknownMedia(reference.hash.to_ref()),
                None => CommandRejected::Empty,
            });
        };
        let store = MediaStore::open(&dir).map_err(|_| {
            CommandRejected::UnknownMedia(
                media
                    .first()
                    .map_or_else(String::new, |reference| reference.hash.to_ref()),
            )
        })?;
        for reference in media {
            let bytes = store
                .load(&reference.hash)
                .map_err(|_| CommandRejected::UnknownMedia(reference.hash.to_ref()))?
                .ok_or_else(|| CommandRejected::UnknownMedia(reference.hash.to_ref()))?;
            let (mime, format_note) = crate::media::probe_mime(&bytes);
            let mut meta = std::collections::BTreeMap::new();
            meta.insert("format".to_owned(), format_note);
            let block = MediaBlock::new(mime, *reference, meta)
                .map_err(|_| CommandRejected::UnknownMedia(reference.hash.to_ref()))?;
            blocks.push(ContentBlock::Media(block));
        }
        Ok(blocks)
    }

    fn queue_blocks(&self, blocks: Vec<ContentBlock>, media: &[MediaRef]) {
        for reference in media {
            self.with_hub(|hub| {
                hub.broadcast(ServerMsg::MediaResult {
                    producer: "daemon-media-intake".into(),
                    artifact: *reference,
                    diff_summary: "media appended to user event".into(),
                })
            });
        }
        let _ = self.cmd_tx.try_send(HostCommand::SubmitBlocks(blocks));
    }
}
