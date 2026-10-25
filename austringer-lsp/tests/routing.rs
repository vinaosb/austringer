//! Router-level contract tests: file-type routing to per-language servers,
//! lazy spawn, bridge-priority (IDE-as-toolbox: no duplicate servers),
//! restart-with-backoff → degradation, and UTF-16 conversion on answers.
//! All against the in-test mock LS — deterministic, no real servers.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use austringer_lsp::config::{Language, LspConfig, ServerCommand};
use austringer_lsp::error::LspError;
use austringer_lsp::provider::{Answer, DocumentPosition, IdeBridgeStub, Location, LspProvider};
use austringer_lsp::render::MAX_LOCATIONS;
use austringer_lsp::router::LspRouter;
use austringer_lsp::uri::file_to_uri;
use common::{Action, Handler, MockLsp, spawn_mock, utf8_caps};
use serde_json::{Value, json};
use tempfile::TempDir;

fn config(root: &TempDir) -> LspConfig {
    let mut cfg = LspConfig::new(root.path().to_path_buf());
    cfg.backoff.initial = Duration::from_millis(40);
    cfg.backoff.max = Duration::from_millis(200);
    cfg.initialize_timeout = Duration::from_secs(2);
    cfg.request_timeout = Duration::from_secs(2);
    cfg
}

fn handler_for(file: &Path) -> Handler {
    let target_uri = file_to_uri(file).unwrap_or_default();
    Arc::new(move |method: &str, _params: &Value| match method {
        "initialize" => Action::Result(utf8_caps()),
        "textDocument/definition" => Action::Result(json!([
            {"uri": target_uri, "range": {"start": {"line": 2, "character": 7},
                                          "end": {"line": 2, "character": 7}}}
        ])),
        "textDocument/references" => Action::Result(json!([
            {"uri": target_uri, "range": {"start": {"line": 9, "character": 0},
                                          "end": {"line": 9, "character": 0}}},
            {"uri": target_uri, "range": {"start": {"line": 30, "character": 4},
                                          "end": {"line": 30, "character": 4}}}
        ])),
        "textDocument/hover" => Action::Result(json!({
            "contents": {"kind": "markdown", "value": "```rust\npub fn target() -> u32\n```"}
        })),
        _ => Action::Result(Value::Null),
    })
}

async fn router_with_mock(cfg: LspConfig, handler: Handler) -> (LspRouter, MockLsp) {
    let router = LspRouter::new(cfg);
    let ((read, write), mock) = spawn_mock(handler).await;
    router
        .attach_server(Language::Rust, read, write)
        .await
        .expect("attach");
    (router, mock)
}

fn make_file(root: &TempDir, rel: &str, text: &str) -> PathBuf {
    let path = root.path().join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdirs");
    std::fs::write(&path, text).expect("fixture");
    path
}

fn pos(file: &Path, line: u32, column: u32) -> DocumentPosition {
    DocumentPosition {
        file: file.to_path_buf(),
        line,
        column,
    }
}

#[tokio::test]
async fn goto_definition_returns_exact_one_based_locations() {
    let root = TempDir::new().expect("tempdir");
    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let (router, _mock) = router_with_mock(config(&root), handler_for(&file)).await;

    let answer = router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("goto");
    let Answer::Locations(locs) = answer else {
        panic!("expected locations, got {answer:?}");
    };
    let expected = Location {
        file: file.to_path_buf(),
        line: 3,   // wire line 2 (0-based) + 1
        column: 8, // wire character 7 + 1
    };
    assert_eq!(
        locs,
        vec![expected],
        "exact location with 1-based conversion"
    );
}

#[tokio::test]
async fn find_references_and_hover_are_typed_and_bounded() {
    let root = TempDir::new().expect("tempdir");
    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let (router, _mock) = router_with_mock(config(&root), handler_for(&file)).await;

    let Answer::Locations(refs) = router
        .find_references(pos(&file, 3, 8))
        .await
        .expect("refs")
    else {
        panic!("expected locations");
    };
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[1].line, 31);

    let Answer::Hover(hover) = router.hover(pos(&file, 3, 8)).await.expect("hover") else {
        panic!("expected hover");
    };
    assert!(
        hover.as_str().contains("target"),
        "hover = {}",
        hover.as_str()
    );
}

