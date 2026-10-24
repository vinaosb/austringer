# ecosystem-alignment - Work Plan

## TL;DR (For humans)
<!-- Fill this LAST, after the detailed plan below is written, so it summarizes the REAL plan. -->
<!-- Plain English for a non-engineer: NO file paths, NO todo numbers, NO wave/agent/tool names. -->

**What you'll get:** Our coding agent finally does in production everything its building blocks already know how to do — actually running tools, routing work to the right model with your sign-off, delegating to helper agents under spending limits, planning before editing, and running tests to prove work is done — and every screen (terminal, browser, editor, command line) shows you what is happening and lets you audit what happened, without drowning you in detail. On top of that: the test robots that check our work gain real evaluation scenarios for the new capabilities (not just code tests), and our documentation becomes honest about what runs today versus what is still a library on the shelf.

**Why this approach:** The audits found the engine's parts are built and tested but never connected to the running product — so step one is wiring (no new invention, just assembly), which then makes the proven ideas other harnesses shipped this fall (delegation categories, plan approval, test evidence, status indicators) straightforward to add on top. We only adopt what other projects have already proven with real users or numbers; unproven ideas go on a watch list, not into code.

**What it will NOT do:** It will not silently swap models on you — ever; every fallback asks first. It will not build a plugin marketplace, voice control, notebooks, a web IDE, or any other unproven experiment. It will not let marketing claims outrun the code: documentation will say exactly what is wired and what is not.

**Effort:** XL
**Risk:** Medium — the foundation step touches the daemon's core paths, but every change lands behind the existing test discipline and performance budgets.
**Decisions to sanity-check:** Tool executors run inside the daemon (not a separate worker process, yet); the delegation helper keeps our ask-first rule instead of the silent fallback chains others use; one plan covering everything rather than several smaller plans.

