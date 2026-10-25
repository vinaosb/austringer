# austringer-tui

Ratatui thin client attaching to the daemon over a real WS connection. A view, never an engine: all logic lives in the daemon.

## STRUCTURE

- app.rs: App core, key dispatch, `App::step` entry chain
- app_server.rs: ServerMsg application; HelloAck sets `negotiated_version`, Usage feeds the HUD
- app_input.rs + commands.rs + app_submit.rs: omnibar parse/dispatch split by size budget; steering works mid-turn and via `/steer`
- transcript.rs: ChildCell detail, SessionEvent render arms
- render.rs + panels.rs + diffs.rs: frame drawing, diffs accordion, diffstat parsing
- hud.rs: cache pct, cost, per-model rows
- keys.rs, omnibar.rs, complete.rs, markdown.rs, client.rs

tests/: screens.rs (insta), daemon_e2e.rs (real client + real daemon, the pattern to copy), client_ws.rs, idle.rs, task19.rs, common/.

## WHERE TO LOOK

| Task | Location |
| --- | --- |
| New slash command / omnibar behavior | commands.rs + app_submit.rs; words from `austringer_daemon::grammar` |
| New ServerMsg variant | app_server.rs exhaustive match |
| New SessionEvent variant | transcript.rs render arm (workspace rule, enforced here) |
| HUD numbers wrong | hud.rs + app_server.rs `Usage` arm (micro-USD to dollars) |
| Hunk review UI | diffs.rs `HunkDecision`, render accordion: y/n per hunk in order |
| Snapshot churn | tests/snapshots/screens__*.snap; regenerate via insta env var only when intentional |

## ANTI-PATTERNS

- NEVER hand-roll a slash grammar table. commands.rs mirrors `austringer_daemon::grammar` and is pinned by test.
- No new SessionEvent variant without a transcript.rs arm; no new ServerMsg without an app_server.rs arm.
- Hunk accept/reject is client-side view state only; the wire carries no decision message yet (logged gap, todo-19). Do not fake acknowledgment.
- Modals reuse the permission-gate pattern (fallback asks, plan asks). No parallel modal machinery.
- Transcript detail stays collapsed by default: one line per tool plus diffstats, expanded on demand.
