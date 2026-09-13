//! Mock seams and builders for hook (todo 28) tests: scripted shell /
//! transport / MCP / evaluator fakes, shared wrappers so assertions can
//! read mock state while `HookDeps` owns the boxes, and journal helpers.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harness_core::agent_loop::PendingCall;
use harness_core::agent_loop::stages::{StageCtx, StageError, ToolExecutor, ToolOutcome};
use harness_core::hooks::{
    FailPolicy, HookDeps, HookEvent, HookFiring, HookHttpRequest, HookHttpResponse, HookKind,
    HookManager, HookSpec, HookTransportError, HttpMethod, PolicyVerdict, PromptEvaluator,
    ShellOutput, ShellRunner,
};

/// What one scripted shell response does.
pub enum ShellScript {
    /// Successful exit with stdout.
    Ok(String),
    /// Non-zero exit code.
    Exit(i32),
    /// Transport-level failure.
    Err(String),
    /// Never resolves (timeout probe).
    Hung,
}

struct MockShell {
    calls: Mutex<Vec<(String, serde_json::Value)>>,
    script: Mutex<VecDeque<ShellScript>>,
}

impl MockShell {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            script: Mutex::new(VecDeque::new()),
        }
    }
}

/// A scripted, asserted shell runner seam.
#[derive(Clone)]
pub struct SharedShell(Arc<MockShell>);

