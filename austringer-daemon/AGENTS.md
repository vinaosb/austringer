# austringer-daemon

## OVERVIEW
The engine crate: sessions, protocol, routing, delegation, mediation, cost governance.

## STRUCTURE
```
server.rs        DaemonState + HTTP route table
attach.rs        SessionHost: per-session command surface + mediator
engine.rs        agent-loop wiring; over LOC ceiling, surgical edits only
protocol.rs      v2 wire frames
compat.rs        downgrade_for_v1: v2 frames downgrade for v1 clients
events.rs        SessionEvent; v1-withheld variants; wire ints u64 never u128 (tagged-Content rejects u128)
routing.rs       SessionRouter; mediate_fallback on every hop; MAX_UNACKNOWLEDGED_ASKS=1; unacked ask = same-model retry + verbatim degraded notice
task/            mod, budget (DelegationBudget per-parent), child, tool, readonly (ReadOnlyApproval)
team.rs          team status; Warp precedence; never flips a terminal status while a child runs (structural)
plan.rs          PlanGate, PlanAwareApproval, PlanGatedExecutor; timeout never approves
grammar.rs       SLASH_COMMANDS, single source of truth
grammar.json     byte-pinned artifact; eol=lf via root .gitattributes
cost_ledger.rs   wire_micros u64 saturating; Warning 80%, typed Stop 100
search.rs        GET /sessions/search + session-scoped search_sessions tool
acp.rs           --acp stdio bridge; ACP_CLIENT_ID rides the same mediation queues
osc.rs           OSC 777 + 7501 emission
notify.rs        ProgramStatus mapping; sanitizes inherited sequences; exhaustive match, no wildcard arm
gate.rs          completion gate: CONFIRMED / REFUTED / INCONCLUSIVE
bridge.rs        TestRunEvidence bound into gate verdicts
tools/run_tests/ framework detection + typed TestReport; tier parity with execute_command
launch/          spec-driven launch + orchestration bridge (children, sink)
```

## WHERE TO LOOK
| Task | Touch |
|------|-------|
| New SessionEvent variant | events.rs + austringer-cli/src/render.rs + austringer-tui/src/transcript.rs + compat.rs; workspace clippy catches missed exhaustive matches |
| New slash command | grammar.rs, regen grammar.json, update JS/TS mirrors, drift tests |
| New route | server.rs route table |
| Model fallback change | routing.rs only; never bypass mediate_fallback |

## ANTI-PATTERNS
- No silent model substitution anywhere; every fallback hop asks.
- No new config formats; routing.json and austringer_TEST_COMMAND extend existing shapes.
- Cost Stop halts spend; it never switches models.
- Permission asks never auto-answer; timeout and deadline never approve.
- A declared write-set is never an approval.
- bin reads routing.json once: absent = silent defaults, invalid = loud typed error.

## NOTE
tests/ files are topic-prefixed. v1_replay_tests corpus gates v1 byte-identity.
fixtures/ holds golden transcripts, incl. the task16 golden session transcript
(plan mode-off byte-compare).
