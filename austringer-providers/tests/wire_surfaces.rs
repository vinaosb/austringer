//! Wire-surface cassettes (research §24.6 stable-first/volatile-last,
//! §27.6 breakpoint placement, §33.1 native surface matrix): system,
//! declared tools, and provider-native cache-control ride the request.
//!
//! These tests pin the NEW request shape per adapter family AND the
//! legacy byte-identity guarantee: a CompletionRequest without surfaces
//! renders exactly the pre-surfaces body (the recorded cassette corpus
//! is the spec — strict replay stays green without regeneration).

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use austringer_core::tools::core_schema;
use austringer_providers::adapters::{AnthropicAdapter, CompatAdapter, GoogleAdapter};
use austringer_providers::auth::StaticApiKey;
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::provider::Provider;
use austringer_providers::transport::{Transport, TransportFailure, TransportRequest};
use serde_json::Value;

/// Captures every rendered request body (parsed), then fails the send —
/// the request shape is the only thing under test.
#[derive(Default)]
struct CaptureTransport {
    bodies: Mutex<Vec<Value>>,
}

impl CaptureTransport {
    fn captured(&self) -> Vec<Value> {
        self.bodies.lock().map(|b| b.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Transport for CaptureTransport {
    async fn send(
        &self,
        request: TransportRequest,
    ) -> Result<austringer_providers::transport::TransportResponse, TransportFailure> {
        let body: Value = serde_json::from_slice(&request.body).map_err(|err| {
            TransportFailure::Protocol(format!("request body is not JSON: {err}"))
        })?;
        if let Ok(mut bodies) = self.bodies.lock() {
            bodies.push(body);
        }
        Err(TransportFailure::Protocol("capture-only transport".into()))
    }
}

fn identity(provider: &str, model: &str) -> ModelIdentity {
    ModelIdentity::new(provider, model)
}

/// Renders one request through a freshly composed adapter over a fresh
/// capture transport; returns the single rendered body.
fn render(
    build: impl FnOnce(Arc<dyn Transport>) -> Box<dyn Provider>,
    request: &CompletionRequest,
) -> Value {
    let capture = Arc::new(CaptureTransport::default());
    let adapter = build(Arc::clone(&capture) as Arc<dyn Transport>);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(async {
            let _ = adapter.stream_completion(request).await;
        });
    capture
        .captured()
        .into_iter()
        .next()
        .expect("one rendered request")
}

const SYSTEM: &str = "STABLE SYSTEM PROMPT";
const TAIL: &str = "TAIL-FOCUS";
const STABLE: &str = "user: hi\n";

fn surfaced_request(id: ModelIdentity) -> CompletionRequest {
    CompletionRequest::new(id, format!("{STABLE}{TAIL}"))
        .with_system(SYSTEM)
        .with_tools(vec![core_schema("view_window"), core_schema("edit_lines")])
        .with_volatile_tail(TAIL.len())
}

// ---------------------------------------------------------------- Anthropic

#[test]
fn anthropic_wire_carries_system_tools_and_cache_breakpoints() {
    let body = render(
        |transport| {
            Box::new(AnthropicAdapter::new(
                transport,
                Arc::new(StaticApiKey::x_api_key("sk-ws")),
                "claude-ws",
            ))
        },
        &surfaced_request(identity("anthropic", "claude-ws")),
    );

    // Top-level system: one marked text block.
    let system = body["system"].as_array().expect("system is an array");
    assert_eq!(system.len(), 1);
    assert_eq!(system[0]["type"], "text");
    assert_eq!(system[0]["text"], SYSTEM);
    assert_eq!(system[0]["cache_control"]["type"], "ephemeral");

    // Tools: native declarations; the LAST carries the breakpoint.
    let tools = body["tools"].as_array().expect("tools is an array");
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "view_window");
    assert_eq!(
        tools[0]["input_schema"],
        core_schema("view_window").parameters
    );
    assert!(
        tools[0].get("cache_control").is_none(),
        "only the last tool is marked"
    );
    assert_eq!(tools[1]["name"], "edit_lines");
    assert_eq!(
        tools[1]["input_schema"],
        core_schema("edit_lines").parameters
    );
    assert_eq!(tools[1]["cache_control"]["type"], "ephemeral");

    // Messages: the single user message splits stable-first /
    // volatile-last; only the stable block is marked.
    let message = &body["messages"][0];
    assert_eq!(message["role"], "user");
    let content = message["content"].as_array().expect("content is an array");
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], STABLE);
    assert_eq!(content[0]["cache_control"]["type"], "ephemeral");
    assert_eq!(content[1]["text"], TAIL);
    assert!(
        content[1].get("cache_control").is_none(),
        "the volatile tail is never marked"
    );
}

