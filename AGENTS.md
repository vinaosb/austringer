# PROJECT KNOWLEDGE BASE

**Generated:** 2026-10-25 (regenerated for the harness→austringer rename)
**Branch:** ecosystem-alignment

## OVERVIEW
One small Rust daemon runs an AI coding agent; four thin clients attach (CLI, TUI, browser SPA, VS Code). 24 Rust workspace crates + TS extension. Every design decision cites a receipt from `.omo/research/harness-research.md` (40-section post-mortem study of 25+ harnesses — the file keeps its category-descriptive name). Public name: **austringer** (decision record: `.omo/research/naming-decision.md`).

## STRUCTURE
```
austringer-daemon/    # THE engine: protocol, sessions, routing, delegation, mediation, cost
austringer-core/      # lib engine: events, agent loop, tool registry, autonomy, budgets, compaction
austringer-providers/ # 5 provider adapters, cassette-tested, env-configured (never default-on)
austringer-cli/       # thin client: run/capture/REPL/attach/doctor, NDJSON for eval drivers (binary: austringer)
austringer-tui/       # ratatui client: HUD, diffs accordion, modals, insta snapshots
austringer-vscode/    # TS extension (NOT a cargo member), node --test with type stripping
austringer-web/       # daemon-served SPA (no node server) + e2e runner
austringer-sessions/  # SQLite + XChaCha20-Poly1305 store, event_fts, lazy paging
austringer-intel/     # model catalog seed + routing surface (R2 grids, trap rows)
docs/                 # AsciiDoc "depth layer"; manual.adoc composed from partials/ via include::; CI deploys Pages
packs/                # skills data (11 packs + manifest.json) - runtime data, never compiled
.omo/                 # orchestration state + research + task evidence (run-continuation/ gitignored)
```
Smaller crates: exec (ConPTY/WSL2), safety (sandboxes), skills (quarantine+lock.json, vendor dir `.austringer`), mcp (client/server/acp), orchestration (launch/roles/phases), evals (run_sweep + scenario pack), bench (perf-gate + budgets.json baselines), snapshots (git refs under `refs/austringer/`), vfs, ast/repomap/search/lsp, media, tools, test-support, wasm (host module `"austringer"`).

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Protocol frames / v1 compat | austringer-daemon/src/{protocol,compat}.rs | v2 frames downgrade for v1; decode-ERROR tests pin it |
| New SessionEvent variant | events.rs + **both** austringer-cli/src/render.rs AND austringer-tui/src/transcript.rs | exhaustive matches break builds; workspace clippy only. Wire ints = u64 (serde tagged-Content rejects u128) |
| Model routing / fallback | austringer-daemon/src/routing.rs | every hop via mediate_fallback; unacked ask = same-model retry + verbatim notice |
| Delegation (task tool) | austringer-daemon/src/task/ | fan-out cap 4, DepthGuard 1, fresh grants, per-parent budget |
| Cost governance | austringer-daemon/src/cost_ledger.rs | Warning 80% / typed Stop 100%; never switches models |
| Plan mode | austringer-daemon/src/plan.rs + core autonomy/ | timeout NEVER approves |
| Command grammar | austringer-daemon/src/grammar.rs + grammar.json | byte-pinned by test; JS/TS mirrors pinned by drift tests; json is eol=lf via .gitattributes |
| Search | austringer-daemon/src/search.rs + austringer-sessions meta.rs | surface wired; live-feed follow-up in .omo notepads |
| Skills/packs | austringer-skills/ + packs/ | {{shared:name}} interpolation; every pack needs {{shared:pack-conventions}} |
| Perf budgets | austringer-bench/{budgets.json,baselines/} | CI gate; re-record procedure in baselines README area |

## CODE MAP
LSP/codegraph unavailable on this machine — centrality unmeasured; table from session-verified knowledge.
| Symbol | Type | Location | Role |
|---------|------|----------|------|
| DaemonState | struct | austringer-daemon/src/server.rs | shared server state; sessions + config |
| SessionHost | struct | austringer-daemon/src/attach.rs | per-session command surface + mediator |
| SessionRouter | struct | austringer-daemon/src/routing.rs | dispatcher; stickiness + ask-first |
| SessionEvent | enum | austringer-daemon/src/events.rs | wire event vocabulary (v1-withheld variants) |
| ToolRegistry | struct | austringer-core/src/tools/registry.rs | per-session tool binding (CORE_TOOLS) |
| Event | enum | austringer-core/src/event.rs | engine event vocabulary |
| EventSearchHit | struct | austringer-sessions/src/meta.rs | bm25 search hit |
| ProgramStatus | enum | austringer-daemon/src/notify.rs | OSC 777/7501 status mapping (app id `austringer`) |
| SLASH_COMMANDS | const | austringer-daemon/src/grammar.rs | single grammar source of truth |
| AustringerMcpServer | struct | austringer-mcp/src/server.rs | MCP server surface (renamed from HarnessMcpServer) |

