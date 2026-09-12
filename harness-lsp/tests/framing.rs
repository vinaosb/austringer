//! LSP base-protocol framing tests: Content-Length framing, garbage on the
//! wire, lying Content-Length (bounded read, never OOM), oversized header
//! blocks, and write-side frame encoding. All against an in-memory duplex
//! pipe — no processes, fully deterministic.

use harness_lsp::proto::{FrameCaps, FrameError, read_frame, write_frame};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::duplex;

fn caps() -> FrameCaps {
    FrameCaps {
        max_header_bytes: 4 * 1024,
        max_frame_bytes: 64 * 1024,
    }
}

#[tokio::test]
async fn reads_a_well_formed_frame() {
    let (client, mut server) = duplex(8 * 1024);
    let body = br#"{"jsonrpc":"2.0","id":1,"result":42}"#;
    server
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await
        .expect("test write");
    server.write_all(body).await.expect("test write");
    server.flush().await.expect("test write");
    drop(server);

    let frame = read_frame(&mut BufReader::new(client), &caps())
        .await
        .expect("frame parses");
    assert_eq!(frame, body.to_vec());
}

#[tokio::test]
async fn reads_two_back_to_back_frames() {
    let (client, mut server) = duplex(8 * 1024);
    let one = br#"{"id":1}"#;
    let two = br#"{"id":2}"#;
    let mut wire = Vec::new();
    wire.extend_from_slice(format!("Content-Length: {}\r\n\r\n", one.len()).as_bytes());
    wire.extend_from_slice(one);
    wire.extend_from_slice(format!("Content-Length: {}\r\n\r\n", two.len()).as_bytes());
    wire.extend_from_slice(two);
    server.write_all(&wire).await.expect("test write");
    server.flush().await.expect("test write");
    drop(server);

    let mut reader = BufReader::new(client);
    assert_eq!(read_frame(&mut reader, &caps()).await.expect("first"), one);
    assert_eq!(read_frame(&mut reader, &caps()).await.expect("second"), two);
}

#[tokio::test]
async fn tolerates_bare_lf_header_termination() {
    let (client, mut server) = duplex(1024);
    let body = br#"{"ok":true}"#;
    server
        .write_all(format!("Content-Length: {}\n\n", body.len()).as_bytes())
        .await
        .expect("test write");
    server.write_all(body).await.expect("test write");
    server.flush().await.expect("test write");
    drop(server);

    assert_eq!(
        read_frame(&mut BufReader::new(client), &caps())
            .await
            .expect("lf frame"),
        body.to_vec()
    );
}

#[tokio::test]
async fn garbage_before_content_length_is_a_typed_error() {
    let (client, mut server) = duplex(1024);
    server
        .write_all(b"this is not lsp at all\r\n\r\n")
        .await
        .expect("test write");
    server.flush().await.expect("test write");
    drop(server);

    let err = read_frame(&mut BufReader::new(client), &caps())
        .await
        .expect_err("garbage must fail typed");
    assert!(
        matches!(err, FrameError::MissingContentLength),
        "garbage header must be the typed MissingContentLength, got: {err}"
    );
}

#[tokio::test]
async fn lying_content_length_is_rejected_without_reading() {
    // Declares 1 GiB of body but sends almost nothing: the reader must fail
    // typed WITHOUT trying to buffer the declared size (never OOM).
    let (client, mut server) = duplex(1024);
    server
        .write_all(b"Content-Length: 1073741824\r\n\r\n{\"id\":")
        .await
        .expect("test write");
    server.flush().await.expect("test write");
    drop(server);

    let err = read_frame(&mut BufReader::new(client), &caps())
        .await
        .expect_err("oversized declared body must fail typed");
    assert!(
        matches!(err, FrameError::BodyOverrun { declared, cap } if declared == 1_073_741_824 && cap == 64 * 1024),
        "lying Content-Length must be the typed BodyOverrun with the declared size, got: {err}"
    );
}

#[tokio::test]
async fn oversized_header_block_is_a_typed_error() {
    // A stream of bytes with no blank line inside the header cap.
    let (client, mut server) = duplex(16 * 1024);
    let junk = vec![b'x'; 8 * 1024];
    server.write_all(&junk).await.expect("test write");
    server.flush().await.expect("test write");
    drop(server);

    let err = read_frame(&mut BufReader::new(client), &caps())
        .await
        .expect_err("runaway header must fail typed");
    assert!(
        matches!(err, FrameError::HeaderOverrun(h) if h == 4 * 1024),
        "runaway header must be the typed HeaderOverrun, got: {err}"
    );
}

#[tokio::test]
async fn eof_mid_header_is_distinct_from_garbage() {
    let (client, mut server) = duplex(1024);
    server
        .write_all(b"Content-Length: 40\r\n")
        .await
        .expect("test write");
    server.flush().await.expect("test write");
    drop(server);

    let err = read_frame(&mut BufReader::new(client), &caps())
        .await
        .expect_err("truncated header must fail");
    assert!(
        matches!(err, FrameError::TruncatedHeader),
        "truncated header must be the typed TruncatedHeader, got: {err}"
    );
}

#[tokio::test]
async fn write_frame_produces_exact_wire_bytes() {
    let (mut client, mut server) = duplex(1024);
    write_frame(&mut server, br#"{"id":7}"#)
        .await
        .expect("write succeeds");
    drop(server);

    let mut wire = Vec::new();
    let _ = client.read_to_end(&mut wire).await;
    assert_eq!(
        String::from_utf8_lossy(&wire),
        "Content-Length: 8\r\n\r\n{\"id\":7}"
    );
}
