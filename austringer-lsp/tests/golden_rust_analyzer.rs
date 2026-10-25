//! Golden tests against REAL rust-analyzer on a fixture crate. Heavy:
//! they spawn a real server process and wait on real indexing, so every
//! test is `#[ignore]`d out of the fast suite — run explicitly:
//! `cargo test -p austringer-lsp --test golden_rust_analyzer -- --ignored`.

use std::path::PathBuf;
use std::time::Duration;

use austringer_lsp::config::{Language, LspConfig, ServerCommand};
use austringer_lsp::error::LspError;
use austringer_lsp::provider::{Answer, DocumentPosition, Location, LspProvider};
use austringer_lsp::router::{LspRouter, RouterEvent};
use tempfile::TempDir;

const LIB_RS: &str = "pub fn golden_target() -> u32 {\n    7\n}\n\npub fn golden_caller() -> u32 {\n    golden_target()\n}\n";

fn fixture_crate() -> (TempDir, PathBuf) {
    let root = TempDir::new().expect("tempdir");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("manifest");
    std::fs::create_dir_all(root.path().join("src")).expect("src dir");
    let lib = root.path().join("src/lib.rs");
    std::fs::write(&lib, LIB_RS).expect("lib.rs");
    (root, lib)
}

fn config(root: &TempDir) -> LspConfig {
    let mut cfg = LspConfig::new(root.path().to_path_buf());
    cfg.servers.insert(
        Language::Rust,
        ServerCommand {
            program: "rust-analyzer".into(),
            args: vec![],
        },
    );
    cfg.backoff.initial = Duration::from_millis(40);
    cfg.backoff.max = Duration::from_millis(200);
    cfg.initialize_timeout = Duration::from_secs(60);
    cfg.request_timeout = Duration::from_secs(60);
    cfg
}

