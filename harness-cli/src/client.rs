//! The client side of the daemon's typed protocol (todo 35): one
//! [`SessionTransport`] shape over two carriers — the WebSocket attach
//! socket (`/s/{sid}/attach?after=N`, the daemon's duplex primary) and
//! the interprocess local socket (newline-delimited JSON). Both are
//! full duplex speaking the SAME frame protocol: Hello, then replayed
//! frames, then HelloAck, then live events. The SSE endpoint is
//! deliberately NOT used: its read-only pump ends the stream as soon
//! as the attach goes idle, so a turn-taking client needs the duplex
//! socket. The CLI never duplicates daemon logic; it submits commands
//! and renders what streams back.

use std::path::PathBuf;

use futures_util::{SinkExt, StreamExt};
use harness_daemon::protocol::{
    Answer, ClientMsg, ErrorCode, PROTOCOL_VERSION, ServerMsg, SessionCommand, decode_server,
    encode_client,
};
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::tokio::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_tungstenite::tungstenite::Message;

use crate::error::CliError;

/// Replay threshold meaning "live events only": the `?after=` value
/// that skips all retained frames (the first message is then
/// HelloAck).
pub const LIVE_ONLY: u64 = u64::MAX;

/// Connection inputs shared by both carriers.
#[derive(Debug, Clone)]
pub struct Connect {
    /// Daemon base URL (`Some` selects the WebSocket carrier).
    pub url: Option<String>,
    /// Auth token when known (either carrier may run without one until
    /// the daemon answers 401).
    pub token: Option<String>,
    /// IPC socket name.
    pub ipc_name: String,
    /// Daemon data dir (IPC token file + session media).
    pub data_dir: PathBuf,
    /// Session id to bind.
    pub session: String,
    /// Replay threshold (`?after=` on WS; IPC always replays its ring).
    pub after: u64,
    /// Stable client id for the handshake.
    pub client_id: String,
}

/// One bound session over one carrier.
pub enum SessionTransport {
    /// WebSocket duplex.
    Http(HttpSession),
    /// Local-socket duplex.
    Ipc(IpcSession),
}

impl SessionTransport {
    /// Bind per the [`Connect`] inputs. Both carriers complete the
    /// Hello handshake here; replayed frames flow into the caller's
    /// read loop ahead of HelloAck (the resume contract).
    pub async fn connect(connect: &Connect) -> Result<Self, CliError> {
        match &connect.url {
            Some(base) => {
                let http = HttpSession::open(base, connect).await?;
                Ok(Self::Http(http))
            }
            None => {
                let ipc = IpcSession::open(connect).await?;
                Ok(Self::Ipc(ipc))
            }
        }
    }

    /// Submit a user turn.
    /// # Errors
    /// typed transport failures (unreachable/auth/protocol).
    pub async fn submit(&mut self, text: &str) -> Result<(), CliError> {
        self.send(&ClientMsg::Command {
            command: SessionCommand::Submit {
                text: text.to_owned(),
            },
        })
        .await
    }

    /// Answer a broadcast permission ask.
    /// # Errors
    /// typed transport failures.
    pub async fn answer(&mut self, ask_id: u64, answer: Answer) -> Result<(), CliError> {
        self.send(&ClientMsg::PermissionAnswer { ask_id, answer })
            .await
    }

    /// Request cancellation of the running turn.
    /// # Errors
    /// typed transport failures.
    pub async fn cancel(&mut self) -> Result<(), CliError> {
        self.send(&ClientMsg::Command {
            command: SessionCommand::Cancel,
        })
        .await
    }

    async fn send(&mut self, msg: &ClientMsg) -> Result<(), CliError> {
        match self {
            Self::Http(http) => http.send(msg).await,
            Self::Ipc(ipc) => ipc.send(msg).await,
        }
    }

    /// Next server message (`None` = stream ended).
    /// # Errors
    /// typed transport failures.
    pub async fn next_msg(&mut self) -> Result<Option<ServerMsg>, CliError> {
        match self {
            Self::Http(http) => http.next_msg().await,
            Self::Ipc(ipc) => ipc.next_msg().await,
        }
    }
}

fn map_error_msg(msg: ServerMsg) -> Result<Option<ServerMsg>, CliError> {
    match msg {
        ServerMsg::Error { code, message } => match code {
            ErrorCode::Auth => Err(CliError::Auth { detail: message }),
            code => Err(CliError::Protocol {
                detail: format!("{code:?}: {message}"),
            }),
        },
        other => Ok(Some(other)),
    }
}

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// WebSocket carrier: one duplex attach socket.
pub struct HttpSession {
    sink: futures_util::stream::SplitSink<WsStream, Message>,
    source: futures_util::stream::SplitStream<WsStream>,
}

