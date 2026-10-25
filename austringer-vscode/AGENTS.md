# austringer-vscode

## OVERVIEW
TypeScript VS Code thin client for the austringer daemon; deliberately NOT a cargo workspace member.

## STRUCTURE
- `src/` 19 modules: `extension.ts` = activation wiring only, `session.ts` = live-session VS Code glue, everything else headless-testable decisions.
- `test/` 12 `*.test.ts` + `fakedaemon.ts` (in-process fake daemon: real HTTP sockets, SSE framing, replay semantics; no live daemon, no live VS Code, zero env vars).
- `npm test` = `node --test "test/*.test.ts"`; TS runs via Node type stripping, no build step. Keep types strippable: no runtime-requiring TS features (enums-as-values, parameter properties, namespaces).
- `npm run typecheck` (tsc --noEmit) and `npm run compile` (emits `out/src/extension.js`) cover the extension-host side.
- Files over 250 pure LOC carry an `allow: SIZE_OK` header stating why (see session.ts, decode.ts).

## WHERE TO LOOK
| Task | Location | Notes |
| --- | --- | --- |
| Transport / reconnect | src/client.ts | SSE URL carries the negotiated `version=`; reconnect backoff |
| Wire codec | src/codec.ts + src/decode.ts | decodes every daemon v2 SessionEvent variant; an UNKNOWN tag decodes to null, stalls the cursor, opens a gap, and triggers an INFINITE RECONNECT LOOP. Every new daemon variant needs a decode arm BEFORE flip |
| Live session glue | src/session.ts | owns client, gauge, diff decorations, transcript panel, prompts, omnibar, hunk commands |
| Prompt mapping | src/prompts.ts | dismissed prompt (Esc) -> null = STAY PENDING, never auto-deny; deadlines resolve fail-closed daemon-side |
| Ask dialogs | src/askdialogs.ts | canonical labels pinned by grammar.ts: allow-once / allow-session / deny / vote-cancel |
| Omnibar | src/omnibar.ts | pure `runOmnibar(input, deps)`; VS Code surface injected via deps |
| Transcript | src/transcript.ts + src/webview.ts | plain-HTML webview, MAX_ENTRIES = 500 ring, `austringer.showTranscript` |
| Grammar mirror | src/grammar.ts | mirrors daemon grammar.json; grammar.test.ts pins drift |
| IDE bridge / telemetry | src/bridge.ts + src/telemetry.ts | both opt-in via configuration; bridge is a token-checked loopback endpoint |

## ANTI-PATTERNS
- NEVER ship a daemon event variant without its decode.ts arm; unknown tags reconnect-loop forever.
- NEVER add a webview framework; transcript stays plain HTML.
- NEVER add an omnibar command that is not in the daemon grammar table.
- NEVER turn Esc / dismissal into a deny; the ask stays pending.
- NEVER let a TS feature that needs emit into src; tests type-strip only.

## HONEST LIMITATION
Daemon HTTP refuses typed `fallback_answer` / `plan_answer` bodies (identity contract). Client wiring is fakedaemon-proven; a WS carrier is the logged follow-up.