/// One request, retrying JSON-RPC -32801 `ContentModified` a bounded
/// number of times. Per the LSP spec, -32801 means the server's state
/// changed under the in-flight request (rust-analyzer applying a
/// didChange or reloading its VFS) and the client SHOULD retry — a slow
/// runner can race that window. On the final attempt the original error
/// surfaces unchanged.
async fn retrying_content_modified<F>(
    mut request: impl FnMut(DocumentPosition) -> F,
    position: DocumentPosition,
) -> Result<Answer, LspError>
where
    F: std::future::Future<Output = Result<Answer, LspError>>,
{
    const ATTEMPTS: u32 = 5;
    let mut attempt = 0;
    loop {
        attempt += 1;
        match request(position.clone()).await {
            Err(LspError::RequestFailed { code: -32801, .. }) if attempt < ATTEMPTS => {
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            other => return other,
        }
    }
}

async fn goto_retrying_content_modified(
    router: &LspRouter,
    position: DocumentPosition,
) -> Result<Answer, LspError> {
    retrying_content_modified(|pos| router.goto_definition(pos), position).await
}

async fn references_retrying_content_modified(
    router: &LspRouter,
    position: DocumentPosition,
) -> Result<Answer, LspError> {
    retrying_content_modified(|pos| router.find_references(pos), position).await
}

async fn hover_retrying_content_modified(
    router: &LspRouter,
    position: DocumentPosition,
) -> Result<Answer, LspError> {
    retrying_content_modified(|pos| router.hover(pos), position).await
}

/// Polls a navigation until it resolves non-empty, or panics at the
/// deadline. rust-analyzer legitimately answers empty results while a
/// crate is still loading, so goldens wait for readiness rather than
/// racing it.
async fn goto_until_ready(router: &LspRouter, position: DocumentPosition) -> Vec<Location> {
    let deadline = std::time::Instant::now() + Duration::from_secs(45);
    loop {
        let Answer::Locations(locs) = goto_retrying_content_modified(router, position.clone())
            .await
            .expect("goto request itself succeeds")
        else {
            panic!("expected locations");
        };
        if !locs.is_empty() {
            return locs;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "rust-analyzer never resolved the definition (still loading?)"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Polls find_references until it resolves non-empty, or panics at the
/// deadline — the same readiness discipline as `goto_until_ready`:
/// rust-analyzer legitimately answers `[]` while the crate graph is
/// still settling (CI receipt 2026-10-24: a single-shot references call
/// came back empty on a slow ubuntu runner).
async fn references_until_ready(router: &LspRouter, position: DocumentPosition) -> Vec<Location> {
    let deadline = std::time::Instant::now() + Duration::from_secs(45);
    loop {
        let Answer::Locations(refs) =
            references_retrying_content_modified(router, position.clone())
                .await
                .expect("references request itself succeeds")
        else {
            panic!("expected locations");
        };
        if !refs.is_empty() {
            return refs;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "rust-analyzer never resolved references (still loading?)"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
#[ignore = "spawns real rust-analyzer; run with --ignored explicitly"]
async fn golden_goto_definition_returns_the_exact_location() {
    let (root, lib) = fixture_crate();
    let router = LspRouter::new(config(&root));

    let locs = goto_until_ready(
        &router,
        DocumentPosition {
            file: lib.clone(),
            line: 6,
            column: 5,
        },
    )
    .await;

    let got = &locs[0];
    println!(
        "golden goto_definition -> {}:{}",
        got.file.display(),
        got.line
    );
    assert_eq!(got.line, 1, "definition is on line 1 (got {locs:?})");
    assert_eq!(got.column, 8, "'golden_target' starts at column 8");
    assert!(
        got.file
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("src/lib.rs"),
        "definition points into the fixture crate: {}",
        got.file.display()
    );
    let _ = router.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns real rust-analyzer; run with --ignored explicitly"]
async fn golden_hover_is_bounded_and_names_the_symbol() {
    let (root, lib) = fixture_crate();
    let router = LspRouter::new(config(&root));
    // Warm the analysis with a resolved goto first.
    let _ = goto_until_ready(
        &router,
        DocumentPosition {
            file: lib.clone(),
            line: 6,
            column: 5,
        },
    )
    .await;

    let deadline = std::time::Instant::now() + Duration::from_secs(45);
    let text = loop {
        let Answer::Hover(hover) = hover_retrying_content_modified(
            &router,
            DocumentPosition {
                file: lib.clone(),
                line: 1,
                column: 8,
            },
        )
        .await
        .expect("hover over the definition") else {
            panic!("expected hover");
        };
        if !hover.as_str().trim().is_empty() {
            break hover.as_str().to_owned();
        }
        assert!(std::time::Instant::now() < deadline, "hover stayed empty");
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    println!("golden hover ({} chars) -> {}", text.chars().count(), text);
    assert!(text.contains("golden_target"), "hover names the symbol");
    assert!(
        text.chars().count() <= 480,
        "hover text stays within budget"
    );

    let refs = references_until_ready(
        &router,
        DocumentPosition {
            file: lib.clone(),
            line: 1,
            column: 8,
        },
    )
    .await;
    println!("golden find_references -> {} locations", refs.len());
    assert!(
        refs.len() >= 2,
        "definition + at least the caller, got {refs:?}"
    );
    let _ = router.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns real rust-analyzer; run with --ignored explicitly"]
async fn did_change_refreshes_positions_in_the_live_server() {
    let (root, lib) = fixture_crate();
    let router = LspRouter::new(config(&root));

    let before = goto_until_ready(
        &router,
        DocumentPosition {
            file: lib.clone(),
            line: 6,
            column: 5,
        },
    )
    .await;
    assert_eq!(before[0].line, 1);

    // Shift the definition down three lines and sync the new full text.
    let shifted = format!("// one\n// two\n// three\n{LIB_RS}");
    router
        .notify_did_change(&lib, &shifted)
        .await
        .expect("did change");

    // Fresh results: the definition resolves at its NEW position (polling
    // tolerates re-analysis time, never a stale line 1).
    let deadline = std::time::Instant::now() + Duration::from_secs(45);
    loop {
        let Answer::Locations(after) = goto_retrying_content_modified(
            &router,
            DocumentPosition {
                file: lib.clone(),
                line: 9,
                column: 5,
            },
        )
        .await
        .expect("goto after") else {
            panic!("expected locations")
        };
        if !after.is_empty() {
            assert_eq!(
                after[0].line, 4,
                "definition moved to line 4 after didChange"
            );
            assert_eq!(after[0].column, 8);
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "goto stayed empty after didChange"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let _ = router.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns real rust-analyzer + taskkill; run with --ignored explicitly"]
async fn real_kill9_restart_with_backoff_then_degrade() {
    let (root, lib) = fixture_crate();
    let router = LspRouter::new(config(&root));
    let mut events = router.take_events().expect("events");

    // Round 1: healthy session.
    let one = goto_until_ready(
        &router,
        DocumentPosition {
            file: lib.clone(),
            line: 6,
            column: 5,
        },
    )
    .await;
    assert_eq!(one[0].line, 1);
    let pid = router
        .server_pid(Language::Rust)
        .expect("rust-analyzer has a real pid");
    println!("rust-analyzer spawned as pid {pid}");

    // Kill -9 the server mid-session.
    kill9(pid);
    await_event(&mut events, "ServerCrashed").await;

    // Round 2: restart with backoff, then service resumes.
    let restarted_at = std::time::Instant::now();
    let two = goto_until_ready(
        &router,
        DocumentPosition {
            file: lib.clone(),
            line: 6,
            column: 5,
        },
    )
    .await;
    assert_eq!(two[0].line, 1);
    match events.try_recv() {
        Ok(RouterEvent::RestartScheduled { wait, .. }) => {
            println!(
                "restart backoff: {wait:?}; wall-clock gap: {:?}",
                restarted_at.elapsed()
            );
            assert_eq!(wait, Duration::from_millis(40));
        }
        other => panic!("expected RestartScheduled first, got {other:?}"),
    }
    assert!(matches!(
        events.try_recv(),
        Ok(RouterEvent::ServerRestarted { .. })
    ));
    let new_pid = router.server_pid(Language::Rust).expect("a fresh server");
    assert_ne!(new_pid, pid, "the restart produced a new process");

    // Second kill: restart budget (1) spent -> typed degraded fallback.
    kill9(new_pid);
    await_event(&mut events, "ServerCrashed").await;

    let answer = router
        .goto_definition(DocumentPosition {
            file: lib.clone(),
            line: 6,
            column: 5,
        })
        .await
        .expect("degradation is a result, not an error");
    println!("after second kill -> {answer:?}");
    assert!(matches!(answer, Answer::DegradedToSearch { .. }));
    assert!(matches!(
        events.try_recv(),
        Ok(RouterEvent::Degraded { .. })
    ));
    let _ = router.shutdown().await;
}

/// kill -9 the server process itself (not /T: rust-analyzer's transient
/// children resist tree-termination and orphan-reaping is todo 19/20's
/// Job-Object work; the stdio we hold belongs to this pid anyway). A
/// "not found" outcome means the server already died — equally dead.
fn kill9(pid: u32) {
    let output = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()
        .expect("taskkill runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!(
        "taskkill /F /PID {pid}: exit={}, stderr: {}",
        output.status.code().unwrap_or(-1),
        stderr.trim()
    );
    assert!(
        output.status.success() || stderr.contains("not found"),
        "taskkill /F /PID {pid} failed: {stderr}"
    );
}

async fn await_event(events: &mut tokio::sync::mpsc::Receiver<RouterEvent>, name: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(event) = events.try_recv()
            && format!("{event:?}").contains(name)
        {
            println!("event: {event:?}");
            return;
        }
        assert!(std::time::Instant::now() < deadline, "{name} never arrived");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