impl SharedShell {
    /// A mock with an empty script (default: success, empty stdout).
    pub fn new() -> Self {
        Self(Arc::new(MockShell::new()))
    }
    /// Queues one scripted response.
    pub fn push(&self, entry: ShellScript) {
        if let Ok(mut script) = self.0.script.lock() {
            script.push_back(entry);
        }
    }
    /// Queues a successful run echoing `stdout`.
    pub fn push_ok(&self, stdout: &str) {
        self.push(ShellScript::Ok(stdout.to_owned()));
    }
    /// Queues a transport-level failure.
    pub fn push_err(&self, message: &str) {
        self.push(ShellScript::Err(message.to_owned()));
    }
    /// Every (command, payload) the seam received, in order.
    pub fn calls(&self) -> Vec<(String, serde_json::Value)> {
        self.0.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl ShellRunner for SharedShell {
    async fn run(
        &self,
        command: &str,
        payload: &serde_json::Value,
    ) -> Result<ShellOutput, HookTransportError> {
        if let Ok(mut calls) = self.0.calls.lock() {
            calls.push((command.to_owned(), payload.clone()));
        }
        let next = self
            .0
            .script
            .lock()
            .ok()
            .and_then(|mut s| s.pop_front())
            .unwrap_or(ShellScript::Ok(String::new()));
        match next {
            ShellScript::Ok(stdout) => Ok(ShellOutput {
                exit_code: 0,
                stdout,
            }),
            ShellScript::Exit(code) => Ok(ShellOutput {
                exit_code: code,
                stdout: String::new(),
            }),
            ShellScript::Err(message) => Err(HookTransportError { message }),
            ShellScript::Hung => std::future::pending().await,
        }
    }
}

enum HttpScript {
    Ok(u16, String),
    Err(String),
}

struct MockTransport {
    calls: Mutex<Vec<HookHttpRequest>>,
    script: Mutex<VecDeque<HttpScript>>,
}

/// A scripted, asserted HTTP transport seam. Zero live network.
#[derive(Clone)]
pub struct SharedTransport(Arc<MockTransport>);

impl SharedTransport {
    /// A mock with an empty script (default: 200, empty body).
    pub fn new() -> Self {
        Self(Arc::new(MockTransport {
            calls: Mutex::new(Vec::new()),
            script: Mutex::new(VecDeque::new()),
        }))
    }
    /// Queues a response.
    pub fn push_ok(&self, status: u16, body: &str) {
        if let Ok(mut script) = self.0.script.lock() {
            script.push_back(HttpScript::Ok(status, body.to_owned()));
        }
    }
    /// Queues a transport failure.
    pub fn push_err(&self, message: &str) {
        if let Ok(mut script) = self.0.script.lock() {
            script.push_back(HttpScript::Err(message.to_owned()));
        }
    }
    /// Every request the seam received, in order.
    pub fn calls(&self) -> Vec<HookHttpRequest> {
        self.0.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl harness_core::hooks::HttpTransport for SharedTransport {
    async fn send(&self, request: HookHttpRequest) -> Result<HookHttpResponse, HookTransportError> {
        if let Ok(mut calls) = self.0.calls.lock() {
            calls.push(request);
        }
        let next = self
            .0
            .script
            .lock()
            .ok()
            .and_then(|mut s| s.pop_front())
            .unwrap_or(HttpScript::Ok(200, String::new()));
        match next {
            HttpScript::Ok(status, body) => Ok(HookHttpResponse { status, body }),
            HttpScript::Err(message) => Err(HookTransportError { message }),
        }
    }
}

struct MockMcp {
    calls: Mutex<Vec<(String, String, serde_json::Value)>>,
    bodies: Mutex<VecDeque<String>>,
}

/// A scripted, asserted MCP-tool seam (todo 29 wires the real client).
#[derive(Clone)]
pub struct SharedMcp(Arc<MockMcp>);

impl SharedMcp {
    /// A mock with an empty script (default: empty tool-result text).
    pub fn new() -> Self {
        Self(Arc::new(MockMcp {
            calls: Mutex::new(Vec::new()),
            bodies: Mutex::new(VecDeque::new()),
        }))
    }
    /// Queues a successful tool-result body.
    pub fn push_ok(&self, body: &str) {
        if let Ok(mut bodies) = self.0.bodies.lock() {
            bodies.push_back(body.to_owned());
        }
    }
    /// Every (server, tool, payload) the seam received, in order.
    pub fn calls(&self) -> Vec<(String, String, serde_json::Value)> {
        self.0.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl harness_core::hooks::McpToolInvoker for SharedMcp {
    async fn invoke(
        &self,
        server: &str,
        tool: &str,
        payload: &serde_json::Value,
    ) -> Result<String, HookTransportError> {
        if let Ok(mut calls) = self.0.calls.lock() {
            calls.push((server.to_owned(), tool.to_owned(), payload.clone()));
        }
        self.0
            .bodies
            .lock()
            .ok()
            .and_then(|mut b| b.pop_front())
            .map(Ok)
            .unwrap_or_else(|| Ok(String::new()))
    }
}

enum VerdictScript {
    Ok(PolicyVerdict),
    Err(String),
}

struct MockEvaluator {
    calls: Mutex<Vec<(String, serde_json::Value)>>,
    script: Mutex<VecDeque<VerdictScript>>,
}

/// A scripted, asserted prompt-evaluator seam. No live LLM.
#[derive(Clone)]
pub struct SharedEvaluator(Arc<MockEvaluator>);

impl SharedEvaluator {
    /// A mock with an empty script (default: Proceed).
    pub fn new() -> Self {
        Self(Arc::new(MockEvaluator {
            calls: Mutex::new(Vec::new()),
            script: Mutex::new(VecDeque::new()),
        }))
    }
    /// Queues a verdict.
    pub fn push(&self, verdict: PolicyVerdict) {
        if let Ok(mut script) = self.0.script.lock() {
            script.push_back(VerdictScript::Ok(verdict));
        }
    }
    /// Queues an evaluator failure.
    pub fn push_err(&self, message: &str) {
        if let Ok(mut script) = self.0.script.lock() {
            script.push_back(VerdictScript::Err(message.to_owned()));
        }
    }
    /// Every (policy, payload) the seam received, in order.
    pub fn calls(&self) -> Vec<(String, serde_json::Value)> {
        self.0.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl PromptEvaluator for SharedEvaluator {
    async fn evaluate(
        &self,
        policy: &str,
        payload: &serde_json::Value,
    ) -> Result<PolicyVerdict, HookTransportError> {
        if let Ok(mut calls) = self.0.calls.lock() {
            calls.push((policy.to_owned(), payload.clone()));
        }
        let next = self
            .0
            .script
            .lock()
            .ok()
            .and_then(|mut s| s.pop_front())
            .unwrap_or(VerdictScript::Ok(PolicyVerdict::Proceed));
        match next {
            VerdictScript::Ok(verdict) => Ok(verdict),
            VerdictScript::Err(message) => Err(HookTransportError { message }),
        }
    }
}

/// The full mock bundle: handles for assertions + the boxed `HookDeps`.
pub struct MockSeams {
    /// Assertable shell mock.
    pub shell: SharedShell,
    /// Assertable transport mock.
    pub transport: SharedTransport,
    /// Assertable MCP mock.
    pub mcp: SharedMcp,
    /// Assertable evaluator mock.
    pub evaluator: SharedEvaluator,
}

/// Mock seams plus deps built from them.
pub fn mock_deps() -> (MockSeams, HookDeps) {
    let seams = MockSeams {
        shell: SharedShell::new(),
        transport: SharedTransport::new(),
        mcp: SharedMcp::new(),
        evaluator: SharedEvaluator::new(),
    };
    let deps = HookDeps {
        shell: Box::new(seams.shell.clone()),
        http: Box::new(seams.transport.clone()),
        mcp: Box::new(seams.mcp.clone()),
        evaluator: Box::new(seams.evaluator.clone()),
    };
    (seams, deps)
}

/// A manager over `specs` with every config approved (trust review
/// simulated) — most tests want running hooks, not inert ones.
pub fn approved_manager(specs: Vec<HookSpec>) -> HookManager {
    let (_, deps) = mock_deps();
    let mut manager = HookManager::new(specs, deps);
    manager.approve_all();
    manager
}

/// A shell hook spec.
pub fn shell_hook(name: &str, events: &[HookEvent], command: &str) -> HookSpec {
    HookSpec::new(
        name,
        events,
        HookKind::Shell {
            command: command.to_owned(),
        },
    )
}

/// Journal payloads for `event`, in firing order.
pub fn payloads(journal: &[HookFiring], event: HookEvent) -> Vec<&serde_json::Value> {
    journal
        .iter()
        .filter(|f| f.event == event)
        .map(|f| &f.payload)
        .collect()
}

/// Serialized golden string of one payload (sorted keys).
pub fn golden(payload: &serde_json::Value) -> String {
    serde_json::to_string(payload).unwrap_or_default()
}

/// A fail-open override builder for specs.
pub fn fail_open(spec: HookSpec) -> HookSpec {
    spec.with_fail_policy(FailPolicy::FailOpen)
}

/// A deterministic tool executor recording what ran.
#[derive(Debug, Default)]
pub struct EchoExecutor {
    /// Tool names executed, in order.
    pub ran: Mutex<Vec<String>>,
}

#[async_trait]
impl ToolExecutor for EchoExecutor {
    async fn execute(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        if let Ok(mut ran) = self.ran.lock() {
            ran.push(call.tool.clone());
        }
        Ok(ToolOutcome::text(format!("executed {}", call.tool)))
    }
}

/// HTTP method re-export for mock assertions.
pub type SeamMethod = HttpMethod;

/// A compaction source that performs exactly one compaction (appending
/// a Condensation marker) then reports not-due — Pre/PostCompact hook
/// wrappers fire around real work, never around idle consultations.
pub struct OnceCompactor {
    done: Mutex<bool>,
}

impl OnceCompactor {
    pub fn new() -> Self {
        Self {
            done: Mutex::new(false),
        }
    }
}

#[async_trait]
impl harness_core::hooks::stop::SignalingCompactor for OnceCompactor {
    async fn is_due(&mut self, _ctx: &mut StageCtx<'_>) -> Result<bool, StageError> {
        Ok(!self.done.lock().map(|d| *d).unwrap_or(true))
    }

    async fn run_compaction(
        &mut self,
        ctx: &mut StageCtx<'_>,
    ) -> Result<harness_core::agent_loop::stages::Flow, StageError> {
        if let Ok(mut done) = self.done.lock() {
            *done = true;
        }
        ctx.append(harness_core::event::Event::Condensation {
            summary: "compacted".to_owned(),
            through_seq: harness_core::event::Sequence(1),
        })?;
        Ok(harness_core::agent_loop::stages::Flow::Continue)
    }
}
