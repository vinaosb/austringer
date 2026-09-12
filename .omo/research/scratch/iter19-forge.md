
# Iteration 19 — Adulari/forge (Rust micro-harness, deep detail)

**Repo:** https://github.com/Adulari/forge · clone: blob:none @ $self (HEAD 24bdcb8, 2026-09-12)
**Verified metadata:** 8★, 3 contributors, created 2026-06-14, pushed 2026-09-12, AGPL-3.0-only, v2.15.0, author Floris Voskamp (solo + 2). Rust LOC ≈ 264,762 across 18 crates (**not ~13 as briefed — workspace grew**), 3,204 #[test]/#[tokio::test] fns counted by grep (forge-core 701, forge-provider 354).

---

## 1. Architecture narrative

### 1.1 Crate graph (18 members, crates/*)
Declared in [https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/Cargo.toml#L1-L40](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/Cargo.toml#L1-L40): forge-types (dependency-free leaf: Message, TaskTier, PermissionMode, SideEffect) → forge-config (figment layers + keyring) → forge-provider (Provider trait; genai fork vendored as orge-agent-genai 0.6.5-forge.1 — pinned =0.6.5-forge.1, path endor/genai-0.6.5; CLI bridges claude/codex/gemini + codex-oauth/xai_oauth) → forge-mesh (routing) → forge-tools / forge-mcp / forge-lsp / forge-index (Lattice) / forge-skills → forge-store (rusqlite bundled, WAL) → **forge-core** (orchestrator + permission broker; ~60 modules incl. model_loop, permission/, subagent/, tool_dispatch/) → surfaces: forge-tui (ratatui + headless behind one presenter interface), forge-cli (thin binary), plus forge-proxy, forge-relay, forge-anywhere-protocol (E2EE chacha20poly1305/ed25519/x25519 remote control), forge-device, forge-voice, forge-workflow, forge-browser. Dependency direction is one-way, acyclic, compiler-enforced ([ADR-0002]). **Crates.io names are orge-agent-*** (forge-* taken) with lib names preserved as orge_X so imports don't change — a clever publish workaround documented in Cargo.toml comments.

### 1.2 Loop design — Session::run_model_loop ([https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-core/src/model_loop.rs#L15-L190](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-core/src/model_loop.rs#L15-L190))
One shared inner loop for primary turns AND autofix re-runs, with a **dense guard lattice** (each bounded, named, test-pinned): doom-loop on identical tool batch (last_tool_sig+epeat_count), sliding-window ecent_sigs VecDeque catching A,B,A,B oscillation that defeats consecutive-repeat checks, failure-loop keyed by (tool, ErrorCategory) across changing args, empty-response nudges, continue-nudges w/ progress capture (
udge_policy::decide), followup-intent nudge ("Let me verify:" w/ no tool = unfinished), prose-tool-call recovery (<invoke> written as text — they observed a **553× spiral** live), phantom-edit nudge, bridge continue/stall nudges, stall narration tracker, verification gate ("done" accepted only with **fresh tool-grounded evidence newer than last mutation**; inspect_ran atomic counts real inspection tools), step caps (attended pause vs unattended warn→hard ceiling→error), per-turn bridge token accumulator, overflow compact-retry bound, per-model transient-retry bound, pinned-model rate-limit backoff with SEPARATE attempt budgets for rate-limit vs outage (comment: the two failure modes "can both occur in the same turn without starving each other"). Failover chain iterates decision.fallbacks; pinned turns never fail over. This file's comments are a masterclass in guard-rationale documentation.

### 1.3 Permission broker ([ADR-0008](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/docs/architecture/decisions/0008-tool-permission-modes.md) + [https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-core/src/permission.rs#L1-L226](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-core/src/permission.rs#L1-L226))
Single chokepoint; pure functions (decide_mode, decide) with an explicit 6-step precedence doc-tested: (1) **Builtin deny floor beats everything incl. ypass**; (2) any deny wins; (3) plan mode is a hard read-only contract no allow can escape; (4) **shell commands evaluated per effective segment** — llow="git *" on segment 1 cannot smuggle git status && <anything>; allow only when *every* segment parses+allows, any ask prompts; (5) most-specific allow/ask by specificity score (exact tool +1000, literal chars in pattern); (6) mode fallback. Shell extraction (permission/shell_commands.rs) unwraps ash -c, ;/&&/| chains, wrapper binaries; builtin denies ALSO match the raw command line (catches curl | sh and hidden tokens), and **any multi-line command hits a conservative literal-token floor** — "newlines are where indirection hides" ([https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-core/src/permission.rs#L157-L180](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-core/src/permission.rs#L157-L180)). SideEffect classes: ReadOnly/Write/Shell/Network/**External** (ADR-0009: MCP = untrusted server code, distinct from benign network reads; gated even in accept-edits). Domain language later renamed modes to "Temper" (Read-only/Ask/Auto-edit/Full — CONTEXT.md). Decisions persisted with session (audit).

### 1.4 Model Mesh routing ([ADR-0006](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/docs/architecture/decisions/0006-model-mesh-rule-based-routing.md), normative doc docs/features/mesh-routing.md 1,276 lines)
Deterministic 8-step pipeline, **zero model calls spent on routing** (opt-in LLM classifier behind Router trait): classify tier (Trivial/Standard/Complex) via weighted lexical heuristic (score ≤0 trivial, ≥5 complex; COMPLEX_HINTS like "ultrathink" short-circuit to Complex; TRIVIAL_PATTERNS "typo/rename" −8; self-hosting infra terms +5 only when ProjectContext::is_self_hosting) → RouteHints (code_heavy + per-prompt seed) → candidate set from discovered catalog → **conservation decision** → oute_score every candidate → effort-dependent ordering + tie-breaks → usability filter (keys/health/quota/credits/context-window/vision) → pick = first usable, **rest = failover chain**. Scoring ([https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-mesh/src/catalog.rs#L363-L411](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-mesh/src/catalog.rs#L363-L411)): capability_score_b (quality from Artificial Analysis index/20 when benchmarks cached, else family-substring quality_class with param-count overrides: -100b→3 before -mini→1, narrow families codellama/palmyra→2, [https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-mesh/src/capability.rs#L21-L111](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-mesh/src/capability.rs#L21-L111)) + per-tier cost_pref + coding/provider prior − tool_reliability_penalty − **subscription burn penalty** − pool-capacity penalty − quota-status penalties (Exhausted −100.0; Warning −5.0 soft demotion with an extensive comment explaining why a same-tier capability gap defeats a 5-point penalty — a real incident). Conservation ([https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/catalog.rs#L445-L464](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-mesh/src/catalog.rs#L445-L464)): probability of routing a whole prompt OFF subscriptions onto free-frontier — Trivial always 1.0; Complex code_heavy **0.0 while healthy** (never trade quality) ramping 50%→80% window; pace-projected fractions (QuotaPace) react to burn-rate before overrun. Failover chain follows **mesh rank verbatim, not provider interleave** (a prior round-robin destroyed rank order — named regression); rate-limit ≤75s is *waited out* rather than degraded. Rationale string persisted per turn in outing_decision table = ground truth.

### 1.5 Session/store design
forge-store = 30 store modules (sessions, transcripts, usage, quota, spending, model_health, provenance, duel, fleet, checkpoint, fork, handoff, lattice, memory, sync_journal…) over one bundled-SQLite DB in WAL. 5 core entities (session/message/tool_call/routing_decision/usage) per  2-architecture.md data model; single-writer serialized writes; blocking isolation via spawn_blocking ([ADR-0005](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/docs/architecture/decisions/0005-persistence-rusqlite-bundled.md); stays on rusqlite 0.39 for MSRV, **not** 0.40 — Cargo.toml says 0.40, ADR says 0.39; workspace actually pins usqlite = "0.40" now — minor doc drift). [ADR-0014](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/docs/architecture/decisions/0014-bounded-local-storage-growth.md) bounds unbounded growth: CI cache budgets (16GiB targets/24GiB docker/4+4+1GiB npm) + sync-journal pruning ≤1000 rows/pass keeping newest revision as logical clock; measured incident: 1.5GiB forge.db w/ 40,480 acknowledged journal rows; explicit VACUUM only, never in daemon.

### 1.6 TUI structure (forge-tui)
ratatui 0.30 + crossterm 0.29 behind a **presenter/event seam in core** ([ADR-0004](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/docs/architecture/decisions/0004-tui-ratatui-crossterm.md)) so headless line/JSON renderer + remote/mobile surfaces are adapters. Subdirs: app/ (render/{input,live,overlays,question,status_line,status_widgets}, tool_cards), board/ (live project board over all sessions — latest feature #1361), commands/, render/. Notable engineering: ratatui features unstable-rendered-line-info + scrolling-regions with a Cargo.toml comment explaining that insert_before without scroll-regions issues ESC[6n DSR queries that race the event-read loop and **freeze the whole TUI** (Cargo.toml#L58-L67). BPE-accurate token display via bundled tiktoken (offline o200k/cl100k). Prompt history persists across restarts (#1353).

---

## 2. ADR archaeology — 14 ADRs (briefed as 8; 6 added since 2026-06-16)

| ADR | Decision | Notes/reversals |
|---|---|---|
| 0001 (06-14) | Rust+Tokio over thread+channels, async-std | rejected async-std as "unmaintained for 2026" |
| 0002 (06-14, clarified 07-17) | modular monolith, single binary, workspace crates | **clarification = partial reversal**: "single binary" ≠ fully static (was over-read) |
| 0003 (06-14) | own Provider trait, genai as one impl, lock-in contained | later **vendored genai fork** (parallel-tool-call loss + sibling gap-fill bug fixed in  .6.5-forge.1; drop when upstream ships) — escalation of control ADR anticipated |
| 0004 (06-14) | ratatui+crossterm, headless via presenter seam | holds; scroll-region feature flags added later |
| 0005 (06-14) | rusqlite bundled over sqlx/ORM; WAL | version note: stayed 0.39 for MSRV 1.88; workspace now shows 0.40 (drift) |
| 0006 (06-14) | rule-based heuristic router, pluggable LLM classifier off by default | holds; became far richer (benchmarks, quota, conservation) |
| 0007 (06-14) | figment layered config; secrets env→keyring, NEVER TOML | holds |
| 0008 (06-14) | modes+rules broker, deny-wins, plan-overrides | holds; External class added by 0009; "Temper" rename |
| 0009 (06-16) | forge-mcp crate, rmcp client both directions, manager-owned (no ToolRegistry mutation), SideEffect::External, deferred tool loading | rejected mutable ToolRegistry for zero hot-path change |
| 0010 (06-16) | tree-sitter **tags queries** (GitHub code-nav mechanism) multi-language | **contains a real downgrade reversal**: PR1 used ts 0.26 → downgraded to 0.24.x because typescript/java/cpp/ruby grammars have no 0.25+ release; refs stored **by name not dst_id** because CASCADE-on-reindex rots resolved edges — a schema lesson |
| 0011 (06-18) | Artificial Analysis benchmark layer over name-heuristic (index/20 maps to old quality scale) | rejected LMArena/OpenRouter/aider/HF boards with reasons; graceful fallback |
| 0012 | hosted APNs relay (opt-out) for iOS push | first hosted component — noted as the only plausible service in ADR-0002 |
| 0013 (07-29) | resource-bounded LSP: ONE live server tree, 250ms RSS sampling of descendants, kill children-first at 2GiB, rust-analyzer cripple profile (CARGO_BUILD_JOBS=1, no proc-macros, LRU 128→32) | evidence-driven: incident 3.7GiB/37 procs/14 cores → post-fix 1.67GiB/4 procs/1 core; rejected cgroups (not portable), RLIMIT_AS (VM≠RSS) |
| 0014 (07-30) | bounded cache + sync-journal growth | rejected age-only cleanup (actively-used target never goes stale) |

Plus docs/archaeology/ — **56 per-behavior archaeology notes** (core-tool-dispatch, session-lifecycle, prose-recovery, quota-stall incident…) — an unusual "why is this code weird" corpus.

**Benchmark reversals** (results.md): the original "bridge token-efficiency superiority" claim was retracted 2026-06-26 as an **accounting bug** (bridge counted only uncached input; looked 10–150× cheaper; fair recount showed 1.9× MORE tokens) — preserved in-place with strikethrough-style honesty. Batch-tools experiment **reverted** (triggered prose spiral).

---

## 3. Benchmark-claims verification

**Methodology** ([swe-bench.md](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/docs/benchmarks/swe-bench.md)): generate predictions in-repo (orge bench swe clones repo@base_commit, one headless turn in Bypass mode, records diff; failures = empty patch not abort) → score with **official swebench Docker evaluator, deliberately NOT reimplemented** ("re-rolling would make our numbers non-comparable") → matched comparisons: same instances, same model family, --agent swaps harness only → metrics sidecar w/ untime identity block (cli_version, resolved model, transport, tool_aliases) → metrics_complete=false rather than inventing numbers → multi-seed pass@k → history-safe hardened runner (removes remotes+future objects, one synthetic base commit, integrity rule, per-cell state, trace audit, one quota-bearing arm per refresh, **83-cell validity ledger**).

**The "+50%" claim** traces to results.md §re-confirmation (v0.4.65): **6/10 vs 4/10 resolved = +50% relative**, strictly dominating (every CLI solve also Forge's, 2 Forge-only, 0 CLI-only) at ~21% lower tokens/resolve. VERIFIED as written — but: N=10, self-reported, no third-party replication, and the doc itself calls it "a signal… not a fresh large-sample proof". Earlier N=20 was 11 vs 9 (+22% rel) with a tied second batch. **Latest canonical (2026-07-27)**: 2 (two!) SWE-bench Verified tasks, 6+4 pairs — Forge 3/6 vs Codex 0/6, 2/4 vs Claude Code 1/4, mesh 1/2; cache-adjusted −12.45% vs Terra disclosed as exception. Statistically near-meaningless N; methodologically exemplary honesty. The **conformance claim** ("324 tests" badge): actual grep counts 3,204 test fns workspace-wide, 701+354 in the two crates the badge's doc references — claim is a stale-but-conservative snapshot (UNDER-claims). **UNVERIFIED:** <100ms startup; Lattice "19 languages" (README) vs ADR-0010's ten initial languages (likely grew; not counted).

---

## 4. Matrix row (25 capabilities; ● strong w/ evidence, ◐ partial, ○ absent)

| # | Capability | Score | Evidence |
|---|---|---|---|
| 1 | Agent loop architecture | ● | model_loop.rs guard lattice, permalinks §1.2 |
| 2 | Tool layer/schema | ● | Tool trait w/ side_effect class + JSON schema; tool_dispatch/, mcp dispatch |
| 3 | Permission model | ● | ADR-0008 + broker precedence + persisted decisions |
| 4 | Shell-command safety | ● | segment unwrap, builtin floor, multi-line conservative deny |
| 5 | MCP client | ● | forge-mcp crate, rmcp, deferred exposure, External class |
| 6 | MCP server exposure | ● | orge mcp-serve (rmcp server side) |
| 7 | Multi-provider abstraction | ● | Provider trait + vendored genai fork + 4 CLI bridges + OAuth |
| 8 | Routing/model selection | ● | Model Mesh — deepest anywhere: tiers, conservation, quota, pace |
| 9 | Failover/resilience | ● | ranked chain, 75s RL wait, pinned backoff budgets, health notices |
| 10 | Cost/token accounting | ● | tiktoken BPE, 5 usage stores, corrected cache accounting |
| 11 | Context mgmt/compaction | ◐ | compaction_policy.rs, context_pipeline.rs, overflow window cap — present, not benchmarked |
| 12 | Code intelligence | ● | Lattice tree-sitter tags queries, impact/path, auto-injection; lang-count claim UNVERIFIED |
| 13 | LSP integration | ● | forge-lsp w/ RSS guard + cripple profile — only harness with bounded LSP |
| 14 | Subagents/orchestration | ● | subagent/, detached, fleet, /duel arena, forge-workflow, assays |
| 15 | Cross-session memory | ● | memory.rs: typed facts, turn-end capture, relevance-ranked reinjection |
| 16 | Session persistence/resume | ● | WAL SQLite, 30 stores, bridge --resume, snapshot/replay/fork |
| 17 | Provenance/audit | ● | orge blame line→model/prompt/reasoning; provenance_store — unique |
| 18 | Skills | ● | forge-skills, Claude-Code-format compatible, tier: frontmatter routing |
| 19 | Headless/automation | ● | presenter seam, headless.rs, remote protocol, OpenAI-compatible local API (forge-proxy) |
| 20 | TUI | ● | ratatui deep integration (scroll-regions freeze fix), board, overlays, qrcode pairing |
| 21 | Test discipline | ● | 3,204 fns; conformance suite drives real loop w/ mock providers; 5,000-case fuzz corpus |
| 22 | Benchmark honesty | ● | retraction preserved, cell-validity ledger, "where Forge does NOT win" section |
| 23 | Docs discipline | ● | 14 ADRs, living architecture doc, 56 archaeology notes, doc-sync TEST |
| 24 | Security posture | ● | deny floor, External class, keyring, E2EE protocol, threat model, SECURITY.md |
| 25 | Packaging/distribution | ● | single binary, brew Formula, install.ps1/sh, desktop (Tauri?) + Expo mobile |

Score: 24● / 1◐ / 0○. Strongest rows vs big-agent baseline: 8, 10, 13, 17, 22, 23.

---

## 5. Notable concepts + ADOPT/AVOID vs ledger

**ADOPT:**
1. **doc-sync test** ([https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-mesh/src/doc_sync.rs#L1-L7](https://github.com/Adulari/forge/blob/24bdcb89302143ef5b9d09f2ce6aad52644ccc8e/crates/forge-mesh/src/doc_sync.rs#L1-L7)): a test reads the normative doc and asserts every constant matches live symbols; fails naming constant+value+doc path. Kills doc rot mechanically. Cheap, huge payoff — micro-harness DOES this better than any big agent.
2. **Builtin deny floor outranking ypass** + multi-line conservative shell floor — safety as structural guarantee, not convention.
3. **Conformance suite**: deterministic mock-provider tests driving the REAL loop asserting each failure-mode guard (553× spiral → named test). Every guard has a cargo test.
4. **Retraction-in-place discipline**: wrong benchmark claims stay visible with correction, plus "Where Forge does NOT win" section.
5. **Rank-verbatim failover chain** (not provider round-robin) + wait-out short rate limits instead of degrading.
6. **Quota as a first-class currency** alongside USD: pace-projected windows, pool capacity, burn weights, conserve-probability per tier (complex+code_heavy=0.0 while healthy).
7. **Refs-by-name not dst_id** in any code graph (CASCADE rot argument).
8. **Resource-bounded LSP** pattern (RSS-sampled process-tree guard).

**AVOID:**
1. **265k LOC / 18 crates for a "micro" harness** — forge-core alone ~60 modules; scope (mobile, voice, E2EE relay, desktop) dwarfs most teams. Their own codebase-size-analysis-2026-07.md admits it.
2. ** Vendored provider-SDK fork** — right call for them (solo, upstream unresponsive) but a maintenance millstone; only vendor with a documented exit condition (they did: "Drop once upstream ships").
3. **Lexical tier classification brittleness** — substring tables (-100b, palmyra) age poorly; their own ADR-0011 concedes this; benchmark layer mitigates but heuristic remains load-bearing.
4. **Huge single-file modules** (mesh lib.rs 6,272 lines; permission.rs 1,379; catalog.rs 2,898) — crate seams good, intra-crate seams eroding.

**What a micro-harness does BETTER than the big ones:** (a) honest self-measurement with retraction trail; (b) doc-code sync enforcement; (c) subscription-bridge harness-comparison methodology (drive claude/codex CLIs THROUGH your harness to isolate harness-vs-model — nobody else can run this experiment); (d) guard-rationale comments at point of incident (every magic constant cites the bug that birthed it).

---

## 6. What our Rust harness should learn (codebase organization)

1. **types-first leaf crate** (forge-types, zero deps) — makes acyclicity trivially enforceable; every cross-cutting enum (TaskTier, PermissionMode, SideEffect) lives there.
2. **One trait-fronted crate per domain reason-to-change** (talk-to-model / choose-model / do-things / remember / show / configure), binary is thin wiring only.
3. **Pure-function safety cores**: permission broker = decide(mode, side_effect, tool, args, rules) -> Decision with zero I/O → exhaustive table tests; UI/store wrap it.
4. **Normative feature doc + doc_sync test** for any subsystem with tunable constants (start with routing/pricing).
5. **Per-behavior archaeology dir** (docs/archaeology/) — one short note per weird behavior: symptom, root cause, guard, test name. Cheaper than tribal memory.
6. **Comment discipline as spec**: every magic number carries Documented in <doc> + the incident that set it (e.g. −5.0 Warning penalty comment recounting the near-exhausted-subscription routing incident).
7. **Workspace-profile hygiene**: debug = "line-tables-only" because full debuginfo piled "hundreds of GB" in target/deps (Cargo.toml#L108-L113) — mirror ADR-0014's bound-growth mindset from day one.
8. **AGENTS.md** at repo root stating verified check commands incl. the FORGE_DB isolation rule ("has taken the user's daemon down four times") — agent-facing ops lore, exactly our pattern.

**Sources:** all paths above at $self; ADRs docs/architecture/decisions/; docs/benchmarks/{swe-bench,results,history-safe-pinned-mesh-2026-07,cell-validity-2026-07}.md; docs/harness/why-forge-is-a-better-harness.md; docs/features/mesh-routing.md; metadata via gh api repos/Adulari/forge (verified 2026-09-12).
