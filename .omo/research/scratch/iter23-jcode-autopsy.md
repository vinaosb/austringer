# jcode Autopsy — Iteration 23

**Repo**: https://github.com/1jehuang/jcode (Rust, MIT)
**Cloned at commit**: `33daef65e5786130ed490feae51e5a321ec496ec` (2026-09-12, "docs: update weekly stars chart")
**Verified meta**: 19,520★ / 2,252 forks (per task brief; repo docs self-chart daily). Latest release **v0.84.0** (2026-09-07). Contributors API: `1jehuang` **7,284** of 7,323 commits (99.5%); remaining 39 = github-actions[bot] (31) + 4 humans with 8 commits total (luciusrockwing 4, FabianAmherd 2, AlazBatu 1, hongnoul 1). **Solo bus factor CONFIRMED.**
**Scale**: 80 crates under `crates/` (counted on disk), ~1,880 files at depth-1. Release cadence sample: 12 releases Aug 25→Sep 7 2026 ≈ **20/mo** — matches ~22/mo brief claim (VERIFIED via releases API).

Permalink base: `https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/<path>`

---

## 1. RAM mechanism — VERIFIED IN CODE

The claim "10 sessions ≈ +9.9MB marginal vs Claude Code ~212.7MB" rests on a real architectural substrate, all four sub-mechanisms found:

### 1a. Client-server daemon split — ● VERIFIED
`docs/SERVER_ARCHITECTURE.md`: single-server, multi-client. One `jcode serve` daemon (spawned detached via `setsid()`), all TUIs connect as thin clients over Unix socket `/run/user/$UID/jcode.sock`. Server owns: provider connections, **MCP pool shared across sessions**, all session state. Clients reconnect with exponential backoff; server idles out after **5 min with no clients**. Hot reload = server `exec()`s into the new binary on the same socket (same PID).
- Doc: `docs/SERVER_ARCHITECTURE.md:11-36, 76-94, 121-128`
- Socket lifecycle hardening (dead-daemon stale socket reaped only when `flock` on `jcode-daemon.lock` proves owner dead; regression notes cite issues #277/#291): `crates/jcode-app-core/src/server/socket.rs:46-100` — unusually mature failure-mode thinking.
- Harness API bridge (stable versioned API on its own socket, JSON→JSON translation to the legacy daemon protocol so the daemon never breaks): `crates/jcode-harness-api-server/src/lib.rs:1-15`.
- SDK `launch()` spins a *private* daemon under a dedicated `JCODE_HOME` for embedders: `sdk/typescript/src/launch.ts:1-19`.

**Why it saves RAM**: model/provider/MCP state amortizes across sessions; clients are renderers. The 10-session table (117MB total, embeddings off) is the daemon + 10 attach costs — marginal per-session cost is a socket + render state, not a full agent runtime. This is the same amortization OpenCode *could* claim but doesn't (OpenCode measured at 3,237MB / 10 sessions in jcode's README table — self-reported).

### 1b. Prompt-cache-preserving append-only context — ● VERIFIED
`crates/jcode-base/src/cache_tracker.rs:1-14`: "Client-side cache tracking for append-only validation… If the prefix changes between requests, we know the cache was invalidated." Uses `stable_message_hash` + rolling `extend_stable_hash` prefix chain (`:55-76`), 10-deep violation history. This *detects* cache violations for providers that don't report cache metrics; the MCP schema cache (1c) *prevents* the known violation class (#206: late tool-snapshot rebuild after MCP handshake = intentional cache miss).

### 1c. On-disk MCP schema cache — ● VERIFIED
`crates/jcode-base/src/mcp/schema_cache.rs:1-31`: `~/.jcode/mcp-schema-cache.json`. At session spawn, advertise tools immediately from schemas observed last connection → "the very first locked tool snapshot already contains them — zero cache miss in the common case." Correctness guard: entries keyed by **config fingerprint** (command+args+sorted env+transport+url+headers+shared, `fingerprint_config` at `:63-92`); cache is "a hint, not truth," reconciled against live `tools/list` after real connect.

