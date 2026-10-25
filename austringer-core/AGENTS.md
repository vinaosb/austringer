# austringer-core

## OVERVIEW
The lib engine consumed by austringer-daemon: events, agent loop, tools, autonomy, budgets, compaction, eviction, hooks, inspectors.

## STRUCTURE
```
src/
  event/           # Event + ContentBlock vocabulary (mod.rs, content.rs, derived.rs, media.rs)
  agent_loop/      # pipeline.rs + operation.rs (ORDER IS A BINDING CONTRACT), stages, inference, dispatch
  tools/           # registry.rs (per-session ToolRegistry), schema.rs (CORE_TOOLS const), hydrate.rs
  autonomy/        # matrix.rs, gate.rs, classifier.rs, ask_policy.rs, store_sqlite.rs
  budget/          # money.rs (MicroUsd/Spend u128 INTERNAL), ledger.rs, signal.rs (80/100), governor keys
  compaction/      # engine.rs, doctrine.rs, handoff.rs, segments.rs
  eviction/        # governor.rs (TaskClass keys), fold.rs, engine.rs, ladder.rs, tiers.rs
  hooks/           # fail-open, always surfaced as transcript notices
  inspectors/      # permission.rs, verdict.rs, security.rs, store.rs (GrantScope)
  context/         # history builder: append-only, never reorders
  log/, media_delivery.rs
tests/             # 69 files, subsystem-prefixed: loop_*, autonomy_*, budget_*, compaction_*,
                   #   eviction_*, hooks_*, tool_registry_*, inspectors_*
tests/fixtures/    # v01 jsonl corpus
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| New engine event | src/event/ + austringer-daemon/src/events.rs mapping + CLI render.rs AND TUI transcript.rs arms | exhaustive matches break builds |
| New tool | src/tools/registry.rs + schema.rs CORE_TOOLS + daemon CoreExecutors | registry serializes exactly the six CORE_TOOLS |
| Autonomy capability | src/autonomy/matrix.rs + gate.rs consult | matrix is the single source; do not fork the decision |
| Money budget | src/budget/ | MicroUsd/Spend are u128 internal; daemon converts to u64 at the wire |
| Eviction budget keying | src/eviction/governor.rs | keys on TaskClass, never one global |
| Pipeline stage change | src/agent_loop/operation.rs | order is binding; never reorder |

## ANTI-PATTERNS
- NEVER reorder the agent_loop pipeline (operation.rs binding comment pins it).
- NEVER key budgets on one global; TaskClass only (eviction/governor.rs).
- NEVER drop return codes in eviction fold.rs.
- NEVER silently degrade: hooks fail-open but must surface a transcript notice; inspectors never fail-open to allow.
- NEVER widen wire money types: u128 stays inside budget/; the wire is u64.
- Autonomy mode switches NEVER touch the serialized tool schema (autonomy/mod.rs; registry stays stable across modes).
