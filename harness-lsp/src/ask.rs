//! The navigation ask pipeline: bridge-first routing, lazy server bring-up
//! with crash retry, document sync (didOpen/didChange), wire→typed-Answer
//! translation, and the search-fallback degradation result.

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::error::LspError;
use crate::native::{PositionEncoding, ServerHandle};
use crate::parse;
use crate::position::{line_at, utf16_col_to_utf8};
use crate::provider::{Answer, DocumentPosition, HoverText, Location, SEARCH_FALLBACK_HINT};
use crate::recovery::{Ensure, Slot};
use crate::router::{AskKind, LspRouter};
use crate::uri::{file_to_uri, uri_to_file};

impl LspRouter {
    /// Notifies the owning server of a full-text document change
    /// (stale-state seam: subsequent requests see fresh content). When the
    /// server was (re)spawned and has not seen the document yet, this is a
    /// didOpen carrying the new text instead of a didChange.
    pub async fn notify_did_change(&self, file: &Path, text: &str) -> Result<(), LspError> {
        let language = self.language_for_file(file)?;
        if self.bridge_for(language).is_some() {
            return Ok(()); // bridges own their sync; nothing to notify
        }
        let slot = self.slot_for(language);
        if let Ensure::Ready(handle, _encoding) = self.ensure(language, &slot).await? {
            let Some(uri) = file_to_uri(file) else {
                return Err(LspError::NoServer {
                    detail: file.display().to_string(),
                });
            };
            let already_open = slot.docs.text(file).is_some();
            let (method, params) = if already_open {
                let version = slot.docs.change(file, text.to_owned());
                (
                    "textDocument/didChange",
                    json!({
                        "textDocument": {"uri": uri, "version": version},
                        "contentChanges": [{"text": text}]
                    }),
                )
            } else {
                let version = slot.docs.open(file, text.to_owned());
                (
                    "textDocument/didOpen",
                    json!({
                        "textDocument": {"uri": uri, "languageId": language.id(),
                                         "version": version, "text": text}
                    }),
                )
            };
            handle.notify(method, params).await?;
        }
        Ok(())
    }

    pub(crate) async fn ask(
        &self,
        position: &DocumentPosition,
        method: &'static str,
        kind: AskKind,
    ) -> Result<Answer, LspError> {
        let language = self.language_for_file(&position.file)?;
        if let Some(bridge) = self.bridge_for(language) {
            return match kind {
                AskKind::Definition => bridge.goto_definition(position.clone()).await,
                AskKind::References => bridge.find_references(position.clone()).await,
                AskKind::Hover => bridge.hover(position.clone()).await,
            };
        }
        let slot = self.slot_for(language);
        let mut retries_left = 1u32;
        loop {
            match self.ensure(language, &slot).await? {
                Ensure::Degraded(reason) => {
                    return Ok(Answer::DegradedToSearch {
                        reason,
                        hint: SEARCH_FALLBACK_HINT,
                    });
                }
                Ensure::Ready(handle, encoding) => {
                    self.ensure_open(&slot, &handle, language, &position.file)
                        .await?;
                    let params = self.request_params(position, kind);
                    match handle
                        .request(method, params, self.shared.config.request_timeout)
                        .await
                    {
                        Ok(value) => {
                            return Ok(self.translate(&value, encoding, kind, &slot));
                        }
                        Err(err) if is_transport_class(&err) && retries_left > 0 => {
                            retries_left -= 1;
                            slot.mark_crashed(&self.shared.events);
                        }
                        Err(err) => return Err(err),
                    }
                }
            }
        }
    }

    async fn ensure_open(
        &self,
        slot: &Arc<Slot>,
        handle: &Arc<ServerHandle>,
        language: crate::config::Language,
        file: &Path,
    ) -> Result<(), LspError> {
        if slot.docs.text(file).is_some() {
            return Ok(());
        }
        let text = self.read_bounded(file)?;
        let version = slot.docs.open(file, text.clone());
        let Some(uri) = file_to_uri(file) else {
            return Err(LspError::NoServer {
                detail: file.display().to_string(),
            });
        };
        handle
            .notify(
                "textDocument/didOpen",
                json!({
                    "textDocument": {"uri": uri, "languageId": language.id(),
                                     "version": version, "text": text}
                }),
            )
            .await
    }

    pub(crate) fn read_bounded(&self, file: &Path) -> Result<String, LspError> {
        let size = std::fs::metadata(file).map_err(LspError::Io)?.len();
        if size > self.shared.config.file_text_cap_bytes {
            return Err(LspError::FileTooLarge {
                path: file.to_path_buf(),
                size_bytes: size,
                cap_bytes: self.shared.config.file_text_cap_bytes,
            });
        }
        std::fs::read_to_string(file).map_err(LspError::Io)
    }

    fn request_params(&self, position: &DocumentPosition, kind: AskKind) -> Value {
        let Some(uri) = file_to_uri(&position.file) else {
            return Value::Null;
        };
        let mut params = json!({
            "textDocument": {"uri": uri},
            "position": {"line": position.line.saturating_sub(1),
                          "character": position.column.saturating_sub(1)}
        });
        if let AskKind::References = kind {
            params["context"] = json!({"includeDeclaration": true});
        }
        params
    }

    fn translate(
        &self,
        value: &Value,
        encoding: PositionEncoding,
        kind: AskKind,
        slot: &Arc<Slot>,
    ) -> Answer {
        match kind {
            AskKind::Hover => {
                Answer::Hover(parse::hover_text(value).unwrap_or_else(|| HoverText::new("")))
            }
            AskKind::Definition | AskKind::References => Answer::Locations(
                parse::wire_locations(value)
                    .iter()
                    .map(|wire| self.to_location(wire, encoding, slot))
                    .collect(),
            ),
        }
    }

    fn to_location(
        &self,
        wire: &parse::WireLoc,
        encoding: PositionEncoding,
        slot: &Arc<Slot>,
    ) -> Location {
        let file = uri_to_file(&wire.uri);
        let mut column = wire.char0;
        if encoding == PositionEncoding::Utf16 {
            let text = slot
                .docs
                .text(&file)
                .or_else(|| self.read_bounded(&file).ok())
                .unwrap_or_default();
            let line_text = line_at(&text, wire.line0 as usize);
            // Columns arrive as UTF-16 code units; convert to a char index
            // before the +1 to the 1-based public convention.
            column = utf16_col_to_utf8(line_text, wire.char0 as usize);
        }
        Location {
            file,
            line: wire.line0.saturating_add(1),
            column: column.saturating_add(1),
        }
    }
}

pub(crate) fn is_transport_class(err: &LspError) -> bool {
    matches!(
        err,
        LspError::Io(_) | LspError::MalformedFrame { .. } | LspError::RequestTimeout { .. }
    )
}
