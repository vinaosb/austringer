# austringer

[![CI](https://github.com/vinaosb/austringer/actions/workflows/ci.yml/badge.svg)](https://github.com/vinaosb/austringer/actions/workflows/ci.yml)
[![docs](https://github.com/vinaosb/austringer/actions/workflows/docs.yml/badge.svg)](https://github.com/vinaosb/austringer/actions/workflows/docs.yml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**One small Rust daemon runs an AI coding agent; four thin clients attach to it — terminal, TUI, browser, VS Code — under hard, CI-enforced performance budgets, and every design decision is a receipt from a studied post-mortem of another harness.**

- Pre-release (0.1.x, workspace scaffold through wave 9). `v0.1.0` (tagged
  2026-09-21) is the first snapshot, cut when the release pipeline went
  green; see [Status](#status) for exactly what works at this commit and
  [Release transparency](#release-transparency) for what every tag
  publishes.
- The claims on this page cite their evidence: research sections refer to
  [.omo/research/harness-research.md](.omo/research/harness-research.md)
  (the in-repo study of 25+ harnesses this project is built from); shipped
  mechanics cite the commit that landed them and the recorded task evidence.

**Docs:** this README is the storefront; the composed manual is built from
`docs/` by CI and served at <https://vinaosb.github.io/austringer/>. Crates
publish as `austringer-*` (API docs on docs.rs) — see [Naming](#naming).

**Contents:** [Why](#why) · [Quickstart](#quickstart) ·
[Four frontends, one engine](#four-frontends-one-engine) ·
[Performance budgets](#performance-budgets) · [Architecture](#architecture) ·
[Receipts](#receipts) · [Built on post-mortems](#built-on-post-mortems) ·
[Honest comparison](#honest-comparison) ·
[Release transparency](#release-transparency) · [Status](#status) ·
[Roadmap](#roadmap) · [Naming](#naming) · [Contributing](#contributing) ·
[License](#license)

## Why

Most agent harnesses rediscover the same failures — silent model substitution,
unbounded context growth, orphaned processes after a crash, sandbox escapes
treated as noise, benchmark numbers that collapse under matched conditions.
This project's approach is to not improvise: each mechanism was adopted (or
explicitly refused) because a documented success or post-mortem in another
harness proves the choice, and the receipt is written down before the code is.

The corollaries:

- **No silent model substitution.** Routing may propose; a fallback hop asks
  the user first, every time. Model identity is a contract.
- **Budgets are the contract, not aspirations.** Cold start, RSS, frame rate,
  and binary size are enforced by a CI gate against committed baselines
  (research §23 frame-budget discipline; the gate lives in
  [austringer-bench](austringer-bench/budgets.json)).
- **Claims carry receipts.** No marketing claim on this page lacks an in-repo
  citation. Benchmark tables publish austringer-matched rows only; unmatched rows
  are marked DO NOT PUBLISH (research §18, the Qwen README discipline).
- **Windows is first-class.** ConPTY pathologies, Job-Object kill trees, and
  restricted-token sandboxing are built in, not ported later.

## Quickstart

Verified executable at the current commit (see
[docs/release/leaderboard.adoc](docs/release/leaderboard.adoc) for the run
environment; on Windows the binaries are `target\release\*.exe`):

```sh
git clone https://github.com/vinaosb/austringer && cd austringer
cargo build --release -p austringer-daemon -p austringer-cli -p austringer-tui
./target/release/austringer-daemon   # loopback-only; prints its URL and writes the auth token
./target/release/austringer doctor   # health-check config, sandbox, MCP, sessions, env
cargo nextest run --workspace     # full test suite: hermetic, zero API keys, zero network
```

Talking to a live model is opt-in per daemon (the key value itself never
appears in any committed file). Environment variables the daemon reads:

| Variable | Meaning |
| --- | --- |
| `austringer_PROVIDER` | provider family: `anthropic` \| `openai` \| `google` \| `xai` \| `compat` |
| `austringer_MODEL` | model id from the seeded catalog (effort may be pinned with the `model[effort=xhigh]` bracket grammar) |
| `austringer_PROVIDER_KEY_ENV` | name of the environment variable holding the API key |
| `austringer_PROVIDER_BASE_URL` | `compat` family only: the OpenAI-compatible base URL |
| `austringer_PROVIDER_TIMEOUT_SECS` | inference timeout in whole seconds |
| `austringer_SESSION_COST_CAP_USD` | per-session spend cap in decimal dollars (default 20.00); warning at 80%, typed stop at 100% |
| `austringer_TEST_COMMAND` | user-global override of the `run_tests` tool's framework detection (a repo-local override file is refused with a typed error) |
| `austringer_TELEMETRY` | OpenTelemetry opt-in (`1`); requires the config arm and an endpoint — never on by default |

An unconfigured daemon stays fully servable and answers inference with a
typed, actionable error naming those keys.

## Four frontends, one engine

The daemon owns the agent loop, sessions, permissions, and the event log. Every
frontend is a thin client over the same protocol (HTTP/WS/SSE on loopback plus
a local-socket IPC carrier):

| Frontend | What it is | Entry point |
| --- | --- | --- |
| CLI | headless `run`/`capture`, REPL, `attach` event tail, `doctor`, `--output-json` NDJSON for eval drivers | `austringer-cli` |
| TUI | event-driven ratatui interface: context gauge, cache-hit and cost HUD, omnibar (`@file`, `/help`, `/steer`, `/cancel`, `/vote-cancel`, `/mode`, `/rewind`, `/skill`), hunk review gates | `austringer-tui` |
| Web | dashboard SPA served by the daemon itself (no node server): live session, hunk review, cost/cache/context dashboards | daemon URL in a browser |
| VS Code | TypeScript thin client: native diff decorations, per-hunk code lenses, editor telemetry (opt-in), command palette | `austringer-vscode` |

Multiple clients can attach to one session simultaneously; a permission granted
from any client applies to all (crush's shared permission queue, research §1).

## Performance budgets

Budgets are normative and CI-enforced
([austringer-bench/budgets.json](austringer-bench/budgets.json)); the gate fails the
build on budget breach or a >10% regression against committed deterministic
baselines ([austringer-bench/baselines/](austringer-bench/baselines)). The
"measured" column below was recorded on the dev machine and is re-measured by
CI on every push; the budget column is the contract.

| Metric | Budget (contract) | Measured (dev machine, task-43 evidence) |
| --- | --- | --- |
| CLI cold start | < 50 ms | 17.23 ms |
| Daemon RSS at ready | < 80 MB | 6.69 MB |
| TUI p99 frame render | < 16 ms | 0.054 ms |
| Frame budget / hang threshold | 10 ms / 100 ms classified | 0 breaches, 0 hangs |
| TUI idle renders | <= 1 per idle period | 0 |
| Binary size (CLI / daemon, stripped) | < 40 MB | 6.21 MB / 3.09 MB |

Deterministic counters are baselined and regression-gated: repomap 965 tokens,
eviction projection 523 events, prompt stable-prefix 570,834 bytes, PTY
observation 10,592 bytes, session page-in 103,500 bytes — plus 12 dhat
allocation metrics. All numbers above are reproducible:
`cargo run --release -p austringer-bench --bin perf-gate` re-measures them.

## Architecture

One daemon, a 25-crate Cargo workspace, four thin clients:

```
                     ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
   thin clients      │ austringer- │ │ austringer- │ │  web SPA │ │ austringer- │
                     │   cli    │ │   tui    │ │ (daemon- │ │  vscode  │
                     └────┬─────┘ └────┬─────┘ │  served) │ │ (TypeScr)│
                          │  WS/SSE/HTTP (loopback, token) + local IPC  │
                          └──────┬──────┴─────┬──────┴──────────┬──────┘
                                 └────────────┴─────────────────┘
                                              │
                                       austringer-daemon
                     (event log, sessions, permissions, scheduler, sandbox)
                                               │
        ┌───────────────┬──────────────┬──────┴──────┬───────────────┬────────────┐
        │ engine        │ models       │ context     │ exec + safety │ extension  │
        │ austringer-core  │ austringer-     │ austringer-ast │ austringer-exec  │ austringer-mcp│
        │ austringer-tools │  providers   │ austringer-    │ austringer-vfs   │ austringer-   │
        │ (events, loop,│ austringer-     │  repomap    │ austringer-      │  skills    │
        │  tools, com-  │  intel       │ austringer-    │  snapshots    │            │
        │  paction)     │ austringer-     │  search     │ austringer-safety│           │
        │               │  orchestration│ austringer-lsp│ austringer-wasm  │           │
        └───────────────┴──────────────┴─────────────┴───────────────┴────────────┘
      persistence: austringer-sessions   media: austringer-media (daemon-served)
      meta: austringer-test-support, austringer-bench, austringer-evals
```

Crate-by-crate map with the plan decision each implements:
[docs/architecture.adoc](docs/architecture.adoc).

## Receipts

Design receipts — every row names the harness that proved the pattern and the
failure it exists to avoid, both cited from the
[research file](.omo/research/harness-research.md):

| Feature | Proven by (research §) | Failure we avoid (research §) |
| --- | --- | --- |
| Third-party skills quarantined: content-hash scan, lock.json ledger, activation only through trust gates | hermes 5-mechanism loop gated by write_approval (research §27) | ClawHub ClawHavoc malicious-skill distribution (research §27) |
| Registry items are declarative config patches; pin + hash verify; tampered pack refused | hermes lock.json content-hash ledger (research §27) | undetected tamper/scope drift in installed extensions (research §27) |
| Model fallback asks, never substitutes; profile stickiness across resume | Plandex 9-role routing + per-task profile pinning (research §13) | Droid silent failover (research §16); Claude Code silent downgrades #40269 (research §2) |
| Per-model protocol dialects + 400-trap feature-detection table (feature-detect per model, never per provider) | elicitation SET/PROMPT/THROTTLE constitution (research §33) | dsh thinkingFormat bug — 72k-char reasoning bloat from a wrong dialect (research §6) |
| Unattended runs end through an adversarial completion gate with CONFIRMED / REFUTED / INCONCLUSIVE verdicts + evidence links | Grok /goal evidence-review pattern (research §15); Droid /verify (research §16) | prose-spiral "done" claims on autonomous runs (research §15) |
| Evals publish austringer-matched rows only; unmatched rows render DO NOT PUBLISH | Qwen Code README exhaustive-disclosure discipline (research §18) | +10-25pt harness inflation in self-reported numbers (research §18) |
| pass^k floors reported beside Mean@k; failure-fingerprint vectors | forge + harness-rs micro-harness discipline (research §26) | the 24pt Mean@k/Pass^k consistency gap read as quality (research §26) |
| Evaluation unit is the model x profile pair (locked profile for regressions; >= 2x2 grids for experiments) | joint model x profile evaluation (research §33) | single-model results with up to 7.8x harness variance unattributed (research §33) |
| 10 ms frame budget with hang classification as a CI gate | Zed GPUI frame-budget discipline (research §23) | perf regressions landing as benchmark noise (research §23) |
| Byte-stable prompt prefixes + cache-violation tracker make cache misses observable events | hermes cache-control engineering (research §27); jcode stable-prefix hash chain (research §32) | silent cache-miss cost spikes (research §32) |
| Sandboxed runs report requested-vs-enforced isolation honestly | harness-rs isolation reporting (research §26) | vendor-escape domains hidden inside a "sandbox" proxy (research §16) |
| Private security reporting channel shipped with the scaffold | dsh missing-SECURITY.md lesson (research §6) | no private channel when the first escape report arrives (research §6) |
| Daemon + thin clients as the day-one shape | the fall-2026 convergence: OpenCode 2.0 daemon-by-default, Codex `/daemon` (rust-v0.163.0, 2026-10-15), Amp shared runners (2026-09-17–24), Cursor agent-host distribution (2026-10-01) (research §34.2) | rewriting the architecture mid-course under ecosystem pressure — OpenCode's plugin-API break cost (research §34.3) |
| Model pinning: workflow-level pins + profile Stickiness across resume | Claude Code 2.1.296 workflow-level model pinning (2026-10-09); Plandex per-task profile pinning (research §13) | silent model drift mid-task — Claude Code downgrades #40269, Droid silent failover (research §2, research §16) |
| Team status precedence: InProgress > Blocked > Error > Cancelled > Success plus WaitingForEvents; never terminal while a child runs | Warp PR #11367/#12084 (UI/UX pattern catalog 2026-10-20, research §35.6) | an aggregate flipping to Success while a child is still blocked |
| Collapsed-by-default transcripts: one line per tool, expand on demand, diffstats on the collapsed line | five-project convergence — Amp, Antigravity, Claude Code, Devin, goose (UI/UX pattern catalog 2026-10-20, research §35.6) | wall-of-noise transcripts burying decisions under raw tool output |
| Changed-files timeline per turn | OpenCode #55064 Changed-files-by-turn; Devin Session Insights (research §22.3) | an unanswerable "what did this turn touch?" at review time |
| task(category) delegation with ask-first chains | OmO categories (research §35.2–35.3); our no-silent-substitution contract (research §2) | silent chain-walking fallbacks — the exact refusal of research §35.4 |
| ACP stdio surface: austringer hostable as an editor-side agent | ACP as the LSP-of-agents — Claude Code E2E 2025-07-18, five SDKs, registry + JetBrains co-launch (research §23.3, research §23.8) | every editor reimplementing agent hosting against a private protocol (research §23.2) |
| Plan-as-artifact: opt-in plan mode, approve/edit/reject before any write | Antigravity artifact-centric UX (research §21.2); JetBrains committed `.junie/plans` artifacts + Confirm gate (research §24); Claude Code plan mode, Zed roadmap Plan Mode, Cursor Rollouts (recorded 2026-10-21, task-16) | skip-permissions surfaces auto-approving plans (Antigravity v1.3.2 receipt, task-16) |
| OSC program-status emission (777 now, 7501 alongside as it standardizes) | OpenCode #55172 + Warp + Antigravity (research §35.6 WATCH, dated 2026-10-20) | terminal tabs blind to blocked-on-permission states |
| Cross-session transcript search over the maintained event_fts index | Zed #58686 roadmap ticket (recorded 2026-10-21 — a reader over an already-maintained index, the lightweight clause); Zed threads.db lineage (research §23) | sessions as write-only archives nobody can query |
| Test-runner evidence bound into the completion gate | Zed #5242 (886 upvotes, recorded 2026-10-21); Qwen Test Failure Explorer (shipped 2026-10) | prose-spiral "done" claims on unattended runs (research §15) |
| Curated cheap-pool agents (explore, librarian) behind read-only allowlists | OmO curated agents (research §35.2–35.3) plus our utility-roles-must-be-cheap routing validation | utility roles quietly burning frontier-model budget |
| Snapshot rewind: files and history restored together, search truncated to the restore point | Zed host-side uniform checkpoints — auto-update on repo change, terminal-kill on restore (research §23.8 ADOPT) | conversation-only rewind leaving worktree and history diverged |
| Session cost governance: warning at 80%, typed stop at 100% | ANTI-RECEIPT: Zed's "cost HUD with zero budget/cap enforcement" (research §23.8 AVOID) — refused to recreate it; the micro-USD ledger is wired instead (task-33) | spend visibility without spend control — a HUD no mechanism bounds |

Shipped-at-HEAD receipts — the differentiating mechanics and the commits that
landed them (provenance: recorded task evidence in `.omo/evidence/`, cited as
task-N, internal):

| Mechanism | Commit | Evidence |
| --- | --- | --- |
| Skill Forge: quarantine + trust gates + correction journal | `bc83de6` | task-31 |
| Registry: lock.json tamper detection, pin + hash verify | `3565b43` | task-32 |
| Model profiles: per-model dialects + 400-trap table | `04a0bba` | task-34 |
| Scheduler + completion gate (CONFIRMED/REFUTED/INCONCLUSIVE) | `26e72c3` | task-41 |
| Role routing with confirmed fallback | `26edbf8` | task-42 |
| Perf budgets + regression gates + committed baselines | `c8926a4` | task-43 |
| Evals: conformance + A/B rig, austringer-matched discipline | `3fb0b07` | task-44 |

## Built on post-mortems

The research file — [.omo/research/harness-research.md](.omo/research/harness-research.md)
— is the working artifact this project was designed from: a 40-section study of
25+ harnesses (Claude Code, Codex, goose, opencode, aider, Gemini CLI, Qwen
Code, crush, OpenHands, SWE-agent, Mentat, Plandex, Pi, Grok Build, Droid,
Amp, Cursor, Copilot, Antigravity, Devin, Zed, JetBrains, Warp, forge,
harness-rs, hermes, Continue, jcode, and more), each with a functionality
matrix (research §1), git-history reversal lessons (research §2), and an ADOPT/AVOID/WATCH
decisions ledger (research §3). It stays in the repository: every adopted or avoided
decision remains traceable to its evidence. The docs-site companion page
([docs/research-companion.adoc](docs/research-companion.adoc)) indexes it.

## Honest comparison

Factual rows only, quoted from the research file's matrix and reversal
lessons; "austringer" column reflects this project's design (research §1
"Ours" column) and current status. All four comparators were studied at the
cited sections.

| Dimension | Claude Code | Codex CLI | goose | opencode | austringer |
| --- | --- | --- | --- | --- | --- |
| Language / runtime | TS (Bun), ~0.5-1 GiB steady RSS (research §1, research §2) | Rust (research §1) | Rust (research §1) | TS (Bun), heavy-ish (research §1) | Rust workspace; < 80 MB daemon RSS budget, CI-gated |
| OS sandbox incl. Windows | partial — no Windows (research §1) | yes, 3 OS (research §1) | partial — mac + Docker (research §1) | none (research §1) | OS-native per backend incl. Windows restricted tokens + ACLs + Job Objects (research §1) |
| Model substitution | silent downgrades, #40269 (research §2) | — (research §1 permission tiers) | — | — | fallback requires confirmation; ask-never-substitute |
| Sessions store | JSONL (research §1) | rollouts (research §1) | SQLite (research §1) | SQLite (research §1) | SQLite + encryption at rest (XChaCha20-Poly1305) + lazy paging (research §1) |
| Prefix-cache discipline | strong (research §1) | partial (research §1) | partial (research §1) | partial (research §1) | byte-stable prefix + violation tracker (research §27, research §32) |
| Repo map (AST/PageRank) | partial (research §1) | none (research §1) | none (research §1) | partial (research §1) | CSR + SIMD PageRank repomap (research §1) |
| Evals transparency | internal (research §1) | internal (research §1) | harbor (research §1) | none (research §1) | austringer-matched published tables; unmatched = DO NOT PUBLISH (research §18) |

## Release transparency

Per todo-44 discipline, every release publishes a benchmark leaderboard and
an eval table, austringer-matched rows only:

- Bench leaderboard (current pre-release baseline):
  [docs/release/leaderboard.adoc](docs/release/leaderboard.adoc)
- Eval tables and publication rules:
  [docs/release/evals.adoc](docs/release/evals.adoc)

## Status

Works at this commit (HEAD `63bc222`; every production-wired row is verified
against the daemon/CLI/TUI/web/VS Code source and carries recorded task
evidence in `.omo/evidence/`). The distinction that matters to a reader:
**production-wired** means the mechanism is reachable through the shipped
binaries (daemon, CLI, TUI, web dashboard, VS Code extension) — not merely
implemented and tested inside a library crate.

| Production-wired at HEAD (exercised through the shipped surfaces) | Library-tested (crate tests green; not wired into the shipped binaries) |
| --- | --- |
| All six CORE_TOOLS served in-process: `view_window`/`edit_lines` (austringer-tools), `execute_command` (austringer-exec under a austringer-safety sandbox/egress policy), `lsp_query` (austringer-lsp) — no NoExecutor path in production (task-3/4) | `austringer-repomap` — CSR + SIMD PageRank repo map; its only consumer today is austringer-bench's committed baselines, not the daemon's context pipeline |
| Provider composition from env (five cassette-tested adapters on the production reqwest transport); typed, actionable no-provider error when unconfigured (M32) | `austringer-search` — Tantivy BM25 lexical index; zero dependents (the shipped `tool_search` is core-internal) |
| Protocol v2 frames: per-turn usage (model/effort/tokens/cost), FallbackAsk/FallbackResolved (mediated, v1-withheld), Verdict with evidence ids (task-1) | `austringer-wasm` — wasmtime/WASI 0.2 plugin runtime; feature-gated, off by default, zero dependents |
| Dispatcher live in the daemon loop: routing.json roles/tiers/categories/fallbacks/cheap-pool, ask-first fallback, same-model retry on timeout (task-9/13) | OAuth device-flow subscription auth (providers `device_flow.rs`, RFC 8628 + PKCE); cassette-tested in the crate, not yet composed into the daemon's env-based selection |
| 2026-10 model catalog wave (GPT-6 Sol, GPT-6.1 Sol, GPT-6 Luna, Gemini 4 Argon, Grok 4.7, Claude Opus/Sonnet/Haiku 5.5 beside the existing 12; fail-closed effort grids) (task-10) | Orchestration best-of-N (same prompt on up to four models) and plan-branch comparison; the spec-driven launch path and fork-join children ARE wired (task-8) |
| Cross-session transcript search over the maintained `event_fts` index: `GET /sessions/search`, `austringer sessions search`, session-scoped `search_sessions` model tool, ranked rendering (task-11). Search surface production-wired; the live-session feed is library-tested/unwired — the daemon engine does not insert live conversation turns into `event_fts`, so searches see seeded/test data only (follow-up logged to wire the recorded-event path) | Worker (out-of-process executor) protocol — deliberately reserved; in-process executors are the production path (task-3) |
| ACP stdio agent mode (`austringer-daemon --acp`): initialize, session/prompt, session/update, fs read; mediation through the same queues (task-12) | |
| `task(category)` delegation (depth cap, fresh grant state, per-parent fan-out/budget caps) + curated read-only `explore`/`librarian` cheap-pool agents + team surfacing with Warp-style status precedence (task-13/14/15) | |
| Opt-in interactive plan mode (off by default; write tools gated until approve/edit/reject; session grants suspended while planning) (task-16) | |
| `run_tests` tool (cargo test/nextest detection, `austringer_TEST_COMMAND` user-global override, typed TestReport) with test evidence bound into the completion gate — REFUTED verdicts cite failing test ids (task-17/18) | |
| Snapshot rewind: `ClientMsg::Rewind`, TUI `/rewind`, event_fts truncated to the restore point (task-5) | |
| Skills runtime: pack loading behind trust gates + quarantine, `/skill` invocation, `GET /packs`; pack MCP entries land disabled (task-6) | |
| Session cost governance: micro-USD ledger keyed off usage frames, default $20/session cap (`austringer_SESSION_COST_CAP_USD`), warning at 80%, typed stop at 100%; children and automations aggregate (task-33) | |
| History + automations surfaces: `GET /s/{sid}/history`, `GET /automations`, sched/verdict notifications on the event stream; declarative `automations.json` (task-2) | |
| MCP CLI family (`austringer mcp list\|scan\|serve\|call`) (task-7); run-history + automations + team events in the four frontends; unified command grammar; OSC 777 + 7501 program-status emission (task-23/24) | |
| Media surface: daemon-served upload/fetch with delivery-cap projection, nine adapter families enabled (image, svg, pdf, audio, office, gltf, georaster, data, text) | |
| Encrypted sessions store (XChaCha20-Poly1305, lazy paging), transactional shadows, sandboxes, permission engine, TUI/web/VS Code/CLI surfaces, perf gates, evals sweep (hermetic, scheduled-only by policy; live-model sweeps never gate PRs) | |

## Roadmap

The plan's wave structure (each wave's todos carry agent-executable
acceptance criteria, which double as good-first-issue definitions — see
[CONTRIBUTING.md](CONTRIBUTING.md)):

| Wave | Contents |
| --- | --- |
| W0 | workspace scaffold, CI matrix, test-support crate (mock provider, SSE DSL, cassettes) |
| W1 | core engine: events, agent loop, tool registry, inspectors, autonomy |
| W2 | providers + context: five adapters, prompt idempotency, eviction, compaction |
| W3 | code intelligence: ast, repomap, editing tools, search, LSP |
| W4 | execution: PTY, ConPTY/WSL2, shadows, pre-flight, snapshots, sessions, budgets |
| W5 | safety: OS sandboxes, permission engine, lifecycle hooks |
| W6 | extension: MCP, skills/packs, Skill Forge, registry, model profiles |
| W7 | surfaces: daemon, TUI, web, VS Code, CLI |
| W8 | orchestration: subagents + spec launcher, scheduler + completion gate, role routing |
| W9 | perf suite, evals, community surface (this README) |

## Naming

The public name is **austringer** (crates `austringer-*`, binaries
`austringer` / `austringer-daemon`). An *austringer* is the falconer who
flies short-winged hawks: the handler whose craft is letting a powerful,
autonomous bird fly — while staying attached to it. That is this project's
whole shape: the daemon keeps the attachment (permissions, budgets, sessions,
ask-first fallback); the model does the flying.

The name was chosen the way everything else here is — by receipt: 160+
candidates across 14 metaphor families, live collision checks (two
perfect-availability names, *creance* and *evener*, turned out to be already
claimed by other agent projects and were rejected), and availability
verification against crates.io, the GitHub org namespace, and registry RDAP
before committing. `harness` itself failed that audit: every dictionary-word
`.com`/`.dev` is squatted, the crate name is taken, and the word names the
category, not the product. Full decision record:
[.omo/research/naming-decision.md](.omo/research/naming-decision.md).

Canonical links: code at [vinaosb/austringer](https://github.com/vinaosb/austringer),
manual at <https://vinaosb.github.io/austringer/>, crates on crates.io as
`austringer-*`. A custom domain is deliberately deferred (see the decision
record): `austringer.dev`/`.io`/`.org`/`.sh` were verified unregistered at
decision time and stay parked until traffic justifies them.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) — one command from clone to green
tests, hermetic by construction (zero API keys, zero network). Good-first-
issue candidates are the wave-map todos whose acceptance criteria are already
written. Security reports: [SECURITY.md](SECURITY.md). Community standards:
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option (the Rust ecosystem norm; contributions are accepted under the
same terms unless stated otherwise).

This documentation embeds no counters, tracking pixels, or telemetry of any
kind.

