# Changelog

All notable changes to austringer are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), adapted to this
project's receipts discipline: where a change implements a studied pattern,
it names the evidence (research §N in
[.omo/research/harness-research.md](.omo/research/harness-research.md);
task-N evidence lives in `.omo/evidence/`, internal).

## [Unreleased]

(none yet)

## [0.1.2] - 2026-10-27

The launch wave: `austringer` just starts (research §34.2, the
daemon-by-default convergence receipt — opencode 2.0, Codex `/daemon`,
Amp shared runners). Installers and the container image attach to this
tag via the Release workflow; the Homebrew tap
([vinaosb/homebrew-austringer](https://github.com/vinaosb/homebrew-austringer))
receives its first formulae.

### Added — the just-starts launch posture

- **Client auto-spawn** (#27): on the pure default transport (no
  `--url`/`--token`), a missing token or a dead daemon spawns one
  `austringer-daemon` and retries exactly once, with a visible notice
  (pid, loopback-only, token path). `AUSTRINGER_NO_AUTOSPAWN=1` keeps
  a typed refusal. New `austringer-launch` crate: binary lookup
  (explicit env override must exist or the error names it — never a
  silent fallback), detached spawn, readiness wait on the token-file
  change (the bind-before-token contract), terminal-aware stderr.
- **The daemon writes `daemon-url`** (bound base URL) just before the
  token, so clients never parse daemon stdout (#27).
- **Bare `austringer` execs the TUI** (#28, via #31) — the
  codex/opencode default-experience pattern; subcommands stay for
  scripts. The TUI is ported to clap (`--help` works) and attaches by
  discovery (`daemon-url` + probe) or auto-spawn, no `--url` needed.
- **`austringer web`** (#29, via #31): ensures the daemon, prints the
  dashboard URL, opens the browser unless `--no-browser`; a missing
  opener is a printed hint, never a failure.
- **One-command install** (#32): `install.sh` / `install.ps1` at the
  repo root install CLI, daemon, and TUI from the SAME release (latest
  or a pinned tag), delegating to the checksum-verifying official
  installers.
- **The TUI ships in release artifacts** (#26) and **Homebrew
  formulae** publish to the tap on every release (#33).

### Fixed

- **Windows handle-inheritance pipe hold** (#27): a detached child
  inherits every inheritable handle, so a spawned daemon held an
  ancestor's stdout pipe forever — `austringer capture | jq` would
  wedge until the daemon died. Inherit flags are now cleared around
  the spawn (except an inherited terminal stderr).
- **Daemon startup prints no longer panic on a closed stdout pipe**
  (#35): `println!` panics on EPIPE; a parent that piped the daemon's
  stdout and stopped reading killed it before the first request (the
  root cause behind four burned CI health-poll deadlines — 30 s ×2,
  90 s, 180 s — finally caught by the #15 stderr diagnostics).

## [0.1.1] - 2026-10-25

The first austringer-named release: everything below previously sat under
Unreleased. Installers and the container image attach to this tag via the
Release workflow (cargo-dist).

### Changed — project identity

- **Renamed `harness` → `austringer` everywhere the project is named**
  (`0f9e198`): 24 cargo crates plus the VS Code extension (`austringer-*`),
  binaries (`austringer`, `austringer-daemon`), env vars (`HARNESS_*` →
  `AUSTRINGER_*`), wire headers (`Austringer-Session-Id`,
  `Austringer-Agent-Commit`), the snapshot ref namespace
  (`refs/austringer/`), the skills vendor dir (`.austringer`), and the OSC
  program-status app id. Decision record with the full method, collision
  kills, and availability evidence:
  [.omo/research/naming-decision.md](.omo/research/naming-decision.md).
  - Deliberately kept: `harness-rs` (an external project cited in receipts),
    the category-named study file `harness-research.md`, generic category
    prose, and third-party fixture corpora.
- Repository renamed and made public:
  [github.com/vinaosb/austringer](https://github.com/vinaosb/austringer);
  the AsciiDoc manual now deploys to GitHub Pages
  (https://vinaosb.github.io/austringer/).
- LICENSE copyright holders updated to "austringer contributors".

### Fixed

- Restored uppercase `AUSTRINGER_*` env-var names that a case-insensitive
  rename rule had lowered (`austringer_PROVIDER`-style) — masked on Windows
  (case-insensitive env lookup), breaking on Linux (`02c8b22`).
- Grammar articles introduced by the rename ("a austringer" → "an
  austringer"), including the system-prompt template; perf baselines
  re-recorded per the documented procedure — `prompt.stable_prefix_bytes`
  moved by exactly the renamed-token byte count — and the gate re-validated
  green post-rename (10 scenarios, 0 breaches).
- The v1 replay corpus was re-captured post-rename: the payload hash and
  byte count changed with the renamed seed; every other wire frame is
  byte-identical. TUI snapshots regenerated (box width only); glb fixtures
  re-chunked with the `austringer-fixture` generator.
- SECURITY.md's placeholder mailbox replaced with GitHub private
  vulnerability reporting (enabled on the repository).

### Added

- This CHANGELOG — the release history previously lived inside the README.
- A migration chapter for people coming from other harnesses
  (docs/partials/migrating.adoc) and prebuilt-binary install instructions
  (README + getting-started).

## [0.1.0] — 2026-09-21

First tagged snapshot (`fcc7b7f`), cut when the release pipeline went green.
Pre-release: APIs, wire formats, and names may still change (and did — see
Unreleased).

### Added — the shape

- One daemon, four thin clients (CLI, TUI, daemon-served web SPA, VS Code
  extension) over one loopback protocol (HTTP/WS/SSE + local-socket IPC);
  multiple clients attach to one session with a shared permission queue
  (research §1).
- Encrypted sessions store — SQLite + XChaCha20-Poly1305 at rest, lazy
  paging, `event_fts` full-text index — and snapshot rewind that restores
  files and history together (research §23.8 ADOPT).
- Five cassette-tested provider adapters (anthropic / openai / google / xai
  / compat) composed from env; a typed, actionable no-provider error;
  per-model protocol dialects with a 400-trap feature-detection table
  (research §33); 2026-10 model catalog wave (20 seeded models, fail-closed
  effort grids).
- Dispatcher: routing.json roles/tiers/categories/fallbacks/cheap-pool,
  ask-first fallback — never silent substitution (research §2) — and
  same-model retry on timeout; profile stickiness across resume.
- `task(category)` delegation with depth caps, fresh grant state, and
  per-parent fan-out/budget caps, plus curated read-only `explore` and
  `librarian` cheap-pool agents (research §35).
- Opt-in interactive plan mode — timeout never approves — with
  approve/edit/reject before any write (task-16).
- `run_tests` tool with typed reports, and test evidence bound into the
  adversarial completion gate (CONFIRMED / REFUTED / INCONCLUSIVE verdicts
  with evidence links; research §15).
- Skills runtime: pack loading behind trust gates and quarantine, `/skill`
  invocation, `GET /packs`; pack MCP entries land disabled (research §27).
- MCP CLI family (`austringer mcp list|scan|serve|call`) and an ACP stdio
  agent mode (`austringer-daemon --acp`) so editors can host austringer as
  a guest agent (research §23.3).
- Session cost governance: micro-USD ledger keyed off usage frames, default
  $20/session cap (`AUSTRINGER_SESSION_COST_CAP_USD`), warning at 80%,
  typed stop at 100% — children and automations aggregate (task-33).
- OSC 777 / 7501 program-status emission so terminal tabs can see
  blocked-on-permission states; media surface with nine adapter families
  served by the daemon under token-authed fetch routes.
- Performance budgets CI-enforced against committed deterministic baselines
  (research §23); hermetic evals — conformance and A/B — under the
  austringer-matched publication discipline (research §18, research §26).

### Known library-tested at 0.1.0 (green in-crate; not yet wired into the shipped binaries)

- `austringer-repomap` (CSR + SIMD PageRank repo map; consumer: bench
  baselines only), `austringer-search` (Tantivy BM25 index; zero
  dependents), `austringer-wasm` (wasmtime/WASI 0.2 plugin runtime;
  feature-gated off by default), OAuth device-flow subscription auth
  (cassette-tested; not composed into env-based selection), orchestration
  best-of-N and plan-branch comparison (the spec-driven launch path and
  fork-join children are wired), and the worker (out-of-process executor)
  protocol — deliberately reserved.

### Delivery structure

- Waves W0–W9: scaffold/CI → engine → providers/context → code
  intelligence → execution/snapshots → safety → extension (MCP/skills) →
  surfaces → orchestration → perf/evals/community. The wave-map todos carry
  agent-executable acceptance criteria and double as good-first-issue
  definitions ([CONTRIBUTING.md](CONTRIBUTING.md)).
- Shipped mechanics, with the commits that landed them: Skill Forge
  (quarantine + trust gates + correction journal) `bc83de6` (task-31);
  registry lock.json tamper detection, pin + hash verify `3565b43`
  (task-32); model profiles (per-model dialects + 400-trap table) `04a0bba`
  (task-34); scheduler + completion gate `26e72c3` (task-41); role routing
  with confirmed fallback `26edbf8` (task-42); perf budgets + regression
  gates + committed baselines `c8926a4` (task-43); evals (conformance +
  A/B, austringer-matched discipline) `3fb0b07` (task-44).
