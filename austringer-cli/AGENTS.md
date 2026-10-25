# austringer-cli

## OVERVIEW
Thin terminal client over the daemon's typed protocol: display and transport only.

## STRUCTURE
```
src/cli.rs         # Commands enum, arg parsing
src/render.rs      # exhaustive SessionEvent match; NDJSON contract (near LOC ceiling)
src/sessions.rs    # list/search/history verbs over HTTP routes
src/repl.rs        # ReplSession loop
src/repl_input.rs  # TurnWait open-ask tracker (/vote-cancel 0/1/N)
src/client.rs      # SessionTransport; negotiated_version, proposes v2 on both carriers
src/run.rs         # headless run/capture; --output-json branch via ndjson_line
src/attach.rs      # event tail
src/target.rs      # TargetSpec, token resolution, default data dir
src/doctor.rs      # health surface (+ doctor_probes.rs)
src/bin/           # austringer.rs entry
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| New daemon SessionEvent variant | render.rs (+ austringer-tui/src/transcript.rs) | exhaustive match; missing arm breaks workspace clippy |
| New sessions verb | cli.rs + sessions.rs | t11 conventions; HTTP-only, typed --url hint when missing |
| NDJSON consumers / eval drivers | run.rs ndjson_line | event envelopes + verbatim pass-through of ALL v2 frames |
| Condensation display | render.rs | renders "~ condensed" |
| REPL ask tracking | repl.rs + repl_input.rs | asks are display-only here |

## ANTI-PATTERNS
- NO daemon logic in this crate. Sessions, turns, routing, budgets live in the daemon. The CLI submits and renders.
- NEVER answer asks from the CLI. Unanswered FallbackAsk follows the daemon's same-model-retry policy; plan approval needs an interactive frontend.
- render.rs is near the LOC ceiling: surgical edits, move new rendering elsewhere when possible.
- Two carriers (WS `--url` and local-socket IPC), one protocol: never fork per-carrier logic.
