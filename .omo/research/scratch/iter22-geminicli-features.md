# Iteration 22 / Agent B — Gemini CLI Deep-Dive (FINAL current state, 2026-09-12)

Repo: google-gemini/gemini-cli @ HEAD `9c1b0a610534d6f8120964cf2672c07807d8fc90` (2026-09-11). Docs: https://geminicli.com/docs/

## Status: ENTERPRISE-ALIVE / CONSUMER-DEAD

Consumer tiers (Google AI Pro/Ultra, free Code Assist individuals, Code Assist for GitHub) stopped being served **June 18, 2026**; the product was handed to **Antigravity CLI** (closed Go binary). The Apache-2.0 OSS repo continues for **enterprise license + paid-API-key users only** — 148 commits landed since Jun 18 (verified via `git rev-list --count --since=2026-06-18 HEAD`), HEAD dated 2026-09-11 with a security-hardening focus (v0.59.0 stable, v0.60.0-preview, v0.61.0-nightly in flight; recent commits: sandbox boundary hardening #29214, indirect prompt-injection fixes #29250/#29215, untrusted tool output provenance).

- Docs banner confirming shutoff: https://geminicli.com/docs/ ("Unpaid tier and Google One users: Gemini CLI was replaced by Antigravity CLI on June 18th, 2026")
- Announcement (May 19, 2026): https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/
- Maintainer shutoff post (Jun 18, 2026): https://github.com/google-gemini/gemini-cli/discussions/28017
- Announcement discussion: https://github.com/google-gemini/gemini-cli/discussions/27274

## 1. Tool set + agent loop (current state)

- Tool registry (`packages/core/src/tools/`, base permalink https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/packages/core/src/tools/tool-registry.ts):
  - File: read-file (windowed), edit, write-file, glob, grep + ripGrep binary, ls, read-many-files
  - Exec: shell + shellBackgroundTools (async background shells)
  - Web: web-search (grounded), web-fetch
  - Memory/plan: memoryTool (save_memory), write-todos, enter-plan-mode/exit-plan-mode
  - Meta: ask-user, activate-skill, list-mcp-resources, read-mcp-resource, topicTool, trackerTools, get-internal-docs, jit-context (just-in-time context, https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/packages/core/src/tools/jit-context.ts)
  - Safety: omissionPlaceholderDetector (catches model eliding file content in write_file)
- Agent loop: turn-based scheduler (packages/core/src/scheduler/, ToolCallRequestInfo in scheduler/types.ts), core/turn.ts streaming turns, agent/event-translator.ts. New agent/ + context/ subsystems: contextManager with incremental/hotstart/barrier variants, pipeline.ts, tracer.ts.
- Extras: Chromium browser agent (agents/browser/browserManager.ts), voice input (core/src/voice/: Whisper local + Gemini Live transcription, whisperModelManager), experimental (flagged in docs as \ud83d\udd2c): Plan Mode + model steering, git worktrees, notifications, Auto Memory.
- MCP client: full OAuth — mcp-oauth-provider.ts, google-auth-provider.ts, sa-impersonation-provider.ts (service-account impersonation), oauth-token-storage with OS-keychain (keychain-token-storage.ts) + hybrid file fallback; resource tools (https://geminicli.com/docs/tools/mcp-resources/); server/tool filtering via excludeTools + allowed-servers settings (https://geminicli.com/docs/tools/mcp-server/).
- Extensions + consent model: explicit third-party install warning + skill prompt-injection warning with per-install consent flow — packages/cli/src/config/extensions/consent.ts (INSTALL_WARNING_MESSAGE, SKILLS_WARNING_MESSAGE, skillsConsentString); https://geminicli.com/docs/extensions/
- Skills: .gemini/skills + SKILL.md, skillLoader/skillManager, activate-skill tool, skill-extraction-agent; https://geminicli.com/docs/cli/skills/
- Hooks: full system — hookRegistry, hookRunner, hookAggregator, hookPlanner, trustedHooks, runtimeHooks (core/src/hooks/); https://geminicli.com/docs/hooks/
- Subagents: .gemini/agents profiles + built-ins (codebase-investigator, cli-help-agent, generalist-agent, skill-extraction-agent) + agent-scheduler + acknowledgedAgents registry (agents/agent-tool.ts, agentLoader.ts) + REMOTE A2A subagents (a2a-client-manager.ts, remote-subagent-protocol.ts; https://geminicli.com/docs/core/remote-agents/)
- Headless: -p non-interactive mode with JSON/stream output (https://geminicli.com/docs/cli/headless/); automation tutorial + GitHub Actions surfaces.
- GCA (Gemini Code Assist) integrations: core/src/code_assist/ (Code Assist server client, oauth2, oauth-credential-storage, telemetry); powers Code Assist IDE extensions + Code Assist for GitHub; enterprise config https://geminicli.com/docs/cli/enterprise/

## 2. Sandbox story — best-in-class breadth

Five backends + tool-level sandboxing (https://geminicli.com/docs/cli/sandbox/, updated Jul 17, 2026):

1. macOS Seatbelt (sandbox-exec), 6 built-in profiles via SEATBELT_PROFILE: permissive-open (default), permissive-proxied, restrictive-open, restrictive-proxied, strict-open, strict-proxied — core/src/sandbox/macos/seatbeltArgsBuilder.ts
2. Container-based Docker/Podman, default image ghcr.io/google/gemini-cli:latest; custom images via GEMINI_SANDBOX_IMAGE / settings object; auto-build from .gemini/sandbox.Dockerfile with BUILD_SANDBOX=1
3. Windows Native Sandbox: icacls Low Mandatory Level + C# helper core/src/sandbox/windows/GeminiSandbox.cs + WindowsSandboxManager
4. gVisor/runsc (strongest; docker --runtime=runsc; explicit opt-in)
5. LXC/LXD (experimental, full-system, for snapcraft/rockcraft)

Plus: tool-level sandboxing with bubblewrap on Linux (sandbox/linux/bwrapArgsBuilder.ts + LinuxSandboxManager), "Sandbox Expansion Requests" (interactive per-command privilege elevation), SANDBOX_MOUNTS host mounts, proactivePermissions/commandSafety utils. Policy engine adds fine-grained allow/deny rules: https://geminicli.com/docs/reference/policy-engine/ — combined with approval modes, trusted folders (https://geminicli.com/docs/cli/trusted-folders/), read-only plan mode, confirmation-bus. Latest commit (Sep 11) hardens sandbox boundaries (#29214).

## 3. Auth / quota current state

Four auth paths: Google OAuth (Code Assist licenses), Gemini API key, Vertex AI (incl. Express Mode), Workspace. Quota page (updated Jun 18, 2026 — shutoff day) still lists consumer tiers: Code Assist Individual 1000 req/day, AI Pro 1500, AI Ultra 2000, free API key 250/day Flash-only, Vertex Express 90-day: https://geminicli.com/docs/resources/quota-and-pricing/ — now consumer-dead in practice.

STALENESS EVIDENCE: README still advertises "Free tier: 60 requests/min and 1,000 requests/day with personal Google account" (https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/README.md) while the docs banner says unpaid tiers were cut Jun 18 — README/docs contradiction. ROADMAP.md also still carries the pre-transition "Free and Open Source: cost isn't a barrier" principle.

Enterprise: Code Assist Standard (1500/day) + Enterprise (2000/day) licenses and paid Gemini / Gemini Enterprise Agent Platform API keys remain fully served (discussion #27274; enterprise Antigravity counterpart: https://antigravity.google/docs/enterprise/).

## 4. a2a-server + SDK surfaces

- packages/a2a-server ("Gemini CLI A2A Server" per its package.json description): exposes the agent as an A2A-protocol HTTP server — src/http/app.ts + endpoints/server.ts, event-driven task executor (src/agent/executor.ts, task.ts); own config + extension allowlist filtering (src/config/config.ts, extension.ts). https://github.com/google-gemini/gemini-cli/tree/9c1b0a610534d6f8120964cf2672c07807d8fc90/packages/a2a-server
- @google/gemini-cli-sdk: embeddable GeminiCliAgent with sendStream (packages/sdk/src/agent.ts; session.ts, tool.ts, skills.ts, shell.ts, fs.ts; SDK_DESIGN.md). https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/packages/sdk/README.md
- ACP mode for editor embedding (Zed etc.): https://geminicli.com/docs/cli/acp-mode/ ; vscode-ide-companion package provides IDE context MCP.

## 5. Antigravity handoff mechanics

MIGRATED AUTOMATICALLY (Antigravity CLI first-run detection of ~/.gemini, per discussion #28017): Skills (auto-import), MCP servers (all configured), Agents (profiles preserved), GEMINI.md memory (full compatibility).

MIGRATED WITH CONVERSION: Extensions renamed to "Antigravity plugins" via `agy plugin import gemini`; skills path .gemini/skills/ -> .agents/skills/; MCP config moved to standalone mcp_config.json (~/.gemini/config/mcp_config.json global; .agents/mcp_config.json workspace; serverUrl replaces url/httpUrl). Sources: https://mcp.directory/blog/antigravity-cli-vs-gemini-cli-migration-2026 ; https://dvnc.dev/blog/gemini-cli-vs-antigravity-cli-production-migration

NOT MIGRATED / NOT 1:1: custom themes + experimental visual overlays (explicit partial-parity callout); Antigravity CLI is a Go rewrite, effectively closed (no LICENSE file at capture, prebuilt binaries — https://github.com/google-antigravity/antigravity-cli); companion Antigravity Python SDK is Apache-2.0. Gemini CLI OSS stays Apache-2.0 "no changes" for enterprise. Community criticism: agy shipped immature (barebones README, curl|bash installer, auth breakage issue #415) — https://thegitreporter.com/articles/2026-06-23-daily-google-antigravity-after-the-transition/ ; governance moved both ways post-launch (1.0.13 stricter Always-Approve matching; 1.0.14 added subagent auto-proceed) — https://frontier.bitter.sh/signals/2026-07-01-gemini-antigravity-tiered-succession/

## 6. Community fork ecosystem (who inherits)

- IndenScale/open-gemini-cli — canonical "Open Gemini CLI": OpenAI-compatible endpoint injection (Ollama/vLLM/LlamaEdge/Azure/Groq/Together), npm @indenscale/open-gemini-cli, Apache-2.0. https://github.com/IndenScale/open-gemini-cli (UNVERIFIED current health; mirror shows 2025-09 origin)
- limkcreply/open-gemini-cli ("kaidex") — MLX/llama.cpp/vLLM + OpenAI/Anthropic, JSON provider config, inherits 320 upstream contributors, last push 2026-04-05. https://github.com/limkcreply/open-gemini-cli
- joeloliver/positron-code — Ollama fork, still active post-transition (push 2026-07-21). https://github.com/joeloliver/positron-code
- zhangshushu15/omni-cli — multi-provider (OpenAI/Anthropic/DeepSeek/OpenRouter/Ollama/vLLM). https://github.com/zhangshushu15/omni-cli
- Smaller: marizmelo/llm-cli, MichaelLeib/gemini-cli-ollama. Qwen Code lineage already covered in section 18 (other agent's slice).

## 7. FULL MATRIX ROW — Gemini CLI (LEGEND: full / partial / none)

| Dimension | Score | Evidence |
|---|---|---|
| language | full | TypeScript/Node >=20, Ink/React 19 TUI; 7 packages (cli/core/sdk/a2a-server/devtools/vscode-ide-companion/test-utils) |
| RSS | partial | UNVERIFIED — no published figures; Node+Ink single process; not measured this iteration |
| OS sandbox | full | 5 backends (Seatbelt 6 profiles / Docker-Podman / gVisor / LXC / Windows-native) + bwrap tool-sandboxing + expansion requests; core/src/sandbox/ tree; https://geminicli.com/docs/cli/sandbox/ |
| permission tiers | full | policy engine + approval modes + trusted folders + read-only plan mode + confirmation-bus; https://geminicli.com/docs/reference/policy-engine/ |
| prefix-cache | full | implicit+explicit token caching; cachedContentTokenCount in agent/event-translator.ts; https://geminicli.com/docs/cli/token-caching/ |
| AST repomap | none | no repo-map; tree-sitter only parses shell commands (core/src/utils/shell-utils.ts); uses grep/@-references/jit-context instead |
| windowed file view | full | read-file offset/limit windowing + omissionPlaceholderDetector anti-elision guard |
| PTY | full | @lydell/node-pty (darwin/linux/win32, x64+arm64) in deps + background shells (tools/shellBackgroundTools.ts) |
| tiered eviction | full | staged: toolOutputMasking -> toolDistillation -> chat/context compression -> truncation; core/src/context/ (contextManager incremental/hotstart/barrier variants) |
| subagents | full | local profiles + 4 built-ins + agent-scheduler + acknowledgedAgents + remote A2A subagents; https://geminicli.com/docs/core/subagents/ + /docs/core/remote-agents/ |
| multi-model | full | modelRouterService + fallback handler + model steering + per-agent model; Gemini-family only (no third-party providers natively); https://geminicli.com/docs/cli/model-routing/ |
| model profiles | partial | per-agent + generation-settings overrides (https://geminicli.com/docs/cli/generation-settings/); no user-facing named profile feature (internal context/profiles.ts only) |
| skills | full | Agent Skills: SKILL.md loader/manager, activate-skill tool, consent flow, extension marketplace; https://geminicli.com/docs/cli/skills/ |
| auto-learning | partial | experimental Auto Memory + skill-extraction-agent + save_memory; https://geminicli.com/docs/cli/auto-memory/ |
| MCP client | full | OAuth (PKCE, Google, SA impersonation), resources, filtering, consent; core/src/mcp/ |
| MCP server | partial | not an MCP server; server surfaces = A2A (a2a-server) + ACP + vscode-ide-companion MCP (open-file/diagnostics context) |
| WASM | none | no WASM runtime (only incidental web-tree-sitter grammar WASM for shell parsing) |
| sessions | full | resume, session summaries, chat recording, Rewind/replay; https://geminicli.com/docs/cli/rewind/ |
| encryption | full | OS keychain + AES-256-GCM FileKeychain fallback (core/src/services/fileKeychain.ts; hybrid-token-storage.ts) |
| checkpoints | full | shadow-git commit per tool call + /restore (core/src/utils/checkpointUtils.ts commitHash via GitService); https://geminicli.com/docs/cli/checkpointing/ |
| cost HUD | partial | /stats model quota snapshot + token footer + exit summary; no $ cost HUD; https://geminicli.com/docs/resources/quota-and-pricing/ |
| frontends | full | TUI, headless -p JSON, ACP, VS Code companion, Code Assist IDE/GitHub surfaces, SEA single-binary launcher (sea/), voice input |
| multi-client | full | a2a-server (http/app.ts) + SDK (GeminiCliAgent) + ACP |
| telemetry | full | OpenTelemetry metrics/logs/traces (core/src/telemetry/; https://geminicli.com/docs/cli/telemetry/) + enterprise GCA telemetry (code_assist/telemetry.ts) |
| evals | full | evals/ + memory-tests/ + perf-tests/ + integration-tests + behavioral evals framework; https://geminicli.com/docs/behavioral-evals/ |

## Verdict

Deepest OSS sandbox+policy stack in the harness cohort, feature-rich loop (skills/hooks/subagents/A2A/SDK all shipped), but strategically a zombie: consumer auth dead 2026-06-18, development narrowed to enterprise security maintenance, successor is closed-source Go; community energy migrating to open-gemini-cli-style forks and Antigravity CLI. Matrix row scored at FINAL current state (post-transition enterprise build).

Key permalink base: https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/
