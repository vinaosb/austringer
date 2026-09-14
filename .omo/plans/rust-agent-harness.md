# rust-agent-harness - Work Plan

## TL;DR (For humans)

**What you'll get:** A complete, fast, general-purpose AI assistant that runs on your machine as a single small program — usable from the terminal, a rich terminal UI, a browser dashboard, and VSCode — that learns from how you work (writing its own skills), can split a big written specification across several AI models in parallel, and never wastes money or memory: it shows live cost, compresses its own conversation intelligently, and undoes any mistake instantly.

**Why this approach:** Every design choice is borrowed from what succeeded — and explicitly avoids what failed — across 20+ studied harnesses (Claude Code, Codex, goose, OpenHands, SWE-agent, opencode, crush, aider, Gemini CLI…), plus your own pasted blueprint and your Oh-My-Openagent/skills experience. One core engine, four attached frontends, industry-standard extension plugs (MCP + Agent Skills), and hard performance budgets enforced by automated tests.

**What it will NOT do:** No embedding/vector search for code, no whole-file rewrites, no silently switching models on you, no always-on telemetry, no unverified third-party plugins running automatically, no Windows second-class support.

**Effort:** XL
**Risk:** Medium — breadth is large, but every component has a proven public precedent recorded in the research file.

**Decisions to sanity-check:** (1) SQLite as session source-of-truth with encrypted-at-rest option and lazy loading (your Q9 concern); (2) WASM plugin runtime scheduled LAST (your paste specifies it; your interview answer deferred it — it stays, but late); (3) shipped domain-pack list is seeded from your own skill library plus the legal/maths/accounting/investors answer.

Your next move: run `/start-work` (or keep amending; research loop continues in parallel). Full execution detail follows below.

---

> TL;DR (machine): XL effort, medium risk; 29-component Rust workspace delivering multi-frontend (CLI/TUI/Web/VSCode) agent harness with learning skills, multi-model orchestration, hard perf budgets; TDD throughout.

## Scope
### Must have
Everything recorded in `.omo/drafts/rust-agent-harness.md` — Decisions D1–D10; amendments R/S/T/U (skill forge, spec launcher, model profiles, perf engineering); the 17 additional improvements (A–Q); the 14 productivity particularities; the 22+7-row components ledger; and all context-economics mechanics from the user's pasted blueprint (§1–§10). Reference of record: the draft + `.omo/research/harness-research.md`.
### Must NOT have (guardrails, anti-slop, scope boundaries)
The 15 Scope-OUT guardrails in the draft (no vector DB for code; no whole-file rewrites; no stacked error dumps; no model-as-linter; no unbounded subprocesses; no unstable serialization; no dirty-state retention; no tool-schema churn; no embedded local inference; no V8/dylib plugins; no auto-run unverified plugins; no telemetry default-on; no silent model substitution; no Windows-afterthought; no MVP-reduction).

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- Test decision: **TDD** (strict red-green per component) + cargo-nextest (archive CI matrix), insta snapshots, assert_cmd; scripted mock-provider crate (rule-DSL fake + SSE event-builder DSL + error injection incl. mid-stream drops) + cassette recorder (strict replay); deterministic loop tests via ResponseGate; tokio start_paused for timers; coverage via cargo-llvm-cov (target ≥80% core crates).
- Performance gates: criterion + dhat in CI; fail on >10% regression vs baseline; budgets: CLI cold start <50 ms, TUI idle CPU 0.0%, daemon RSS <40 MB baseline (dsh achieves 35–40 MB in TypeScript — Rust must beat it; research §6), repomap incremental <10 ms, p99 render <16 ms, binary <40 MB stripped.
- Evals: live-model conformance sweeps run SCHEDULED, never as PR gates (goose policy); expected-failures YAML pins known failures; skill A/B mini-benchmarks via evals adapter.
- Evidence: `<attemptDir>/task-<N>-rust-agent-harness.<ext>` (attemptDir = currentAttemptDir from `omo ulw-loop status --json`, i.e. `.omo/evidence/ulw/<session>/<goalId>/a<attempt>`; outside ulw-loop use `.omo/evidence/`).

## Execution strategy
### Parallel execution waves
> 10 waves, 4–5 todos each. Waves 1–5 build the engine bottom-up (TDD); waves 6–8 build surfaces; waves 9–10 close the intelligence and performance loops.

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1–3 (Wave 0) | — | all | each other |
| 4–8 (W1) | 1–3 | W2–W10 | each other |
| 9–13 (W2) | 4–8 | W3–W10 | each other |
| 14–18 (W3) | 6 | W4, W7 | each other |
| 19–25 (W4) | 8 | W5–W10 | each other |
| 26–28 (W5) | 19–20 | W6–W10 | each other |
| 29–34 (W6) | 5,9,30 | W7–W10 | each other |
| 35–39 (W7) | 4,24,29 | W8–W10 | each other |
| 40–42 (W8) | 29–34 | W9–W10 | each other |
| 43–45 (W9) | all prior | F-wave | each other |
| 46 (W9 companion) | 1 | F | any |

## Todos
> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->

