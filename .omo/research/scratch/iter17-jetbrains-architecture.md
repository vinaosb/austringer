# Iteration 17 — JetBrains AI Agents: Architecture Deep-Dive (Agent A)

**Date:** 2026-09-11 · **Scope:** Junie (IDE agent), AI Assistant 2026.2, Junie CLI, Junie Local, Air, ACP, MCP
**Scout facts status:** All headline scout facts re-verified against primary JetBrains sources unless flagged UNVERIFIED.
**Method:** Primary sources = jetbrains.com help docs (2026.2 line), blog.jetbrains.com posts (Mar–Sep 2026), junie.jetbrains.com docs. Cross-checked with third-party coverage (devops.com, adtmag, thenewstack, daily.dev).

---

## 1. Junie agent architecture (VERIFIED)

### Plan mode = committed artifacts in `.junie/plans`
- GA post (Jun 2026): plan is "a first-class artifact" — a structured document with tabs for **product requirements, technical design, delivery stages**, and optional testing strategy. "It lives in `.junie/plans`. You can commit it, and it becomes living task documentation."
- Flow: agent asks clarifying **multiple-choice + freeform questions** when ambiguous → writes plan doc → user edits in editor → hits **Confirm** → implementation begins. "Plan on a strong model; implement on a cheap one" is an explicit cost lever.
- Shortcuts: **Shift+Tab** enters Plan mode; **Ctrl+P** opens the plan.
- Source: https://blog.jetbrains.com/junie/2026/06/junie-coding-agent-out-of-beta/ (VERIFIED)

### Agentic debugging — how the agent drives the IDE debugger (VERIFIED, mechanics documented)
Two layers of evidence:
1. **GA post (capability):** Junie can "start or join a debug session… set breakpoints anywhere that matters, including project code, library code, SDK code — even **decompiled .class files and sources inside JARs**"; inspect stack frames, thread state, expression evaluation, run-to-line. Three usage patterns: fully autonomous ("debug why this test fails only on the second iteration"), co-op ("prepare the debugger, I'll trigger the UI flow"), and hand-off ("continue my session and tell me why this value becomes null").
2. **2026.2 docs (mechanics):** Junie **Debug mode** (IntelliJ IDEA Ultimate) runs against a live debugger session through a **specialized MCP toolset**, not bespoke plumbing. Setup requires the bundled **MCP Server + Debugger MCP Toolset** plugins (IDE ≥ 2026.1.1), and Junie connects to the **IDE's own MCP server at `http://127.0.0.1:64342/stream`** (`"mcpServers": {"idea-mcp": {"type": "streamable-http", ...}}`). Tool groups: session control (launch/attach/resume/step over-into-out), breakpoint management (line + exception, list/remove), state inspection (variables, call stack, thread/frame switch), expression evaluation in paused frame. In Debug mode Junie does **not** modify source unless explicitly asked.
- Sources: https://www.jetbrains.com/help/ai-assistant/junie-agent.html (VERIFIED); GA post above (VERIFIED)

### Remote control / async (VERIFIED)
- "Start a task from your laptop. Check progress from your phone… Review the PR over coffee. Junie runs **asynchronously** and keeps the session available from anywhere you sign in." Target use cases: Spring Boot upgrades, record migrations, legacy test-coverage sweeps.
- Source: GA post (VERIFIED)

