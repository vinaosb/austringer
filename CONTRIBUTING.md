# Contributing to harness

Thanks for helping build a harness whose every design decision carries a
receipt. This ramp is written so that one command takes you from a fresh
clone to green tests — the command below is the actual CI test path, and the
test suite is hermetic by construction: a scripted mock provider, recorded
cassettes, and fake clocks. **Zero API keys, zero network, zero live models.**

## One command from clone to green

```sh
git clone https://github.com/placeholder-org/harness && cd harness  # or your fork
cargo nextest run --workspace     # the whole suite: hermetic, no keys, no network
```

That is the entire setup. If `cargo nextest` is not installed:
`cargo install cargo-nextest` (the CI matrix pins `0.9.144` at the time of
writing; see `.github/workflows/ci.yml` for the live pin).

Gates that also run in CI and are worth running locally before a PR:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

Before pushing, you can run the repo's local CI parity harness — the
ubuntu legs of the CI workflow inside an `ubuntu:24.04` container with the
pinned toolchain and tools, the windows legs natively — so runner-environment
divergences (ANSI codes, path separators, fixture eol, process layout; the
receipt chain behind it) surface on your PC instead of on billed Actions
minutes: `powershell -NoProfile -ExecutionPolicy Bypass -File
scripts/install-hooks.ps1` wires a fast `pre-commit` (fmt) and a full
`pre-push` gate; see [ci-local/README.md](ci-local/README.md) for the phase
table and the `HARNESS_SKIP_LOCAL_CI=1` bypass.

The workspace lints (`Cargo.toml`) already escalate `warnings = "deny"` and
`clippy::all = "deny"` for every crate, so a clean `cargo check` is a
lint-clean check.

## Building on Windows