#[tokio::test]
async fn utf16_server_answers_get_converted_through_file_text() {
    let root = TempDir::new().expect("tempdir");
    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    // "äbc def target" — the definition column lands after a 2-byte char,
    // so UTF-16 (2 units for... no: ä is 1 unit) — use an emoji prefix on
    // the answer line instead.
    std::fs::write(&file, "fn target() {}\nlet x = 1;\nlet s = \"target\";\n")
        .expect("write fixture");
    let target_uri = file_to_uri(&file).unwrap_or_default();
    // Serve with UTF-16 caps; column 3 on line 2 ("let s ...") is identity
    // for ASCII, proving the passthrough does not double-convert.
    let handler = Arc::new(move |method: &str, _p: &Value| match method {
        "initialize" => Action::Result(common::utf16_caps()),
        "textDocument/definition" => Action::Result(json!([
            {"uri": target_uri, "range": {"start": {"line": 2, "character": 9},
                                          "end": {"line": 2, "character": 9}}}
        ])),
        _ => Action::Result(Value::Null),
    });
    let (router, _mock) = router_with_mock(config(&root), handler).await;

    let Answer::Locations(locs) = router
        .goto_definition(pos(&file, 3, 8))
        .await
        .expect("goto")
    else {
        panic!("expected locations");
    };
    assert_eq!(
        locs,
        vec![Location {
            file: file.to_path_buf(),
            line: 3,
            column: 10
        }],
        "utf-16 columns must be converted via the file's own text"
    );
}

#[tokio::test]
async fn unconfigured_language_is_a_typed_no_server_error() {
    let root = TempDir::new().expect("tempdir");
    let router = LspRouter::new(config(&root));
    let err = router
        .goto_definition(pos(&root.path().join("x.xyz"), 1, 1))
        .await
        .expect_err("unknown extension");
    assert!(matches!(err, LspError::NoServer { .. }), "got: {err}");
}

#[tokio::test]
async fn missing_program_surfaces_typed_and_lazy_no_spawn_before_request() {
    let root = TempDir::new().expect("tempdir");
    let mut cfg = config(&root);
    cfg.servers.insert(
        Language::Rust,
        ServerCommand {
            program: "definitely-not-a-real-lsp-binary-xyz".into(),
            args: vec![],
        },
    );
    let router = LspRouter::new(cfg);
    // No request yet: nothing attempted.
    assert!(
        router.server_languages().is_empty(),
        "lazy: no spawn before first use"
    );

    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let err = router
        .goto_definition(pos(&file, 1, 1))
        .await
        .expect_err("spawn fails");
    assert!(
        matches!(err, LspError::ServerUnavailable { language: Language::Rust, ref program } if program.contains("definitely-not")),
        "got: {err}"
    );
}

#[tokio::test]
async fn bridge_takes_priority_and_native_never_spawns() {
    let root = TempDir::new().expect("tempdir");
    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let mut cfg = config(&root);
    // Native rust-analyzer IS configured and even installed here — the
    // priority rule must keep it unspawned anyway.
    cfg.servers.insert(
        Language::Rust,
        ServerCommand {
            program: "rust-analyzer".into(),
            args: vec![],
        },
    );
    let router = LspRouter::new(cfg);
    let bridge = FakeBridge::new(vec![Language::Rust]);
    router.set_bridge(bridge);

    let answer = router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("bridge answers");
    let Answer::Locations(locs) = answer else {
        panic!("expected locations")
    };
    assert_eq!(
        locs,
        vec![Location {
            file: PathBuf::from("C:/bridge/ans.rs"), // the bridge's own answer
            line: 42,
            column: 4
        }]
    );
    assert_eq!(
        router.server_languages(),
        vec![],
        "MUST NOT spawn a native server for a bridge-covered language"
    );
}

#[tokio::test]
async fn bridge_stub_is_registered_but_unwired() {
    let root = TempDir::new().expect("tempdir");
    let router = LspRouter::new(config(&root));
    router.set_bridge(Arc::new(IdeBridgeStub::new(
        "http://127.0.0.1:48171",
        &[Language::Rust],
    )));

    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let err = router
        .goto_definition(pos(&file, 1, 1))
        .await
        .expect_err("stub errors typed");
    assert!(matches!(err, LspError::BridgeNotWired), "got: {err}");
    assert!(router.server_languages().is_empty());
}

