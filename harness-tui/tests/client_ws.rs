//! Client-task tests against a scripted WebSocket server: handshake,
//! bounded malformed input, drop-and-reconnect across a flaky link
//! (×3), and the park-until-manual-reconnect path.

mod common;

use std::time::Duration;

use common::{wait_until, wire};
use futures_util::{SinkExt, StreamExt};
use harness_daemon::protocol::{
    ClientMsg, Frame, ServerMsg, SessionEvent, decode_client, encode_server,
};
use harness_tui::app::{App, Conn};
use harness_tui::client::{ClientCmd, ClientError, ClientEvent};

type Socket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;
type ServerError = Box<dyn std::error::Error + Send + Sync>;

async fn read_hello(socket: &mut Socket) -> ClientMsg {
    loop {
        match socket.next().await {
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                if let Ok(msg) = decode_client(text.as_bytes()) {
                    return msg;
                }
            }
            Some(Ok(_)) => {}
            other => panic!("expected hello, got {other:?}"),
        }
    }
}

async fn send_server(socket: &mut Socket, msg: &ServerMsg) {
    let text = encode_server(msg).unwrap_or_default();
    let _ = socket
        .send(tokio_tungstenite::tungstenite::Message::Text(text.into()))
        .await;
}

async fn ephemeral() -> (tokio::net::TcpListener, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap_or_else(|_| unreachable!());
    let port = listener
        .local_addr()
        .map(|addr| addr.port())
        .unwrap_or_default();
    (listener, format!("127.0.0.1:{port}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_streams_events_and_delivers_sends() {
    let (listener, base) = ephemeral().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        let hello = read_hello(&mut socket).await;
        send_server(
            &mut socket,
            &ServerMsg::HelloAck {
                version: 1,
                last_event_id: 0,
            },
        )
        .await;
        send_server(
            &mut socket,
            &ServerMsg::Event {
                frame: Frame {
                    id: 1,
                    event: SessionEvent::TurnStarted { turn: 1 },
                },
            },
        )
        .await;
        let ping = socket.next().await;
        Result::<_, ServerError>::Ok((hello, ping))
    });

    let wired = wire(&base, "s1", "tok");
    let mut app = App::new(100_000);
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| a.turn_running(),
            Duration::from_secs(5)
        ),
        "attached + streamed turn event; conn {:?}",
        app.conn
    );
    assert!(matches!(app.conn, Conn::Attached));

    let _ = wired.cmd.send(ClientCmd::Send(ClientMsg::Ping)).await;
    let _ = wired.cmd.send(ClientCmd::Shutdown).await;
    let joined = server.await;
    let (hello, ping) = joined
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_else(|| panic!("server task failed"));
    assert!(
        matches!(hello, ClientMsg::Hello { version: 1, .. }),
        "handshake is a typed v1 hello"
    );
    assert!(ping.is_some(), "the UI-loop send reached the server");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flaky_link_dropped_three_times_reconnects_each_time() {
    let (listener, base) = ephemeral().await;
    let server = tokio::spawn(async move {
        let mut connections = 0u32;
        while connections < 3 {
            let (stream, _) = listener.accept().await?;
            let mut socket = tokio_tungstenite::accept_async(stream).await?;
            read_hello(&mut socket).await;
            connections += 1;
            send_server(
                &mut socket,
                &ServerMsg::HelloAck {
                    version: 1,
                    last_event_id: u64::from(connections),
                },
            )
            .await;
            if connections == 1 {
                send_server(
                    &mut socket,
                    &ServerMsg::Event {
                        frame: Frame {
                            id: 1,
                            event: SessionEvent::TurnStarted { turn: 1 },
                        },
                    },
                )
                .await;
            }
            drop(socket);
        }
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        read_hello(&mut socket).await;
        send_server(
            &mut socket,
            &ServerMsg::HelloAck {
                version: 1,
                last_event_id: 2,
            },
        )
        .await;
        send_server(
            &mut socket,
            &ServerMsg::Event {
                frame: Frame {
                    id: 2,
                    event: SessionEvent::TurnFinished {
                        turn: 1,
                        outcome: "task_completed".into(),
                    },
                },
            },
        )
        .await;
        loop {
            match socket.next().await {
                Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                    if let Ok(ClientMsg::Ping) = decode_client(text.as_bytes()) {
                        return Result::<_, ServerError>::Ok(4);
                    }
                }
                Some(Ok(_)) => {}
                _ => return Result::<_, ServerError>::Ok(4),
            }
        }
    });

    let wired = wire(&base, "s1", "tok");
    let mut app = App::new(100_000);
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| a.view.turns().last().is_some_and(|t| t.outcome.is_some()),
            Duration::from_secs(10)
        ),
        "survived 3 drops and observed the finished turn; conn {:?}",
        app.conn
    );
    assert_eq!(app.view.turns().len(), 1, "no duplicate turn replay");
    assert_eq!(app.view.last_event_id, 2);

    let _ = wired.cmd.send(ClientCmd::Shutdown).await;
    let connections = server.await.ok().and_then(|result| result.ok());
    assert_eq!(
        connections,
        Some(4),
        "exactly four connections: three dropped + one stable"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_server_frames_are_bounded_and_typed() {
    let (listener, base) = ephemeral().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        read_hello(&mut socket).await;
        for _ in 0..3 {
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    "not a server message".into(),
                ))
                .await?;
        }
        let _ = socket.next().await;
        Result::<_, ServerError>::Ok(())
    });

    let wired = wire(&base, "s1", "tok");
    let deadline = Duration::from_secs(5);
    let mut saw_failure = None;
    loop {
        match wired.events.recv_timeout(deadline) {
            Ok(ClientEvent::Failed(err)) => {
                saw_failure = Some(err);
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    assert!(
        matches!(saw_failure, Some(ClientError::Protocol(_))),
        "typed protocol failure after three strikes, got {saw_failure:?}"
    );
    let _ = wired.cmd.send(ClientCmd::Shutdown).await;
    let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn absent_daemon_fails_three_times_then_parks_until_manual() {
    let (listener, base) = ephemeral().await;
    drop(listener);

    let wired = wire(&base, "s1", "tok");
    let mut app = App::new(100_000);
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| matches!(a.conn, Conn::Failed(ref err) if err.contains("unreachable")),
            // Refused loopback connects can take ~2 s each on Windows;
            // three attempts + backoff need a wide budget.
            Duration::from_secs(20)
        ),
        "typed unreachable failure surfaced, conn {:?}",
        app.conn
    );

    let _ = wired.cmd.send(ClientCmd::Shutdown).await;
    let mut closed = false;
    for _ in 0..20 {
        if wired
            .events
            .recv_timeout(Duration::from_millis(50))
            .is_err()
        {
            closed = true;
            break;
        }
    }
    assert!(closed, "shutdown ends the client task");
}