### 1d. Session sharing — ● VERIFIED (as daemon-owned sessions)
Sessions live in the daemon; clients attach/detach/resume (`jcode --resume fox`). Multi-client attach to one session, takeover, and reconnect-with-history all have dedicated test suites (`crates/jcode-app-core/src/server/client_session_tests/resume/*.rs`, 8 scenario files). Multi-session *client UI* (surfaces) is still **Proposed**, not built: `docs/MULTI_SESSION_CLIENT_ARCHITECTURE.md:1-3`.

**RAM table audit** (README.md:76-136, 154-170): 1-session jcode 27.8MB **with "(local embedding off)" in the row label** and 167.1MB with embeddings on (6.0x row vs Codex) — the asterisk is disclosed in-table, not hidden. Methodology (README:223, 242, 267): "Measured on this Linux machine across 10 interactive PTY launches," competitor versions pinned (e.g. "GitHub Copilot CLI 1.0.24 for the 1-session rerun, 1.0.27 for the 10-session rerun"). PSS methodology, machine disclosed. Still self-reported, single-machine, author-chosen workloads → treat as directional, not citable.

---

## 2. Self-dev mode — ● VERIFIED (mechanics + guardrails)

jcode develops itself: when the client runs inside the jcode repo, it auto-detects, connects to the shared server, and marks that session as canary/self-dev via subscribe metadata (`docs/SERVER_ARCHITECTURE.md:138-147`).

**Tool surface** (`crates/jcode-app-core/src/tool/selfdev/mod.rs`): actions `build | build-reload | test | cancel-build | reload | status` (`:512-517`). Clone source is hardcoded to the public repo (`JCODE_REPO_URL`, `:36`).

**The loop**: agent edits its own source → `build` (queued, background) → `build-reload` → server `exec()`s new binary, all clients auto-reconnect → `ReloadContext` (task_context, version_before/after, session_id) persisted per-session to `~/.jcode/reload-context-<sid>.json` and restored after restart as a continuation message (`crates/jcode-app-core/src/tool/selfdev/reload.rs:14-45`; `ReloadRecoveryDirective` in `crates/jcode-selfdev-types/src/lib.rs:20-23`). The agent literally survives its own hot-patch with context intact.

**Guardrails found**:
- **Build queue serialization per worktree scope** with file-based request records, tolerance for transient unreadable entries, and cancellation: `crates/jcode-app-core/src/tool/selfdev/build_queue.rs:29-60` (`wait_for_turn`).
- Cargo wrapped so dev builds route through `JCODE_DEV_CARGO_SCRIPT` with re-entrancy flag `JCODE_IN_DEV_CARGO`: `build_queue.rs:4-21`.
- Canary lifecycle types: `CanaryStatus::{Testing,Passed,Failed}`, `CrashInfo{build_hash, exit_code, stderr, git diff}`, `MigrationContext` saved pre-migration (`crates/jcode-selfdev-types/src/lib.rs:127-188`).
- Reload-loop suppression + "never downgrade on version mismatch" logic scanning both self-dev flavors; issue #291 post-mortem comments in `crates/jcode-app-core/src/server/util.rs:88-95, 440-445` — a real self-hosting failure mode (self-dev daemon pinned to old build reload-loops) was hit and fixed.
- Channel promotion deliberately split from reload (`jcode server promote`): "updates may advance a shared server… but must not overwrite a build the user deliberately promoted" (`src/cli/commands.rs:2061-2089`).

**Verdict**: this is the most complete publicly-visible agent-self-development loop in any OSS harness — versioned builds, crash capture with diffs, queueing, reload-context carryover, and documented regressions from actually running it. It also explains the 20 releases/mo: the author's agent ships the author's tool.

---

## 3. Swarm-core + memory graph — ●/◐ MIXED

