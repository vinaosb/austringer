# Unwired-surfaces + org-transfer decision ledger

**Date:** 2026-10-29
**Trigger:** roadmap item 9 (adopt/drop for every "library-tested, not yet
wired" surface) + item 6 (org-transfer/merge-queue), per the same treatment
`austringer-search` received (commit `feee4bb`): wire it or delete it with a
receipt — or record a dated deferral with explicit criteria, which is what
this ledger does. Deferral-with-criteria is a decision, not drift: each entry
names its revisit trigger.

## 1. wasm plugin runtime (`austringer-wasm`) — HOLD; wire-or-delete at 0.2.0

- Verdict: keep feature-gated (off by default) until 0.2.0. At 0.2.0: wire or
  delete with a receipt.
- Receipt: zero daemon dependents; CI keeps it compiling (ci.yml wasm
  feature job); MCP just became load-bearing for extension (session tool
  mounting, `00f6c4d`) — the honest question at 0.2.0 is whether MCP covers
  the plugin use cases, making the wasm host module redundant.
- Revisit trigger: 0.2.0 planning, or the first real plugin consumer.

## 2. OAuth device-flow — DEFER

- Verdict: not wired; revisit at 0.2.x.
- Receipt: all five provider adapters are env-key configured
  (`AUSTRINGER_PROVIDER_KEY_ENV`); no cataloged provider requires OAuth
  today. A second auth surface without a demanding provider is pure surface
  area.
- Revisit trigger: a cataloged provider that requires device-flow in
  practice.

## 3. Sessions dream consolidation — DEFER

- Verdict: not wired; revisit at 0.2.x with telemetry in hand.
- Receipt: speculative long-session memory feature; no frontend surface
  requests it. Compaction/eviction only recently became production-wired;
  deciding dream consolidation before real long-session compaction telemetry
  exists would be deciding without evidence.
- Revisit trigger: first real long-session corpus + compaction telemetry.

## 4. Worker (out-of-process executor) protocol — RESERVED BY DESIGN, KEEP

- Verdict: stays reserved; no action.
- Receipt: README names it "deliberately reserved" — the in-process
  registry executor + PlanGatedExecutor serve current loads; the hunk-staging
  wave (making local edits emit `SessionEvent::Diff` and apply through the
  VFS shadow-preflight path) removes the remaining pressure to ship the
  worker for diff parity.

## 5. Org transfer (native merge queue) — DEFERRED

- Verdict: stay User-owned; transfer only on a concrete trigger.
- Receipt: merge queue requires org ownership (github.blog 2024-03
  availability note; verified 2026-10-29 — REST rulesets and GraphQL
  `createRepositoryRuleset` both reject the `merge_queue` rule on this
  User-owned repo, and the Settings UI offers no queue option). The
  pre-wiring is complete and merged regardless: `merge_group` triggers on
  ci.yml, docs.yml, deep-qa.yml (`de088f0`), so the queue activates the day
  the repo moves. Solo flow shows no queue pain (PRs #37/#38/#42 merged
  same-day each).
- Revisit trigger: collaborators arrive, or same-day PR collisions actually
  bite. On transfer day: create the ruleset
  (`{"rules":[{"type":"merge_queue","parameters":{"check_response_timeout_minutes":10,
  "grouping_strategy":"ALLGREEN","max_entries_to_build":5,"merge_method":"merge",
  "min_entries_to_merge":1,"min_entries_to_merge_wait_minutes":1}}]}` on
  `refs/heads/main`) via `gh api -X POST repos/<org>/austringer/rulesets
  --input <file>`.

## Publishing note (roadmap item 5)

All 25 crate names verified free on crates.io (2026-10-29, API sweep);
`cargo publish --dry-run -p austringer-test-support` packages and verifies
clean. Publishing executes once, at the 0.1.3 bump, after the Tier-1 wiring
waves land (dependency order: leaf libraries first, dependents after).
