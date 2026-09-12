---
slug: rust-agent-harness
status: drafting
intent: clear
review_required: false
pending-action: write .omo/plans/rust-agent-harness.md
approach: Rust general-purpose agent harness (low-memory, TDD-first, plug-and-play): core library crate + interactive CLI, provider-agnostic LLM layer, MCP client, layered tool safety, skill/prompt packs for multi-domain use. Synthesizes cross-harness lessons from 5 in-flight librarian research tasks.
---

# Draft: rust-agent-harness

> NOTE: this environment exposes no shell tool, so `scaffold-plan.mjs` could not be
> executed; its `--draft-only` output for slug `rust-agent-harness`, intent `clear`,
> review_required `false` was replicated faithfully from the script source
> (<skill-root>/scripts/scaffold-plan.mjs, buildDraft()) and then enriched with state.

## Components (topology ledger)
<!-- Lock the SHAPE before depth. One row per top-level component that can succeed or fail independently. -->
<!-- id | outcome (one line) | status: active|deferred | evidence path -->

| id | outcome | status | evidence |
| --- | --- | --- | --- |
| ws-scaffold | cargo workspace + CI (nextest archive matrix) + cargo-dist + TDD infra + mock-provider test-support crate | active | librarian-5 |
| core | agent loop as named-operation state machine (goose-style) + events + doom-loop/stuck detection | active | librarian-2 |
| context-engine | Binary Prompt Idempotency Manager; 3-tier eviction @75%; compaction; Blake3 dedup folding; single volatile error slot | active | paste §3-4 |
| ast-repomap | tree-sitter tag tables (Blake3+mtime); Personalized PageRank CSR+SIMD; ≤1k-token map; view_window ≤100 lines | active | paste §3 |
| providers | dyn-safe Provider trait + adapters (Anthropic/OpenAI/Google/xAI/OpenAI-compat); streaming; retries; cost accounting; model-identity transparency | active | D2 + librarian-3 |
| tools | 6 invariant tools (view_window, edit_lines, execute_command, lsp_query, tool_search, task_completed) + deferred tool loading | active | paste §3 |
| exec | PTY pipeline (portable-pty+vt100 120×40, first25+last50); ConPTY supervisor (3 pathologies); Job Objects; WSL2 bridge; PS7 default shell | active | paste §6 |
| vfs-shadow | CoW reflink / gix worktree shadows; pre-flight (AST→formatter→compiler, max-2 corrective subcycles); semantic hunk staging (imara-diff + tree-sitter, y/n/e) | active | paste §4 |
| snapshots | refs/harness/snapshots/{session}/{turn} via gix in-memory objects; /rewind (files + history) | active | paste §4 |
| sessions | SQLite (source of truth) + JSONL export; encryption-at-rest (XChaCha20-Poly1305 per-session keys); lazy index-based loading (never full-file parse); fork/resume | active | D8 + librarian-2 |
| safety | permissions allow/ask/deny (+patterns); OS-native sandbox (Seatbelt / Landlock+bwrap / Win restricted tokens+Job Objects); container backend; RepetitionInspector | active | D5 + librarian-2 |
| mcp | rmcp 3.3 client (stdio+Streamable HTTP, roots confinement) + SERVER mode (harness AS MCP server) | active | D3 + librarian-3 |
| skills-packs | agentskills.io-compatible SKILL.md format (triggers, negative scoping, languages, version); loader; pack aggregation (_shared templates, findings schemas) | active | D4 + local inspection |
| domain-packs | shipped base packs: programming (.NET/frontend/backend/devops), legal, maths, accounting, finance/investors + user-domain packs (audit squads, datacenter/AEC, AI deployment) | active | D6 + local inspection |
| registry | community pack registry: spec + reference server (install/search/pin+hash verify, trust review) | active | D6 + librarian-6 |
| lsp | headless LSP client (goto/find_references/hover, <25 tokens) | active | paste §5 |
| search | tantivy BM25 + grep-regex lexical index (NO vectors) | active | paste + librarian-3 |
| tui | ratatui event-driven (0% idle); accordions; context/cache/cost HUD; @file/@symbol/@git; y/a/n gates; /rewind; /mode | active (D1) | paste §7 |
| web-frontend | SPA served BY the daemon (static assets from the Rust binary, no Node server): session browser + live streaming view, inline semantic-hunk diff review (y/n/e), cost/cache/context dashboard, pack & registry manager, eval results, share links | active (D1 amended) | latent-need: opcode 22.4k★, claudecodeui 13.6k★, opencode share links |
| vscode-ext | TypeScript extension = thin daemon client: editor telemetry (active file/cursor/selection/diagnostics as ambient context), inline hunk review via native diff decorations + code lenses, command palette integration, status-bar gauge, bridges IDE's OWN LSP/ripgrep to the harness (no duplicate indexing) | active (D1 amended) | Windsurf telemetry + crush workspace-sharing + opencode/codex IDE ext patterns |
| daemon | axum WS+SSE headless + interprocess IPC (UDS/named pipes); ONE client protocol powering TUI/Web/VSCode; multi-client session attach (crush pattern: shared permission queue); serves web SPA; binds 127.0.0.1 + auth token by default | active (D1) | paste §7 + librarian-2 |
| cli | clap CLI; interactive REPL fallback; headless run; --output-json for external eval harnesses | active | librarian-5 |
| wasm-ext | wasmtime WASI 0.2 plugin runtime (32MB ResourceLimiter, fuel, preopened dirs) — LATE wave; extension trait admits it | active (late) | paste §5 |
| evals | conformance patterns: expected-failures YAML; evals-not-PR-gates policy; harbor-style harness-comparison adapter | active | librarian-5 |

## Open assumptions (announced defaults)
<!-- Record any default you adopt instead of asking, so the user can veto it at the gate. -->
<!-- assumption | adopted default | rationale | reversible? -->

| assumption | adopted default | rationale | reversible? |
| --- | --- | --- | --- |
| async runtime | tokio | industry default; ecosystem alignment (reqwest, rmcp, etc.) | yes |
| MSRV | latest stable Rust toolchain | greenfield one-shot; no legacy constraints | yes |
| config format | layered TOML (defaults < user < project < CLI flags), serde-validated, schema-documented | Rust community convention (cargo-style) | yes |
| test stack | cargo-nextest + insta (snapshots) + assert_cmd (CLI); scripted mock-provider with SSE replay for the agent loop | TDD is a hard user requirement | yes |
| logging/observability | tracing + tracing-subscriber, JSON option | standard, zero-cost when disabled | yes |
| workspace layout | cargo workspace, one crate per bounded context | extensibility requirement | yes |

## Findings (cited - path:lines)

- C:\Users\vsbb1\source\repos\harness\prompts.md — EMPTY (0 lines). The user-supplied
  conversation log did not persist. User answered interview Q1 by PASTING the entire
  conversation into chat (visible portion absorbed below; full text ~60KB+ in
  C:\Users\vsbb1\.local\share\opencode\tool-output\tool_08f76aafb001fn3I5Vznu0V9zh —
  explore agent digesting the truncated tail).
- Repo contains ONLY prompts.md. Greenfield; not a git repo yet.
- Research in flight (5 background librarians):
  - bg_cf3fbae6 — Claude Code / Codex CLI / opencode / aider / Gemini CLI lessons
  - bg_45847c9a — goose / crush / OpenHands / SWE-agent / AutoGPT-era lessons
  - bg_90ff57f9 — Rust LLM/agent ecosystem inventory (build vs reuse)
  - bg_c6b6851d — extensibility + multi-domain patterns
  - bg_6dce9fb5 — TDD strategies for agent harnesses

### User-pasted conversation: requirements extracted (visible portion)

**Inspiration + techniques to steal (per harness):**
- Aider: tree-sitter repo map (whole-workspace mental model in 500–1000 tokens);
  search/replace blocks + unified diffs w/ fuzzy fallback; Architect/Editor
  dual-model pipelining (reasoning model plans, cheap model edits); git micro-commits.
- SWE-agent: ACI discipline — windowed file view (e.g. 100 lines), linewise
  navigation, scroll/jump-to-match, NO full-file dumps.
- Claude Code: ephemeral fork-and-join subagents (isolated context burns, compact
  synthesized answers return); stdout/stderr pruning (first 20 + last 20 lines,
  middle elided); hooks pipeline.
- Zed Assistant: Rust-native async streaming, zero-copy context providers.
- Cursor: Shadow Workspace — headless mirror of repo (CoW reflinks / git worktrees
  via gix), run checks in shadow BEFORE touching user tree, feed diagnostics back.
- Windsurf Cascade: ambient telemetry bus (cursor pos, recent files, clipboard) →
  zero-prompt context resolution (notify crate + IPC socket).
- Roo/Cline: mode-scoped tool masking; checkpoint rewind trees.
- Mentat: interactive per-hunk staging w/ precise rejection feedback.
- difftastic: syntax-aware (AST-node) diffing to avoid false-positive drift.

**Context-minimization mechanics (core requirements):**
- AST symbol slicing (tree-sitter): fetch target function + imported signatures only.
- Windowed file paging: view_window(path, line, radius), never read_file whole.
- AST-aware patch validation BEFORE disk commit (reject invalid syntax inline).
- Deterministic error traps: cargo check --message-format=json filtered to
  file:line + primary message only.