impl HttpSession {
    async fn open(base: &str, connect: &Connect) -> Result<Self, CliError> {
        let ws_base = base.replacen("http://", "ws://", 1);
        let ws_base = ws_base.replacen("https://", "wss://", 1);
        let mut url = format!(
            "{ws_base}/s/{}/attach?after={}",
            connect.session, connect.after
        );
        if let Some(token) = &connect.token {
            url.push_str("&token=");
            url.push_str(&percent_encode(token));
        }
        let (stream, response) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(map_ws_error)?;
        let _ = response;
        let (sink, source) = stream.split();
        let mut session = Self { sink, source };
        session
            .send(&ClientMsg::Hello {
                version: PROTOCOL_VERSION,
                token: connect.token.clone().unwrap_or_default(),
                client_id: connect.client_id.clone(),
                session: Some(connect.session.clone()),
            })
            .await?;
        Ok(session)
    }

    async fn send(&mut self, msg: &ClientMsg) -> Result<(), CliError> {
        let frame = encode_client(msg).map_err(|error| CliError::Protocol {
            detail: error.to_string(),
        })?;
        self.sink
            .send(Message::Text(frame.into()))
            .await
            .map_err(|error| CliError::Unreachable {
                detail: error.to_string(),
            })
    }

    async fn next_msg(&mut self) -> Result<Option<ServerMsg>, CliError> {
        loop {
            let message =
                self.source
                    .next()
                    .await
                    .transpose()
                    .map_err(|error| CliError::Unreachable {
                        detail: error.to_string(),
                    })?;
            let Some(message) = message else {
                return Ok(None);
            };
            let bytes = match message {
                Message::Text(text) => text.as_bytes().to_vec(),
                Message::Binary(bytes) => bytes.to_vec(),
                Message::Close(_) => return Ok(None),
                _ => continue,
            };
            let msg = decode_server(&bytes).map_err(|error| CliError::Protocol {
                detail: error.to_string(),
            })?;
            return map_error_msg(msg);
        }
    }
}

fn map_ws_error(error: tokio_tungstenite::tungstenite::Error) -> CliError {
    if let tokio_tungstenite::tungstenite::Error::Http(response) = &error {
        match response.status().as_u16() {
            401 | 403 => {
                return CliError::Auth {
                    detail: format!("daemon refused the token ({})", response.status()),
                };
            }
            status => {
                return CliError::Protocol {
                    detail: format!("attach handshake returned {status}"),
                };
            }
        }
    }
    CliError::Unreachable {
        detail: error.to_string(),
    }
}

/// Percent-encode for the query string: unreserved characters pass,
/// everything else becomes `%XX`.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// IPC carrier: one duplex local-socket stream, newline JSON frames.
pub struct IpcSession {
    writer: tokio::io::WriteHalf<Stream>,
    reader: BufReader<tokio::io::ReadHalf<Stream>>,
}

impl IpcSession {
    async fn open(connect: &Connect) -> Result<Self, CliError> {
        let ns_name = connect
            .ipc_name
            .as_str()
            .to_ns_name::<GenericNamespaced>()
            .map_err(|error| CliError::Protocol {
                detail: format!("bad ipc name: {error}"),
            })?;
        let stream = Stream::connect(ns_name)
            .await
            .map_err(|error| CliError::Unreachable {
                detail: format!("local socket {}: {error}", connect.ipc_name),
            })?;
        let (reader, writer) = tokio::io::split(stream);
        let mut session = Self {
            writer,
            reader: BufReader::new(reader),
        };
        session
            .send(&ClientMsg::Hello {
                version: PROTOCOL_VERSION,
                token: connect.token.clone().unwrap_or_default(),
                client_id: connect.client_id.clone(),
                session: Some(connect.session.clone()),
            })
            .await?;
        Ok(session)
    }

    async fn send(&mut self, msg: &ClientMsg) -> Result<(), CliError> {
        let mut line = encode_client(msg).map_err(|error| CliError::Protocol {
            detail: error.to_string(),
        })?;
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .await
            .map_err(|error| CliError::Unreachable {
                detail: error.to_string(),
            })
    }

    async fn next_msg(&mut self) -> Result<Option<ServerMsg>, CliError> {
        let mut line = String::new();
        let read =
            self.reader
                .read_line(&mut line)
                .await
                .map_err(|error| CliError::Unreachable {
                    detail: error.to_string(),
                })?;
        if read == 0 {
            return Ok(None);
        }
        let msg = decode_server(line.trim().as_bytes()).map_err(|error| CliError::Protocol {
            detail: error.to_string(),
        })?;
        map_error_msg(msg)
    }
}