Windows is a first-class target, and the normal MSVC toolchain
(`x86_64-pc-windows-msvc`) is the default. A caveat observed on some
developer machines: Visual Studio servicing can leave the MSVC include
directories incomplete, which surfaces as `C1083` errors ("cannot open
include file") during the C builds that some dependencies compile via `cc`
(bundled SQLite, `ring`, tree-sitter grammars, wasmtime trampolines). If you
hit this:

- The workaround is shell-scoped environment repair — point `INCLUDE`/`LIB`
  at a complete MSVC + Windows SDK header/library set (the C/C++ include
  directory first, then the Windows SDK `ucrt`/`shared`/`um` directories),
  with `CC` unset so `cc` uses its default discovery. Nothing repo-side
  changes; a normal machine with an intact "Desktop development with C++"
  workload needs none of this.
- Ask the maintainers before inventing repo-level fixes: do not hardcode
  machine-specific SDK paths into any committed file.
- The daemon, CLI, and TUI otherwise build and run on Windows out of the box;
  PowerShell 7 is the daemon's default shell for spawned commands.

## Test discipline

- **TDD, strict red-green.** Write the failing test first; the repo's tests
  are written against the intended API before the implementation exists.
  Engine crates use `cargo-nextest` (process-per-test), `insta` snapshot
  suites, `assert_cmd` for the CLI, a fake-clock (`start_paused`) for timer
  logic, and the `harness-test-support` mock provider (rule DSL, SSE
  event-builder DSL, mid-stream drop injection, cassettes with strict
  replay) — there is no code path that needs a live model to be tested.
- **RED evidence is part of the record.** When you fix a defect or land a
  mechanism, the test that proves it should have been seen failing (or be
  demonstrably new). Never delete or weaken a failing test to get green; if
  a test is wrong, say why in the PR and replace it with a stronger one.
- **Adversarial-verification culture.** Test the failure modes, not the happy
  path alone: malformed input, mid-stream drops, torn final lines, load,
  flakiness (re-run x3), stale caches, dirty worktrees, hung processes. The
  existing suites are full of these channels (`evals_adversarial`, PTY
  pathology fixtures, sandbox escape-attempt tests); a PR that adds a
  mechanism without adding its adversarial twin will be asked for one.
- **Hermetic by default.** No network in tests, ever. Live-model sweeps are
  scheduled-only by policy and are never part of the PR path. A handful of
  live-PTY tests are timing-sensitive under heavy parallel load (they spawn
  real processes and kill them): if exactly one flakes in a full-workspace
  run, re-run it in isolation — `cargo nextest run -p harness-exec --test
  <suite>` — before investigating; a genuine regression does not pass
  alone.
- House style the CI and reviewers hold the line on (it is observable
  throughout the tree): files stay small (a ~250-line ceiling on pure-logic
  modules — split instead of growing), and `unwrap`/`expect`/`panic!` stay
  out of library code paths in favor of typed errors.

## Evidence culture

PRs that change behavior cite existing evidence or add it (the research file
or the evals suite):

- A new mechanism or a changed default should reference the research-file
  section (`.omo/research/harness-research.md`, cited as "research §N") or
  another harness precedent that motivates it — the project's core discipline
  is that adopted and avoided decisions stay traceable to evidence.
- A behavioral claim in docs or PR descriptions should be backed by a test,
  a committed baseline, or an eval artifact in the repository. Numbers
  published in README/docs must match `harness-bench/budgets.json`,
  `harness-bench/baselines/`, or recorded eval reports — budgets are the
  contract, measurements are labeled as machine-specific.
- Perf-relevant PRs: the CI perf gate fails >10% regressions against the
  committed deterministic baselines. If your change legitimately moves a
  baselined number, re-record it per
  `harness-bench/baselines/README.md` and say so in the commit message.

The PR template (`.github/PULL_REQUEST_TEMPLATE.md`) has fields for exactly
this: evidence cited, and guardrail impact.

## Commit conventions

Observed history (Conventional Commits, one commit per unit of work — see
`git log --oneline`):

```
feat(core): operation-pipeline agent loop
feat(providers): five adapters + cost accounting
fix(core): drop redundant u64 cast flagged by clippy in spill test
refactor(mcp): split oversized session surface module
ci: nextest archive matrix, coverage, dist, perf-gate stub
docs: positioning README + community ramp + AsciiDoc docs site
```

- Format: `type(scope): summary` — types in use: `feat`, `fix`, `refactor`,
  `test`, `style`, `ci`, `perf`, `docs`, `chore`.
- One logical change per commit; a todo-sized unit of work lands as one
  commit, not a trail of fixes (follow-up corrections are their own small
  commit, never an amend of a pushed branch).
- Squash-merge is the norm; tags are semver — `v0.1.0` (2026-09-21) is the
  first tagged snapshot, and every tagged release adds its page under
  `docs/release/` (leaderboard + eval tables per the publication rules).

## Wave map and parallelization

The work plan is structured in waves (see the README roadmap table). Guidance
for picking up work:

- **Good first issues are wave-shaped.** Every todo in the plan carries
  agent-executable acceptance criteria — those criteria are the issue text.
  Ask on an issue which wave/todo it maps to, or propose one from the README
  roadmap table.
- **Bottom-up dependency order matters**: engine crates (`harness-core`,
  `providers`, `ast`, ...) land before the surfaces that consume them
  (`daemon`, `tui`, `web`, `vscode`, `cli`), and the meta crates
  (`harness-test-support`, `harness-bench`, `harness-evals`) are safe to work
  on in parallel with the engine.
- **Parallel-work discipline** for multi-change days: keep edits to shared
  files (`Cargo.toml`, `Cargo.lock`) minimal and anchored — re-read
  immediately before editing, prefer appending a dependency line over
  restructuring, and stage only your hunks when the lockfile carries a
  sibling's deltas.
- New to the codebase? `harness-test-support` (the mock provider and SSE DSL)
  is small, self-testing, and teaches the provider trait in one sitting.

## Docs changes

The README and CONTRIBUTING stay Markdown (the storefront). The manual,
architecture guide, and research companion are AsciiDoc under `docs/`, built
to a static site in CI. Cross-references use explicit stable IDs
(`[[id]]` anchors + `xref:`), chapters are composed with `include::` —
when adding a page, register it in the build list in
`.github/workflows/docs.yml` and keep every xref explicit so the linkcheck
gate (`.github/scripts/linkcheck.ps1`) can prove zero broken references.

## Code of conduct

By participating in this project you agree to uphold the
[Code of Conduct](CODE_OF_CONDUCT.md). Security reports go through the
private channel in [SECURITY.md](SECURITY.md), never public issues.