### Swarm (◐ — architecture real, doc says "Largely implemented," comm migration "in progress")
`docs/SWARM_ARCHITECTURE.md:1-5` status line is honest. Verified in code:
- **Mode-gated spawning**: ad hoc/light = one-level fan-out (root only); `swarm-deep` allows recursive spawning with live-worker budget + member cap; ancestry via `report_back_to_session_id`; subtree-scoped stop permissions; orphaned children reparented to grandparent (`SWARM_ARCHITECTURE.md:22-58`).
- **Comms**: DM / subtree broadcast / channels / shared-context-keys (last two "discouraged" in own docs). Tool enum verified: `["share","share_append","read","message","broadcast","dm","channel","list","list_channels","channel_members",…]` and `"dm" =>` dispatch at `crates/jcode-app-core/src/tool/communicate.rs:1969, 2354`.
- **Notifications as soft interrupts** injected at safe points; idle agents never auto-resume (`SWARM_ARCHITECTURE.md:209-219`).
- **DAG-first task graph** superseding agent-first framing: `docs/SWARM_TASK_GRAPH.md`; plan ops (assign/approve/task-control) gated to a single root-claimed coordinator slot per `swarm_id` — one `VersionedPlan` per swarm, sane.
- Message hygiene in `crates/jcode-swarm-core/src/lib.rs:5-15`: completion reports capped 4,000 chars, `tldr` mandatory >240 chars. Discipline rare in multi-agent code.

### File-touch collision alerts — ● VERIFIED (this is the "collision notification" mechanism)
- write/edit/apply_patch publish `BusEvent::FileTouch{path, op, intent, summary, detail}` to the global bus "for swarm coordination" (`crates/jcode-app-core/src/tool/write.rs:89-90`, `edit.rs:118-120`; event def `crates/jcode-base/src/bus.rs:401-402`).
- Server consumes it (`crates/jcode-app-core/src/server.rs:1983-2105`): records into dual-index `FileTouchService` (forward path→accesses, reverse session→paths, `crates/jcode-app-core/src/server/file_touch_service.rs:26-31`), records a swarm event, then **on modification only**, looks up prior peer touches within the same swarm and "alert[s] both sides so they can coordinate before the work diverges further" (server.rs:2093-2098). Reads tracked but silent. Touches expire on a cleanup interval.

### Memory graph (◐ — core shipped, advanced tiers not)
`docs/MEMORY_ARCHITECTURE.md` status: "Implemented (Core), Planned (Graph-Based Hybrid)" — though Phase 4 graph is checked ✅. Verified in code:
- HashMap-based graph (chose simpler-than-petgraph for JSON serialization), `GRAPH_VERSION=2`, `EdgeKind{HasTag, InCluster, RelatesTo{weight}, Supersedes, Contradicts, DerivedFrom}` with BFS traversal weights (HasTag 0.8 / InCluster 0.6 / Supersedes 0.9): `crates/jcode-memory-types/src/graph.rs:16, 90-120`.
- **Cosine retrieval**: `cosine_similarity` + `batch_cosine_similarity` (top-K via bounded heap, `top_k_scored`) in `crates/jcode-embedding/src/lib.rs:369-410`; local embeddings = all-MiniLM-L6-v2 via tract-onnx (doc:713).
- Cascade retrieval: top-10 embedding hits → BFS depth 2 with 0.7/step decay → sidecar LLM relevance filter ("GPT-5.3 Codex Spark" — pinned to a specific small model, UNVERIFIED generalizability) → results land one turn later (fully async, non-blocking `try_send`) (doc:219-325, 618-667).
- Write-time consolidation: duplicate → reinforce, contradiction → supersede, with `Vec<Reinforcement>` provenance (doc:751-760). Privacy section explicitly bans storing API keys/secrets, regex-filtered (doc:778-796).
- **NOT shipped**: HDBSCAN clustering is doc'd as algorithm but Phase 6 negative memories / temporal awareness unchecked; Phase 8 graph-wide consolidation all unchecked (doc:737-774). Cluster/`InCluster` edges exist in types; auto-discovery maturity UNVERIFIED.

---

## 4. Matrix row scoring (for iter23 table)

