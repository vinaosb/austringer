# Issues — ecosystem-alignment

Problems and gotchas encountered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-10-21T02:10Z Task: todo-1

- Dev-machine toolchain: VS 18 Insiders' MSVC 14.50 and VS 2022's 14.44 under Program Files both ship bin+lib but NO include dir, and 18 Insiders' vcvars64.bat calls a missing vcvarsall.bat — any cc-rs build (ring, libsqlite3-sys) fails with C1083 'vcruntime.h'/'stdarg.h'. Workaround in place (C:\BuildTools wrapper, see learnings); a durable fix is reinstalling the C++ workload headers. Not a repo defect.
- `handlers.rs` post_command still hardcodes "http" as the answering identity for PermissionAnswer/PermissionCancel (pre-existing wart, untouched). FallbackAnswer on that route is a typed 400 instead (no per-connection identity to key policy on). When a later wave introduces real per-connection ids for HTTP, both should move onto them.
- Production Usage emission is dormant by design: `UsageInference` exists and is proven by the fake-provider test, but the shipped binary does not compose it because the `Inference` seam carries no token counts (InferenceAdapter drops `CompletedTurn.usage`). The cost-ledger todo (33) must surface meters through the seam (or a side-channel) and then wrap the session factory. Emitting zero-token frames meanwhile would be dishonest accounting.
- v1 clients see fallback asks only as `Notification { kind: "fallback" }` (the ask frame itself is withheld); they cannot ANSWER one over v1. That matches the plan's "retries the SAME model and surfaces a v1-decodable Notification" contract, but means a v1-only client on a fallback-heavy session will accumulate same-model retries until a v2 client answers or the deadline expires. Acceptable per plan; revisit if external v1 clients matter operationally.

## 2026-10-21T04:20Z Task: todo-1 (resume)