### Tool set / semantic index / build & test integration (VERIFIED)
- "Your IDE, the agent's toolbox. Junie uses your IDE's **semantic index, build configurations, test runners, and debugger**, not its own approximation of them." Plus **database integration**: Junie queries databases configured in the IDE via DataGrip/Database plugin; "writes, fixes, and validates SQL in the same session that handles your code."
- Agent loop (VERIFIED, from the Qwen deep-dive): user task → LLM → tool calls (Bash commands, read/write files, etc.) → results back; cloud path re-requests file contents freely; local path keeps a **rolling context** so KV-cache survives across tasks. Progress updates are an XML-ish block the model emits (model-specific adaptation: Qwen3.6's plain narration is used instead). Helper/internal LLM calls: context summarization, task classification/routing, memory extraction, capability filtering (docs: "routing may use GPT-4.1 by default").
- Sources: https://blog.jetbrains.com/junie/2026/08/qwen-for-junie/ (VERIFIED); https://junie.jetbrains.com/docs/junie-cli-model-selection.html (VERIFIED)

### Code review (VERIFIED)
- Three entry points: **GitHub Actions, GitLab (incl. on-prem), or `/review` in CLI or plugin**. Scope: unstaged / staged / diff-vs-main. Interactive walkthrough with inline accept/reject and PR comments; follow-up question reorders the remaining review.
- Source: GA post (VERIFIED). GH Action repo exists: https://github.com/JetBrains/junie-github-action (VERIFIED, repo listing only).

### Benchmarks (VERIFIED)
- SWE-Rebench cycle: **#1 coding agent, 61.6% resolved, 72.7% pass@5** (quote from Alexander Golubev, Nebius, in GA post).
- Source: GA post (VERIFIED).

---

## 2. AI Assistant 2026.2 vs Junie — "one engine" split (VERIFIED)

- **Claim:** "One engine, many surfaces. The same agent is behind the AI chat, the dedicated Junie tool window, and Junie CLI. Improvements ship once and show up everywhere." (GA post)
- **Mechanism:** the IDE↔agent integration was **rebuilt on ACP** at GA — "the same protocol Junie CLI uses to talk to your IDE." Junie appears inside AI Chat as a selectable agent/mode (2026.2 docs: "select Junie in AI Chat"); AI Assistant is the host surface that also hosts **external** agents (Claude Agent, Codex, Gemini CLI, ACP registry agents).
- **Division of labor in 2026.2 docs:** AI Assistant = chat + inline features + agent host + model/provider management + MCP config; Junie = the autonomous plan/execute agent + Debug mode + review; external agents = first-class via ACP. AI Assistant itself also has non-agent AI features (inline prompts, etc.) and its own free tier (third-party coverage, UNVERIFIED pricing details).
- Sources: GA post (VERIFIED); https://www.jetbrains.com/help/ai-assistant/acp.html (VERIFIED); https://www.jetbrains.com/help/ai-assistant/junie-agent.html (VERIFIED)

## 3. Junie CLI beta (VERIFIED)

- Beta announced **2026-03-09**: "the evolution of Junie into a fully standalone agent… terminal, inside any IDE, in CI/CD, and on GitHub or GitLab." https://blog.jetbrains.com/junie/2026/03/junie-cli-the-llm-agnostic-coding-agent-is-now-in-beta/
- **LLM-agnostic:** all top models from OpenAI, Anthropic, Google, Grok (xAI) + latest as released.
- Onboarding/config: **one-click migration** from Claude Code/Codex; guidelines, **custom agents and agent skills**, commands, **MCP** ("install popular MCP servers in a few clicks"; agent can *recommend* an MCP server relevant to the task), BYOK.
- Features: real-time prompting (steer mid-run), **next-task prediction**, context-aware by default.
- **Model selection docs (Sep 2026):** four provider types — **Junie** (JB subscription/API key), **BYOK** (openai, anthropic, google, **xai, openrouter, copilot**), **Custom** (profile JSON; includes **Junie Local**), **Proxy** (custom endpoint in config.json). Env vars `JUNIE_MODEL`, `JUNIE_LLM_PROVIDER`, `JUNIE_EFFORT` (minimal…max). **Internal helper model** per provider handles summarization/routing/memory/filtering (e.g., Claude Haiku if primary is Anthropic). Aliases: `sonnet`→Claude Sonnet 5, `opus`→Claude Opus 4.8, `gpt`→GPT-5.4, `gpt-codex`→GPT-5.3-codex, `gemini-pro`→Gemini 3.1 Pro Preview, `gemini-flash`→Gemini 3 Flash, `grok`→**Grok 4.3**.
- **Local runtimes:** "Junie connects to local model runtimes — point it at **LiteLLM, LMStudio, Ollama**… Prompts and code never shared externally." (GA post). AI Assistant separately supports Ollama + LM Studio for local models (help docs).
- Sources: https://junie.jetbrains.com/docs/junie-cli-model-selection.html (VERIFIED); GA post (VERIFIED); CLI beta post (VERIFIED)

## 4. Junie Local (Aug 2026) — on-Mac inference (VERIFIED)

- **`/local` command** in Junie: downloads model, starts local server, switches agent. **Qwen3.6-27B @ 4-bit, ~20 GB download, requires M5 Mac with 64 GB RAM. Free, no registration/credits; works fully offline** ("once the weights are on disk… airplane"). Existing setup carries over: Plan mode, live prompting, guidelines, skills, /commands.
- **Optimizations (deep-dive):**
  - Agent-harness: **rolling context** (append instead of re-select → KV-cache reuse across tasks in session); **maximized initial reusable prefix** (system prompt + stable context ordered first, cached up to the user request; volatile project context placed after); model-specific progress-update parsing; **optional LLM calls disabled** (task summaries) and **multi-agent mode completely disabled** locally (sequential is optimal on M5).
  - Model: **`reasoning_effort: none`** — reasoning off gives ~2x speedup at insignificant quality cost; Qwen3.8 rejected (needs reasoning → ~4–5x slower, loop-stuck failure mode).
  - Inference (MLX-VLM fork): **int8 prefill patch** → ~40% prefill throughput on M5 Neural Accelerator (8-bit arithmetic M4 lacks; M4 ≈20–30% slower prefill). Patch: https://github.com/JetBrains/mlx-vlm/tree/feature/int8-prefill/research (VERIFIED link from post). vLLM variant via config: https://github.com/erokhins/vllm-qwen3.6-27b-nvfp4/commit/b85e97d316ee16590c65c4739f70ed7eaeab3025. **Speculative decoding = MTP draft model + n-gram matching simultaneously → up to 2x generation speedup.**
- **Quality (JetBrains private set):** on par with Sonnet 4.5 (10k-token reasoning limit); GPT-5 medium slightly higher; local ships with reasoning fully disabled.
- **Roadmap:** prototypes for **DGX Spark and RTX 5090**, evaluating 24 GB cards; prefill optimization shifts for discrete GPU.
- Sources: https://blog.jetbrains.com/junie/2026/08/junie-local-launch/ (VERIFIED); https://blog.jetbrains.com/junie/2026/08/qwen-for-junie/ (VERIFIED); https://junie.jetbrains.com/local (corroborating, VERIFIED via search snippet)

## 5. Air — multi-agent orchestration (VERIFIED, public preview Mar 2026)

- **Positioning:** "Agentic Development Environment… IDEs add tools to the code editor, while Air builds tools around the agent." Free macOS app at preview (Win/Linux added by Aug 2026: Toolbox install, `curl -fsSL https://jb.gg/air-install.sh | sh` for Linux). https://blog.jetbrains.com/air/2026/03/air-launches-as-public-preview-a-new-wave-of-dev-tooling-built-on-26-years-of-experience/ (VERIFIED)
- **Control-plane mechanics (quickstart, Aug 2026):**
  - **Workspace** = per-project container for agent sessions, Git state, tools; one window, multiple workspaces; **Tasks** tool = one task = one agent session, run **in parallel**, notification when a background task needs attention.
  - **Agent providers:** Claude Agent, OpenAI Codex, Gemini CLI, Junie out of the box (via JetBrains AI as universal provider, or direct Anthropic/OpenAI/Google sign-in, or BYOK); **ACP supported** + registry integration planned/landed. Air docs: https://www.jetbrains.com/help/air/quick-start-with-air.html (VERIFIED)
  - **Execution environments:** **Local Workspace / Git Worktree / Docker** (isolation + concurrency); "bring changes from a container or worktree to your main copy" via Apply Locally / Check Out Branch Locally. **Cloud execution (remote sandboxed runs) = tech preview.**
  - **Permission modes** per task incl. **Plan permission mode** (agent drafts plan, approval before code).
  - **Review-with-agent:** separate fresh-agent review task checks diff vs. review focus areas, leaves line comments; accept → fed back as follow-up instructions; "second opinion from a different agent."
  - Context: `@` attach files/folders/Git items/terminal output/**symbols (classes, functions, methods)**; "mention a specific line, commit, class, method" — precise context over pasted blobs (semantic navigation inherited from JetBrains stack).
- Third-party corroboration: devops.com "JetBrains Launches Air and Junie CLI…", adtmag (Mar 19, 2026) (VERIFIED headlines).
- **UNVERIFIED/unclear:** whether Air exposes a public control-plane API (vs. GUI only); "JetBrains Central" enterprise control plane is separate (see §8).

## 6. ACP implementation depth (VERIFIED)

- **What:** Agent Client Protocol — "standard communication interface, so any agent that implements the protocol can be added without requiring a custom integration." JetBrains co-launched the **ACP Agent Registry with Zed** (Jan 2026): https://blog.jetbrains.com/ai/2026/01/acp-agent-registry/ (VERIFIED, date from URL; content via search snippets — registry integrated directly into JetBrains IDEs), https://zed.dev/blog/acp-registry (VERIFIED existence), https://www.jetbrains.com/acp/ (VERIFIED landing).
- **Where:** AI Assistant 2026.2 hosts ACP agents **in AI Chat** (agent-mode selector → "Install From ACP Registry"). Install flow: IDE downloads agent files, **downloads/manages Node.js or Python runtimes** (reuses existing compatible runtime), auth flow on first use, update/uninstall from Settings | Tools | AI Assistant | Agents. Custom agents via **`~/.jetbrains/acp.json`** (`agent_servers` with command/args/env; IDE auto-detects locally installed ACP agents). Config example in docs includes opencode, goose, auggie. **Limitation: no WSL support currently.** ACP usable **without** a JetBrains AI subscription.
- **GA post:** Junie's own IDE integration "rebuilt on top of ACP" — Junie IDE plugin and Junie CLI share the protocol. ReSharper 2026.2 = "first step toward ACP-based agent support in Visual Studio, starting with Junie in Preview" (https://blog.jetbrains.com/dotnet/2026/07/22/resharper-2026-2-release/, VERIFIED headline snippet).
- **External agent hosting:** agents run as **local subprocesses** of the IDE (`command`+`args`); remote/in-house agents are the ACP pitch ("connect local, remote, or in-house AI agents") but JetBrains-side docs only document local spawn — remote hosting depth UNVERIFIED.
- Sources: https://www.jetbrains.com/help/ai-assistant/acp.html (VERIFIED); GA post (VERIFIED)

## 7. MCP — local vs remote status + the external-agent gap (VERIFIED, nuance found)

- **Client (AI Assistant/Junie):** transports **STDIO** (local subprocess), **Streamable HTTP** (local or remote), **SSE** (legacy). JSON `mcpServers` config, Claude-Desktop import, global or project level, per-server status/logs. Docs: https://www.jetbrains.com/help/ai-assistant/mcp.html (VERIFIED)
- **Server (IDE-as-MCP-server):** bundled **MCP Server plugin since IDE 2025.2**; external clients auto-config: **Junie, VS Code, Claude Code, Codex, Air, GitHub Copilot CLI**. Exposed tools (toggleable per tool): `analyze_calls` (call hierarchy incl. **decompiled JAR symbols**), `build_project`, `get_file_problems`, `lint_files`, `get_symbol_info`, full **database toolset** (execute_sql_query etc.), **debugger toolset** (breakpoints/step/evaluate — powers Junie Debug mode). **"Brave mode"** = run shell/run-configs without confirmation. **Router-only tools** hidden from direct list to save context, surfacing via a router tool. Terminal-session setup banners for Codex/Claude.
- **The gap — nuance vs. scout fact:** historically, IDE-configured MCP servers were **not consumable by external ACP agents** (YouTrack **LLM-20452** "Add support for configured MCP servers in Claude Agent" documents the request; issue page is JS-rendered — details UNVERIFIED, existence VERIFIED via search). By **2026.2 the gap is opt-in closed**: ACP agents settings expose **"Pass custom MCP servers"** (expose user-configured MCP to installed agents) and **"Pass IntelliJ MCP server"** (expose the IDE server); `acp.json` `default_mcp_settings` has `use_custom_mcp` (default **true**) and `use_idea_mcp` (default **false**, optional `idea_mcp_allowed_tools` allowlist). Junie itself connects to the IDE MCP server for Debug mode. Remaining friction: local models ("AI Assistant does not support invoking tools from configured MCP servers when using local models" — help snippet, VERIFIED) and the CLI's MCP story is per-run rather than IDE-shared (UNVERIFIED).
- Sources: mcp.html + acp.html + junie-agent.html (all VERIFIED)

## 8. Trust / permissions / governance (VERIFIED)

- **Per-operation approval (Junie default):** every bash command, code change, file op, external-tool use requires approval → **Yes / No / Always allow**; diffs reviewable pre-approve; commands openable "Open in editor" pre-run; **per-file and all-files rollback**.
- **Brave mode** (Junie): skip confirmations; **Auto** = agent decides. **MCP brave mode** (IDE server): external clients run shell/ configurations without confirmation. **Air:** per-task **permission modes** (incl. Plan mode approval gate) + workspace trust dialog (Preview vs Trust).
- **Plan-approval gate:** Plan mode is itself a governance gate — implementation blocked until the `.junie/plans` doc is Confirmed (GA post).
- **Guardrails:** `.aiignore` respected (no processing of listed paths unless explicitly permitted); instructions via **AGENTS.md** (project-root, version-controlled); MCP `idea_mcp_allowed_tools` allowlists.
- **Enterprise (post-Junie-2026.2):** **JetBrains IDE Services / JetBrains Central** admins can **preconfigure MCP servers, block user-added servers, curate/disable ACP registry agents, block custom agents** (both mcp.html and acp.html governance sections). Governance post (Jun 10, 2026): incremental autonomy ("treat agents like new hires"), audit trail requirements (who/what/systems/cost/policy), risk-scored human checkpoints, isolation/blast-radius/rollback, indemnification; "AI audit dashboard" as product direction; JetBrains Central Design Partners program. https://blog.jetbrains.com/ai/2026/06/agentic-ai-governance-designing-for-accountability-and-control/ (VERIFIED)

## 9. Models matrix (VERIFIED against supported-llms.html, 2026.2, updated 2026-08-06)

- **JetBrains AI service (cloud):** Claude **Fable 5 / Opus 5 / Sonnet 5** (the "Claude 5 family", 2026.1.x, 1M ctx) + Claude 4.8/4.7/4.6/4.5 tiers; **GPT 5.6 Luna / Sol / Terra** (2026.1.x, 1M ctx) + GPT-5.5/5.4/5.3-Codex/legacy; Gemini 3.6/3.5 Flash, 3.1 Pro, 3 Flash (Junie default was **Gemini 3.7 Flash** per Aug 2026 blog at 40% off base pricing; **Gemini 3.8 Flash** promo Sep 2026 — blog posts VERIFIED, presence in supported-llms table lags, UNVERIFIED for 3.7/3.8 exact listing); **Grok-4.3** (2026.1.x, 1M). Scout fact CONFIRMED: Claude 5 family + GPT 5.6 Luna/Sol/Terra + Gemini 3.x + Grok-4.3.
- **BYOK:** OpenAI, Anthropic, Google, xAI, OpenRouter, GitHub Copilot (CLI docs). **Local:** Ollama, LM Studio (AI Assistant), LiteLLM proxy (CLI provider list), Junie Local (bundled Qwen3.6-27B).
- **Source:** https://www.jetbrains.com/help/ai-assistant/supported-llms.html (VERIFIED); https://junie.jetbrains.com/docs/junie-cli-model-selection.html (VERIFIED); https://blog.jetbrains.com/junie/2026/08/junie-gemini-3-7-flash/ (VERIFIED headline)

## 10. Evals heritage (VERIFIED)

- **SWE-Rebench** leaderboard entry (61.6% / 72.7% pass@5). **Private evaluation set** gates every model before shipping ("We evaluate every model on JetBrains' own private test set before it goes near Junie"). **Trajectory-eval pipeline** ("From Leaderboards to Model Profiles," Aug 31, 2026): 4 perspectives (outcome, efficiency, patch quality, process quality) over 523 tasks; deterministic metrics (tokens, calls, repeated reads, tool-failure loops) + **LLM judges** on milestones (localization, root cause, implementation, validation); produced behavioral **model profiles** (Opus 4.7 = strong diagnosis/weak completion; Gemini 3.5 Flash = strong validation/weak grounding, 37.3% moderate-severe hallucination rate vs 24.7% Opus; Qwen3.6-27B FP8 = 38.9% resolve at 3% of GPT-5.5 cost). Aliases in CLI are pinned by these internal evaluations.
- Source: https://blog.jetbrains.com/junie/2026/08/from-leaderboards-to-profiles/ (VERIFIED)

---

## FULL MATRIX ROW — JetBrains stack (IDE agent + CLI + Local + Air), as of Sep 2026

Legend: ● strong/first-class with evidence · ◐ partial/present-but-limited · ○ absent/no evidence

| # | Dimension | Score | Evidence / Notes |
|---|-----------|-------|------------------|
| 1 | language | ● | Whole JetBrains IDE catalog (Java/Kotlin/Python/JS/TS/.NET/Go/Rust/PHP/Ruby/C++/SQL…) runs Junie; PSI/semantic index per language. GA post: "available in all JetBrains IDEs." |
| 2 | RSS | ○ | No built-in feed/retrieval capability found in agent context; achievable only via generic MCP servers (mcp.html). |
| 3 | OS sandbox | ◐ | Air: Docker + Git-worktree isolation per task, cloud sandboxes "tech preview" (air quickstart). IDE Junie/CLI run unsandboxed on host; no macOS seatbelt/namespace-style OS sandbox documented. |
| 4 | permission tiers | ● | Default per-op approval (Yes/No/Always-allow) → Brave mode → Auto; Air permission modes + Plan-approval gate; MCP brave mode; `.aiignore`; tool allowlists (`idea_mcp_allowed_tools`); admin-curated agents/MCP via IDE Services/Central. junie-agent.html + acp.html + governance post. |
| 5 | prefix-cache | ● | Explicitly engineered, not incidental: system prompt + stable context **reordered to maximize cached prefix up to user request**; KV-cache reuse across tasks in session (Qwen post); cloud-side caching implied by "prefix" design. Best documented prefix discipline of any surveyed harness. |
| 6 | AST repomap (semantic index) | ● | Deepest heritage in the field: IDE **semantic index**, PSI, `analyze_calls` call-hierarchy MCP tool (in/out, incl. decompiled JAR symbols), `search_symbol`, symbol-level `@` mentions in Air, debugger over decompiled code. Supersedes tree-sitter grep-repomap approach — but only inside JetBrains IDEs (CLI alone lacks it unless pointed at an IDE MCP server). |
| 7 | windowed file view | ◐ | Auto-context = current file + selection only; `@` attach for more; MCP tools page outputs (`childOffset`, `fetch_query_result` offset); local mode sidesteps windowing via rolling context. No explicit windowed file-view paging layer like Claude Code's file-pager documented. |
| 8 | PTY | ◐ | Terminal commands executed and reviewable (approval widget, "Open in editor"); Air ships a built-in terminal; MCP `brave mode` shell exec. No evidence of interactive PTY control (raw REPL/tui steering, session attach) à la tmux-driven agents. |
| 9 | tiered eviction | ◐ | Session task 2 "takes only relevant pieces from the context and puts them in the window" (selection-based compaction); internal helper model "summarizing context"; local mode replaces eviction with append-only rolling context. No documented tiered/priority eviction hierarchy. |
| 10 | subagents | ◐ | **Multi-agent mode exists in cloud Junie** (explicitly "completely disabled" in Local) but its shape (spawnable subagents vs internal parallelism) is undocumented; CLI supports "custom agents" definitions; Air = sibling tasks, not child subagents; helper models ≠ subagents. UNVERIFIED depth of cloud multi-agent mode. |
| 11 | multi-model | ● | 4 provider classes (Junie/BYOK/Custom/Proxy) + auto-detection + per-task model choice + "plan strong, implement cheap" + provider-local helper models + reasoning-effort dial (`minimal…max`) + `Default` price-quality router. |
| 12 | model profiles | ● | Literal product feature: research-driven behavioral profiles; per-alias model pinning by internal evals; effort levels per model; Gemini 3.7 Flash default-tier pricing. |
| 13 | skills | ● | Agent skills in CLI + IDE ("Skills" settings page, **bundled skills** e.g. `ij-debugger`, installable to external Claude Code/Codex dirs); Air Skills + Commands; guidelines + AGENTS.md. |
| 14 | auto-learning | ◐ | Internal "extracting information for memory" task + next-task prediction + guidelines/AGENTS.md (manual) — but no documented cross-session auto-generated memory (no equivalent of Claude Code auto-memory files) found. UNVERIFIED whether memory extraction persists user-visibly. |
| 15 | MCP client | ● | STDIO + Streamable HTTP + SSE; Claude-Desktop import; one-click popular servers; server-recommendation in CLI; per-tool exposure; router-only mode to save context; Junie consumes IDE MCP server for debugger tools. |
| 16 | MCP server | ● | IDE-as-MCP-server bundled (2025.2+): auto-config for Junie/VS Code/Claude Code/Codex/Air/Copilot CLI; exposed analysis/debugger/database/insight tools; brave mode; router-only tools; localhost streamable-http endpoint. |
| 17 | WASM | ○ | No WASM runtime/plugin story found for agents (IDE plugin ecosystem is JVM/Kotlin-based). |
| 18 | sessions | ● | Async remote sessions resumable from any signed-in device ("check progress from your phone"); Air workspaces bind sessions/Git/tools per project, parallel tasks with attention notifications; `/commands` session management; CLI interactive sessions. |
| 19 | encryption | ◐ | Strong privacy posture documented (Local = code never leaves machine; "no vendor data policy to review"); enterprise controls via Central; **no explicit session-data encryption-at-rest/in-transit spec found** — UNVERIFIED. |
| 20 | checkpoints | ◐ | Per-file & all-files **rollback** after agent edits; Air accept/revert/"roll back to a previous commit"; worktree isolation as coarse checkpointing. No automatic step-level checkpoint timeline documented. |
| 21 | cost HUD (AI Credits) | ◐ | **AI Credits** metering is the billing primitive; per-model credit multipliers (Opus 4.6 "2-3x more credits"); "no credits" for Local; enterprise audit trail captures cost; eval tooling reports USD/run. No documented **live in-IDE per-run cost HUD**. |
| 22 | frontends | ● | Widest surface in survey: IDE plugins (all IDEs + ReSharper/VS preview), AI Chat, Junie tool window, Junie CLI, Air desktop (macOS/Linux/Windows), GitHub/GitLab (incl. on-prem) actions, phone/browser remote control, CI/CD. |
| 23 | multi-client | ● | ACP-first: hosts Claude Agent/Codex/Gemini CLI/opencode/goose/auggie + ACP Registry in IDE; IDE-as-MCP-server serves arbitrary external clients; Air hosts 4 agents + ACP; protocol co-authored with Zed. |
| 24 | telemetry | ◐ | `llm.agent.extended.logging` registry key + Get ACP Logs (request/response capture); MCP server logs per process; internal trajectory telemetry fuels evals; enterprise audit dashboard (direction). No user-facing live trace viewer documented. |
| 25 | evals | ● | Strongest public eval program surveyed: SWE-Rebench #1 (61.6%/72.7%), private gating test set per model, published 4-perspective trajectory pipeline w/ LLM judges (523 tasks), model-profile research, alias pinning by internal evals. |

**Row tally:** ● 13 · ◐ 10 · ○ 2

**Architecture verdict:** JetBrains' differentiator is *inversion of the toolbox* — the agent doesn't reimplement search/build/debug (à la repomap+LSP approximations elsewhere); it drives the IDE's real semantic index, build configs, test runners, debugger (via localhost MCP), and databases. The stack is now protocol-native (ACP for agents, MCP for tools), with an unusual second front: hand-tuned local inference (prefill-first optimization is the correct insight — agent time is dominated by file reads, not generation). Weakest flanks: no OS-level sandboxing outside Air's Docker mode, shallow subagent story, no step-level checkpoints, and the MCP-passing-to-external-agents capability is new/opt-in (defaults still conservative).

## Source register
1. GA post — https://blog.jetbrains.com/junie/2026/06/junie-coding-agent-out-of-beta/
2. Junie Local launch — https://blog.jetbrains.com/junie/2026/08/junie-local-launch/
3. Qwen optimization — https://blog.jetbrains.com/junie/2026/08/qwen-for-junie/
4. Junie CLI beta — https://blog.jetbrains.com/junie/2026/03/junie-cli-the-llm-agnostic-coding-agent-is-now-in-beta/
5. Junie docs: model selection — https://junie.jetbrains.com/docs/junie-cli-model-selection.html
6. Junie docs: agent/permissions/debug — https://www.jetbrains.com/help/ai-assistant/junie-agent.html
7. AI Assistant MCP — https://www.jetbrains.com/help/ai-assistant/mcp.html
8. AI Assistant ACP — https://www.jetbrains.com/help/ai-assistant/acp.html
9. Supported models — https://www.jetbrains.com/help/ai-assistant/supported-llms.html
10. Air preview — https://blog.jetbrains.com/air/2026/03/air-launches-as-public-preview-a-new-wave-of-dev-tooling-built-on-26-years-of-experience/
11. Air quickstart — https://www.jetbrains.com/help/air/quick-start-with-air.html
12. ACP registry (JB) — https://blog.jetbrains.com/ai/2026/01/acp-agent-registry/ · (Zed) https://zed.dev/blog/acp-registry · https://www.jetbrains.com/acp/
13. Governance — https://blog.jetbrains.com/ai/2026/06/agentic-ai-governance-designing-for-accountability-and-control/
14. Model profiles eval — https://blog.jetbrains.com/junie/2026/08/from-leaderboards-to-profiles/
15. ReSharper ACP step — https://blog.jetbrains.com/dotnet/2026/07/22/resharper-2026-2-release/
16. Gemini default — https://blog.jetbrains.com/junie/2026/08/junie-gemini-3-7-flash/ · https://blog.jetbrains.com/junie/2026/09/junie-gemini-3-8-flash/
17. MLX patch — https://github.com/JetBrains/mlx-vlm/tree/feature/int8-prefill/research
18. YouTrack MCP gap — https://youtrack.jetbrains.com/issue/LLM-20452 (existence VERIFIED; body UNVERIFIED, JS-only page)
19. junie-github-action — https://github.com/JetBrains/junie-github-action
