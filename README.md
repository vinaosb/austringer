# austringer

[![CI](https://github.com/vinaosb/austringer/actions/workflows/ci.yml/badge.svg)](https://github.com/vinaosb/austringer/actions/workflows/ci.yml)
[![docs](https://github.com/vinaosb/austringer/actions/workflows/docs.yml/badge.svg)](https://github.com/vinaosb/austringer/actions/workflows/docs.yml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**One small Rust daemon runs an AI coding agent; four thin clients attach to it — terminal, TUI, browser, VS Code — under hard, CI-enforced performance budgets, and every design decision is a receipt from a studied post-mortem of another harness.**

- Pre-release (0.1.x). What shipped and when: [CHANGELOG.md](CHANGELOG.md).
- The claims on this page cite their evidence: research sections refer to
  [.omo/research/harness-research.md](.omo/research/harness-research.md)
  (the in-repo study of 25+ harnesses this project is built from); shipped
  mechanics cite the commit that landed them.

**Docs:** this README is the storefront; the composed manual is built from
`docs/` by CI and served at <https://vinaosb.github.io/austringer/>. Crates
publish as `austringer-*` (API docs on docs.rs) — see [Naming](#naming).

**Contents:** [Why](#why) · [Install](#install) ·
[Quickstart from source](#quickstart-from-source) ·
[Four frontends, one engine](#four-frontends-one-engine) ·
[Performance budgets](#performance-budgets) · [Architecture](#architecture) ·
[Receipts](#receipts) · [Built on post-mortems](#built-on-post-mortems) ·
[Honest comparison](#honest-comparison) ·
[Release transparency](#release-transparency) · [Status](#status) ·
[Migrating from another harness](#migrating-from-another-harness) ·
[ACP: austringer as a guest agent](#acp-austringer-as-a-guest-agent) ·
[Naming](#naming) · [Contributing](#contributing) · [License](#license)

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

## Install

No Rust toolchain required — the Release workflow ships installer scripts and
a multi-arch container for every tagged release (latest: **v0.1.1**). The
CLI and the daemon install as separate apps:

```sh
# macOS / Linux — CLI (austringer) and daemon (austringer-daemon)
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/vinaosb/austringer/releases/latest/download/austringer-cli-installer.sh | sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/vinaosb/austringer/releases/latest/download/austringer-daemon-installer.sh | sh
```

```powershell
# Windows (PowerShell)
iwr -useb https://github.com/vinaosb/austringer/releases/latest/download/austringer-cli-installer.ps1 | iex
iwr -useb https://github.com/vinaosb/austringer/releases/latest/download/austringer-daemon-installer.ps1 | iex
```

The release archives are `cargo binstall`-compatible by construction, so
`cargo install cargo-binstall && cargo binstall austringer-cli` also works
once the crates are published. A daemon+CLI container image is published to
GHCR by the same pipeline (see `publish-container.yml`).

After installing: `austringer doctor` health-checks config, sandbox, MCP,
sessions, and env in one pass.

## Quickstart from source

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
| `AUSTRINGER_PROVIDER` | provider family: `anthropic` \| `openai` \| `google` \| `xai` \| `compat` |
| `AUSTRINGER_MODEL` | model id from the seeded catalog (effort may be pinned with the `model[effort=xhigh]` bracket grammar) |
| `AUSTRINGER_PROVIDER_KEY_ENV` | name of the environment variable holding the API key |
| `AUSTRINGER_PROVIDER_BASE_URL` | `compat` family only: the OpenAI-compatible base URL |
| `AUSTRINGER_PROVIDER_TIMEOUT_SECS` | inference timeout in whole seconds |
| `AUSTRINGER_SESSION_COST_CAP_USD` | per-session spend cap in decimal dollars (default 20.00); warning at 80%, typed stop at 100% |
| `AUSTRINGER_TEST_COMMAND` | user-global override of the `run_tests` tool's framework detection (a repo-local override file is refused with a typed error) |
| `AUSTRINGER_TELEMETRY` | OpenTelemetry opt-in (`1`); requires the config arm and an endpoint — never on by default |

An unconfigured daemon stays fully servable and answers inference with a
typed, actionable error naming those keys.

## Four frontends, one engine

The daemon owns the agent loop, sessions, permissions, and the event log. Every
frontend is a thin client over the same protocol (HTTP/WS/SSE on loopback plus
a local-socket IPC carrier):

| Frontend | What it is | Entry point |
| --- | --- | --- |
| CLI | headless `run`/`capture`, REPL, `attach` event tail, `doctor`, `--output-json` NDJSON for eval drivers | `austringer-cli` |
| TUI | event-driven ratatui interface: context gauge, cache-hit and cost HUD, omnibar (`@file`, `/help`, `/steer`, `/cancel`, `/vote-cancel`, `/mode`, `/rewind`, `/skill`), hunk review gates | `austringer` (no arguments) · `austringer-tui` |
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

Deterministic counters are baselined and regression-gated — plus 12 dhat
allocation metrics. All numbers above are reproducible:
`cargo run --release -p austringer-bench --bin perf-gate` re-measures them
(the gate re-validated green after the austringer rename: 10 scenarios,
0 breaches).

## Architecture

One daemon, a 25-crate Cargo workspace, four thin clients:

```
 thin clients ── cli · tui · web SPA (daemon-served) · vscode
                     │   WS/SSE/HTTP on loopback (token) + local IPC
                     ▼
              austringer-daemon
        loop · sessions · permissions · scheduler
        cost ledger · mediation · sandbox · event log
                     │
     ┌───────────┬───┴───────┬───────────────┬───────────────┐
     engine      models      context        exec + safety
     core        providers   ast            exec
     tools       intel       repomap        vfs
                 orchestration search       snapshots
                             lsp            safety
                                            wasm
     ── persistence: sessions          media: media (daemon-served)
     ── extension: mcp, skills         meta: test-support, bench, evals
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
| Model fallback asks, never substitutes; profile stickiness across resume | Plandex 9-role routing + per-task profile pinning (research §13) | Droid silent failover (research §16); Claude Code silent downgrades #40269 (research §2) |
| Per-model protocol dialects + 400-trap feature-detection table (feature-detect per model, never per provider) | elicitation SET/PROMPT/THROTTLE constitution (research §33) | dsh thinkingFormat bug — 72k-char reasoning bloat from a wrong dialect (research §6) |
| Unattended runs end through an adversarial completion gate with CONFIRMED / REFUTED / INCONCLUSIVE verdicts + evidence links | Grok /goal evidence-review pattern (research §15); Droid /verify (research §16) | prose-spiral "done" claims on autonomous runs (research §15) |
| Evals publish austringer-matched rows only; unmatched rows render DO NOT PUBLISH | Qwen Code README exhaustive-disclosure discipline (research §18) | +10-25pt harness inflation in self-reported numbers (research §18) |
| pass^k floors reported beside Mean@k; failure-fingerprint vectors | forge + harness-rs micro-harness discipline (research §26) | the 24pt Mean@k/Pass^k consistency gap read as quality (research §26) |
| 10 ms frame budget with hang classification as a CI gate | Zed GPUI frame-budget discipline (research §23) | perf regressions landing as benchmark noise (research §23) |
| Byte-stable prompt prefixes + cache-violation tracker make cache misses observable events | hermes cache-control engineering (research §27); jcode stable-prefix hash chain (research §32) | silent cache-miss cost spikes (research §32) |
| Sandboxed runs report requested-vs-enforced isolation honestly | harness-rs isolation reporting (research §26) | vendor-escape domains hidden inside a "sandbox" proxy (research §16) |
| Daemon + thin clients as the day-one shape | the fall-2026 convergence: OpenCode 2.0 daemon-by-default, Codex `/daemon` (rust-v0.163.0, 2026-10-15), Amp shared runners (2026-09-17–24), Cursor agent-host distribution (2026-10-01) (research §34.2) | rewriting the architecture mid-course under ecosystem pressure — OpenCode's plugin-API break cost (research §34.3) |
| task(category) delegation with ask-first chains | OmO categories (research §35.2–35.3); our no-silent-substitution contract (research §2) | silent chain-walking fallbacks — the exact refusal of research §35.4 |
| ACP stdio surface: austringer hostable as an editor-side agent | ACP as the LSP-of-agents — Claude Code E2E 2025-07-18, five SDKs, registry + JetBrains co-launch (research §23.3, research §23.8) | every editor reimplementing agent hosting against a private protocol (research §23.2) |
| Plan-as-artifact: opt-in plan mode, approve/edit/reject before any write | Antigravity artifact-centric UX (research §21.2); JetBrains committed `.junie/plans` artifacts + Confirm gate (research §24) | skip-permissions surfaces auto-approving plans (Antigravity v1.3.2 receipt, task-16) |
| Snapshot rewind: files and history restored together, search truncated to the restore point | Zed host-side uniform checkpoints — auto-update on repo change, terminal-kill on restore (research §23.8 ADOPT) | conversation-only rewind leaving worktree and history diverged |
| Session cost governance: warning at 80%, typed stop at 100% | ANTI-RECEIPT: Zed's "cost HUD with zero budget/cap enforcement" (research §23.8 AVOID) — refused to recreate it; the micro-USD ledger is wired instead (task-33) | spend visibility without spend control — a HUD no mechanism bounds |

The full receipts ledger (28 rows at last count, including the UI/UX pattern
catalog adoptions) is indexed section-by-section by the
[research companion](docs/research-companion.adoc); the mechanics-and-commits
history lives in the [CHANGELOG](CHANGELOG.md).

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
- Release history: [CHANGELOG.md](CHANGELOG.md)

## Status

Pre-release 0.1.x. Everything below is **production-wired** — reachable
through the shipped binaries (daemon, CLI, TUI, web dashboard, VS Code
extension), not merely implemented and tested inside a library crate. The
per-task landing history is in the [CHANGELOG](CHANGELOG.md).

- All six CORE_TOOLS served in-process; five cassette-tested provider
  adapters on the production transport; protocol v2 frames with mediated,
  v1-withheld fallback asks; per-turn usage (model/effort/tokens/cost).
- Dispatcher live in the daemon loop (routing.json roles / tiers /
  categories / fallbacks / cheap-pool, ask-first fallback, same-model retry
  on timeout); `task(category)` delegation with curated read-only
  `explore` / `librarian` agents and team-status precedence.
- Opt-in plan mode (write tools gated until approve/edit/reject);
  `run_tests` with evidence bound into the completion gate — REFUTED
  verdicts cite failing test ids.
- Encrypted sessions store with cross-session search over `event_fts`
  (`austringer sessions search`), snapshot rewind, and history /
  automations surfaces.
- Skills runtime behind trust gates + quarantine; MCP CLI family; ACP stdio
  agent mode; session cost governance; OSC 777/7501 program status; media
  surface with nine adapter families; 2026-10 model catalog (20 models,
  fail-closed effort grids).

Library-tested, not yet wired into the shipped binaries: `austringer-repomap`
and `austringer-search` (zero daemon dependents today), the wasm plugin
runtime (feature-gated, off by default), OAuth device-flow auth,
orchestration best-of-N / plan-branch comparison (the spec-driven launch
path and fork-join children **are** wired), and the worker
(out-of-process executor) protocol — deliberately reserved.

## Migrating from another harness

Coming from Claude Code, Codex, goose, or opencode? The manual's migration
chapter maps what you already have onto austringer — API keys and env
vars, first-run `doctor`, editor coexistence, transcript import status:
[docs/partials/migrating.adoc](docs/partials/migrating.adoc).

## ACP: austringer as a guest agent

austringer speaks the Agent Client Protocol over stdio, so any ACP host —
Zed, JetBrains, Neovim plugins, orchestrating agents — can drive it like
any other guest agent:

```sh
austringer-daemon --acp
```

For example, register austringer as an external agent in Zed
(`settings.json`):

```json
{
  "agent_servers": {
    "austringer": {
      "command": "austringer-daemon",
      "args": ["--acp"],
      "env": {}
    }
  }
}
```

Permission asks, plan approvals, and cost caps ride the same mediation
queues as the first-party frontends. (Listing in the ACP registry is
roadmap: the registry currently requires agent-declared auth methods.)

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
