# Iteration 21 — Agent A Detail: NousResearch/hermes-agent (architecture + full matrix row)

**Repo**: https://github.com/NousResearch/hermes-agent — "The agent that grows with you"
**Verified**: 244,644★ / 50,720 forks (live API, 2026-09-12); pushed 2026-09-12T02:58Z; MIT license; HEAD `284d220ba48e25f2e3623b3afe72db8f24a4c2db`
**Languages (GH linguist)**: Python 77.3 MB dominant, TypeScript 26.2 MB (desktop+TUI), Rust ~196 KB (native shell bits)
**Releases verified**: v0.18.0 "The Judgment Release" 2026-07-01 · v0.19.0 "Quicksilver" 2026-07-20 · v0.20.0 2026-08-03 (codename Herald — quoted in Pantheon notes: "v0.20.0 made Hermes the herald") · **v0.21.0 2026-08-31 "The Pantheon Release"** (body confirms "Since v0.20.0: ~5,800 commits · ~2,475 merged PRs · ~2,100 issues closed · **760+ contributors**") · v0.21.2 2026-09-11
**Total all-time contributors**: ~3,418 (contributors API, anon included). The "760+" figure is the Pantheon *window* (since v0.20.0), per release notes — VERIFIED.
**UNVERIFIED**: "$1.5B valuation talks led by Robot Ventures" (TechCrunch Jul 13 2026) — not checked this pass; treat as rumor. Fork claim "40k" is stale (actual 50.7k).

---

## 1) SKILL SYSTEM — origin story & the self-improving claim

