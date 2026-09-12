# Iteration 20 — Agent A: Architecture — continuedev/continue (FROZEN)

**Repo**: continuedev/continue (Apache 2.0, 35.9k stars, acquired by Cursor June 2026 — repo read-only, CLOSED-CHAPTER autopsy)
**Frozen state examined**: default branch `main`, HEAD `5522c6f44ca0ac3528b37244818fbfa39b5af470` — final commit: `docs: remove Sign in link (login flow retired) (#13005)` (the login/hub flow was being retired as the repo froze — a literal closed-chapter artifact)
**Clone**: shallow depth-1 at `C:\Users\vsbb1\AppData\Local\Temp\opencode\continue`
**Version discrepancy (VERIFY flag)**: task brief says final v2.0.0/v2.1.0 (June 19 2026). The frozen default branch shows `extensions/vscode/package.json` = **1.3.40**, `core/package.json` = **1.1.0**, `extensions/cli/package.json` = **0.0.0-dev** (CLI used semantic-release; version stamped at publish, not in-tree). Tags were not fetched at depth 1. UNVERIFIED that a v2.x tag maps to this tree; all findings below are from the actual frozen HEAD.

**Top-level layout**: `core/` (shared TS brain, 619 TS/TSX files) · `extensions/vscode` (72 TS files) · `extensions/intellij` (86 Kotlin files) · `extensions/cli` (`cn`) · `gui/` (shared React webview) · `binary/` (core→native binary for JetBrains) · `packages/` (config-yaml, config-types, openai-adapters, llm-info, fetch, terminal-security, continue-sdk) · `docs/`, `docs-site/`, `eval/` (gutted), `skills/`, `actions/` (GitHub Action), `sync/`, `manual-testing-sandbox/`

---

## 1. Extension architecture — VS Code + JetBrains core-sharing

**Shared-vs-duplicated verdict: high sharing via one TS core + one React GUI; duplication confined to shell glue (72 VS Code TS files + 86 JetBrains Kotlin files).** The "JetBrains-secondary problem" was solved architecturally (same core) but organizationally re-lost: the frozen README demotes the plugin.

- **The shared core is real**: `extensions/vscode/src/VsCodeIde.ts` and `src/commands.ts` import directly from `core` (`import { ConfigHandler } from "core/config/ConfigHandler"`; `import { CompletionProvider } from "core/autocomplete/CompletionProvider"`). VS Code runs core in-process as TS. VERIFY: `extensions/vscode/src/commands.ts:4-7`, `extensions/vscode/src/VsCodeIde.ts:4-36`.
- **JetBrains rides the same core through a native binary**: `binary/package.json` bundles `core` (`"core": "file:../core"`) with esbuild + `pkg` into per-OS binaries (pkgJson/{darwin,linux,win}-{x64,arm64}). `extensions/intellij/src/main/kotlin/.../continue/process/ContinueBinaryProcess.kt` spawns it via `ProcessBuilder` and talks stdin/stdout (with `xattr -dr com.apple.quarantine` on macOS, chmod on Linux — runtime permission friction, line 54-79: "todo: consider setting permissions ahead-of-time during build/packaging, not at runtime"). IPC messenger: `binary/src/IpcMessenger.ts`, `IpcIde.ts`, `TcpMessenger.ts`. IntelliJ README confirms: "Communication: stdin/stdout message passing with core binary from ../../binary; UI: Embeds React webview from ../../gui" (`extensions/intellij/README.md`).
- **Shared GUI**: `gui/` (React+Redux) is embedded as a webview by both IDEs (`gui/src/redux/...` is the mode/state engine both consume).
- **JetBrains demoted at freeze**: `extensions/intellij/README.md`: *"The JetBrains plugin is now community-maintained. We strongly recommend using the Continue CLI instead… without depending on a plugin framework we can't give the attention it deserves."* — the historical JetBrains-secondary problem is codified in the final state: architecture shared, maintenance not.
- Also in-repo: `extensions/cli` (below). All three frontends share `packages/*` and `core/`.

## 2. Config-as-code system (config.yaml)

