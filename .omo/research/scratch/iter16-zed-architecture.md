# Zed — Architecture Deep-Dive (Iteration 16, Agent A)

**Repo**: [zed-industries/zed](https://github.com/zed-industries/zed) — Rust monorepo, dual GPL-3.0 / Apache-2.0 per-crate (e.g. `crates/agent_skills/LICENSE-GPL` + APL).
**Verified HEAD**: `a9cdfc9936c4ccf0503ee4631204e045f913e2c1` (2026-09-11); tag `v1.19.2` exists in-repo (matches scout's stable release).
**Method**: blob:none clone → direct source reads. All paths below relative to repo root. Permalink base: `https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/`.

---

## 1. Native Agent Loop (crates/agent)

The native agent is **implemented as an ACP server against Zed's own host** — dogfooding: `NativeAgentServer` implements `AgentServer::connect() -> Rc<dyn acp_thread::AgentConnection>` ([native_agent_server.rs#L23-L54](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/native_agent_server.rs#L23-L54)). Native and external agents are indistinguishable to the host UI.

**Turn loop** (`Thread::run_turn` → `run_turn_internal`): per-iteration it (a) runs `perform_compaction_if_needed`, (b) re-reads model + refreshes tools so mid-turn model/profile/tool switches take effect between tool rounds, (c) streams completion with batched event processing, parallel tool results via `FuturesUnordered`, early-abort on erroring in-flight tool input, cancellation watch channel, refusal→fallback-model retry with attempt counter ([thread.rs#L2753-L2969](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L2753-L2969)). Stop reasons modeled: EndTurn / Refusal (truncates messages) / MaxTokens ([thread.rs#L2724-L2744](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L2724-L2744)).

**Tools (29 files, crates/agent/src/tools/)**: read/write/edit file, grep, find path, list dir, terminal + sandboxed-terminal variants (one `terminal` name exposed matching sandbox state — [thread.rs#L2177-L2183](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L2177-L2183)), web search, fetch, diagnostics, go-to-def / find-refs / code-actions (LSP-backed), ask_user, copy_path, spawn_agent, create_thread, list_agents_and_models, skill tool, context-server (MCP) tools.

**Profiles**: builtin `write` / `ask` / `minimal` ([agent_profile.rs#L16-L21](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_settings/src/agent_profile.rs#L16-L21)) + custom; restricted worktrees auto-downgrade write/ask→minimal unless user-customized ([thread.rs#L2240-L2265](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L2240-L2265)).

**Compaction**: summarize-strategy; `Thread::compact()` forces manual compaction ([thread.rs#L2560-L2650](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L2560-L2650)); auto via `AutoCompactThreshold` = Percentage | TokensUsed | TokensRemaining, 0 invalid ([agent_settings.rs#L143-L198](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_settings/src/agent_settings.rs#L143-L198)); `COMPACTION_PROMPT` include_str ([agent_settings.rs#L32](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_settings/src/agent_settings.rs#L32)); dedicated `compaction_model` setting; auto-compact gated to models with ≥ some context floor (comment at thread.rs:120); compaction telemetry with outcomes manual/auto/canceled/failed + retries.

**Checkpoints/rewind**: host-side, in `acp_thread`, NOT in the agent crate. `Checkpoint { git_checkpoint: GitStoreCheckpoint }` attached per user message ([acp_thread.rs#L306-L310](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/acp_thread/src/acp_thread.rs#L306-L310)); `rewind()` restore path ([acp_thread.rs#L4116](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/acp_thread/src/acp_thread.rs#L4116)); checkpoint auto-updates when repo state changes (`update_last_checkpoint_if_changed`, [acp_thread.rs#L2336-L2337](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/acp_thread/src/acp_thread.rs#L2336-L2337)); restoring a checkpoint kills terminals (test `test_restore_checkpoint_kills_terminal` acp_thread.rs:9130). Works uniformly for native + external agents.

## 2. Parallel Agents / Subagents

- **spawn_agent tool**: doc text mandates disjoint write sets, parallel delegation patterns, session_id follow-ups; output = final message only ([spawn_agent_tool.rs#L13-L47](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/tools/spawn_agent_tool.rs#L13-L47)).
- **Depth limit**: `pub const MAX_SUBAGENT_DEPTH: u8 = 1;` on main ([thread.rs#L77](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L77)) — **CONFLICTS with scout's "depth≤4"; current main enforces 1 nested level**. Enforcement at [agent.rs#L3196-L3201](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/agent.rs#L3196-L3201); tool only registered while `depth() < MAX_SUBAGENT_DEPTH` ([thread.rs#L2203-L2205](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L2203-L2205)). Subagent tree: `parent_session.subagents.push(acp_thread)` + "Subagent Started" telemetry event ([agent.rs#L3220-L3236](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/agent.rs#L3220-L3236)).
- **Sibling threads**: `CreateThreadTool` at every depth (flag-gated `CreateThreadToolFeatureFlag`), with **worktree isolation**: `use_new_worktree` creates linked git worktrees of each git repo + `ref` (default HEAD) ([create_thread_tool.rs#L40-L95](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/tools/create_thread_tool.rs#L40-L95)); full worktree archive/restore lifecycle via `git worktree remove` in [thread_worktree_archive.rs#L21-L95](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_ui/src/thread_worktree_archive.rs#L21-L95).
- **Persistence**: `ThreadStore` global entity ([thread_store.rs#L12](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread_store.rs#L12)) → SQLite `threads.db` via `Connection::open_file` ([db.rs#L446-L447](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/db.rs#L446-L447)).

## 3. ACP Host + External Agents

- External agents = child processes over stdio JSON-RPC: `Child::spawn(..., Stdio::piped(), Stdio::piped(), Stdio::piped())`, line-buffered stdout reader, background stderr log task, dispatch + wait tasks ([agent_servers/acp.rs#L846-L918](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_servers/src/acp.rs#L846-L918)); built-in auth methods incl. Gemini CLI spawn (acp.rs:45). Agent binaries auto-installed from GitHub release assets with SHA-256 verification (`Sha256` import, agent_server_store.rs:25; `AgentServerCommand { path, args, env }` with env redaction [agent_server_store.rs#L35-L66](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/project/src/agent_server_store.rs#L35-L66)).
- **Protocol version**: code imports `agent_client_protocol::schema::v1` ([thread.rs#L19](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/thread.rs#L19)) — v1 schema in-tree. ACP is Zed-authored (scout: agentclientprotocol.org, v1 stable, 5 SDKs).
- **Terminal threads**: `acp_thread::Terminal` + `SandboxWrap` ([acp_thread/terminal.rs#L35, L402-L430](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/acp_thread/src/terminal.rs#L35)) — external agent commands run in host-owned headless terminals with sandbox wrap metadata; "sandbox not applied" reason recording ([acp_thread.rs#L258, L877-L907](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/acp_thread/src/acp_thread.rs#L877-L907)).

## 4. OS Sandbox + Permissions

- **Sandbox**: `sandbox` crate glue; **macOS Seatbelt, Linux Bubblewrap, Windows Bubblewrap-via-WSL** ([sandboxing.rs#L20-L23](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/sandboxing.rs#L20-L23)); feature-flag gated; auto-granted writable paths = worktree roots + `.git` metadata protection ([sandboxing.rs#L42-L85](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/sandboxing.rs#L42-L85)); network policy (HostPattern); explicit "no sandbox" variant vs wide-open sandbox; model `unsandboxed:true` escape requires once/thread approval ([sandboxing.rs#L87-L100](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/sandboxing.rs#L87-L100)).
- **Permission tiers**: `ToolPermissionMode` = Allow | Deny | Confirm, global default + per-tool override + profile-scoped ([tool_permissions.rs#L275-L297](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/tool_permissions.rs#L275-L297)); "follow-up instead of approval = permission denied" UX (thread.rs:73-75).

## 5. MCP

- **Client**: `crates/context_server` — stdio/HTTP transports (transport/, oauth.rs), MCP protocol-version negotiation stamping `MCP-Protocol-Version` ([client.rs#L501-L502](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/context_server/src/client.rs#L501-L502)); registered via `project/context_server_store.rs` (exists, verified); per-thread `ContextServerRegistry` tool.
- **Server mode**: `context_server::McpServer` serves tools over a Unix socket in a `zed-mcp` tempdir ([listener.rs#L34-L59](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/context_server/src/listener.rs#L34-L59)) — **but no `McpServer::new` callers found in crates/ (grep)**; library capability, production wiring UNVERIFIED. WIT `context-server.wit` lets Wasm extensions act as MCP servers (extension_api/wit/since_v0.6.0/context-server.wit).

## 6. Skills

`crates/agent_skills`: `.agents/skills/` global (`~/` or `%USERPROFILE%\`) + per-project scopes; `SKILL.md` YAML frontmatter; caps: 100KB file, 50KB total descriptions, IO concurrency 16 ([agent_skills.rs#L14-L53](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_skills/agent_skills.rs#L14-L53)); `disable_model_invocation` flag ([agent_skills.rs#L85-L88](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_skills/agent_skills.rs#L85-L88)); builtin `create-skill` skill = guided skill authoring (auto skill learning is assistant-assisted, not autonomous loop) ([builtin/create-skill/SKILL.md](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_skills/builtin/create-skill/SKILL.md)).

## 7. GPUI / Perf Discipline

- **Frame budget**: GPUI hang profiler — `HANG_THRESHOLD = 10ms`, `FRAME_BUDGET = HANG_THRESHOLD`, HangDetector attributes foreground spend per frame ([gpui/src/profiler/hang.rs#L33-L76, L484-L487](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/gpui/src/profiler/hang.rs#L484-L487)).
- **Data structures**: own `rope` crate (crates/rope/src/rope.rs) + `sum_tree` (crates/sum_tree/src/sum_tree.rs, tree_map.rs) — the incremental trees backing buffers/editors; agent file reads slice via `Buffer::text_for_range` on the same buffers.
- Agents get **line-windowed reads**: 1-indexed inclusive `(start_line,end_line)` → buffer row ranges ([read_file_tool.rs#L22-L32](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/tools/read_file_tool.rs#L22-L32)).

## 8. Extension Host (WASM)

- **Wasmtime 48** + **wasmtime-wasi 48**, default-features off (workspace Cargo.toml, verified lines).
- **WIT versioning**: 10 versioned WIT dirs `crates/extension_api/wit/since_v0.0.1 … since_v0.8.0` (verified glob, 77 .wit files) — per-release API surface incl. `lsp.wit`, `dap.wit`, `slash-command.wit`, `context-server.wit`, `process.wit`, `github.wit`, `http-client.wit`, `nodejs.wit`.
- **Capability security**: `ExtensionCapability` enum = `process:exec` | DownloadFile | `npm:install` ([extension/src/capabilities.rs#L11-L20](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/extension/src/capabilities.rs#L11-L20)) — extensions must declare capabilities; wasm sandbox = default-deny otherwise.

## 9. Models / Cache / Telemetry / Evals

- **Per-role model profiles**: `default_model`, `subagent_model`, `inline_assistant_model`, `commit_message_model`, `thread_summary_model`, `compaction_model` ([agent_settings.rs#L214-L222](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_settings/src/agent_settings.rs#L214-L222)); refusal fallback model + mid-turn model switch (thread.rs:2762, 2839-2852); `ListAgentsAndModelsTool` exposes agent/model catalog to the model itself ([list_agents_and_models_tool.rs#L19](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent/src/tools/list_agents_and_models_tool.rs#L19)).
- **Prefix cache**: Anthropic explicit `cache_control` / `mark_last_cacheable_content` ([anthropic/src/completion.rs#L129-L164](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/anthropic/src/completion.rs#L129-L164)); Google AI only accounts `cached_content_token_count` → billing split ([google_ai/src/completion.rs#L451-L477](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/google_ai/src/completion.rs#L451-L477)). No unified cross-provider cache strategy.
- **No repo-map anywhere** (grep `repo_map|repomap` → 0 hits). Agents get tree-sitter outlines + LSP go-to-def/find-refs/diagnostics + grep instead.
- **Cost HUD**: live `thread.cost()` label with currency + precision handling ([agent_ui/thread_view.rs#L4687-L4693](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/agent_ui/src/conversation_view/thread_view.rs#L4687-L4693)) + "Cost" panel (thread_view.rs:5793); **no budget caps found** (grep budget in agent_settings → 0).
- **Telemetry**: first-class `telemetry::event!` — "Agent Thread Completion" (thread_id, parent_thread_id, prompt_id, model, attempt, thread.rs:2854-2864), "Subagent Started" (agent.rs:3230); compaction outcome telemetry; `telemetry` + `telemetry_events` crates.
- **Evals**: `eval-cli` = "Headless CLI binary for running Zed's agent in evaluation/benchmark environments", `ZED_EVAL_ENABLE_FLAGS` flag hook, JSON `EvalResult` output ([eval_cli/src/main.rs](https://github.com/zed-industries/zed/blob/a9cdfc9936c4ccf0503ee4631204e045f913e2c1/crates/eval_cli/src/main.rs)); plus `eval_utils`, in-agent `tools/evals.rs`, shared `agent_servers::e2e_tests` macro (native_agent_server.rs:67).
- **Remote**: SSH-only headless `remote_server` + proxy (crates/remote: remote.rs, proxy.rs, remote_client.rs) — no "Zed Cloud" app server in-repo; `ZED_CLOUD_PROVIDER_ID` exists only as model-provider auth channel (thread.rs:48).

## FULL MATRIX ROW — Zed @ v1.19.2 / a9cdfc9

| Dimension | Score | Evidence (one-liner) |
|---|---|---|
| Language | ● | Rust monorepo, GPL-3.0+Apache-2.0 dual-crate licensing |
| Steady RSS | ◐ UNVERIFIED | No RSS benchmark in-repo; GPUI native stack implies lean, no numbers verified |
| OS sandbox | ● | Seatbelt/Bubblewrap/Bubblewrap-via-WSL (sandboxing.rs:20-23); git-metadata protection; net policy; ⚠ Windows path = WSL-dependent, feature-flag gated |
| Permission tiers | ● | Allow/Deny/Confirm global+per-tool+profile; restricted-workspace downgrade; redacted env logging |
| Prefix-cache | ◐ | Anthropic explicit cache_control markers; Google AI cached-token accounting only; no unified strategy |
| AST repomap | ◐ | Tree-sitter heritage everywhere, but NO repo-map tool (grep 0); agents get LSP defs/refs/diagnostics + grep + outlines |
| Windowed file view | ● | resolve_line_range 1-indexed inclusive → Buffer::text_for_range slices (read_file_tool.rs:22-32) |
| PTY scraping | ● | alacritty_terminal from own fork rev 4c12966 (workspace Cargo.toml); host-owned headless Terminals w/ SandboxWrap; restore-ckpt kills terminals |
| Tiered eviction | ○ | Single summarize-compaction (auto %/token thresholds + manual force); no tiered/multi-layer eviction found |
| Subagents | ● | SpawnAgentTool + parallel-delegation doc contract + session_id follow-ups + subagent tree + telemetry; ⚠ **MAX_SUBAGENT_DEPTH=1 on main (scout "≤4" CONFLICT)** |
| Multi-model spec launch | ● | Per-role models (default/subagent/inline/commit/summary/compaction); refusal-fallback model; mid-turn switch; ListAgentsAndModelsTool; CreateThreadTool flag-gated |
| Model profiles | ● | builtin write/ask/minimal + custom; profile downgrade rules; per-profile tool visibility |
| Skills | ● | .agents/skills SKILL.md, global+project scopes, size caps, disable_model_invocation |
| Auto skill learning | ◐ | Builtin create-skill = guided authoring on request; no autonomous skill-learn loop |
| MCP client | ● | context_server crate: stdio/HTTP+OAuth, protocol-version negotiation, store + registry tools |
| MCP server mode | ◐ | context_server::McpServer Unix-socket tool server in-tree but zero production callers found (UNVERIFIED wiring); WIT context-server.wit for wasm extensions |
| WASM | ● | Wasmtime 48 + wasi 48; WIT since_v0.0.1→v0.8.0 (77 files, 10 versions); capability system process:exec/download/npm:install |
| Sessions | ● | SQLite threads.db + ThreadStore global; resume; subagent sessions persisted (SubagentSessionInfo) |
| Encryption at rest | ○ | Plain SQLite (`Connection::open_file threads.db`); no SQLCipher/PRAGMA/encrypt hits in db.rs |
| Checkpoints/rewind | ● | GitStoreCheckpoint per user msg, rewind(), auto-update on repo change, terminal kill on restore; uniform for native+external agents |
| Cost HUD/budgets | ◐ | Live cost label + Cost panel; no budget/limit enforcement found |
| Frontends | ◐ | GPUI desktop editor + headless eval-cli + remote server; no web/mobile frontend in-repo |
| Multi-client | ● | ACP host multiplexes native+external agents (NativeAgentServer + stdio children); Delta multiplayer app is EXTERNAL (scout-verified, not in-repo — UNVERIFIED here) |
| Telemetry | ● | Structured events: Agent Thread Completion (parent_thread_id, prompt_id, attempt), Subagent Started (depth), compaction outcomes |
| Evals | ● | eval-cli headless harness (ZED_EVAL_ENABLE_FLAGS, JSON EvalResult), eval_utils, tools/evals.rs, shared e2e macro |

**Scout-fact deltas**: (1) subagent depth is **1**, not ≤4, on 2026-09-11 main; (2) no "Zed Cloud" server confirmed — SSH remote + cloud provider auth only; (3) checkpoints live in acp_thread (host), not agent crate.
