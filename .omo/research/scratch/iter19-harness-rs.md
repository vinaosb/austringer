# Iteration 19 — Deep Dive: liliang-cn/harness-rs (Rust micro-harness slice, target B)

- **Repo**: https://github.com/liliang-cn/harness-rs — MIT, Rust edition 2024, MSRV 1.92
- **Verified metadata** (gh api, 2026-09-11): 12 stars, 3 forks, 0 open issues, **1 contributor** (`gh api repos/liliang-cn/harness-rs/contributors` → 1), created 2026-05-16, last push 2026-09-06
- **Cloned at HEAD**: `061bf3e1d026ccb8becc97b8df3bb911e38d6f13` (local: `%TEMP%\opencode\harness-rs`)
- **crates.io VERIFIED** (all 15 crates re-synced 2026-09-06 at **0.0.63** — coordinator brief's "core 0.0.48/tools 0.0.63" is stale; workspace uses a single shared version, `harness` name itself is squatted by an unrelated crate so everything publishes as `harness-rs-*`): core 5,559 dl / loop 2,505 / models 2,654 / context 3,406 / cli 1,001 / tools **90** dl
- Key files read: `DESIGN.md`, `README.md`, `docs/guide.md`, `docs/benchmarks.md`, `CHANGELOG.md` (135 KB!), `crates/harness-core/src/{event,redact/*}.rs`, `crates/harness-loop/src/{lib,compactor,sandbox,memory_layer,blueprint}.rs`, `loop_engine/level.rs`, `orchestrator/store.rs`, `experience/mod.rs`, `Cargo.toml`, `crates/harness-cli/src/main.rs` (partial)

---

## 1. Architecture narrative

### 1.1 Crate fan-out map — planned vs actual (the single most important finding)

DESIGN.md §4 plans a ~20-crate maximal workspace (`harness-compactor`, `harness-hooks`, `harness-blueprint`, `harness-sandbox`, `harness-skills`, `harness-templates`, `harness-orchestrator`, `harness-loop-engine`, `harness-scheduler`, `harness-permissions`, `harness-sensors-rust/common`, `harness-tools-fs/shell`, `harness-mcp`…). **Reality: 15 workspace crates, because ~10 planned crates were merged into `harness-loop`.** The merge rationale is written into the code itself ([crates/harness-loop/src/lib.rs#L13-L17](https://github.com/liliang-cn/harness-rs/blob/061bf3e1d026ccb8becc97b8df3bb911e38d6f13/crates/harness-loop/src/lib.rs)):

> "Ten of them, and not one carried a dependency a caller would want to avoid. The boundaries bought nothing and cost every release ten more publishes and every host ten more version pins; a crate boundary here has to name a dependency you might not want, and these did not."

Actual layers (all paths relative to repo root, sha `061bf3e`):

| Layer | Crate(s) | What's inside (verified) |
|---|---|---|
| Types+traits core | `crates/harness-core/` (24 small files) | `Model`, `Tool` (w/ `ToolRisk`), `Guide`, `Sensor`, `Signal` (+`auto_fix: FixPatch`), `Hook`, `Compactor`, `Event`, `Skill`, `Memory`/`RecallStore`, `permissions.rs` (12 KB), `redact/` (PII engine), `embed`, `speech`, `image` |
| Macros | `crates/harness-macros/` | `#[skill] #[tool] #[guide] #[sensor] #[hook]` proc-macros, `inventory`-based auto-registration, compile-time SKILL.md validation |
| Models | `crates/harness-models/` | **Native** providers: `openai_compat.rs` (77.7 KB, SSE streaming), `anthropic.rs` (34 KB), `gemini.rs` (46.7 KB, search grounding), `router.rs`, `retry.rs`, `mock.rs`, image (openai/dashscope) + speech (openai/dashscope) + embedders (openai/gemini/ollama) |
| Context | `crates/harness-context/` | prompt/Context blocks + prefix-cache, skills subsystem (`skills/{loader,validate,lint,export,write}`), `redacting_memory.rs`, `memory_guard.rs` (GuardedMemory), `file_recall.rs` (FTS5/CJK), **`user_model/` — portrait.rs alone is 74 KB of user-modeling code (undocumented in DESIGN)** |
| Memory backends | `crates/harness-memory/` | URL-scheme dispatch `open_memory(url)`/`open_recall(url)`: `file://`, `sqlite://` (FTS5+CJK), `mem://`, `surrealkv://`, `ws://` (SurrealDB BM25+HNSW hybrid), `cortexdb://`; unknown scheme = compile-time-named feature error, never silent fallback |
| CortexDB | `crates/harness-cortexdb/` | gRPC "brain" shared with Claude Code/Codex |
| **The mega-crate** | `crates/harness-loop/` | `lib.rs` **120.7 KB** — AgentLoop, guards, Outcome; plus modules: `compactor.rs` (33 KB), `hooks/{mod,audit,broadcast,otel}` (41 KB), `sandbox.rs` (45 KB), `blueprint.rs`, `loop_engine/` (L1/L2/L3), `orchestrator/` (Job DAG, 57 KB), `scheduler/`, `experience/` (episodes+skill distillation, 190 KB), `memory_layer.rs` (38 KB), `recall_layer.rs`, `replay.rs`, `telemetry.rs`, `daemon/`, `seal.rs`, `receipt.rs`, `goal.rs`, `acceptance.rs`, `subagent.rs`, `templates/` |
| Tools | `crates/harness-tools/` | `fs.rs` (41.7 KB), `shell/` (risk-gated + background exec, 45 KB), **`browser/` — full CDP browser automation (ws/page/policy/session/tool, ~190 KB, a Playwright-class surface)**, `tasks.rs`, `agents.rs` (29 KB), `datetime/` parser (35 KB+tests), `mcp/` (server), `recall/`, `skills.rs` |
| Tool satellites | `harness-tools-docs` (PDF/Word/Excel/PPT pure-Rust + OCR `pdftoppm`+`tesseract` + LLM/vision fallback), `harness-tools-web` (`GroundedWebSearch` — model's own grounded search first, scraper fallback), `harness-tools-sql` (guarded SQL) |
| MCP | `crates/harness-mcp-client/` | client + stdio/HTTP proxy |
| Service | `crates/harness-serve/` | HTTP + gRPC service layer w/ auth (beyond DESIGN) |
| CLI | `crates/harness-cli/src/main.rs` (56.9 KB) | `harness code` REPL (NORMAL / `--yolo`), `run --record`, `replay`, `trace`, `sched`, `new`, `mcp serve` |

15 examples are workspace members and are the real test surface (`eval-bench`, `redaction-demo`, `ecommerce-ops-agent` full-stack, `cap` — a reimplementation of oh-my-pi's **hashline editing**, `experience-cortexdb`…).

### 1.2 Loop + hook bus + compactor

- **Loop** (`harness-loop/src/lib.rs`, run body ~L1241-2100): guides inject at session start → per iteration: cancellation check → budget check → compaction stages → `PreModel` → `complete()`/`stream()` (`ModelTokenDelta` fragments) → `PostModel` → per action: `PreToolUse` (a `Deny` short-circuits and the reason is fed back to the model) → tool dispatch → `PostToolUse` → sensors → partition signals into `auto_fix` patches (applied directly, gated by `PreAutoFix`/`PostAutoFix` and `is_default_safe_fix`) vs blocking feedback (fed to model). Terminates in `Outcome::{Done, BudgetExhausted, Stuck, Cancelled, …}`.
- **Guards (the loop's real innovation)**: `StuckPolicy` (byte-identical consecutive tool calls → nudge at 3, abort at 6), `MonotonyPolicy` (one-tool-many-rounds spiral; **off by default with a written measured justification**: "the false positive is unbounded in N… a default-on version would terminate working agents"), `ToolResultPolicy` (result ceiling with disk **spill** — nothing lost), `CompactPolicy` with hysteresis (test: `tests/compaction_hysteresis.rs`), acceptance checks w/ retries, `SealSet` (sealed acceptance contracts + `SealBreached`), `GoalStore` phases, per-tool deadlines, run-level `CancellationToken` (0.0.63: Ctrl-C → `Outcome::Cancelled` + process-group kill of tool children, `GroupKill::spawn`).
- **Hook bus**: DESIGN promised 27 events ([DESIGN.md §10](https://github.com/liliang-cn/harness-rs/blob/061bf3e1d026ccb8becc97b8df3bb911e38d6f13/DESIGN.md)); **shipped `Event` enum has 32 variants** (`harness-core/src/event.rs`): added `AcceptanceChecked`, `SealBreached`, `Pre/PostAutoFix`, `ModelTokenDelta`, `Cancelled`; `PostCompact` carries `before`/`after` token counts so a hook can measure what a stage actually bought. Hooks are Rust `Hook` trait objects (DESIGN's "shell-script or MCP-callable hooks" not implemented).
- **Compactor** (`compactor.rs`): exactly the 5 Claude-Code-derived stages — `BudgetReduce` → `Snip` → `Microcompact` → `ContextCollapse` → `AutoCompact`, cumulative; two impls: `DefaultCompactor` (structural: trims, file-ref elision to path+hash) and `ModelBackedCompactor` (stages 3-5 call a cheap model for real summarization); invariant test: "compaction never splits a tool call from its result"; `CALIBRATION_KEY` correction channel.

### 1.3 Sandbox / memory / scheduler / orchestration

- **Sandbox** (`sandbox.rs`, 45 KB): `FsPolicy`/`NetPolicy` = what the host *requests*; `Isolation::{None, Changes, Process}` = what the backend *actually enforces* — the "honest isolation" doctrine. Five backends: `NullSandbox`, `WorktreeSandbox` (real `git worktree add` + drop-time remove; `Isolation::Changes`), `ContainerSandbox` (`docker run --network none`), `SeatbeltSandbox` (macOS `sandbox-exec`, one sandboxed process per command, availability probe), **`BubblewrapSandbox` (Linux `bwrap` — beyond DESIGN)**. Docs honestly state the known hole: in-process fs tools bypass the sandbox and are "jailed separately".
- **Memory (3 layers)**: (1) facts — `MemoryWriter` (final answer, only on `TaskCompleted`, never on partial runs), `DecisionWriter` (`DECISION:`-marker capture, zero model calls, "the only extraction that stayed noise-free" in their measurement), `MemorySynthesizer` (LLM distillation to ≤N atomic facts); (2) recall — `MemoryGuide` re-recalls each turn using the last user message (tracks topic drift), strips stale recall blocks; (3) **experience** (`experience/`) — episodes (situation → tool trace → outcome) recorded via hooks, recalled + injected each run; **closed learning loop**: `SkillDistiller` promotes a proven episode into a routable SKILL.md (~100 catalogue tokens), `SkillReviser` drafts a fix when a skill misled a run (`SkillUseTrace` is the trigger) — auditable self-evolution.
- **Loop engine** (`loop_engine/`): `LoopLevel::{L1Report, L2Assisted, L3Unattended}` as the single autonomy switch (L1 provably read-only, unit-tested); `HumanGate` = `AlwaysEscalate` / `AllowlistGate` (auto-proceeds *only* at L3) / `CallbackGate`; named patterns (`daily_triage`, `pr_babysitter`, `ci_sweeper`, `dependency_sweeper`…); `TokenBudget` hard ceiling.
- **Orchestrator** (`orchestrator/`): Run = Job DAG; `RunState`×`JobState` double state machine; `RunStore` persists the whole Run after **every** transition (JSONL, `orchestrator/store.rs`) and `resume()` resets stuck Jobs to `Pending`; `Planner` trait returns `PlanDelta::Add` — **dynamic replanning mid-run** ("the missing feedback edge that turns a workflow engine into an agent runtime"); concurrency is deliberately `FuturesUnordered` cooperative (subagent futures are `!Send`), each Job gets a fresh `World`; `RunBudget` cross-Job cost gate; cycle detection at startup; dead-letter blocks downstream instead of hanging.
- **Scheduler** (`scheduler/`): recurring jobs, persistent store, channel-based firing, exposed to the model as a tool.

### 1.4 Benchmarks (docs/benchmarks.md — measured, not asserted)

"The suite measures the harness, not the model": 10 tasks with **machine verifiers run outside the agent**; three disciplines: **pass^k** reliability floor (CI gates on it), **leave-one-out guard ablation** (each row = default minus exactly one guard), and **trigger coverage** — a guard that never fired is flagged as *noise, not measurement*. Measured 2026-08-14 (gemini-3.6-flash, k=1): stuck detector **+2 pass** (without it a retry-trap burns 16 rounds and dies), compactor **+1** (halved trap-task input 93k→48.5k tok), result ceiling +1, spill **−17% cost/pass** at same pass, dedupe −12% cost on gemini but *lost a task* on qwen ("guard verdicts are model-relative"), acceptance ×0 on gemini but ×3 on local qwen ("rescues weaker models"). Plus `docs/per-turn-cost.md` (23 KB) per-task token cost table.

---

## 2. DESIGN.md doctrine vs implementation reality

**Shipped & exceeded**: 5-stage compactor (✅ + ModelBackedCompactor beyond spec); HookBus (27 planned → 32 shipped); Worktree+Container sandboxes (✅ + Bubblewrap, un-planned); agentskills.io skills compliance incl. compile-time validation, `harness skills lint` (Jaccard >50% overlap flagging) and `export` to spec dirs consumable by Claude Code/Cursor; session record/replay (`SessionRecorder` → JSONL → deterministic `MockModel`, `harness replay` = zero-cost regression tests); OTel spans; MCP server + client; L1/L2/L3 loop engineering (§11.5 shipped essentially verbatim); orchestrator with replanning (§11.6 shipped verbatim); auto-fix `FixPatch::{ReplaceFile, UnifiedDiff, RunCommand}`.

**Diverged**: workspace layout (§10-crate merge, §1.1 above — doctrine doc never updated); models stack (§2 "use Rig" / §13 `RigAdapter` — **no rig dependency exists anywhere**; all providers are native implementations); Blueprint `Node::Agent{guides, tools, sensors, budget}` (§8) — shipped as an opaque `BoxedAgentFn` closure; `blueprint.rs` header admits "a first-class Node::Agent lands once we resolve the variance/lifetime story for `&mut AgentLoop` in a Box" (UNRESOLVED as of 0.0.63).

**Aspirational / not shipped**: CLI subcommands `harness diff`, `guide add`, `sensor add/promote`, `compactor stats` (§14 — absent from `harness-cli`); `harness.toml` + `harness/` config-as-reviewed-artifact (partial at best); shell-script/MCP-callable hooks (§10); sensors as separate crates (now `sensors_rust.rs`/`sensors_common.rs` modules in harness-tools).

**Shipped but absent from DESIGN** (framework outgrew its doctrine doc): browser CDP suite (~190 KB), `user_model` portrait system (74 KB), `harness-serve` HTTP/gRPC, daemon, image/speech multimodality, embeddings, experience→skill distillation loop, seal/receipt/goal machinery, `BubblewrapSandbox`, hashline-editing example (`cap`), extended guards (monotony, spill, seal, acceptance).

**Verdict**: doctrine is ~80% implemented, and where reality diverged the *code comments are better engineering writing than the design doc* — but DESIGN.md is now materially stale (crate map, provider strategy, CLI surface).

---

## 3. Matrix row — 25-capability scoring (0=absent, 1=partial, 2=shipped & solid)

Note: scored against the standard 25-capability iteration-19 list; anchors = repo paths above (sha `061bf3e`).

| # | Capability | Score | Evidence anchor |
|---|---|---|---|
| 1 | ReAct agent loop | 2 | `harness-loop/src/lib.rs` AgentLoop + streaming + cancellation |
| 2 | Tool abstraction w/ risk tiers | 2 | `harness-core/src/tool.rs` (`ToolRisk`), `#[tool]` macro |
| 3 | Built-in tool breadth | 2 | fs/shell/web/docs+OCR/sql/browser/datetime/tasks/memory |
| 4 | Shell safety | 2 | risk-gated `shell/`, background exec, process-group kill, deadlines |
| 5 | File editing | 2 | `fs.rs` 41.7 KB edit tools + `cap` hashline example |
| 6 | Context compaction | 2 | 5 stages, hysteresis, ModelBackedCompactor, never-split invariant |
| 7 | Prompt/context assembly | 2 | `Context` blocks, guides, prefix-cache (test: `prefix_cache_live.rs`) |
| 8 | Lifecycle hooks | 2 | 32-event bus, `#[hook]`, Deny/Mutate/Inject outcomes |
| 9 | Loop guards | 2 | stuck/monotony/cap-spill/acceptance/seal/goal — benchmark-ablated |
| 10 | Auto-fix (skip-model repair) | 2 | `FixPatch` ×3, `is_default_safe_fix`, Pre/PostAutoFix |
| 11 | Skills (agentskills.io) | 2 | loader/validate/lint/export + `skills_dir!` + superpowers compat |
| 12 | Subagents | 1 | `SubagentSpec`+status protocol; no parallel spawn (cooperative by design) |
| 13 | Multi-agent orchestration | 2 | Job DAG + retry/backoff + dead-letter + resume + dynamic replanning |
| 14 | Scheduler | 1 | cron-style recurring jobs, tool-exposed; lean vs Quartz-class |
| 15 | Sandboxing | 2 | Seatbelt/Docker/bwrap/worktree, honest `Isolation` reporting |
| 16 | Permission model | 1 | `permissions.rs`, L1/L2/L3 gates, REPL y/N — no per-call policy engine like CC 7-modes |
| 17 | Long-term memory | 2 | 3 layers (facts/decision/synth) + URL-scheme backends |
| 18 | Recall/RAG | 1 | FTS5/BM25/HNSW hybrid via backends; no first-party vector store (declared non-goal §2) |
| 19 | PII/privacy | 2 | regex+Luhn detectors, Label/Mask/Hash/Block, Guarded/Redacting memory decorators |
| 20 | Observability | 2 | tracing+OTel spans (`TelemetryHook`) + audit/broadcast hooks |
| 21 | Record/replay determinism | 2 | JSONL capture → `MockModel` replay; `harness trace`; golden tests |
| 22 | Provider coverage | 2 | OpenAI-compat/Anthropic/Gemini + vision/speech/embed + router/retry + Mock |
| 23 | MCP | 2 | server (`harness mcp serve`) + client + proxy |
| 24 | Evals/benchmarks | 2 | pass^k + guard ablation + trigger coverage + per-turn cost; CI-gated |
| 25 | Docs/doctrine | 1 | DESIGN+guide+benchmarks excellent but materially stale (§2 above) |

**Total: 45/50** — the highest-scoring micro-harness of this slice's weight class; loses points on subagent parallelism, scheduler depth, first-party vector recall, permissions granularity, and doc-rot.

---

## 4. ALSO-RANS SIDEBAR (30-min pass, API-level)

- **epicsagas/epic-harness — ALIVE.** 18★, Rust, Apache-2.0, created 2026-04-09, **pushed 2026-09-11 (yesterday)**; gh-verified description: "Multi-tool AI agent harness — 22 skills, self-evolving engine, unified memory, autonomous spec-to-PR pipeline. Works with Claude Code, Codex, Cursor, OpenCode, and Cline." Has a Homebrew tap (`epicsagas/homebrew-tap`, 2★, pushed 2026-09-12) — real distribution effort. Coordinator brief said "26 auto-trigger skills" — current description says 22 (count drift, UNVERIFIED which is right). Unlike harness-rs it is a *wrapper/orchestrator over existing coding CLIs* rather than a from-scratch loop; its self-evolving skill engine is the same idea as harness-rs's `SkillDistiller/SkillReviser` but shipped at the CLI layer where users already are. Bigger mindshare per engineering dollar; less depth (no compactor/sandbox/benchmarks visible at API level).
- **io-harness (embeddable runtime, durable resume) — NOT LOCATED.** GitHub search across rust+general finds no such repo (results dominated by Harness.io CI company projects). Neither `io-harness` nor plausible owners resolve. Possibly renamed, private, or deleted since the coordinator's verification pass. Flag: **UNVERIFIED / NOT FOUND** — recommend re-checking the source list before publication.
- **pi-rust/rpi-harness (pi port) — DOES NOT EXIST under that name** (`gh repo view pi-rust/rpi-harness` → "Could not resolve to a Repository"). The actual pi-harness-to-Rust port niche is held by two micro-projects: **N3mes1s/pi-rs** ("pi harness, but in rust", 0★, pushed 2026-08-06 → **DORMANT**, ~5 wks) and **JoNil-Botta/bit** ("A port of the pi harness to Rust", 0★, pushed 2026-08-18 → **DORMANT**). Both are experiments, neither has docs/releases.
- **tiny-agent — DORMANT.** No canonical Rust "tiny-agent" harness exists; the best-fitting match to the brief is **jxors/tiny-agent-loop** (0★, Rust, "a tiny LLM agent loop library for simple one-off projects", last push 2026-05-21, ~3.5 months stale) — mapping UNVERIFIED. (Python namesakes — SqueezeAILab/TinyAgent 498★, askbudi/tinyagent 50★ stale since 2025-09 — are different species.) Verdict for the Rust niche: DORMANT.
- **tiny-loop — DEAD (archived).** **DiscreteTom/tiny-loop**: 3★, Rust, "Minimal AI agent framework in Rust", `isArchived=true`, last push 2026-03-04. A teaching-grade loop, archived ~6 months in. Confirms the pattern: minimal loops get built once and abandoned when the maintenance reality hits — exactly the fate harness-rs is sprinting to outrun.

---

## 5. Notable concepts — ADOPT / AVOID

**ADOPT (micro-harness innovations worth stealing):**
1. **Honest isolation reporting** — `FsPolicy`/`NetPolicy` (requested) vs `Isolation::{None,Changes,Process}` (actually enforced), with docs stating the in-process-fs hole. The single most transferable security-API idea in the repo: sandbox APIs should be unable to lie.
2. **Trigger-coverage benchmarking** — a guard that never fired during a benchmark run is *flagged as noise instead of posing as a measurement*; plus leave-one-out guard ablation so every delta is attributable to one guard; plus pass^k as a floor (not pass@k ceiling). Best eval discipline seen at this scale.
3. **Default-off guard policies with written, measured justifications** (`MonotonyPolicy`: "the false positive is unbounded in N… a default-on version would terminate working agents, which is worse than the problem it fixes") — adopt the *documentation style* for every risky default.
4. **Spill-not-truncate** for oversized tool results (ceiling exceeded → disk, reference in context; −17% cost/pass, nothing lost).
5. **`DECISION:` marker capture** as the zero-cost middle of the memory spectrum (verbatim writer ↔ LLM synthesizer), measured noise-free.
6. **`PostCompact { before, after }`** — compaction events that report what they bought in tokens; every framework should make its compactor accountable.
7. **`SealBreached` as a first-class event** — separate from check-failure because it means "the measuring instrument was moved by the party being measured… the one event a host may reasonably want to page on."
8. **Cancellation as an `Outcome`, not an error** (`Outcome::Cancelled` carries partial work; exit 130; process-group kill; REPL re-arms a fresh token per turn).
9. **Crate-merge doctrine** — "a crate boundary has to name a dependency you might not want"; publish few crates, keep modules inside (they learned this from publishing 15 crates every ~2 days).
10. **Closed experience loop** — episodes → `SkillDistiller` → SKILL.md → `SkillUseTrace` → `SkillReviser`: self-evolution that lands in *human-readable, lintable* skill files instead of opaque weights/prompts.
11. **URL-scheme backend selection** (`sqlite://`, `cortexdb://`…) that fails loudly naming the feature to enable — never a silent fallback.
12. **Dynamic replanning** (`PlanDelta::Add` mid-DAG) as the explicit bridge from workflow engine to agent runtime, with crash-resumable per-transition persistence.

**AVOID / caution:**
1. **Shared 0.0.x versioning across 15 crates**: every release breaks every consumer's pins simultaneously and "every release is already incompatible" under Cargo 0.0.x rules (their own 0.0.63 changelog says so). Two breaking releases within recent memory (Outcome::Cancelled, memory-crate merge). Adoption stat: `harness-rs-tools` has **90 downloads** — nobody can build on sand.
2. **The DESIGN.md crate map is doctrine debt**: guide.md still names crates that don't exist (`harness-loop-engine`, `harness-orchestrator`, `harness-experience`, `harness-redact`, `harness-skills`) — the docs/prelude mismatch confuses every new user (it confused this review).
3. **`harness-loop` as a 120 KB lib.rs mega-crate** with ~40 modules — merges fixed the release treadmill but bought compile-time and discoverability debt; the boundary criterion ("names a dependency") is necessary but not sufficient.
4. **Browser CDP suite (~190 KB) + `user_model/portrait.rs` (74 KB) + image/speech** — feature creep far beyond "harness" mission for a solo maintainer; each is a project on its own.
5. **Aspirational hook transports** (shell-script/MCP hooks) advertised in DESIGN but unimplemented — don't cite harness-rs as evidence that CC-style script hooks have a Rust equivalent.

---

## 6. Solo-maintainer sustainability risks

- **Bus factor = 1, verifiably**: `contributors` API → 1 (Liang Li); 12★/3 forks; **0 open issues** — that's not low friction, that's no external traffic. Downloads (core 5.5k over 4 months, mostly dependency mirroring) suggest near-zero real adopters.
- **Release treadmill**: 63 releases in <4 months (~every 2 days), 135 KB CHANGELOG, and the treadmill is self-inflicted ×15 (15 crates re-published per change) — they already revolted once by merging 10 crates into harness-loop. Expect further consolidation.
- **Velocity-vs-audience mismatch**: the code ships bleeding-edge practice (edition 2024, MSRV 1.92 = "latest stable − 2" per DESIGN §17, days-fresh model names like gemini-3.6) at a pace only its author can follow. Nothing pins compatibility for downstream users; 0.0.x is honest but adoption-repelling.
- **Surface creep**: browser automation, multimodal (image/speech/embeddings), user modeling, daemon, serve layer — each adds a maintenance liability no second pair of eyes is reviewing.
- **Mitigants already in place** (to their credit): near-zero-dependency core; no heavy frameworks (tokio/reqwest/serde/inventory only); doctrine-first docs ("DESIGN.md itself is a Guide… so future engineers and agents can decide correctly without reading all the source"); benchmarks as CI gates; examples as workspace members (dogfooding); exceptionally rationale-dense CHANGELOG. If the maintainer stops, the repo freezes into a well-documented time capsule rather than rotting silently — the doctrine-writing was the right hedge. But "well-documented, single-committer, 0.0.x" is the classic profile of a brilliant abandoned project; treat as study material, not infrastructure.

**Bottom line**: harness-rs is the most intellectually serious micro-harness in this slice — a maximal framework whose guards, compactor, sandboxes, and eval discipline are all *measured*, with an honesty policy (isolation, cancellation, benchmarks) worth adopting wholesale. Its risks are the classic maximalist-solo ones: versioning that repels users, doctrine docs already behind the code, and a surface area growing faster than one person can hold.