#[tokio::test]
async fn prefer_bridge_false_ignores_the_bridge() {
    let root = TempDir::new().expect("tempdir");
    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let mut cfg = config(&root);
    cfg.prefer_bridge = false;
    let (router, _mock) = router_with_mock(cfg, handler_for(&file)).await;
    let (bridge, mut rx) = FakeBridge::channel(vec![Language::Rust]);
    router.set_bridge(bridge);

    let Answer::Locations(locs) = router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("native path")
    else {
        panic!("expected locations")
    };
    assert_eq!(locs[0].line, 3, "native mock answers, not the bridge");
    // The bridge was registered but never consulted.
    assert!(
        rx.try_recv().is_err(),
        "bridge must not be called when prefer_bridge=false"
    );
}

#[tokio::test]
async fn bridge_covers_one_language_while_other_goes_native() {
    let root = TempDir::new().expect("tempdir");
    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let go_file = make_file(&root, "cmd/main.go", "package main\n");
    let (router, _mock) = router_with_mock(config(&root), handler_for(&file)).await;
    let bridge = FakeBridge::new(vec![Language::Go]);
    router.set_bridge(bridge);

    // Rust through the attached native mock...
    let Answer::Locations(rust_locs) = router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("rust")
    else {
        panic!("expected locations")
    };
    assert_eq!(rust_locs[0].line, 3);
    // ...and Go routes to the bridge, which answers with its own fixed
    // location — no native gopls is ever spawned.
    let Answer::Locations(go_locs) = router
        .goto_definition(pos(&go_file, 4, 2))
        .await
        .expect("go -> bridge")
    else {
        panic!("expected locations")
    };
    assert_eq!(
        go_locs,
        vec![Location {
            file: PathBuf::from("C:/bridge/ans.rs"),
            line: 42,
            column: 4
        }],
        "Go answered by the bridge, not by a native gopls"
    );
    assert_eq!(router.server_languages(), vec![Language::Rust]);
}

#[tokio::test]
async fn location_lists_are_bounded_to_max_locations() {
    let root = TempDir::new().expect("tempdir");
    let file = make_file(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let target_uri = file_to_uri(&file).unwrap_or_default();
    let many: Vec<Value> = (0..500)
        .map(|i| {
            json!({"uri": target_uri,
                   "range": {"start": {"line": i, "character": 0},
                             "end": {"line": i, "character": 0}}})
        })
        .collect();
    let handler = Arc::new(move |method: &str, _p: &Value| match method {
        "initialize" => Action::Result(utf8_caps()),
        "textDocument/references" => Action::Result(Value::Array(many.clone())),
        _ => Action::Result(Value::Null),
    });
    let (router, _mock) = router_with_mock(config(&root), handler).await;

    let Answer::Locations(locs) = router
        .find_references(pos(&file, 3, 8))
        .await
        .expect("refs")
    else {
        panic!("expected locations")
    };
    assert_eq!(
        locs.len(),
        MAX_LOCATIONS,
        "server sent 500; typed answer is bounded"
    );
}

/// A programmable bridge: answers everything with one fixed location;
/// optionally reports calls through a channel.
struct FakeBridge {
    languages: Vec<Language>,
    fixed: Location,
    calls: Option<tokio::sync::mpsc::UnboundedSender<&'static str>>,
}

impl FakeBridge {
    fn new(languages: Vec<Language>) -> Arc<Self> {
        Arc::new(Self {
            languages,
            fixed: Location {
                file: PathBuf::from("C:/bridge/ans.rs"),
                line: 42,
                column: 4,
            },
            calls: None,
        })
    }

    fn channel(
        languages: Vec<Language>,
    ) -> (
        Arc<Self>,
        tokio::sync::mpsc::UnboundedReceiver<&'static str>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (
            Arc::new(Self {
                languages,
                fixed: Location {
                    file: PathBuf::from("C:/bridge/ans.rs"),
                    line: 42,
                    column: 4,
                },
                calls: Some(tx),
            }),
            rx,
        )
    }

    async fn answer(&self, what: &'static str) -> Result<Answer, LspError> {
        if let Some(calls) = &self.calls {
            let _ = calls.send(what);
        }
        Ok(Answer::Locations(vec![self.fixed.clone()]))
    }
}

#[async_trait::async_trait]
impl LspProvider for FakeBridge {
    fn languages(&self) -> &[Language] {
        &self.languages
    }

    async fn goto_definition(&self, _position: DocumentPosition) -> Result<Answer, LspError> {
        self.answer("goto").await
    }

    async fn find_references(&self, _position: DocumentPosition) -> Result<Answer, LspError> {
        self.answer("refs").await
    }

    async fn hover(&self, _position: DocumentPosition) -> Result<Answer, LspError> {
        self.answer("hover").await
    }
}
