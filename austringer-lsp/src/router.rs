//! The multi-server router: one client manager routing files to their
//! configured language servers, spawned lazily on first use of a language,
//! with bridge-priority (IDE-as-toolbox: when a bridge covers a language,
//! its native server is never spawned) and crash-restart-degrade recovery.

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use crate::config::{Language, LspConfig, language_for_extension};
use crate::error::LspError;
use crate::handshake;
use crate::lock;
use crate::native::{PositionEncoding, ServerHandle};
use crate::provider::{Answer, DocumentPosition, LspProvider};
use crate::recovery::{Ensure, Slot};

pub use crate::events::RouterEvent;

/// Reader half of an attached transport.
pub type BoxRead = Box<dyn AsyncRead + Unpin + Send>;
/// Writer half of an attached transport.
pub type BoxWrite = Box<dyn AsyncWrite + Unpin + Send>;

/// Produces one transport pair per invocation — the (re)spawn seam. The
/// router falls back to the config's `ServerCommand` when no factory is
/// installed; factories serve in-process servers, tests, and future
/// bridge transports.
pub type ServerFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = std::io::Result<(BoxRead, BoxWrite)>> + Send>>
        + Send
        + Sync,
>;

/// Which navigation question is being asked.
#[derive(Clone, Copy)]
pub(crate) enum AskKind {
    /// `textDocument/definition`.
    Definition,
    /// `textDocument/references`.
    References,
    /// `textDocument/hover`.
    Hover,
}

pub(crate) struct Shared {
    pub(crate) config: LspConfig,
    pub(crate) bridge: StdMutex<Option<Arc<dyn LspProvider>>>,
    pub(crate) slots: StdMutex<HashMap<Language, Arc<Slot>>>,
    pub(crate) factories: StdMutex<HashMap<Language, ServerFactory>>,
    pub(crate) events: mpsc::Sender<RouterEvent>,
    events_rx: StdMutex<Option<mpsc::Receiver<RouterEvent>>>,
}

/// The headless navigation client. Cheap to clone; clones share slots and
/// events.
#[derive(Clone)]
pub struct LspRouter {
    pub(crate) shared: Arc<Shared>,
}

impl LspRouter {
    /// Creates the router. Servers spawn lazily on first use per language.
    #[must_use]
    pub fn new(config: LspConfig) -> Self {
        let (events, events_rx) = mpsc::channel(256);
        Self {
            shared: Arc::new(Shared {
                config,
                bridge: StdMutex::new(None),
                slots: StdMutex::new(HashMap::new()),
                factories: StdMutex::new(HashMap::new()),
                events,
                events_rx: StdMutex::new(Some(events_rx)),
            }),
        }
    }

    /// Registers the IDE-bridge provider. When `prefer_bridge` is set (the
    /// default) and the bridge covers a language, requests route to it and
    /// the native server for that language is never spawned. Do NOT
    /// register the router itself (or a clone) — that recurses.
    pub fn set_bridge(&self, provider: Arc<dyn LspProvider>) {
        *lock(&self.shared.bridge) = Some(provider);
    }

    /// Installs a transport factory for one language (in-process servers,
    /// tests, future IDE-bridge transports). Takes priority over the
    /// config's spawn command and participates in restarts.
    pub fn set_server_factory(&self, language: Language, factory: ServerFactory) {
        lock(&self.shared.factories).insert(language, factory);
    }

    /// Attaches one already-connected server for a language and runs the
    /// initialize handshake now (single-shot convenience over
    /// [`LspRouter::set_server_factory`]).
    pub async fn attach_server(
        &self,
        language: Language,
        read: BoxRead,
        write: BoxWrite,
    ) -> Result<(), LspError> {
        let handle = Arc::new(ServerHandle::attach(read, write, None));
        let slot = self.slot_for(language);
        let events = self.shared.events.clone();
        handle.set_on_death(Box::new(move || slot.mark_crashed(&events)));
        let encoding = handshake::initialize(
            &handle,
            &self.shared.config.root,
            language,
            self.shared.config.initialize_timeout,
        )
        .await?;
        handshake::initialized(&handle).await?;
        self.slot_for(language).force_ready(handle, encoding);
        Ok(())
    }

    /// Languages with a live (Ready) server right now.
    #[must_use]
    pub fn server_languages(&self) -> Vec<Language> {
        let slots = lock(&self.shared.slots);
        let mut languages: Vec<Language> = slots
            .iter()
            .filter(|(_, slot)| slot.ready_handle().is_some())
            .map(|(language, _)| *language)
            .collect();
        languages.sort();
        languages
    }

    /// The OS pid of a language's live spawned server, if any (attached
    /// transports have none).
    #[must_use]
    pub fn server_pid(&self, language: Language) -> Option<u32> {
        let slots = lock(&self.shared.slots);
        slots
            .get(&language)
            .and_then(|slot| slot.ready_handle())
            .and_then(|handle| handle.pid())
    }