| Claim | Score | Evidence |
|---|---|---|
| Client-server daemon, session amortization | ● | SERVER_ARCHITECTURE.md + socket.rs + 8 resume test suites |
| MCP schema disk cache (prompt-cache preservation) | ● | schema_cache.rs:1-31 + fingerprint guard :63-92 |
| Append-only context / cache violation detection | ● | cache_tracker.rs:1-14, 55-76 |
| Self-dev loop w/ hot-reload + guardrails | ● | selfdev/* (mod, build_queue, reload), selfdev-types, util.rs #291 notes |
| Swarm multi-agent (DAG, DMs, collisions) | ◐ | doc "Largely implemented"/"in progress"; comms + collisions + DAG verified; worktree-manager integration flow not end-to-end verified |
| Memory graph w/ cosine retrieval | ◐ | graph.rs + cosine verified; clusters/negative/temporal/Phase-8 unshipped |
| RAM claims (27.8MB / ~9.9MB marginal) | ◐ | mechanism real; numbers self-reported, single machine, embeddings-off asterisk disclosed in-table |
| Solo authorship | ● | 7,284/7,323 commits; 8 human non-author commits ever |

---

## 5. r/opencodeCLI criticism — "API keys and secrets all over"

**Verdict: PARTIALLY SUBSTANTIABLE, but the criticism is weaker than it sounds.**

How keys are actually stored (verified `docs/AUTH_CREDENTIAL_SOURCES.md` + code):
- **Dual-auth design**: OAuth (`~/.jcode/auth.json` → `anthropic_accounts[].access`, `sk-ant-oat…`) and API keys (`ANTHROPIC_API_KEY` env **or** `~/.config/jcode/anthropic.env`, written by `jcode login --provider anthropic-api`) are separate providers. Same for OpenAI (`~/.jcode/openai-auth.json` vs `~/.config/jcode/openai.env`).
- So yes: **plaintext env-style files in `~/.config/jcode/`** — multiple files, multiple locations (`~/.jcode/auth.json`, `~/.jcode/openai-auth.json`, `~/.config/jcode/*.env`, plus `~/.jcode/auth-validation.json` cache). A grepper finding "keys in several files" is correct.
- Mitigations verified: `harden_secret_file_permissions` (`crates/jcode-storage/src/lib.rs`) chmods owner-only (0o600, test-asserted in `crates/jcode-base/src/storage/tests.rs:34,139`) and on Windows replaces ACLs — synchronously on writes, deferred/coalesced on hot read paths with explicit perf rationale. OAuth refresh handled in-file. Import of *other tools'* credentials is **consent-gated per source** (`crates/jcode-base/src/auth/external.rs`, "Nothing is copied into jcode's own stores; the external file is read in place"; `!`-prefixed shell-command values never executed — `AUTH_CREDENTIAL_SOURCES.md:99-127`). It can *read* Claude Code's macOS Keychain (`claude.rs:16,700`) but stores nothing in an OS keychain itself.
- `jcode auth status --json` is the canonical inspection surface; the doc opens by mocking naive grep-based debugging — the author knows the layout is confusing and wrote a single-source-of-truth doc because "the same confusion keeps recurring."

**Fair comparison**: OpenCode/pi/many peers also store OAuth in plaintext `auth.json` (~/.local/share/opencode/auth.json is listed in jcode's own import table). File+chmod is the ecosystem norm; a keyring would be better; "all over" is rhetorical. The legitimate kernel: API keys in *dot-env files under a config dir* plus a validation-cache JSON can absolutely leak via misconfigured backups/dotfile syncers, and jcode ships no OS-keychain option for its own keys.

---

## 6. ADOPT / AVOID

### ADOPT (for our harness)
1. **Client-server daemon + session amortization** — biggest structural lever for multi-session RAM; exec()-same-socket reload is elegant (zero-downtime upgrade for a TUI app). Port the pattern, not the code.
2. **MCP schema disk cache with config fingerprinting** — cheap, kills the #1 prompt-cache miss at spawn; the fingerprint-invalidate guard is the part everyone else omits.
3. **Append-only prefix-hash cache-violation tracker** (~400 LOC) — provider-agnostic cache-regression detector, great CI/test hook.
4. **File-touch collision alerts in multi-agent runs** — write/edit publishes intent+summary; server alerts *both* sides on overlapping modification. Small, high-value coordination primitive.
5. **Self-dev build queue + reload-context carryover** — if we ever do agent-self-development, copy the guardrail set: serialized builds per scope, canary status machine, crash capture w/ diff, reload loop suppression, promote-vs-reload separation.
6. **Stale-socket reaping gated on flock proof** (socket.rs) — the exact daemon-lifecycle edge most implementations get wrong.

### AVOID / DEPRIORITIZE
1. **Solo-bus-factor dependency** — 19.5k★ with 8 human non-author commits ever; release cadence and roadmap are one person's wrist. Fine to adopt *patterns*; do not build critical workflow on jcode binaries. No governance, no co-maintainers visible.
2. **80-crate fragmentation as a style** — crate-per-type granularity (`jcode-ambient-types`, `jcode-side-panel-types`…) is churn-prone at 20 releases/mo; their own docs (COMPILE_TIME_ISOLATION_REFACTOR.md, CRATE_OWNERSHIP_BOUNDARIES.md, TUISTATE_TRAIT_DECOMPOSITION.md) are mostly about managing the fallout. Adopt the *seams*, not the count.
3. **Self-reported benchmark tables as growth collateral** — methodology is disclosed and pinned, but single-machine PSS with author-chosen workloads and an embeddings-off headline row; never cite without reproducing.
4. **Memory sidecar pinned to one small model** ("GPT-5.3 Codex Spark") as relevance gate — model-specific coupling in an architectural load-bearing spot.
5. **jcode-bench "uncontaminatable benchmark"** — manifesto page (jcode.sh/bench) still UNVERIFIED/unfinished per brief; nothing in-repo yet proves contamination-proofing (ATTRIBUTION_BENCHMARK.md / DISCOVERY_* docs exist but full validation UNVERIFIED from this slice).

### WATCH
- Swarm DAG task graph + coordinator slot: right topology (single VersionedPlan per swarm, subtree ownership); revisit when comm migration "in progress" lands.
- Multi-session client surfaces: still Proposed; if shipped, the RAM/marginal-session story gets even stronger.

---

## 7. Growth-mechanics verdict: **engineered organic** (Reddit-first, data-driven)

Not passive organic, not astroturf — it's *instrumented* grassroots:
1. **Subreddit targeting dashboard** checked into the repo: `docs/reddit_dashboard.py` hardcodes a 15-subreddit table with subs count, author's avg/max upvotes & comments per sub, subjective `relevance` score, and tier 1-5 (r/ClaudeAI tier 1 relevance 10 → r/SideProject tier 5). Plus `docs/jcode_reddit_dashboard.png` rendered output. The author A/B-tests where to post.
2. **Automated star-chart freshness**: `.github/workflows/update-star-history.yml` — daily cron (`17 3 * * *`) regenerates `docs/images/star-history.svg` via `.github/scripts/generate_star_history.py` and auto-commits. The repo's *latest commit* at clone time was literally "docs: update weekly stars chart" — the README carries a perpetually-fresh growth chart as social proof.
3. **Benchmark-table marketing**: README leads with "The most RAM efficient harness" + PSS tables vs pinned competitor versions (README:12, 76-136) — linkable, screenshot-able claims built for Reddit/Twitter circulation.
4. Solo author, 176 releases, docs/README engineered for shareability.

**Verdict**: growth is deliberate solo dev-marketing with measurement loops (subreddit analytics + automated social-proof chart), distribution-wise Reddit-first. Effective and honest-ish (methodology disclosed, asterisks in-table), but it means the 19.5k★ measures marketing ops + RAM-niche differentiation as much as adoption. For our purposes: copy the *instrumentation* habit (track where posts perform; keep a fresh chart), don't confuse star velocity with contributor depth — this repo has none of the latter.

---

## Source index (commit `33daef65`)
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/docs/SERVER_ARCHITECTURE.md
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/docs/SWARM_ARCHITECTURE.md
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/docs/MEMORY_ARCHITECTURE.md
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/docs/AUTH_CREDENTIAL_SOURCES.md
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-base/src/mcp/schema_cache.rs
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-base/src/cache_tracker.rs
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-app-core/src/tool/selfdev/mod.rs
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-app-core/src/tool/selfdev/build_queue.rs
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-app-core/src/server.rs#L1983-L2105
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-app-core/src/server/file_touch_service.rs
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-memory-types/src/graph.rs
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-embedding/src/lib.rs#L369-L410
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/crates/jcode-storage/src/lib.rs
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/docs/reddit_dashboard.py
- https://github.com/1jehuang/jcode/blob/33daef65e5786130ed490feae51e5a321ec496ec/.github/workflows/update-star-history.yml

*Local clone: `C:\Users\vsbb1\AppData\Local\Temp\opencode\jcode`. UNVERIFIED items flagged inline: jcode-bench contamination-proofing, Desktop closed binaries, live RAM reproduction, HDBSCAN cluster discovery maturity.*