- **Schema (Zod)**: `packages/config-yaml/src/schemas/index.ts` — `configYamlSchema` (L113-153): `name, version, schema, models[], context[], data[], mcpServers[], rules[], prompts[], docs[]`. **Blocks = `uses`/`with`/`override` composition** (L71-84 `blockItemWrapperSchema`; `uses` slugs pull from registry, `override` patches fields); `blockSchema` (L194) requires exactly ONE list entry of exactly one item (a package = one block).
- **Rules**: `ruleSchema` (L39-49): string or `{name, rule, globs, regex, alwaysApply, invokable}`; markdown rules via `packages/config-yaml/src/markdown/` (`markdownToRule.ts`, `getRuleType.ts`, `agentFiles.ts` — AGENTS.md-style rule ingestion).
- **Model schema**: `packages/config-yaml/src/schemas/models.ts` — `modelSchema` (L201): `provider, model, apiKey, apiBase, roles[]` (L23-32: chat/autocomplete/embed/rerank/edit/apply/summarize/**subagent**), `capabilities[]` (tool_use/image_input/next_edit), `defaultCompletionOptions` incl. **`promptCaching`** (L60), `cacheBehavior {cacheSystemMessage, cacheConversation}` (L78-82), per-mode system messages `chatOptions {baseSystemMessage, baseAgentSystemMessage, basePlanSystemMessage, toolOverrides}` (L117-124), 18 prompt templates incl. llama3/chatml/codestral (L126-145).
- **Context providers API**: 35 files in `core/context/providers/` (Codebase, Docs, Git, GitHubIssues, GitLabMR, Greptile, Postgres, Database, HTTP, MCP-as-context, RepoMap, Search, Terminal, OS, Problems, OpenFiles, CustomContextProvider for user JS providers, plus `context_provider_server.py`). `contextSchema = {name, provider, params}` (schemas/index.ts L11-15).
- **Loader**: `packages/config-yaml/src/load/` — `unroll.ts` (block unrolling), `merge.ts`, `blockDuplicationDetector.ts`, `injectBlocks.ts`; loaded by `core/config/ConfigHandler.ts` from `~/.continue/config.yaml` plus `assistants/agents/configs` definition dirs (L6-12).
- **MCP in config**: `packages/config-yaml/src/schemas/mcp/index.ts` — stdio (`command/args/env/cwd`) or sse/streamable-http (`url, apiKey, requestOptions`), with `connectionTimeout`.
- **Secrets resolution**: `packages/config-yaml/src/interfaces/SecretResult.ts` — SecretType = user/package/organization/models_add_on/**local_env/process_env** (FQSN-based `uses secret` resolution). But `apiKey: z.string().optional()` remains a first-class plaintext field — see Friction.

## 3. Four-mode mechanics + MCP wiring

**GUI/IDE modes** (`gui/src/redux/selectors/selectActiveTools.ts`, read in full):
- `chat` → **zero tools** (L15-16);
- `plan` → all enabled tools **filtered to readonly built-ins** (L27-31) + separate `basePlanSystemMessage` (models.ts L120);
- `agent` → everything enabled, MCP included;
- `edit` → separate pipeline (`core/edit/` incl. `core/edit/lazy/`, vertical diff accept/reject commands in `extensions/vscode/package.json`: `continue.acceptVerticalDiffBlock` etc.), plus `core/quickEdit`, `core/nextEdit` (web-tree-sitter document-history next-edit prediction).
- Autocomplete: `core/autocomplete/` full FIM pipeline (classification/context/filtering/generation/postprocessing/prefiltering/snippets/templating) + `prefixPercentage/maxSuffixPercentage/debounceDelay` options; VS Code commands `continue.toggleTabAutocompleteEnabled`, `continue.forceAutocomplete`.
- **MCP is client-wired into agent/plan (NOT chat/autocomplete)**: tools enter via `core/tools/` + `core/context/mcp/MCPManagerSingleton.ts` (@modelcontextprotocol/sdk Client) with OAuth (`core/context/mcp/MCPOauth.ts`); GUI selectActiveTools admits MCP tools in agent mode and (with wildcard) in CLI plan mode, excludes all tools in chat. VERIFY note: the brief's "MCP agent-mode-only" is accurate for the GUI chat exclusion; CLI plan-mode policies explicitly "Allow MCP tools" (`extensions/cli/src/permissions/defaultPolicies.ts`, PLAN_MODE_POLICIES).

**CLI modes** (`extensions/cli/src/services/ToolPermissionService.ts`): Shift+Tab cycles **normal/plan/auto** (slashCommands.ts help text: "Shift+Tab Cycle permission modes (normal/plan/auto)"); plan/auto are "ONLY mode policies (absolute override)" (L215-220); `PLAN_MODE_POLICIES` excludes Edit/MultiEdit/Write but allows Bash-read + MCP wildcard; `AUTO_MODE_POLICIES = [{tool:"*", permission:"allow"}]` (`defaultPolicies.ts`).

## 4. Continue CLI `cn` — the late Continuous-AI pivot

- `extensions/cli/package.json`: bin `cn` → `dist/cn.js`; **Ink v6** TUI (`"ink": "^6.1.0"`); depends on `@continuedev/{config-yaml,openai-adapters,terminal-security,sdk}` + `core`.
- Commands (`extensions/cli/src/index.ts`): default = interactive agent TUI; `ls` (resume sessions); **`serve [prompt]` (hidden)** = Express HTTP headless agent on :8000 — endpoints `/state /message /permission /pause /diff /exit` (`commands/serve.ts` L110+) — the CI/remote execution surface; **`checks [action] [pr-url]`** = AI Checks: `commands/checks.ts` polls Continue backend `CheckStatus {name,state,sessionId,suggestionStatus}` and **auto-detects PR via GitHub API** (`detectPrUrl`, `api.github.com/repos/{o}/{r}/pulls?head={o}:{branch}`); **`review`** = PR review inbox: `commands/review.ts` + `commands/review/` (`worktree.ts` creates git worktrees per review, `reviewWorker.ts`, `renderReport.ts`, `resolveReviews.ts`).
- Repo headline at freeze: JetBrains README banner "**Source-controlled AI checks, enforceable in CI**" (docs.continue.dev) — the pivot from IDE-assistant to Continuous-AI is the frozen brand line. `actions/action.yml` + `actions/buildPrompt.js|writeMarkdown.js` = GitHub Action integration; `extensions/cli/src/services/BackgroundJobService.ts`, `GitAiIntegrationService.ts`.
- **Hub removed at freeze**: `session.ts`: "Remote sessions are no longer available (Hub integration removed)"; `mcp-flag.integration.test.ts`: "should reject all hub loading since hub has been removed"; final commit retires login flow. The CLI + config.yaml + git-native checks are what remains.

## 5. Local-model support

- **79 files in `core/llm/llms/`** — local servers first-class: `Ollama.ts` (+test), `LMStudio.ts`, `LlamaCpp.ts`, `Llamafile.ts`, `TextGenWebUI.ts`, `Vllm.ts`, `Deepseek`, `Together`, `Groq`, plus clouds (Anthropic, Bedrock, VertexAI, SageMaker, WatsonX, Azure, xAI, zAI, Moonshot, MiniMax, SiliconFlow, Nvidia, OpenRouter…). `packages/openai-adapters/src/apis/` = ~30 streaming adapters (`AnthropicCachingStrategies.ts`, `Bedrock.ts`, `Gemini.ts`, `VertexAI.ts`, `openaiResponses.ts`, `AiSdk.ts` Vercel-AI-SDK bridge) — the "30+ providers" claim VERIFYs.
- Docs WSL support friction: `docs/troubleshooting.mdx` "Ollama not working in WSL" — requires `.wslconfig`, firewall rules, `netsh interface portproxy` to reach Windows-host Ollama. 18 prompt templates for local model chat formats (models.ts L126-145); `keepAlive` option (Ollama); `template: none/chatml/llama3/...` overrides.
- Embeddings local: `TransformersJsEmbeddingsProvider.ts` + vendored `@xenova/transformers` (`core/vendor/`), bundled `extensions/vscode/models/all-MiniLM-L6-v2/` (offline embedding model shipped in the extension).

## 6. Known frozen friction

- **Plaintext API keys (VERIFY)**: `apiKey: z.string().optional()` sits directly in `modelSchema` (`packages/config-yaml/src/schemas/models.ts:181`) and `configSchema` has literal `api_key` (schemas/index.ts L261); MCP `apiKey` too (mcp/index.ts L25). No encryption anywhere local: repo-wide grep for `encrypt|cipher|keytar` hits only OpenAI reasoning-item `encrypted_content` passthrough (`core/llm/llms/OpenAI.ts`, `openaiTypeConverters.ts`). Secrets *resolution* exists (SecretResult/env) but the default path is plaintext YAML on disk.
- **Editor lag on large files (UNVERIFIED from repo)** — not documented in `docs/troubleshooting.mdx` (which covers logs, prereleases, keyboard shortcuts, MCP "spawn ENAMETOOLONG" on macOS, SSL/CA). Plausible from architecture (IPC round-trips through the binary for JetBrains; tree-sitter parse + LanceDB indexing in core) but no in-repo admission. Flagged, not claimed.
- **Flaky agent terminal in WSL (PARTIAL VERIFY)**: docs document WSL networking fragility for local models (firewall + portproxy dance); `tools/runTerminalCommand.ts` uses plain `child_process.spawn` with PowerShell on Windows (`-NoLogo -ExecutionPolicy Bypass -Command`) — no PTY, so shell/interop edge cases under WSL interop are expected. The specific "flaky agent terminal in WSL" complaint is UNVERIFIED in-repo.
- **node-pty problems (VERIFY)**: `extensions/vscode/src/terminal/terminalEmulator.ts` line 2: *"Terminal emulator - commented because node-pty is causing problems."*
- **Runtime binary permission fixes**: macOS quarantine + Linux chmod at every IntelliJ launch (ContinueBinaryProcess.kt, with its own TODO admitting it belongs in packaging).
- **JetBrains formally demoted**: community-maintained banner (README).
- **Eval infrastructure gutted**: `eval/` contains only `.gitignore`; `manual-testing-sandbox/` is the leftover.
- **Hub/login removed**: final commits + "Hub integration removed" comments — remote-agent/secret features now point at removed infrastructure (e.g., MCPService error text still references Hub org secrets).

---

## FULL MATRIX ROW — Continue @ freeze (● full / ◐ partial / ○ absent)

| Capability | Score | Evidence (frozen HEAD 5522c6f) |
|---|---|---|
| language | ● | TS core/CLI/gui/packages (619+72 files) + Kotlin JetBrains (86 files); Python only for one context-provider server (`context_provider_server.py`) |
| RSS (retrieval/search stack — interpretation flagged) | ● | `core/indexing/`: LanceDB vector index (`LanceDbIndex.ts`), SQLite FTS (`FullTextSearchIndex`), `CodeSnippetsIndex`, chunking (`chunk/code.ts` via tree-sitter); 35 context providers. (If "RSS" meant feeds/other, ○ — not present) |
| OS sandbox | ○ | No OS-level sandboxing; `packages/terminal-security` is shell-quote **command parsing/policy analysis** (1241-line `evaluateTerminalCommandSecurity.ts`); file-access guard is `.continueignore` (`core/indexing/ignore.ts`) |
| permission tiers | ● | CLI: `permissions/` (permissions.yaml, precedenceResolver, policyWriter, agent-file policies, headless vs TUI) + modes normal/plan/auto (`defaultPolicies.ts`: Edit/MultiEdit/Write=ask; Bash ask→allow in headless; plan=exclude writes; auto=allow all) + `terminal-security` allowedWith/WithoutPermission; GUI: per-tool policies + readonly filtering (selectActiveTools.ts) |
| prefix-cache | ● | Opt-in `promptCaching` completion option → `cache_control:{type:"ephemeral"}` (Anthropic.ts L440, VertexAI.ts L266), Bedrock cache points + `PromptCachingMetrics` (Bedrock.ts L50); `cacheBehavior{cacheSystemMessage,cacheConversation}`; GUI tracks totalCachedTokens/cacheWriteTokens (useTotalUsage.ts) |
| AST repomap | ◐ | `RepoMapContextProvider` + `core/util/generateRepoMap.ts` = **LLM-generated** repo map (Aider-style outline, not AST tags); AST (web-tree-sitter) is used for autocomplete static context & code chunking, not the map |
| windowed file view | ◐ | CLI `Read` tool = whole file with truncation caps (100k chars/5k lines, env-overridable) — **no offset/limit window params**; line-windowed content only via Search tool snippets |
| PTY | ○ | `runTerminalCommand.ts` = `child_process.spawn`; node-pty only in VS Code debug-terminal emulator and marked problematic (terminalEmulator.ts L2); no core/CLI PTY |
| tiered eviction | ◐ | Single-tier auto-compaction: `compaction.ts` — summarize when context ≥ 80% (BUFFER_RATIO 0.8, cap 15k tokens), prune-oldest for compaction input; plus tool-output truncation caps. No multi-tier memory hierarchy |
| subagents | ● | `extensions/cli/src/subagent/` (executor, get-agents) + `tools/subagent.ts` (`SUBAGENT_TOOL_META`); dedicated **`subagent` model role** in schema (models.ts L31) = per-agent model routing |
| multi-model | ● | 79 provider files (core/llm/llms) + ~30 streaming adapters (openai-adapters/apis); role-based routing (8 roles); per-mode models (`selectedModelsByProfileId` globalContext, core/config/selectedModels.ts) |
| model profiles | ◐ | `core/config/profile/LocalProfileLoader.ts` — only the **local** profile ships (ConfigHandler.vitest expects id "local"); hub assistants/profiles removed at freeze; role-level model selection survives |
| skills | ● | SKILL.md format with name/description frontmatter; dirs `<cwd>/.continue/skills`, **`<cwd>/.claude/skills`** (Claude-compat), `~/.continue/skills` (loadMarkdownSkills.ts L95-99); `Skills` tool for on-demand read + auto slash-commands (`skill-<name>`); bundled `skills/cn-check` |
| auto-learning | ○ | No cross-session learning/memory subsystem; nearest analogs: `experimental_includeRecentlyEditedRanges/VisitedRanges` autocomplete context, nextEdit `DocumentHistoryTracker` — session-scoped only |
| MCP client | ● | core: `MCPManagerSingleton.ts` + OAuth (`MCPOauth.ts`, SDK ^1.25/1.29); CLI: `MCPService.ts` with stdio/SSE/streamable-http transports; MCP-as-context (`MCPContextProvider.ts`); config schema `mcpServers` |
| MCP server | ○ | Client-only; no McpServer/exposure anywhere. `cn serve` exposes a **custom** Express API (/state,/message,/permission,/pause,/diff,/exit) — not MCP, not OpenAI-compat |
| WASM | ● | web-tree-sitter across core (`core/util/treeSitter.ts`, autocomplete static-context, `indexing/chunk/code.ts`, nextEdit); WASM grammars shipped as binary assets (`binary/package.json`: `out/tree-sitter.wasm`, `tree-sitter-wasms/*`) |
| sessions | ● | CLI: `session.ts` — `~/.continue/sessions/*.json` + `sessions.json` index, cost/usage snapshots, resume via `cn ls`; IDE: `continue.newSession/shareSession/viewHistory` commands; remote sessions removed (comment in session.ts) |
| encryption | ○ | No local secret encryption: plaintext `apiKey`/`api_key` in config YAML schemas; zero cipher/keytar hits (only OpenAI `encrypted_content` passthrough); hub secret resolution points at removed infra |
| checkpoints | ◐ | **Delegated to external `git-ai`**: `GitAiIntegrationService.ts` shells `git-ai checkpoint continue-cli --hook-input stdin` + pre-compaction hook; no native snapshot/undo system in CLI (IDE only has diff accept/reject) |
| cost HUD | ● | GUI: `gui/src/components/console/TotalUsage.tsx` + `hooks/useTotalUsage.ts` (prompt/generated/thinking/cached/cache-write tokens, totalCost, CostBreakdown via `core/llm/utils/calculateRequestCost`); CLI: per-session cost in session files + OTel costCounter |
| frontends | ● | VS Code ext (1.3.40), IntelliJ ext (Kotlin+binary IPC), `cn` Ink v6 TUI, shared React `gui/` webview, VS Code debug terminal; JetBrains demoted to community at freeze |
| multi-client | ◐ | Three clients share one core/GUI/packages (true multi-frontend); but **no live shared session state across clients** (CLI JSON sessions vs IDE storage), and hub sync removed — cross-client continuity lost at freeze |
| telemetry | ● | CLI: full OpenTelemetry metrics service (`telemetry/telemetryService.ts`: OTLP HTTP exporter; counters for sessions, linesOfCode, PRs, commits, cost, tokens, codeEditDecisions, activeTime, mcpConnections, startupTime; GitHub-Actions/remote-agent detection) |
| evals | ◐ | `eval/` emptied (only .gitignore); remaining: `llmTestHarness.ts` in llms, vitest suites everywhere (incl. OpenAI-compatible harness), `manual-testing-sandbox/`, gui chat-tests — regression tests yes, standalone eval harness gone |
| — (row complete: 25/25 scored) | | |

**Overall read**: The pioneer of local-first, config-as-code, multi-IDE AI assistance froze as a consolidation casualty: hub/login stripped mid-freeze (final commit retires login), JetBrains handed to community, eval infra emptied, and the surviving center of gravity is the `cn` CLI + `cn checks/review` — the Continuous-AI pivot — running on an unusually complete permission/config/MCP/local-model substrate.

