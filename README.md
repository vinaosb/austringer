# harness

[![CI](https://github.com/placeholder-org/harness/actions/workflows/ci.yml/badge.svg)](https://github.com/placeholder-org/harness/actions/workflows/ci.yml)
[![docs](https://github.com/placeholder-org/harness/actions/workflows/docs.yml/badge.svg)](https://github.com/placeholder-org/harness/actions/workflows/docs.yml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**One small Rust daemon runs an AI coding agent; four thin clients attach to it — terminal, TUI, browser, VS Code — under hard, CI-enforced performance budgets, and every design decision is a receipt from a studied post-mortem of another harness.**

- Pre-release (0.1.x, workspace scaffold through wave 9). No releases have been
  tagged yet; see [Status](#status) for exactly what works at this commit.
- The claims on this page cite their evidence: research sections refer to
  [.omo/research/harness-research.md](.omo/research/harness-research.md)
  (the in-repo study of 25+ harnesses this project is built from); shipped
  mechanics cite the commit that landed them and the recorded task evidence.

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
  [harness-bench](harness-bench/budgets.json)).
- **Claims carry receipts.** No marketing claim on this page lacks an in-repo
  citation. Benchmark tables publish harness-matched rows only; unmatched rows
  are marked DO NOT PUBLISH (research §18, the Qwen README discipline).
- **Windows is first-class.** ConPTY pathologies, Job-Object kill trees, and
  restricted-token sandboxing are built in, not ported later.

## Quickstart

Verified executable at the current commit (see
[docs/release/leaderboard.adoc](docs/release/leaderboard.adoc) for the run
environment; on Windows the binaries are `target\release\*.exe`):

```sh
git clone https://github.com/placeholder-org/harness && cd harness  # URL is a placeholder until the naming decision
cargo build --release -p harness-daemon -p harness-cli -p harness-tui
./target/release/harness-daemon   # loopback-only; prints its URL and writes the auth token
./target/release/harness doctor   # health-check config, sandbox, MCP, sessions, env
cargo nextest run --workspace     # full test suite: hermetic, zero API keys, zero network
```

Talking to a live model is opt-in per daemon: set `HARNESS_PROVIDER`
(`anthropic` | `openai` | `google` | `xai` | `compat`),
`HARNESS_MODEL`, and `HARNESS_PROVIDER_KEY_ENV` naming the environment
variable that holds the API key (the key value itself never appears in
any committed file); `compat` additionally takes
`HARNESS_PROVIDER_BASE_URL`. An unconfigured daemon stays fully
servable and answers inference with a typed, actionable error naming
those keys.

## Four frontends, one engine

The daemon owns the agent loop, sessions, permissions, and the event log. Every
frontend is a thin client over the same protocol (HTTP/WS/SSE on loopback plus
a local-socket IPC carrier):

| Frontend | What it is | Entry point |
| --- | --- | --- |
| CLI | headless `run`/`capture`, REPL, `attach` event tail, `doctor`, `--output-json` NDJSON for eval drivers | `harness-cli` |
| TUI | event-driven ratatui interface: context gauge, cache-hit and cost HUD, omnibar (`@file`, `/mode`, `/rewind`), hunk review gates | `harness-tui` |
| Web | dashboard SPA served by the daemon itself (no node server): live session, hunk review, cost/cache/context dashboards | daemon URL in a browser |
| VS Code | TypeScript thin client: native diff decorations, per-hunk code lenses, editor telemetry (opt-in), command palette | `harness-vscode` |

Multiple clients can attach to one session simultaneously; a permission granted
from any client applies to all (crush's shared permission queue, research §1).

## Performance budgets

Budgets are normative and CI-enforced
([harness-bench/budgets.json](harness-bench/budgets.json)); the gate fails the
build on budget breach or a >10% regression against committed deterministic
baselines ([harness-bench/baselines/](harness-bench/baselines)). The
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
`cargo run --release -p harness-bench --bin perf-gate` re-measures them.

## Architecture

One daemon, a 24-crate Cargo workspace, four thin clients:

```
                     ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
   thin clients      │ harness- │ │ harness- │ │  web SPA │ │ harness- │
                     │   cli    │ │   tui    │ │ (daemon- │ │  vscode  │
                     └────┬─────┘ └────┬─────┘ │  served) │ │ (TypeScr)│
                          │  WS/SSE/HTTP (loopback, token) + local IPC  │
                          └──────┬──────┴─────┬──────┴──────────┬──────┘
                                 └────────────┴─────────────────┘
                                              │
                                       harness-daemon
                    (event log, sessions, permissions, scheduler, sandbox)
                                              │
        ┌───────────────┬──────────────┬──────┴──────┬───────────────┬────────────┐
        │ engine        │ models       │ context     │ exec + safety │ extension  │
        │ harness-core  │ harness-     │ harness-ast │ harness-exec  │ harness-mcp│
        │ (events, loop,│  providers   │ harness-    │ harness-vfs   │ harness-   │
        │  tools, com-  │ harness-     │  repomap    │ harness-      │  skills    │
        │  paction)     │  intel       │ harness-    │  snapshots    │            │
        │               │ harness-     │  search     │ harness-safety│           │
        │               │  orchestration│ harness-lsp│ harness-wasm  │           │
        └───────────────┴──────────────┴─────────────┴───────────────┴────────────┘
                persistence: harness-sessions   meta: harness-test-support,
                                                    harness-bench, harness-evals
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
| Evals publish harness-matched rows only; unmatched rows render DO NOT PUBLISH | Qwen Code README exhaustive-disclosure discipline (research §18) | +10-25pt harness inflation in self-reported numbers (research §18) |
| pass^k floors reported beside Mean@k; failure-fingerprint vectors | forge + harness-rs micro-harness discipline (research §26) | the 24pt Mean@k/Pass^k consistency gap read as quality (research §26) |
| Evaluation unit is the model x profile pair (locked profile for regressions; >= 2x2 grids for experiments) | joint model x profile evaluation (research §33) | single-model results with up to 7.8x harness variance unattributed (research §33) |
| 10 ms frame budget with hang classification as a CI gate | Zed GPUI frame-budget discipline (research §23) | perf regressions landing as benchmark noise (research §23) |
| Byte-stable prompt prefixes + cache-violation tracker make cache misses observable events | hermes cache-control engineering (research §27); jcode stable-prefix hash chain (research §32) | silent cache-miss cost spikes (research §32) |
| Sandboxed runs report requested-vs-enforced isolation honestly | harness-rs isolation reporting (research §26) | vendor-escape domains hidden inside a "sandbox" proxy (research §16) |
| Private security reporting channel shipped with the scaffold | dsh missing-SECURITY.md lesson (research §6) | no private channel when the first escape report arrives (research §6) |

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
| Evals: conformance + A/B harness, harness-matched discipline | `3fb0b07` | task-44 |

## Built on post-mortems

The research file — [.omo/research/harness-research.md](.omo/research/harness-research.md)
— is the working artifact this project was designed from: a 33-section study of
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
lessons; "harness" column reflects this project's design (research §1
"Ours" column) and current status. All four comparators were studied at the
cited sections.

| Dimension | Claude Code | Codex CLI | goose | opencode | harness |
| --- | --- | --- | --- | --- | --- |
| Language / runtime | TS (Bun), ~0.5-1 GiB steady RSS (research §1, research §2) | Rust (research §1) | Rust (research §1) | TS (Bun), heavy-ish (research §1) | Rust workspace; < 80 MB daemon RSS budget, CI-gated |
| OS sandbox incl. Windows | partial — no Windows (research §1) | yes, 3 OS (research §1) | partial — mac + Docker (research §1) | none (research §1) | OS-native per backend incl. Windows restricted tokens + ACLs + Job Objects (research §1) |
| Model substitution | silent downgrades, #40269 (research §2) | — (research §1 permission tiers) | — | — | fallback requires confirmation; ask-never-substitute |
| Sessions store | JSONL (research §1) | rollouts (research §1) | SQLite (research §1) | SQLite (research §1) | SQLite + encryption at rest (XChaCha20-Poly1305) + lazy paging (research §1) |
| Prefix-cache discipline | strong (research §1) | partial (research §1) | partial (research §1) | partial (research §1) | byte-stable prefix + violation tracker (research §27, research §32) |
| Repo map (AST/PageRank) | partial (research §1) | none (research §1) | none (research §1) | partial (research §1) | CSR + SIMD PageRank repomap (research §1) |
| Evals transparency | internal (research §1) | internal (research §1) | harbor (research §1) | none (research §1) | harness-matched published tables; unmatched = DO NOT PUBLISH (research §18) |

## Release transparency

Per todo-44 discipline, every release publishes a benchmark leaderboard and
an eval table, harness-matched rows only:

- Bench leaderboard (current pre-release baseline):
  [docs/release/leaderboard.adoc](docs/release/leaderboard.adoc)
- Eval tables and publication rules:
  [docs/release/evals.adoc](docs/release/evals.adoc)

## Status

Works at this commit (HEAD `c8926a4`, all with recorded task evidence): the
full engine crate set (events, operation-pipeline loop, tool registry with
schema hydration, inspector chain, autonomy modes, compaction, tiered
eviction, budgets, hooks), five cassette-tested provider adapters composed
into the daemon's session factory behind environment configuration
(`HARNESS_PROVIDER` / `HARNESS_MODEL` / `HARNESS_PROVIDER_KEY_ENV`; opt-in,
never enabled by default), ast / repomap / search / lsp, exec + ConPTY +
WSL2 bridge, transactional shadows, snapshots, encrypted sessions store,
sandboxes, permission engine, MCP client + server mode, skills + Skill
Forge + registry, model profiles, daemon protocol (multi-client, token
auth, scheduler), TUI, web dashboard, VS Code extension, CLI, WASM plugin
runtime, perf gates, evals crate. Live-model eval sweeps are
scheduled-only by policy and never gate PRs.

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

The public name is `harness` (crates `harness-*`). An explicit naming
decision is deliberately deferred to the pre-v1.0 marketing checklist: a
crate rename is cheap before the first release and brand-breaking after it.
The placeholder repository URL above will change with that decision.

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