    /// The router event stream (spawn/crash/restart/backoff/degrade/rss).
    /// Takes the single receiver: first call gets it, later calls `None`.
    pub fn take_events(&self) -> Option<mpsc::Receiver<RouterEvent>> {
        lock(&self.shared.events_rx).take()
    }

    /// Gracefully shuts down every live server and resets the slots.
    pub async fn shutdown(&self) -> Result<(), LspError> {
        let live: Vec<(Arc<Slot>, Arc<ServerHandle>)> = {
            let slots = lock(&self.shared.slots);
            slots
                .values()
                .filter_map(|slot| slot.ready_handle().map(|handle| (Arc::clone(slot), handle)))
                .collect()
        };
        for (slot, handle) in live {
            let _ =
                handshake::shutdown_graceful(&handle, self.shared.config.initialize_timeout).await;
            slot.reset();
        }
        Ok(())
    }

    /// Starts the RSS-sampled bounded-LSP guard (forge §26): periodically
    /// samples each live spawned server and kills runaways over the
    /// configured cap; restarts flow through the normal crash machinery.
    /// Must be called inside a tokio runtime. No-op-per-tick when disabled
    /// or when only attached (pid-less) servers exist.
    pub fn start_guard(&self, probe: Arc<dyn crate::guard::RssProbe>) {
        let shared = Arc::clone(&self.shared);
        let config = self.shared.config.rss;
        let events = self.shared.events.clone();
        let provide = move || {
            let slots = lock(&shared.slots);
            slots
                .iter()
                .filter_map(|(language, slot)| {
                    let handle = slot.ready_handle()?;
                    Some(crate::guard::GuardTarget::new(
                        *language,
                        handle.pid(),
                        move || handle.kill(),
                    ))
                })
                .collect()
        };
        tokio::spawn(crate::guard::run_guard_loop(provide, probe, config, events));
    }

    pub(crate) fn language_for_file(&self, file: &Path) -> Result<Language, LspError> {
        let ext = file
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        language_for_extension(&ext).ok_or_else(|| LspError::NoServer {
            detail: format!(".{ext} files"),
        })
    }

    pub(crate) fn bridge_for(&self, language: Language) -> Option<Arc<dyn LspProvider>> {
        if !self.shared.config.prefer_bridge {
            return None;
        }
        lock(&self.shared.bridge)
            .clone()
            .filter(|b| b.languages().contains(&language))
    }

    pub(crate) fn slot_for(&self, language: Language) -> Arc<Slot> {
        let mut slots = lock(&self.shared.slots);
        slots
            .entry(language)
            .or_insert_with(|| Arc::new(Slot::new(language)))
            .clone()
    }

    pub(crate) async fn ensure(
        &self,
        language: Language,
        slot: &Arc<Slot>,
    ) -> Result<Ensure, LspError> {
        slot.ensure_ready(&self.shared.config, &self.shared.events, || {
            self.bring_language(language, Arc::clone(slot))
        })
        .await
    }

    async fn bring_language(
        &self,
        language: Language,
        slot: Arc<Slot>,
    ) -> Result<(Arc<ServerHandle>, PositionEncoding), LspError> {
        let handle = self.transport_for(language, slot).await?;
        let encoding = handshake::initialize(
            &handle,
            &self.shared.config.root,
            language,
            self.shared.config.initialize_timeout,
        )
        .await?;
        handshake::initialized(&handle).await?;
        Ok((handle, encoding))
    }

    async fn transport_for(
        &self,
        language: Language,
        slot: Arc<Slot>,
    ) -> Result<Arc<ServerHandle>, LspError> {
        let factory = lock(&self.shared.factories).get(&language).cloned();
        let handle = match factory {
            Some(make) => {
                let (read, write) = make().await.map_err(LspError::Io)?;
                ServerHandle::attach(read, write, None)
            }
            None => {
                let command = self.shared.config.servers.get(&language).ok_or_else(|| {
                    LspError::NoServer {
                        detail: language.name().to_owned(),
                    }
                })?;
                ServerHandle::spawn(
                    command,
                    self.shared.config.stderr_hook.as_ref(),
                    self.shared.config.framing,
                )
                .map_err(|_| LspError::ServerUnavailable {
                    language,
                    program: command.program.clone(),
                })?
            }
        };
        let handle = Arc::new(handle);
        let events = self.shared.events.clone();
        handle.set_on_death(Box::new(move || slot.mark_crashed(&events)));
        Ok(handle)
    }
}

#[async_trait]
impl LspProvider for LspRouter {
    fn languages(&self) -> &[Language] {
        Language::ALL
    }

    async fn goto_definition(&self, position: DocumentPosition) -> Result<Answer, LspError> {
        self.ask(&position, "textDocument/definition", AskKind::Definition)
            .await
    }

    async fn find_references(&self, position: DocumentPosition) -> Result<Answer, LspError> {
        self.ask(&position, "textDocument/references", AskKind::References)
            .await
    }

    async fn hover(&self, position: DocumentPosition) -> Result<Answer, LspError> {
        self.ask(&position, "textDocument/hover", AskKind::Hover)
            .await
    }
}