- RESOLVED (was raised in round 1): production Usage emission is no longer dormant — AssistantTurn carries `Option<TurnUsage>`, the adapter threads `CompletedTurn.usage`, and every identity-bearing session stack (the shipped provider composition) is wrapped in UsageInference at session creation. The round-1 items.md entry describing the dormancy is superseded.
- `harness-exec::pty_live::cargo_version_style_command_renders_cleanly` is red ON THIS MACHINE ONLY (ConPTY child spawn cannot find `cargo`; it is not on the persistent PATH — only on the cargo-env wrapper's). Verified pre-existing on clean HEAD. Not a repo defect; would pass on CI. No repo change made for it.
- `IntoSessionFactory` keeps legacy scripted factories working, but any code that wants Usage frames in tests must go through the identity-bearing factory (`resolved.compose_factory(transport)` or a hand-built `ComposedInference { identity: Some(..), .. }`) — the identity deliberately cannot be attached after the fact (drift-proofing). Future test kits that want usage emission should follow `production_factory_emits_real_provider_usage`.
- Effort still rides as `None` on Usage frames (session composition has no effort input until routing/T42 lands in sessions); the field is protocol-ready and the decorator takes it as a constructor param for that wave.

## 2026-10-21T06:30Z Task: todo-2

- FIXED IN-TASK (pre-existing, found by the new SSE test): `handlers.rs::sse_events` built a throwaway inbound channel whose sender dropped instantly, so `pump_client` read closed-inbound as ClientGone and cut EVERY `/s/{sid}/events` connection right after replay + hello_ack — live SSE streaming never worked. Never noticed before because the only prior SSE consumer (`examples/qa_transcript.rs`) reads three lines and disconnects. Fix: the pump task holds the sender for the connection's lifetime. `transport.rs` untouched.
- `engine.rs` is now 295 pure LOC (over the 250 ceiling): pre-existing ~230-pure module plus the task-mandated `EventStreamSink` and its unit test. Flagged instead of refactoring the engine driver mid-wave; if a later todo touches engine.rs, extracting `SharedSteerer` or the sink into its own module is the natural split (the handlers.rs "extracted from server for the size budget" precedent).
- `handlers.rs::typed_error` is a deliberate local mirror of `media.rs`'s private helper (same shape as server.rs keeping its own `unauthorized()`): per-module response helpers are the existing convention; promoting a shared `http_error` module would touch media.rs for zero behavioral gain.
- `/automations` re-reads `automations.json` + `pack-tiers.json` from disk per request (validated through `AutomationStore::load`, exactly the binary's startup path). Deliberate: the route reflects on-disk truth without inventing reload semantics, and DaemonState carries no new scheduler state. If a later todo wants startup-frozen defs instead, plumb `Arc<AutomationStore>` into DaemonState then — the handler is the only reader.



## 2026-10-21T02:32Z Task: todo-30

- For todo 31 (docs honesty pass): README:176 and docs/research-companion.adoc:11 still claim a "33-section study"; the research file now has 40 uniquely numbered sections (35 after todo 29, 40 after todo-30's dedup renumber). Companion also lacks map rows for sec34 (OpenCode 2.0) / sec35 (OmO), and its "not listed" line (updated by todo 30 to include sec36-sec40) deliberately does not characterize sec34/sec35 because they are findings, not process records.
- For todo 31/32: one internal research-file drift remains by design (zero-content rule): sec5 iteration log L221 cites "sec10 (mode-scoped tooling origin...)" whose description matches the moved sec40 title; sec10 now resolves to the kept death-autopsy synthesis. High content overlap, but "BETTER/WORSE vs Cline" detail lives at sec40.4. Also sec6's kept title still says "full sec6 source record above" (stale phrase for what is now sec8).

## 2026-10-21T16:05Z Task: todo-3

- FIXED IN-TASK (pre-existing, blocks a mandated measurement): `harness-bench/src/bin/heap-profile.rs` did not compile at clean HEAD (`E0382 use of moved value: budgets` at line 103 � budgets moves into ScenarioContext at line 64, then read again in the evaluate call). The CI `perf-gate` job's heap-profile step (`--features dhat`) would fail to compile identically; it evidently never ran green since that shape landed. Fix mirrors perf-gate.rs one screen above: hoist `let regression_tolerance_pct = budgets.regression_tolerance_pct;` before the move. One-line, behavior-identical, must ride in todo-3's commit body.
- REPORTED, NOT FIXED (pre-existing harness-lsp behavior, exposed by binding lsp_query in-process): dispatching lsp_query against a language whose server IS present on PATH works and is fast (verified: real pyright hover answer in 1.006s), but a `#[tokio::test]` whose turn spawns a real language server never exits � the test body completes, then the current-thread runtime teardown hangs >19min with the spawned server child (kill_on_drop + tokio::process orphan handling on Windows). harness-lsp's own suites avoid this by attaching in-process transports (`set_server_factory`), never spawning real servers. Production shape is unaffected (multi-thread runtime, long-lived process), but any FUTURE daemon test that lsp_query's a ROUTABLE extension on a machine with rust-analyzer/pyright/etc. on PATH will hang the suite. The daemon e2e deliberately uses an unroutable `.zzz` extension (typed NoServer through the bound executor) � do not "fix" it to .py/.rs without solving the teardown hang first.

## 2026-10-21T18:40Z Task: todo-5
- None blocking. Known follow-ups owned by later todos: (a) snapshot store is session-scoped pending the workspace binding (todos 21/23); (b) post-rewind, attached clients that lagged behind the ring head still hold pre-rewind frames in replay — full resync semantics on rewind are TUI/stream polish (todos 19/23); (c) `sessions.last_seq` is the persisted-log contract only — the in-memory hub keeps its own monotonic frame ids across the rewind boundary by design.

## 2026-10-21T20:50Z Task: todo-4
- None blocking; suites green (392/392 on `-p harness-safety -p harness-daemon`; the only red anywhere is the known machine-local `pty_live::cargo_version_style_command_renders_cleanly`, re-confirmed identical signature, see todo-1's entry).
- Deliberate v1 conservativeness of the egress guard (documented in egress/guard.rs, not a defect): any non-flag token after an egress program word that does not reduce to a (host, port) pair denies — e.g. `curl -u user:pass url` denies on the unresolvable `user:pass` token; IPv6 literal destinations are unresolvable and deny. Unobservable under today's deny-all default; when a real allowlist grant surface arrives (perm::compile::compile_egress is the seam), widen the destination grammar tests-first or accept the conservative denials as the posture.
- The PTY-lane IsolationReport names fs_write/fs_read as NAMED GAPS by design (the lane enforces userland guard + seam scrub, not kernel fs confinement). If a later todo wants kernel enforcement on this lane, the honest route is a second spawn lane through EnforcedSandbox (losing the screen-delta contract) — not borrowing the backend's report. Recorded for todo 17 (run_tests) which should reuse tools::safety::gate + ExecSpec.env_scrub verbatim.
- Doctor observation for todo 31 (no change made, per MUST NOT): doctor's sandbox probe says `backend: windows-appcontainer` passively; every execute_command result now corroborates with `backend=windows-appcontainer` in its [safety] line — the two surfaces agree by construction (both call platform_backend()).

## 2026-10-21T21:30Z Task: todo-6

- attach.rs is at 295 pure LOC (pre-existing session-hosting module + ~70 for the skills gate). Warning band; a further split (intake gates vs hosting) is a mechanical follow-up, deliberately not improvised inside todo-6.
- Trigger-driven ACTIVATION (approved + consented skill auto-injecting context) is implemented and unit-shaped (SubmitRouting::Activate) but has NO e2e yet: nothing grants consent in this todo, so no fixture can reach the Activate arm over the wire. First consent-granting surface (skill-consent.json writer) should add that e2e.
- provider_binary_tests::unconfigured_daemon_fails_typed_over_the_wire flaked ONCE under 3-parallel-binary nextest contention (root-serve race in spawn_binary); green in isolation and in the final full run. If it recurs, the spawn_binary readiness wait is the place to look - not the daemon.
- pack-tiers.json names the SKILL (pack name == skill name for pack-installed skills today). If packs ever ship multiple skills, the tier key needs a pack-qualified form - decision deferred to todo 23's grammar unify.

## 2026-10-21T21:05Z Task: todo-8
- No unresolved problems. Two test-side corrections during the red->green walk (POST route /s/{sid}/cmd; reqwest .json() unavailable - both learnings appended). The launch/ module was split once after the first draft measured 344 pure LOC (ceiling 250); final files 159/85/134.

## 2026-10-21T23:50Z Task: todo-33
- PRE-EXISTING harness-core loop quirk (NOT fixed, out of todo-33 scope): Operation::ALL runs Retry -> StopHook -> ExitOnError, and StopHook's consult guard (pipeline.rs stop_hook) does not check last_error. On any multi-turn session, turn_ended stays stale-true from the previous clean turn, so a FIRST-inference failure is swallowed: StopHook returns TurnComplete before ExitOnError can FailSession, and the run reports turn_complete with zero failure surface. Affects every inference error on turns >= 2 (single-turn sessions fail typed correctly, which is why ExitOnError's own tests never caught it). Todo-33 dodged it deliberately: the cost halt is proven at the provider seam (preflight refuses before inner.infer -> zero provider calls, no assistant events) plus the typed intake refusal and CostStop event. A core fix is one guard (require last_error.is_none() in the StopHook consult) but belongs to a core-side todo with its own tests.
- attach.rs grew again (cost gate + accessor; ~340 pure LOC now) - still the file todo-6 already flagged in the warning band. The mechanical split (intake gates vs hosting) is more attractive each todo; not improvised inside todo-33.


## 2026-10-21T21:32Z Task: todo-15

- task/mod.rs CROSSED the 250 pure-LOC ceiling in this window (HEAD 233 -> 293): todo-15's budget publications (+~32) plus todo-14's concurrent curated-agent additions in the same file. Split deliberately NOT done here — the file carries the sibling's UNCOMMITTED work (stay-in-your-lane; the todo-9/11/13 precedent), and todo-14's own learnings entry says they rebased on top of todo-15's edits mid-flight. Natural split for the orchestrator: run_child + spawn_and_run + publish_budget + ChildSpawn -> task/runner.rs (keeps mod.rs as the services/types facade, both halves ~150 pure LOC).
- transcript.rs (TUI) now 548 pure LOC — pre-existing 422 at HEAD (already 1.7x the ceiling before this task); received surgical additions (~35 prod + ~90 tests: ChildCell.detail, TeamBudgetView, team_status accessor, three view tests). The view/reducer split is a Wave-U-shaped refactor (todo 19 touches this file again) — flagged, not smuggled.
- harness-cli/src/render.rs now 253 pure LOC (HEAD 224; crossed by the two MANDATED sibling arms + their test). Todo 22 extends the same human_event match (Condensation + fallback asks + plan states) and owns the natural split (human lines vs NDJSON envelope vs ask renderers).
- Launch-path (/launch) children do not surface blocked asks (only the task-tool path does — the delegation surface todo 15 names, per the t13 handoff note). SessionChildExecutor::run still awaits TurnFinished without watching its child's control bus; wiring the same translation there is a ~15-line follow-up if a launch-surface todo wants it.
- Web store.js still carries the dead worker_claimed/worker_healed reducer cases (never emitted since t8) — kept because they are v1-decodable shapes the store historically tolerates; removing them belongs to a web-store cleanup with the todo-20 dashboard work.


## 2026-10-21T12:15Z Task: todo-10

- PRE-EXISTING BREAKAGE (not fixed, out of todo-10 file scope): harness-cli does not compile at HEAD. Commit 2947ed4 (todo-33 session-cost wiring) added SessionEvent::CostWarning/CostStop to harness-daemon/src/events.rs (+21 lines) WITHOUT the exhaustive-match sibling arms in harness-cli/src/render.rs:88 (E0004); that commit verified only -p harness-core -p harness-daemon, so the cli breakage shipped unnoticed. Any task needing the harness binary (doctor, run, repl) is blocked until render.rs (and possibly harness-tui/src/app.rs, todo-1's other exhaustive-match site) gains the two arms. Verified read-only: render.rs clean in git status, variants present at HEAD. Todo-10's doctor check used the plan's fallback clause (catalog-seam test) - see .omo/evidence/task-10-ecosystem-alignment.txt section 5.

## 2026-10-21T09:18Z Task: todo-9

- Pre-existing over-ceiling files received surgical additions: harness-daemon/src/server.rs (285 pure LOC, +~15 for the routing field/branch) and src/bin/harness-daemon.rs (288, +~45 for routing_intake + single routing.json read). Both were over the 250 ceiling BEFORE this task; splitting them (DaemonState factory extraction / bin run() decomposition) is an orchestration-level refactor outside todo-9's surface - flagged for the orchestrator rather than smuggled into this task's diff.
- The daemon Inference seam cannot express refusal/length stop reasons (AssistantTurn drops CompletedTurn.finish_reason at the adapter boundary - todo-1's learning 32 déjà vu, this time genuinely absent from the SEAM not the producer). Routing's refusal-retry semantics stay intel-side until a seam widening; noted here so a future todo doesn't rediscover it.

## 2026-10-21T18:32Z Task: todo-11

- RANKING DEVIATION (for orchestrator sign-off): the plan's QA prose says "recency-weighted bm25", but the binding References instruction is "reuse meta.rs's MATCH/bm25/snippet/rank idiom" (pure bm25, ORDER BY rank). event_fts has no timestamp column to weight by; a recency blend would need a schema change on a SELECT-only task. Shipped bm25-ascending ranking; evidence + receipt note state this explicitly.
- Pre-existing over-ceiling file received a surgical 1-line addition: harness-daemon/src/server.rs now 283 pure LOC (flagged since todo-9). All other touched files under the 250 ceiling (meta.rs 161, store.rs 214, search.rs 187 new, providers.rs 212, cli sessions.rs 101 new). The search surface itself went into a NEW module (search.rs) rather than growing handlers.rs (288 total lines) - kept both under their existing debt.
- meta.rs error-classifier hardening was IN SCOPE: without classifying tokenizer-level FTS5 errors ("unterminated string") as BadQuery, the route would have returned 500 for the plan-mandated typed-400 failure path. search_events' mapping changed in the same direction (Sqlite{..} -> BadQuery for SQLITE_ERROR on MATCH statements); no caller regressed (full 3-crate suite green).
- CLI `sessions` family is HTTP-only for now: the IPC carrier has no HTTP base and the daemon does not persist its port. The missing --url case is a typed transport hint raised BEFORE Target::resolve (else it masquerades as a token-file failure). If a later todo wants IPC-mode search, the daemon needs to persist its bound port (or serve search over the IPC frame protocol).
- Engine-to-store persistence gap (carried from todo-5, re-confirmed): today only embedders/tests + rewind write the store; the search route/tool read whatever the store holds. When the engine writer lands, the search surface needs zero changes.
- Machine events (not repo defects): one leaked harness-daemon.exe (PID 110572, from 09:28) locked C:\ct\debug and failed the first RED build with "failed to remove file ... Access is denied (os error 5)" - killed it; any worker hitting E05/os error 5 on harness-daemon.exe should check for a stale daemon process first.


## 2026-10-21T16:19Z Task: todo-12

- FIXED IN-TASK (pre-existing wire bug, exposed by the new e2e): harness-mcp/src/acp/types.rs ToolCallContent::Diff emitted `new_text` on the wire where ACP v1 specifies `newText` (the module doc's camelCase promise was false for exactly this one field). Added the serde rename; no prior consumer asserted the field name, so nothing else moved.
- REPORTED, NOT FIXED (pre-existing daemon semantics, now documented in harness-daemon/src/acp.rs module docs): a wire-sent Cancel (any transport) is deferred by the engine driver to the turn boundary and the sticky token then cancels the NEXT turn's run() at entry. Mid-run cancellation from WS/SSE/IPC/ACP is therefore impossible today; a core-side fix (processing commands concurrently with run_turn or resetting the token per turn) belongs to a daemon-driver todo with its own tests. ACP inherits the semantics unchanged (never more capable than local clients).
- For the wave budget gate: harness-mcp (rmcp/schemars/toml/url) entered the daemon's REAL dependency graph via todo-12 - perf-gate/perf numbers should be re-measured at the next wave checkpoint (expected harmless: 11.03 MB of the 40 MB binary budget at todo-3, but unmeasured here).
- --acp v1 ignores session/new's cwd param (documented): relative tool paths resolve against the daemon process cwd (IDE hosts spawn the agent with the workspace as cwd, which is the practical contract); a true per-session cwd needs an executor-factory widening - future todo if an IDE host requires it.

## 2026-10-21T17:25Z Task: todo-13

- No unresolved problems; 412/412 green on the three-crate gate, workspace clippy -D warnings clean.
- Pre-existing over-ceiling file touched surgically: bin/harness-daemon.rs now 304 pure LOC (+16 for the TaskServices binding; flagged since todo 9 at 288). The run()/build_state() split remains an orchestrator-level refactor - not smuggled into this diff (todo-9/todo-11 precedent).
- For todo 15: the budget surface to consume is harness_daemon::task::{DelegationSnapshot, TaskServices::budget_of(parent_id)} plus the ChildStarted/ChildFinished events task spawns already publish (role field = ""worker"", task field = the category). TaskChildOutcome carries the child Arc<SessionHost> for blocked-child ask surfacing.
- For todo 14: curated explore/librarian agents should register through RoleRegistry + the cheap-pool validation, then reach the spawn path via TaskServices::run_child or the task tool - the category vocabulary (routing.json categories[]) and the role registry (explorer/worker/deep) are currently two disjoint key spaces; unifying them is todo 14's call.
- Untracked .harness-daemon/.harness/skills-lock.json in the worktree predates this task's first build (16:23 vs ~17:05) - sibling-task test leftover, untouched.


## 2026-10-21T19:50Z Task: todo-14

- DESIGN DECISION for orchestrator sign-off (plan did not specify): curated agent names (explore/librarian) are RESERVED task-tool vocabulary - the curated ROLE dispatch takes precedence over a user routing.json category of the same name, and an unrouted curated name refuses typed (UnconfiguredRole) instead of falling through to the category table. Rationale: the todo's cheap-pool + read-only guarantees are the contract; a category must not smuggle the agent onto a frontier model. Documented in the task tool description + task/mod.rs module docs; pinned by curated_names_are_reserved_over_user_categories and curated_agent_without_role_route_is_typed_refusal.
- Pre-existing over-ceiling files touched surgically (split remains orchestrator-level; todo-9/11/13 precedent): harness-daemon/src/task/mod.rs now 293 pure LOC (+~55 for the curated dispatch; already over from todo-13/15 growth), src/engine.rs 309 (+~26 for readonly_stages; the only home with SessionHost internals visibility). New module task/readonly.rs 60.
- Transient sibling breakage (resolved by waiting, no cross-edits): daemon lib failed to compile twice mid-task from todo-15's in-flight files (SessionEvent::TeamBudget variants without compat.rs arms; then E0658 anonymous-lifetime in their new team.rs). Any worker hitting daemon-lib compile errors ~19:00-19:40Z should retry, not diagnose.
- For todo 15 (consumable): ChildStarted.role now carries "explore"/"librarian" for curated dispatches (worker for user categories) - the status aggregator should treat them as read-only children; the readonly_refusal Notification kind is on the child stream if team surfacing wants to surface guard denials.

## 2026-10-21T19:50Z Task: todo-17

- DESIGN DECISION for orchestrator sign-off (plan said "execute via executor #3 path"; todo-4 issues said "reuse tools::safety::gate + ExecSpec.env_scrub verbatim"): the GATE and the SCRUB POLICY are reused verbatim, but the spawn lane is the media M15 headless capture (std::process + kill-tree), not the PTY runner - the PTY screen grid is bounded by construction and silently truncates per-test lines on real suites, so it cannot feed a parser. The kill-tree guard is the media-lane thin copy (win32job/libc, both already in the workspace graph; win32job added to harness-daemon's cfg(windows) deps - no new external crate). Documented in .omo/evidence/task-17 and run.rs module docs.
- For todo 18 (gate binding): no SessionEvent variant was added (per task instruction), so "test-run event ids" must reference recorded ToolCall/ToolResult events. TestReport is Deserialize at harness_daemon::tools::run_tests; the tool observation's FIRST line is the report JSON (line 2 is the [safety] suffix) - parse by line, not by prefix.
- For todo 18: nextest-ignored tests appear only in the run summary counts ("N skipped"), not as individual entries - "suite green" should key on summary.failed == 0 (entries + counts both live in the report), not on entry-count arithmetic.
- Known machine-local reds re-confirmed during the final gate, both pre-existing and documented: pty_live::cargo_version_style_command_renders_cleanly (todo-1 entry; fast PATH-failure signature) and provider_binary_tests::unconfigured_daemon_fails_typed_over_the_wire (todo-6 contention flake; green in isolation immediately after).
- Concurrent todo-16 worker broke the daemon lib twice mid-task (Plan* ServerMsg/ClientMsg variants without transport/handlers/mediation arms; also a transient 9-error state while they edited compat.rs). Protocol per todo-14: wait ~2-4min, re-run, never diagnose sibling files. My module type-checked throughout (its one E0277 surfaced in the FIRST check and was fixed immediately).
- Pre-existing over-ceiling file touched surgically (todo-9/11/13 precedent): bin/harness-daemon.rs now 312 pure LOC (+3 for the hoisted test_command capture; flagged since todo-9 at 288). harness-daemon/src/tools/mod.rs at 167 and run_tests/mod.rs at 235 (warning band, cohesive provider+orchestration) - split point if the next todo grows them: parse_results + the RunTestsError vocabulary -> run_tests/errors.rs.

## 2026-10-21T19:55Z Task: todo-16

- mediation.rs is now 496 pure-prod LOC (pre-existing ~330 + the plan ask lane ~165): over the 250 ceiling, carried pre-existing (the whole file was already the multi-ask-type mediator). Natural split for the orchestrator: mediation/{policies,permission,plan}.rs keeping the PermissionMediator facade in mod.rs - NOT improvised here (concurrent todo-17 worker building in the same crate; todo-14 stay-in-your-lane precedent). All other touched files within ceiling (plan.rs NEW at 249 pure-prod; protocol.rs 230; render.rs 196; events/compat enum-declaration files).
- Pre-existing over-ceiling files received surgical additions only (todo-9/11/13 precedent): attach.rs 348 (+14: plan field/accessor), engine.rs 268 (+10: stage wrap + Arc receiver), transcript.rs 312 and app.rs 296 (+plan arms) - the transcript.rs/app.rs splits already belong to todo 19, attach.rs to the standing mechanical-split flag from todo-6/33.
- DESIGN DECISIONS recorded in .omo/evidence/task-16-ecosystem-alignment.txt for sign-off: (1) phases.rs intentionally untouched (building block referenced, forcing BoostPhase into the daemon adds a cross-crate dep for zero behavior); (2) session-start mode flag = PlanMode command, Hello shape frozen; (3) reject KEEPS plan mode active (only approval flips autonomy); (4) artifact seeds empty, materialized via Edit decisions (no transcript-to-artifact auto-capture invented).
- A plan ask left open when plan mode exits (user /mode off) resolves by timeout at worst; a late answer to it broadcasts PlanResolved but applies nothing (plan-id checked). If a later todo wants explicit cancellation on exit, the mediator needs an ask-id return from mediate_plan - noted as the seam.

## 2026-10-21T23:33Z Task: todo-18

- DESIGN DECISIONS for orchestrator sign-off (plan named the binding shape only): (1) suite identity = TestReport.framework (cargo-nextest / cargo-libtest / override) - the only suite-ish field the task-17 report carries; criteria bind "tests: <framework> green" or "tests: suite <name> green". A richer suite notion (workspace path, -p filter, multiple suites per run) needs a TestReport field addition and belongs to a report-shape todo. (2) The Verdict frame's evidence stays ALL observed frames (task-41 semantics untouched - "do NOT weaken the task-41 gate semantics"); test-run event ids are included by construction, and the bundle's EvidenceLinks carry them explicitly as "test-run:{id} suite:{s}" labels. (3) REFUTED's failing-ids citation is observable in GateVerdict::Refuted.reason + the TurnComplete notification message (now "gate: refuted: <reason>"); the Verdict wire frame intentionally carries only the kind - no protocol shape change, per the task's SessionEvent constraint and the no-sibling-churn budget. (4) A green run's EvidenceLink claims its suite criterion for bundle readability, but the gate ignores claims for tests: criteria - structured runs are the single coverage path, so prose and structured evidence can never double-cover or drift apart.
- bridge.rs is now 260 pure-prod LOC (pre-existing ~225 + ~35 task-mandated binding/collection): over the 250 ceiling. Split deliberately NOT done here (todo-9/11/13/15 precedent - the file is cohesive around the run sink and a mid-task split of a verified-green module buys nothing but risk). Natural split for the orchestrator: watch_turn + watch_asks + collect_test_run -> bridge/watch.rs (or gate-evidence collection into a bridge/evidence.rs), keeping RunSink and the test module in bridge.rs; both halves land ~150 pure-prod.
- Known machine-local flake re-confirmed (todo-6/todo-17 lineage): provider_binary_tests::unconfigured_daemon_fails_typed_over_the_wire failed ONCE under full-suite nextest contention, green in isolation AND in the final --no-fail-fast run. No repo defect.
- harness-daemon/src/gate.rs at 141 pure-prod after the extension (healthy); tools/run_tests/report.rs untouched (read-only consumption honored, git-verified).

## 2026-10-21T23:55Z Task: todo-22

- SIZE (pre-existing flag extended + one new): render.rs remains over the 250 pure-LOC ceiling (312 -> 426 by the repo counting; the plan cited 253 from an earlier count) - additions kept surgical (1 Condensation arm + ndjson_line + mandated tests) per the task's explicit instruction; the standing split flag from earlier todos covers it. NEW: harness-cli/src/client.rs 246 -> 253 (+7) from the enum->struct conversion that owns negotiated_version once - alternatives (per-carrier fields, a Carrier trait) cost more lines or a one-off abstraction; carried with this justification rather than contorted. Split point if it grows: HttpSession/IpcSession -> client/{http,ipc}.rs keeping SessionTransport in client.rs (~90 pure each with their halves).
- OBSERVATION for todo 23 (grammar unification): `sessions list` output order is daemon HashMap order (no sort promised/tested); if the unified grammar spec wants stable output, the daemon route or the CLI render needs a sort - one-liner either way, deliberately not snuck in here.
- OBSERVATION: `sessions list` = LIVE sessions only (GET /sessions returns session_ids() = live hosts); store-resident-but-never-attached sessions surface only via search. That is the task-2 route's semantics; if the orchestrator wants list=store-scan, that is a daemon-route change outside this task's read-only consumption.
- Concurrency note (no action): the todo-24 worker transiently broke the daemon lib (osc.rs:109 E0308) and 4 osc_status_tests mid-window; both were green by the final full run. Any worker seeing daemon-lib breakage ~23:30-23:45Z should attribute it there, not diagnose.
- The plan's "flipping it if it proposes 1" negotiation clause: verified no-op (client.rs sends PROTOCOL_VERSION=2 on both carriers; grep found no v1 proposal anywhere in harness-cli). Recorded in evidence rather than changed in code.

## 2026-10-22T00:35Z Task: todo-21

- FOLLOW-UP (cross-crate, out of scope): the daemon's HTTP command route refuses ClientMsg::FallbackAnswer and ClientMsg::PlanAnswer typed (handlers.rs 147-160 - answers require a WS/IPC connection identity; SSE is read-only). harness-vscode is SSE+HTTP only, so its (complete, tested, fakedaemon-proven) fallback/plan answer wiring surfaces the typed failure against the REAL daemon. Resolution seam: either a WS carrier in harness-vscode or a daemon-side identity scheme for the HTTP route - belongs to a protocol/transport todo, not a frontend todo.
- SIZE (flagged, additions kept surgical): session.ts 246 -> 271 pure LOC (every todo-21-extractable unit - ask dialogs, transcript projection, webview panel - split into src/askdialogs.ts / src/transcript.ts / src/webview.ts; remainder is one routing switch + thin delegates; SIZE_OK noted in header - moving untouched omnibar/hunk glue is task-23 territory). decode.ts 200 -> 339 (mechanical one-arm-per-wire-variant extraction table mirroring the daemon's enums; SIZE_OK in header). client.ts 248 -> 279 (the +31 is the v2 protocol surface itself; SIZE_OK in header). New modules all well under budget.
- fakedaemon takeCommand remains permissive for fallback_answer/plan_answer (202 for any ask id; only permission_answer/cancel 404 unknown asks). Deliberate: it fixtures the transport contract, not the daemon's identity policy - which the caveat above records. If a future test wants daemon-faithful refusals, mirror handlers.rs 147-160 there.
- npm test hangs (not fails) if any test leaks a FakeDaemon - observable on Windows when a RED run's assertions throw before teardown. No repo defect; noted so the next worker doesn't misread a timeout as a suite-wide hang.


## 2026-10-22T12:55Z Task: todo-19

- PROTOCOL GAP for a future todo: SessionEvent::Diff carries {path, hashes, hunks} but NO call_id, so a surfaced diff cannot be attributed to the tool call (edit_lines) that produced it - per-tool diffstats on a tool row, and any per-hunk accept/reject ROUND TRIP to the engine, both need a call_id (or a ClientMsg::HunkDecision surface) added daemon-side. TUI hunk decisions are client-side view state + notes until then.
- DESIGN DECISIONS for orchestrator sign-off (plan named the features, not the shapes): (1) hunk accept/reject = y/n on the next pending hunk while the diffs accordion is open (the permission-gate chords reused; no new bindings); (2) scroll is a single body offset (lines from the tail, auto-follow reset on every Event/Snapshot) - "focused panel" collapses to the body because the body is the only scrollable region; the clamp is compose_body().len() so Up never marks dirty past the top; (3) fallback modal y=proposed/n=current via ClientMsg::FallbackAnswer (mediator-owned, never a local hop), plan modal y=Approve/n=Reject - PlanDecision::Edit is NOT TUI-reachable (an artifact editor is an omnibar shape that does not exist; the modal doc says so); (4) ctrl+5 = toggle_diffs (ctrl+4 was taken by the HUD); (5) routing_savings REMOVED from Hud/HudUpdate (task item c: remove/blank dead fields until fed) - if a savings-bearing frame ever lands, the field comes back with its feed, not before.
- File sizes after the task: app.rs 203, app_input.rs 248, app_server.rs 160, diffs.rs 69, panels.rs 246, render.rs 94, hud.rs 93 prod-pure (all under ceiling); transcript.rs 317 (+5 net, pre-existing over-ceiling 312 - DiffEntry extracted to diffs.rs but SessionView gained the hunk-review methods; natural next split: apply_event/entries vs team arms). keys.rs 249 (+3).
- For any todo surfacing diffs elsewhere (web timeline todo-20 consumes the same Diff events): the parseability rule is "@@ header line present" and diffstats skip +/-'s own file headers (+++/---) - harness-tui/src/diffs.rs is the shared parsing precedent; do not fork the heuristic per frontend.
- Known machine-local note: harness-cli already had its own attach-version test (sessions_verbs_cli::attach_connection_negotiates_protocol_version_2, todo-22 worker) - the TUI one is the daemon_e2e twin; both green together, no conflict.


## 2026-10-22T04:01Z Task: todo-24

- OBSERVATION for the orchestrator (design gap, not fixed here): TurnComplete maps to Done on the OSC status lanes even when the gate verdict is REFUTED or the outcome is failed:: - NotificationKind carries no verdict, and bridge.rs emits one TurnComplete regardless of outcome. An error-vs-done split on the terminal lanes needs a verdict-bearing field on Notification (bridge change), or a new kind; out of todo-24's mapping scope. The PTY QA run's earlier 404-provider iteration shows the current behavior honestly: failed inference still reports done (the run DID finish).
- OBSERVATION (receive-neither contract kept): the 7501 spec's feature detection (OSC 7501 ; ? + terminal reply listing states/kinds) is deliberately NOT implemented - answering a detection query would be receiving. If a future todo wants capability-gated emission, the reply parser is a receiver and must be its own decision.
- OBSERVATION (Warp-side recognition, not ours): Warp resolves the JSON gent field against ITS CLIAgent enum command prefixes; "harness" is not in that list (claude/opencode/codex/gemini/...), so our lifecycle frames parse but resolve to CLIAgent::Unknown and do not drive Warp's session model. Emit-only is the todo contract (we emit both, receive neither); a one-line Warp-side PR adding the prefix would light their side up. cmux/herdr-class listeners that key off 7501 state tokens need no registration.
- Flake intersection note (pre-existing class): dispatcher_live_acknowledged_hop_revalidates_effort_grid failed once under a full-suite parallel run, passed in isolation (4/4) minutes later - the known machine-local contention flake, not todo-24's change (no shared files with dispatcher tests; evidence [3] records both runs).
- The todo-19 worker's issues entry about "the todo-24 worker transiently broke the daemon lib (osc.rs:109 E0308)" was this task's serde_json Value-assign mistake mid-window; fixed in-window, final 345/345 + clippy exit 0 ran on the merged tree including their TUI work.

## 2026-10-22 01:09:44 Task: todo-20

- Deviation from plan's "No Rust changes expected": the acceptance criteria
  (fixture run verdict, fixture children, priced usage frames) are impossible
  without e2e fixture data, which lives in harness-web/examples/e2e_daemon —
  daemon.rs gained an identity-bearing factory + seed_fixtures(), and
  harness-web Cargo.toml gained dev-dep harness-providers (ModelIdentity).
  Still zero harness-daemon src changes.
- Usage/verdict totals are per-connection (transient broadcasts); reconnecting
  pages lose pre-attach cost history. Server-side retention would need a
  protocol change — noted, not attempted (out of todo-20 scope).
- The dashboard route no longer renders the trends panel (moved to the session
  view where v2 frames actually flow); dashboard keeps sessions/heatmap/packs.

## 2026-10-22T03:35:39Z Task: todo-23

- provider_binary_tests::unconfigured_daemon_fails_typed_over_the_wire flaked ONCE under full-suite parallel load (CARGO_BUILD_JOBS=3 + 522 tests); passed on immediate rerun and in isolation. Pre-existing binary-spawn timing sensitivity, unrelated to grammar unification (no daemon runtime changes in this task). Flagging for any future flake-budget audit.
- Pre-existing over-ceiling files touched surgically, not split (out of todo-23 scope, flagged for a future deslop pass): harness-web/assets/ui.js (685 pure LOC), harness-cli/src/render.rs (444), harness-vscode/src/session.ts (270, carries SIZE_OK note; net smaller after this task extracted the omnibar dispatch).
- vscode /skill intentionally submits WITHOUT the telemetry context suffix (unlike chat submits): the daemon trust gate parses the "/skill" prefix off the submit lane and appended context would ride the skill args. Documented in omnibar.ts.

## 2026-10-22T04:45:20Z Task: todo-26

- workflow_dispatch-green-in-Actions is unverifiable from this task: the orchestrator owns commits, the remote (vinaosb/harness) has no ref carrying .github/workflows/evals-scheduled.yml until it lands. Captured honestly in task-26 evidence: local sweep green + YAML structural validation (triggers exactly schedule+workflow_dispatch, no pull_request/push key). Re-dispatch once the orchestrator pushes.
- The committed docs/release/evals-conformance-report.json honestly records source=dirty:23 files (the pre-commit worktree including concurrent-wave edits). After the orchestrator's commit the row stays dirty-flagged for that build - by design (distinguishable from clean builds); the first CI-scheduled run will overwrite it with a clean row.
- Guard trigger-coverage on a green pack reports all three guards as unmeasured noise (fires=0) - surfaced in the artifact per the metrics contract, not hidden; if this reads as alarming in review, the fix is a guard-exercising negative case in a future pack, not suppressing the coverage block.

## 2026-10-22T07:12:00Z Task: todo-27

- workflow_dispatch-green-in-Actions is unverifiable from this task (same class as todo-26's note): the orchestrator owns commits, so deep-qa.yml reaches the default branch only with `ci: deep-qa + bench-smoke schedules`. Attempt captured in task-27 evidence (HTTP 404, workflow not on default branch; Actions IS live on vinaosb/harness — 4 active workflows). Local receipts stand in: per-filter nextest enumeration, goldens 4/4 green on the pinned rust-analyzer 2026-10-12, named-FAIL demo exit 100, bench smoke exit 0. Re-dispatch after the orchestrator pushes.
- capture_v1_ws_and_ipc_corpus (harness-daemon) is an ignored test that must NEVER be scheduled: it re-records the v1 wire-corpus fixtures and its own ignore message says a re-run after protocol change "would falsify history". deep-qa.yml excludes it by name on both legs. Flagging here per the task contract ("ignored test that cannot run on ANY runner" — this one must-not-run rather than cannot-run; documented, not fixed).
- docker_daemon_smoke_run is feature-gated out of default builds (`container` feature, default-off). Without `--features harness-safety/container` the test does not exist in the binary — a silent-skip trap for ANY future workflow that sweeps ignored tests. deep-qa.yml carries the flag plus a removal-warning comment; future sweep authors must re-check the feature inventory (see task-27 evidence §2).
- Concurrent task-26 edits left harness-evals briefly unbuildable twice during verification (manifest declaring run_sweep before its source existed; then uncompiling sources). Not a defect of either task - transient worktree state. If the orchestrator's combined commit ever fails CI on harness-evals, re-check task-26's own evidence before bisecting.
- The pinned rust-analyzer (2026-10-12 / 0.3.3081-standalone) will drift from the rustup-component rust-analyzer on dev machines; the goldens passing on BOTH is not guaranteed. The workflow's env-pin comment records the bump policy (a drift after a bump is a rust-analyzer finding, not a repo regression).

## 2026-10-22T07:49:53Z Task: todo-28

- run_tests_e2e_structured_report_flows_through_registry (harness-daemon) flaked ONCE under full-suite parallel load (429 tests, cold fixture builds): "turn never finished: Elapsed(())" after 493s while its sibling passed at 493.1s; green in isolation (3.6s) and green in the warm full re-run (13.2s total). Pre-existing load-shape sensitivity of the task-17 tool's internal turn deadline, unrelated to the tungstenite unification (no WS behavior change; all WS suites passed in the flaking run). Flagging for any future flake-budget audit alongside todo-23's note.
- The task-literal perf-gate command (no process-binary flags) structurally exits 3 (skipped-scenario config exit) even at verdict PASS on a dev machine; only the CI-shaped invocation reaches exit 0. If future task specs quote the bare command with an exit-0 acceptance, they should quote the ci.yml shape � captured in task-28 evidence with both runs.
- Concurrent task-26 worker briefly made the whole workspace cargo-dead (~11 min: harness-evals manifest declared run_sweep before its source existed). Same class as todo-27's note; no action, recorded so a CI red on the combined commit is diagnosed as transient-ordering, not bisected as a regression.

## 2026-10-22T08:23:10Z Task: todo-25

- BRANCH CANNOT BE CI-GREEN YET (first CI contact on ecosystem-alignment, probe runs 39741424221 + 39745026154): (1) cargo fmt --check fails branch-wide (harness-cli/client.rs, harness-web/lib.rs, ...; rustfmt 1.9.0 both sides) - needs a fmt sweep before the orchestrator push; (2) harness-media does not compile on linux/macos (process_group E0599 + 2 warnings) so ubuntu/macos matrix + coverage jobs are structurally red off-Windows; (3) harness-daemon grammar_json_is_byte_pinned_to_the_const_table FAILS on windows CI ("grammar.json drifted - regenerate"); (4) heap-profile dhat gate: 1 breach (perf-gate itself PASS, 0 breaches - baselines de-facto current for todo-28). None introduced by todo-25 (zero Rust files touched); all four pre-date it and only surface now because the branch was never pushed.
- todo-25's new steps verified despite (1)-(4): frontends job SUCCESS on real windows runner (npm 89/89 + e2e ALL PASS); wasm step 52/52 + count-assertion locally (CI-skipped behind the pre-existing reds); doctest failure path PROVEN on a real windows runner via trimmed probe (run 39745026154: _ci_probe_broken_doctest line 81, left:4 right:5, exit 1).
- Artifact storage quota that killed uploads 2026-09-28 has been recalculated (probe uploaded a 1.05 GB archive fine); the 1-day retention policy stays the right mitigation.
- Probe refs deleted (ls-remote shows main only), worktree pruned, working branch never pushed; probe runs deliberately left in Actions history as receipts for this task's evidence.

## 2026-10-22T09:36:08Z Task: ci-reds

- All four todo-25-documented reds FIXED, no commit (orchestrator commits): (1) grammar pin - root .gitattributes eol=lf on harness-daemon/src/grammar.json (red reproduced via CRLF working copy, mutation check still fails on content change); (2) fmt sweep 233 files, mechanical-only by provenance + token-class analysis (see task-cireds evidence); (3) harness-media linux compile - guard.rs io-import scope, executor.rs #[cfg(unix)] CommandExt import (the E0599), spawn.rs attach_guard split; cross-check green for x86_64-unknown-linux-gnu lib+all-targets via check-only stub CC (documented caveat: proves compilation, not linking); (4) dhat tui.alloc_count +12.32% - named cause (post-todo-43 UI wave: live-HUD per-frame lines + panels), NOT waste (bytes +1.7%, p99 0.077 ms vs 16 ms), re-recorded per baselines/README procedure, check PASS.
- OPEN (found, deliberately not fixed - compiles fine, out of task scope): harness-media/src/tool/spawn.rs find_program splits PATH on ';' - on unix the separator is ':' so bare-name PATH resolution never succeeds there (explicit overrides + mocktool lane unaffected). Portable fix is std::env::split_paths; worth a small follow-up before any unix runner actually exercises PATH-resolved tools.
- Final local gates all green on the final tree: fmt --check 0, clippy --workspace --all-targets -D warnings 0, nextest daemon/tui/cli/media 922/922 (+9 skipped), heap-profile --check PASS, linux cross-check exit 0. None of the four known flakes fired. Next orchestrator probe should go green on lint + ubuntu/macos/windows test + perf-gate jobs (frontends job was already green in todo-25's probe).


## 2026-10-22 07:08 Task: todo-31
- None blocking. Note: task-32 receipt row for Zed #5242 says '888-upvote' in the plan text but the recorded sources (draft C6 + task-17 evidence) say 886 upvotes - using 886 (repo-grounded).



## 2026-10-22 07:11 Task: todo-32
- Discrepancy noted: plan checkbox text says Zed #5242 '888-upvote'; repo-recorded sources (draft C6 line 20, task-17 evidence) say 886 upvotes. Row uses 886. If 888 is verifiable upstream, it is a one-character fix in README.md.

## 2026-10-23 Follow-up: wire event_fts live feed
- The daemon engine's recorded-event path must insert into `event_fts`: harness-sessions `session.rs:238` shows the store-API insert, but the engine path does not traverse it — live conversation turns never reach the index.
- Found by the F3 CLI QA cell negative drive + orchestrator reconciliation against t11's seeded-store happy path.
- The search surface itself is proven (5/5 integration tests); the gap is the feed, not the query path.
- Acceptance for the future fix: a real daemon conversation's terms return as ranked hits via the same daemon's search route.