The marketing claim: **"the only agent with a built-in learning loop — it creates skills from experience, improves them during use, nudges itself to persist knowledge"** (README, https://github.com/NousResearch/hermes-agent#readme; docs: https://hermes-agent.nousresearch.com/docs/). The loop is REAL and multi-mechanism (all verified in source at HEAD):

1. **`skill_manage` tool** (`tools/skill_manager_tool.py`, docstring: "Skill Manager Tool - agent-managed skill creation & editing... New skills land in ~/.hermes/skills/"). The agent writes its own SKILL.md files as procedural memory. Default = free writes; opt-in post-write security scan (`skills.guard_agent_created`, default False) and a `skills.write_approval` gate that stages writes under `~/.hermes/pending/skills/` for human approve/deny (https://hermes-agent.nousresearch.com/docs/user-guide/features/skills).
2. **Background review after EVERY turn** (`agent/background_review.py` docstring: "fork the agent to evaluate the turn... asks 'should any skill/memory be saved or updated?'. Writes go straight to the memory + skill stores; the main conversation and prompt cache are never touched... hits the same prefix cache, and runs under a dispatch-side tool whitelist"). This is the core loop: a forked agent decides post-hoc what to persist.
3. **In-use self-improvement**: docs state "Skills self-improve during use" — the background review "can suggest or stage skill changes after a session" (skills docs page). Usage tracked via `tools/skill_usage.py`, provenance via `tools/skill_provenance.py`.
4. **Curator** (`agent/curator.py`, docstring: "Curator - background skill maintenance orchestrator. Inactivity-triggered (no cron daemon): when the agent is idle and the last run is older than interval_hours, maybe_run_curator() auto-transitions lifecycle states... forks an AIAgent that [reviews skills]... uses the auxiliary client and never touches the main session's prompt cache"). Routed to its own cheap aux model slot ("Curator" in the 11 aux slots, https://hermes-agent.nousresearch.com/docs/user-guide/configuring-models).
5. **Nudges**: `memory.nudge_interval: 10` (persist-knowledge nudge every 10 user turns) and `creation_nudge_interval: 15` in `cli-config.yaml.example`.
6. **"Learning made visible"** (`agent/learning_graph.py`: desktop graph of "non-base, learned/profile skills (agent-created or used) plus MEMORY.md/USER.md chunks as first-class nodes").

**Marketplace**: **Skills Hub** = https://agentskills.io (open standard, "Compatible with the agentskills.io open standard" per README). Install sources: Skills Hub official (`tools/skills_hub_official.py`), skills.sh (`skills_hub_skillssh.py`), GitHub repos/taps (`skills_hub_github.py`, `hermes skills tap add`, skills.sh schema with category groupings), **ClawHub adapter** (`skills_hub_clawhub.py`: "ClawHub (clawhub.ai) HTTP API. Every skill is community trust — the ClawHavoc incident (341 malicious skills, Feb 2026) showed their vetting is insufficient") — i.e., it indexes OpenClaw's registry. Every install is content-hash scanned, quarantined on "dangerous" verdicts (prompt-injection directives, credential-exfil commands, hidden-text tricks), recorded in `skills/*/**/lock.json` (skills docs). Bundled skills sync to `~/.hermes/skills/` with a `.bundled_manifest` hash ledger; project-repo skills (`.hermes/skills/`, `.agents/skills/`) win precedence `project → local → external_dirs` and are scanned on every content change.

**Verdict**: the self-improving loop is real but **knowledge-level, not weight-level**: skills/memory curation + optional evolutionary optimization via the companion repo `NousResearch/hermes-agent-self-evolution` (DSPy+GEPA; Phase 1 SKILL.md evolution implemented; "All changes go through human review, never direct commit", ~$2–10/run; https://github.com/NousResearch/hermes-agent-self-evolution). No fine-tuning of the model itself in-repo; instead "Research-ready" trajectory generation for RL (`batch_runner.py`, `trajectory_compressor.py`, Atropos RL environments per README).

## 2) 2026 PLATFORM EXPANSION (Pantheon + Herald window)

- **Voice + wake words**: `agent/tts_provider.py`, `tts_registry.py`, `transcription_provider.py`, `tools/voice_mode.py`, `voice_live.py`, `tools/wake_word.py` + `wake_word_engines.py`. Deps: `openwakeword==0.6.0` (free default, ONNX/tflite on macOS ARM64), `pvporcupine==4.0.3`, `sounddevice==0.5.5` (pyproject). Docs: Voice Mode in CLI, Telegram, Discord + Discord VC (https://hermes-agent.nousresearch.com/docs/user-guide/features/voice-mode). Pantheon credits "@helix4u (31 PRs) — wake-word capture...".
- **A2A v1.0 — VERIFIED**: https://hermes-agent.nousresearch.com/docs/user-guide/messaging/a2a — "A2A is the open Agent2Agent protocol (v1.0, stewarded by the Linux Foundation)". Bidirectional: outbound tools (`a2a_discover/call/list/history/orchestrate`) + inbound server (Agent Card at canonical v1.0 `/.well-known/agent-card.json`, JSON-RPC 2.0 `SendMessage/SendStreamingMessage/GetTask/...`, SSE streaming, HMAC-signed push notifications, anti-loop caps, per-peer tokens, audit jsonl). "Interoperability verified against the official Python a2a-sdk." In code: `agent/turn_author.py` `a2a_key()` = `a2a:<bot id>` turn authorship; gateway platform adapter `a2a`.
- **`hermes peer` bot-to-bot DMs**: Pantheon release body — "Any Hermes agent can now message any other by handle, across profiles and gateways... Replies land in each agent's canonical Bot Chat" (PRs #88725/#88178/#91487, @teknium1).
- **Kanban multi-agent**: `tools/kanban_tools.py` + `gateway/kanban_watchers*.py`; Pantheon: "Kanban hardening — collision-hotspot flagging, split-brain decision-ownership contract, memory-aware dispatch guard, notify/wake delivery modes" (#83428/#83424/#88115/#85487). Aux slots exist for Kanban triage-specifier and decomposer models.
- **Desktop app**: native app (Rust bits + TS), Bot Mode bundled default-on — "a society of named agents with their own faces and group chats" (#87886 et al.), agent-drivable in-app browser (#90197), MCP command center dashboard (#87525+), seq-stamped WS event replay (#94219), detached update hand-off, macOS TCC identity persistence (#95091). Community desktops: `fathah/hermes-desktop` (14.2k★), `nesquena/hermes-webui` (18.3k★), `outsourc-e/hermes-workspace` (6.6k★).
- **Messaging platforms**: docs comparison table lists **28 platforms** — Telegram, Discord, Slack, Google Chat, WhatsApp, WhatsApp Cloud API, Signal, SMS, Email, Home Assistant, Mattermost, Matrix, DingTalk, Feishu/Lark, WeCom, WeCom Callback, Weixin, BlueBubbles (iMessage), Photon (iMessage), QQ, Yuanbao, Teams, LINE, ntfy, Raft, IRC, Buzz, SimpleX (https://hermes-agent.nousresearch.com/docs/user-guide/messaging/) — plus A2A, Webhooks, API server, Relay connector layer. **The brief's "22 platforms" is an undercount.** Deep Chinese-platform coverage (QQ/Yuanbao/DingTalk/WeCom/Weixin/Feishu) + zh-CN README/docs = deliberate TAM strategy.
- **MCP 2.x SDK — VERIFIED**: pyproject pins `mcp==2.0.0` ("implements MCP revision 2026-07-28, moved its HTTP stack from httpx to httpx2"), both as client extras and dev dep; CVE-pinned `starlette==1.3.1`.
- **Browser control**: full stack — `tools/browser_tool*.py` (Camoufox stealth Firefox, CDP, Lightpanda fallback, cloud, vision-mode, real-profile, snapshot), `browser_supervisor*.py`, `browser_use_cli.py`, plus desktop in-app browser driving.

## 3) AGENT LOOP + CONTEXT ENGINE + SESSIONS

- **Turn machinery**: `agent/turn_*.py` (~35 modules: preflight, iteration prep, tool round, response intake, recovery, overflow, stop gates, finalizer, usage) under `run_agent.py` (86 KB) + `agent/conversation_loop.py`. MoA loop (`moa_loop.py`), fast_mode, iteration_budget, repetition/empty-response guards.
- **Context engine**: pluggable ABC `agent/context_engine.py` ("ContextCompressor is the default" — alt: 'lcm'). Compressor (`agent/context_compressor.py`): "cheap auxiliary model summarizes middle turns while head and tail are protected (iterative summaries, token-budget tail, tool-output pruning first, scaled budgets)"; config: `protect_first_n: 3`, `protect_last_n: 20`, `target_ratio: 0.20`, proactive prune (`proactive_prune_tokens`), `micro_compaction.py`, `native_compaction.py` (provider-side), stale-image eviction, handoff-scan so cross-window summaries aren't mistaken for instructions. Multi-compaction strategy per route (Codex caps Astra at 272K vs OpenRouter full window).
- **Sessions**: SessionDB = SQLite WAL (`journal_mode: "wal"` default) split across `hermes_state_*.py` (~20 modules: schema, sessions, messages, fts, search, repair, wal, readpool, portability, compression). **FTS5** with CJK-bigram tokenizer (`hermes_state_fts.py`), cross-session recall with LLM summarization, delivery ledger with at-least-once redelivery semantics ("♻️ Recovered reply" dup labeling), durable `/model` overrides surviving gateway restarts (https://hermes-agent.nousresearch.com/docs/user-guide/messaging/).

## 4) PERMISSIONS / SANDBOX POSTURE

Docs: "defense-in-depth... eight layers" (https://hermes-agent.nousresearch.com/docs/user-guide/security):
1. User authorization (allowlists + DM pairing: 8-char codes, `secrets.choice`, 1h TTL, rate-limited, lockout; fail-closed default deny)
2. Dangerous-command approval: modes **smart** (aux-LLM risk scoring) / **manual** / **off**; per-surface defaults `cron_mode/single_query_mode/unattended_mode: deny` (fail-closed); YOLO toggle with persistent banner
3. **Hardline blocklist floor** — `rm -rf /`, fork bombs, `mkfs`, `dd` to block devices survive YOLO/off; user-extensible `approvals.deny` glob rules evaluated on deobfuscated command variants (`git pu""sh` caught); `hermes approvals suggest` mines approval history into allowlists (destructive classes never proposed)
4. File-write safety: protected credential paths always blocked (`~/.ssh`, `.env`, MCP tokens); optional `HERMES_WRITE_SAFE_ROOT` prefix sandbox; file-mutation verifier footer
5. Container isolation: Docker `--cap-drop ALL --security-opt no-new-privileges --pids-limit 256` + tmpfs noexec; dangerous-cmd checks *skipped* when the container is the boundary; backends local/ssh/docker/singularity/modal/daytona/vercel_sandbox (microVM)
6. MCP credential filtering (env stripping + skill-scoped `required_environment_variables` passthrough)
7. Cross-session isolation + cron path-traversal hardening
8. Input sanitization (cwd allowlists)
Plus gateway **admin/user tiers** per scope (`allow_admin_from` / `user_allowed_commands`, `/whoami`), MCP skills scanning/quarantine (above), A2A localhost-by-default + injection filtering + reply redaction.

## 5) MODEL SUPPORT

Fully BYO + Nous-native funnel. Main model + **11 auxiliary slots** (title-gen, vision, compression, approval scoring, web extract, skills-hub search, MCP routing, kanban triage/decomposer, profile describer, curator), each `auto` by default with per-task fallback chains (https://hermes-agent.nousresearch.com/docs/user-guide/configuring-models). Adapters in-tree: Anthropic (native + OAuth + credentials), Bedrock, Vertex, Gemini-native, Codex (responses + app-server), OpenAI-compat, OpenRouter, Moonshot, LM Studio, Nous Portal (`nous_wire.py`, `nous_rate_guard.py`; "300+ models under one subscription", 10% off token-billed providers). Model aliases, `/model --once/--global`, credential pools, fallback-providers, curated catalogs. **No hard tie to Nous Hermes models** — the agent is model-agnostic; Nous monetizes via Portal subscription & free tier, and harvests trajectories (Atropos) for RL. Notable hygiene: data-training-tier models (`-contributor` suffix) get explicit consent gating, fail-closed on unattended surfaces.

## 6) WHY 244k★ — GROWTH MECHANICS (analysis, evidence-backed)

1. **Positioning**: "self-improving" + "grows with you" is the differentiator vs every other harness; the learning loop is real enough to demo (learning graph, skill ledger).
2. **OpenClaw on-ramp**: README ships a migration path ("If you're coming from OpenClaw, Hermes can automatically import your settings, memories, skills, and API keys" — skills land in `~/.hermes/skills/openclaw-imports/`); dedicated migration skill v2 with "17 new modules" (PR #2906); ClawHub registry adapter indexes the rival's ecosystem. It explicitly competes with OpenClaw while absorbing its users AND its skills.
3. **Runs anywhere**: $5 VPS → serverless (Modal/Daytona "hibernate when idle... costing nearly nothing", `gateway/scale_to_zero.py`); 28+ chat platforms incl. full China coverage; native Windows + Android (Termux constraints file).
4. **Voice + wake word + desktop Bot Mode** = consumer spectacle (agents with faces in group chats) that drives stars.
5. **Cadence & contributor flywheel**: ~5,800 commits / 2,475 PRs / 760+ contributors in the ~4-week Pantheon window; release notes credit dozens of community handles; heavy localization (es/zh-CN/ur-PK READMEs; i18n module).
6. **Research credibility**: Nous Research brand, Atropos trajectory generation, self-evolution (DSPy/GEPA) companion.
7. **Security signaling post-ClawHavoc**: quarantine/scanner/lock.json posture turned a community incident into a trust moat.
Cost: solo-maintainer concentration risk visible (teknium1 ~1,340 merged PRs in the window, OutThisLife 209).

---

## FULL MATRIX ROW (● full / ◐ partial / ○ absent) — evidence at HEAD `284d220`

| Capability | Score | Evidence |
|---|---|---|
| language | ● | Python 77.3 MB / TS 26.2 MB (GH linguist API); pyproject 33.8 KB, uv.lock |
| RSS | ○ | No feed reader/`feedparser` anywhere; web_tools can fetch any URL and cron can poll, but nothing RSS-native shipped |
| OS sandbox | ● | 7 terminal backends (local/docker/singularity/modal/daytona/vercel_sandbox microVM/ssh), hardened Docker flags, scale-to-zero serverless (`tools/terminal_tool.py:4-6`, security docs table) |
| permission tiers | ● | smart/manual/off + YOLO + hardline floor + user deny rules + admin/user scopes + DM pairing + write-safe-root + fail-closed headless modes (8-layer docs, tools/approval*.py) |
| prefix-cache | ● | `agent/prompt_caching.py`: 4 cache_control breakpoints, shared TTL 5m/1h, Qwen detection, OpenRouter part-level relocation; boundary/scope modules; background review shares prefix cache; dashboard cache-reset cost warnings |
| AST repomap | ○ | zero tree-sitter/repomap hits repo-wide; only `tools/skills_ast_audit.py` (AST security audit of skills) |
| windowed file view | ◐ | `read_file(path, offset=1, limit=2000)` paginated reads + read-tracking/dedup (`tools/file_operations.py:100`, `file_tools_read_tracking.py`); no map-style repo viewport |
| PTY | ● | `pty=true + background=true` interactive CLIs, process write/submit driving — "Local backend only" (`tools/terminal_tool.py:166`) |
| tiered eviction | ● | protected head/tail + middle summarize, tool-output prune-first, in-protected demotion under pressure, stale-image eviction, proactive prune, micro/native compaction, pluggable engines (context_compressor.py:1-2,977-988,1249) |
| subagents | ● | delegate_tool family (10 modules: registry, dispatch, worktrees, progress, live-steer), /bg background sessions, code_execution RPC, Pantheon "subagents steered live mid-flight" |
| multi-model | ● | Any provider; native adapters (Anthropic/Bedrock/Vertex/Gemini/Codex/OpenRouter/Moonshot/LMStudio/Nous Portal), fallback chains, credential pools, `/model` hot-swap |
| model profiles | ● | Agent profiles (own HERMES_HOME/models/memories/skills), profile distributions, per-channel `channel_overrides` model+prompt, 11 aux model slots, aliases |
| skills | ● | Skills Hub (agentskills.io standard), 5 install sources incl. ClawHub + GitHub taps, bundled/project/local tiers, scanner+quarantine+lock.json, slash-invocable |
| auto skill learning (core question) | **●** | **Real, 5-mechanism loop**: skill_manage writes + post-turn background_review fork ("should any skill/memory be saved?") + in-use self-improvement + idle curator + creation nudges; optional human gate (`skills.write_approval`); knowledge-level not weight-level; DSPy/GEPA companion repo under human review |
| MCP client | ● | `mcp==2.0.0` (rev 2026-07-28), 20+ `tools/mcp_tool_*.py` (OAuth device flow, sampling, health, schema cache, loop, scope), MCP command center, `/reload-mcp` cache-invalidation warning |
| MCP server | ● | `mcp_serve.py` (31 KB) + `agent/transports/hermes_tools_mcp_server.py` (exposes Hermes tools as MCP server) |
| WASM | ○ | No runtime; only incidental doc/string mentions |
| sessions | ● | SQLite WAL SessionDB (~20 hermes_state_* modules), FTS5 CJK-bigram search, LLM-summarized recall, restart-surviving model overrides, at-least-once delivery ledger, auto-archive sweep |
| encryption | ◐ | Fernet-encrypted local vault for browser secrets/TOTP (`agent/vault_store.py`); 1Password/Bitwarden vault backends; SSL guard — but state.db/ transcripts NOT encrypted at rest |
| checkpoints | ● | shadow-git checkpoint store, pre-mutation snapshots per turn, `/rollback`, cross-project blob dedupe, agent-write ledgers (`tools/checkpoint_manager.py:1-7`) |
| cost HUD | ● | `/usage`, `/insights`, per-model analytics cards w/ cost, desktop fleet cost/usage overlay (MCP schema token estimates, 30-day usage), credits_tracker, cache-reset cost warnings, `-contributor` tier notices |
| frontends | ● | CLI, full TUI (`ui-tui/`), native Desktop (Bot Mode, dashboard, in-app browser), gateway, community WebUI/workspace (18.3k★/6.6k★) |
| multi-client | ● | one gateway → 28 platforms concurrently, per-channel overrides, multi-profile gateways, desktop↔many-instances, A2A inbound |
| telemetry | ◐ | OTLP exporter + health/monitoring emitter with pseudonymous rotatable install-id and redaction module (`agent/monitoring/policy.py`) — operator observability, no forced product analytics found |
| evals | ◐ | `evals/` = ~25 wire/regression probes (kanban, cron, compaction scorecards), `runtime_bench.py`; batch trajectory generation + Atropos for RL; NO public agent-capability benchmark suite |

**Row tallies**: 18● · 5◐ · 3○ (RSS, AST repomap, WASM absent).