- [x] 1. Workspace scaffold + release profile + lint gates
  What to do / Must NOT do: create cargo workspace with crate skeletons (`harness-core`, `harness-providers`, `harness-ast`, `harness-repomap`, `harness-tools`, `harness-exec`, `harness-vfs`, `harness-snapshots`, `harness-sessions`, `harness-safety`, `harness-mcp`, `harness-skills`, `harness-lsp`, `harness-search`, `harness-daemon`, `harness-tui`, `harness-web` (static assets), `harness-vscode` (TS project), `harness-cli`, `harness-test-support`, `harness-bench`, `harness-evals`); rustfmt.toml, clippy `-D warnings`, cargo-deny (licenses/advisories), release profile (lto=fat, codegen-units=1, opt-level=3, strip=true), placeholder README + LICENSE-MIT + LICENSE-APACHE + SECURITY.md (private reporting channel — dsh's missing-SECURITY.md lesson, research §6); full positioning README + community surface = todo 46. Must NOT add business logic.
  Parallelization: Wave 0 | Blocked by: — | Blocks: all
  References (executor has NO interview context - be exhaustive): draft `.omo/drafts/rust-agent-harness.md` §Components; research `.omo/research/harness-research.md` §decisions/crates
  Acceptance criteria (agent-executable): `cargo build --release && cargo clippy -- -D warnings && cargo deny check` all green on Windows/macOS/Linux CI
  QA scenarios (name the exact tool + invocation): happy: `cargo nextest run` on empty test; failure: introduce clippy lint → CI fails. Evidence `.omo/evidence/task-1-rust-agent-harness.txt`
  Commit: Y | chore(workspace): scaffold 22-crate workspace with lint/release gates

- [x] 2. Test-support crate: mock provider + SSE DSL + cassettes
  What to do / Must NOT do: `harness-test-support` with (a) rule-based fake provider (goose DummyApi pattern: `api.on(matcher).reply(...)`; variants NoChoices/OutputLimit/ContextLimitError/ServerError/ReplyThenServerError); (b) SSE event-builder DSL (codex responses.rs pattern: ev_* constructors, sequencing, `up_to_n_times`, delays); (c) mid-stream drop + idle-timeout simulation (mockito `with_chunked_body` + sleeps; wiremock close-after-N); (d) ResponseGate (Condvar) for race/cancel tests; (e) cassette recorder with strict replay (die on unused interactions) + redaction; (f) hermetic defaults (no network in tests, ever). Must NOT depend on live APIs.
  Parallelization: Wave 0 | Blocked by: 1 | Blocks: 4–45
  References: draft §librarian-5 (Codex/goose/opencode test patterns); research file §testing
  Acceptance: dedicated self-test suite proving each injection mode fires (incl. mid-stream drop leaves partial event stream)
  QA: happy: full scripted conversation replays; failure: corrupted cassette aborts with clear error. Evidence task-2
  Commit: Y | feat(test-support): mock provider, SSE DSL, cassettes, race gates

- [ ] 3. CI pipelines + nextest archive matrix + coverage
  What to do / Must NOT do: GH Actions: fmt+clippy+deny; `cargo nextest archive` per OS (win/mac/linux) then run-from-archive; cargo-llvm-cov coverage upload; cargo-dist release workflow + MULTI-ARCH CONTAINER IMAGE publish on release tags (linux/amd64+arm64, daemon+CLI) — the CI-runner consumption path (gh-aw/Copilot-Actions pattern, research §20); perf-gate placeholder job. Must NOT include live-model jobs in PR path. PROCESS NOTE: any future vendored-upstream code (if we ever track an upstream harness) gets annotated-change markers + a CI duplication gate (Kilo kilocode_change discipline, research §11).
  Parallelization: Wave 0 | Blocked by: 1 | Blocks: 45
  References: draft §librarian-5 (codex nextest-archive CI); §Verification
  Acceptance: green matrix run on all three OS; artifact uploaded
  QA: happy: push triggers full matrix; failure: broken test blocks merge. Evidence task-3
  Commit: Y | ci: nextest-archive matrix, coverage, dist, perf-gate stub

- [x] 4. Event model + append-only session log
  What to do / Must NOT do: typed event enum (user/assistant/tool-call/tool-result/system/condensation/snapshot markers — LLM-visible vs internal separation per OpenHands V1); MULTIMODAL CONTENT BLOCKS from turn one: user + tool-result events carry image blocks (mime, dims, out-of-line storage under <session>/media/ w/ hash refs — NEVER inline base64 in the JSONL); vision-capable models receive them, text-only models get typed placeholders; serde-stable serialization (sorted keys, forward-slash paths — prefix-cache rule applies to logs too); append-only JSONL writer w/ fsync policy + DISK-FULL/IO-ERROR CONTRACT (write failure degrades to read-only safe mode + loud event — never corruption, never silent loss). Must NOT mutate written events.
  Parallelization: Wave 1 | Blocked by: 2 | Blocks: 5–8, 24
  References: draft §librarian-2 (OpenHands V1); §paste §3 serialization rules
  Acceptance: fuzz round-trip serde; append under crash (kill -9) leaves valid prefix
  QA: happy: 10k-event session replays; failure: torn final line ignored w/ warning. Evidence task-4
  Commit: Y | feat(core): typed append-only event log

- [x] 5. Agent loop state machine (named operations)
  What to do / Must NOT do: Operation pipeline exactly: EntryHook→SlashCommand→Steer→MaxTurns→Compaction→ToolPairCompaction→ToolApproval→ToolExecution→Retry→StopHook(w/ block cap)→ExitOnError→Inference; cancellable mid-operation w/ cancel-mark/high-water sequences (crush lesson: never poison later prompts); FOCUS-CHAIN anchor: model-maintained todo list re-injected every turn as a stable environment block + completion BLOCKED while todos open (Roo's preventCompletionWithOpenTodos — anti-drift); two-level completion + plan.md hard-block landed; GRACEFUL-CANCEL SEMANTICS (Warp, research §25): session status exposes InProgress/Success/Failed/Blocked/Cancelled w/ a short cancel-grace window before terminal status. Must NOT use unstructured loop.
  Parallelization: Wave 1 | Blocked by: 4 | Blocks: 6–8, 29–45
  References: draft §librarian-2 goose agent.rs; §components/core
  Acceptance: lifecycle tests for every operation transition + cancel-during-each
  QA: happy: scripted 3-turn convo; failure: cancel mid-tool leaves no half-state. Evidence task-5
  Commit: Y | feat(core): operation-pipeline agent loop

- [x] 6. Tool registry + 6 invariant tools + deferred loading
  What to do / Must NOT do: ToolProvider trait; registry serializes ONLY 6 core tools (view_window, edit_lines, execute_command, lsp_query, tool_search, task_completed) + markdown aux-table; `tool_search(query)` hydrates full schema into observation (never touches system prefix); TOOL BEHAVIOR VERSION REGISTRY: versioned tool contracts (current + legacy presets, Active/Deprecated/RemovalCandidate lifecycle — Grok Build pattern, research §15: kills tool-schema churn breaking older sessions); TOOL-I/O BYTE CAPS w/ transparent disk spill for oversized results (pointer + first N bytes in context). Must NOT vary tool-schema set between modes.
  Parallelization: Wave 1 | Blocked by: 5 | Blocks: 14–18, 29
  References: draft §paste §3 deferred tool loading
  Acceptance: byte-identical system prompt across turns w/ different active tools (snapshot test)
  QA: happy: hydrate→call roundtrip; failure: unknown tool → graceful tool_search suggestion. Evidence task-6
  Commit: Y | feat(core): invariant tool set + schema hydration

- [x] 7. Inspector chain (permissions, repetition, optional LLM reviewer)
  What to do / Must NOT do: per-tool-call chain PermissionInspector (allow/ask/deny patterns incl. globs, session/forever memory) + RepetitionInspector (identical-call window detection) + optional SecurityInspector (LLM risky-call review — off by default); SECRET-FILE READ GUARD: reads of high-risk paths (.env*, *id_rsa*, *.pem, *.p12, credentials, .npmrc, .netrc, keychain files — config-extensible) default to ASK + REDACT-IN-TRANSCRIPT (values masked in logs/compaction summaries; the model sees contents only after approval) — env-var scrub (todo 26) covers child processes, this covers the read path into context; gh-aw SAFE-OUTPUTS pattern (Copilot, research §20): read-only result tokens by default + one validated write channel + threat-scanning on outputs before they leave the tool boundary. Must NOT execute tools before chain passes.
  Parallelization: Wave 1 | Blocked by: 5 | Blocks: 26, 40
  References: draft §librarian-2 goose inspectors; §D5
  Acceptance: pattern-matching tests; repetition detector trips at N=3 identical calls
  QA: happy: allow-rule short-circuits; failure: deny returns model-feedback. Evidence task-7
  Commit: Y | feat(core): tool inspector chain

- [x] 8. Autonomy modes + permission store
  What to do / Must NOT do: modes Auto/Approve/Chat/SmartApprove exposed as an AUTONOMY-MATRIX config (levels × per-capability toggles: auto-read, auto-edit, auto-exec, auto-compact, auto-context — Plandex's 5-level × 10-toggle pattern, richer than a bare dial); /mode pairs plan-mode w/ read-only tool set; permission store (sqlite) w/ y/a/n semantics, `a` scoped pattern+duration; AUTO MODE 3-LAYER CLASSIFIER (Qwen, research §18): fast-path exceptions → safe-tool allowlist → two-stage LLM classifier w/ allow-stripping while active + softDeny-on-exact-intent + loop guard; NL hints (natural-language allow/deny phrases) as user-friendly permission customization; headless default = Ask; ARTIFACT-GATED APPROVAL (Antigravity, research §21): plan/artifact review policies (Request-Review vs Always-Proceed) as an autonomy-mode dimension. Must NOT persist bypassPermissions.
  Parallelization: Wave 1 | Blocked by: 7 | Blocks: 26
  References: draft §librarian-2 goose modes; §productivity #7
  Acceptance: mode-switch tests change approval behavior w/o schema churn
  QA: happy: SmartApprove approves benign; failure: destructive always asks. Evidence task-8
  Commit: Y | feat(core): autonomy dial + permission persistence

- [x] 9. Provider trait + streaming + mid-stream recovery
  What to do / Must NOT do: dyn-safe `#[async_trait] Provider` (goose pattern); stream events; retry w/ jitter; MID-STREAM RECOVERY: resume after disconnect preserving turn state, idle-timeout, no-duplicate-side-effects (Codex #8865/#19121/#927 lessons). Must NOT silently substitute models (identity contract; fallback requires confirmation).
  Parallelization: Wave 2 | Blocked by: 2,5 | Blocks: 10–13, 34, 40–42
  References: draft §D2; §librarian-5 failure lessons; research §providers
  Acceptance: chaos tests (drop at byte K of stream) recover exactly-once
  QA: happy: 200-event stream completes; failure: drop mid-args → resumed turn completes tool once. Evidence task-9
  Commit: Y | feat(providers): resilient streaming provider abstraction

- [x] 10. Adapters: Anthropic, OpenAI, Google, xAI, OpenAI-compatible
  What to do / Must NOT do: five adapters over the trait; OAUTH/SUBSCRIPTION AUTH: device-flow login where providers offer subscription auth (Anthropic/OpenAI/Google/xAI), refresh-token lifecycle via OS keyring, subscription-vs-API billing surfaced to the cost HUD — API-key-only is day-one friction (Claude Code/Copilot/Gemini CLI precedent); cost tables + per-response token accounting (input/output/cache-read/cache-write) w/ UNKNOWN-PRICING FALLBACK (models outside the cost table — arbitrary OpenAI-compat endpoints — report tokens-only + 'unpriced' HUD state, never invented costs; user-supplied price overrides supported); OpenAI-compatible base-URL adapter covers Ollama/vLLM/OpenRouter/Groq. TDD against cassettes per provider. Must NOT parse beyond stable API surfaces w/o feature flag.
  Parallelization: Wave 2 | Blocked by: 9 | Blocks: 11, 34, 42
  References: draft §D2; §librarian-3 (rig/genai/async-openai verdicts)
  Acceptance: cassette-driven tests per adapter incl. 429/5xx/timeouts/malformed SSE
  QA: happy: streaming completion w/ tool-call; failure: malformed SSE skips bad event, logs, continues. Evidence task-10
  Commit: Y | feat(providers): five adapters + cost accounting

- [x] 11. Prompt Idempotency Manager (cache layout)
  What to do / Must NOT do: stable serialization (sorted keys, `/` paths, no timestamps); layout [0,L0]=system+6 tools, [L0,L1]=rules+skills schemas, [L1,L2]=repomap+history, volatile in last user msg; provider cache-control markers (Anthropic 4-marker, OpenAI automatic); CACHE-CONTROL ENGINEERING (hermes, research §27): explicit breakpoint placement w/ 5m/1h TTL spans, provider-specific relocation (OpenRouter part-level), and post-turn background tasks (review/curator) deliberately composed to SHARE the prefix cache; VOCABULARY: treat the stable span as a Context EPOCH — a named, immutable provider-cache baseline that ends only at completed compaction or session movement; durable mid-conversation updates are appended IN-HISTORY (never mutate the system prompt) (Kilo CONTEXT.md terms, research §11); CACHE-VIOLATION TRACKER (jcode, research §32): an append-only stable-prefix-hash chain detects provider-side cache invalidation the moment it happens (rolling hash comparison per turn) — cache misses become observable events, not silent cost spikes; HOLD-EFFORT-CONSTANT DISCIPLINE (research §33): on cache-coupled providers (Anthropic renders effort+budget INTO the prompt), changing either invalidates every cache breakpoint — effort changes go through the per-message beta channel or wait for a compaction boundary. Must NOT reorder history or inject volatile data above L2.
  Parallelization: Wave 2 | Blocked by: 6,9 | Blocks: 12–13, 45
  References: draft §paste §3 (cache mechanics table)
  Acceptance: property test — same logical state ⇒ byte-identical prefix; mutation only allowed in last message
  QA: happy: 20-turn session keeps ≥90% prefix identity; failure: injected timestamp detected + rejected by builder. Evidence task-11
  Commit: Y | feat(context): prefix-cache-stable prompt builder

- [x] 12. Tiered eviction + dedup folding + error slot
  What to do / Must NOT do: trigger @75% window; Tier0 immutable; Tier1 condensation → `<active_plan>` via cheap model; Tier2 LRU→Blake3 pointers + return codes; tool-result clearing; `<folded_tool_output>` markers; SINGLE volatile error slot (replace, never stack); PRUNING LADDER for stale tool results (soft-trim to head/tail excerpts → hard-clear to placeholder w/ retrieval pointer — Grok [compaction.pruning] pattern) + PRE-COMPACT MEMORY FLUSH trigger (flush salient facts to memory store N tokens before the compact threshold — research §15); PROTECTED HEAD/TAIL + PLUGGABLE ENGINES (hermes, research §27): first ~3 + last ~20 messages eviction-immune w/ aux-model middle condensation; compaction engines pluggable behind one trait; LADDER VOCABULARY (Gemini CLI, research §31): masking → distillation → compression → truncation as named, ordered stages — each stage's trigger + budget explicit; TWO-TIER CALL GOVERNORS (research §33): soft per-class call budget injected in context + hard budget denying further calls w/ a structured error — correct budgets vary ~10× across task classes, so they key on the task classifier, never one global cap. Must NOT evict Tier0 or the active plan.
  Parallelization: Wave 2 | Blocked by: 11 | Blocks: 13, 40
  References: draft §paste §3 eviction/folding
  Acceptance: deterministic eviction sim across synthetic 200-turn sessions
  QA: happy: budget met w/ plan preserved; failure: folding never drops tool return codes. Evidence task-12
  Commit: Y | feat(context): tiered eviction + folding + single error slot

- [x] 13. Compaction engine
  What to do / Must NOT do: preserve architectural decisions, unresolved bugs, ~5 recent files + last ~10 turns (Anthropic doctrine); subagent summaries capped 1–2k tokens; hooks PreCompact/PostCompact events; OVERFLOW-RECOVERY RULE: a deterministic basic-compaction fallback must exist for when the provider rejects over-long context — recovery must NEVER depend on another successful LLM call (Cline lesson, research §9); persisted compaction state validated against a prefix hash (Cline pattern); SAME-MODEL condensing by design (condense with the session's own model — avoid dialect mismatch, Roo lesson); user-configurable threshold + custom condense prompt + manual /condense button; EXTERNALIZED SEGMENT STORE: compaction renders recoverable segment files + INDEX per session (detail level configurable) so compaction output is auditable + re-loadable — Grok compaction-transcript pattern, research §15; HANDOFF complement (Amp, research §17): model-distilled transfer of built-up context into a fresh session for long-horizon work + read_thread-style second-model extraction from referenced sessions — complements compaction, never replaces it; TEAMWORK SUCCESSOR-HANDOFF (Antigravity, research §21): at milestone boundaries spin a FRESH successor session w/ distilled context instead of compaction-aging one long context — context-degradation defense; COMPACTION TELEMETRY w/ outcomes (manual/auto/canceled/failed + threshold basis %|tokens-used|remaining — Zed, research §23); POSTCOMPACT ACCOUNTABILITY (forge, research §26): before/after token counts + seal-integrity status recorded per compaction — a breached seal (edits above the compaction watermark) is a FIRST-CLASS pageable session event, never silently absorbed. Must NOT naive-truncate.
  Parallelization: Wave 2 | Blocked by: 12 | Blocks: 40
  References: draft §librarian-1 Anthropic doctrine; §paste
  Acceptance: replay-equivalence test — compacted session continues task w/o re-asking known facts (scripted)
  QA: happy: compact at 75%; failure: compaction error → session read-only safe mode. Evidence task-13
  Commit: Y | feat(context): doctrine-compliant compaction

- [x] 14. harness-ast: tag tables + validation gates
  What to do / Must NOT do: tree-sitter grammars (feature-gated per language); tags.scm definition/reference queries; persistent tables keyed Blake3+mtime; `is_error()/is_missing()` validation API. Must NOT block on missing grammar (degrade to text tools).
  Parallelization: Wave 3 | Blocked by: 6 | Blocks: 15–16, 22
  References: draft §paste §3; research §crates
  Acceptance: incremental re-parse <10 ms on mtime hit (bench); validation catches unbalanced braces
  QA: happy: 50-language smoke; failure: broken syntax rejected w/ line/col. Evidence task-14
  Commit: Y | feat(ast): tree-sitter tag tables + syntax gates

- [x] 15. harness-repomap: PageRank CSR + rendering
  What to do / Must NOT do: file graph from tag tables; Personalized PageRank d=0.85 on CSR (SIMD path ≥50k nodes); personalization on focused/@mentioned files; signature-only rendering w/ ellipsis; ≤1k-token budget via binary search on density; REFS-BY-NAME (forge Lattice, research §26): cross-references stored by symbol NAME not internal node-id so the map survives reindexing without dangling edges. Must NOT ship adjacency-list graph for large repos.
  Parallelization: Wave 3 | Blocked by: 14 | Blocks: 18, 35
  References: draft §paste §3 repomap
  Acceptance: map of 10k-file synthetic repo ≤1k tokens, renders <50 ms cold
  QA: happy: focus files rank top; failure: empty repo → empty map, no panic. Evidence task-15
  Commit: Y | feat(repomap): CSR PageRank + token-budgeted rendering

- [x] 16. Editing tools: view_window/edit_lines + semantic hunks + hashline
  What to do / Must NOT do: view_window ≤100 lines 1-based w/ jump; edit_lines returns changed lines ±3 fringe; AST-validated application + uniqueness check; OPTIONAL line-hash anchors (hashline) reject-on-mismatch; semantic hunk staging via imara-diff Histogram grouped by tree-sitter entities (y/n/e per entity; finest granularity = per-REPLACEMENT walk under entity hunks — Mentat pattern); INDENTATION-ANCHOR block reads as second primitive: anchor_line → whole semantic block (Roo pattern — cheap targeted retrieval beyond line windows); definitions-only read mode (tree-sitter signatures) for cheap file orientation; OPTIONAL STRICT MODE: write authority scoped to read scope — edits to files/lines NOT in context are rejected (Mentat day-1 invariant); WIP-DIFF CONTEXT MODE: "continue my work" anchored on current diff + untracked (Mentat git-diff-anchored context); ANCHOR-SCHEME BENCHMARK HARNESS: benchmark competing hashline anchor schemes (content-hash vs chunk-fingerprint vs checkpoint-chain) on read-amplification + shifted-anchor recovery before locking the default — Grok grok_build_hashline independent validation, research §15; ANTI-ELISION GUARD (Gemini CLI omissionPlaceholderDetector, research §31): windowed reads emit explicit omission placeholders the model is trained to notice — silent elision of relevant content is a defect class w/ a detector; EDIT-FORMAT ROUTING PER FAMILY (research §33): ship the tool format each model was RL-trained on (patch-style for OpenAI-family, string-replace/anchors for Anthropic-family) + schema-similarity rule (custom edit schemas closer to the trained shape = fewer malformed calls — schema priors leak across generations). Must NOT offer whole-file rewrite; Must NOT echo full files.
  Parallelization: Wave 3 | Blocked by: 14 | Blocks: 21–23, 37–38
  References: draft §paste §3-4; §J hashline
  Acceptance: property tests — ambiguous match rejected; hunk grouping never spans entities
  QA: happy: 3-hunk edit applied selectively; failure: drifted anchor → re-anchor or reject, never corrupt. Evidence task-16
  Commit: Y | feat(tools): windowed editing + AST hunks + hash anchors

- [x] 17. harness-search: Tantivy BM25 + grep
  What to do / Must NOT do: tantivy index (respects .gitignore; incremental); grep-regex integration; results windowed + deduped. Must NOT add embeddings.
  Parallelization: Wave 3 | Blocked by: 6 | Blocks: 24, 35
  References: draft §Scope-OUT; §librarian-3
  Acceptance: index 10k files <5 s cold; query <20 ms warm
  QA: happy: symbol search ranks definitions first; failure: binary files skipped safely. Evidence task-17
  Commit: Y | feat(search): lexical index, no vectors

- [x] 18. harness-lsp: headless client
  What to do / Must NOT do: spawn/connect LSP servers (rust-analyzer, gopls, tsserver, clangd, pyright); lsp_goto_definition / lsp_find_references / lsp_hover tools returning <25-token locations; multi-server routing by file-type; RSS-SAMPLED BOUNDED LSP (forge, research §26): LSP processes are the memory hog of coding harnesses — sample RSS, cap/kill/restart runaway servers, keep the guard cheap enough to run always; IDE-AS-TOOLBOX (JetBrains, research §24): the IDE extension exposes the host's REAL semantic index/build configs/test runners/debugger over a localhost MCP endpoint instead of the harness reimplementing approximations (incl. decompiled-JAR symbols). Must NOT duplicate when IDE bridge supplies LSP (priority: bridge > own).
  Parallelization: Wave 3 | Blocked by: 6 | Blocks: 38
  References: draft §paste §5
  Acceptance: golden tests against rust-analyzer on fixture crate
  QA: happy: goto returns exact file:line:col; failure: server crash → restart w/ backoff, degrade to search. Evidence task-18
  Commit: Y | feat(lsp): headless navigation client

- [x] 19. PTY pipeline + execute_command
  What to do / Must NOT do: portable-pty → vt100 120×40; >75 lines ⇒ first 25 + last 50 + omission marker; ANSI/SGR stripped; timeout ceiling + SIGKILL/Job-Object kill TREE; DAEMON-DEATH ORPHAN REAPING: Unix children spawned w/ PR_SET_PDEATHSIG (kernel reaps them if the daemon dies — mirrors Windows Job-Object KILL_ON_JOB_CLOSE semantics) — no orphaned tool processes after a crash, ever; OPTIONAL DEFERRED BATCH-EXECUTION MODE — accumulate commands in the pending change-set, review + apply ATOMICALLY with file edits (Plandex _apply.sh pattern — alongside live exec, never replacing it); managed tool-output SPILL FILES for oversized outputs w/ locators (dsh/Kilo pattern — SPILL-NOT-TRUNCATE measured −17% cost/pass, forge research §26). Must NOT pipe raw bytes to context.
  Parallelization: Wave 4 | Blocked by: 6 | Blocks: 20, 22
  References: draft §paste §3 PTY
  Acceptance: chaos suite — progress bars, TUI apps, 100MB output → bounded tokens
  QA: happy: cargo test output bounded; failure: timeout kills tree, no orphans (ps check in test). Evidence task-19
  Commit: Y | feat(exec): screen-delta PTY + kill trees

- [x] 20. ConPTY supervisor + WSL2 bridge
  What to do / Must NOT do: fix 3 Windows pathologies (headless wait() hang via read-close+destroy; Arc shutdown assertion via RAII+Job Objects KILL_ON_JOB_CLOSE; DA1 ESC[c 60 s hang via proactive injection); WSL2 detection (binfmt_misc/wsl.exe) + `wsl.exe --exec` routing + in-memory C:\↔/mnt/c mapping; PowerShell 7 default shell on Windows; FS-CONTENTION HARDENING: retry-w/-backoff on ERROR_SHARING_VIOLATION (Defender/OneDrive/indexer locks) in every edit/apply path, long-path (\\?\) awareness + worktree path-length budget, OneDrive-synced-folder detection + warning. Must NOT require WSL for default operation.
  Parallelization: Wave 4 | Blocked by: 19 | Blocks: 27, 35–39
  References: draft §paste §6
  Acceptance: Windows CI green on all three pathology fixtures
  QA: happy: headless Windows run exits cleanly; failure: runaway child dies with parent. Evidence task-20
  Commit: Y | feat(exec): ConPTY hardening + WSL2 bridge

- [x] 21. Shadow workspaces (CoW/worktrees)
  What to do / Must NOT do: reflink-copy (FICLONE/APFS) w/ gix worktree fallback (NTFS); apply edits through shadow; instant rollback; WORKTREE GC w/ RECLAIMED-REFS: retired worktrees archived under refs/harness/reclaimed/<worktree>/<commit> for one-GC-cycle grace before deletion (Grok pattern — prevents CoW/worktree disk sprawl, research §15 412GB lesson). Must NOT touch user tree before pre-flight passes.
  Parallelization: Wave 4 | Blocked by: 16 | Blocks: 22–23
  References: draft §paste §4
  Acceptance: cross-platform apply+rollback ≤50 ms on 1k files (CoW) / documented fallback timing
  QA: happy: failed pre-flight leaves user tree untouched; failure: reflink unsupported → worktree fallback transparent. Evidence task-21
  Commit: Y | feat(vfs): transactional shadows

- [x] 22. Pre-flight pipeline
  What to do / Must NOT do: in shadow — (a) AST gate, (b) formatter (rustfmt/prettier/gofmt/black adapters), (c) compiler/static check (cargo check --message-format=json, tsc --noEmit) w/ structured error extraction (file:line+primary only); closed corrective sub-cycle MAX 2 iterations before surfacing; AGENT-TASK ENVIRONMENT BOOTSTRAP: repo-owned setup-steps file (single job, default-branch triggers) preps the task env before the agent runs (Copilot copilot-setup-steps.yml, research §20). Must NOT stack error dumps (single volatile slot).
  Parallelization: Wave 4 | Blocked by: 14,19,21 | Blocks: 23
  References: draft §paste §4 pre-flight
  Acceptance: fixture repo — injected syntax error caught at (a), never reaches compiler
  QA: happy: 2-iteration fix loop then success; failure: persists → compact rejection signal to model. Evidence task-22
  Commit: Y | feat(exec): deterministic pre-flight

- [x] 23. Snapshots + /rewind
  What to do / Must NOT do: gix in-memory tree/commit objects → refs/harness/snapshots/{session}/{turn}; /rewind N restores files AND truncates event history; GC policy for old snapshots; PERF: persistent scratch GIT_INDEX_FILE per session (stat-cache avoids re-hashing unchanged untracked files; 14-day reaper — Cline pattern); SAFETY: restore TRANSACTION — stash user's dirty worktree to a private ref (refs/harness/restore-transactions/<uuid>) before restore, w/ commit-or-rollback; refuse restore when user commits exist after the snapshot (never knock them off the branch — Cline 4.1.17 lesson); HYGIENE (Roo): built-in excludes via shadow .git/info/exclude, LFS/.gitattributes awareness, disable-on-nested-repos (never touch user's .git), TWO restore modes: files-only vs files+task; COMMIT PROVENANCE (Amp, research §17): session-ID git trailer (Amp-Thread-ID pattern) + Co-authored-by attribution on agent commits + optional detached signing w/ signing keys kept OUTSIDE agent processes (Amp server-side signing service lesson); INCREMENTAL CHECKPOINT WRITES: persist only NEW transcript entries per checkpoint, never rewrite the whole conversation (Cursor CLI, research §19); subagents resume from their last checkpoint after failure; FILE-ONLY /rewind restore mode that SKIPS externally-modified files (Copilot, research §20) + WORKFLOW-APPROVAL PROVENANCE: agent-authored commits/PRs are marked so CI can require a write-access human approval before agent workflows run (Copilot/gh-aw gate, research §20); HOST-SIDE CHECKPOINTS (Zed, research §23): the host (not the agent) records checkpoints in the session thread — native AND external/ACP agents get uniform rewind; checkpoints auto-update on repo change; restore kills live terminals for coherence; PERIODIC WORKSPACE CHECKPOINT COORDINATOR (Warp, research §25): safe-boundary-gated 5-state machine + end-of-run snapshots + file-rewind UI; NO-GIT FALLBACK: non-repo projects snapshot via the CoW shadow layer (content-addressed file snapshots under .harness/, no gix objects) — /rewind + dirty-state restore degrade gracefully, git-only features (provenance trailers, WIP-diff context) disabled w/ capability flags surfaced to the model; SUBMODULE DISCIPLINE: snapshots/rewind/shadows treat submodules as FROZEN POINTERS (superproject commit only, never recurse into submodule history); dirty-submodule state preserved as pointer+patch; nested-repo disable rule (Roo hygiene) respected. Must NOT pollute user branches/log; Must NEVER rename/mutate the user's .git directory itself (Cline .git_disabled catastrophe, research §9).
  Parallelization: Wave 4 | Blocked by: 21 | Blocks: 24, 35–38
  References: draft §paste §4 checkpoints
  Acceptance: rewind round-trip test — state@N identical after wander to N+5
  QA: happy: rewind restores file+context in sync; failure: missing ref → clear error, no partial state. Evidence task-23
  Commit: Y | feat(snapshots): namespace checkpoints + time travel

- [x] 24. Sessions store: SQLite + encryption + lazy load + memory
  What to do / Must NOT do: sqlx SQLite source-of-truth (schema versioned, atomic updates — goose pattern) + JSONL export; encryption-at-rest XChaCha20-Poly1305 per-session keys (argon2 from passphrase, keyring optional); index-based LAZY turn loading (never full-file parse — user's bloat concern); fork/resume; SESSION RETENTION POLICY: configurable archive/GC tiers (age + size, compact-then-archive cold storage, pinned sessions exempt) + /sessions prune command — never auto-deletes without an explicit user policy; cross-session BM25 search + curated MEMORY.md-style notes (/memory, /recall) injected Tier-1; MIGRATION DISCIPLINE: round-trip migration tests in BOTH directions for every schema generation, no no-downgrade formats ever (dsh v3 lesson: 4 format generations in a month, migration-bug storm, research §6); MEMORY RETRIEVAL: hybrid FTS5+vec0 (sqlite-vec) w/ ~0.7/0.3 vector/BM25 blend + temporal decay + MMR re-rank + first-turn injection w/ min-score gate + POST-COMPACTION re-search + gated /dream consolidation (min-hours + min-sessions gates — Grok memory pattern, research §15); FOREIGN-SESSION IMPORT: pickup/resume of Claude Code + Codex sessions (Grok foreign-sessions — day-one adoption-friction killer); FAIL-CLOSED WRITER LEASES for concurrent session access (Qwen daemon, research §18): leases w/ boot-id liveness proofs, no force-unlock — crashed writers detected via PID/boot-id, never manually overridden; SESSION-STATE DIRECTORY CONTRACT (Copilot, research §20): {session}/checkpoints/*.json + plan.md + files/ + events.jsonl; API keys NEVER persisted in the session store — BYOK re-supplied on resume; OAuth refresh tokens live ONLY in the OS keyring (todo 10 subscription auth). Must NOT deserialize whole sessions on open.
  Parallelization: Wave 4 | Blocked by: 4 | Blocks: 29, 35–39, 41
  References: draft §D8; §librarian-2; §A memory
  Acceptance: 100 MB session opens <50 ms; page-in <5 ms/100 turns; encrypted file unreadable w/o key (test)
  QA: happy: resume mid-conversation; failure: wrong key → clean decrypt error. Evidence task-24
  Commit: Y | feat(sessions): encrypted lazy store + memory subsystem

- [x] 25. Budget caps
  What to do / Must NOT do: $ + token ceilings per task/session/day (config + HUD data); warn at 80%, hard-stop at 100% w/ graceful turn ending; INCOMPLETE-STATUS RESUME (Antigravity, research §21): hitting a token budget ends w/ status:incomplete that resumes exactly where it stopped — never a hard failure; CREDIT ROLLOVER THE AGENT GRAPH (Warp, research §25): cost accounting rolls up child-agent spend through the orchestration tree — per-subagent cost visible at the parent; QUOTA-AS-CURRENCY (forge, research §26): budgets expressed as paced quotas (pool capacity, burn-rate weights, conservation windows that pause expensive tiers while cheap ones are healthy) — not just $ ceilings; TOKEN-BUDGET GOVERNOR PRIMARY (research §33): token budget is the dominant outcome variable (80% of outcome variance in multi-agent research) — the auto-degrade ladder (collapse fan-out → serialize → solo) keys on token budget BEFORE per-call caps. Must NOT block mid-tool (finish tool, then stop).
  Parallelization: Wave 4 | Blocked by: 10 | Blocks: 40–42
  References: draft §C; §librarian-2 OpenHands budgets
  Acceptance: simulated cost stream triggers thresholds exactly
  QA: happy: warn+stop fire; failure: ceiling=0 refuses inference w/ message. Evidence task-25
  Commit: Y | feat(core): budget ceilings

- [x] 26. OS-native sandbox backends
  What to do / Must NOT do: Seatbelt (macOS), Landlock+seccomp OR bubblewrap (Linux, detect), Windows restricted tokens + ACLs + Job Objects; container backend (Docker/Podman) optional feature; protected paths (.git/.harness/.agents) + network egress allowlist; SESSION-PINNED PROFILES: profile fixed for session lifetime + restored on resume (Grok); custom kernel-enforced DENY-GLOBS must FAIL CLOSED (unresolvable glob = refuse, never allow); SHELL-ENV SECRET SCRUB: drop *KEY*/*SECRET*/*TOKEN* vars from child envs by default (inherit-core policy); FAIL-CLOSED POSTURE (Droid): whole-process mode refuses to start when isolation cannot be established; SETTINGS-FILE WRITE PROTECTION (sandbox guards its own config) + interactive sandbox.violations prompt (allow-once/always/deny); NETWORK FILTER-PROXY fallback: userland HTTP/SOCKS allowlist proxy where the kernel lacks net isolation — ZERO built-in vendor-escape domains (Droid AVOID: Factory+WorkOS always-allowed inside their proxy, research §16); CREDENTIAL HYGIENE (Amp, research §17): per-command SHORT-LIVED tokens via credential-helper indirection — no long-lived provider tokens in the agent environment, nothing persisted to disk; OIDC WORKLOAD-IDENTITY SOCKET (Cursor, research §19): a socket reachable only inside the sandbox mints short-lived signed JWTs for cloud/API auth — long-lived credentials never enter agent processes at all; ONE-POLICY/MANY-BACKENDS abstraction (Copilot MXC pattern, research §20): declare sandbox policy once, per-OS backends implement it; SANDBOX TIER DETAILS (Zed, research §23): worktree roots AUTO-GRANTED writable, .git metadata protected, HostPattern network policy, unsandboxed-escape requires once-per-thread approval; SUBJECT-TEMPLATE OIDC FEDERATION claims (principal/email/teams/environment/agent_name/run_id — Warp federate pattern, research §25); HONEST ISOLATION REPORTING (harness-rs, research §26): every sandboxed run reports requested-vs-enforced policy (Isolation::{None,Changes,Process}) — the gap is surfaced, never hidden behind a green checkmark; HARDENED CONTAINER BACKENDS (hermes, research §27): container execution backends ship --cap-drop ALL + no-new-privileges + pids-limit + tmpfs-noexec flags by default, and permission checks are SKIPPED when the container IS the boundary (isolation ⊇ permission there); SANDBOX EXPANSION REQUESTS (Gemini CLI, research §31): interactive per-command privilege elevation — a sandboxed run can request exactly the additional capability it needs, once, w/ user consent, instead of pre-loosening the profile. Must NOT fake sandboxing (probe tests verify enforcement).
  Parallelization: Wave 5 | Blocked by: 20 | Blocks: 45
  References: draft §D5; §librarian-1/2 sandbox findings
  Acceptance: per-OS escape-attempt test suite (write outside root, net call w/o allow) FAILS closed
  QA: happy: allowed ops transparent; failure: violation → EPERM + audit event. Evidence task-26
  Commit: Y | feat(safety): OS-native sandboxes + egress control

- [x] 27. Permission engine + protected paths integration
  What to do / Must NOT do: wire inspector chain to sandbox + egress + protected paths; audit log of every decision; COMMAND-PREFIX allow/deny lists w/ longest-prefix precedence + dangerous-substitution guard (Roo pattern: `npm i` allowed must not auto-approve `npm i && curl evil`); COMMAND BLOCKLIST w/ PROGRAM-RESOLUTION (Droid pattern, research §16): denylists are soft; a hard BLOCKLIST resolves each command to its actual program before matching — wrapper shells, absolute paths, quoting tricks, && substitution cannot bypass; max-requests circuit breaker on auto-approved loops; ADMIN FLOOR (Cursor, research §19): org/admin sandbox+permission rules that user/project config CANNOT weaken (deny-always-wins merge) + SSRF guards on egress rules (RFC1918/169.254.169.254/IPv6-ULA blocked by default); SECURITY-PROFILE INTERSECTION (Devin, research §22): profiles bind at a hierarchy (org→automation→session), lower levels only TIGHTEN, mid-session grants clamp to the effective intersection; ACTION(target) GRAMMAR (Antigravity, research §21): permissions are actions over named targets w/ Deny>Ask>Allow, prefix-default + opt-in regex; read_url vs execute_url split (fetch ≠ actuate); PERMISSIONS-DERIVED SANDBOX: allowed read_url grants compile into the container network allowlist — one config source drives both layers; TYPED PERMISSION REASONS (Warp, research §25): every allow/deny carries a machine-readable reason (explicitly-allowlisted / read-only / agent-decided / run-to-completion / contains-redaction…) surfaced in the UI — the user sees WHY an action ran, not just that it did; YOLO-SURVIVING HARDLINE FLOOR (hermes, research §27): a deobfuscated dangerous-command blocklist that persists even in yolo/bypass modes — bypass escapes approval flows, never the floor. Must NOT bypass via any tool path (fuzz).
  Parallelization: Wave 5 | Blocked by: 8,26 | Blocks: 45
  References (executor has NO interview context - be exhaustive): draft §D5; research §10 Roo auto-approve mechanics + CVE farm lesson
  Acceptance criteria (agent-executable): exhaustive tool×permission matrix test; substitution-guard test (allowed prefix + injected suffix = denied)
  QA scenarios (name the exact tool + invocation): happy/failure paths per cell; guard blocks substitution. Evidence task-27
  Commit: Y | feat(safety): unified permission engine

- [x] 28. User lifecycle hooks
  What to do / Must NOT do: SessionStart/SessionEnd/UserPromptSubmit (augment)/Stop (block-until)/Pre+PostToolUse (+Failure) w/ OUTPUT REPLACEMENT + redaction/PermissionDenied/Notification/SubagentStart+Stop/Pre+PostCompact; kinds shell/HTTP/MCP-tool; CLAUDE + CURSOR hook-format compatibility (compat-swallow, research §15); FAIL-CLOSED OPTION REQUIRED — hooks gating safety decisions must be configurable fail-closed (Grok ships fail-open = AVOID); gates may MODIFY or SYNTHESIZE tool results (Amp tool.call gate actions: allow/reject-and-continue/modify/synthesize, research §17); PROMPT-TYPE hooks: LLM-evaluated policy hooks (Cursor, research §19) + loop_limit budget for stop-hook iterate-until-goal patterns; config trust review + hash pinning (Codex model); hook timeout caps. Must NOT allow hooks to bypass permissions.
  Parallelization: Wave 5 | Blocked by: 5 | Blocks: 45
  References: draft §G; §librarian-4
  Acceptance: each event fires w/ payload snapshot; Stop-block proven in test (loop until criterion)
  QA: happy: prompt augmented by hook; failure: failing hook → logged, non-fatal (except Stop semantics). Evidence task-28
  Commit: Y | feat(core): lifecycle hooks w/ trust review

- [x] 29. harness-mcp: client + SERVER mode
  What to do / Must NOT do: rmcp 3.x client (stdio + Streamable HTTP, initialize handshake, roots confinement, stderr→tracing; PER-SERVER connectTimeout + non-stalling unresponsive-server handling — Droid, research §16); expose harness AS MCP server (tools: ask/status/session control); ACP SERVER MODE (Agent Client Protocol — Zed-authored, JetBrains-adopted, research §23/§24): the daemon also speaks ACP agent-side so Zed/JetBrains-style hosts drive OUR harness as a native panel agent (ACP Agent Registry listing) — one more attach surface over the same engine, host-side checkpoint interop preserved; MCP RESOURCES + PROMPTS surfaces supported (Amp ships neither — AVOID, research §17); MCP TRANSPORT POOL keyed by (server + config fingerprint) shared across sessions w/ refcounted release + idle drain + budget guardrails w/ hysteresis (off/warn/enforce — Qwen, research §18); MCP TRUST BINDS TO THE FULL ENTRY HASH of each server config — never the entry name alone (Cursor MCPoison CVE-2025-54136 TOCTOU: benign entry approved then command swapped, research §19); ORG-MANAGED MCP ALLOWLISTS (Copilot, research §20): unoverridable org-layer server allow/deny (secure name/URL/stdio-command matching) layered above user config; MCP-server mode exposes session create/search/interact/events + cost-limit param + GATHER fan-in barrier for parallel session completion (Devin, research §22); FILE-BASED MULTI-PROVIDER MCP MANAGER (Warp, research §25): scan config files across providers (per-repo + global), hash-keyed TEMPLATABLE installations (shareable server defs w/ substituted secrets), per-path parse diagnostics, auth-gated deferred autostart; MCP SCHEMA DISK CACHE (jcode, research §32): last-seen tool schemas cached on disk keyed by config fingerprint, advertised at spawn (zero first-snapshot cache miss), reconciled against live tools/list — kills the late-tool-snapshot rebuild penalty. Must NOT auto-trust servers (hash/pin + approval on first use).
  Parallelization: Wave 6 | Blocked by: 6,24 | Blocks: 30,32,38,45
  References: draft §D3; §librarian-3 rmcp
  Acceptance: golden interop tests against reference MCP servers
  QA: happy: mount filesystem server, use tool; failure: root-escape attempt denied. Evidence task-29
  Commit: Y | feat(mcp): rmcp client + server mode

- [x] 30. Skills/packs system
  What to do / Must NOT do: SKILL.md format = agentskills.io-compatible + extensions (triggers list incl. non-English, negative scoping, languages[], version, allowed-tools, per-model guidance sections); loader scans .harness/.opencode/.claude/.agents/.cursor skills paths w/ cross-vendor dedup-by-name (Grok 4-vendor discovery — compat-swallow, research §15); project-rules pickup: AGENTS.md std + CLAUDE.md + .claude/rules + .cursor/rules, per-directory stacking, folder-trust gated; RULES PRECEDENCE Team > Project > User w/ enforceable team floor + nested AGENTS.md COMBINED semantics (child-wins-on-conflict — Cursor, research §19); NESTED MONOREPO SKILLS auto-scoped to their subtree + /migrate-to-skills-style converters for legacy rules/slash-commands (Cursor, research §19); SKILLS HOT-RELOAD watchers + skills-lock.json pinning (Warp, research §25) + per-harness global rules dirs w/ CLAUDE.md→AGENTS.md migration on first init (Warp, research §25); pack aggregation (_shared templates + findings-schema pattern from user's audit packs); progressive disclosure (metadata only until invoked); skills may run CONTEXT:FORK — in a dedicated subagent w/ only the result returned + allowed-tools pre-approval at skill level (Copilot, research §20); PER-INSTALL CONSENT STRINGS (Gemini CLI, research §31): every extension/skill install surfaces a plain-language INSTALL_WARNING describing what it can do before first activation. Must NOT inject full skill bodies by default.
  Parallelization: Wave 6 | Blocked by: 6 | Blocks: 31–34, 45
  References: draft §D4/D6; §local-inspection; §librarian-4
  Acceptance: load user's real skills dirs (fixture copy) — all 60+ parse, route, negative-scope
  QA: happy: trigger routes correctly; failure: DO-NOT-USE respected. Evidence task-30
  Commit: Y | feat(skills): compatible pack system

- [x] 31. Skill Forge (auto-authoring + correction journal)
  What to do / Must NOT do: post-task /learn distiller (cheap model) → proposes SKILL.md diffs; correction journal capture (hunk rejections w/ feedback, denials, post-output edits) w/ SECRET REDACTION at capture time (Kilo kilo-memory pattern — never store keys/tokens in the journal); DRAFT tier + approval gate; eval-before-activate (replay originating session type); usage analytics + context-cost + pruning suggestions; GROWING+REFINEMENT delta updates only — never whole-skill rewrites (ACE "context collapse" mitigation, research §7); trajectory quality filtering before learning (Atropos-style export format); /learn-STYLE SYNTHESIS from URLs/dirs/videos + /curator-style lifecycle (stale 30d → archive 90d → pin — Qwen, research §18); KNOWLEDGE SUGGESTIONS auto-generated from chat feedback — regenerable, updates existing items in place, org→enterprise promotion path (Devin, research §22); CLOSED SKILL LOOP (harness-rs, research §26): episodes (situation→trace→outcome) → SkillDistiller promotes PROVEN episodes to routable SKILL.md → SkillUseTrace triggers SkillReviser fix-drafts when a skill misled — self-evolution as auditable, lintable files; HERMES 5-MECHANISM LOOP (research §27): in-use improvement + post-turn background-review fork + idle curator + memory nudges, ALL gated by write_approval (human gate) — never a single magic step; THIRD-PARTY SKILL QUARANTINE (ClawHavoc lesson via ClawHub, research §27): every marketplace/installed skill passes a content-hash scan → quarantine → lock.json ledger before activation; PRIME-AGENT PATTERN (research §27): continual-harness CRUD (/refine evidence-backed versioned edits) over an IMMUTABLE base prompt. Must NOT auto-activate or silent-overwrite.
  Parallelization: Wave 6 | Blocked by: 30 | Blocks: 34, 45
  References: draft §R + librarian bg_96ff4596 findings in research file §skill-forge
  Acceptance: scripted session → skill proposed → approved → replay passes → activated; rejected proposal leaves nothing
  QA: happy: full loop; failure: eval-fail blocks activation w/ report. Evidence task-31
  Commit: Y | feat(skills): Skill Forge w/ trust gates

- [x] 32. Registry: spec + reference server
  What to do / Must NOT do: pack registry protocol (search/install/pin+hash verify, trust review, signatures); reference server implementation (self-hostable, static-index capable); CLI + Web manager; MARKETPLACE ITEMS = declarative config patches (Roo pattern): an item writes config files (packs, MCP servers, rules) w/ project-vs-global scope choice + parameterized secrets prompted at install — never executable code; PLUGIN BUNDLES w/ managed manifests (Devin, research §22): skills+rules+hooks+MCP+subagents ship as one unit; requiredPlugins/optionalPlugins/forbiddenPlugins w/ glob deny + SHA pinning + secret refs; governance = higher authority wins, forbid-'*' lockdown; LOCK.JSON LEDGER (hermes, research §27): installed-skill/server provenance recorded as content-hash ledger — tampering detectable, upgrades diffable. Must NOT phone home by default.
  Parallelization: Wave 6 | Blocked by: 30 | Blocks: 33,45
  References: draft §D6; §librarian-6 registry precedents
  Acceptance: install→hash-mismatch rejection test; offline install from local dir
  QA: happy: pin+verify; failure: tampered pack refused. Evidence task-32
  Commit: Y | feat(registry): verified pack distribution

- [x] 33. Domain pack content
  What to do / Must NOT do: author shipped packs — programming (.NET per user's skills, frontend, backend, devops), legal, maths, accounting, finance/investors; port exemplars of user's audit-squad + datacenter/AEC + AI-deployment packs (fixture-quality, documented). Must NOT hardcode packs into binary (they're registry content).
  Parallelization: Wave 6 | Blocked by: 30 | Blocks: 45
  References: draft §D6 + local inspection pack inventory
  Acceptance: each pack: triggers + negative scope + one worked example + eval scenario
  QA: happy: pack routes on representative prompts; failure: cross-domain prompts don't mis-trigger. Evidence task-33
  Commit: Y | feat(packs): shipped domain content

- [x] 34. Model-profiles subsystem
  What to do / Must NOT do: per-model failure-taxonomy computation from transcripts (syntax/tool-misuse/context-overflow/retry-storm/edit-rejection rates, cost/turn); behavioral axes (verbosity, thinking depth, delegation preference, drift curve, edit-format competence — draft §T); profile store; feeds router + skill per-model guidance + launcher balancing; A/B skill optimization via evals adapter, proposals through Skill Forge gate. PROTOCOL DIALECTS EXPLICIT per profile (thinking/reasoning-content format, usage semantics, tool-call identity rules, finish-reason vocabulary) — never inferred from endpoint URLs (dsh thinkingFormat bug: 72k-char reasoning bloat vs 6k tuned, research §6); DISJOINT token accounting — normalize cache-hit-inclusive prompt_tokens (DeepSeek pattern: prompt_tokens = cache_hit + cache_miss), trailing-usage-chunk and gateway null-identity handling per dsh translate.ts precedents; DIAL effort-bundle grammar (Amp, research §17): named user-facing levels bundle model+effort+prompt+tools per position — profiles make each dial position concrete instead of exposing raw model pickers; ELICITATION CONSTITUTION (research §33): profiles implement SET/PROMPT/THROTTLE — SET API knobs per family (effort explicit per workload class, NEVER trust defaults; xAI effort-on-every-call; cache-preserving per-message effort beta where supported), PROMPT per-model counterweights for documented pathologies (commit-to-approach for over-exploration; batching+progress+search-when-unsure for under-parallelizers; stopping-conditions+retrieval-budgets for overthinking; bias-to-action for early-stoppers) gated on OUR evals, THROTTLE structurally what neither fixes; PER-MODEL 400-TRAP feature-detection table in the catalog (budget_tokens on 4.7+; prefill on 4.6+; forced tool_choice on Fable; thinking_budget+level on Gemini; none+sampling on Astra; penalties/stop on Grok) — feature-detect per MODEL, never per provider. Must NOT share data externally.
  Parallelization: Wave 6 | Blocked by: 10,31 | Blocks: 40,45
  References (executor has NO interview context - be exhaustive): draft §T + behavioral axes; research §6 dsh adapter lessons + §7 skill-learning art
  Acceptance criteria (agent-executable): synthetic transcripts produce correct taxonomies; A/B picks better variant deterministically; dialect table drives wire handling (wrong-dialect test fails closed)
  QA scenarios (name the exact tool + invocation): happy: profile renders per-model tips + correct dialect; failure: insufficient data → neutral profile; unknown dialect → explicit config error, never guessing. Evidence task-34
  Commit: Y | feat(intel): model weakness profiles + skill A/B

- [x] 35. Daemon: protocol + multi-attach + web serving
  What to do / Must NOT do: axum WS+SSE + interprocess IPC; ONE client protocol (events/commands/permissions/diffs); multi-client attach w/ shared permission queue (crush pattern); serve web SPA static assets; 127.0.0.1 + generated token by default; URL-ADDRESSABLE sessions (Amp threads, research §17): every session deep-linkable + attachable from any client; MULTI-CLIENT PERMISSION MEDIATION (Qwen, research §18): 4 policies (first-responder/designated/consensus-quorum/local-only) + cancel-vote sentinel w/ anti-injection guards + N1 synchronous-timeout invariant; TYPED SSE EVENT SCHEMA w/ divergence-proof resync (monotonic ids, bounded replay ring, slow-client eviction, state_resync_required + delta-skipping reducer); ACTIVE-SESSION REGISTRY keyed by PID w/ boot-id liveness proofs; scheduler (cron packs) + notification emitter; DURABLE-EXECUTION OPTION (Cursor Temporal pattern, research §19): workflow-per-turn + signal-with-start follow-ups + child-workflow subagents + content-addressable activity retry WITHOUT token replay + production-history CI replay tests for non-determinism; REMOTE-WORKER SPLIT-BRAIN mode: agent loop stays in the daemon, tool execution delegated to a user machine via OUTBOUND-ONLY connection — stdio MCP stays on the worker, HTTP MCP proxied, credentials never co-located w/ the model client (Cursor self-hosted workers, research §19); OUTPOSTS-STYLE RECONCILIATION for remote workers: workers CLAIM sessions from named queues, claims EXPIRE on deadline → the session self-heals back into the queue (Devin, research §22); ZERO-DOWNTIME RELOAD (jcode, research §32): the daemon exec()s the new binary on the SAME socket (same PID, clients auto-reconnect) — upgrades are not restarts; STALE-SOCKET REAPING gated on flock proof of owner death (jcode, research §32); OPTIONAL OTel EXPORT for fleet ops: double opt-in, content-free by default (4 content gates: prompts/responses/tool-details/tool-content), mTLS, secret-scrubbed — Grok post-scandal posture, research §15; spans follow GenAI semconv (gen_ai.* attrs incl. cache/reasoning tokens, compaction flags) + traceparent propagated to hooks (Copilot, research §20); CLIENT-APPLIES-EDITS flag in the protocol (Mentat pattern — VS Code client applies diffs itself for native IDE undo/redo integration). AUTH = peer-address verification + token — NEVER trust client-supplied Host headers (CVE-2026-82533 lesson, research §6: dsh's 9.4-CVSS escape); ship SECURITY.md + private reporting channel; release policy: security fixes reach ALL install channels simultaneously. Must NOT require network for local use; Must NOT ship a known perf regression (dsh 0.1.3-alpha.1 admission).
  Parallelization: Wave 7 | Blocked by: 23–24,29 | Blocks: 36–39,41–42,45
  References (executor has NO interview context - be exhaustive): draft §D1; research §librarian-2 crush; research §6 dsh CVE + release-skew lessons; §productivity
  Acceptance criteria (agent-executable): two simultaneous clients see identical state; permission answered once applies to both; NEGATIVE TEST — request with spoofed Host header + no/invalid token is refused (regression test for CVE-2026-82533 class)
  QA scenarios (name the exact tool + invocation): happy: TUI+Web attach; failure: token mismatch + Host-spoof refused; security-fix release drill reaches all channels. Evidence task-35
  Commit: Y | feat(daemon): multi-client protocol + scheduler + hardened auth

- [x] 36. TUI
  What to do / Must NOT do: ratatui event-driven (0.0% idle verified by test); collapsed accordions; HUD (context gauge, cache-hit %, cost w/ PER-MODEL breakdown + routing-savings estimate — Devin /session-stats pattern, research §22); omnibar @file/@symbol/@git + /mode /rewind /skill; y/a/n gates; team grid visualization; streaming markdown; keybind config. Must NOT poll-render.
  Parallelization: Wave 7 | Blocked by: 35 | Blocks: 45
  References: draft §paste §7; §productivity
  Acceptance: insta snapshot suites for main screens; idle-CPU assertion in test harness
  QA: happy: full session driven by key scripts; failure: resize/EOF handled. Evidence task-36
  Commit: Y | feat(tui): event-driven interface

- [x] 37. Web frontend
  What to do / Must NOT do: SPA (served by daemon, no node server): live session view w/ IMAGE PASTE/UPLOAD in the composer (vision turns — blocks from todo 4), semantic-hunk review UI (y/n/e), dashboards (cost/cache/context, tool heatmaps, eval trends), pack/registry manager, share links (redacted+expiring), replay debugging from event log. Must NOT depend on cloud services.
  Parallelization: Wave 7 | Blocked by: 35 | Blocks: 45
  References: draft §productivity; §latent-needs GUI evidence
  Acceptance: e2e (playwright) happy-path: attach, approve hunk, see HUD update
  QA: happy: two-browser attach; failure: expired share link 410. Evidence task-37
  Commit: Y | feat(web): dashboard frontend

- [x] 38. VSCode extension
  What to do / Must NOT do: TS thin client over daemon protocol; editor telemetry channel (active file/cursor/selection/diagnostics, opt-in); native diff decorations + per-hunk code lenses; command palette (omnibar grammar); status-bar gauge; IDE LSP/ripgrep BRIDGING (priority over own servers). Must NOT bundle model calls in extension.
  Parallelization: Wave 7 | Blocked by: 35 | Blocks: 45
  References: draft §productivity #2–4; Cline/Roo spike findings in research file
  Acceptance: extension integration test harness (vscode-test): telemetry flows, lens accepts hunk
  QA: happy: "fix this" uses cursor context (no file question); failure: daemon down → reconnect UX. Evidence task-38
  Commit: Y | feat(vscode): thin-client extension

- [x] 39. CLI
  What to do / Must NOT do: clap; interactive REPL fallback (colors/streaming); headless run + `--output-json` (eval adapter contract); quick-capture one-liner; `--image <path>` attachment flag (multimodal turns, headless + REPL); `harness doctor` (config/provider/sandbox/MCP health + env + migration + network probes — Droid validates the doctor pattern, research §16); NIGHTLY BUILDS EMBED THE HEAD SHA in the version tag (vX.Y-nightly.DATE.SHA — tag↔SHA traceability by construction, Gemini CLI research §31). Must NOT duplicate daemon logic (client only).
  Parallelization: Wave 7 | Blocked by: 35 | Blocks: 44–45
  References: draft §productivity #11/#14
  Acceptance: assert_cmd suite; doctor exits nonzero on induced failure
  QA: happy: scripted headless run; failure: doctor catches broken provider. Evidence task-39
  Commit: Y | feat(cli): repl, headless json, doctor

- [x] 40. Subagents + spec-driven launcher
  What to do / Must NOT do: fork-join ephemeral subagents (isolated child sessions, hidden from main list, 1–2k summaries returned; task_id-RESUMABLE children — Kilo pattern; DENY-ONLY permission inheritance + task/todowrite capability-gated for children); spec→decompose (objectives/task graph/per-task capability+model)→heterogeneous launch→synthesis w/ cross-consistency check; BEST-OF-N multi-version runs (up to 4 parallel same-prompt implementations w/ per-version models — Kilo Agent Manager pattern) + PLAN-BRANCH model comparison (same spec on different models via snapshot branches — Plandex pattern); effort-scaling rules + hard budget guards (no 50-subagent spawns); tmux-grid/team visualization via daemon events; HARNESS-AS-SUBAGENT backends: optionally delegate tasks to real Claude Code/Codex child processes via their official protocols (app-server --stdio / Agent SDK) as additional launcher backends, supply-chain-carve-out (platform binaries opt-in, disabled by default) — dsh-precedent, research §6; BUILT-IN SUBAGENT SPLIT (Droid): explorer = read-only light tier, worker = read-write medium tier (+ our deep tier); MISSION VALIDATOR MILESTONES: at task boundaries spawn separate scrutiny + user-testing validator workers, config-skippable (Droid Missions, research §16); CLEAN-CONTEXT REVIEWER: milestone reviews run w/ ZERO shared context — the reviewer reasons backward from the diff alone (Devin Review: ~2 bugs/PR, 58% severe — attention math beats shared context, research §22); SUBAGENT PERMISSION BUBBLING (children request escalation through the parent) + commandExecutionPolicy ladder off/auto/eager/sandbox (Antigravity, research §21); /boost-STYLE 3-PHASE DECOUPLE: strategy → isolated execution → verification as separate agent phases (Antigravity, research §21); SPAWN-PACING guard: burst quota + refill rate on child spawns (Amp orb pacing: 20-burst then 1/5min, research §17); CACHE-PRESERVING FORK SUBAGENTS (Qwen, research §18): forks inherit parent's exact system prompt + tool DECLARATIONS, execution narrowed via a fork_tools-style allowlist — never change model-visible declarations (protects the shared provider prefix; Qwen claims 80%+ savings); best-of-N applies the WINNER'S DIFF from its worktree, not just reports (Qwen Arena); per-subagent MODEL BRACKET PARAMS — model[effort=high,context=300k] portable knobs in agent definition files (Cursor, research §19); DISJOINT-WRITE-SET CONTRACT for parallel children + flat depth-1 default w/ opt-in nesting (Zed MAX_SUBAGENT_DEPTH, research §23); FILE-TOUCH COLLISION ALERTS (jcode, research §32): every write/edit publishes a FileTouch{path, op, intent} event and the daemon alerts BOTH sides when siblings touch overlapping files — cheap, high-value multi-agent coordination; SPAWN-AUTHORIZATION CLASSES (research §33): standing authorization for read-only/validation/exploration vs fresh authorization for edits/commits/deploys — spawn refusals are LOUD structured errors, never silent fallback to parent-DIY (the Codex silent-degradation failure class); PARALLELISM NORMALIZATION (research §33): tool results batched in ONE message (one-result-per-message silently teaches serialization) + batch_tool meta-tool for under-parallelizing models + disable_parallel_tool_use for mutating tools; external-harness backends AUTO-INSTALL from release assets w/ SHA-256 verification + env redaction (Zed ACP child discipline, research §23); PER-HARNESS PLUGIN MANAGERS (Warp, research §25): marketplace plugins installed + version-checked per external CLI agent so OUR terminal can observe/steer them (OSC 777 emitter side). Must NOT share parent context wholesale; Must NOT allow silent thinking-block/context loss when model family changes mid-run (Kilo bug class — surface it).
  Parallelization: Wave 8 | Blocked by: 12–13,25,34 | Blocks: 45
  References (executor has NO interview context - be exhaustive): draft §B/S; research §librarian-2 Anthropic multi-agent doctrine + §6 dsh subagent providers
  Acceptance criteria (agent-executable): scripted multi-model spec run — decomposition, parallel children, merged result, per-agent telemetry; external-harness backend smoke test (mock child protocol)
  QA scenarios (name the exact tool + invocation): happy: 3-agent spec completes in budget; failure: budget breach stops spawning, reports partials. Evidence task-40
  Commit: Y | feat(orchestration): spec launcher + team mode + harness backends

- [x] 41. Scheduled runs + notifications
  What to do / Must NOT do: daemon cron of packs (goose-scheduler pattern); system notifications on completion/approval-needed (extensible via skills/MCP) + OSC 777/9 TERMINAL NOTIFICATION EMISSION (Warp, research §25): escape-sequence bus any listening terminal/harness receives — cross-harness notification channel for ~free; run history in sessions store; COMPLETION GATE for unattended loops: adversarial evidence-review — autonomous runs end only when an independent reviewer pass accepts the evidence bundle against the goal criteria (/goal pattern, research §15); verdicts use the CONFIRMED/REFUTED/INCONCLUSIVE vocabulary w/ evidence links (Droid /verify pattern, research §16); AUTOMATIONS AS DECLARATIVE DEFINITIONS (terraform-style) w/ per-automation budget + invocation caps + network-policy intersection (Devin, research §22). Must NOT schedule unapproved draft-tier skills.
  Parallelization: Wave 8 | Blocked by: 35 | Blocks: 45
  References: draft §D/Q
  Acceptance: fake-clock scheduled triggers; notification delivered (mock)
  QA: happy: nightly pack run; failure: missed schedule catches up w/ jitter. Evidence task-41
  Commit: Y | feat(daemon): scheduler + notifier

- [x] 42. Per-role model routing
  What to do / Must NOT do: role→model config (architect/editor/reviewer/hydrator/summarizer) + UTILITY ROLES on cheap models (commit-messages, session-titles, auto-continue, dedup-folding — Plandex's 9-role pattern); cheap-model hydration + summarization; profile-aware suggestions; fallback = explicit confirmation; PER-TASK PROFILE STICKINESS: task pins its launch profile across resume + subtasks inherit parent profile (Roo pattern — kills silent model drift mid-task); ON-DEMAND model-catalog SEARCH TOOL for the model itself (kilo agent_manager_models pattern — full catalog NEVER injected into context); BENCHMARK-DRIVEN LIVE ROUTING option: per-request difficulty classification → cheapest benchmark-proven model (Kilo efficient-tier; evals todo 44 feeds the ranking table); COMPLEXITY-TIER ROUTING (Droid): light/medium/heavy task tiers → per-tier model+effort defaults (composes with benchmark routing, research §16); modelFallbacks PER-MODEL fallback MAP as primitive — every hop surfaced to the user, NEVER silent (our D-decision guard vs Droid's silent failover, research §16); ROUTING-INCENTIVE ALIGNMENT: auto-routed usage priced below manual picks (~10% Copilot discount pattern) so user economics favor the router (research §20); FUSION SIDEKICK ROUTING (Devin, research §22): a cheap sidekick w/ its OWN persistent cached context runs parallel to the frontier lead; model switches execute ONLY at compaction boundaries — the cache miss happens anyway, so the switch is free; PER-ROLE MODEL PROFILES incl. utility roles (default/subagent/inline/commit-message/summary/compaction — Zed, research §23) + REFUSAL→FALLBACK retry w/ attempt counter + modeled stop reasons (end-turn/refusal-truncation/max-tokens); RANK-VERBATIM FAILOVER CHAINS (forge, research §26): fallback order is an explicit verbatim list (never provider round-robin) + rate-limit WAIT-OUT windows (≤75s) before failing over — deterministic, documented, reviewable. Must NOT route silently around failures.
  Parallelization: Wave 8 | Blocked by: 10,34 | Blocks: 45
  References: draft §K
  Acceptance: routing table tests incl. confirmation flow on fallback
  QA: happy: roles hit configured models; failure: outage → ask, never substitute. Evidence task-42
  Commit: Y | feat(intel): role routing w/ confirmed fallback

- [x] 43. Performance suite + CI gates
  What to do / Must NOT do: criterion benches (repomap, eviction, prompt-builder, PTY scrape, session paging, TUI render); dhat heap profiles archived; CI perf gate fails >10% regression; budgets enforced (cold start <50 ms, idle 0.0%, RSS <80 MB, p99 <16 ms); binary-size budget <40 MB; allocation discipline applied (compact_str, arenas, buffer reuse; mimalloc feature); FRAME-BUDGET DISCIPLINE (Zed GPUI, research §23): 10ms frame budget w/ hang-threshold detection + per-frame foreground-spend attribution surfaced in a debug overlay — regressions fail the gate, not just benchmarks. Must NOT merge perf regressions.
  Parallelization: Wave 9 | Blocked by: all engine todos | Blocks: 45
  References: draft §U
  Acceptance: baseline artifacts committed; gate active in CI
  QA: happy: run green at budgets; failure: injected +50% slowdown fails gate. Evidence task-43
  Commit: Y | perf: benchmark suite + regression gates

- [x] 44. Evals + conformance
  What to do / Must NOT do: evals adapter (drives CLI --output-json); expected-failures YAML pinning; live-model sweeps SCHEDULED ONLY; skill A/B mini-benchmarks; harness-comparison mode (harbor-style); EVAL TRANSPARENCY DISCIPLINE (Qwen README, research §18): publish per-version benchmark tables w/ exhaustively disclosed harness settings + harness-matched official rows — never vendor-harness numbers w/o matched baselines (+10–25pt inflation lesson); CURSOR-BLAME-STYLE EVAL MINING: mine commit-to-request histories as eval task sources (real user intent, not synthetic) + resolution-rate metrics for review agents (Bugbot 52→76-78% arc, research §19); FAIR-INTERNET-USE SCANNER: eval runs that open upstream PRs/diffs are zeroed automatically (FrontierCode integrity rule, Devin, research §22); REPLAY-FROM-TRACES BENCHMARKS (Warp Factories, research §25): stored real-run traces re-scored across model/harness/scorer variations feeding the routing table — private benchmark infrastructure as an eval surface; MICRO-HARNESS EVAL DISCIPLINE (forge + harness-rs, research §26): pass^k floors (not pass@1 peaks); TRIGGER-COVERAGE (a guard that never fires = unmeasured noise); leave-one-out GUARD ABLATION (each guard's on/off delta measured); every guard TEST-PINNED incl. the measured prose-spiral it prevents; RETRACTION-IN-PLACE when a claim fails audit (benchmark honesty); doc-constant SYNC TESTS (normative docs must match live code constants); JOINT MODEL×PROFILE EVALUATION (research §33): the unit of evaluation is the harness–model pair (harness variance measured up to 7.8× model variance — single-model results are unattributable); regression runs = locked profile; harness experiments = ≥2×2 model×profile grid w/ ETCSOVG-style harness cards; report tokens/solved + failure-fingerprint vectors + Mean@k AND Pass^k (the 24pt consistency gap); TSS golden-prefix replay (first-two-tool-calls vs pinned reference) as the drift gate. Must NOT gate PRs on live-model results.
  Parallelization: Wave 9 | Blocked by: 39 | Blocks: 45
  References: draft §librarian-5
  Acceptance: nightly sweep produces report artifact; known-failure pinned and annotated
  QA: happy: A/B picks correct variant; failure: unpinned regression surfaces in report. Evidence task-44
  Commit: Y | feat(evals): conformance + A/B harness

- [x] 45. WASM plugin runtime (late wave)
  What to do / Must NOT do: wasmtime WASI 0.2 behind feature; ResourceLimiter 32 MB + fuel metering; preopened dirs; capability-scoped host API aligned with ToolProvider trait (extensions register as tools); DECLARED CAPABILITY ENUM (process-exec/download/package-install — Zed, research §23) + VERSIONED WIT-style interface ladder so old extensions keep running across host upgrades (Zed: 10 WIT versions, research §23); hot-reload. Must NOT enable by default; Must NOT exceed caps (test kills offender).
  Parallelization: Wave 9 | Blocked by: 6,26 | Blocks: F
  References: draft §paste §5; §librarian-4 Zed capability model
  Acceptance: fixture plugin (Rust→wasm) registers a tool, enforces caps; infinite-loop plugin killed by fuel
  QA: happy: sandboxed tool call; failure: memory hog terminated cleanly. Evidence task-45
  Commit: Y | feat(wasm): sandboxed plugin runtime

- [x] 46. Community surface: positioning README + contributing ramp + AsciiDoc docs site
  What to do / Must NOT do: positioning README.md per the blueprint — hero one-liner + badges; RECEIPTS TABLE (feature → harness that proved it [research §N] → failure we avoid [research §N]) — claims carry evidence by construction; 5-line quickstart; four-frontends showcase; performance-budget table w/ CI-enforcement note; "built on post-mortems" section linking the research file; architecture diagram (one daemon, 22-crate workspace, four thin clients); honest sourced comparison table vs Claude Code/Codex/goose/opencode (factual rows only, each citing research §§); contributing on-ramp w/ good-first-issue wave map (each todo's acceptance criteria is already issue-shaped); roadmap (waves); dual-license note. CONTRIBUTING.md: one-command dev setup (cargo nextest + mock-provider hermetic loop — zero API keys needed to contribute), TDD discipline, EVIDENCE CULTURE (PRs that change behavior cite existing evidence or add it — research file or evals), wave map + parallelization guidance, code-of-conduct link. CODE_OF_CONDUCT.md (contributor-covenant). Issue templates: bug / feature / research-suggestion (external study input keeps flowing); PR template w/ evidence + guardrail-impact fields. DOCS SITE (HYBRID FORMAT — user decision, session m0351): README.md + CONTRIBUTING.md stay Markdown (zero-friction storefront); docs/ manual + ARCHITECTURE guide + research companion in AsciiDoc w/ xref stable-ID cross-references + include:: modularity, built to a static docs site (Asciidoctor/Antora) as a CI job; the research file stays Markdown as the working artifact, the companion indexes it. NAMING GATE: public name stays 'harness' (crates harness-*) — an explicit naming decision is a pre-v1.0-marketing checklist item (crate rename is cheap pre-release, brand-breaking after). RELEASE TRANSPARENCY: publish per-release bench leaderboard + eval tables (todo 44 discipline) linked from README. Must NOT make marketing claims without in-repo evidence; Must NOT embed counters/tracking pixels in docs.
  Parallelization: Wave 9 companion (startable after todo 1; polished w/ 43–44 outputs) | Blocked by: 1 | Blocks: F
  References: positioning analysis (session m0349–m0351); research file receipts; draft §success
  Acceptance criteria (agent-executable): README renders w/ all internal + research-file links resolving (lychee/linkcheck CI job); docs site builds in CI (asciidoctor) w/ ZERO broken xrefs; all community files present (README/CONTRIBUTING/CODE_OF_CONDUCT/issue+PR templates); every receipts-table row carries a research § cite (grep-verifiable)
  QA scenarios (name the exact tool + invocation): happy: new contributor clones → one command from CONTRIBUTING quickstart → tests green (quickstart is literally executable); failure: broken xref → docs CI job fails red. Evidence task-46
  Commit: Y | docs: positioning README + community ramp + AsciiDoc docs site

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.
- [x] F1. Plan compliance audit
- [ ] F2. Code quality review
- [ ] F3. Real manual QA
- [ ] F4. Scope fidelity

## Commit strategy
Conventional Commits, one commit per todo (messages specified per-todo above); squash-merge waves; tags per wave (`v0.wave-N`); cargo-dist produces release artifacts on main tags; CHANGELOG generated from conventional commits.

## Success criteria
- All 46 todos green w/ evidence files; final wave F1–F4 APPROVE.
- All budgets met: cold start <50 ms; idle CPU 0.0%; daemon RSS <80 MB; binary <40 MB; repomap <10 ms incremental; p99 render <16 ms.
- Full user journey demonstrated agentically: spec → multi-model launch → hunk review in three frontends → /rewind → skill learned via /learn → scheduled run next day (fake clock).
- Every Must-NOT-have guardrail has a test proving enforcement.
- Research file `.omo/research/harness-research.md` cross-referenced: every adopted/avoided decision traceable to evidence.