## CONVENTIONS
- Tests: `cargo nextest run --locked` — never plain `cargo test`. Hermetic: zero API keys, zero network; real-model/real-tool tests are `#[ignore = "reason"]` (run via deep-qa workflow).
- Integration tests: `tests/*.rs` snake_case, subsystem-prefixed (`loop_*`, `autonomy_*`, `media_*`); shared kits in `tests/common/`.
- 250 pure-LOC file ceiling (comments/blanks excluded); over-ceiling needs a logged justification.
- Docs receipts: every README claim cites research §N or a commit; docs/ AsciiDoc builds fail on warnings; stable `[[id]]` anchors only (linkcheck.ps1 gates both OSes).
- New deps: cargo-deny curated licenses (deny.toml); prefer already-in-workspace crates.
- Commit bodies: temp-file `-F`; on this machine use `[System.IO.File]::WriteAllText(..., UTF8Encoding($false))` — PS 5.1 `Out-File UTF8` adds a BOM that corrupts subjects.
- Env vars are `AUSTRINGER_*` (renamed from `HARNESS_*`); wire headers are `Austringer-Session-Id` / `Austringer-Agent-Commit`.

## ANTI-PATTERNS (THIS PROJECT)
- NEVER silent substitution/degradation: fallback asks first; fail-open paths surface as transcript notices; timeout/deadline NEVER approves (ask.rs, mediation.rs, plan.rs).
- Media bytes NEVER ride protocol frames — token-authed fetch route only. Host header NEVER auth.
- CLI NEVER duplicates daemon logic (austringer-cli/src/lib.rs).
- Full model catalog NEVER injected into prompts — bounded hydration; capability is per-model, never family-inferred (austringer-intel).
- Agent-loop pipeline order is binding — do not reorder (austringer-core/agent_loop/operation.rs).
- Budgets key on TaskClass, never one global; eviction fold never drops return codes.
- Secrets NEVER enter config files; declared write-set is NEVER an approval; run_tests NEVER auto-allowed.
- `DO NOT USE FOR:` in a pack hard-blocks routing even when USE FOR also hits.

## UNIQUE STYLES
- Three doc regimes, CI-enforced each: Markdown storefront (README), AsciiDoc manual w/ stable-ID xrefs (deployed to GitHub Pages), `.omo/` evidence ledger.
- Wire format pinned by test-side serde mirrors (v1-shaped enums) + byte-pinned JSON artifacts.
- "harness" in prose = the industry category (kept deliberately); `harness-rs` = an external cited project (never rename); `austringer` = this product.

## COMMANDS
```powershell
# dev-machine wrapper (MSVC pin + link-OOM guard); CARGO_TARGET_DIR=C:\ct
cmd /c "set CARGO_BUILD_JOBS=3&& %TEMP%\opencode\cargo-env.cmd cargo nextest run --workspace"
cargo clippy --workspace --all-targets -- -D warnings   # the ONLY safe gate for cross-crate matches
cargo fmt --check
npm test --prefix austringer-vscode                     # node --test, TS via type stripping
$env:AUSTRINGER_E2E_EXE="C:\ct\debug\examples\e2e_daemon.exe"; $env:NODE_PATH=(npm root -g)
node austringer-web/e2e/run-e2e.mjs                     # playwright-core is GLOBAL, not a repo dep
cargo run -p austringer-evals --bin run_sweep -- --conformance
```

## NOTES
- Known local flakes (documented, not bugs): pty_live cargo-PATH, provider_binary contention, dispatcher_live under load.
- `node --test <dir>` fails on this Node (25.x) — always pass explicit test files.
- e2e daemon prints an `E2E-DAEMON {base,token}` banner; drive detached (Start-Process -PassThru) and kill by PID — foreground launches hang agents.
- release.yml is cargo-dist-generated (dist-workspace.toml 0.33.0); regen via `dist generate-ci` — template drift fails Release if not regenerated.
- `.austringer-daemon/` and `.omo/run-continuation/` are gitignored runtime state — never commit.
- rust-analyzer unavailable locally: clippy is the verification evidence.
- Bench fixture prompt strings changed in the rename — if the perf-gate flags stable-prefix/byte-count baselines, follow the documented re-record procedure (austringer-bench/baselines/README.md) and note it in the commit.