#[test]
fn anthropic_wire_without_surfaces_stays_byte_identical() {
    let body = render(
        |transport| {
            Box::new(AnthropicAdapter::new(
                transport,
                Arc::new(StaticApiKey::x_api_key("sk-ws")),
                "claude-ws",
            ))
        },
        &CompletionRequest::new(identity("anthropic", "claude-ws"), "user: hi"),
    );
    assert!(
        body.get("system").is_none(),
        "no system key on the legacy shape"
    );
    assert!(
        body.get("tools").is_none(),
        "no tools key on the legacy shape"
    );
    assert_eq!(
        body["messages"][0]["content"],
        Value::String("user: hi".into()),
        "legacy content is the plain prompt string (cassette corpus identity)"
    );
}

// ------------------------------------------------- chat-completions family

#[test]
fn chat_wire_carries_system_message_and_tool_declarations() {
    let body = render(
        |transport| {
            Box::new(austringer_providers::adapters::OpenAiAdapter::chat(
                transport,
                Arc::new(StaticApiKey::bearer("sk-ws")),
                "gpt-ws",
            ))
        },
        &surfaced_request(identity("openai", "gpt-ws")),
    );
    // System rides the FIRST message; the whole prompt stays one user
    // message (no markers — automatic prefix caching needs none).
    let messages = body["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], SYSTEM);
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], format!("{STABLE}{TAIL}"));
    // Tools: the OpenAI function shape.
    let tools = body["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["function"]["name"], "view_window");
    assert_eq!(
        tools[0]["function"]["description"],
        core_schema("view_window").description
    );
    assert_eq!(
        tools[0]["function"]["parameters"],
        core_schema("view_window").parameters
    );
}

#[test]
fn chat_wire_without_surfaces_stays_byte_identical() {
    let body = render(
        |transport| {
            Box::new(austringer_providers::adapters::OpenAiAdapter::chat(
                transport,
                Arc::new(StaticApiKey::bearer("sk-ws")),
                "gpt-ws",
            ))
        },
        &CompletionRequest::new(identity("openai", "gpt-ws"), "user: hi"),
    );
    let messages = body["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 1, "no system message on the legacy shape");
    assert_eq!(messages[0]["role"], "user");
    assert!(
        body.get("tools").is_none(),
        "no tools key on the legacy shape"
    );
}

#[test]
fn compat_wire_carries_the_same_chat_shape() {
    let body = render(
        |transport| {
            Box::new(CompatAdapter::new(
                transport,
                Arc::new(StaticApiKey::bearer("sk-ws")),
                "http://127.0.0.1:11434/v1",
                "compat-ws",
            ))
        },
        &surfaced_request(identity("openai-compat", "compat-ws")),
    );
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], SYSTEM);
    assert_eq!(body["tools"][0]["function"]["name"], "view_window");
}

// ---------------------------------------------------------------- Responses

