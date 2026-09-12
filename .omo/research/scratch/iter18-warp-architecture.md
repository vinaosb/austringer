# Iteration 18 — Agent A: Warp (warp.dev) Architecture Deep-Dive

**Repo**: github.com/warpdotdev/warp | **Commit inspected**: `4143c09ff8be80ff73165f36085cf4a013295a01` (2026-09-11, HEAD)
**Clone**: partial `--filter=blob:none`, ~124MB. ~86 crates + `app/` GUI crate + `agents/specs` (Factory-produced spec corpus).
**Licensing (verified)**: `warpui`/`warpui_core` = MIT; everything else AGPL v3 — [README.md#L52-L56](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/README.md#L52-L56). Scout facts (dual MIT+AGPL, Apr 28 2026 open-sourcing) confirmed.
**Oz→Warp rename (verified in code)**: dual `OZ_*`/`WARP_*` env vars injected identically — [crates/warp_cli/src/lib.rs#L42-L54](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/lib.rs#L42-L54); legacy `oz` aliases remain in schedule/task/federate CLI help text.

---

## 1. Terminal agent architecture

### 1.1 Blocks model ("AI blocks")
- The conversation surface is a **blocklist**: `app/src/ai/blocklist/` (222 files). Core UI block is `AIBlockModel` in [block.rs](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/blocklist/block.rs#L1-L16) — 7,338 lines handling queries, outputs, diffs, todo ops, subagent calls, secret redaction, citations. Terminal commands are blocks too: `app/src/terminal/model/block.rs` (`BlockId`, `SerializedBlock`).
- History/selection/topology: `blocklist/history_model.rs`, `orchestration_topology.rs` (parent→children conversation tree, orchestrator + named agents, **credit rollup over child agents** — [orchestration_topology.rs#L1-L16](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/blocklist/orchestration_topology.rs#L1-L16)).

### 1.2 Agent driver & "full terminal use"
- `app/src/ai/agent_sdk/driver.rs` (4,612 lines) is the orchestration hub: wires `BlocklistAIHistoryModel`, `AIDocumentModel`, permissions, MCP, skills, checkpoints, ambient tasks ([driver.rs#L44-L100](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/agent_sdk/driver.rs#L44-L100)).
- Warp's own agent runs as a **TerminalDriver** — the agent acts through a real PTY inside the terminal ([driver/terminal.rs](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/agent_sdk/driver/terminal.rs)); planning notebooks surface via `ai::document` (`DEFAULT_PLANNING_DOCUMENT_TITLE`).
- **Inference is cloud-mediated**: `crates/warp_multi_agent_client` POSTs a protobuf `warp_multi_agent_api::Request` and consumes an SSE stream of protobuf `ResponseEvent`s ([warp_multi_agent_client/src/lib.rs#L43-L90](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_multi_agent_client/src/lib.rs#L43-L90)). No model loop in the client; prompt caching is therefore server-side (UNVERIFIED from OSS).

### 1.3 Cross-harness (CLI agent) integration — the standout
- **OSC 777 structured notifications**: Warp listens on the PTY for OSC 777 JSON events emitted by plugins inside CLI agents (Codex falls back to plain-text OSC 9) — [cli_agent_sessions/listener/mod.rs#L14-L30](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/terminal/cli_agent_sessions/listener/mod.rs#L14-L30).
- Supported agents in listener: **Claude, OpenCode, Codex, Gemini, Auggie, Droid, Pi, OhMyPi, Grok, WarpTui** ([listener/mod.rs#L52-L66](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/terminal/cli_agent_sessions/listener/mod.rs#L52-L66)) — Auggie/Pi via community plugins, OhMyPi/WarpTui native.
- Session status machine (InProgress/Success/Failed/Blocked/**Cancelled with 2s Ctrl-C grace window**) — [cli_agent_sessions/mod.rs#L17-L53](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/terminal/cli_agent_sessions/mod.rs#L17-L53).
- **Plugin managers** install/version-check marketplace plugins per harness: Claude via `warpdotdev/claude-code-warp` (`warp@claude-code-warp` + platform plugin `oz-harness-support@claude-code-warp`, min 2.1.0/1.1.2) — [plugin_manager/claude.rs#L8-L21](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/terminal/cli_agent_sessions/plugin_manager/claude.rs#L8-L21); siblings `codex.rs`, `gemini.rs`, `opencode.rs`.
- **Third-party harness runners** drive the CLIs as processes: `ClaudeHarness`/`CodexHarness`/`GeminiHarness` with `ResumePayload` transcripts (Claude/Codex) fetched from server transcript endpoints, `exit_escalation`, `process_control`, skill-dir publishing — [driver/harness/mod.rs#L45-L60](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/agent_sdk/driver/harness/mod.rs#L45-L60), plus `claude_code/parent_bridge.rs`, `wake_driver.rs`, `codex_transcript.rs`, `claude_transcript.rs`.
- agent_sdk also contains: `runner.rs`, `federate.rs`, `harness_support.rs`, `checkpoint_coordinator.rs`, `environment_snapshot.rs`, `artifact.rs` (+upload), `cloud_provider/{aws,gcp}.rs`, `bedrock_credentials.rs`, `agent_management.rs` — 99 files.

### 1.4 Permissions (rich, tiered)
- Typed allow/deny **with reasons**: `CommandExecutionPermission` (ExplicitlyAllowlisted, IsReadOnlyAndSettingEnabled, AgentDecided, RunToCompletion, ContainsRedirection, AlwaysAskEnabled…) — [permissions.rs#L33-L58](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/blocklist/permissions.rs#L33-L58); same for `FileReadPermission`, `FileWritePermission`, plus `AskUserQuestionPermission`, `WriteToPtyPermission`.
- `AIExecutionProfile` (server-synced execution profiles) + workspace/team `AiAutonomySettings` ([permissions.rs#L17-L29](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/blocklist/permissions.rs#L17-L29)).

---

## 2. Agent CLI standalone (`crates/warp_cli`, lib + CLI)
- Subcommand families: `agent`, `task`, `schedule`, `environment`, `skill`, `memory`, `mcp`, `secret`, `model`, `provider`, `federate`, `share`, `artifact`, `local_control`, `harness_support`, `runner` — [lib.rs#L16-L41](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/lib.rs#L16-L41). Output formats json/ndjson/pretty/text ([agent.rs#L20-L35](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/agent.rs#L20-L35)).
- `federate` = **OIDC workload-identity federation**: `issue-token` with subject templates (principal/email/teams/environment/agent_name/skill_spec/run_id/host) — [federate.rs#L17-L59](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/federate.rs#L17-L59); GCP executable-sourced credentials hidden subcommand.
- **"Mux"**: the marketing term for Warp's multi-session orchestration; in OSS code it materializes as: run-to-run messaging + ambient task spawning + shared-session join links (below). No literal `mux` symbol in client (server-side). UNVERIFIED beyond this.

---

## 3. Automation Platform (ex-Oz; rename complete per scout Oct 6 2026)
- **Cloud agents = "ambient agent tasks"**: `app/src/ai/ambient_agents/` — `spawn.rs` streams spawn/follow-up via `SpawnAgentRequest`/`RunFollowupRequest` with 3s status polling, shared-session **join links** ([spawn.rs#L1-L45](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/ambient_agents/spawn.rs#L1-L45)).
- **Triggers**: `warp schedule create|list|update|pause|unpause` (cron schedules; "scheduled Oz agent" wording) — [schedule.rs#L48-L62](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/schedule.rs#L48-L62).
- **Self-hosted workers**: schedule targets match a **worker host ID** for self-hosted jobs vs. Warp-hosted — [schedule.rs#L134-L135](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/schedule.rs#L134-L135); `integration.rs` carries worker host IDs.
- **Cloud environments**: `warp environment create --docker-image --repo --setup-command`, base image management — [environment.rs#L21-L64](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/environment.rs#L21-L64).
- **Isolation platforms**: Docker / DockerSandbox / Kubernetes / Namespace + workload tokens — [isolation_platform/src/lib.rs#L26-L43](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/isolation_platform/src/lib.rs#L26-L43). **Build caching** (repo-level cache restore) only enabled on Namespace — [driver/cache_setup.rs#L23-L31](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/agent_sdk/driver/cache_setup.rs#L23-L31).
- **Agent-to-agent messaging**: `task message send|watch|list|read|mark-delivered` (multi-recipient runs, inbox) — [task.rs#L20-L57](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/task.rs#L20-L57).
- **Managed secrets**: envelope encryption (`UploadKey`, `init_envelope`), GCP federation, per-task identity tokens — [crates/managed_secrets/src/lib.rs](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/managed_secrets/src/lib.rs#L1-L13).

## 4. Warp Factories (EA — NOT in OSS client)
- README: Factories = "defined in code", power this very repo, with "evals, benchmarks, and self-improvement built in"; dashboard at build.warp.dev (web-compiled Warp terminals) — [README.md#L38-L50](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/README.md#L38-L50).
- Physical evidence in-repo: `agents/specs/*.md` (Factory-generated spec files with ticket IDs), `.warp/workflows/*.yaml` (config-as-code workflows). No Factory config schema in OSS — UNVERIFIED beyond README claims.

## 5. Knowledge & context
- **Warp Drive**: cloud objects with folders, sharing, workflows, import/export, trash — [app/src/drive/mod.rs#L1-L33](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/drive/mod.rs#L1-L33); `crates/cloud_object*` persistence family.
- **Rules**: `WARP.md` + `AGENTS.md` as repo rule files (`project_rule_file_names: ["WARP.md","AGENTS.md"]` — [repo_metadata/standing_queries.rs#L22](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/repo_metadata/src/standing_queries.rs#L22)); init-flow also checks/migrates `CLAUDE.md` ([init_project/mod.rs#L48-L51](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/terminal/view/init_project/mod.rs#L48-L51)); per-harness **global rules** dirs (Agents→`AGENTS.md`) — [project_context/global_rules.rs#L39-L42](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/ai/src/project_context/global_rules.rs#L39-L42); rules readable over SSH via remote context files.
- **Codebase indexing**: `crates/repo_metadata` (gitignore-aware standing queries) + `crates/ai/src/index`: file **outlines** (Outline/Symbol), **full-source-code embeddings with Merkle-tree snapshots** — [index/mod.rs](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/ai/src/index/mod.rs#L1-L20); telemetry emits Merkle rebuild/diff + sync fragment metrics (crates/ai/src/telemetry.rs).
- **Skills**: `.agents/skills` (project) + `~/.agents/skills` (global), `skills-lock.json` via `npx skills` (AGENTS.md#L41-L54); SkillManager + **hot-reload file watchers** ([skills/file_watchers/](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/skills/file_watchers/skill_watcher.rs)); bundled skills in `resources/bundled/skills` (incl. `claude-api`); Drive-remote skills.
- **Agent Memory (research preview)**: cloud **memory stores** with agents attached; memory CRUD with **version history** — [crates/warp_cli/src/memory_store.rs#L8-L44](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warp_cli/src/memory_store.rs#L8-L44).
- **Windowed file view**: agent file-content actions carry `line_range` (start/end) — [crates/ai/src/agent/action/mod.rs](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/ai/src/agent/action/mod.rs).

## 6. MCP
- Official Rust SDK **rmcp 1.6** client (Cargo.toml#L425), SSE transport + OAuth — [crates/mcp/src/lib.rs](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/mcp/src/lib.rs#L1-L17).
- **File-based, multi-provider**: `FileBasedMCPManager` scans config files across providers (per-repo + global), hash-keyed templatable installations, per-path parse diagnostics, deferred auth-gated autostart in TUI — [file_based_manager.rs#L21-L66](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/mcp/file_based_manager.rs#L21-L66); templatable (variable-substitution) servers + gallery + reconnecting peer. Repo root ships `.mcp.json`.
- **MCP server mode (Warp as MCP server)**: no evidence in OSS — UNVERIFIED.

## 7. Rendering / frontends
- **wgpu 30 + WGSL**, `cosmic-text` (Warp fork), winit; Flutter-style Element/View layout — [crates/warpui/Cargo.toml#L54-L166](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/warpui/Cargo.toml#L54-L166). Entity-Component-Handle core shared by both frontends; terminal emulation from Alacritty.
- **Two frontends + web**: GUI (`app/`) and headless **TUI** (`crates/warp_tui`, cell-grid `TuiElement`, crossterm) sharing core ([AGENTS.md#L56-L97](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/AGENTS.md#L56-L97)); WASM target pervasive (`cfg(target_family="wasm")`), `crates/serve-wasm`, `crates/warp_js`, web event bus + websocket crates → build.warp.dev web terminal.
- **Sessions**: diesel/SQLite local persistence ([crates/persistence](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/persistence/src/lib.rs#L1-L5)), cloud session sharing (session_sharing_protocol, `share.rs`), harness **resume** via server-side transcripts (Claude/Codex ResumePayload).
- **Checkpoints/rewind**: periodic **workspace checkpoint coordinator** (5-state machine, safe-boundary gating, upload via HarnessSupportClient) — [checkpoint_coordinator.rs#L1-L25](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/agent_sdk/driver/checkpoint_coordinator.rs#L1-L25); file-rewind dialogs in workspace.
- **Cost HUD**: credit/dollar usage model with `RequestLimitInfo`, denial reasons, bonus grants — [request_usage_model.rs](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/app/src/ai/request_usage_model.rs#L1-L36); GUI usage/spend unit setting (credits|dollars) in settings/ai.rs; child-agent credit rollup.
- **SSH**: standard-PTY SSH works with no remote component; optional `crates/remote_server` host binary (`install_remote_server.sh`, `preinstall_check.sh`) adds managed sessions + codebase-index protobufs ([crates/remote_server/src/lib.rs#L1-L15](https://github.com/warpdotdev/warp/blob/4143c09ff8be80ff73165f36085cf4a013295a01/crates/remote_server/src/lib.rs#L1-L15)).

---

## FULL MATRIX ROW — Warp

Legend: ● full evidence-backed | ◐ partial/heuristic | ○ absent or unverifiable from OSS client

| Dimension | Score | Evidence |
|---|---|---|
| language | ● | 100% Rust OSS client; MIT(warpui)/AGPL(rest); README#L52-56 |
| steady RSS | ○ | Not measurable from source. GUI=wgpu30 GPU stack; TUI=headless cell-grid (lower); no published numbers — UNVERIFIED |
| OS sandbox | ◐ | isolation_platform: Docker/DockerSandbox/K8s/Namespace (cloud/self-hosted); local runs gated by permission engine, **no per-command OS sandbox (no seatbelt/apparmor code found)** |
| permission tiers | ● | Typed Allow/Deny+reason for command/file-read/file-write/ask-user/write-to-pty; execution profiles (AgentDecided/AlwaysAsk/RunToCompletion/allow/deny); workspace+team scope — permissions.rs#L33-58 |
| prefix-cache | ○ | Client has no cache_control plumbing; inference is server-side SSE protobuf; bundled claude-api skill documents prefix caching for harnesses — server behavior UNVERIFIED |
| AST repomap | ◐ | No Aider-style repomap; instead: tree-sitter (arborium) file **outlines** + full-source **embeddings** + Merkle-tree snapshot sync (crates/ai/src/index) |
| windowed file view | ● | `line_range` (start/end) on agent file-content actions — crates/ai/src/agent/action |
| PTY scraping | ● | Terminal-native: OSC 777 JSON notifications from in-agent plugins (OSC 9 Codex fallback); 10 agents tracked incl. community plugins; listener/mod.rs#L14-30 — best-in-class |
| tiered eviction | ◐ | Summarization-based conversation compaction (`SummarizationType`, summarization_cancel_dialog); no tiered/eviction store in client; context assembly server-side |
| subagents | ● | Orchestration tree (orchestrator+named agents, child tracker), `prepare_local_oz_child_launch` w/ parent_run_id, RunAgentsRequest/SubagentType, run-to-run messaging |
| multi-model spec launch | ● | `HarnessModelConfig`/`AgentConfigSnapshot`/`HarnessConfig` (cloud_object_models); spawn w/ model+harness per child; Claude/Codex/Gemini harnesses + TerminalDriver |
| model profiles | ● | Server-synced AIExecutionProfilesModel; `warp agent profile list`; profiles drive permissions & model selection |
| skills | ● | .agents/skills project+global, skills-lock.json, hot-reload watchers, bundled+Drive skills, skill_dirs_publish to harnesses |
| auto skill learning | ◐ | No autonomous skill-writing loop in client; adjacent: suggested-rule modal (auto-suggests rules), Agent Memory (versioned, cross-harness preview) |
| MCP client | ● | rmcp 1.6, SSE+OAuth, templatable servers, multi-provider file watcher, gallery, auto-start, repo `.mcp.json` |
| MCP server mode | ○ | No client evidence (jsonrpc/http_server crates exist but no Warp-as-server surface) — UNVERIFIED |
| WASM | ● | cfg(target_family="wasm") throughout, serve-wasm + warp_js crates, wasm index paths, web-compiled Warp (build.warp.dev) |
| sessions | ● | SQLite(diesel) local, Drive cloud sync, session sharing/join links, cross-machine harness resume via server transcripts |
| encryption at rest | ◐ | Managed secrets envelope-encrypted (UploadKey); secrets in keychain; **local SQLite not encrypted (no SQLCipher found)**; server-side ZDR claim = marketing, code UNVERIFIED |
| checkpoints/rewind | ● | Periodic workspace CheckpointCoordinator (safe-boundary state machine) + end-of-run snapshots + file rewind UI |
| cost HUD/budgets | ● | Credit/dollar usage+spend model, RequestLimitInfo refresh, denial reasons, ambient-trial CTA threshold, child credit rollup; hard budget caps = team quota (server) ◐ |
| frontends | ● | GUI (wgpu) + headless TUI + web/WASM + Agent CLI — one shared core (AGENTS.md#L56-97) |
| multi-client | ● | Cloud-synced objects/sessions across GUI/TUI/web/CLI; shared_session join links; websocket + web_event_bus |
| telemetry | ● | Typed registry (`register_telemetry_event`), AITelemetryEvent (incl. index sync metrics), Sentry crash reporting, third-party-harness telemetry, UGC telemetry gating |
| evals | ◐ | No eval harness in OSS client; Factories promise "evals, benchmarks, self-improvement built in" (README#L48-50); repo itself Factory-built (agents/specs) |

**Benchmarks caution**: client contains no eval/leaderboard artifacts; scout's TB #1 52% / SWE-bench V 75.8% are 2025-dated vendor claims — UNVERIFIED here.

### Architecture verdict (3 lines)
1. Warp's moat is the **terminal-as-runtime** inversion: its own agent, third-party harnesses (Claude/Codex/Gemini/opencode), and human operators all act through PTYs observed via OSC escape sequences — one observer plane for everything.
2. Intelligence is deliberately **cloud-split**: SSE/protobuf inference, transcript storage, task orchestration, schedules, memory, secrets, and workers live server-side; the OSS client is the (very large) cockpit — so capability parity claims must be audited per-surface, not from the repo.
3. Cross-harness depth (plugin marketplaces, per-harness resume transcripts, global rule dirs per tool, skill-dir publishing) is the deepest OSS-visible integration layer of any terminal; Factories/Automation Platform enterprise features remain closed (EA).
