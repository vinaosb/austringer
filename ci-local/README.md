# local-ci — run the CI on the author's PC before it costs Actions credits

One command re-runs the repo's GitHub Actions jobs on the machine you are
about to commit or push from. The point is not "run the tests locally"
(that's `cargo nextest run --workspace`); it is **runner parity**: prove,
before the push, that the exact step sequence CI executes also passes in
the exact class of environment CI provides.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/local-ci.ps1            # default gate
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/local-ci.ps1 -All       # + optional phases
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-hooks.ps1       # wire the git hooks
```

## Why docker for the ubuntu legs

Every CI red this repo has shipped was a **runner-environment divergence**
between a dev PC and a GH runner, never a logic bug that local tests would
have caught as-is:

| Receipt | What diverged |
| --- | --- |
| `46fe3ef` | ANSI color codes from child processes broke the run_tests log parser — fixed by forcing `NO_COLOR`; the harness runs everything ANSI-free (`NO_COLOR=1`, `CARGO_TERM_COLOR=never`) |
| `3e984a4` / `30ce144` | hardcoded path separators survived every Windows check and died on the ubuntu runner's `/`-only paths |
| `e7a81a2` | `autocrlf` drifted fixture line endings between checkout platforms until a byte-compare test went red on CI only |
| `88674b7` | a chaos-oracle test was flaky under the runner's sibling-process layout, not the dev machine's |

So the ubuntu jobs (`lint`, the ubuntu leg of `test`, the `docs` site
build, `coverage`) are re-run inside an **ubuntu:24.04** container — the
generation the `ubuntu-latest` runners are built from — with the same
toolchain pin (rust 1.98 = workspace MSRV), the same tool pins
(`cargo-nextest@0.9.144`, `cargo-deny@0.20.2`, from
`.github/workflows/ci.yml`), and the same env knobs
(`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`). The
windows-only jobs (`test` windows leg, `frontends`, `perf-gate`) run
natively on the host — a Windows host **is** that environment.

`ci-local/run-ubuntu.sh` mirrors the workflow steps **exactly and only**:
no local-only additions, no skipped CI-reds. If it passes here, the GH
ubuntu legs of those jobs would have passed (modulo Actions' own
infrastructure).

## Phases

| Phase | Where | Mirrors (`.github/workflows/`) | Default | `-All` |
| --- | --- | --- | --- | --- |
| `docker-lint` | container | `ci.yml` job `lint` (fmt, clippy, clippy feature matrices `dhat`/`mimalloc`/`wasm-plugins`, `cargo deny check`) | yes | yes |
| `docker-test` | container | `ci.yml` job `test`, ubuntu leg (nextest archive + run-from-archive, wasm-plugins run with count guard, doc tests) | yes | yes |
| `native-test` | host | `ci.yml` job `test`, windows leg (nextest archive + run-from-archive) | yes | yes |
| `native-frontends` | host | `ci.yml` job `frontends` (e2e_daemon build, `npm ci && npm test` in harness-vscode, web e2e via global playwright-core) | — | yes |
| `evals` | host | `evals` conformance sweep (`cargo run -p harness-evals --bin run_sweep -- --conformance`) | — | yes |
| `docs` | host + container | `docs.yml` (linkcheck natively via `.github/scripts/linkcheck.ps1`; asciidoctor zero-warning site build in a throwaway ubuntu:24.04 container) | — | yes |
| `coverage` | host | `ci.yml` job `coverage` (`cargo llvm-cov nextest --lcov`; requires host-installed `cargo-llvm-cov@0.9.1`) | — | yes |
| `perf-gate` | host | `ci.yml` job `perf-gate` (release build, perf-gate vs committed baselines, dhat heap-profile check) | — | yes |

Overrides: `-Phases native-test` (just one), `-SkipDocker` (native legs
only — e.g. no docker daemon), `-RebuildImage` (rebuild the image and
re-seed the cargo volume). Exit code is non-zero if any requested phase
failed; failures are listed at the end. All phases are idempotent.

First `docker-*` run cold-builds the workspace inside the container
(~10–25 min at 3 jobs); later runs reuse the volumes. `native-test`
uses the repo's dev-machine cargo wrapper (`%TEMP%\opencode\cargo-env.cmd`,
`CARGO_BUILD_JOBS=3`, `CARGO_TARGET_DIR` from the environment) — the same
invocation shape documented in `AGENTS.md`.

## Image, volumes, caches

- Image `harness-localci:<dockerfile-sha8>` (+ `harness-localci:latest`),
  rebuilt automatically when `ci-local/Dockerfile` changes or on
  `-RebuildImage`. Runs as non-root `runner` (uid 1000 — the GH runner
  convention).
- Named volume `harness-localci-cargo` at `/opt/cargo` — rustup home with
  the pinned tools; seeded from the image on first mount (drop it with
  `-RebuildImage`).
- Named volume `harness-localci-target` at `/target` — the cargo target
  dir, so container builds stay incremental across runs.
- Containers run with `--cpus=4` — the standard ubuntu-latest runner
  shape. This is parity, not throttling for its own sake: nextest sizes
  its default test parallelism from core count, and the suite's
  wall-clock budget tests (`harness-search::index_perf`'s generous
  "CI-safe" bounds) are calibrated for runner-shaped 4-cpu contention;
  unbounded container parallelism on a busy dev host starves them
  (observed: two consecutive docker-test runs, each a different
  index_perf test tripping its bound; green ever since).
- The repo root is bind-mounted **rw** at `/src`: tests legitimately write
  temp fixtures under the workspace. Mounts and `--env` parity vars are
  supplied by `scripts/local-ci.ps1`.

## Git hooks (commit fast, push full)

`scripts/install-hooks.ps1` sets `git config core.hooksPath scripts/hooks`
(versioned hooks — nothing is copied into `.git`):

- **pre-commit**: `cargo fmt --all -- --check` only — seconds, host
  toolchain.
- **pre-push**: the full default gate via `scripts/local-ci.ps1`
  (docker-lint + docker-test + native-test), exit code propagated.

Bypass either once with `HARNESS_SKIP_LOCAL_CI=1` (e.g. WIP commits,
emergency pushes). Uninstall with
`git config --unset core.hooksPath`.

Note: the hooks and `ci-local/run-ubuntu.sh` are POSIX sh and must stay
LF on disk. `install-hooks.ps1` and `local-ci.ps1` re-normalize them
automatically, which matters on `core.autocrlf=true` checkouts (the
`e7a81a2` receipt, applied to tooling this time).

## Machine parity notes (the native legs are only as CI-like as the host)

The windows CI legs assume the host resembles a GH windows runner. One
divergence on the original dev machine is load-bearing and worth knowing:
ConPTY children (portable-pty's `CommandBuilder`) get their environment
from the **registry** SYS+USER PATH overlay, not the spawning process's
PATH — so a cargo that exists only on the `cargo-env.cmd` wrapper's PATH
is invisible to `harness-exec`'s PTY tests. That was the documented
machine-local red `pty_live::cargo_version_style_command_renders_cleanly`
(`.omo/notepads/ecosystem-alignment/issues.md`; AGENTS.md "known local
flakes"). The standard rustup install puts `%USERPROFILE%\.cargo\bin` on
the USER PATH, which is exactly the parity fix (one-time, reversible):

```powershell
$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
$k.SetValue('Path', $k.GetValue('Path','', 'DoNotExpandEnvironmentNames').TrimEnd(';') + ";$env:USERPROFILE\.cargo\bin", $k.GetValueKind('Path'))
```

The harness does **not** edit the registry for you — machine config is
user-owned; this note is the receipt. The hooks and `local-ci.ps1`
tolerate both worlds (they prepend `~/.cargo/bin` / use the repo's
cargo wrapper when present, plain `cargo` otherwise).

## Intentionally NOT mirrored

- **deep-qa** (`.github/workflows/deep-qa.yml`) — it needs docker-in-docker
  plus a pinned standalone rust-analyzer and real-PTY/heavy tests; it is
  scheduled-only by policy and never gates a PR. Do not "fix" this gap.
- **coverage / perf-gate are default-off** — they double the build matrix
  (a second instrumented build; a full release build). CI runs them; the
  local default stays fast. Both are one `-Phases` or `-All` away.
- **macos leg of the test matrix** — no macOS host here, and no docker
  image can approximate it; CI remains the only gate for that leg.
- **actions/upload-artifact steps** — nothing to upload locally; the
  nextest archive itself IS run (from `%TEMP%`) in the test phases.

## Merge gating (client-side, the last line)

Server-side branch protection is unavailable here: the repo is private on
a free GitHub plan, and the settings API answers **HTTP 403 — "Upgrade to
GitHub Pro or make this repository public"** (receipt, 2026-10-24). Until
the plan changes, the merge contract is discipline —
`scripts/merge-gate.ps1 <PR>` must print PASS immediately before every
`gh pr merge`: it re-reads the PR's `statusCheckRollup` and refuses unless
every check is `COMPLETED`/`SUCCESS` (SKIPPED/NEUTRAL entries are
informational; scheduled workflows like deep-qa and evals-scheduled never
gate PRs by policy anyway). The pre-push gate above remains the first
line of defense; this script is the last.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/merge-gate.ps1 3   # must print PASS...
gh pr merge 3 --merge                                                          # ...before this runs
```

### Server-side blockers (dormant)

The server-side half of that contract ships dormant and ready:
`ci-local/branch-protection.json` is the exact
`PUT /repos/vinaosb/harness/branches/main/protection` payload (the nine
required check names, verified against real check runs), and
`scripts/enable-branch-protection.ps1` applies it. The repo owner chose to
keep protection **off** until the repository goes public ("until then let's
leave it commented or off"); when that happens, run
`powershell scripts/enable-branch-protection.ps1` — it prints ACTIVE with a
verification summary, or the 403 receipt and exits 1 while the repo is
still private. Until then `scripts/merge-gate.ps1` remains the whole
contract. (Going public also restores free Actions minutes on standard
runners, so the required checks can actually run.)