#[test]
fn responses_wire_carries_instructions_and_tool_declarations() {
    let body = render(
        |transport| {
            Box::new(austringer_providers::adapters::OpenAiAdapter::responses(
                transport,
                Arc::new(StaticApiKey::bearer("sk-ws")),
                "gpt-ws",
            ))
        },
        &surfaced_request(identity("openai", "gpt-ws")),
    );
    assert_eq!(body["instructions"], SYSTEM);
    let tools = body["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["name"], "view_window");
    assert_eq!(
        tools[0]["parameters"],
        core_schema("view_window").parameters
    );
    assert_eq!(body["input"], format!("{STABLE}{TAIL}"));
}

#[test]
fn responses_wire_without_surfaces_stays_byte_identical() {
    let body = render(
        |transport| {
            Box::new(austringer_providers::adapters::OpenAiAdapter::responses(
                transport,
                Arc::new(StaticApiKey::bearer("sk-ws")),
                "gpt-ws",
            ))
        },
        &CompletionRequest::new(identity("openai", "gpt-ws"), "user: hi"),
    );
    assert!(
        body.get("instructions").is_none(),
        "legacy shape has no instructions"
    );
    assert!(body.get("tools").is_none(), "legacy shape has no tools");
    assert_eq!(body["input"], Value::String("user: hi".into()));
}

// ------------------------------------------------------------------- Google

#[test]
fn google_wire_carries_system_instruction_and_function_declarations() {
    let body = render(
        |transport| {
            Box::new(GoogleAdapter::new(
                transport,
                Arc::new(StaticApiKey::google_api_key("g-ws")),
                "gemini-ws",
            ))
        },
        &surfaced_request(identity("google", "gemini-ws")),
    );
    assert_eq!(body["systemInstruction"]["parts"][0]["text"], SYSTEM);
    let declarations = body["tools"][0]["functionDeclarations"]
        .as_array()
        .expect("functionDeclarations");
    assert_eq!(declarations.len(), 2);
    assert_eq!(declarations[0]["name"], "view_window");
    assert_eq!(
        declarations[0]["parameters"],
        core_schema("view_window").parameters
    );
    assert_eq!(
        body["contents"][0]["parts"][0]["text"],
        format!("{STABLE}{TAIL}")
    );
}

#[test]
fn google_wire_without_surfaces_stays_byte_identical() {
    let body = render(
        |transport| {
            Box::new(GoogleAdapter::new(
                transport,
                Arc::new(StaticApiKey::google_api_key("g-ws")),
                "gemini-ws",
            ))
        },
        &CompletionRequest::new(identity("google", "gemini-ws"), "user: hi"),
    );
    assert!(
        body.get("systemInstruction").is_none(),
        "legacy shape has no systemInstruction"
    );
    assert!(body.get("tools").is_none(), "legacy shape has no tools");
}

// ------------------------------------------------------------- the seam

mod seam {
    //! The Inference seam maps InferenceRequest surfaces onto the
    //! CompletionRequest — providers never re-derive them.

    use std::sync::{Arc, Mutex, OnceLock};

    use async_trait::async_trait;
    use austringer_core::agent_loop::{CancelToken, Inference, InferenceRequest};
    use austringer_core::event::RecordedEvent;
    use austringer_core::tools::core_schema;
    use austringer_providers::adapter::InferenceAdapter;
    use austringer_providers::error::ProviderStreamError;
    use austringer_providers::identity::{CompletionRequest, ModelIdentity};
    use austringer_providers::provider::{Provider, ProviderEventStream};
    use austringer_providers::transport::TransportFailure;

    #[derive(Default)]
    struct CaptureProvider {
        requests: Mutex<Vec<CompletionRequest>>,
    }

    #[async_trait]
    impl Provider for CaptureProvider {
        fn identity(&self) -> &ModelIdentity {
            static IDENTITY: OnceLock<ModelIdentity> = OnceLock::new();
            IDENTITY.get_or_init(|| ModelIdentity::new("anthropic", "claude-ws"))
        }

        async fn stream_completion(
            &self,
            request: &CompletionRequest,
        ) -> Result<ProviderEventStream, ProviderStreamError> {
            if let Ok(mut requests) = self.requests.lock() {
                requests.push(request.clone());
            }
            Err(ProviderStreamError::Transport(TransportFailure::Protocol(
                "capture-only provider".into(),
            )))
        }
    }

    fn drive(request: &InferenceRequest<'_>) -> CompletionRequest {
        let provider = Arc::new(CaptureProvider::default());
        let mut adapter = InferenceAdapter::new(
            Arc::clone(&provider) as Arc<dyn Provider>,
            super::turn_config(),
            99,
        );
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(async {
                let _ = adapter.infer(request, &CancelToken::new()).await;
            });
        let requests = provider.requests.lock().expect("capture");
        requests.first().cloned().expect("one completion")
    }

    #[test]
    fn seam_passes_system_tools_and_tail_onto_the_completion() {
        let events: Vec<RecordedEvent> = Vec::new();
        let tools = [core_schema("view_window")];
        let request = InferenceRequest {
            events: &events,
            focus_block: "TAIL",
            system: "SEAM SYSTEM",
            tools: &tools,
        };
        let completion = drive(&request);
        assert_eq!(completion.system.as_deref(), Some("SEAM SYSTEM"));
        assert_eq!(completion.tools, tools.to_vec());
        assert_eq!(completion.volatile_tail_bytes, "TAIL".len());
    }

    #[test]
    fn seam_without_surfaces_declares_none() {
        let events: Vec<RecordedEvent> = Vec::new();
        let request = InferenceRequest {
            events: &events,
            focus_block: "TAIL",
            system: "",
            tools: &[],
        };
        let completion = drive(&request);
        assert!(completion.system.is_none());
        assert!(completion.tools.is_empty());
        // The adapter is the layout authority: the tail boundary rides
        // every request even when no surface is declared.
        assert_eq!(completion.volatile_tail_bytes, "TAIL".len());
    }

    #[test]
    fn turn_id_binds_the_wire_surfaces() {
        let bare = CompletionRequest::new(ModelIdentity::new("anthropic", "m"), "p");
        let with_system = bare.clone().with_system("S");
        let with_tools = bare.clone().with_tools(vec![core_schema("view_window")]);
        assert_ne!(
            bare.turn_id(),
            with_system.turn_id(),
            "system is identity-bearing"
        );
        assert_ne!(
            bare.turn_id(),
            with_tools.turn_id(),
            "tools are identity-bearing"
        );
        let twin = CompletionRequest::new(ModelIdentity::new("anthropic", "m"), "p")
            .with_system("S")
            .with_tools(vec![core_schema("view_window")]);
        let again = CompletionRequest::new(ModelIdentity::new("anthropic", "m"), "p")
            .with_system("S")
            .with_tools(vec![core_schema("view_window")]);
        assert_eq!(
            twin.turn_id(),
            again.turn_id(),
            "same surfaces re-issue stable"
        );
    }
}

/// The providers-crate recovery config the seam tests share.
fn turn_config() -> austringer_providers::turn::RecoveryConfig {
    austringer_providers::turn::RecoveryConfig {
        retry: austringer_providers::retry::RetryPolicy {
            max_attempts: 1,
            initial_backoff: std::time::Duration::from_millis(1),
            max_backoff: std::time::Duration::from_millis(1),
            multiplier: 1.0,
        },
        max_resumes: 0,
        idle_timeout: std::time::Duration::from_secs(1),
    }
}