- Deterministic prefix caching: stable byte-level prompts (sorted keys/indexmap),
  monotonic append-only buffers, tool schemas byte-identical across turns;
  forward-slash paths in all LLM-facing text.
- Tiered context allocation: Tier0 immutable (system prompt, tool defs, rules),
  Tier1 episodic (goals, scratchpad, sub-task summaries), Tier2 volatile (raw
  tool outputs); eviction at ~70% ceiling condenses Tier2 to 1-line event
  signatures (e.g. read_file("src/main.rs") -> 120 lines).
- Streaming JSON tool dispatch: jiter early-argument extraction + speculative
  I/O pre-warming (open fd/parse AST while model still streams args).
- Virtual PTY + screen-delta scraping: portable-pty + vt100, strip ANSI, capture
  final screen state / delta, not raw byte stream.
- Tool output dedup + reference folding: hash table of prior outputs, fold to
  <inspected path hash lines status/> markers.
- Externalized scratchpad: .harness/scratchpad.md; inject only active step
  ("Step 3/7: ..."), not the backlog.
- LSP client (goto_definition/find_references in <25 tokens vs grep sweeps).
- Lexical search over vectors: Tantivy BM25 + ripgrep; NO vector DB for code.
- Transactional workspaces: CoW staging (gix worktrees, reflink-copy w/ NTFS
  fallback), rollback on failed checks, compact rejection signal instead of
  multi-turn failure loops; single volatile error slot (replace, don't stack).
- Micro-commit checkpoints: refs/harness/snapshots/{turn_id} namespace, /rewind
  reverts files AND truncates context history to that turn.

**Anti-patterns (Must NOT have):**
- Vector DBs for code retrieval; whole-file rewrites / full-file echo; stacked
  compiler-error dumps in context; model-as-linter (offload to local binaries);
  unbounded subprocesses (timeout ceiling ~30s + SIGKILL); arbitrary JSON key
  ordering breaking KV caches; dirty-state retention after failed multi-file edits.

**Extensibility (per user conversation):**
- MCP client (JSON-RPC over stdio + SSE) for ecosystem tools.
- WASM plugins (wasmtime/extism): capability-based I/O, hard memory cap
  (~32MB/plugin), hot-reload, parallel execution without destabilizing core.

**Cross-platform (explicit user requirement):**
- Native Linux/macOS/Windows (ConPTY via portable-pty), optional WSL2 bridge
  (--wsl flag → wsl.exe -e); dunce for \\?\ UNC path stripping; notify for FS
  watching (inotify/FSEvents/ReadDirectoryChangesW); interprocess (UDS + named
  pipes); reflink-copy w/ gix-worktree fallback on NTFS; PowerShell 7 default
  shell on Windows; path normalization filter on every tool argument.

**UI/UX (explicit user requirement — dual interface):**
- PRIMARY: Ratatui + crossterm TUI: repo/branch/mode header; context fuel gauge
  (38.4k/128k, %), KV-cache hit ratio, session cost ticker; collapsible tool
  accordions; streaming markdown; @file @symbol omnibar mentions; /mode
  architect|code|ask; [y] allow-once [a] always-allow-pattern [n] deny+feedback
  permission gates; /rewind checkpoint slider.
- SECONDARY: headless daemon, embedded Axum HTTP/WebSocket on 127.0.0.1 for
  browser/VSCode/Tauri frontends; engine core decoupled from presentation.

**Crate shortlist from conversation (to validate vs research):**
tree-sitter + grammars, wasmtime or extism, compact_str/smartstring, similar,
tokio + reqwest(SSE), indexmap, gix, reflink-copy, jiter, portable-pty, vt100,
tiktoken-rs, tantivy, grep-regex (ripgrep core), interprocess, notify, dunce,
ratatui + crossterm, axum.

### Research findings — librarian 3: Rust ecosystem build-vs-reuse (crates.io + GitHub API, 2026-09-10/11)

**LLM client layer (VERDICT: own thin trait; use crates per-adapter where fit):**
- rig-core 0.42.0 MIT, 2.69M dl, active — restructured (rig-core + rig-agent +
  facade `rig`), 20+ providers, AgentRun state machine, cassette-backed tests in
  their own suite; NOT 1.0, breaking-changes warning → candidate, accept churn.
- genai 0.6.5 / 0.7.0-beta.23 (MIT OR Apache-2.0) — 26+ adapters (OpenAI,
  Anthropic, Gemini, xAI, Ollama(+Cloud), Vertex, Bedrock, OpenRouter, Groq +
  custom OpenAI-compatible), native protocols; single-maintainer (bus factor).
- async-openai 0.42.0 MIT, 8.07M dl — OpenAI(+Azure+compatible) only; Responses
  API, SSE, backoff, tower middleware, WASM; fastest cadence tracking OpenAI.
- llm-chain 0.13.0 — DEAD (2023-11) → AVOID. kalosm 0.4.0 (crate 19-mo stale;
  repo active) local-inference heavy → not core. mistralrs 0.8.1 MIT — local
  inference ENGINE w/ OpenAI-compatible server → run as sidecar process, not embed.
**MCP:** rmcp 3.3.0 (2026-09-10) Apache-2.0, 25.7M dl, Tier-1 SDK (2026-08-21;
  67/67 server + 50/50 client conformance), spec 2026-07-28, client+server+macros,
  tokio/hyper, streamable HTTP + stdio → USE.
**Runtime/util:** tokio 1.53.1; tracing 0.1.44; serde 1.0.229; notify 8.2.0
  (9.0.0-rc.5; CC0); crossterm 0.29; ratatui 0.30.2 (22.5k★); clap 4.6.6;
  rusqlite 0.40.2; redb 4.2.0; fjall 3.1.10 (LSM); sled STALLED (2024-10) → AVOID;
  reqwest 0.13.5; jiff 0.2.35 (time); schemars 1.2.2 (1.0-stable line; tool JSON
  schemas; rmcp dependency).
**Extensions:** wasmtime 48.0.2 (Apache-2.0 W/ LLVM-exception; monthly) → USE;
  deno_core 0.411.0 (MIT; lives in denoland/deno; V8 heavy) → only if JS needed;
  rquickjs 0.13.0 (MIT, active; ~MB footprint); mlua 0.12.1 (MIT, active, luau);
  abi_stable 0.11.3 DORMANT (2023-10) → AVOID (kills dylib plugin route);
  extism 1.30.0 (BSD-3; WASM plugin framework on wasmtime) → candidate to
  evaluate vs raw wasmtime.
**Testing:** mockito 1.7.2; wiremock 0.6.5 (12+ mo quiet — flag); httpmock 0.8.3
  (active, org-moved); mockall 0.15.0; insta 1.48.0; cargo-nextest 0.9.144;
  cargo-tarpaulin 0.37.2 — NOW supports Windows/macOS via LLVM engine (ptrace
  default only Linux x86_64); cargo-llvm-cov 0.9.1; testcontainers 0.28.0;
  reqwest-vcr 0.4.0 (VCR replay for reqwest; low adoption → consider custom
  thin replay on reqwest-middleware; rig cassette = prior art).
**Prior art (Rust harnesses):** goose (aaif-goose/goose, 54k★, Apache-2.0,
  MCP-first, 70+ extensions, Linux-Foundation AAIF April 2026); OpenAI Codex CLI
  (122k★, Apache-2.0, Rust TUI); Grok Build (Apache-2.0 2026-07, Rust fullscreen
  TUI + extension system); swiftide 0.32.1 (slowed since 2025-11); autoagents
  0.4.0 (young).
**Memory note (engineering estimate, unsourced):** client crates thin over
  reqwest (low RSS); mistralrs/kalosm load GB weights; deno_core embeds V8
  (tens-of-MB); wasmtime tens-of-MB w/ pooling allocator; QuickJS/Lua ~MB-class.

### Research findings — librarian 1: coding-agent CLI harnesses (full digest)

**Claude Code (2026 state):** closed-source binary distribution (npm wrapper → Bun/JSC
native build; npm Node/V8); Mar 2026 source-map leak (~512k LOC TS, "undercover mode"
fallout). Memory: ~0.5–1 GiB steady-state RSS (engineer-confirmed), leak reports to
14.5 GiB (issue #86202, v2.1.2xx, Aug 2026); CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
workaround; /heapdump hidden. Features: hooks (PreCompact/PostCompact, SubagentStart;
shell/HTTP/MCP-prompt hook kinds), permission modes (default/acceptEdits/plan/auto-
classifier/dontAsk/bypassPermissions), sandboxed bash (Seatbelt; bubblewrap+seccomp;
NOT native Windows), skills (Oct 2025; agentskills.io open standard; AAIF/Linux
Foundation reported), subagents + agent teams + worktrees, checkpoints (v2 Sept 2025),
context-editing APIs (clear_tool_uses_20250919, memory_20260818→memory_20250818,
compact_20260112), CLAUDE.md hierarchy, Agent SDK (Sept 2025), web version (Oct 2025),
Cowork (Jan 2026). Failures: silent model downgrades (Sept 2025 incident; #40269 Mar
2026 Opus 4.6→Haiku 4.5 mid-conversation), weekly usage-limit backlash (Jul–Aug 2025).
Anthropic context-engineering doctrine: compaction preserves architecture decisions +
unresolved bugs + ~5 recent files; tool-result clearing; subagents return 1–2k-token
summaries; "context anxiety" (Sonnet 4.5 needed harness resets; Opus 4.5 didn't —
harnesses encoding stale assumptions about model limits); Managed Agents
brain/hands/session decoupling; session = append-only log.

**Codex CLI:** TS→Rust rewrite (May 2025, discussion #1174): zero-dep install, native
security bindings, no GC → lower memory; crates core/tui(ratatui)/exec/linux-sandbox;
wire protocol + app server. Sandbox: macOS Seatbelt; Linux bubblewrap+seccomp default
(Landlock legacy); Windows restricted tokens + ACLs + firewall + sandbox users.
Approvals: suggest/auto-edit/full-auto → read-only/auto/workspace-write churn;
untrusted retired; Guardian auto-review (approvals_reviewer="auto_review"); safety
monitoring can pause tasks; network_proxy; protected paths (.git/.agents/.codex);
config.toml + enterprise requirements.toml + MDM; hooks; MCP; AGENTS.md; subagents;
rollouts ~/.codex/sessions + resume/compact; gpt-5.6 default (2026); desktop/web/IDE.
Failures: TS deprecation churn, approval naming churn, Windows long-broken.

**opencode:** saga TermAI→OpenCode (Kujtim Hoxha)→SST→Charm acqui-hire→git-history
rewrite allegations→SST fork kept name (TS/Bun)→Charm renamed theirs Crush→2026
anomalyco/opencode under Anomaly (ex-SST); HN 48491407. Architecture: headless
server + TUI/clients; server API sessions/messages/fork/revert/unrevert/summarize
(compaction endpoint); permissions allow/ask/deny + agent-level overrides +
doom_loop guard + external_directory + .env deny default; plugins (JS); skills;
ACP; LSP; Zen gateway; share links. Failures: rename drama, rapid breaking
changes, Zen monetization friction.

**aider:** v0.86.0 Aug 2025 last release; single-maintainer risk (issue #4613);
maintenance mode; forks (cecli). Repo map tree-sitter + PageRank ranking; edit
formats whole/diff/udiff + architect dual-model mode; git auto-commit + /undo;
human-in-loop by design; NO MCP (deliberate contrast point).

**Gemini CLI:** Apache-2.0 TS/Node; Gemini 3, 1M ctx; free tier 60 RPM/1000 RPD;
preview/stable/nightly weekly. Core: GeminiClient/GeminiChat/Turn;
ChatCompressionService; LoopDetectionService; ToolOutputMaskingService; agents
registry + AgentLoopContext. Checkpointing; sandbox (seatbelt/docker); trusted
folders; extensions; telemetry default-on (criticism); GEMINI.md. Failures: launch
overload; Aug 2025 quota cuts (partial reversal); silent Pro→Flash fallback;
infinite rate-limit loops (#1626/#6986/#2909); prompt-injection RCE demo (Tracebit).

**Converged patterns (5-harness):** MCP everywhere except aider; AGENTS.md/GEMINI.md/
CLAUDE.md project memory files; plan mode; subagents; compaction; session resume;
permission tiers w/ allowlists; OS sandboxing; skills spec; headless/SDK modes;
ACP emerging.

**Librarian-1 deltas (full-report digest):**
- Claude Code: memory issue cluster 2.1.x era = #86202, #67433, #24644, #56693,
  #33594, #32752, #34263, #11315. Skills launch 2025-10-16; open standard
  2025-12-18; v2 2025-09-29 (checkpoints); web 2025-10-20; Cowork 2026-01-12.
- Codex: crates core/tui/exec/linux-sandbox; cutover burndown #1262; TS rationale
  discussion #1174 (2025-05-30): zero-dep install, native sandbox bindings, no GC,
  wire protocol+MCP; GPT-6 Astra in 2026 docs nav; Memories feature; network_proxy;
  codex exec --full-auto DEPRECATED → granular approval policy; enterprise
  requirements.toml + config_toml_base64 MDM + allow_managed_hooks_only.
- opencode: npm opencode-ai; ~v1.1.1; Zen deprecations (GPT 5.2 Codex 2026-07-23,
  Claude Opus 4.1 2026-08-05); doshby-vs-HN history discrepancy noted.
- aider: MCP request #4506 (2025-09-09) unfilled — AiderDesk third-party gap-filler;
  last substantial commit 2026-05-22 (#5647); forks aider-ce/cecli, dwash96.
- Gemini CLI: code map — packages/core/src/core/client.ts (GeminiClient/GeminiChat/
  Turn), context/chatCompressionService.ts, agents/local-executor.ts (renderAgentSkills,
  AgentLoopContext); checkpointing = shadow git ~/.gemini/history/<project_hash> +
  /restore, --checkpointing flag REMOVED v0.11.0 (settings-only) — documented
  reversal; sandbox methods: Seatbelt/docker/podman/runsc/lxc; approvalMode
  yolo/auto_edit/default; telemetry default-on (criticized).
- 5 convergent findings: (1) 2025→2026 consensus = loop + OS sandbox + tiered
  approvals + MCP + compaction + session persistence (aider lacks all → dying);
  (2) documented reversals are design data (Codex untrusted retired; Gemini
  --checkpointing removed; Claude npm→binary→dual after leak); (3) memory/loop
  stability = #1 operational failure class → ship loop-detection + memory budgets
  day one; (4) silent model degradation = trust killer → model-identity transparency
  in harness contract; (5) Rust rationale on record (#1174) — citable argument.
- Standards dates: MCP spec 2025-06-18 (streamable HTTP); SKILL.md open standard
  2025-12-18; AGENTS.md de-facto; ACP (Zed) opencode-confirmed only.
  UNVERIFIED: Anthropic-Bun acquisition (layer5 single-source); AAIF/Linux
  Foundation governance + OpenAI-48h-skills adoption (aictrl-reported).

### Pasted-conversation FINAL REPORT (Portuguese, recovered from tail) — blueprint highlights

- **6-layer architecture, internal crates:** harness-tui, harness-daemon,
  harness-mcp, harness-wasm, harness-lsp, harness-cache, harness-sanitizer,
  harness-eviction, harness-ast, harness-repomap, harness-vfs, harness-cow,
  harness-snapshots, harness-os. No global state; privilege separation.
- **Per-harness overhead table:** Claude Code ~500–1,500 new tok/turn, ~92% cache
  reuse; Codex medium-high (terminal dumps); Cursor ~2–4k tok RAG/turn; Windsurf
  high (telemetry pollution; 70–90% CPU peaks reported); Antigravity very high
  (relies on huge windows); Copilot WS medium (rigid); Aider <1k (repomap);
  SWE-agent low/action but exponential over long sessions; Roo/Cline med-high
  (schema churn between modes; Node race conditions on writes); Zed minimal
  (zero-copy). Key thesis: prompt must be an immutable byte-layout, not a
  re-interpolated string each turn.
- **Provider cache mechanics:** Anthropic cache_control ephemeral markers (min
  1,024 tok Sonnet/Opus, 2,048 Haiku; ≤4 markers; 90% read discount; +25% write
  5-min TTL / +100% 1-h TTL); OpenAI automatic prefix (1,024 min, 128-tok blocks,
  50% discount, no write cost); vLLM/SGLang RadixAttention/KV trees (16–64 tok
  blocks, 100% compute skip, VRAM LRU).
- **Cache-stable prompt layout:** [0,L0]=system+static core tools (1st cache
  point); [L0,L1]=AGENTS.md/CLAUDE.md+stable plugin schemas (2nd); [L1,L2]=repo
  map+cumulative history (3rd marker); ALL volatile vars (timestamp, branch,
  recent cmd results) strictly inside last user message.
- **Deferred tool loading:** only 6 invariant tools declared in system prompt
  (view_window, edit_lines, execute_command, lsp_query, tool_search,
  task_completed); dozens of aux tools listed as compact markdown table (name +
  ~10-word desc); tool_search(query) hydrates full JSON schema into observation;
  system prefix cache never invalidated.
- **Repo map:** tree-sitter tags.scm (@name.definition.* / @name.reference.*),
  persistent tag tables indexed by Blake3 (+mtime skip), directed file graph,
  Personalized PageRank PR(u)=(1−d)·p_u + d·Σ PR(v)/L(v), d=0.85, personalization
  on focused/@mentioned files, power iteration ms-convergence, signatures-only
  rendering with ellipsis markers, ≤1,000-token budget via binary search on node
  density; custom CSR+SIMD PageRank module for 50k+ node monorepos.
- **view_window:** ≤100 lines/call, 1-based, jump via lsp_goto_definition;
  edit_lines returns altered lines + 3-line fringe above/below.
- **PTY pipeline:** portable-pty → vt100 virtual screen 120×40; if >75 log lines
  keep first 25 + last 50 + "[... N intermediate lines omitted ...]"; ANSI/SGR
  stripped before aggregation.
- **Eviction:** trigger at 75% of window; Tier0 immutable (zero eviction);
  Tier1 plan/episodic → condensed to <active_plan> XML by cheap subagent;
  Tier2 volatile → LRU purge to Blake3 reference pointers (+ return codes).
- **Shadow workspaces:** reflink-copy (ioctl FICLONE on Btrfs/XFS; APFS
  copyfile CLONE) or gix ephemeral worktree fallback (NTFS); pre-flight pipeline
  in shadow: (a) tree-sitter parse — abort on is_error()/is_missing() nodes;
  (b) formatter (rustfmt/prettier/gofmt); (c) cargo check --message-format=json /
  tsc --noEmit; failures re-injected in CLOSED sub-cycle, max 2 corrective
  iterations before surfacing to user.
- **Checkpoints:** refs/harness/snapshots/{session_id}/{turn_id}; gix writes
  tree/commit objects IN MEMORY directly to ODB (no git subprocess, no index
  touch, no branch pollution); /rewind N restores files AND truncates history.
- **Semantic hunk staging:** imara-diff (Histogram; claims 10%–100× vs similar)
  correlated w/ tree-sitter entity boundaries; per-block y/n/e; rejection
  feedback names the entity ("user rejected change to compute() signature...").
- **MCP client:** JSON-RPC 2.0 async; stdio (child spawn, newline-delimited,
  stderr→tracing) + Streamable HTTP (SSE); initialize handshake; roots
  confinement blocks paths outside project.
- **WASM:** wasmtime, WASI 0.2; ResourceLimiter 32 MB/plugin; fuel metering
  (infinite-loop kill); preopened dirs only; network denied by default; AOT
  instantiation in tens of µs.
- **LSP client:** lsp_goto_definition / lsp_find_references / lsp_hover;
  <25 tokens per answer vs grep sweeps.
- **Windows ConPTY pathologies (must handle):** (1) headless output hang —
  pty.wait() blocks forever; fix: async timer + output read-descriptor close
  detection + explicit master destroy; (2) shared-ptr/Arc shutdown assertion —
  fix: RAII + Job Objects w/ JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE for atomic
  descendant-tree kill; (3) DA1 probe ESC[c 60-s hang — fix: proactive simulated
  response injection into input channel.
- **Paths/FS:** dunce strips \\?\ UNC; forward slashes in ALL LLM-facing text
  (cache stability); notify watcher w/ filters rejecting target/, node_modules/,
  .git/ (fd-exhaustion avoidance).
- **WSL2 bridge:** detect /proc/sys/fs/binfmt_misc/WSLInterop or wsl.exe;
  route via wsl.exe --exec; C:\↔/mnt/c/ mapping in-memory (no wslpath calls).
- **TUI:** event-driven render (0.0% idle CPU); collapsed accordions
  "[✓] cargo test (58 passed, 1 warning) — Enter to expand"; HUD = cache-recovered
  tokens, cost w/ cent precision, context saturation; @file/@symbol/@git fuzzy.
- **Daemon:** Axum WebSockets+SSE, binary-efficient deltas; local IPC via
  interprocess (UDS / named pipes).
- **Crate table (chosen vs discarded):** portable-pty (+own hardening wrapper);
  vt100 (>vte); gix (>git2: pure Rust, no C deps, in-memory objects);
  reflink-copy; tree-sitter (+official grammars); imara-diff (>similar,
  dissimilar); jiter (>serde_json for partial/streaming); tantivy+grep-regex
  (>vector DBs); interprocess; tokio multi-thread; wasmtime (>extism);
  ratatui+crossterm (>cursive).
- **Custom modules to build:** Binary Prompt Idempotency Manager (JSON field
  ordering, path separator serialization, prefix anchoring); CSR PageRank SIMD;
  ConPTY Lifecycle Supervisor (RAII + Job Objects); AST Semantic Diff Grouper.
- **Section 9 ANTI-PATTERN CATALOG (8 failure modes → Rust fixes):**
  1. Accidental Prefix Invalidation — unordered dicts/timestamps in system prompt/
     changing tool set → KV-cache breakage every turn, costs up to 10×, latency →
     Immutable Static Buffer: system+tools at offset 0; dynamic vars ONLY in last msg.
  2. Ghost Edits — naive str.replace() w/ ambiguous multi-match patterns → wrong
     file sections mutated, agent loses sync → Unequivocal AST Validation: mutations
     bound to syntactic node ranges + pre-application uniqueness check.
  3. Poisonous Self-Correction Cycles — blind patches straight onto user's branch →
     dozens of iterations amplifying errors, $$ burn → Shadow Workspaces + Pre-Flight:
     test/compile on isolated clone; rollback on static error.
  4. PTY Noise Saturation — raw build output into history → tens of thousands of
     junk tokens, plan eviction → VT100 emulator + intermediate-line pruning.
  5. PTY Hang on Windows — blocking child.wait() under headless ConPTY → indefinite
     freeze after tests finish (Windows/CI) → synchronous MasterPty destruction via
     async timers.
  6. Orphan Process Proliferation — Ctrl+C leaves servers/compilers alive → RAM
     saturation, port 8080 blocked, file locks → RAII guards + Job Objects (Drop).
  7. Path & Cache Inconsistency — mixed \, /, \\?\ → script call failures +
     systematic cache breakage on hybrid-OS teams → mandatory POSIX normalization
     + dunce prefix cleanup.
  8. Monorepo Memory Exhaustion — networkx-style OOP graph libs → tens of GB RAM
     for millions of nodes → contiguous CSR matrix graph (no per-node pointers).
- **Section 10 CONCLUSION:** prefix idempotency → ~80–90% inference billing cut on
  long tasks (economic viability of autonomous agents); repomap <1,000 tokens;
  CoW/worktree shadows kill invalid-syntax context-degradation cycles; Job Objects +
  ConPTY anomaly fixes + dunce normalization = corporate-environment robustness.
  NO separate roadmap/MVP sections exist in the pasted report (verified to EOF).
- **Numbers ledger (source of truth):** 500–1,500 tok/turn & ~92% cache (Claude);
  >1,024 & 128-tok blocks (OpenAI); 2,000–4,000 RAG (Cursor); 70–90% CPU (Windsurf);
  1M–2M windows (Antigravity); <1,000 map (Aider); 60 FPS TUI; d=0.85 PageRank;
  100-line view ceiling; 3-line fringe; 6 core tools; 10-word aux descriptions;
  120×40 vt100; >75 lines → 25+50; 75% eviction trigger; 32 MB WASM; <25 tok LSP;
  max-2 corrective iterations; >90% cache-hit target; 4 markers; 2,048 Haiku min;
  −90% read / +25% / +100% write; 5 min/1 h TTL; −50% OpenAI; 16–64 vLLM blocks;
  10× anti-pattern cost; 10%–100× imara-diff; 50,000-node CSR threshold; tens-of-µs
  WASM AOT; 0.0% idle CPU; 80–90% billing cut; ESC[c 60 s; KILL_ON_JOB_CLOSE.

### Research findings — librarian 6: plugin ecosystems / latent needs (2026-09-11 data)

**Scale:** anthropics/claude-code 144.7k★; anomalyco/opencode 206.6k★; gemini-cli
106.9k★; aider 48.9k★. claudemarketplaces.com indexes 23.6k skills, 2.7k
marketplaces, 12.8k MCP servers (380k monthly visitors). Official curated:
anthropics/claude-plugins-official.

**Top plugins by stars (gap filled → latent need):**
- obra/superpowers 284.9k★ — skills METHODOLOGY pack (TDD/debugging/planning
  discipline); works cross-harness (Claude/Gemini/Cursor/OpenCode manifests).
- anthropics/skills 175.7k★ — official Agent Skills repo (absorbed community).
- wshobson/agents 39.6k★ — multi-harness plugin marketplace (83 plugins/191
  agents/155 skills/102 commands; targets Claude+Codex+Cursor+OpenCode+Copilot).
- davila7/claude-code-templates 30.6k★ — config/setup CLI + template gallery.
- VoltAgent/awesome-claude-code-subagents 25.0k★ — 100+ specialist subagents.
- winfunc/opcode 22.4k★ (fka claudia) — GUI workbench (agents/sessions/bg agents).
- ccusage 18.5k★ + Usage-Monitor 8.7k★ — cost tracking (absorbed → /cost,
  /usage, spend limits, modelPricing, /skill-doctor in 2.1.x).
- claudecodeui 13.6k★ — cross-harness web/mobile GUI.
- claude-code-security-review 6.2k★ (official action); spec-workflow 3.9k★;
  hooks-mastery 3.9k★; wshobson/commands 2.6k★; ruvnet/ruflo 72.0k★ (fka
  claude-flow; swarm orchestration); anthropics/life-sciences 596★ (OFFICIAL
  non-coding domain pack — precedent for our domain packs!).
- OpenCode: oh-my-openagent 68.9k★; awesome-opencode 10.2k★; opencode-mem
  1.6k★; claude-auth 1.3k★; notifier 815★; browser 573★; pty 572★; scheduler 486★.
- MCP (capability outsourcing evidence): firecrawl 178.9k★; awesome-mcp-servers
  94.8k★; reference servers 90.2k★; context7 61.9k★; playwright-mcp 37k★;
  github-mcp 32.9k★; DesktopCommander 9.5k★. npm weekly: filesystem 504k,
  everything 148k, sequential-thinking 85k, memory 75k, github 68k, postgres 58k.
- Cursor: awesome-cursorrules 40.8k★; .cursorrules DEPRECATED → official Project
  Rules (.cursor/rules); 2026 OFFICIAL plugin marketplace (verified skills/
  subagents/rules/hooks/commands/MCP categories).
- Gemini CLI: official extensions system (60+ official org; partners: Dynatrace,
  Elastic, Figma, Harness, Postman, Shopify, Snyk, Stripe); community: jules 415,
  prompt-library 413, ralph 333 (ralph-loop AS extension!).
- Aider: NO plugin system (IDE companions only). Windsurf: official Rules +
  Memories + Workflows ("became Devin Desktop, Cascade retired 2026-07-01" —
  UNVERIFIED flag).

**Absorption (plugin-born → core) evidence:**
- Claude Code 2.1.x changelog: native subagents (cap 20, per-agent cache TTL,
  SUBAGENT_MODEL_FORCE), plugins CLI + enterprise marketplaces gating
  (strictKnownMarketplaces/blockedMarketplaces), MEMORY.md native memory
  (2.1.214), /workflows AGENT TEAMMATES + dynamic workflow size (native team
  orchestration), self-hosted runners, /insights, monitors, Artifact tool.
  Launches: plugins 2025-10-09; Agent Skills 2025-10; subagents ~2025-07/08;
  hooks ~2025-06/07; output styles ~2025-08 (approx flags).
- OpenCode absorbed natively: Agent Skills (loads .opencode/.claude/.agents
  skills paths!), plugins, custom commands, agents, themes, LSP config,
  custom tools.
- Anthropic: "single agent + skills beats subagent design" claim; commerce
  agents blueprint; memory extraction async w/ +13% fact recall.

**LATENT-NEED MATRIX (category × harnesses; ≥2 = build-in candidates):**
1. Prompt/rule-skill libraries (4+)  2. Subagent/specialist packs (4)  3. Memory/
persistent context (5+; absorbed 3 cores)  4. Cost/usage tracking (3; absorbed)
5. Browser automation + visual QA (3)  6. Docs/knowledge retrieval — context7
(3+)  7. Web search/scrape (cross-cutting)  8. Orchestration/teams (3+; absorbed
as /workflows)  9. Spec/discipline workflows (4+)  10. Hooks/safety gates (4)
11. GUI/workbench over daemon (2; validates our daemon+TUI)  12. Auth/provider
bridging (OpenCode-specific)  13. Notifications/scheduler (OpenCode-specific)
14. Multi-domain non-coding packs (life-sciences/commerce/equity — emerging)
15. Theme/statusline/output-styles (3)  16. Enterprise governance (emerging).
STRONGEST cross-harness signals: memory, cost tracking, orchestration, spec
workflows, hooks, rule/skill libraries, browser automation, docs retrieval.

### Research findings — librarian 2: autonomous + Rust-native harnesses

**goose (→ aaif-goose/goose, AAIF/Linux Foundation Dec 2025, repo moved 2026-04-07):**
- Client-server: goose-server REST + ACP JSON-RPC; CLI/desktop/custom UIs on one core.
- Loop = imperative STATE-MACHINE of named Operations (agent.rs): EntryHook →
  SlashCommand → Steer → MaxTurns → Compaction → ToolPairCompaction →
  ToolApproval → ToolExecution → Retry → StopHook → ExitOnError → Inference.
- 4 autonomy modes: Auto/Approve/Chat/SmartApprove. Per-tool-call inspection
  chain: SecurityInspector (LLM reviews risky calls) + PermissionInspector +
  RepetitionInspector (runaway-loop detector). StopHook w/ block cap
  (GOOSE_STOP_HOOK_BLOCK_CAP); recipe retry {max_retries, checks}.
- Compaction at threshold fraction of context_limit (keep system + last ~10
  turns, summarize middle); ToolPairCompaction for old tool pairs.
- Sandbox: macOS Seatbelt (v1.25 Desktop); Docker container for extensions w/
  env scrubbing (Envs::DISALLOWED_KEYS); NO Windows/Linux native sandbox.
- Security incident: GHSA-r5pp-p5r8-466r — arbitrary command execution via
  `goose review` + malicious git core.fsmonitor (v1.44 fix). Git-integrated
  agents = attack surface.
- Multi-agent: subagents + skills CONSOLIDATED into "Summon" extension
  (load/delegate) v1.25 — "overlapped in confusing ways". SubAgent sessions
  isolated w/ parent link + auto-cleanup; recipes compose via sub_recipes.
- Persistence: SQLite via sqlx, schema v16, atomic SessionUpdateBuilder;
  resumable + forkable; JSON→SQLite migration mid-life (precedent!);
  chat_history_search + chatrecall over own history.
- Provider traits: #[async_trait] pub trait Provider: Send + Sync (dyn-safe);
  ProviderRetry split "to keep Provider dyn-compatible".
- Scaled to ~60% of Block (recipes); governance headwinds → LF donation.

**crush (Go, TUI craft reference):** App owns services (Sessions/Messages/
Permissions/FileTracker/LSPManager/AgentCoordinator) via pubsub.Broker[tea.Msg];
Workspace interface unifies local + remote `crush serve` (SSE) — TWO TUI clients
share one workspace incl. permission queue. Deterministic shutdown order
(agents→DB→MCP→LSP). SessionAgent: per-session queuing, cancel-mark/high-water
sequences (no poisoning later prompts), auto-summarize near limit. SQLite+sqlc,
SetMaxOpenConns(1) vs WAL races, data-dir locking, platform driver split.
Sub-agent runs = child sessions (messageID$$toolCallID) hidden from list.
Origin dispute w/ opencode (git-history rewrite allegations) — community
ownership lesson.

**OpenHands:** V0 EventStream pub/sub failed ("very confusing, thread/async
issues, few order guarantees") → V1 EVENT SOURCING (MLSys 2026): immutable
event log; ConversationState only mutable; agent/LLM/tools immutable
serializable specs; events as JSON files + state.json; deterministic replay;
sub-ms persist, <20ms crash recovery. Stuck detection (sliding window);
condensers; max_iterations + max_budget_per_task. Docker everywhere; Windows
WSL2-only. V0 removed 2026-04-01.

**SWE-agent:** ACI discipline; "agents succeed quickly and fail slowly" (cost
budget finding); SWE-ReX runtime (local/Docker/AWS/Modal; non-Linux w/o
Docker); .traj trajectories; HistoryProcessor; mini-SWE-agent = 100 lines, 65%
of full performance. Team remains academic.

**Anthropic harness doctrine:** multi-agent orchestrator-worker; failure
patterns: 50-subagent spawns, endless searches; ~15× token cost vs chat.
Long-running: initializer + coding agent; claude-progress.txt; feature-file
JSON; git commits as progress. "Context anxiety" (Sonnet 4.5; gone in Opus
4.6) — harnesses encode stale model assumptions. Managed Agents:
planner/generator/evaluator; brain/hands/session decoupling; append-only
session log; getEvents().
**Sandbox landscape:** containers vs microVMs vs OS-level (Firecracker ≤125ms
but Linux+KVM only, no Windows; gVisor; Claude Code bubblewrap default on
Linux/WSL2); E2B <200ms; agentjail ADR: microsandbox/libkrun cross-platform
incl. Windows WSL2.
**Rust ecosystem extras:** Swiftide (stop conditions, docker executor);
NorviaLabs/forge (SQLite event journal + resume); adulari/forge (model mesh,
doom-loop guards, completion gate); harness-rs (L1/L2/L3 autonomy,
StuckPolicy).

### Research findings — librarian 4: extensibility patterns

**Mechanisms per harness (2026):**
- Claude Code: hooks (shell/HTTP/MCP-tool/LLM-prompt/subagent kinds; events
  SessionStart/End, UserPromptSubmit, Stop/StopFailure, Pre/PostToolUse);
  skills (SKILL.md per agentskills.io open standard; commands ≡ skills;
  progressive disclosure; subagent execution); plugins (.claude-plugin/
  plugin.json bundling skills+agents+hooks+MCP); settings precedence
  managed > CLI --settings > local > project > user; schemastore schema;
  claude doctor.
- Codex: config.toml + requirements.toml + enterprise MDM; hooks w/ TRUST
  REVIEW + hash pinning + allow_managed_hooks_only; skills on Agent Skills
  standard (+ agents/openai.yaml per skill); ChatGPT+Codex share ONE universal
  plugin directory (skills+MCP+hooks+UI); "Submit a Claude Code plugin"
  guide — plugin-format convergence!; AGENTS.md (60k+ projects, LF-hosted).
  codex-rs ModelProviderInfo = serde DATA + registry, NOT trait objects.
- opencode: 8-layer config precedence (remote .well-known → global → env →
  project → .opencode dirs → inline env → managed files → macOS MDM);
  published JSON schema; auto-migration of legacy keys; JS plugins
  (tool.execute.before hooks).
- goose: recipes YAML (parameters, retry, subrecipes, cron via `goose
  schedule add`); custom distributions.
- Zed: Rust→wasm32-wasip2 via zed_extension_api; WIT versioned; CAPABILITY
  system (process:exec, download_file, npm:install w/ patterns); wasmtime
  host w/ epoch interruption + 32MB incremental compile cache; chose Rust+Wasm
  over JS/Deno.
- Neovim (cautionary): nvim-treesitter master→main full incompatible rewrite
  ("You don't migrate — it's a separate plugin"); ABI 15 bumps.
- Obsidian: manual plugin review; PHANTOMPULSE RAT (CSA 2026-04) — malicious
  shared vault + legit Shell Commands plugin → PowerShell on vault open.

**Rust trait design (2026 state):** AFIT stable since 1.75 but NOT
dyn-compatible; Rust 2026 Project Goal "Native async fn dyn dispatch in
traits" (dyn_box! nightly, 2026-27); workarounds: async-trait (goose),
dynosaur, trait_variant; genai uses ENUM dispatch (AdapterKind); Rust 2024
edition = 1.85; async closures stable 1.85. Design guidance: dyn-safe traits
via async-trait OR enum dispatch for closed sets; data-driven configs via
serde+schemars.

**Extensibility security incidents (catalog):**
- GlassWorm (2025-10): first self-propagating worm in VS Code/OpenVSX;
  invisible Unicode C2; 35,800 downloads; returned Nov 2025 (~10k more, 60
  victims). Shai-Hulud npm worm (2025-09, 180+ packages).
- MCP: Supabase MCP exfil (prompt injection via support ticket → service_role
  leak); CVE-2025-6514 mcp-remote CVSS 9.6 (437k+ downloads); CVE-2025-49596
  Inspector 9.4; tool poisoning (Invariant); postmark-mcp rug pull (15 clean
  versions then backdoor); MCPTox benchmark: 72.8% attack success; Docker MCP
  sweep: 43% command injection, 33% unrestricted fetch, 5.5% tool poisoning.
→ Implications: pin + hash-verify external plugins; capability-scoped tool
surfaces; trust review on hook/config changes; namespaces for plugin-provided
tools.

### Research findings — librarian 5: TDD strategies

**How the real ones test:**
- Codex (Rust): core/tests/common/responses.rs (~1.6k lines) — SSE event
  builder DSL (ev_response_created/ev_assistant_message/ev_completed,
  sse_failed, up_to_n_times sequencing, set_delay); mock_model_server
  (SeqResponder); app-server v2 suite ~50 files (thread lifecycle, turn
  interrupt/steer, retries); cli/tests via assert_cmd; WebSocket test server
  (accept_delay, close_after_requests — mid-stream drop simulation!);
  hermetic default /models; CI = cargo test + cargo nextest ARCHIVE on
  platform matrix; dev-deps: insta 1.46.3, wiremock 0.6, assert_cmd 2,
  tempfile 3. wiremock SSE = whole-body via set_body_raw (adequate — parsers
  read line-wise).
- goose (Rust): DummyApi rule-based fake (api.on(matcher).reply(...);
  ApiResponse variants: Reply/ToolCall/NoChoices/OutputLimit/ContextLimitError/
  ServerError/EmptyServerError/ReplyThenServerError) + ResponseGate (Condvar)
  for race/cancel tests; mcp_replays recorded fixtures; acp_fixtures scripted
  transcripts + dummy provider; scenario_tests run SERIALLY (--jobs 1);
  conformance CI: model-toolcall-conformance.yml — "deliberately NOT a PR
  gate" (scheduled daily; live models too flaky for PRs); mcp-conformance w/
  expected-failures YAML pinning known failures by version; evals/harbor =
  Terminal-Bench 2 (89 tasks) comparing harnesses (pass%, cost, turns).
- opencode (TS/Bun): bun test --timeout 30000; @opencode-ai/http-recorder —
  cassette schema + redaction + matching + STRICT replay (die on unused
  interactions; CI=always-replay; record-once-locally w/ env-gated auth);
  regression-named tests (negative-tokens-regression, snapshot-tool-race,
  worktree-endpoint-repro).
- aider (Python): MagicMock patch of litellm.completion w/ side_effect
  sequences (RateLimitError injection); docker-isolated benchmark dir.
- Claude Code: closed source, no public suite.
**Replay/cassette in Rust:** rvcr stale (2024-03, reqwest 0.11); NO crate
handles SSE streaming semantics — field answer: scripted mock servers (codex/
goose pattern) or purpose-built recorder (opencode pattern). mockito
with_chunked_body = chunked encoding w/ sleeps between writes — closest to
real SSE timing; use for idle-timeout/mid-stream tests.
**Rust TDD stack:** cargo-nextest (0.9.143, archive-based CI, ~3× faster);
insta snapshots; assert_cmd; tempfile; tokio start_paused for timers;
coverage: cargo-llvm-cov (tarpaulin Linux-only ptrace).
**Evals vs tests boundary:** deterministic mocked per-PR blocking vs
live-model scheduled non-blocking (goose policy, explicit). Anthropic
"Demystifying evals for AI agents" (2026-01-09): task/trial/harness/grader
taxonomy; prefer outcome/state checks over rubrics; regression vs capability
evals; 20-50 tasks; LLM judges w/ structured outputs. No mature native Rust
eval framework — pattern: expose --output-json/session transcripts so
external harnesses (promptfoo/inspect-ai/harbor) drive the binary.
**Failure lessons (tests that would have caught them):** Codex stream-
disconnect epidemic (#8865 root cause: newer models' inter-batch latency
exceeded HTTP timeout; #19121 reconnect exhaustion loses turn state; #927
feature: recover runs after disconnect) → idle-timeout + mid-stream-drop +
resume tests; gemini-cli data loss (#4586 mkdir-failed-treated-as-success
cascade; #7389/#11442/#15821/#19729 .git deleted, sandbox disabled) →
  confirmation-gate + read-after-write verification tests; Replit agent
  deleted prod DB during code freeze (2025-07) → guardrails as tested code not
  prompts; Anthropic eval-harness defect (model dodged grader bug w/ LIMIT
  clauses) → graders themselves need tests.

### Frontend & productivity particularities (user amendment m0042 — design requirements)

**Frontend architecture principle:** ONE daemon, ONE client protocol (WS events +
REST/IPC commands), FOUR first-class frontends (CLI, TUI, Web, VSCode). Any
frontend can ATTACH to a live session; multiple frontends attach
simultaneously (crush-proven pattern: shared permission queue, synchronized
state). Other IDEs (JetBrains) = future thin clients over the same protocol.

**Productivity particularities (research-backed, in plan scope):**
1. **Seamless handoff / attach-detach**: start a task from VSCode, monitor from
   phone via Web, take over in TUI over SSH — daemon owns the session,
   frontends are views (opcode 22.4k★ GUI latent-need validates).
2. **Ambient editor telemetry** (Windsurf pattern): VSCode ext streams active
   file, cursor, selection, recent edits, diagnostics as opt-in context
   channel — "fix this lifetime error" with no "which file?" roundtrip.
3. **IDE-native diff review**: semantic hunks as native VSCode diff
   decorations w/ per-hunk Accept/Reject code lenses — never wall-of-text
   diffs in chat.
4. **IDE resource bridging**: extension proxies the IDE's ALREADY-RUNNING
   language servers + ripgrep to the harness — zero duplicate indexing in the
   most common workflow.
5. **Omnibar everywhere**: @file / @symbol (tree-sitter fuzzy) / @git + /mode
   /rewind /skill — identical grammar across CLI, TUI, Web, VSCode palette.
6. **Always-visible efficiency HUD**: context fuel gauge, KV-cache hit %,
   live cost ticker (cent precision) in TUI header, VSCode status bar, Web
   dashboard (ccusage 18.5k★ latent need → built-in).
7. **One-key permission gates**: [y] once / [a] always-this-pattern / [n]
   deny+feedback — identical across frontends (toast+palette in Web/VSCode).
8. **Time-travel slider**: /rewind + Ctrl+Left/Right through snapshots;
   visual timeline in Web.
9. **Background sessions + notifications**: multiple sessions; system
   notifications when long tasks finish or need approval (opencode-notifier
   latent need → daemon built-in).
10. **Share links** (opencode pattern): read-only session replays for team
    review — redacted, encrypted, expiring, served by the daemon.
11. **Quick-capture**: CLI one-liner + global-hotkey skill target for
    background task submission; minimal keystrokes philosophy.
12. **Progressive disclosure**: collapsed tool accordions w/ Enter-to-expand
    (paste §7); lazy trees in Web/VSCode.
13. **Dashboard analytics**: cost + cache-saved $ + tool heatmaps + eval
    trends (Web; local data only).
14. **Terminal-velocity CLI**: everything scriptable headless
    (--output-json) — doubles as the eval-harness adapter.

**Web security defaults:** daemon binds 127.0.0.1 + generated auth token;
remote = explicit opt-in; share links redacted+expiring; no cloud dependency.

### Additional improvements round (user m0047 — folded into scope)

A. **Cross-session memory subsystem** (latent-need #3, 5+ harnesses): curated
   MEMORY.md-style notes + BM25 index over past sessions (local), injected via
   Tier 1; /memory + /recall commands; goose chatrecall + Claude MEMORY.md +
   basic-memory 3.9k★ + opencode-mem 1.6k★ precedents.
B. **Subagent orchestration / team mode** (latent-need #8; OMO Team Mode +
   Anthropic orchestrator-worker + goose Summon): lead + bounded parallel
   members; effort-scaling rules (NO 50-subagent spawns — Anthropic failure
   pattern); subagents return 1–2k-token summaries; child sessions isolated +
   hidden (crush messageID$$toolCallID pattern); tmux-grid viz in TUI.
C. **Budget caps**: per-task/session/day $ + token ceilings (OpenHands
   max_budget_per_task + Claude spend limits); hard-stop + warn thresholds.
D. **Scheduled runs**: cron execution of packs/recipes via daemon (goose
   scheduler; opencode-scheduler 486★ latent need).
E. **Mid-stream recovery**: transport-disconnect resume w/ turn-state
   preservation; idle timeouts; reconnect w/o context loss (Codex #8865 root
   cause = model latency exceeded HTTP timeout; #19121 loses turn state;
   #927 feature ask — we build it day one).
F. **Rules files hierarchy**: AGENTS.md/CLAUDE.md/GEMINI.md-compatible layered
   rules (global > project > nested dirs), Tier-0 injection, /init-deep-style
   generation command (OMO pattern).
G. **User lifecycle hooks**: SessionStart/UserPromptSubmit (context
   augmentation)/Stop (block-until-criteria)/Pre+PostToolUse; kinds: shell/
   HTTP/MCP-tool; trust review + hash pinning (Codex model).
H. **Autonomy dial**: Auto / Approve / Chat / SmartApprove (LLM-judged) modes;
   plan-mode pairs with read-only sandbox (goose + Claude pairing).
I. **Guardian inspector + egress control**: optional LLM reviewer for risky
   tool calls (goose SecurityInspector + Codex Guardian auto_review);
   protected paths (.git/.agents/.harness defaults); network egress allowlist
   for sandboxed commands (Codex network_proxy).
J. **Hash-anchored edits (hashline-style)**: optional line-hash anchors in
   edit blocks; reject-on-mismatch — weak-model edit success 6.7%→68.3%
   (OMO evidence); complements AST validation.
K. **Per-role model routing**: Architect/Editor dual-model (aider), cheap
   model for tool_search hydration + summarization, strong for planning;
   fallback chains REQUIRE user confirmation (no silent substitution).
L. **`harness doctor`**: config/provider/sandbox/MCP health diagnostics
   (Claude doctor + OMO doctor precedents).
M. **Output styles**: concise/verbose/default personas (small, cheap).
N. **Session replay debugging**: deterministic replay view in Web from the
   event log (OpenHands V1 killer feature); doubles as share-link replay.
O. **ACP adapter (optional)**: Agent Client Protocol for Zed/editor interop
   (goose-proven; cheap once daemon protocol exists).
P. **Thinking budgets + display**: surface extended-thinking streams; config
   thinking budgets per mode.
Q. **Background tool tasks**: fire-and-forget bash w/ log checkpoints +
   completion notification (Claude Code background bash).

### Self-improvement & orchestration amendments (user m0051 — folded into scope)

**R. Skill Forge (auto skill authoring/updating — "learning from the user"):**
- Distiller agent (cheap model) runs post-task (or on /learn): reviews session
  trajectory → extracts reusable procedure → drafts/updates SKILL.md w/
  triggers + negative scoping; proposes DIFF against existing skill (never
  silent overwrite).
- User-correction journal: hunk rejections w/ feedback, permission denials w/
  custom feedback, manual edits after agent output — all captured as structured
  signals feeding skill updates ("the user always wants X" → skill note).
- Trust gate: auto-generated skills land in local DRAFT tier; require explicit
  approval before activation; hashed + versioned w/ changelog (registry trust
  rules apply); NEVER auto-executed.
- Eval-before-activate: new/updated skill must replay successfully against the
  originating session type (deterministic replay) before promotion.
- Usage analytics: invocations, success/failure signal, context cost (Claude
  /skill-doctor precedent) → pruning suggestions (skill sprawl guard).
- Precedents: Anthropic memory extraction (+13% recall), agents-author-skills,
  Voyager skill library, Windsurf Memories; failure guards vs drift/fossilizing
  (approval + eval + versioning). Hermes/"Auto Agent" identification pending
  (librarian dispatched).

**S. Spec-driven multi-agent launcher:**
- Spec format: markdown spec → structured decomposition (objectives, tasks,
  dependencies, per-task capability+model requirements) — mirrors our own
  ulw-plan topology (components/waves/dependency matrix).
- Launch: N agents w/ heterogeneous (capability, model) profiles on subtasks;
  isolated child sessions; budget guards + effort-scaling rules; synthesis
  agent merges w/ conflict resolution; per-agent telemetry (tokens, cost,
  duration, failure class).
- Merge discipline: structured results, dedup, cross-consistency check
  (audit-consolidated pattern from user's own skill packs).

**T. Model-profiles subsystem (understand each model's problems):**
- Per-model failure taxonomy telemetry: syntax-error rate, tool-misuse rate,
  context-overflow frequency, retry storms, hallucinated-API calls, edit-
  rejection rate, cost/turn — computed from session transcripts (local).
- **BEHAVIORAL AXES (user m0062):** verbosity index (output tokens/task),
  thinking depth + reasoning-token budgets, delegation preference (spawns
  subagents vs works directly), tool-call precision, instruction-drift curve
  over long contexts, edit-format competence (whole/diff/udiff/hybrid) —
  all tracked per model+version; elicitation layer chooses per-model strategy:
  prompt shaping, tool-design compensation (e.g., hashline for imprecise
  editors), thinking-budget tuning, architect/editor role assignment, output-
  style enforcement, subagent-count ceilings for over-delegators.
- Model profile store: capability/weakness records per model (+version),
  updatable; feeds (a) router (per-role model routing D/K), (b) skill
  hydrator — skills gain per-model guidance sections ("with model X always
  request exact line ranges"), (c) launcher balancing.
- A/B skill optimization: run skill variants across models on benchmark
  mini-tasks (evals component) → auto-select best variant per model; report
  as skill-improvement proposals (through Skill Forge trust gate).

**U. Performance engineering (hard budgets + CI gates — user re-emphasis):**
- Compile-time: release profile w/ LTO=fat, codegen-units=1, opt-level=3,
  strip; feature-gated heavy deps (wasmtime/tantivy/tree-sitter grammars
  optional); workspace thin-binary trick for CLI.
- Runtime budgets (targets, CI-enforced): CLI cold start <50ms; TUI idle CPU
  0.0% (event-driven only); daemon steady RSS <80MB baseline (excl. indexes);
  AST repomap incremental update <10ms on mtime-hit; PTY scraper throughput
  ≥10MB/s; stream render p99 <16ms/frame; session lazy-load page <5ms.
- Allocation discipline: compact_str for small strings; arena for tree-sitter
  node payloads; zero-copy jiter parsing; CSR graph in contiguous Vec; reuse
  buffers in PTY pipeline; mimalloc optional feature.
- Measurement: criterion benchmarks (repomap, eviction, prompt-builder,
  PTY scraping, session paging); dhat/heaptrack profiles archived per
  release; perf regression gate in CI (fail on >10% regression vs baseline);
  binary-size budget (<40MB stripped release, per-target tracked).
- Efficiency behaviors already core: prefix-cache-stable serialization,
  deferred tool loading, tiered eviction, dedup folding, lazy sessions,
  PTY delta scraping — the entire context-economics stack IS the efficiency
  story; U adds the measurement+enforcement layer.

**Coverage inventory (user Q2, m0047):**
DEEP: Claude Code; Codex CLI (source-level); opencode (+tests, saga); aider;
Gemini CLI; goose (source-level); crush; OpenHands (paper-level); SWE-agent
(paper-level); AutoGPT-era; Zed extension arch; OMO + user skills (local);
Cursor/Windsurf (pattern-level via multiple sources); Anthropic doctrine.
LIGHT/GAPS (execution-time spikes, none block planning): Cline/Roo checkpoint
+ diff-view internals (VSCode wave); Grok Build (TUI wave); Copilot Workspace
(cloud-coupled); Antigravity/Devin/post-merger Windsurf (closed, UNVERIFIED);
Kilo/Mentat/Plandex/Pi (pattern-level); AMP/Droid/Qwen (derivative/closed);
Rust micro-harnesses forge/harness-rs/adulari (core-wave trait skim);
Hermes/"Auto Agent" auto-skill-learning systems (librarian bg_96ff4596 in
flight, folds into Skill Forge R on return).

### Local inspection results — Oh My Openagent v4.19.4 + user skills (explore)

**OMO (ultimate edition, OpenCode plugin, npm dual-name oh-my-openagent/
oh-my-opencode; light edition for Codex via lazycodex-ai):**
- Components: 11 agents (Sisyphus orchestrator, Hephaestus deep worker, Prometheus
  interview-planner, Oracle, Librarian, Explore, Multimodal-Looker, sisyphus-junior,
  plan, review-work, Metis); 5+12 skills; 5 commands; 54–61 lifecycle hooks
  (disabled_hooks config); tools: hashline hash-anchored edits (LINE#ID tags,
  reject-on-mismatch; cited 6.7%→68.3% weak-model edit success), LSP tools,
  AST-grep, tmux, session tools, team_* tools; runtime-injected MCPs (Exa,
  Context7, Grep.app) + skill-embedded MCPs spin-up/teardown on demand;
  AGENTS.md + .omo/rules auto-injection; /init-deep hierarchical context.
- Discipline: ultrawork keyword, /goal persistent objectives w/ idle re-injection,
  Todo Enforcer, ulw-loop durable state in .omo/ulw-loop/, Comment Checker.
- Team Mode: lead + up to max_parallel_members (e.g. 4-8) category-specialized
  members; members are kind:"category" routed via sisyphus-junior; categories
  (quick, deep, ultrabrain, artistry, unspecified-low/high, writing,
  visual-engineering) map to model + prompt shaping; tmux visualization.
- Config: opencode.json plugin entry; .opencode/oh-my-openagent.jsonc walked-up
  to $HOME closest-wins; JSONC; schema.json export; doctor CLI.
- Distribution: npm; bin aliases omo/lazycodex; platform binaries as
  optionalDependencies ({os}-{arch} + -baseline variants); package cache
  install @latest-pinned; monorepo of 27 focused packages (skills-loader-core,
  team-core, hashline-core, rules-engine, config-core, claude-code-compat-core);
  explicit multi-harness refactor (OpenCode/Codex/Pi/Claude Code targets).
- Skills of note: hyperplan (5 hostile critics adversarial planning; hard
  preconditions + degraded-roster fallback; Korean trigger phrases), pre-publish-
  review (16-agent release gate w/ release-layer taxonomy), work-with-pr (task-
  owned git worktree lifecycle + evidence QA), github-triage (1 item = 1
  background task; permalink evidence mandate; never acts), security-research
  (3 hunters + 2 PoC engineers), tech-debt-audit (9-dim), codex-qa/opencode-qa
  (self-QA with mock model + isolated CODEX_HOME), work-with-pr-workspace ships
  evals/ alongside skill.
- KEY design patterns to copy: commands = thin skill aliases (skill(name=...) +
  $ARGUMENTS, argument-hint frontmatter); description-embedded Triggers lists +
  negative scoping (DO NOT USE FOR / NEVER); hard preconditions w/ fail-fast user
  guidance; aggregator squads (audit-consolidated pattern); durable squad runs on
  disk (.audit/in-progress/{run-id}/squads/{name}.json) w/ chunked atomic writes +
  checkpoint/resume; announcement ritual ("HYPERPLAN MODE ENABLED!"); category-
  based delegation (role+model-class, not hardcoded models); evals shipped with
  skills; skill-to-skill hand-off contracts.

**User's own skills (domain-pack evidence):**
- Pack A AUDIT SQUADS (27 skills, _shared/audit-template.md v3.2): engineering
  quality (security, dependencies, license, code-quality, refactoring,
  test-coverage, architecture, fault-tolerance, observability), infra (cicd,
  test-meta), business/financial (financial-model, tax-fiscal, acquisition-metrics,
  ai-cost, competitive-intel, product-roadmap), regulatory (dataprotection LGPD/
  GDPR, financial CVM/CSSF, sector ANAC/ANVISA/ANEEL, standards ISO/SOC2/PCI,
  contractual), meta (consolidated aggregator, consistency-coherence,
  schema-metrics, lmas-traceability). Template mandates: name prefix audit- +
  kebab; language-agnostic description; languages[]; version; findings schema
  (root_cause, impact, effort_estimate, confidence, verification_command,
  fp_risk_self_assessment + trending fingerprint/first_seen/status); delta
  tracking + baselines + SARIF; disk persistence + resume.
- Pack B .NET ENGINEERING (11-12): dotnet-clean-arch, aspnetcore-health,
  postgresql-efcore, keycloak-oidc, signalr-realtime, distributed-caching,
  quartz-scheduling, http-error-handling, blazor-mudblazor, docker-deployment,
  lgpd-data-privacy, ui-ux-accessibility. Uniform frontmatter w/ USE FOR /
  DO NOT USE FOR, license MIT, metadata block.
- Pack C DATACENTER/AEC (8, NO frontmatter — migration target): bim-authoring,
  civil-, structural-, electrical-, thermal-engineering, engineering-compliance,
  itu-fiber-validator (+flesch-kincaid writing tool).
- Pack D CAVEMAN token-compression (5): caveman, -commit, -review, -compress,
  -help (~75% token cut modes).
- Pack E MICROSOFT-FOUNDRY vendor pack: hierarchical nested skill tree w/
  namespaced names (microsoft-foundry:resource/create), allowed-tools
  frontmatter, intent-routing hand-offs between sub-skills.
- Meta: find-skills (discovery), markitdown (doc MCP).
- IMPLICATION for base domain packs (Q7): user's demonstrated domains = software
  engineering (.NET-heavy), audit/regulatory/tax/financial, datacenter/AEC
  engineering, AI deployment, writing/readability — align shipped pack selection
  with legal/maths/accounting/investors/programming answer PLUS these.

## Decisions (with rationale)

| # | decision | rationale / source |
| --- | --- | --- |
| D1 | Product shape: core library crate + **four frontends — CLI, full TUI (ratatui/crossterm), Web, VSCode extension** — all attaching to ONE headless daemon (Axum WS/SSE + IPC). Other IDEs (JetBrains etc.) deferred, architecture must not preclude them. User also asked for productivity particularities (recorded below). | interview Q2 + paste §7 + **user amendment 2026-09-11 (m0042)** |
| D2 | Providers: big set + local — Anthropic, OpenAI, Google, xAI + OpenAI-compatible endpoints (Ollama/vLLM/OpenRouter/Groq) | interview Q3 |
| D3 | MCP: client **and** server mode | interview Q4 |
| D4 | Runtime extensions: **MCP is THE runtime path** (+ Rust traits compile-time; WASM per paste report §5) | interview Q5 + paste |
| D5 | Safety: layered — permission prompts + OS-native sandboxing + container backend | interview Q6 |
| D6 | Domains: core + **community registry** + **shipped base domain packs** (legal, maths, accountants, investors/finance, programming w/ frontend/backend/devops branches, + more base subjects as base cases) | interview Q7 |
| D7 | Tri-platform parity (Win/mac/Linux) + cargo-dist releases; **MIT OR Apache-2.0** dual license | interview Q8 |
| D8 | Sessions: user defers design to plan — REQUIRES cryptography consideration ("secure sessions"), and lazy loading (worried: "context bloated if we load the entire JSONL"). Fallback if nothing better: JSONL + index | interview Q9 → design task for plan |
| D9 | Name: suggest later; working title = repo dir | interview Q10 |
| D10 | TDD: hard requirement (user goal) — tests-first, scripted mock provider, deterministic loops | user goal |

## Scope IN

- D1–D10 (see Decisions): core lib + CLI + full TUI + daemon; big provider set +
  local; MCP client+server; MCP-as-runtime-path extensions; layered safety;
  registry + base domain packs; tri-platform + cargo-dist + MIT OR Apache-2.0;
  encrypted lazy-loaded sessions; TDD.
- Cross-harness lessons ENCODED AS REQUIREMENTS: named-operation loop state
  machine; per-tool inspection chain w/ RepetitionInspector + stuck detection;
  compaction preserving decisions+bugs+recent files; tool-result clearing;
  subagent 1–2k-token summaries; deferred tool loading (6 invariant tools);
  byte-stable prefix-cached prompt layout; PTY screen-delta scraping; tiered
  eviction @75%; transactional shadows + pre-flight; snapshot namespaces +
  /rewind; semantic hunks; doom-loop guard; single volatile error slot; cost +
  cache-hit HUD; model-identity transparency (no silent substitution);
  session fork/resume; eval-harness adapter (--output-json); conformance CI
  patterns (expected-failures YAML; live-model evals NOT PR gates); scripted
  mock-provider + SSE-DSL + cassette test infra; hermetic tests.
- Memory discipline: lazy session loading; memory budgets; no full-file parses.

## Scope OUT (Must NOT have)

- NO vector DB / embeddings for code retrieval (Tantivy BM25 + tree-sitter only).
- NO whole-file rewrites or full-file echo in edit tools.
- NO stacked compiler-error dumps (single volatile error slot replaces).
- NO model-as-linter (deterministic local checks only).
- NO unbounded subprocesses (timeout ceilings + Job-Object kill trees).
- NO unstable JSON key ordering or cache-breaking serialization anywhere.
- NO dirty-state retention after failed multi-file edits (rollback to snapshot).
- NO tool-schema churn between modes (deferred tool_search loading instead).
- NO embedded local inference in core (mistralrs = optional sidecar process).
- NO V8/deno_core scripting runtime in v1; NO dylib plugins (abi_stable dead).
- NO auto-execution of unverified external plugins (pin + hash + trust review).
- NO telemetry default-on (opt-in only).
- NO silent model substitution ever.
- NO Windows-as-afterthought (Windows parity is a release gate, incl. ConPTY
  pathologies + restricted-token sandbox).
- NO scope reduction to "MVP": the user explicitly wants the complete one-shot.

## Open questions

- Q1–Q8, Q10 RESOLVED via interview (see Decisions). Q9 = design task for the plan
  (encryption-at-rest / secure sessions + lazy/paginated session loading; fallback JSONL+index).
- NEW (user msg 2, in flight): plugin-ecosystem latent-need research; local Oh My
  Openagent + user-skills inspection; digest of Portuguese-report Section 9+ tail.
- No blocking questions remain; synthesis pending research returns.

## Approval gate
status: awaiting-approval
<!-- Exploration exhausted 2026-09-11: interview complete (D1–D10), user paste
     fully digested (incl. Portuguese final report §1–§10), 6 librarians + 3
     explorers returned and recorded above. Next action after okay: write
     .omo/plans/rust-agent-harness.md replicating buildPlanSkeleton() headers,
     APPEND task batches under ## Todos (grammar '- [ ] N. title' / F1–F4),
     fill TL;DR LAST, run structural self-check. Review NOT required (CLEAR,
     no accuracy modifier) — offer high-accuracy review option at handoff. -->
approach-confirmed: 22-component workspace per topology ledger; waves: scaffold
→ core+context+providers+tools (TDD) → exec/vfs/snapshots/sessions/safety →
mcp/skills/packs/registry/search/lsp → tui/daemon/cli → wasm-ext + domain-pack
content + evals; final verification wave F1–F4.