Your next move: approve as-is (the requested reviewer validation is part of this plan's completion), or flag any decision above. Full execution detail follows below.

---

> TL;DR (machine): XL effort, medium risk (protocol+executor foundation touches daemon core), 33 todos in 7 waves + 4 final verifiers; delivers ecosystem-parity features (delegation/plan-mode/test-runner/ACP/search/cost-governance), the production wiring the daemon was missing, UI/UX macro-audit surfacing, eval scenarios for the new mechanisms, CI/test debt closure, and docs/receipts honesty.

## Scope
### Must have
- Wave F Foundation: protocol v2 frames (usage/model/fallback-ask/verdict); history/automations/team surfaces; CORE_TOOLS executors bound in-process (harness-tools/exec/lsp wired into daemon); harness-safety on the exec path; snapshots→/rewind; skills runtime + packs endpoint; MCP CLI surface; orchestration launch path + team events.
- Wave R: Dispatcher live in the daemon loop (ask-first fallback surfaced); model catalog refreshed to the verified 2026 wave; event_fts cross-session search; ACP stdio agent mode.
- Wave D: task(category) delegation tool with ask-first chains + fan-out caps + depth cap + fresh grant state; curated cheap-pool explore/librarian agents; team surfacing with Warp status-precedence; session cost governance via the wired money ledger (task 33).
- Wave P: opt-in interactive plan mode; run_tests tool with framework detection; test evidence bound into the completion gate.
- Wave U: TUI hunk panel + live HUD + scroll + collapsed-by-default transcript; web history/timeline/workers/focus; VS Code transcript + explicit prompts; CLI sessions verbs; unified grammar; OSC 777+7501 program-status.
- Wave Q: CI for wasm/vscode/web-e2e/doctests/--locked; evals driver + scheduled workflow; deep-qa + bench-smoke schedules; repomap failure tests + tokio-tungstenite unification + baseline acceptance.
- Wave L: research §34-35 + WATCH entries; unique section renumbering; README/docs honesty pass (production-wired vs library-tested); convergence receipt rows.
- Every adopted mechanism carries a receipt (research §N or dated external source) per repo discipline.

### Must NOT have (guardrails, anti-slop, scope boundaries)
- No silent model substitution — every fallback hop asks first; chains never auto-walk (README contract).
- No plugin marketplace/distribution system. No voice, notebooks, web-IDE, Code-Mode-style MCP interpreter, git-history-as-context tool, BYO-subscription auth work, background Ctrl+B UX — WATCH ledger entries only.
- No live-model eval gating of PRs (scheduled-only policy). No new public config formats. No removal of the worker protocol or daemon-local scheduler/gate (orchestration wired additively). No invented security contact email. Nothing outside the workspace + .omo artifacts.

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- Test decision: tests-after within the same commit as implementation (repo's established cassette/hermetic discipline; Implementation + Test = ONE todo). Framework: cargo nextest (Rust), vitest/npm test (harness-vscode), node e2e (harness-web). No live-model tests gate anything.
- Evidence: each todo's happy+failure QA run records command + output into .omo/evidence/task-N-ecosystem-alignment.txt (extension is always .txt; if running under ulw-loop, mirror into the attemptDir from 'omo ulw-loop status --json').
- QA invocation convention: every QA scenario executes via the commands named in that todo's Acceptance criteria (`cargo nextest run -p <crate> <test-filter>`; `npm test -- <filter>` for harness-vscode; `node harness-web/e2e/run-e2e.mjs` for web) — prose scenarios name the behavior the filter's tests must assert; the evidence file records the exact invocation + output.
- F3 definition: 'Real manual QA' means agent-executed hands-on QA — the verifier drives the daemon + frontends with fixture scripts and records transcripts; zero human intervention.
- Structural gate: every implementation row is column-zero `- [ ] N. <title>`; final verifiers `- [ ] F<n>. <title>`; self-check before handoff.
- Budget gate: `cargo run --release -p harness-bench --bin perf-gate` must stay green through Waves F-U (daemon RSS/frame budgets are the contract; any breach blocks the wave).

## Execution strategy
### Parallel execution waves
> Target 5-8 todos per wave. Fewer than 3 (except the final) means you under-split.

| Wave | Todos | Theme |
| --- | --- | --- |
| F (8) | 1-8 | Foundation: protocol v2, executors, safety, activation |
| R (4) | 9-12 | Routing live, catalog, search, ACP |
| D (4) | 13-15, 33 | Delegation + cost governance + team surfacing |
| P (3) | 16-18 | Plan mode + test runner + gate binding |
| U (6) | 19-24 | UI/UX across four frontends + OSC status |
| Q (4) | 25-28 | CI/evals/quality debt |
| L (4) | 29-32 | Research ledger + docs honesty |

Wave-internal parallelism is allowed where Blocked-by is empty; Wave Q and L items are content-independent and may run alongside earlier waves if dependencies permit. Waves execute F → R → D → P → U → (Q,L). Same-file coordination within waves: todos 1 and 5 both extend protocol.rs — serialize 1 before 5 inside Wave F; six Wave-F todos touch harness-daemon (protocol.rs / server.rs / engine.rs) — coordinate commits on those shared files even where Blocked-by is empty.

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1 protocol v2 | — | 9,15,19,20,21,22,24,33 | 2,3,5,6,7,10,11 |
| 2 history routes | — | 20,22 | 1,3,5,6,7 |
| 3 executors bound | — | 4,13,16,17 | 1,2,5,6,7,10,11 |
| 4 safety on exec | 3 | 17 | 5,6,7 |
| 5 rewind bridge | — | 21,23 | 1,2,3,6,7,10,11 |
| 6 skills runtime | — | 23 | 1,2,3,5,7 |
| 7 mcp cli | — | 23 | 1,2,3,5,6 |
| 8 orchestration wiring | 1,2 | 13,15 | 4,5,6,7 |
| 9 dispatcher live | 1 | 13,20 | 10,11,12 |
| 10 catalog refresh | — | 13 | 9,11,12 |
| 11 fts search | 3 | 22 | 9,10,12 |
| 12 acp stdio | 3 | — | 9,10,11 |
| 13 task(category) | 3,8,9,10 | 14,15 | 16 |
| 14 curated agents | 13 | — | 15,16,17 |
| 15 team surfacing | 1,8,13 | 19,20 | 16,17 |
| 16 plan mode | 1,3 | 21 | 13-15,17 |
| 17 run_tests | 3,4 | 18 | 16 |
| 18 gate binding | 17 | — | 16 |
| 19 tui | 1,15,16,33 | — | 20,21,22 |
| 20 web | 1,2,9,15,16,33 | — | 19,21,22 |
| 21 vscode | 1,5,15,16 | — | 19,20,22 |
| 22 cli | 1,2,11 | — | 19,20,21 |
| 23 grammar unify | 5,6,7 | — | 19-22 |
| 24 osc status | 1 | — | 19-23 |
| 25 ci additions | — | — | any (content-independent; run early) |
| 26 evals driver | — | — | any |
| 27 deep-qa | — | — | any |
| 28 qa fixes | — | — | any |
| 29 research §34-35 | — | 30,32 | any |
| 30 renumber | 29 | 31,32 | any |
| 31 docs honesty | 30 | — | 32 |
| 32 receipt rows | 29,30 | — | 31 |
| 33 cost ledger | 1 | 19,20 | 14,15,16 |

## Todos
> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->

### Wave F — Foundation (protocol + executors + activation)

- [x] 1. Protocol extension: usage/model/fallback/verdict frames (additive to v2, v1-withheld)
  What to do / Must NOT do: Extend harness-daemon/src/protocol.rs ServerMsg with `Usage { turn, model, effort, input_tokens, output_tokens, cache_read, cache_write, cost_usd }` emitted per assistant turn; `FallbackAsk { current, proposed, reason }` + `FallbackResolved { chosen }` carrying harness-providers/src/identity.rs FallbackRequest; `Verdict { turn, verdict: confirmed|refuted|inconclusive, evidence: [event-ids] }` from daemon/src/gate.rs. This is an ADDITIVE extension to the existing protocol v2 (PROTOCOL_VERSION is already 2 = the media surface — do NOT bump it). v1 compatibility is mandatory and mechanical: every new ServerMsg variant is WITHHELD from v1 wires via the crate::compat downgrade (the MediaResult precedent — compat.rs's exhaustive match fails to compile until each new variant is listed: fail-closed by design); on a v1 connection a fallback hop retries the SAME model and surfaces a v1-decodable Notification (no v1 client silently misses an ask). All four in-repo frontends (TUI, web SPA, VS Code, CLI) negotiate client_version = PROTOCOL_VERSION (2) and are updated together with the daemon in this monorepo/plan; v1 withholding targets external or legacy v1 wires only. Add `ClientMsg::FallbackAnswer { ask, chosen }` so fallback asks are answerable over the wire. FallbackAsk resolution MUST flow through the existing PermissionMediator under the session's MediationPolicy (FirstResponder default; deadline_ms carried; timeout ⇒ same-model retry; two divergent client answers resolve exactly-once N1-style) — it is NEVER answered by a direct command-handler bypass. No new ask type may treat client-asserted `client_id` as authority: Designated/LocalOnly enforcement keys on a server-assigned per-connection id (the SSE `'sse-readonly'` pattern, handlers.rs:61) — today `client_id` is self-asserted and HTTP answers arrive hardcoded as "http" (handlers.rs:113-120), which this todo must not amplify. Must NOT change/remove existing 10 ServerMsg variants.
  Parallelization: Wave F | Blocked by: none | Blocks: 9, 15, 19, 20, 21, 22, 24, 33
  References (executor has NO interview context - be exhaustive): harness-daemon/src/protocol.rs (full ServerMsg/ClientMsg enums; PROTOCOL_VERSION already 2); harness-daemon/src/compat.rs (downgrade_for_v1 — its doc comment mandates new variants be listed-or-withheld and states "Rust serde cannot decode unknown enum variants"; exhaustive match is the fail-closed gate); harness-daemon/src/events.rs (SessionEvent, where Diff/Notification ride); harness-daemon/src/gate.rs (verdict vocabulary); harness-providers/src/identity.rs (FallbackRequest shape); harness-intel/src/routing/fallback.rs (FallbackDecision::AskFallback origin); harness-daemon/src/bridge.rs (on_turn_finished verdict sink to mirror).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon` green including NEW tests: (a) fake provider turn emits exactly one Usage frame with model identity; (b) forced AskFallback path emits FallbackAsk then FallbackResolved on answer; (c) scheduled run verdict emits Verdict frame with non-empty evidence ids; AND `cargo clippy --workspace --all-targets -- -D warnings` green — new ServerMsg variants that break exhaustive frame-dispatch matches in harness-cli/harness-tui are patched in-scope of this todo's commit (task 8's sibling-arm precedent), or the commit message explicitly documents that frontend ServerMsg dispatch is wildcard-covered so no sibling patch was required.
  QA scenarios: happy — `cargo nextest run -p harness-daemon usage_frame fallback_ask verdict_frame` (emission tests above); failure — `cargo nextest run -p harness-daemon compat_withholds_v2_frames fallback_mediation_policy` asserting `downgrade_for_v1(Usage|FallbackAsk|FallbackResolved|Verdict) == None`, a decode test where a truncated v1-shaped enum (lacking the new variants) ERRORS on the new frame's JSON, AND a mediation test where a LocalOnly policy ignores a non-designated FallbackAnswer while two divergent designated answers resolve exactly once, evidence .omo/evidence/task-1-ecosystem-alignment.txt
  Commit: Y | feat(daemon): protocol v2 usage/fallback/verdict frames

- [x] 2. History, automations, and team HTTP/WS surfaces
  What to do / Must NOT do: Add routes in harness-daemon/src/server.rs + handlers: `GET /s/{sid}/history` reading harness-daemon/src/run_history.rs (`list()`, `for_automation()` — APIs exist, zero readers today); `GET /automations` listing scheduler entries (name, cron, next-fire, last verdict from RunRecord); route the daemon's Notification kinds `sched`/`verdict` onto the event stream (engine.rs currently publishes only kind "steer"). Must NOT expose admin/reload semantics beyond existing.
  Parallelization: Wave F | Blocked by: none | Blocks: 20, 22
  References: harness-daemon/src/server.rs:262-275 (route table); harness-daemon/src/run_history.rs (list/for_automation/latest_for_session); harness-daemon/src/scheduler.rs + automations.rs (declarative defs); harness-daemon/src/engine.rs (Notification publish path, kind "steer" only today).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon` green with NEW handler tests: history route returns JSON array containing verdict field; automations route lists a fixture automation with next-fire; a scheduled fixture run surfaces a sched Notification to an attached SSE client.
  QA scenarios: happy — `cargo nextest run -p harness-daemon history_route automations_route`; failure — history route on unknown sid returns typed 404 (not 500), evidence .omo/evidence/task-2-ecosystem-alignment.txt
  Commit: Y | feat(daemon): run-history + automations surfaces

- [x] 3. Bind CORE_TOOLS executors in-process
  What to do / Must NOT do: At daemon startup, call harness-core/src/tools/registry.rs `bind_executor` for each CORE_TOOL with in-process implementations: view_window + edit_lines ← harness-tools/src/window.rs + edit.rs; execute_command ← harness-exec; lsp_query ← harness-lsp; tool_search stays internal (registry.rs:244); task_completed is loop-intercepted and needs no executor — after this todo all six CORE_TOOLS are production-served. This closes the NoExecutor production gap (bind_executor currently test-only). Wiring seam: executors are constructed once at startup; a per-session ToolRegistry (ToolRegistry::open is per-session-dir) is opened and bound inside the daemon's session/loop factory, replacing the current stages.execution construction (harness-core/src/agent_loop/tools.rs — the loop dispatches via the stage trait, not the registry directly); bin/harness-daemon.rs injects the factory. Must NOT delete the worker protocol (WorkerToDaemon stays for future out-of-process). Budget sizing is explicit: this todo records `perf-gate` AND `heap-profile --check` (harness-bench) before/after — the daemon gains harness-exec (tree-sitter + 4 C grammars, portable-pty, vt100, win32job, harness-vfs), harness-lsp (sysinfo), harness-tools, harness-skills — if the dhat >10% gate (ci.yml heap-profile job) trips, either lazy-init the new subsystems or re-record baselines per baselines/README.md WITH justification in the commit message (never re-record silently).
  Parallelization: Wave F | Blocked by: none | Blocks: 4, 13, 16, 17
  References: harness-core/src/tools/registry.rs:77 (bind_executor, "the path the core tools take when their real executors arrive (todos 16-19)"), :244-252 (dispatch NoExecutor); harness-core/src/tools/schema.rs:56-58 (CORE_TOOLS list, admits deferred executors); harness-tools/src/window.rs + edit.rs (implementations); harness-exec/src/lib.rs; harness-lsp/src/lib.rs; harness-daemon/src/bin/harness-daemon.rs (startup wiring point); daemon Cargo.toml gains harness-tools/harness-exec/harness-lsp deps.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon` green with NEW e2e: a session turn invoking view_window and edit_lines through the real registry returns tool results (not NoExecutor); grep-verified: `bind_executor` called from non-test daemon src.
  QA scenarios: happy — `cargo nextest run -p harness-daemon executor_bound` (all six tools served); failure — invoking an unregistered tool name returns the typed unknown-tool error (NoExecutor path preserved for unbound tools), evidence .omo/evidence/task-3-ecosystem-alignment.txt
  Commit: Y | feat(daemon): bind core tool executors in-process

- [x] 4. Wire harness-safety into the execute_command path
  What to do / Must NOT do: Route executor #3's execute_command through harness-safety: per-command sandbox policy (OS backend via platform_backend), EnvScrub, egress guard — generalize the pattern harness-media/src/tool already uses (its only production consumer today). Doctor (harness-cli doctor.rs) keeps reporting requested-vs-enforced honestly. Must NOT weaken deny-fails-closed semantics of daemon/src/mediation.rs.
  Parallelization: Wave F | Blocked by: 3 | Blocks: 17
  References: harness-safety/src/lib.rs (backends/*, perm/*, egress.rs, supply.rs); harness-media/src/tool/* (existing sandbox-policy usage pattern); harness-exec/src (spawn/runner where policy applies); harness-cli/src/doctor_probes.rs:174 (platform_backend probe — currently the only safety surface).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-safety -p harness-daemon` green with NEW test: execute_command under a deny-egress policy blocks a curl-shaped command and the result states requested-vs-enforced; Windows restricted-token path exercised via existing safety tests.
  QA scenarios: happy — allowed command runs with policy metadata in result; failure — denied command returns typed block with reason, evidence .omo/evidence/task-4-ecosystem-alignment.txt
  Commit: Y | feat(exec): safety policy on execute_command

- [x] 5. Snapshots → /rewind bridge
  What to do / Must NOT do: Wire Event::Snapshot (harness-core/src/event/mod.rs:135) to harness-snapshots restore points; add ClientMsg::Rewind { steps }; daemon restores via the snapshots crate; TUI /rewind + omnibar (app_input.rs:147-153 currently prints "wiring lands with todo 40/42") sends it. Must NOT auto-restore without explicit user command.
  Parallelization: Wave F | Blocked by: none | Blocks: 21, 23
  References: harness-snapshots/src/lib.rs (crate, zero dependents today; "session log (T4) and loop wire the concrete handle in later todos"); harness-core/src/event/mod.rs:135; harness-tui/src/app_input.rs:147-153 (stub); harness-tui/src/transcript.rs:193 (Event::Snapshot currently discarded `=> {}`); harness-daemon/src/protocol.rs (ClientMsg extension point).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon -p harness-tui` green with NEW tests: rewind command restores snapshot N and emits System event "restored to <label>"; TUI /rewind 2 sends ClientMsg::Rewind{2}; post-rewind, event_fts holds no rows with seq > restored head (index truncated to the restore point — search never returns un-happened events) AND sessions.last_seq/event_count/turn_index are consistent with the restored head: the next append receives seq = head+1 and a fork at the post-rewind head copies only events ≤ head.
  QA scenarios: happy — rewind roundtrip state matches snapshot fixture; failure — rewind beyond oldest snapshot returns typed error naming the oldest available, evidence .omo/evidence/task-5-ecosystem-alignment.txt
  Commit: Y | feat(daemon): snapshot rewind wired

- [x] 6. Skills runtime activation + packs endpoint
  What to do / Must NOT do: Daemon loads skill packs at startup via harness-skills SkillLoader (trust gates + quarantine honored — draft packs never activatable); wire TUI /skill (app_input.rs:154 stub) to invocation; add `GET /packs` (web ui.js packsPanel is a stub admitting "no packs endpoint yet"). Must NOT bypass trust.rs/consent.rs gates for any activation — including TRIGGER-driven activation inside delegated child sessions. Pack install/activation must NOT enable or start MCP servers: pack MCP entries (writes.rs merge_mcp writes them into .harness/mcp.json — an executable manifest every MCP consumer scans) land as DISABLED declarations requiring a separate explicit consent step before any consumer may launch them.
  Parallelization: Wave F | Blocked by: none | Blocks: 23
  References: harness-skills/src/lib.rs (loader/routing/triggers/trust/consent/watch); harness-skills/src/registry/mod.rs (packs carry files/rules/MCP entries + secret declarations); harness-daemon (gains harness-skills dep; currently zero skills surface — grep-verified); harness-tui/src/app_input.rs:154-158; harness-web/assets/ui.js:176-184 (packs stub).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-skills -p harness-daemon` green with NEW tests: quarantined pack listed but not invocable (typed refusal carrying tier); trusted pack invocable via /skill; quarantined skill TRIGGER match inside a delegated child session produces the typed refusal (gates apply to trigger-driven activation, not just /skill); GET /packs returns tiers + trust status; installing a fixture pack carrying an mcp/ entry writes the declaration, starts NO process, and GET /packs renders the entry with a disabled marker.
  QA scenarios: happy — trusted pack invocation injects skill context; failure — quarantined pack invocation refused with quarantine reason, evidence .omo/evidence/task-6-ecosystem-alignment.txt
  Commit: Y | feat(daemon): skills runtime + packs endpoint

- [x] 7. MCP CLI surface
  What to do / Must NOT do: Add `harness mcp` subcommand family in harness-cli (cli.rs Commands enum: Run/Capture/Repl/Attach/Doctor today): `harness mcp list|scan` (manager scan_directories), `harness mcp serve` (harness-mcp server mode), `harness mcp call <server>` smoke. Closes the seam where harness-mcp (client+server, rmcp 3.3) is reachable only via doctor probe. Must NOT auto-enable any MCP server.
  Parallelization: Wave F | Blocked by: none | Blocks: 23
  References: harness-cli/src/cli.rs (Commands enum, line ~52); harness-cli/src/doctor_probes.rs:185 (scan_directories — only existing surface); harness-mcp/src/lib.rs (manager, server.rs reference server, client adapters); harness-mcp/src/acp/ (ACP surface stays separate, task 12).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-cli` green with NEW tests: `mcp list` renders fixture directory scan; `mcp serve` starts and answers initialize over stdio fixture then exits.
  QA scenarios: happy — serve+initialize roundtrip on fixture config; failure — malformed server config produces typed error naming the file + key, evidence .omo/evidence/task-7-ecosystem-alignment.txt
  Commit: Y | feat(cli): mcp subcommand family

- [x] 8. Orchestration wiring: launch path, children events, outposts decision
  What to do / Must NOT do: Wire harness-orchestration's unique capabilities into the daemon: Launcher-driven fork-join children as child sessions with write_set collision alerts + budgets; emit real team events ChildStarted/ChildProgress/ChildFinished (replace never-emitted WorkerClaimed/WorkerHealed — either emit them from the scheduler claim path wiring outposts.rs ClaimLedger, or delete outposts.rs as dead code — implementer decides by which path is smaller, decision recorded in commit message; the three Child* event names are the contract task 15 consumes). New SessionEvent variants must be v1-withheld/downgraded in compat.rs — task 1's v1 contract applies to Event payloads riding ServerMsg::Event, not just ServerMsg variants (NOTE: compat.rs downgrade_frame is a let-else PASS-THROUGH for non-Recorded events — no compile gate exists for SessionEvent, unlike ServerMsg; this todo ADDS the gate). Write-sets are advisory collision surfaces: children never receive auto-allow for paths inside their declared set, and out-of-set writes take the normal permission path (no silent escape — write_set.rs detects sibling overlap only). Must NOT duplicate daemon scheduler/gate (they stay canonical for cron/durable).
  Parallelization: Wave F | Blocked by: 1, 2 | Blocks: 13, 15
  References: harness-orchestration/src/lib.rs (fork-join, spec launcher, budgets, write-sets; crate has zero dependents); harness-orchestration/src/launch.rs (child sessions {launch}-child-{task}); harness-daemon/src/events.rs:55-65 (WorkerClaimed/Healed declared, never emitted); harness-daemon/src/outposts.rs (dead module, 1 ref); harness-daemon/src/scheduler.rs (stays canonical).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-orchestration -p harness-daemon` green with NEW tests: a 2-child launch produces child sessions + collision alert on overlapping write sets; team events visible to an attached client (SSE); `downgrade_for_v1(Event{ChildStarted|ChildProgress|ChildFinished}) == None` (or rewritten) plus a truncated-v1-enum decode-ERROR test on the new variants; `cargo clippy --workspace --all-targets -- -D warnings` green (new SessionEvent variants break exhaustive matches in harness-cli render.rs AND harness-tui transcript.rs — patching those sibling arms is IN SCOPE of this todo's commit).
  QA scenarios: happy — parallel children with disjoint writes complete; failure — overlapping write sets produce collision alert + second child quarantined, evidence .omo/evidence/task-8-ecosystem-alignment.txt
  Commit: Y | feat(daemon): orchestration launch path + team events

### Wave R — Routing live, catalog, search, ACP

- [x] 9. Dispatcher live in the daemon main loop
  What to do / Must NOT do: Route session turns through harness-intel Dispatcher::dispatch_turn (stickiness pins, cheapest_proven tiers, sidekick) from the daemon engine; FallbackDecision::AskFallback now surfaces as protocol v2 FallbackAsk (task 1) — ask-first contract finally visible + answerable. routing.json fully consumed (today only summarizer utility route, bin/harness-daemon.rs:117-141). Must NEVER auto-hop without user confirmation (README contract). The Wave-R→Wave-U interim (fallback asks emitted before frontends can answer) is a DOCUMENTED degraded mode: the v1-path policy (same-model retry + v1-decodable Notification with explicit wording "model fallback asked; no interactive client — retrying same model") is stated in the task's commit and carried into todo 31's README honesty pass.
  Parallelization: Wave R | Blocked by: 1 | Blocks: 13, 20
  References: harness-intel/src/routing/dispatch.rs:75-132 (dispatch_turn, spawn_subtask); resolve.rs:237-293 (Stickiness); harness-daemon/src/projection.rs:204-212 (only current consumer, utility_route); harness-daemon/src/bin/harness-daemon.rs:117-141; routing/config.rs (RoutingSpec); receipt: Claude Code 2.1.296 workflow-level model pinning (2026-10-09).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-intel -p harness-daemon` green with NEW e2e: role-routed turn resolves via routing.json role entry; forced provider failure emits FallbackAsk; acknowledged hop revalidates effort grid (dispatch.rs:94-117 path live).
  QA scenarios: happy — `cargo nextest run -p harness-daemon dispatcher_live stickiness_resume` (pinned profile sticks across resume); failure — `cargo nextest run -p harness-daemon fallback_no_silent_substitution` (unacknowledged fallback retries the SAME model; zero substitution events), evidence .omo/evidence/task-9-ecosystem-alignment.txt
  Commit: Y | feat(daemon): dispatcher live, ask-first fallback surfaced

- [x] 10. Model catalog refresh (verified 2026 wave)
  What to do / Must NOT do: Extend harness-intel/src/seed.rs catalog: GPT-6 Sol, GPT-6.1 Sol, GPT-6 Luna, Gemini 4 Argon, Grok 4.7, Claude Opus 5.5, Sonnet 5.5, Haiku 5.5 (keep existing 12). Each entry gets supported_efforts grid + trap-table rows per model-profiles.md discipline (R2 vendor adapter keyed on model-name generation; R5 parameter legality). Must NOT remove existing entries or guess unverified capability flags — unverified = conservative default + TODO marker in seed data.
  Parallelization: Wave R | Blocked by: none | Blocks: 13
  References: harness-intel/src/seed.rs (12 seeded models); harness-intel/src/catalog.rs:48-61 (EffortValue), :191-214 (CatalogEntry, trap table), :252 (fail-closed validate); .omo/research/model-profiles.md (axes A1-A8, rules R1-R6); wave receipts in research synthesis (m0024-era: Sol Sep 22 / 6.1 Sol Sep 29 / Grok 4.7 Sep 21 / Argon Oct 19 / Opus-Sonnet-Haiku 5.5 Sept-Oct).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-intel` green incl. catalog validation tests for all new entries; `cargo run -p harness-cli -- doctor` reports expanded catalog without unknown-model warnings.
  QA scenarios: happy — new model routes through bracket grammar `model[effort=xhigh]`; failure — illegal effort for a model (per grid) rejected fail-closed, evidence .omo/evidence/task-10-ecosystem-alignment.txt
  Commit: Y | feat(intel): 2026-10 model catalog wave

- [x] 11. event_fts query path: cross-session transcript search
  What to do / Must NOT do: Add SELECT surface over the already-maintained event_fts index: harness-sessions search API (FTS5 MATCH, session-scoped + cross-session), daemon `GET /sessions/search?q=`, CLI `harness sessions search`, and a model-facing search_sessions tool registered via task-3's registry. The model-facing tool is session-scoped BY DEFAULT; cross-session search requires explicit session ids and never crosses working-dir roots (cross-session hits flow into model context → provider egress). Receipt note: Zed #58686 is a roadmap ticket, not shipped-proven — justified under the filter's lightweight clause (a reader over an already-maintained index), which this todo states explicitly.
  Parallelization: Wave R | Blocked by: 3 (tool registry binding) | Blocks: 22
  References: harness-sessions/src/migrations.rs (event_fts creation); harness-sessions/src/session.rs:238 + fork.rs:106 (writes on every append/fork; the one existing SELECT is harness-sessions/src/meta.rs:108 — reuse its MATCH/bm25/snippet/rank idiom for the search API rather than mirroring retrieval.rs); harness-sessions/src/retrieval.rs (memory_fts query pattern); harness-daemon/src/server.rs (route table); harness-cli/src/cli.rs.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-sessions -p harness-daemon -p harness-cli` green with NEW tests: seeded 3-session store, query returns ranked hits with session ids + seq; empty result is 200 with empty array (not 404); tool invocation returns bounded results.
  QA scenarios: happy — phrase match across sessions ranks by recency-weighted bm25; failure — FTS5 syntax-error query returns typed 400 (not 500), evidence .omo/evidence/task-11-ecosystem-alignment.txt
  Commit: Y | feat(sessions): event_fts search surface

- [x] 12. ACP stdio surface
  What to do / Must NOT do: Expose harness-mcp/src/acp (AcpAgent/AcpEngine/AcpWire — implemented + tested, zero daemon/cli refs) as `harness-daemon --acp` stdio mode bridging ACP sessions to daemon sessions: initialize, session/prompt, session/update (diff tool_call), fs/read_text_file, session/cancel. Receipts: Zed 1.21 ACP async wakeup; Cursor CLI ACP child sessions; goose W3C trace ACP→MCP; Gemini CLI usage_update; §23.8 ADOPT. Must NOT grant ACP clients more than the permission engine allows local clients. ACP mediation contract: the bridge joins the mediation queue under a distinct server-assigned client id subject to the session's MediationPolicy; FallbackAsk and plan approval are NOT answerable over ACP v1 (deadline ⇒ same-model retry / deny per tasks 9/16); the permission-request mapping is GREENFIELD (harness-mcp/src/acp has zero permission code today — the plan builds it, it does not exist).
  Parallelization: Wave R | Blocked by: 3 | Blocks: none
  References: harness-mcp/src/acp/{mod,agent,types,channel}.rs (full v1 agent surface); harness-mcp/tests/acp_agent.rs (protocol tests to mirror); harness-daemon/src/bin/harness-daemon.rs (new flag); receipts in .omo/research synthesis + plan draft C3.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-mcp -p harness-daemon` green with NEW stdio e2e: spawn `harness-daemon --acp`, drive initialize + session/prompt fixture, assert session/update diff events flow and permission asks map to ACP permission requests.
  QA scenarios: happy — full ACP roundtrip against fixture provider; failure — unauthorized tool via ACP denied through same mediation queue (deny-fails-closed preserved), evidence .omo/evidence/task-12-ecosystem-alignment.txt
  Commit: Y | feat(daemon): ACP stdio agent mode

### Wave D — Delegation

- [x] 13. task(category) delegation tool + RoutingSpec categories
  What to do / Must NOT do: Add `Category` variant to RouteRequest (harness-intel/src/routing/mod.rs:181); extend RoutingSpec with categories[] (model + effort + score_floor like tiers); register a model-facing `task` tool (category + prompt) that spawns a child session via task-8's launch path, routes via Dispatcher, returns child summary. Ask-first chains preserved (a chain hop mid-task is a FallbackAsk). Receipts: OmO categories; Claude fork-subagents default-on; Cursor Projects; Qwen managed-agent; Amp Pucks.
  Parallelization: Wave D | Blocked by: 3, 8, 9, 10 | Blocks: 14, 15
  References: harness-intel/src/routing/mod.rs:181 (RouteRequest); routing/config.rs:77-106 (RoutingSpec); harness-orchestration/src/launch.rs; harness-core/src/tools/registry.rs (register + bind pattern from task 3); harness-daemon/src/engine.rs (child spawn); effort-scaling rule receipt model-profiles.md T10 (A6 subagent storms — cap fan-out, default 4, escalation criteria explicit).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-intel -p harness-daemon` green with NEW tests: task(category=quick) routes to category model; fan-out >4 without explicit escalation refused with typed error; child result returns to parent transcript as tool result — routed through clamp_summary at the role's summary_budget and labeled as child output; a CHILD session invoking `task` is refused with the typed DepthExceeded error (fork.rs DepthGuard enforced on the task-tool path, default depth 1); task-tool spawns carry FRESH grant state (no inherited AllowSession/GrantAlways(Session) grants); the fan-out cap and token budget aggregate PER PARENT SESSION across task() calls (not per launch — BudgetGuard today is per-launch with a 20M-token ceiling that is not a cost guard).
  QA scenarios: happy — two parallel category tasks return summaries; failure — unknown category typed error listing valid ones; chain hop triggers FallbackAsk not silent substitution, evidence .omo/evidence/task-13-ecosystem-alignment.txt
  Commit: Y | feat(core): task delegation with categories

- [x] 14. Curated cheap-pool read-only agents (explore, librarian)
  What to do / Must NOT do: Ship two curated subagents bound to the cheap pool: explore (fast codebase grep — search tools only) and librarian (docs/OSS search — existing research/tools surface), enforced by RoleClass::Utility cheap-pool validation (table.rs:91-96 exists). Read-only tool allowlists. Receipts: OmO explore/librarian on cheap-fast models; utility-roles-must-be-cheap is our own validated rule.
  Parallelization: Wave D | Blocked by: 13 | Blocks: none
  References: harness-orchestration/src/role.rs:139-179 (RoleRegistry + with_declarations); harness-intel/src/routing/table.rs:91-96 (cheap-pool validation); RoutingSpec cheap_pool; tool allowlist mechanism in RoleDef.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-orchestration -p harness-daemon` green with NEW tests: explore invoked via task tool routes to cheap pool + read-only allowlist (write attempt refused); registry rejects an explore pinned to a non-cheap model (validation live).
  QA scenarios: happy — explore returns grep-shaped findings summary; failure — write tool call inside explore refused + logged, evidence .omo/evidence/task-14-ecosystem-alignment.txt
  Commit: Y | feat(orchestration): curated explore + librarian agents

- [x] 15. Team surfacing: children, budgets, status precedence
  What to do / Must NOT do: Surface delegation state: protocol frames ChildStarted/ChildProgress/ChildFinished (from task-8 events; new SessionEvent variants are v1-withheld/downgraded per task 1's compat contract) + aggregate status with Warp's precedence algorithm (InProgress > Blocked > Error > Cancelled > Success, plus WaitingForEvents — never flip to terminal while any child in-progress; receipt Warp PR#11367/#12084); budgets + collision alerts rendered (TUI team panel upgrades from queue/worker rows; web renders state.workers it already captures but never draws).
  Parallelization: Wave D | Blocked by: 1, 8, 13 | Blocks: 19, 20
  References: harness-daemon/src/events.rs; harness-tui/src/render.rs:73-83,109-114 (team accordion today); harness-web/assets/store.js:107-113 (workers captured, ui.js never renders); Warp precedence receipts in plan draft C9.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon -p harness-tui` green with NEW tests: 3-child fixture (1 in-progress, 1 blocked, 1 succeeded) aggregates to InProgress-not-terminal; blocked child surfaces ask; budget exhaustion halts further spawns with typed event; new Child* frames are v1-withheld (`downgrade_for_v1` test, task 8's gate).
  QA scenarios: happy — team panel shows children + aggregate; failure — budget-exceeded spawn refused + visible, evidence .omo/evidence/task-15-ecosystem-alignment.txt
  Commit: Y | feat(daemon): team surfacing + status precedence

### Wave P — Plan mode + test runner

- [x] 16. Interactive plan mode (main session)
  What to do / Must NOT do: Add an opt-in plan-approval flow: mode flag on session start (off by default); in plan mode the agent produces a plan artifact (markdown, task list + files-to-touch) and the loop gates ALL write tools until the user approves/edits/rejects via protocol command; approval flips autonomy back per matrix; wire TUI /mode to enter/exit plan mode (replacing the app_input.rs:141-146 stub). Building blocks exist: phases.rs Strategy→Execution→Verification + gate.rs verdicts + autonomy/matrix.rs capability gating. Receipts: Claude Code plan mode shipped; Zed roadmap Plan Mode (Coming Up Next); Antigravity artifacts (plan-as-editable-artifact with inline comments); Antigravity v1.3.2: skip-permissions must not auto-approve plans.
  Parallelization: Wave P | Blocked by: 3, 1 (protocol v2 frame conventions) | Blocks: 21
  References: harness-core/src/autonomy/matrix.rs (AutonomyMode + capability matrix); harness-orchestration/src/phases.rs (BoostPhase); harness-daemon/src/gate.rs; protocol.rs (plan command extension); Cursor Rollouts receipt (editable plan artifact).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon` green with NEW tests: in plan mode an edit_lines call is blocked pre-approval and the block event carries the pending plan id; approve → edit proceeds; edit-the-plan → updated artifact re-gates; plan mode OFF by default — the existing daemon test suite passes unchanged AND a golden session-transcript fixture (recorded at pre-task-16 HEAD and committed BEFORE implementation begins) byte-compares equal with mode off; entering plan mode SUSPENDS all session-scoped grants (GrantScope::Session) until approval — the existing enter_plan() (matrix.rs:193) is flag-only and does NOT touch the grant store, so this todo adds the suspension; write-tool execution re-checks the plan gate at execution time (in-flight calls included); approve/reject protocol commands are subject to the same MediationPolicy machinery as other asks (task 1's routing contract).
  QA scenarios: happy — plan→approve→execute flow completes; failure — write attempt before approval typed-blocked (never silently allowed), evidence .omo/evidence/task-16-ecosystem-alignment.txt
  Commit: Y | feat(core): interactive plan mode

- [x] 17. Test-runner tool: detection + structured results
  What to do / Must NOT do: New `run_tests` tool: detect framework day-one = cargo test + cargo nextest (workspace-native; configurable command override extends the EXISTING session config shape — no new config format, per guardrail); execute via executor #3 path (safety policies apply); parse results (per-test pass/fail/ignored + durations) into a typed TestReport, not raw text. run_tests is classified in the SAME permission tier as execute_command (asks unless policy allows — it executes build scripts and test binaries, i.e. arbitrary code; never auto-allow); the command override is read ONLY from daemon-owned user-global config — a repo-local override is refused with a typed error naming the file (DaemonConfig has no session-level test-command shape today; the todo adds the field to the EXISTING daemon config, not a repo-local format). Receipts: Zed #5242 (886 upvotes); Qwen Test Failure Explorer (shipped 2026-10); JetBrains native.
  Parallelization: Wave P | Blocked by: 3, 4 | Blocks: 18
  References: harness-exec (execution); existing CLI/eval patterns for nextest parsing (harness-evals uses nextest archive); harness-core tool registration from task 3.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon` green with NEW tests: run_tests on a fixture crate with 1 passing + 1 failing test returns TestReport with exactly those entries; unknown framework → typed error with config instructions; an auto-allow attempt under default policy is refused (tier parity with execute_command asserted); a repo-local override file is refused with a typed error.
  QA scenarios: happy — structured report usable without parsing text; failure — test binary crash surfaces as typed error with exit code, evidence .omo/evidence/task-17-ecosystem-alignment.txt
  Commit: Y | feat(tools): run_tests with framework detection

- [x] 18. Test evidence → completion gate integration
  What to do / Must NOT do: Feed task-17 TestReports into daemon/src/gate.rs evidence bundles: gate criteria accept "tests: suite X green" bindings; verdict evidence links include test-run event ids. A REFUTED verdict must cite failing test ids when tests ran.
  Parallelization: Wave P | Blocked by: 17 | Blocks: none
  References: harness-daemon/src/gate.rs (CriteriaGate, evidence bundle); harness-daemon/src/bridge.rs (verdict sink); task-17 TestReport type.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon` green with NEW tests: scheduled run whose criteria bind a suite; suite green → CONFIRMED cites test event; suite red → REFUTED cites failing test ids.
  QA scenarios: happy — verdict evidence drilldown reaches test cases; failure — missing test run yields INCONCLUSIVE (never CONFIRMED without bound evidence), evidence .omo/evidence/task-18-ecosystem-alignment.txt
  Commit: Y | feat(daemon): gate criteria bind test evidence

### Wave U — UI/UX (progressive disclosure + macro audit)

- [x] 19. TUI: render diffs (hunk panel), live HUD, scroll fix
  What to do / Must NOT do: (a) render view.diffs() — the TUI collects DiffEntry today and render.rs never draws them (grep-verified; README's "hunk review gates" is currently false): add a diffs accordion with per-hunk accept/reject; (b) feed Hud::apply with task-1 Usage frames so cache/cost/model go live (HudUpdate currently test-only); (c) remove/blank dead fields until fed; (d) implement ScrollUp/ScrollDown dispatch (bound in keys.rs, no dispatch arm — dead bindings); (e) collapse transcript detail by default per the 5-project convergence receipt (Amp/Antigravity/Claude/Devin/goose) — one line per tool with diffstats, expand on demand; (f) render + answer FallbackAsk (reuse the permission-ask modal) and plan-mode approve/reject prompts (task 16 commands; TUI /mode enters plan mode per task 16).
  Parallelization: Wave U | Blocked by: 1, 15, 16, 33 | Blocks: none
  References: harness-tui/src/render.rs (body_lines — no diffs call), transcript.rs:134-139 (DiffEntry collected), hud.rs (HudUpdate apply test-only), keys.rs (dead scroll bindings); receipts in draft C9.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-tui` green with NEW tests: a Diff event renders a hunk line + accept/reject updates engine state; Usage frame updates HUD cost/cache (no n/a); scroll offset applies to the focused panel (in-memory; no persistence requirement); diff with unparseable hunk renders placeholder not crash; an attach-level assertion confirms the negotiated protocol version is 2 (flipping a frontend that proposes 1 is in-scope for this todo).
  QA scenarios: happy — diff → accept flow visible end-to-end; failure — diff with unparseable hunk renders placeholder not crash, evidence .omo/evidence/task-19-ecosystem-alignment.txt
  Commit: Y | feat(tui): hunk panel, live HUD, scroll

- [x] 20. Web dashboard: history, timeline, workers, focus view
  What to do / Must NOT do: (a) session-history view consuming task-2 routes: per-session cost/model totals, verdicts with evidence links, asks answered (resolvedLog finally rendered); (b) per-turn changed-files timeline (receipt: OpenCode #55064 Changed-files-by-turn; Devin Session Insights timeline) fed by Diff events; (c) render state.workers (captured since forever, never drawn); (d) collapsed-by-default transcript with expand (focus-view receipt Claude /focus); (e) trends panel fed by v2 frames (delete the "not carried by protocol v1" confession); (f) FallbackAsk answer UI reusing the gates surface + plan-mode approve/reject (task 16 commands).
  Parallelization: Wave U | Blocked by: 1, 2, 9, 15, 16, 33 | Blocks: none
  References: harness-web/assets/ui.js (sessionView, trendsPanel confession at ~:172, packs stub :176-184), store.js:107-113 (workers), share.js (unchanged); task-2 routes; receipts draft C9.
  Acceptance criteria (agent-executable): `node harness-web/e2e/run-e2e.mjs` green (task-25 wires it into CI) with NEW assertions: history view shows fixture run cost+verdict; timeline lists changed files per turn; workers panel non-empty with fixture children; the e2e connection negotiates protocol version 2 (flipping a web client that proposes 1 is in-scope for this todo).
  QA scenarios: happy — dashboard renders macro→micro drill (session → turn → file); failure — empty history renders empty-state not spinner, evidence .omo/evidence/task-20-ecosystem-alignment.txt
  Commit: Y | feat(web): history, timeline, workers, focus

- [x] 21. VS Code: transcript, frames, Esc-deny fix, grammar
  What to do / Must NOT do: (a) minimal transcript webview (today VS Code has NO transcript — drops all non-turn/diff frames `default: break`); (b) handle Condensation/Notification/team frames; (c) fix the silent-deny footgun: dismissed prompt (Esc) must require an explicit choice or time out per mediation deadline — never silent deny; (d) omnibar grammar changes are owned by task 23 — this todo must NOT touch grammar. (e) render + answer FallbackAsk and plan-mode approve/reject via the existing dialog surfaces (tasks 1/16).
  Parallelization: Wave U | Blocked by: 1, 5, 15, 16 | Blocks: none
  References: harness-vscode/src/session.ts:121 (`default: break`), :173-182 (promptAsk, pick===undefined→deny), extension.ts (7 commands), package.json; harness-daemon/src/mediation.rs (deadline semantics).
  Acceptance criteria (agent-executable): `npm test` in harness-vscode green (task-25 wires CI) with NEW tests: Esc on a prompt leaves ask pending until deadline (not denied); transcript webview renders tool/condensation/team entries from fixture stream; the extension's connection negotiates protocol version 2 (flipping it if it proposes 1 is in-scope for this todo).
  QA scenarios: happy — ask answered explicitly or expires with typed outcome; failure — malformed frame ignored without breaking webview, evidence .omo/evidence/task-21-ecosystem-alignment.txt
  Commit: Y | feat(vscode): transcript, explicit prompts

- [x] 22. CLI: sessions verbs, condensation render, NDJSON completeness
  What to do / Must NOT do: Add `harness sessions list|search|history` verbs (routes from tasks 2+11); render Condensation in human output (render.rs returns None today — invisible); human render also shows fallback asks and plan-mode approval states; NDJSON passes all v2 frames (usage/fallback/verdict/team) for eval drivers. CLI is deliberately display-only for asks: an unanswered FallbackAsk follows task 9's same-model-retry policy; plan approval requires an attached interactive frontend (tasks 19/20/21).
  Parallelization: Wave U | Blocked by: 1, 2, 11 | Blocks: none
  References: harness-cli/src/cli.rs (Commands enum), render.rs:87 (Condensation → None), run.rs/attach.rs (output paths).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-cli` green with NEW tests: `sessions search` prints ranked hits; human render shows "~ condensed" line; NDJSON fixture contains usage frame; the CLI attach/NDJSON connection negotiates protocol version 2 (flipping it if it proposes 1 is in-scope for this todo).
  QA scenarios: happy — verbs work against fixture daemon; failure — search with FTS syntax error prints typed error exit≠0, evidence .omo/evidence/task-22-ecosystem-alignment.txt
  Commit: Y | feat(cli): sessions verbs + frame completeness

- [x] 23. Terminology + grammar unification pass (all frontends)
  What to do / Must NOT do: One interaction grammar across TUI/web/vscode/cli: steering = same verb everywhere (TUI implicit stays, explicit /steer everywhere else); /cancel means cancel-turn everywhere (web's cancel-vote becomes /vote-cancel or explicit label); allow-scope labels identical (y=once, a=session, n=deny); "snapshot" disambiguated (rewind-point vs share-snapshot vs resync renamed in UI copy); team vocabulary unified to "agents". Update docs snippets to match.
  Parallelization: Wave U | Blocked by: 5, 6, 7 | Blocks: none
  References: audit B-section (3 steering grammars, divergent slash vocab, 3 snapshot meanings); harness-tui/src/app_input.rs; harness-web/assets/ui.js; harness-vscode/src/omnibar.ts; harness-cli/src/repl.rs.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-tui -p harness-cli` + `npm test` (vscode) green with updated tests asserting the shared grammar table; grep check: no frontend mentions a grammar the others lack.
  QA scenarios: happy — same cheat-sheet renders from one table in all frontends; failure — unknown slash command in any frontend gives identical help, evidence .omo/evidence/task-23-ecosystem-alignment.txt
  Commit: Y | feat(ui): unified command grammar

- [x] 24. Program-status emission (OSC 777 now, 7501 tracked) + attention states
  What to do / Must NOT do: Daemon emits terminal program-status: working / blocked (needs permission or input) / done / error via OSC 777 (pattern already wired at bin:247 stdout_sink — extend the Notifier with kind→status mapping; receipts: OpenCode #55172, Antigravity v1.4.0, Warp OSC-777 lifecycle) and implement OSC 7501 Program Status Protocol emission alongside (Warp issue #16407: Crush emits, cmux receives — emerging standard; we emit both, receive neither). Frontend surfacing of the blocked/attention state rides the v2-frame consumption already delivered by tasks 19-21 (their HUD/gauge/status lines render it); this todo adds only daemon emission + tests.
  Parallelization: Wave U | Blocked by: 1 | Blocks: none
  References: harness-daemon/src/osc.rs (OscSink wired), notify.rs (TracingSink today; "OSC/desktop land todo 41" comment), mediation.rs (deadline → blocked state source); receipts draft C9.
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-daemon` green with NEW PTY-capture test: session lifecycle emits expected 777 + 7501 sequences at working/blocked/done transitions; permission ask flips status to blocked within one event loop turn; every 7501/777 payload field passes sanitize_osc_text (injection test: a session label containing ESC produces exactly the framing ESC bytes — sanitize covers C0+C1 today; the NEW 7501 path must inherit it).
  QA scenarios: happy — external terminal shows correct tab status through a full session; failure — unknown kind does not emit (no garbage sequences), evidence .omo/evidence/task-24-ecosystem-alignment.txt
  Commit: Y | feat(daemon): OSC program-status emission

### Wave Q — Quality (CI, evals, tests, hygiene)

- [x] 25. CI additions: wasm, vscode+web-e2e, doctests, --locked
  What to do / Must NOT do: ci.yml gains: (a) `cargo clippy -p harness-wasm --all-targets --features wasm-plugins -- -D warnings` + `cargo nextest run -p harness-wasm --features wasm-plugins` (entire wasmtime runtime currently 0% CI coverage); (b) job: `npm ci && npm test` in harness-vscode + `node harness-web/e2e/run-e2e.mjs` — runs-on windows-latest (repo is Windows-first; run-e2e.mjs is dev-Windows-bound today: HARNESS_E2E_EXE + LOCALAPPDATA chrome path + NODE_PATH — parameterizing the exe/browser env vars, PLAYWRIGHT_CHROMIUM override already exists, is IN SCOPE); (c) `cargo test --doc --workspace` step; (d) `--locked` on all cargo invocations; (e) fix stale "perf-gate stub" header comment (ci.yml:1).
  Parallelization: Wave Q | Blocked by: none (content-independent) | Blocks: none
  References: .github/workflows/ci.yml:1 (stale comment), :72-77 (feature matrix covers only harness-bench), :105-123 (nextest default features); harness-wasm/Cargo.toml (wasm-plugins default-off; 5 test files `#![cfg(feature)]` inner-gated); harness-vscode/test/*.test.ts (9 suites); harness-web/e2e/run-e2e.mjs.
  Acceptance criteria (agent-executable): push branch → all new jobs green; locally: `cargo clippy -p harness-wasm --all-targets --features wasm-plugins -- -D warnings` exit 0; `cargo test --doc --workspace` exit 0.
  QA scenarios: happy — wasm tests run count >0 in CI log (assert in workflow via nextest junit); failure — intentionally break a doctest on a throwaway `ci-probe/*` scratch ref and confirm the job fails on that ref (then delete the ref; never red-commit the working branch), evidence .omo/evidence/task-25-ecosystem-alignment.txt
  Commit: Y | ci: wasm/vscode/web-e2e/doctests coverage

- [x] 26. Evals driver + scheduled workflow
  What to do / Must NOT do: Add a `run_sweep` bin to harness-evals (library-only today, zero consumers) exposing the conformance sweep; the bin ALSO ships a hermetic scenario pack for the new mechanisms — golden-prefix conformance pins (conformance.rs pin_prefix) for ask-first fallback + task(category) fan-out, EvalCase datasets (dataset.rs) for plan-gate refusal + run_tests report shape + search_sessions scoping, run under the fake provider per policy (the plan's headline mechanisms must not ship unreceipted); new `.github/workflows/evals-scheduled.yml` (schedule + workflow_dispatch) running the HERMETIC conformance sweep per policy — live-model sweeps stay scheduled-only and never gate PRs (ci.yml:12-15 promise, finally honored). Report artifact lands under docs/release/ per evals.adoc publication rules.
  Parallelization: Wave Q | Blocked by: none | Blocks: none
  References: harness-evals/src (sweep.rs, runner.rs, pinning.rs; 58 hermetic tests prove the machinery); harness-evals/Cargo.toml (no [[bin]]); .github/workflows/ci.yml:12-15; docs/release/evals.adoc:49-50 ("When the first scheduled sweep runs...").
  Acceptance criteria (agent-executable): `cargo run -p harness-evals --bin run_sweep -- --conformance` exit 0 producing report artifact; workflow_dispatch run green in Actions.
  QA scenarios: happy — sweep produces harness-matched table artifact; failure — sweep with corrupted cassette fixture fails loudly (non-zero, named cassette), evidence .omo/evidence/task-26-ecosystem-alignment.txt
  Commit: Y | feat(evals): run_sweep driver + scheduled workflow

- [x] 27. deep-qa.yml: scheduled ignored-tests + bench-smoke
  What to do / Must NOT do: New scheduled workflow SPLIT by runner OS (single-OS is internally contradictory): windows-latest job runs pty_qa (3 of its 7 ignored tests are #[cfg(windows)] — unrunnable on ubuntu); ubuntu-latest job with Docker runs container_args + installs pinned rust-analyzer for the goldens + runs the cross-platform remainder (qa transcripts, stress); per-OS accounting of the ≥21 target (the union across both jobs covers it; neither alone does). `cargo nextest run --workspace --run-ignored only --exclude harness-media` on each runner. Bench-smoke job: `cargo bench -p harness-bench -- --warm-up-time 1 --measurement-time 2 --sample-size 10` (6 criterion benches currently never run). Media real-tool tiers stay opt-in by design (documented in the workflow).
  Parallelization: Wave Q | Blocked by: none | Blocks: none
  References: audit R4 inventory (15 files, 31 ignored); harness-bench/benches/*.rs (6 files); .github/workflows/ (4 files today, no schedule anywhere).
  Acceptance criteria (agent-executable): workflow_dispatch run green; log shows ≥21 ignored tests executed.
  QA scenarios: happy — goldens pass on pinned rust-analyzer version; failure — a golden drift surfaces as named failure (not silent skip), evidence .omo/evidence/task-27-ecosystem-alignment.txt
  Commit: Y | ci: deep-qa + bench-smoke schedules

- [x] 28. Repomap failure paths, tokio-tungstenite unification, baseline acceptance
  What to do / Must NOT do: (a) harness-repomap/tests/malformed_inputs.rs ≥3 NEW failure cases (empty graph, self-loops, no-symbols source) expanding coverage beyond the existing malformed-tables test (repomap.rs:197), each asserting typed degraded behavior; (b) BOTH 0.28 pins → `workspace = true`: harness-cli/Cargo.toml:38 AND harness-daemon/Cargo.toml dev-dependencies (~line 66) — dual 0.28+0.29 majors in Cargo.lock violate the root comment's single-version claim; (c) run perf-gate acceptance once at HEAD: `cargo run --release -p harness-bench --bin perf-gate -- --budgets harness-bench/budgets.json --baseline harness-bench/baselines/gate_baseline.json` — if green, baselines de-facto current (close R8); if red, re-record with the established procedure + commit.
  Parallelization: Wave Q | Blocked by: none | Blocks: none
  References: harness-repomap/tests/{graph,pagerank,render,repomap}.rs (35 tests, 0 failure-path); harness-cli/Cargo.toml:38; root Cargo.toml:225-230 (comment); harness-bench/baselines/ (recorded at c8926a4 2026-09-14, HEAD a8ce27f).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-repomap` green incl. new failure tests; `cargo tree -i tokio-tungstenite` (or lockfile grep) shows single version; perf-gate command exit 0 or re-recorded baseline committed.
  QA scenarios: happy — all three sub-checks pass; failure — perf-gate failure names the regressed metric + re-record procedure link, evidence .omo/evidence/task-28-ecosystem-alignment.txt
  Commit: Y | fix(qa): repomap failure paths, tungstenite unification, baseline acceptance

### Wave L — Ledger + docs honesty

- [x] 29. Research ledger: new sections + WATCH entries
  What to do / Must NOT do: Append to .omo/research/harness-research.md: §34 OpenCode 2.0 (daemon-by-default convergence — receipt for our architecture; plugin-API break + OmO Native pivot as the plugin-dependency cautionary receipt; LSP-removed decision recorded); §35 OmO iteration (categories/curated agents/chains — cite what we adopted in tasks 13-14 and what we refused: silent chain-walk); Zed §23 addendum (1.20-1.24: ACP async wakeup, subagent model selection, auto-compaction); WATCH entries: goose→AAIF governance, Pi (116k stars) pending study, BYO-subscription auth wave, Devin memory standard (memory.rs interop), plugin marketplace (AVOID pending stabilization), background-UX, git-history-context (Zed in-progress), OSC 7501 standardization, voice, notebooks, web-IDE, Code-Mode MCP-scripting, /highlights-style in-product release-notes pattern (cataloged, deliberately not adopted). Continue/Clang death-verdict confirmations dated.
  Parallelization: Wave L | Blocked by: none | Blocks: 30, 32
  References: synthesis in .omo/drafts/ecosystem-alignment.md findings; research file structure (## N. headers); README receipts table (new rows cite §34/§35).
  Acceptance criteria (agent-executable): headers present + numbered uniquely (no duplicates — see task 30); every new ADOPT/WATCH row cites a dated external source or in-repo commit.
  QA scenarios: happy — linkcheck passes (.github/scripts/linkcheck.ps1); failure — a claim without citation is flagged by the self-review grep (no bare "proven by" without §/URL), evidence .omo/evidence/task-29-ecosystem-alignment.txt
  Commit: Y | docs(research): §34-35 + WATCH wave

- [x] 30. Research file renumbering fix (duplicate §6, §10-§13)
  What to do / Must NOT do: Renumber the duplicated section headers (§6 ×2 at lines 243/683; §10 ×2 423/982; §11 ×2 490/1298; §12 ×2 573/1592; §13 ×2 629/1846; §8 out-of-order 681) so every §N is unique; update all "research §N" citations in README.md, docs/**, and docs/research-companion.adoc to the new numbers (companion maps §10/§11/§13 — reconcile).
  Parallelization: Wave L | Blocked by: 29 (renumber after new sections land) | Blocks: 31, 32
  References: docs audit D6 (duplicate header lines); docs/research-companion.adoc (section map); README citation lines (§6 at :145/:153, §13 at :144, etc.).
  Acceptance criteria (agent-executable): `Select-String '^## '` count == unique count (no duplicate numbers); grep "research §N" across README/docs resolves each N to exactly one header; companion index matches.
  QA scenarios: happy — every citation resolves uniquely; failure — grep finds an unresolved §N (fix before commit), evidence .omo/evidence/task-30-ecosystem-alignment.txt
  Commit: Y | docs(research): unique section numbering

- [x] 31. README + docs honesty pass (production-wired vs library-tested)
  What to do / Must NOT do: README.md: (a) new two-column status distinction — production-wired vs library-tested (post tasks 1-28 most former seams become wired; anything still library-only says so explicitly); (b) fix HEAD pin (D1, stale c8926a4 → current); (c) fix stale crate count in docs (they say 24; the workspace is ALREADY 25 — media+tools existed undocumented; this plan adds no crates) and add harness-media + harness-tools to the architecture diagram and Status paragraph (D4/D5); (d) "no releases tagged" → v0.1.0 acknowledgment (D3); (e) env var list + HARNESS_PROVIDER_TIMEOUT_SECS (D10). docs/architecture.adoc:54 crate map rows for media/tools; index.adoc:42 count; leaderboard.adoc HEAD relabel (D2); CONTRIBUTING tag-naming (D8); CODE_OF_CONDUCT.md placeholder-mailbox callout matching the one SECURITY.md:15 already carries (D7 — do NOT invent an email). ALSO: document the new config surfaces in docs/partials (multi-model.adoc: routing.json categories/tiers/cheap_pool schema example + fallback/mediation policy keys; getting-started.adoc: test-command override + automations) — routing.json is documented NOWHERE today outside bin comments; and add a "What's new / first-run" section to getting-started.adoc covering categories, plan mode, /rewind, /skill, unified grammar, automations, cost caps (onboarding receipt: OpenCode /highlights pattern evaluated, WATCH-recorded as not-adopted).
  Parallelization: Wave L | Blocked by: 30 | Blocks: none
  References: docs audit D1-D10 table in draft C7; README.md lines 46 + 249 (no-releases claim), 104 (24-crate), 106-129 (diagram), 209-223 (Status HEAD pin); docs/architecture.adoc:54; docs/index.adoc:42; docs/release/leaderboard.adoc:26; CONTRIBUTING.md:125; SECURITY.md:14-15; CODE_OF_CONDUCT.md:63.
  Acceptance criteria (agent-executable): grep "24-crate|Twenty-four" → 0 matches; `git rev-parse --short HEAD` matches README pin; diagram contains harness-media and harness-tools.
  QA scenarios: happy — fresh-clone reader can tell what runs in production vs what is library-only; failure — any unverified "works at HEAD" claim for a library-only feature (self-review grep against the distinction table), evidence .omo/evidence/task-31-ecosystem-alignment.txt
  Commit: Y | docs(readme): honesty pass + counts + media/tools

- [x] 32. README receipt rows for adopted mechanisms
  What to do / Must NOT do: Add rows to the README receipts table (each: mechanism | proven-by | failure-avoided): daemon+thin-client convergence (OpenCode 2.0/Codex /daemon/Amp runners — validates our day-one shape); model pinning (Claude Code 2.1.296 + our Stickiness); status-precedence algorithm (Warp PR#11367); collapsed-transcript progressive disclosure (Amp/Antigravity/Claude/Devin/goose); changed-files timeline (OpenCode #55064/Devin); task categories w/ ask-first chains (OmO + our no-silent-substitution §2 receipts); ACP surface (Zed/Cursor/goose/GemIni acceleration + §23.8); plan-as-artifact (Claude/Zed roadmap/Antigravity/Cursor Rollouts); OSC program-status (OpenCode #55172/Warp/Antigravity + 7501 emerging); cross-session transcript search (Zed #58686 + our maintained event_fts index); test-runner evidence into the completion gate (Zed #5242 888-upvote receipt + Qwen Test Failure Explorer); curated cheap-pool agents (OmO explore/librarian + our utility-roles-must-be-cheap validation); snapshot rewind (wiring of the library-tested harness-snapshots crate — internal-debt class like tasks 2-8; §23.8 host-side-checkpoints prior art); session cost governance (anti-receipt: the repo's OWN §23 AVOID "cost HUD with zero budget/cap enforcement" — task 33 closes it).
  Parallelization: Wave L | Blocked by: 29, 30 | Blocks: none
  References: README receipts table structure (lines 140-168); draft C8/C9 receipts; research §34/§35 from task 29.
  Acceptance criteria (agent-executable): every new row cites research §N (post-renumber) or dated external source; table renders in docs build (docs.yml green).
  QA scenarios: happy — linkcheck + docs build pass with new rows; failure — a row citing a nonexistent § fails the grep cross-check from task 30, evidence .omo/evidence/task-32-ecosystem-alignment.txt
  Commit: Y | docs(readme): convergence receipt rows

- [x] 33. Session cost governance: wire the money ledger (Wave D-assigned, listed last to preserve ascending order — see wave table row D: "13-15, 33" and its Parallelization line)
  What to do / Must NOT do: Wire the EXISTING library-only harness-core money budget (budget/money.rs MicroUsd/Spend, ledger.rs SpendLedger, signal.rs WARN_THRESHOLD_PCT=80 / STOP_THRESHOLD_PCT=100 — zero daemon wiring today, grep-verified) into the daemon engine loop, keyed off task-1 Usage frames' cost_usd: session-level default cap + config override (DaemonConfig field, user-global only); at 80% a Warning event + HUD badge; at 100% a Stop that halts further turns (delegation children and scheduled automations aggregate into the parent/owning session's ledger — closing the repo's own §23 AVOID receipt "cost HUD with zero budget/cap enforcement", which the plan would otherwise recreate: display via tasks 19/20, per-launch token caps only, ABSOLUTE_MAX_TOKENS=20M/launch is not a cost guard). Must NOT block the ask-first contract: a Stop is a typed terminal event, never a silent model substitution.
  Parallelization: Wave D (parallel with 13-15) | Blocked by: 1 | Blocks: 19, 20 (HUD badge consumes the ledger state)
  References (executor has NO interview context - be exhaustive): harness-core/src/budget/money.rs:15,75 (MicroUsd, Spend); harness-core/src/budget/ledger.rs:10 (SpendLedger); harness-core/src/budget/signal.rs:15-17 (80/100 thresholds); harness-orchestration/src/budget.rs:12 (ABSOLUTE_MAX_TOKENS — the non-guard this replaces at session level); task-1 Usage frame (cost_usd source); harness-daemon/src/engine.rs (loop wiring point); research §23 AVOID row (the anti-receipt).
  Acceptance criteria (agent-executable): `cargo nextest run -p harness-core -p harness-daemon` green with NEW tests: a fixture session crossing 80% spend emits Warning + HUD badge state; crossing 100% halts further turns with a typed Stop event (no further provider calls); delegation children's spend aggregates into the parent session ledger; an automation session respects its own cap.
  QA scenarios: happy — `cargo nextest run -p harness-daemon cost_ledger_warning_stop`; failure — a session at cap with a pending fallback ask resolves the ask (same-model retry path) BEFORE the Stop lands — no silent substitution, evidence .omo/evidence/task-33-ecosystem-alignment.txt
  Commit: Y | feat(daemon): session cost ledger wiring

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.
- [x] F1. Plan compliance audit
  Criteria: re-run every todo's acceptance command from the evidence files (not checkbox inspection) — all green; verify each evidence file exists with happy+failure output; no todo left partially satisfied.
- [x] F2. Code quality review
  Criteria: `cargo clippy --workspace --all-targets -- -D warnings` green (incl. wasm-plugins feature set per task 25); `npm test` green in harness-vscode; no new `#[allow(dead_code)]` introduced by the plan's todos.
- [x] F3. Real manual QA
  Criteria (agent-executed hands-on): drive the 4-frontend × 7-mechanism observable matrix — for each of TUI/web/vscode/cli × (role-routed turns + visible fallback asks, delegation children + status precedence, plan mode approve/reject [CLI cell: visible-only, asks resolve via same-model-retry by design], run_tests evidence in verdicts, cross-session search, cost Warning/Stop badge, hunk review) — with fixture transcripts recorded as evidence; zero silent substitutions across the entire matrix.
- [x] F4. Scope fidelity
  Criteria: grep the diff against Scope Must-NOT-Have (no marketplace/voice/notebooks/web-IDE/Code-Mode/git-history/BYO-sub; no silent substitution paths; no new public config formats — routing.json/test-command/automations extend existing shapes only); WATCH ledger complete per task 29's 13-entry enumeration.

## Commit strategy
One conventional commit per todo (messages specified in each todo's Commit line), on branch `ecosystem-alignment`. Waves F-P merge in order (each wave lands green: nextest + vscode npm + docs build + perf-gate). Waves Q/L may interleave. No commit lands with failing tests or an unexplained budget breach. Final PR description links every todo's evidence file under .omo/evidence/task-N-ecosystem-alignment.txt.

## Success criteria
- All 33 implementation todos complete: every acceptance criterion's command green; every evidence file exists with happy+failure QA output.
- The daemon can, in production paths: execute all six CORE_TOOLS (no NoExecutor), run role-routed turns with visible ask-first fallback, spawn category-delegated children with team surfacing AND enforced depth/grant/cost governance, gate plans, run tests as gate evidence, answer ACP stdio clients, and emit program-status — all observable in TUI/web/vscode/cli without a single silent substitution; session spend is governed by the wired money ledger (Warning at 80%, Stop at 100%).
- CI: wasm/vscode/web-e2e/doctests covered; evals + deep-qa scheduled workflows exist and green on dispatch; single tokio-tungstenite major; perf-gate green at final HEAD.
- Docs: README honest (production-wired vs library-tested, correct counts/HEAD/media+tools), research uniquely numbered with §34-35 + WATCH entries, receipts rows added.
- Final verification wave F1-F4 all APPROVE with receipts.
