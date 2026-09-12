# Harness Research & Decision Record — companion to `.omo/plans/rust-agent-harness.md`

> Living document. Method: ONE harness per iteration, several parallel agents on
> different aspects (architecture / features / git-history mistakes+reversions /
> community+performance). Iterate until no comparable harness remains. Every
> ADOPT/AVOID decision must cite evidence here.

## 1. Master functionality matrix (harness × capability)

Legend: ● yes · ◐ partial · ○ no · A=ADOPTING · V=AVOIDING · M=missed-by-us

| Capability | Claude Code | Codex CLI | opencode | goose | aider | Gemini CLI | Qwen Code | crush | OpenHands | SWE-agent | Mentat | Plandex | Pi | Grok | Devin | Copilot | Cursor | Antigravity | Zed | JetBrains | Warp | Droid | Amp | Rust micro | Hermes/Auto | Continue/Cline | T-bench/Harbor | Ours |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Language | TS (Bun) | Rust | TS (Bun) | Rust | Python | ● [enriched, FINAL] TS/Node>=20, Ink/React 19 TUI; 7-pkg monorepo (cli/core/sdk/a2a-server/devtools/vscode-ide-companion/test-utils); Apache-2.0 post-2026-06-18 = enterprise+API-key maintenance only | TS (Node 22; npm-workspaces monorepo + Rust auxiliaries (cua-driver, desktop src-tauri); gemini-cli fork: v0.1.x-era import 2025-06-26, history grafted 2025-08-02, LAST upstream sync v0.8.2 2025-10-23, independent since) | Go | Python | Python | Python (+TS ext) | Go (cli+server monorepo) | TS (11-pkg monorepo) | Rust (1.3M LoC sync-mirror) | ○ closed (SaaS + closed IDE; publishes research posts, not source) | ○ closed (SaaS on GitHub Actions substrate; VS Code/CLI clients closed) | ○ closed (Electron/VS Code-fork IDE + closed agent CLI binary + cloud agents; TS SDK @cursor/sdk Node 22.13+ w/ per-platform native binaries; CLI impl language UNVERIFIED; SpaceXAI-owned since 2026-08-14) | ◐ [corrected from ○] (closed core but surfaces characterized: Electron desktop 2.0 + VS Code-fork IDE [3rd-party RE]; agy CLI closed, Go per community UNVERIFIED; Python SDK google-antigravity w/ open examples repo) | ● open core (dual GPL-3.0/Apache-2.0 per-crate; editor + native agent + Zeta weights in-repo @ v1.19.2; hosted-model service closed; "Zed Cloud" nonexistent — SSH remote only) | ○ closed harness (JetBrains EULA; GitHub repo is distribution and issues only; Mellum model weights open) | ● [corrected from ◐] open Rust MIT+AGPL client (100% Rust verified @ 4143c09 2026-09-11; warpui/warpui_core MIT + everything else AGPL-3.0; open-sourced 2026-04-28 w/ OpenAI founding sponsor; server/Drive backend/Oz closed — repo is a mirror of private warp-internal via repo-sync.yml) | closed source (TS/Bun inferred; npm distro) | closed (TS/Bun client inferred; Go substrate fork; npm distro) |● All Rust (all 6 projects)| ● [enriched] Python dominant 77.3 MB (py3.11+uv) + TS 26.2 MB desktop/TUI + Rust shell bits |● TypeScript (both)|● Python (both; Apache-2.0)| **Rust** |
| Steady RSS | ~0.5–1 GiB | low (no GC) | heavy-ish | low | ~100–300 MB | ◐ [corrected from heavy] unpublished (no published figures; Node+Ink single process; UNVERIFIED) | unpublished (UNVERIFIED; Node heritage) | low | heavy | heavy | unpublished (Python/TUI) | unpublished (client/server; Docker+Postgres) | unpublished (single proc; native binaries) | unpublished (UNVERIFIED; native Rust/TUI) | ○ unpublished | ○ unpublished (cloud ephemeral envs; local sandbox on MS MXC) | ○ unpublished (Electron editor baseline; 2.0 changelog admits fixing memory leaks) | ○ unpublished ("near-zero, extremely lightweight" marketing only; no measurements) | ◐ [enriched] (Rust GPUI native stack; 10ms frame-budget hang profiler + weekly perf drumbeat; no official RSS figures) | ◐ (IntelliJ-engine heritage; published prefill and inference-optimization posts, no RSS figures) | ○ [corrected from ◐] unpublished (wgpu-30 GPU GUI + headless cell-grid TUI share one ECS core; no published figures — not measurable from source) | unpublished (UNVERIFIED) | unpublished (UNVERIFIED; thin client + cloud threads) |◐ Unmeasured; framework claims ~1GB peak (AutoAgents bench)| ◐ [enriched] Unmeasured; $5-VPS deployable + scale-to-zero serverless (Modal/Daytona hibernate, gateway/scale_to_zero.py) |○ Unpublished (extension host + CLI processes)|○ Unpublished (orchestrator + per-trial containers)| **<80 MB budget** |
| OS sandbox | ◐ (no Win) | ● (3 OS) | ○ | ◐ (mac+Docker) | ○ | ● [corrected from ◐] (5 backends: Seatbelt 6 profiles / Docker-Podman / Windows-native icacls+C# / gVisor / LXC + bwrap tool-level + Sandbox Expansion Requests; best-in-class breadth) | ● (macOS Seatbelt 6 profiles + custom .sb; Docker/Podman published image + SANDBOX_FLAGS; permissive-open default; Win/Linux = containers only) | ○ | ● Docker | ● Docker | ○ | ○ (git-branch sandbox only) | ○ (none built-in; 3 BYO container patterns) | ◐ (Landlock+Seatbelt; no Win; child-net Linux-only) | ● (every Devin in an isolated cloud VM: own terminal, browser, dev env; managed fleets give each child a clean context and own shell) | ● (ephemeral Actions envs + egress firewall by default; local sandbox MXC cross-OS incl Windows (2026-06 preview); cloud sandbox copilot --cloud) | ● (mac Seatbelt; Linux Landlock+seccomp k6.2+ w/ bwrap fallback; default-on w/ Auto-review classifier default 3.6; sandbox.json admin floor + protected paths + SSRF-guarded default-deny + deny-always-wins; Win undocumented; cloud = Firecracker microVMs in separate AWS account w/ 3-mode egress) | ● [corrected] (Terminal Sandbox preview: Linux namespaces + macOS sandbox-exec; permission-derived RW/RO mounts + read_url-grant network AllowedDomains; ~/.ssh/.env blocked; unsandboxed() escapes; opt-in; Windows UNDOCUMENTED; managed API agent = remote Ubuntu sandbox) | ● [corrected from ○] (Seatbelt/Bubblewrap/Bubblewrap-via-WSL, feature-flag gated; worktree roots auto-granted + .git protected; HostPattern network policy; unsandboxed:true = once-per-thread approval; Windows = WSL-dependent) | ◐ [corrected from ○] (Air Docker + Git-worktree isolation per task + cloud sandboxes tech preview; IDE Junie/CLI run unsandboxed on host) | ◐ [enriched] (isolation_platform Docker/DockerSandbox/K8s/Namespace for cloud + self-hosted workers [Enterprise self-hosts the sandbox as K8s Jobs; control plane stays Warp-managed]; local runs permission-gated ONLY — no seatbelt/apparmor/bwrap in client) | ● (Seatbelt+bwrap/seccomp + net-filter proxy; per-cmd or whole-proc; fail-closed; Win=WSL2 only) | ○ (explicit none local — "does not try to protect" vs injected Bash; isolation = cloud orbs) | ● Seatbelt/bwrap/WSL2 (NorviaLabs), Docker (kwaak); harness-rs 5 backends (Null / git Worktree / Container docker --network none / Seatbelt / bwrap) w/ honest Isolation::{None,Changes,Process} — FsPolicy/NetPolicy = requested vs actually-enforced, in-process-fs hole disclosed; adulari/forge: none found (LSP RSS guard only) | ● [corrected from ◐] 7 terminal backends (local/ssh/docker/singularity/modal/daytona/vercel_sandbox microVM), Docker --cap-drop ALL + no-new-privileges + pids-limit + tmpfs noexec | ○ [confirmed] None in either; Continue FINAL: terminal-security = shell-quote command-policy analysis only + .continueignore file guard; approval gates + checkpoints carry safety (Cline) |● Docker locally; Daytona/Modal/Runta etc. for cloud trials| ● 3-OS native + container A |
| Permission tiers | ● | ● | ● (doom_loop) | ● (4 modes) | ○ (human all) | ● [corrected from ◐] (policy-engine allow/deny + approval modes + trusted folders + read-only plan mode + confirmation-bus; fail-closed workspace trust) | ● (5 modes Plan/Ask/Auto-Edit/Auto/YOLO; Auto classifier default: deny-first, allow-stripping, fail-closed, loop-guard, NL hints; headless = Ask) | ● | ◐ | ○ | ◐ (Y/n/i hunks) | ◐ (5 presets x 10 toggles) | ◐ (Project Trust ask/remember; no tool perms) | ● (6 modes + deny/ask/allow + admin locks) |● [corrected from ◐] (Read/Write/Exec rule DSL w/ allow/ask/deny + specificity resolution; Security Profiles GA 2026-08-07: network/MCP/git/gh-cli restrictions w/ 5-level binding + intersection semantics (lower levels only tighten); AI Guardrails + Attribution Filtering)| ● (copilot/* branches only; human approval gates CI/CD; no self-approve/merge; co-authored commits; org policies + audit logs; gh-aw safe-outputs read-only default + threat detection) | ● (Auto-review default 3.6: allowlist → sandbox → classifier (Haiku/GPT-5.4-Mini) → escalate; NL permissions.json user+project merged, team floor; honest "not a security boundary" doc) | ● [enriched] (unified action(target) engine: Deny>Ask>Allow; read_file/write_file/read_url/execute_url/command/unsandboxed/mcp; prefix default + regex: opt-in; workspace auto-allow; prompt-card scope editing; Security Presets Default/Full-machine/Turbo/Custom; unconfigured MCP tools default Ask + mcp(server/tool) scoping; subagent permission bubbling + inheritance + commandExecutionPolicy off/auto/eager/sandbox; headless soft-deny) | ● [enriched] (tool_permissions: allow/confirm/deny global default + per-tool regex + precedence chain + un-overridable built-in rm-rf safety rules + chained-command parsing + UI safe-pattern extraction + profile-scoped modes + restricted-worktree auto-downgrade write/ask→minimal) | ● (three-state Brave mode off/auto-safety-classifier/on + Action Allowlist command patterns in allowlist.json + per-action approval + per-subagent permissionMode incl. bypassPermissions) | ● [corrected from ◐] (typed allow/deny WITH reasons: CommandExecutionPermission ExplicitlyAllowlisted/IsReadOnlyAndSettingEnabled/AgentDecided/RunToCompletion/ContainsRedaction + FileRead/FileWrite/AskUserQuestion/WriteToPty; server-synced AIExecutionProfiles + workspace/team AiAutonomySettings; denylist overrides everything) | ● (4 autonomy levels + allow/deny/BLOCKLIST w/ program-resolution; org clamp) | ◐ (approval-free default; plugin tool.call gates allow/reject/modify/synthesize; mcpPermissions first-match) | ● toml allow/deny w/ repo-grant rejection; harness-rs L1/L2/L3 autonomy ladder + ToolRisk + risk-gated shell + HumanGate; adulari broker: builtin deny floor outranks bypass, per-segment shell parsing (allow "git *" cannot smuggle git status && x), multi-line conservative literal-token floor, SideEffect::External gates MCP even in accept-edits, plan = hard read-only, decisions persisted | ● [corrected from ◐] 8 layers: smart(aux-LLM-scored)/manual/off + YOLO + hardline blocklist floor surviving YOLO (deobfuscated variants) + DM pairing + protected credential paths + fail-closed headless + admin/user scopes | ● [enriched] Cline: Plan/Act split + granular auto-approve toggles, approval-first default; Continue FINAL: CLI normal/plan/auto cyclable modes w/ absolute policies (plan excludes writes, auto = allow-all wildcard) + permissions.yaml precedence + terminal-security allowedWith/WithoutPermission + GUI per-tool policies + readonly plan filtering |◐ Agents run autonomous by design; harness grants the whole sandbox| ● + inspectors A |
| Prefix-cache discipline | ● strong | ◐ | ◐ | ◐ | ◐ | ● [corrected from ◐] (implicit+explicit token caching; cachedContentTokenCount in event-translator) | ● (fork subagents share parent's exact request prefix; fork_tools narrows execution w/o declaration change; btw on cache path; /stats cache-savings; DashScope 80%+ claim) | ◐ | ○ | ○ | ○ | ◐ (advertised, unmeasured) | ● (per-provider cache modules; one-off write suppression) | ◐ (prompt_cache_key fwd + cached-tok telemetry) | ● (Fusion: persistent cached contexts for main AND sidekick; model switches only at compaction points to avoid cache misses; 5-min expiry engineering) | ◐ (AI credits billed per tokens; no user-facing cache discipline) | ◐ (SDK cacheRead/cacheWrite tokens; Temporal CAS activity retry resumes w/o token replay; no user-facing assembly discipline) | ◐ [corrected] (implicit only: 50–70% of input tokens typically cached; cached tokens excluded from max_total_tokens; no user-facing cache controls) | ◐ [enriched] (Anthropic explicit cache_control/mark_last_cacheable_content; Google AI cached-token accounting only; NO unified cross-provider strategy) | ● [corrected from ◐] (engineered max reusable initial prefix + KV-cache reuse across Junie Local tasks in-session) | ○ (no cache_control plumbing client-side; inference = server-side SSE protobuf; bundled claude-api skill documents prefix caching for harnesses — server behavior UNVERIFIED) | ◐ (Router cache-aware claim; internals closed) | ◐ (server-managed routing; internals unpublished) | ◐ Cache-aware benchmarking + prompt caching (adulari); harness-rs Context prefix-cache module (live-tested, unmeasured) | ● [corrected from ○] prompt_caching.py: 4 cache_control breakpoints, 5m/1h TTL, Qwen detection, OpenRouter part-level relocation; background review + curator share prefix cache | ● [corrected from ○] Continue FINAL: opt-in promptCaching → cache_control ephemeral (Anthropic/Vertex), Bedrock cache points + PromptCachingMetrics, cacheBehavior cacheSystemMessage/cacheConversation, GUI cached + cache-write token tracking; Cline undocumented |○ N/A (batch evaluation, not interactive)| ● core A |
| Repo map (AST/PageRank) | ◐ | ○ | ◐ | ○ | ● (invented) | ○ [confirmed] (tree-sitter only parses shell commands; grep/@-refs/jit-context instead) | ○ (grep/Glob + LSP integration; no AST/PageRank map) | ○ | ○ | ○ | ◐ (ctags→removed→ragdaemon) | ● (tree-sitter maps; no PageRank) | ○ (AGENTS.md only) | ◐ (tree-sitter def/ref graph; no PageRank) | ◐ (DeepWiki repo indexing + architecture diagrams; Ask Devin context; no AST/PageRank map) | ◐ (tool-based repo research + GitHub MCP server for issues/PRs; no AST/PageRank map) | ◐ (embeddings codebase index + .cursorignore scoping + Explore fast-model parallel-search subagent; no AST/PageRank map) | ○ [corrected from ◐] (no AST/tree-sitter repo-map documented; tool-based understanding + /codesearch with undetailed semantics) | ◐ [verified] (grep-confirmed NO repo-map in-repo; tree-sitter outlines + LSP go-to-def/find-refs/diagnostics + grep + line-windowed buffer reads instead) | ● [corrected from ◐] (semantic index + PSI + analyze_calls call-hierarchy incl. decompiled JAR symbols + symbol-level @ mentions) | ◐ [enriched] (tree-sitter file outlines + full-source-code embeddings w/ Merkle-tree snapshot sync [crates/ai/src/index] + repo_metadata standing queries; cloud-synced, not an aider-style local AST/PageRank map) | ◐ (cross-repo guidance + cloud Knowledge index; no public AST map) | ◐ (Librarian remote search subagent; no AST repomap) | ● Lattice tree-sitter TAGS-QUERY graph (adulari; README "19 languages" UNVERIFIED — ADR-0010 names ten initial; refs stored by name not dst_id to survive reindex); duckdb index (kwaak) | ○ [confirmed] zero tree-sitter/repomap repo-wide (only skills AST security audit); FTS5 indexes sessions, not code | ◐ [enriched] Continue FINAL (frozen): LanceDB vector + SQLite FTS + CodeSnippets indexes + tree-sitter chunking + 35 context providers + LLM-generated RepoMap (not AST tags); Cline reads project structure |○ None (tasks are self-contained environments)| ● A (CSR+SIMD) |
| Windowed file view | ◐ | ◐ | ◐ | ○ | ◐ | ● [corrected from ◐] (read-file offset/limit + omissionPlaceholderDetector anti-elision guard) | ◐ (read truncation heritage + Multi-File Read tool; no AST windowing) | ◐ | ○ | ● (invented) | ◐ (interval includes) | ○ (whole files <=100k tok) | ◐ (read + truncate; no AST windowing) | ◐ (codex slice + hashline anchors) |○ [corrected from ◐] (VM file tools; no documented windowed file-view mechanism; VSCode view is human-facing)| ◐ (agent file tools; internals closed) | ◐ (tool-based file reads; 2.0 "simplified the agent to use the LSP less when reading files") | ◐ [enriched] (view_file/edit_file built-ins; no documented line-window/offset semantics; artifacts as review surface) | ● [enriched] (native editor buffers; line-windowed 1-indexed-inclusive reads sliced from rope/sum_tree buffers; follow-agent mode; inline per-hunk diff review) | ◐ [corrected from ●] (auto-context = current file + selection only; @-attach for more; no windowed file-view paging layer) | ● [corrected from ○] (agent file-content actions carry line_range start/end — crates/ai/src/agent/action; diff-review panels for edits) | ◐ (UNVERIFIED; internals closed) | ◐ (@-mentions truncate 500 lines/2KB per line) |◐ Patch-based editing (kwaak); hashline anchors (harness-rs cap)| ◐ [corrected from ○] read_file offset/limit paginated reads + read-tracking dedup; no map-style viewport | ◐ [enriched] Cline: per-edit diffs + IDE views; Continue FINAL: CLI Read = whole-file w/ truncation caps (100k chars/5k lines, no offset/limit windows); line windows only via Search snippets |○ None| ● A (≤100 lines) |
| PTY screen-delta scraping | ◐ prune | ◐ | ◐ | ○ | ○ | ● [corrected from ◐ masking] (@lydell/node-pty darwin/linux/win32 x64+arm64 + background shells) | ◐ (shell on real PTYs: node-pty x4 platform triples + ConPTY host + web-terminal registry; output captured by tool, not screen-scraped) | ○ | ○ | ○ | ○ (run-capture+GPT-4V) | ○ (capture + Chrome console) | ○ (bash executor; external tmux doc) | ● (ptyctl/alacritty_terminal emulation) |◐ [corrected from ○] (CLI persistent-PTY shell (pty_for_noninteractive_exec); cloud shell command history; no scrape-harness pattern documented)|● [corrected from ○] (pty x319 strings + native runtime.node in shipped CLI 1.0.83 bundle; background shells w/ completion signals; autopilot waits on background shells)| ◐ [corrected from ○] (CLI shell mode + before/afterShellExecution hooks + disk transcripts; scrape internals UNVERIFIED) | ◐ [corrected from ○] (commands stream stdout into TUI; /tasks stdout logs; no PTY-scrape architecture documented) | ● [corrected from ○] (alacritty_terminal emulation from own fork; host-owned headless Terminals w/ SandboxWrap; searchable terminal threads; checkpoint-restore kills terminals) | ◐ [corrected from ○] (reviewable terminal commands w/ approval widget + Open-in-editor pre-run; Air built-in terminal; no interactive PTY control) | ● [enriched] (best-in-class: blocks capture command+output as structured units AND the agent drives a real PTY; OSC 777 JSON notifications from in-agent plugins [OSC 9 Codex fallback] tracked across 10 harnesses — Claude/OpenCode/Codex/Gemini/Auggie/Droid/Pi/OhMyPi/Grok/WarpTui — w/ per-harness plugin marketplaces + session status machine w/ 2s Ctrl-C grace) | ◐ (Droid Control GUI automation; PTY internals unpublished) | ◐ (tmux-shared orb terminal + wmux; TUI internals unpublished) |○ None; shells executed directly, not scraped| ● [corrected from ○] pty=true + background=true interactive CLIs, process write/submit driving (local backend only) | ◐ [enriched] Cline watches terminal output incl. background dev servers; Continue FINAL: child_process.spawn only — no PTY (VS Code node-pty emulator disabled in-tree as causing-problems) |◐ Terminus reference agent drives the terminal; trajectories recorded| ● A |
| Tiered eviction | ● compaction | ◐ compact | ● summarize | ● | ○ | ● [enriched] staged ladder: toolOutputMasking → toolDistillation → chat compression → truncation; contextManager incremental/hotstart/barrier | ◐ (manual /compress code-verified + proactive tips at 50-80% and >=95%; bounded daemon structures (ring 8000, 512 FIFO, 20-slot queue); auto-compress UNVERIFIED) | ● summarize | ● condensers | ○ | ○ (grows-only) | ◐ (summarizer + smart-context) | ◐ (auto-compaction + branch summarization) | ● (prune→2-pass compact→segments→memory flush) |◐ [corrected from ●] (compaction load-bearing: Fusion switches models at compaction events + self-compaction trained into SWE models; no documented tiered eviction ladder)|◐ [corrected from ○] (2-threshold compaction: background at 0.80 / block at 0.95 + manual /compact + checkpoint summaries; no per-item tool-result ladder)| ◐ (preCompact hook + incremental checkpoint transcript writes; no published eviction ladder) | ○ [corrected from ◐] (no local auto-compaction/eviction documented; mitigations architectural: Teamwork fresh-successor handoff, /rewind//fork//clear, /goal; managed API agent compacts ~135k) | ◐ [corrected from ●] (single summarize-compaction: AutoCompactThreshold %|tokens-used|remaining + manual force + dedicated compaction model + New From Summary/@-mention threads; NO tiered ladder) | ◐ [corrected from ○] (selection-based compaction + helper-model summarization; local mode = append-only rolling context; no tiered ladder) | ◐ [corrected from ○] (summarization-based conversation compaction [SummarizationType + summarization_cancel_dialog]; context assembly server-side; no tiered/eviction store in client) | ◐ (compactionTokenLimit ± per-model + compactionModel; ladder unpublished) | ◐ (anti-compaction: Handoff distill + read_thread extraction; 200k doctrine) | ● harness-rs 5-stage ladder (BudgetReduce→Snip→Microcompact→ContextCollapse→AutoCompact) + ModelBackedCompactor + hysteresis + never-split-invariant + PostCompact{before,after} accountability; 3-layer memory; adulari compaction_policy/context_pipeline present-not-benchmarked (◐ internally) | ● [enriched] ContextCompressor: head-3/tail-20 protected + aux-model middle summarize, tool-output prune-first, stale-image eviction, micro+native compaction, pluggable engines | ◐ [enriched] Continue FINAL: single-tier auto-compaction (summarize at ≥80% context, 15k-token cap, prune-oldest input) + tool-output truncation caps — no ladder; Cline context condensing + truncation in docs |○ N/A (short per-trial horizons)| ● 3-tier A |
| Subagents | ● (+teams) | ● | ● | ◐ (Summon) | ○ | ● [enriched] (local profiles + 4 built-ins + agent-scheduler + remote A2A subagents) | ● (named + fork subagents w/ cache-prefix sharing; bg default w/ 20-slot priority-eviction queue + list_agents/send_message continuation; worktree pinning; /coordinate leader + 3 read-only workers) | ● child-sessions | ● delegate | ○ | ◐ (agent mode) | ◐ (pipeline roles; bg off) | ○ (tmux/extensions) | ● (spawn/resume/steer/personas; flat depth-1) | ● (managed-Devin fleet: coordinator scopes, delegates, monitors, reads child trajectories, messages children mid-task; Devin Review as separate agent) |● [corrected from ◐] (/fleet orchestrator: dependency-graph waves of parallel bg subagents w/ own context windows; built-ins explore/task/general-purpose/code-review/research/rubber-duck; custom agents as subagents; VS Code + JetBrains subagents GA; shared-fs last-writer-wins caveat)| ● [corrected from ◐] (built-ins Explore/Bash/Browser + custom .cursor/agents/*.md w/ model bracket params [effort=,context=]; exactly 1 nesting level; worktree/VM swarm isolation; /in-cloud + /autopilot; computer-use subagent shared VNC+Chrome; ID-resumable; legacy up-to-8-parallel-agents framing superseded) | ● [enriched] (invoke_subagent w/ inherit/branch-worktree/share + context isolation; built-ins research/browser/self; custom MD+YAML agents w/ per-subagent model tier + commandExecutionPolicy + mcpServers/skills; define_subagent; permission bubbling; /agents + Alt+J; /teamwork-preview Sentinel/Orchestrator/Explorers/Workers; /boost DeepCoder/DeepInvestigator) | ● [corrected from ◐] (SpawnAgentTool parallel-delegation contract w/ disjoint write sets + MAX_SUBAGENT_DEPTH=1 + CreateThreadTool sibling threads in linked git worktrees w/ ref pinning + persisted subagent tree + telemetry) | ● (YAML subagent files with tool allowlists, per-subagent model/effort/maxTurns, MCP-server scoping; SameModelOnly vs Auto tiering; imports .claude/.cursor/.codex agent files) | ● [enriched] (orchestration tree: orchestrator + named agents, parent→children topology w/ CREDIT ROLLUP over child agents; prepare_local_oz_child_launch w/ parent_run_id; RunAgentsRequest/SubagentType; run-to-run messaging inbox; Factories role-teams triage/spec/implement/review/verify) | ● (Custom Droids + worker/explorer; complexity-tier routing; flat d-1; bg+resume) | ● (Oracle/Librarian/search/read; custom agent modes; agent-to-agent msgs) | ● Worktree-isolated children (NorviaLabs); adulari subagents + fleet + /duel arena + forge-workflow + detached; harness-rs Job-DAG orchestrator + PlanDelta::Add dynamic replanning BUT no parallel spawn (cooperative !Send); actor teams (AutoAgents) | ● [enriched] delegate_tool family (10 modules: registry/dispatch/worktrees/progress/live-steer) + /bg sessions + code-exec RPC; subagents steered live mid-flight | ● [enriched] Cline multi-agent teams (coordinator → specialists, persistent state); Continue FINAL: subagent tool + executor + dedicated subagent model role = per-agent model routing |○ None (parallel trials, not subagents)| ● fork-join A |
| Multi-model spec launch | ◐ | ○ | ○ | ◐ recipes | ◐ arch/editor | ● [corrected from ○] (modelRouterService + fallback + model steering + per-agent model; Gemini-family only) | ● (/arena up to 5 cross-provider agents in per-model worktrees; per-subagent model: authType:modelId routing; /model hot-switch; 5-protocol modelProviders catalog) | ○ | ○ | ○ | ◐ (spice, 1/session) | ● (9 roles + fallback chains) | ● (pi-ai unified + cross-provider handoffs) | ◐ (per-role/type models; no chains) | ● (Fusion: frontier main + cheap sidekick + dynamic mid-session routing; multi-model by architecture) | ● (model fleet picker incl OpenAI/Anthropic/Google + auto; Agent HQ third-party engines Claude/Codex/Gemini in-subscription) | ● (per-turn pick + per-subagent model w/ bracket params + Router Auto Cost/Balance/Intelligence incl flat-price Auto Cost; parallel cloud agents; no single spec→N launch primitive) | ● [updated] (Gemini 3.8/3.7/3.6 Flash + 3.1 Pro + Claude Sonnet/Opus 4.6 thinking + GPT-OSS-120b in one selector, sticky-per-turn; Claude/GPT absent on Enterprise; subagent model= inherit|flash|pro only; managed agent model override via agent_config) | ● [enriched] (broadest provider matrix: Zed-hosted, Anthropic, OpenAI, Google, Bedrock, OpenRouter, Ollama local + any ACP external agent; per-role models default/subagent/inline/commit-message/thread-summary/compaction + refusal-fallback + mid-turn hot-switch) | ● (BYOK 6 providers + custom endpoints LiteLLM/Ollama/LM Studio + subscription + on-device Local; same-week support for new frontier models) | ● [enriched] (HarnessModelConfig/HarnessConfig spawn w/ model+harness per child; Claude/Codex/Gemini harness runners + own TerminalDriver; custom routers + BYOK + custom endpoints + SuperGrok subscription) | ● (per-role orch/worker/validator + modelFallbacks map + Router) | ● (Dial per-mode bundles; per-role pinning; BYOK + ChatGPT-sub link; silent swaps) | ● Mesh 8-step deterministic zero-LLM-call routing: lexical tier classify → conservation probability (complex+code_heavy = 0.0 off subscriptions while healthy) → QuotaPace burn windows → score (AA benchmarks + quota/burn/capacity penalties) → rank-verbatim failover chain + 75s RL wait-out (adulari); per-task models (kwaak) | ● [enriched] model-agnostic: native adapters (Anthropic/Bedrock/Vertex/Gemini/Codex/OpenRouter/Moonshot/LM Studio/Nous Portal 300+ models) + fallback chains + credential pools + /model hot-swap + 11 aux slots | ● [enriched] Continue FINAL: 79 provider files + ~30 streaming adapters + 8 model roles (incl. subagent) + per-mode/per-profile models; Cline all major providers + gateways + local runtimes |● Any agent via adapters; leaderboard runs open + closed models| ● heterogeneous A |
| Model-weakness profiles | ○ | ○ | ○ | ○ | ○ | ◐ [corrected from ○] (per-agent + generation-settings overrides; no user-facing named profiles) | ○ (per-model generationConfig knobs only; no weakness catalog) | ○ | ○ | ○ | ○ | ◐ (role-level + edit race) | ◐ (thinking levels; per-model compaction budgets) | ◐ (per-model ctx/effort/stream knobs) | ● (the postmortem genre: senior-at-understanding/junior-at-execution profile; per-model behavior notes like context anxiety; per-strength model selection) | ○ (no published per-model weakness catalog; engine routing internal) | ● (Router learns per-taxonomy model strengths: Grok=routine, Sol=planning, Opus=execution, Fable=debug/visual — server-side, non-portable) | ◐ [corrected from ○] (per-model thinking levels low/med/high + sticky selection + plan-tier gating + split Gemini-vs-Claude/GPT quota pools; no weakness catalog) | ◐ (runs behavior-diff evals on every model release with internal datasets; no published weakness catalog) | ● [corrected from ◐] (per-alias pinning via internal trajectory evals; documented model rejects; behavioral model-profiles research program) | ● [corrected from ○] (server-synced AIExecutionProfilesModel + warp agent profile list; profiles drive BOTH permissions and model selection) | ◐ (per-model compaction/effort catalog; no behavioral axes) | ◐ (capability bundles per mode; "grain of the model" doctrine; no published weakness catalog) | ● Benchmark-ranked tiers (AA index/20) + /duel outcomes + quota-status/pool-capacity/subscription-burn penalties feed routing; every penalty constant cites its incident (adulari) | ◐ [enriched] agent profiles (own HERMES_HOME/models/memories/skills) + per-channel overrides + 11 aux model slots — no model-weakness catalog |○ None|○ None| ● A (novel) |
| Skills/packs | ● standard | ● | ● | ◐ | ○ | ● [enriched] (SKILL.md loader/manager + activate-skill + consent flow + skill-extraction-agent) | ● (SKILL.md personal/project/extension scopes + paths: glob gating + priority; extensions bundle commands/agents/skills/MCP; /learn + /curator lifecycle) | ○ | ○ | ○ | ○ | ○ (model packs not skills) | ● (Agent Skills std; pi authors skills) | ● (SKILL.md std + 4-vendor discovery + marketplace) | ● (Playbooks + Knowledge + setup scripts extracted as repo testing skills via one-click PRs) | ● (AGENTS.md + .github custom instructions + hooks + custom agents) | ● (SKILL.md 4 discovery roots + .claude/.codex compat + nested monorepo scoping + marketplace + 10 built-ins incl /create-skill /migrate-to-skills; rules 4 types Team>Project>User; Custom Modes removed 2.1 → reintroduced as pinned skills 2026-08-19) | ● [enriched] (SKILL.md open std + progressive disclosure; resolution workspace .agents/skills/ + global ~/.gemini/config/skills/ + plugin + builtins; workflows deprecated → skills; /learn distills feedback into Rules/Skills; Rules AGENTS.md/GEMINI.md + Plugins + Hooks + Sidecars) | ● [corrected content] (crates/agent_skills: SKILL.md std; global ~/.agents/skills + per-project scopes; size caps 100KB file/50KB descriptions + IO-16; disable_model_invocation flag; builtin create-skill guided authoring; @-mentionable + model-invoked) | ● (agent skills + custom slash commands + guidelines cascade: .junie/AGENTS.md, AGENTS.md + playbook + rules, legacy guidelines.md; global vs project with dedup) | ● [enriched] (.agents/skills project+global + skills-lock.json via npx skills + hot-reload file watchers + bundled skills [incl. claude-api] + Drive-remote skills + skill_dirs_publish to harnesses; rules WARP.md/AGENTS.md + CLAUDE.md migration) | ● (SKILL.md std + manager + builtins-off + marketplace) | ● (SKILL.md std + Claude-dir discovery + git-managed personal/workspace skills + building-skills) | ● adulari forge-skills: Claude-Code-format compatible, frontmatter tier routing; harness-rs agentskills.io std: compile-time #[skill] validation + skills lint (Jaccard >50% overlap flag) + export consumable by Claude Code/Cursor + superpowers compat | ● [enriched] Skills Hub (agentskills.io) + 5 install sources incl. GitHub taps + ClawHub adapter; content-hash scan + quarantine + lock.json (ClawHavoc lesson); bundled/project/local precedence | ● [enriched] Cline .clinerules + loadable skills; Continue FINAL: SKILL.md dirs (project + ~/.continue + .claude/skills compat) + Skills tool + auto slash-commands; hub blocks dead with the hub |○ None| ● agentskills-compatible A |
| Auto skill learning | ◐ memory | ◐ memories | ◐ (plugin) | ◐ chatrecall | ○ | ◐ [corrected from ○] (experimental Auto Memory + skill-extraction-agent + save_memory) | ● (/learn synthesizes skills from URLs/dirs/videos; auto-skill generation + /curator stale(30d)/archive(90d)/pin; auto-memory extraction + /dream consolidation) | ○ | ○ | ○ | ○ | ○ | ◐ (agent-authored hot-reload extensions) | ◐ (/create-skill + reminders; no hot-reload) | ● (Session Insights mines completed sessions into improved prompts; agent proposes saving discovered setup as skills) | ◐ (Copilot Memory public preview: repo-scoped self-learned facts; no skill synthesis) | ◐ [corrected: Memories REMOVED v2.1 2025-11-21 silently] (/create-skill + /migrate-to-skills + stop-hook skill-update pattern; no autonomous distillation) | ● [corrected from ◐] (/learn distills session feedback into persistent Rules or Skills, all plans; legacy Knowledge Items on disk UNVERIFIED; de-facto memory = Rules+Skills+/learn+resume/fork) | ◐ [corrected from ○] (builtin create-skill = assistant-assisted guided skill authoring on request; no autonomous skill-learn loop) | ◐ [corrected from ○] (internal memory-extraction helper task + next-task prediction; cross-session auto-memory persistence UNVERIFIED) | ◐ [corrected from ○] (no autonomous skill-writing loop in client; adjacent: suggested-rule modal auto-proposes rules + Agent Memory versioned stores, cross-harness preview) | ◐ (/create-skill; missions author skills; no hot-reload evidence) | ◐ (building-skills authoring; no auto-loop) | ● harness-rs CLOSED learning loop: episodes (situation→tool trace→outcome) → SkillDistiller promotes proven episode to routable SKILL.md (~100 catalogue tokens) → SkillUseTrace triggers SkillReviser fix drafts — auditable self-evolution in lintable files; adulari /duel outcomes feed mesh | ● [enriched] 5 mechanisms @ HEAD: skill_manage writes + post-turn background_review fork + in-use improvement + idle curator + 10/15-turn nudges; opt-in write_approval human gate; knowledge-level not weight-level | ○ None (Continue FINAL: session-scoped recentlyEdited/Visited autocomplete context + nextEdit document history only — no cross-session learning) |○ None| ● Skill Forge A |
| MCP client | ● | ● | ● | ● | ○ (refused) | ● [enriched] (full OAuth: PKCE + Google + SA impersonation; OS-keychain + AES-256-GCM file fallback; resource tools + filtering) | ● (stdio/SSE/streamable-HTTP + OAuth w/ opt-in encrypted token store; workspace transport pool keyed by config fingerprint + hysteresis budgets off/warn/enforce) | ● | ◐ | ○ | ○ (pre-MCP) | ○ (zero in code) | ○ (explicit No; mcporter/ext) | ● (4 transports + OAuth + elicitation) |● [corrected from ◐] (stdio/SSE/HTTP(Streamable); marketplace + custom servers; OAuth org-shared/personal; enterprise CA bundles + private-network MCP; scope-gated admin management)| ● (GitHub + Playwright servers default; repo JSON config; MCP tools can assign tasks) | ● (stdio/SSE/Streamable-HTTP + OAuth + enterprise allowlists; cloud split: HTTP proxied, creds never in VM, stdio in-VM; MCPoison CVE-2025-54136 fixed 1.3 w/ full-entry-hash trust) | ● [enriched] (full client stdio+SSE/HTTP/websocket; MCP Store ~47 curated servers; global ~/.gemini/config/ + workspace .agents/ mcp_config.json; OAuth DCR + ADC + headers; disabledTools[]; unconfigured tools Ask-default; per-subagent servers; CLI /mcp manager; config shared 2.0/IDE/CLI) | ● [enriched] (context_server crate: stdio/HTTP + OAuth + protocol-version negotiation + per-thread registry; profile-scoped MCP tools + mcp:<server>:<tool> permission keys + MCP forwarding over ACP) | ● [corrected] (4 provider classes Junie/BYOK/Custom-Proxy; Claude-Desktop config import; one-click popular servers; router-only mode; MCP brave-mode) | ● (MCP servers first-class with agent-decides / allowlist / denylist per server; mcpServers in factory.yaml) | ● (OAuth + per-srv timeout + enterprise policy) | ● (Streamable HTTP + OAuth/bearer; deferred in-skill loading; Resources/Prompts unsupported) | ● 5 of 6; adulari forge-mcp rmcp client both directions + manager-owned (no registry mutation) + deferred tool loading + External-class gating; harness-rs mcp client + stdio/HTTP proxy; filtered tools (kwaak), toolboxes (swiftide) | ● [enriched] mcp==2.0.0 (rev 2026-07-28) + 20+ tool modules (OAuth device flow, sampling, schema cache) + MCP command center + curated optional-mcps catalog | ● [enriched] Cline cline mcp; Continue FINAL: MCPManagerSingleton + OAuth + stdio/SSE/streamable-http + MCP-as-context provider + config.yaml mcpServers w/ connectionTimeout |○ None (adapters drive agent CLIs directly)| ● A |
| MCP server mode | ◐ | ● | ◐ | ◐ ACP | ○ | ◐ [corrected from ○] (not MCP; server surfaces = A2A a2a-server + ACP + vscode-ide-companion MCP) | ○ (ACP daemon + SDK JSON-RPC are not MCP) | ○ | ○ | ○ | ○ | ○ | ○ | ○ |● [corrected from ○] (official https://mcp.devin.ai/mcp Streamable HTTP: session create/search/interact/events/gather (gather = fan-in barrier) + playbooks/knowledge/schedules/integrations/wiki tools; cog_ keys + X-Org-Id; plus free DeepWiki MCP)| ○ (not evidenced) | ○ (CLI/SDK + cloud handoff; no MCP-server mode evidenced) | ○ [corrected: no MCP-server mode; community a2a-antigravity wrapper fills the interop gap] | ◐ [corrected from ○] (in-tree McpServer: Unix-socket tool server + WIT context-server.wit for wasm extensions — zero production callers found, wiring UNVERIFIED; ACP remains the interop protocol) | ○ (not evidenced) | ◐ (closed Automation Platform exposes runs via CLI/REST/SDK + Factory MCP docs; OSS client shows NO Warp-as-MCP-server surface — UNVERIFIED beyond docs) | ○ (SDK JSON-RPC is not MCP) | ○ (SDK/API only; no MCP-server mode evidenced) | ● adulari forge mcp-serve (rmcp server side); harness-rs harness mcp serve + mcp/ server crate + proxy; resources + prompts served | ● [enriched] mcp_serve.py (31 KB) + transports server exposes agent own tools as an MCP server | ○ None (Continue FINAL confirmed client-only — cn serve exposes a custom Express API, not MCP) |○ None| ● A |
| WASM plugins | ○ | ○ | ○ | ○ | ○ | ○ [confirmed] (incidental web-tree-sitter grammar WASM only) | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ● [enriched] (Wasmtime 48 + wasi 48; 10-version WIT ladder since_v0.0.1→v0.8.0 (77 .wit files incl. lsp/dap/context-server); declared capabilities process:exec/download-file/npm:install, default-deny wasm sandbox — the one surveyed harness with WASM as native plugin substrate) | ○ (IntelliJ plugin ecosystem is JVM-based, not WASM) | ● [corrected from ○] (cfg(target_family="wasm") throughout, serve-wasm + warp_js + web event bus/websocket crates; web-compiled Warp terminals at build.warp.dev — WASM as a real frontend target, not a plugin system) | ○ | ○ |○ None| ○ [confirmed] no WASM runtime in tree | ● [corrected from ○] (Continue: web-tree-sitter WASM grammars bundled + used across core/autocomplete/chunking/nextEdit + offline transformers.js embeddings — WASM as shipped substrate, no plugin API; Cline: SDK/TS lifecycle hooks instead) |○ None| ● late-wave A |
| Sessions | JSONL | rollouts | SQLite | SQLite (sqlx v16) | git | ● [enriched] (resume + session summaries + chat recording + Rewind/replay) | ● (daemon load/resume/recap/btw + paged transcripts + fail-closed writer leases + PID-keyed cross-session registry w/ boot-id proofs; /resume) | SQLite+sqlc | event-sourced V1 | .traj | ◐ (/save /load + history) | ● (plan-git server-side) | ● (JSONL tree id/parentId; in-place branch; HTML) | ● (JSONL + SQLite FTS5 + foreign import) | ● (per-child session links, readable trajectories, Session Insights, Slack/Linear/IDE/API surfaces) | ● (issue/PR-backed sessions + session logs + agents panel; cross-device pickup via cloud sandbox) | ● (SQLite SqliteLocalAgentStore/JSONL per-workspace + resume-by-ID + --continue; cloud indefinite history + bc- IDs + signal-with-start follow-ups; format closed) | ● [enriched] (persistent conversations + /resume /fork /rewind /clear + archival + Projects scoping + cross-surface continuity incl. CLI↔2.0 export + settings sync + SDK persistence save/restore) | ● [enriched] (SQLite threads.db via global ThreadStore; subagent sessions persisted; thread history across projects, restore, archive, auto-titles, thread import from external agents, threads as Markdown) | ● [corrected] (async remote sessions resumable from any signed-in device; Air workspaces bind sessions/Git/tools per project w/ parallel tasks + attention notifications; /commands + CLI interactive sessions) | ● [enriched] (SQLite diesel local + Drive cloud sync + session sharing/join links + cross-machine harness resume via server-side transcripts [Claude/Codex ResumePayload]; prompt queueing, Agent Management Panel) | ● (local + cloud mirror default-ON + FTS + daemon + fork/tree; format closed) | ● (cloud threads: URL-addressable, cross-client, fork, labels/map, export MD/JSON; storage closed) | ● SQLite durable journals (NorviaLabs); adulari 30-store bundled WAL SQLite (5 core entities, single-writer, spawn_blocking) + sync-journal pruning + snapshot/fork/handoff stores + ADR-0014 growth bounds; harness-rs JSONL record/replay → MockModel deterministic + orchestrator persists every transition + resume() | ● [enriched] SQLite WAL SessionDB (~20 modules) + FTS5 CJK-bigram search + LLM-summarized cross-session recall + at-least-once delivery ledger + restart-surviving /model overrides | ● [enriched] Cline task history + persistent team state + messenger threads as sessions; Continue FINAL: CLI ~/.continue/sessions JSON + sessions index + cn ls resume + IDE newSession/share/history — remote sessions removed at freeze |● Every trial persists result.json + full trajectory logs| ● SQLite+JSONL+crypto+lazy A |
| Encryption at rest | ○ | ○ | ○ | ○ | ○ | ● [corrected from ○] (OS keychain + AES-256-GCM FileKeychain fallback) | ◐ (plaintext default for MCP OAuth tokens; opt-in keychain/AES-256-GCM via QWEN_CODE_FORCE_ENCRYPTED_FILE_STORAGE=true -- credentials only, not transcripts) | ○ | ○ | ○ | ○ | ○ (RAM-only keys though) | ○ | ○ |● [corrected from ◐] (encrypted transit+at-rest by default; SOC 2 Type II; Enterprise Assured Customer Managed Keys (AWS KMS); ZDR opt-out)| ◐ (GitHub enterprise security posture; no client-side keys evidenced) | ● [corrected from ◐] (AES-256 per-agent keys + KMS secrets + CMEK/BYOK + encrypted VM snapshots; client-side keys N/A — server-side per-agent keying) | ◐ [corrected] (transcripts safeStorage-encrypted .pb — Keychain/DPAPI/Secret Service, hardware-bound key [3rd-party RE]; BUT MCP OAuth tokens plain JSON per official docs; no client-side key management) | ○ [corrected from ◐] (at-rest: plain SQLite threads.db, no SQLCipher/PRAGMA-encrypt hits; data posture = opt-in-only training data; hosted-model service closed) | ◐ (SOC 2 certification claimed; no client-side encryption documented) | ◐ [enriched] (managed secrets envelope-encrypted [UploadKey/init_envelope] + secrets in OS keychain; local SQLite NOT encrypted — no SQLCipher; server ZDR = contracted claim, code UNVERIFIED) | ○ (none evidenced; enterprise ZDR claim only) | ◐ (AES-256 server volumes + encrypted orb FS; TLS 1.2+; no client-side keys) | ◐ OS keyrings; adulari secrets NEVER in TOML (env→keyring) + E2EE remote-control protocol (chacha20poly1305/ed25519); harness-rs PII redaction engine (regex+Luhn, Label/Mask/Hash/Block) w/ Guarded/Redacting memory decorators; no session-encryption at rest anywhere | ◐ [corrected from ○] Fernet-encrypted local vault (browser secrets/TOTP) + 1Password/Bitwarden backends; state.db + transcripts NOT encrypted | ○ [confirmed] Continue FINAL: plaintext apiKey/api_key first-class in YAML schemas, zero cipher/keytar hits (only OpenAI encrypted_content passthrough); Cline undocumented |○ Undocumented| ● A (novel-ish) |
| Checkpoints/rewind | ● v2 | ○ | ● revert | ◐ | ● git | ● [enriched] (shadow-git commit per tool call + /restore via GitService) | ● (shadow-git snapshots + /restore conversation+tool-call re-propose (default-off); daemon worktree session restore w/ integrity gates) | ◐ | ◐ | ○ | ◐ (in-mem undo/redo) | ● (all-state VC + auto-revert) | ◐ (session-tree branching; no file VC) | ◐ (workspace RewindPoints in-code; UI rewind convo-only) | ◐ (scheduled messages as coordinator checkpoints; no file-level rewind evidence) |● [corrected from ◐] (/rewind + double-Esc timeline picker to any turn; file-only restore WITHOUT git; conversation-only vs +files modes; survives compaction via checkpoint summaries; VS Code agent-mode checkpoints; SDK checkpoints/*.json)| ● [corrected from ◐] (IDE local non-git checkpoints; CLI /rewind default-on files+conversation + /undo//restore; subagent checkpoint-resume; VM snapshot fork pipelines; forum integrity complaints) | ◐ [corrected] (CLI /rewind//undo checkpoint picker + /fork branching — conversation-level; fs-side-effect reconciliation underspecified; IDE Review Changes editor for edit-level accept/reject) | ● [enriched] (GitStoreCheckpoint per user message, host-side in acp_thread — uniform for native + external ACP agents; auto-updates on repo change; restore kills terminals; available even mid-interrupt) | ◐ (conversation branching; per-edit restore checkpoints not documented) | ● [corrected from ◐] (periodic workspace CheckpointCoordinator: 5-state machine, safe-boundary gating, HarnessSupportClient upload + end-of-run snapshots + file-rewind dialogs) | ◐ (/rewind-conversation + /fork + worktrees; no file-level rewind) | ◐ (Edit & Restore msg time-travel; orb snapshots/suspend; no file-level local rewind) | ● Worktree isolation + auto-commit per turn; adulari snapshot/replay/fork + bridge --resume + forge blame (line→model/prompt/reasoning); harness-rs resume() resets stuck jobs + replay/fork | ● [corrected from ◐] shadow-git checkpoint store: pre-mutation snapshots per turn + /rollback + cross-project blob dedupe + agent-write ledgers | ● [enriched] Cline checkpoints over all changes, undoable; Continue FINAL: delegated to external git-ai checkpoint hook + IDE diff accept/reject only — no native snapshot system |◐ Trials replayable from logs; no interactive rewind| ● refs-namespace A |
| Cost HUD/budgets | ● /cost | ◐ | ◐ | ◐ tokens | ◐ | ◐ [enriched] (/stats quota snapshot + token footer + exit summary; no $ cost HUD) | ◐ (/stats cache-savings display + statusline context presets + MCP budgets off/warn/enforce; no monetary-cost HUD or token caps) | ● stats | ● budget | ◐ | ● (TUI sidebar) | ◐ (usage cmd; budgets cloud) | ● (token+cost tracking; no hard caps) | ● (/usage=/cost; goal --budget; no hard caps) | ● (per-child ACU consumption monitoring; ACU-based pricing) |● [corrected from ◐] (/usage + statusline budget details + additional-spend budgets w/ limit enforcement; AI Credits 1=0.01 USD token-metered + premium-request multipliers per model; user-level org budgets GA 2026-06)| ◐ (SDK TokenUsage incl cache tokens + Agent.getUsage() dollar cost + Token Rate $0.25/M third-party; no real-time HUD) | ● [enriched] (View Usage HUD w/ weekly+5h windows split per provider pool; /usage + /credits + statusline; API max_total_tokens budget → status:incomplete + usage.total_tokens) | ◐ [corrected from ●] (live thread.cost() label w/ currency + Cost panel; NO budget caps or limit enforcement found) | ● (/usage breakdown + live session cost in toolbar + dollar-denominated credits with top-ups) | ● [enriched] (credit/dollar usage model w/ RequestLimitInfo refresh + denial reasons + bonus grants + child-agent credit rollup + credits-vs-dollars unit setting; hard budget caps = team quota server-side) | ◐ (/cost /limits /stats; credit multipliers; NO hard caps) | ● (showCosts default-on; amp usage --details; per-thread usage; no hard caps) | ● Daily/weekly/monthly caps, auto-downshift, strict credit mode + quota as first-class currency (QuotaPace projection, pool capacity, burn-rate weights, conservation windows) (adulari) | ● [corrected from ◐] /usage + /insights + per-model analytics cards + desktop fleet cost overlay + cache-reset cost warnings | ◐ [enriched] Cline per-request token/cost display; Continue FINAL: GUI TotalUsage HUD (prompt/generated/cached/cache-write tokens + totalCost breakdown) + CLI per-session cost + OTel costCounter — no caps in either |◐ Per-trial token/cost accounting in results| ● HUD + hard caps A |
| Frontends | CLI/IDE/web | CLI/IDE | CLI/TUI/IDE | CLI/desktop/API | CLI | ● [corrected] (TUI + headless -p JSON + ACP + VS Code companion + Code Assist surfaces + SEA single-binary + voice) | ● (CLI/TUI + headless + ACP + serve/Web Shell w/ LAN-QR mobile + Desktop Tauri (Electron removed 2026-08) + VS Code/Zed/JetBrains + Chrome ext + 10 channel types + SDKs TS/Py/Java; broadest in survey) | TUI+serve | web | lib | ◐ (TUI/VSCode/SDK) | ◐ (CLI/TUI; VSCode unshipped) | ◐ (CLI/TUI+RPC+SDK+HTML; pi-chat) | ● (TUI/headless/ACP stdio+WS/relay; no web) | ● (web app + IDE + Slack + Linear + API + DeepWiki; broadest async surface) | ● (github.com + Mobile + VS Code + CLI + Slack/Teams/Jira/Linear/Azure Boards/Raycast + MCP tools; broadest enterprise surface) | ● (editor + agent CLI + web + mobile + Slack/PR monitors + Automations + Bugbot review) | ● [enriched] (2.0 desktop Win/mac/Linux + Web + VS Code-fork IDE + agy TUI/headless + extensions VS Code/VS/JetBrains/Zed/Xcode + browser surface + Remote Control + Python SDK + Gemini API managed agent) | ● [enriched] (desktop editor macOS/Linux/Windows; agent panel + threads sidebar + terminal threads; ACP host; headless eval-cli + SSH remote_server proxy; no first-party web/mobile — Delta app external) | ● (terminal CLI + all JetBrains IDEs via ACP + web app via /remote + GitHub Actions/GitLab CI + headless) | ● [enriched] (GPU GUI [wgpu] + headless TUI + web/WASM [build.warp.dev] + Agent CLI in any terminal over SSH + cloud platform + Slack/Linear/GitHub surfaces + REST API/SDK — one shared core) | ● (TUI + headless + Desktop + web + IDE + Slack + SDK) | ● (CLI/TUI, web, macOS, iOS, Slack/Puck, SDK; editor ext killed 2026-02) | ● adulari TUI (ratatui + headless via presenter seam) + CLI + desktop + Expo mobile + forge-proxy OpenAI-compat API + E2EE relay/device; harness-rs CLI REPL + harness-serve HTTP/gRPC + daemon + scheduler | ● [enriched] CLI + full TUI + native Desktop (Bot Mode, in-app browser, MCP command center) + gateway to 28 platforms + community WebUI 18.3k★ / workspace 6.6k★ | ● [enriched] Cline: VS Code + JetBrains (closed client) + CLI + desktop + SDK; Continue FINAL: VS Code ext (4.1M installs, orphaned) + JetBrains (community-demoted at freeze) + cn Ink-v6 TUI + shared React webview + hidden serve headless mode |◐ CLI + tbench.ai leaderboard; no editor UI| ● CLI+TUI+Web+VSCode A |
| Multi-client attach | ○ | ○ | ◐ | ○ | ○ | ● [corrected from ○] (a2a-server HTTP + SDK GeminiCliAgent + ACP) | ● (daemon qwen serve multi-client shared agent over HTTP+SSE/ACP + IM channels) | ● (2 TUIs) | ○ | ○ | ◐ (stream channels) | ◐ (connect re-attach) | ● (RPC stdin/stdout + server/client/protocol) | ● (leader proc + WS reconnect + dashboard) |● [corrected from ◐] (Slack/Teams + GitHub/GitLab/Linear/Jira/Pylon + API v3 + Terraform + MCP server + webhooks + email; live session monitor/takeover via session tools; npx devin-review)|● [corrected from ◐] (ACP localhost server (Zed; subagent IDs, plan/mode updates, session load) + SDK stdio/TCP/URI in 6 languages; app/CLI/IDE/cloud share one harness + session-state; background delegation & from CLI to cloud agent)| ● [corrected from ○] (same cloud agent concurrently from IDE/web/mobile/Slack/GitHub/API; resume anywhere via bc- ID) | ◐ (Remote Control remote access; sessions cross-surface; no live shared attach evidence) | ◐ [enriched] (ACP host multiplexes native + external agents in one panel; thread import over ACP + Markdown export; Delta multiplayer app external Aug 2026; no live multi-client attach verified) | ● (/remote shares the live session to the web app; continue from another device) | ● [enriched] (cloud-synced objects/sessions across GUI/TUI/web/CLI + shared_session join links + remote control of third-party CLI agents + websocket/web_event_bus) | ● (daemon + cloud sync across CLI/Desktop/web/mobile) | ● (remote-control cross-client + multiplayer + runners; strongest in survey) |◐ Same sessions across surfaces (adulari); daemon scheduler (harness-rs)| ● [enriched] one gateway serves 28 platforms concurrently + per-channel model/prompt overrides + multi-profile gateways + A2A inbound + hermes peer bot-to-bot DMs | ● [enriched] Cline Telegram/Slack/Discord/GChat/WhatsApp/Linear thread bridges; Continue FINAL: 3 frontends share one core but NO live cross-client session state (hub sync removed) |○ N/A| ● any-frontend A |
| Editor telemetry | ◐ | ◐ | ◐ | ○ | ○ | ● [corrected from ○] (OpenTelemetry metrics/logs/traces + enterprise GCA telemetry) | ◐ (OTel logs/metrics/spans + qwen-code.* namespace + Aliyun/local export (verified); opt-in-vs-default state UNVERIFIED) | ○ | ○ | ○ | ○ (Sentry ON) | ○ (none at all) | ○ (NOOP contracts pkg; opt-in) | ◐ (OTel double-opt-in; internal default UNVERIFIED) | ◐ (Session Insights + enterprise metrics; content defaults closed) |● [corrected from ◐] (first-class OpenTelemetry GenAI: chat/tool spans, cache/reasoning token attrs, compaction flags, time-to-first-chunk; managed-settings OTel export + mTLS/private-CA OTLP; hooks receive traceparent; enterprise audit logs actor_is_agent + agent_session.task 180d)| ◐ (Privacy Mode ZDR routing default-off on individual plans ~30d provider retention; enterprise OTel export + Analytics APIs; defaults UNVERIFIED, closed) | ○ [corrected from ◐] (no public telemetry docs; only implicit forum mention; enterprise controls UNVERIFIED) | ◐ [enriched] (opt-in-only training data; public anonymized Agent Metrics page 846.4K sessions/7.1M turns per 30d @ 2026-08-03; first-class in-repo telemetry::event! incl. compaction + subagent outcomes) | ◐ (feedback commands + JetBrains AI Terms; Local variant keeps everything on-device) | ◐ [enriched] (Sentry + Rudderstack w/ exhaustive event-name table + X-Warp-Telemetry-Enabled header; post-OSS Network Log = in-app 50-item buffer, Sentry traffic NOT captured; ASYMMETRY: telemetry MANDATORY for Free-plan AI, opt-out paid+, ZDR enterprise; no training on repo contributions) | ◐ (OTLP metrics default-ON; content opt-in; opt-out documented) | ◐ (server-side usage tracking; no client OTel evidence) |○ None are editors| ◐ [corrected from ○] OTLP exporter + health/monitoring emitter, pseudonymous rotatable install-id + redaction module; no forced product analytics | ◐ [enriched] Cline telemetry present; Continue FINAL: anonymous telemetry + auth STRIPPED in final 2.0.0 (read-only handoff; previously full OTel CLI metrics service w/ OTLP exporter) |○ Not an editor| ● A (opt-in) |
| Evals harness | ◐ internal | ◐ | ○ | ● harbor | ● benchmark | ● [corrected from ○] (evals/ + memory-tests/ + perf-tests/ + behavioral evals framework) | ◐ (README self-reported SWE-bench Verified 500x3 across 7 versions w/ exhaustively disclosed harness settings; Arena eval-as-feature; no eval-harness code in repo) | ○ | ◐ | ● born-from | ● (suite+SB SOTA bot) | ○ (promised, never) | ◐ (packages/evals; TB participation) | ◐ internal (SWE-bench 70.8% claimed) |◐ [corrected from ●] (FrontierCode 1.1 internal suite w/ published methodology - self-run, self-headlined, no external replication; Artificial Analysis + Vals AI third-party Fusion evals; Security Swarm vendor bench; no user-facing in-harness eval runner)| ◐ (internal SWE-bench-style claims + code review agent step; no public suite) | ◐ (CursorBench-3 + Cursor-Blame commit-mined tasks + agentic graders + Bugbot resolution-rate leaderboard 78.13% n=50,310 + real-time-RL eval gates; no public agent-harness suite) | ○ [corrected from ◐] (no public eval harness/benchmarks in current docs; Nov-2025 SWE-bench claims not re-verified; AA independently tests the SDK harness only) | ● [enriched] (eval-cli headless NativeAgent pipeline (ZED_EVAL_ENABLE_FLAGS, JSON EvalResult) + zed-eval on Modal/Harbor/Pier + zeta eval + shared e2e macro + per-release behavior-diff model evals + public Agent Metrics) | ◐ (private test-set gating per model; SWE-Rebench top placement claimed at 61.6% resolved / 72.7% pass-at-5; no public eval harness) | ◐ [corrected from ●] (no eval harness in OSS client; last public submission 2025-12-12 [TB2.0 61.2%]; TB 52% + SWE-bench 71% maintainer-reproduced / 75.8% #3 are 2025 vendor runs; pivot to private Factory Benchmarks replay product — closed) | ● (vendor suite: TB + Agent Arena + Legacy-Bench + NextJS + Review) | ◐ (internal model evals as /models pages; no public suite) | ● adulari: official SWE-bench Docker evaluator (deliberately not reimplemented) + 83-cell validity ledger + multi-seed pass@k + retraction-in-place; harness-rs: pass^k CI-gated + leave-one-out guard ablation + trigger coverage (never-fired guard = noise) + per-turn cost; conformance suite drives real loop w/ mock providers (3,204 test fns; badge "324" under-claims) | ◐ [corrected from ●] ~25 wire/regression probes + runtime_bench + Atropos RL trajectory generation; NO public capability suite (GAIA runs were AutoAgent) | ◐ [corrected from ○] (Continue FINAL: eval/ emptied at freeze; remaining = vitest suites + llmTestHarness + manual-testing-sandbox — regression tests yes, standalone harness gone; Cline none documented) |● THE eval harness: Terminal-Bench + SWE-Bench + Polyglot via harbor| ● adapter A |

## 2. Git-history / reversal lessons (mistakes they reverted)

- **Codex**: TS→Rust full rewrite (May 2025, #1174: install weight, native
  sandbox, no GC); approval-mode naming churn suggest/auto-edit/full-auto →
  read-only/auto/workspace-write; **untrusted policy RETIRED** (documented
  migration); `codex exec --full-auto` deprecated; Windows long-broken then
  restricted-token sandbox. LESSON: naming churn burns trust; sandbox-first
  design should exist day one (we do).
- **Gemini CLI**: `--checkpointing` flag REMOVED v0.11.0 (settings-only);
  Aug 2025 free-tier cuts partially reverted after backlash; silent Pro→Flash
  fallback (trust killer); infinite rate-limit loops (#1626/#6986/#2909);
  data-loss incidents (#4586 mkdir-cascade; #7389/#19729 .git deleted).
- **Claude Code**: closed-sourcing → March 2026 source-map leak (512k LOC);
  silent model downgrades (#40269, Sept 2025 + Mar 2026); memory leaks
  (#86202, 14.5 GiB; steady 0.5–1 GiB); weekly-limit backlash Jul–Aug 2025;
  npm→binary→dual distribution. LESSON: memory discipline + model-identity
  transparency are contract-grade (we test both).
- **opencode**: THREE renames/two owners (TermAI→OpenCode→sst→Anomaly;
  Crush split with git-history-rewrite allegations); JSON→SQLite session
  migration mid-life; rapid breaking changes; Zen deprecation table churn.
  LESSON: community ownership + stable storage migration story (we ship
  versioned schema + JSONL export from day one).
- **goose**: subagents + skills OVERLAP → consolidated into Summon ext
  ("overlapped in confusing ways", v1.25); EventStream→operation-pipeline
  rewrite (2.0); `goose review` command-exec vuln (GHSA-r5pp-p5r8-466r) —
  git-integrated agents are attack surfaces; governance headwinds → Linux
  Foundation donation (Apr 2026).
- **OpenHands**: V0 pub/sub EventStream abandoned ("very confusing, thread/
  async issues, few order guarantees") → V1 event sourcing (MLSys 2026);
  V0 removed Apr 1 2026. LESSON: event-sourced core from day one (we do).
- **aider**: single-maintainer → maintenance mode (v0.86.0 Aug 2025 last
  release; "Where is Paul?" #4613); NO MCP (refused, #4506). LESSON: bus
  factor is real; MCP refusal cost ecosystem relevance.
- **Neovim-adjacent**: nvim-treesitter master→main full-incompatible rewrite
  ("you don't migrate"); ABI-15 parser breakage. LESSON: extension-ABI
  stability planning (our skills format is std-compatible; WASM capsuled).
- **Windsurf/Cursor**: .cursorrules deprecated → official Project Rules →
  official plugin marketplace (2026) — community conventions get absorbed;
  build on open standards early.
- **AutoGPT-era**: infinite loops (context-window discipline absent),
  platform pivot, "single omnipotent agent" dead. LESSON: budgets +
  loop detection + deterministic tools (all in our plan).

## 3. Decisions ledger (ADOPT / AVOID / WATCH)

**ADOPT** (with sources in draft §findings): operation-pipeline loop (goose); || Droid adds (sec16): command blocklist with program-resolution anti-bypass (wrapper-shell / abs-path / quoting / substitution-proof) layered under allow+deny; complexity-tier subagent routing (light/medium/heavy per-tier model+effort) as simpler role grammar composing with custom droids; mission VALIDATOR milestones (separate scrutiny + user-testing workers at phase boundaries, config-skippable); modelFallbacks per-model fallback map (cheap chain primitive); fail-closed sandbox posture (refuse-to-run unsandboxed) + settings-file write protection + interactive sandbox.violations prompt; sandbox network filter-proxy for kernels without net isolation; per-server MCP connectTimeout + non-stalling unresponsive servers; /verify claim-verification verdicts (validates our /goal evidence gate); explorer read-only vs worker built-in split; droid doctor diagnostics (validates existing doctor ADOPT).
event-sourced sessions (OpenHands V1); inspector chain + RepetitionInspector
(goose); Anthropic compaction doctrine; deferred tool loading + 6 invariant
tools + byte-stable cache layout (user blueprint §3); CSR PageRank repomap
(aider+blueprint); view_window ≤100 + 3-fringe (SWE-agent ACI); PTY vt100
scraping (blueprint); CoW/worktree shadows + pre-flight + max-2 subcycles
(Cursor+blueprint); snapshot refs + /rewind (Roo/blueprint); semantic hunks
y/n/e (Mentat/blueprint); SQLite+sqlx versioned schema + fork/resume (goose);
doom_loop guard (opencode); 4-mode autonomy dial (goose); rmcp client+server
(D3); agentskills.io-compatible skills + pack aggregation (user's audit packs);
MEMORY.md+BM25 cross-session memory (Claude+basic-memory); Skill Forge gates
(approval+eval-before-activate+versioning); model profiles + A/B (novel,
user); hard budgets (OpenHands); harbor/Terminal-Bench eval adapter (goose);
nextest-archive CI + SSE-DSL + DummyApi + cassettes (Codex/goose/opencode);
evals-never-PR-gates + expected-failures YAML (goose); hashline anchors
(OMO 6.7→68.3%); per-role routing w/ confirmed fallback; scheduler+notifier;
daemon multi-attach (crush); editor telemetry opt-in (Windsurf); IDE LSP
bridge; share links (opencode); doctor (Claude/OMO); output styles; ACP adapter
(optional); background bash; Stop-block hooks; protected paths + egress
|| Zed adds (sec23): tool_permissions pattern (allow/deny/confirm + regex lists + un-overridable built-in safety rules + chained-command parsing + UI safe-pattern extraction); ACP as the LSP-of-agents (editor-agnostic harness hosting); public anonymized Agent Metrics page; detail-agent adds: SHA-256-verified release-asset auto-install + env redaction for ACP child agents; host-side uniform checkpoints (acp_thread; auto-update on repo change, terminal-kill on restore); OS-sandbox tier Seatbelt/Bubblewrap/Bubblewrap-via-WSL (worktree-root auto-grant, .git protection, once-per-thread unsandboxed escape); per-role model profiles + refusal-fallback retry + modeled stop reasons; spawn_agent disjoint-write-set contract + MAX_SUBAGENT_DEPTH=1; sibling threads in linked git worktrees w/ ref pinning + archive/restore; profile-scoped permissions + restricted-worktree auto-downgrade; line-windowed inclusive buffer reads over rope/sum_tree; GPUI 10ms frame budget + hang attribution + bench-infra productization; extension capability declarations + 10-version WIT ladder; eval-cli JSON EvalResult + shared e2e macro; compaction telemetry outcomes + AutoCompactThreshold knobs
|| Junie adds (sec24): three-state Brave mode as a named cyclable autonomy dial (off / auto safety-classifier / on); debugger-integrated agent mode (/debug drives a live IDE debugger session: breakpoints, stack frames, expression eval); per-model private eval gating before a model ships in the harness || Junie detail adds (sec24): IDE-as-toolbox inversion — drive the real semantic index/build configs/test runners/debugger over a localhost MCP endpoint instead of reimplementing approximations; plans as committed .junie/plans artifacts w/ Confirm-approval gate; engineered prefix discipline (max reusable initial prefix + KV-cache reuse across tasks); agentic debugging incl. decompiled JAR symbols via debugger MCP toolset; ACP-registry agent install w/ IDE-managed runtimes
|| Warp adds (sec25): the per-action x per-level autonomy matrix with denylist precedence over every other setting; blocks-as-context (structured terminal capture feeding the agent); local-to-cloud handoff mid-task; detail-agent adds: OSC 777 structured-JSON cross-harness bus w/ session-status machine + 2s cancel grace + per-harness plugin marketplaces; typed permission REASONS in the UI; credit rollup over child agents; periodic workspace CheckpointCoordinator (safe-boundary) + file rewind; OIDC workload-identity federation for agent runs; skills hot-reload watchers + skills-lock.json; FileBasedMCPManager multi-provider templatable servers + parse diagnostics; replay-from-traces benchmarks feeding custom model routers; one wgpu core across GUI/TUI/WASM/CLI frontends; per-harness global rules dirs + CLAUDE.md migration
| **ADOPT**: — **Rust micro adds (sec26): Adulari Forge proves one Rust binary can carry mesh routing, line-level provenance, a completion gate, and budget caps at 8 stars; adoption is not a capability signal.  iter19 detail deltas — forge: doc-sync test asserting normative-doc constants match live symbols; conformance suite driving the REAL loop w/ mock providers (every guard test-pinned, incl. the 553x prose spiral); retraction-in-place benchmark honesty + a where-forge-does-NOT-win section; rank-verbatim failover chains (provider round-robin = named regression) + wait-out of short rate limits; quota as first-class currency (QuotaPace projection, pool capacity, conservation windows, burn weights); refs-by-name not dst_id in code graphs; RSS-sampled bounded LSP; builtin deny floor outranking bypass + per-segment shell parsing + multi-line conservative floor. harness-rs: honest isolation reporting (FsPolicy requested vs Isolation enforced); trigger-coverage + leave-one-out guard ablation + pass^k floor benchmarks (CI-gated); default-off guards with measured written justification (MonotonyPolicy); spill-not-truncate oversized tool results (-17% cost/pass); DECISION:-marker capture (measured noise-free); PostCompact{before,after} compaction accountability; SealBreached as a first-class pageable event; cancellation as Outcome w/ process-group kill; crate-merge doctrine (a crate boundary has to name a dependency you might not want); closed experience loop (SkillDistiller promotes proven episode to SKILL.md, SkillReviser drafts fixes); URL-scheme memory backends that fail loudly naming the feature; PlanDelta::Add dynamic replanning with per-transition persistence; guard-rationale comments citing the incident that set every constant.**
| **ADOPT**: — **Hermes/Auto adds (sec27): Hermes makes the skill-learning loop first-class infrastructure (autogenesis, in-use improvement, nudges, FTS5 recall); adopt the loop as a subsystem, not a demo. iter21 delta: the loop is source-verified as FIVE mechanisms (skill_manage writes, post-turn background_review fork, in-use improvement, idle curator, 10/15-turn nudges) — keep the write_approval human gate and the ClawHavoc lesson (content-hash scan + quarantine + lock.json on every install; registry trust is not vetting) + shadow-git pre-mutation checkpoints + head/tail-protected aux-summarized compressor + 11-aux-slot model topology; wave adds — prime-agent Continual Harness (harness state H=(ρ,G,K,M) as typed CRUD on disk, /refine = smallest evidence-backed versioned edit w/ rollback-by-ID, base system prompt immutable), EvoSkill program/* git-branch frontier (auditable offline evolution vs your own benchmarks), SkillClaw proxy-level cross-agent skill pooling.**
| **ADOPT**: — **Continue/Cline adds (sec28): adopt the checkpoint-undo + Plan/Act approval split as the trust floor; Cline proves gated autonomy scales to 68k stars without an OS sandbox.; iter20 adds: config-as-code as the durable artifact (config.yaml Zod schema + blocks uses/with/override + markdown/AGENTS.md rules lived in-repo and outlived the company — the user exit path is the repo itself); the Continue handoff protocol as best-in-class wind-down (final release strips telemetry+auth, Apache-2.0 preserved, plain read-only README note, docs left live, explicit export deadline, billing cut early — no cliff); graded-claims reporting (verify the repo, hedge single-sourced deal claims, trim without primary); email-first EOL notification to registered users**
| **ADOPT**: — **T-bench/Harbor adds (sec29): adopt the instruction + verifier + oracle triple as our regression contract; make every capability row in this study a harbor-runnable check. Sweep adds (sec29-30): adopt Terminus 2 as the neutral measurement reference harness (product-free scores isolate model+dataset) and the mini-SWE-agent ~100-line bash ReAct loop as the minimal-floor regression harness (frontier-near-SOTA through 100 lines validates the 6-tool core); pin eval runs to a dataset GENERATION, never a moving board (2.1 fixed-task revision alone moved Opus 4.6/Claude Code +12.1pts; boards shipped 2.1 May then 3.0 Jul 30 then 4.0 Aug 28); adopt the Hacks reward-hacking-delta column as our eval-integrity gate — publish hack deltas next to any claimed score.**
| **ADOPT**: — **Gemini CLI adds (sec31): sandbox breadth as the reference taxonomy (5 backends — Seatbelt 6 profiles / Docker-Podman / Windows-native / gVisor / LXC — + bwrap tool-level sandboxing + per-command Sandbox Expansion Requests); policy-engine allow/deny DSL + fail-closed workspace trust + trusted folders; staged eviction ladder (toolOutputMasking → toolDistillation → chat compression → truncation); per-install consent strings for extensions/skills; SHA-embedding nightlies (tag↔SHA traceability by construction); omissionPlaceholderDetector anti-elision guard.**
| **ADOPT**: — **jcode adds (sec32): client-server daemon + session amortization as the multi-session RAM lever (exec()-same-socket zero-downtime reload); MCP schema disk cache with config-fingerprint invalidation (kills the first-snapshot cache miss — the guard everyone else omits); append-only prefix-hash cache-violation tracker (~400 LOC, provider-agnostic CI hook); file-touch collision alerts (write/edit publish intent+summary, server alerts BOTH sides on overlapping modification); self-dev guardrail set (serialized per-scope build queue, canary state machine, crash capture w/ diff, reload-loop suppression, promote-vs-reload separation); flock-gated stale-socket reaping.**
| **ADOPT**: — **Elicitation adds (sec33): the SET/PROMPT/THROTTLE doctrine as the model-profiles constitution (SET API knobs per family — effort explicit per workload + cache-preserving per-message effort beta + xAI effort-on-every-call; PROMPT per-model counterweights — commit-to-approach for Opus 4.6 over-exploration, batching+progress+search-when-unsure for Fable 5.1, stopping-conditions+retrieval-budgets for GPT-5.5, bias-to-action+delegation for GPT-6 Astra; THROTTLE structurally — two-tier call governors keyed to task class, split subagent caps concurrent×total×depth-1, token-budget governor as primary economic control w/ auto-degrade ladder); per-model 400-trap feature-detection table in the model catalog; spawn-authorization classes (standing vs fresh auth) with LOUD refusal, never silent parent-DIY fallback; parallelism normalization (one-message tool_results + batch_tool + disable_parallel_tool_use); per-family edit-format routing + schema-similarity rule (Cursor/Copilot precedent); tool-integration tests re-run per model upgrade; joint (model×profile) eval — locked-harness regression + 2×2 factorial + ETCSOVG cards + TSS/golden-prefix/Pass^k consistency gates.**
| **ADOPT**: — **Cursor adds (sec19): adopt the short-workflow durable execution blueprint (workflow-per-turn + signal-with-start + child-workflow subagents + production-history CI replay); CAS activity retry without token replay; split-brain self-hosted workers (outbound-only tunnels; stdio MCP local / HTTP MCP proxied; creds never where the model runs); OIDC workload-identity socket; unweakenable sandbox.json admin floor + SSRF-guarded default-deny; full-entry-hash MCP trust (CVE-2025-54136).**
allowlist (Codex); perf budgets in CI (novel discipline). Roo Code: mode-scoped tool masking + fileRegex (ORIGIN — core of our plan); per-mode model profiles + sticky models (validates todo 42); deliberately tool-less orchestrator w/ summary-only return (boomerang — but keep delegation available from ANY agent, Kilo caveat); structured todo handoff in new_task; non-destructive truncation tags restorable on rewind; same-model condensing policy; context-poisoning doctrine + session-restart guidance in docs; shadow-repo-per-task checkpoints w/ sanitized git env (validates todo 23); delegation-state repair on resume (Roo #11281); allowed/denied command matchers in auto-approval. Kilo adds (sec11): capability-scoped managed-session tool (agent_manager: worktree/local/prompt/stop/move separately approvable); best-of-N multi-version w/ per-version model; subagent_depth opt-in nesting + deny-only permission inheritance + child task/todowrite gating; task_id-resumable subagents; stored-only board receipts; Context-Epoch KV-baseline + mid-conversation system messages (validates todo 10/11); snapshots OUTSIDE repo per user-turn (todo 23 alt); on-demand model-catalog search tool. Plandex adds (sec13): exec-as-reviewable-diff `_apply.sh` batch execution (tentative-apply -> run -> auto-rollback on failure); per-role fallback CHAINS in model profiles (large-context/strong/error - Sonnet-200k->Gemini-2M auto-swap); deterministic-first edit ladder with race (aider-diff vs whole-file) on failure; autonomy levels = named presets over ~10 explicit toggles (independently settable + defaults); plan-branch-per-model as cheap best-of-N variant; branch-before-rewind + auto-revert-on-rewind with conflict prompts; RAM-only ephemeral API keys for daemon-held provider keys; settings/context mutations as rewindable session events. Pi/Claw adds (sec14): 4-tool minimal-baseline discipline (features justify themselves vs read/write/edit/bash floor); JSONL TREE sessions (id/parentId in-place branching + branch summarization on switch); cross-provider context handoffs + prompt-cache discipline (incl. cache-write suppression for one-off prompts); agent-authored skills/extensions hot-reload (Skill Forge synergy); Project Trust ask-and-remember per canonical dir for project-local automation; supply-chain hardening checklist (exact pins, min-release-age, shrinkwrap, ignore-scripts, audit cron); event-router daemon OUTSIDE agent context (clawhip); per-model compaction token budgets; NOOP-default vendor-neutral telemetry contracts. Pi verification pass 2 (sec14.9): SSH remote-execution backend (ssh.ts tool-ops delegation). || Grok adds (sec15): hashline anchor-scheme benchmark harness (read-amplification metric, shifted-anchor search; ChunkFingerprint default); tool behavior version registry (current/legacy presets + Active/Deprecated/RemovalCandidate lifecycle); tool-result pruning ladder (keep_last_n=3 / soft-trim 4000 to head+tail 1500 / hard-clear placeholder at 10 turns) + pre-compact memory flush trigger at threshold-4000 tok; compaction segment store (compaction/INDEX.md + segment files + 4 detail levels); post-compaction memory re-search + first-turn injection (min_score) + temporal decay + MMR + gated /dream consolidation; compat-swallow day-one interop (AGENTS.md + Claude/Cursor rules/skills/hooks/MCP/settings + foreign-session resume + provenance inspect report); hook events w/ PostToolUse output replacement (redaction) + Stop decision control + Pre/PostCompact (with fail-closed option); sandbox session-pinned profiles across resume + fail-closed custom deny-glob profiles + shell env secret scrubbing; /goal adversarial evidence-review completion gate; tool-I/O byte caps w/ disk spill (MCP 20KB); worktree GC w/ reclaimed-refs safety naming; OTel double-opt-in content-free external schema. || Amp adds (sec17): plugin tool.call interception gates with modify+synthesize actions (a permission layer that can rewrite or synthesize tool args/results — validates inspector chain); Handoff second-model distillation + read_thread extraction as no-compaction complements to our 3-tier eviction; Dial effort-bundle grammar (low/medium/high/ultra = model+effort+prompt+tools+oracle per mode) as the UX for model profiles; Amp-Thread-ID + Co-authored-by git trailers + server-side SSH signing service (private key never on agent machine); per-command short-lived provider tokens via credential helper (no long-lived tokens in workdir); lowest-layer secret redaction before LLM/cache/transport/storage; URL-addressable cross-client threads + remote control (validates daemon multi-attach); orb spawn pacing (burst 20 then 1/5min) as fan-out abuse guard; deferred MCP-in-skill tool loading with includeTools globs (validates deferred loading ADOPT); grain-of-the-model per-lab behavioral profiles (independent validation of model-weakness profiles as the right final phase). || Qwen adds (sec18): prompt-cache-preserving fork subagents (tool RESTRICTION without declaration change protects the shared DashScope prefix; btw/recap on cache path; explicit 80%+ savings claim) as the subagent-x-cache design law; multi-client permission mediation (first-responder/designated/consensus-quorum/local-only + cancel sentinel w/ anti-injection guards + N1 synchronous-timeout invariant); MCP transport pool keyed by config fingerprint w/ refcounted idle drain + hysteresis budget guardrails (off/warn/enforce); typed SSE event schema w/ divergence-proof resync (monotonic ids, bounded replay ring, state_resync_required + delta-skipping reducer); Auto Mode 3-layer classifier stack (fast-path exceptions -> safe-tool allowlist -> two-stage LLM classifier; broad-allow stripping while active; softDeny clears only on exact user intent; NL hints; loop guard; headless = Ask); fail-closed conversation writer leases (no force-unlock, boot-id/PID-ns identity proofs, transcript-proven sealed handoff); Agent Arena worktree best-of-N across providers w/ winner-diff apply; /learn skill synthesis from docs/repos/videos + /curator stale(30d)/archive(90d)/pin lifecycle; cross-session PID-keyed registry w/ boot-id liveness proofs; per-version self-published SWE-bench Verified 500x3 w/ exhaustively disclosed harness settings (eval-transparency discipline). || Cursor adds (sec19): sandbox.json network/path allowlists merged user+project w/ unweakenable admin floor; NL permission instructions the agent itself edits; published routing economics (uplift gate + per-mode budget) [deepened 2026-09-11 rewrite: Temporal durable execution + CAS activity retry + split-brain workers + OIDC socket + full-entry-hash MCP trust; see dedicated sec19 lines above] || Copilot adds (sec20): gh-aw safe-outputs (read-only token default, validated writes, threat detection) as publishable capability-belt pattern; MXC local sandbox brings first credible Windows story; 59-min hard session cap as honest resource bound || Antigravity adds (sec21): action(target) permission grammar (Deny>Ask>Allow, regex: opt-in after the silent-auto-approve bug) + permissions-derived sandbox mounts + read_url/execute_url split (fetch vs actuate) + artifacts as trust surface || Antigravity detail adds (sec21): subagent permission bubbling + inheritance and per-subagent commandExecutionPolicy (off/auto/eager/sandbox); permissions-derived sandbox compilation (one policy source -> prompt gates + mounts + network AllowedDomains); Teamwork fresh-successor-between-milestones as context-degradation defense + per-agent scratch dirs + artifact-mediated state handoffs; /boost decoupling (read-only investigation workstreams + failed-assertion verification feedback loop, DeepCoder/DeepInvestigator); /learn distillation of session feedback into Rules/Skills; artifact-review gates staged before filesystem writes (Request Review vs Always Proceed); max_total_tokens hard budget -> status:incomplete + resumable via previous_interaction_id; MCP Ask-default for unconfigured tools + mcp() scoping grammar + disabledTools; split per-provider quota pools (weekly+5h) surfaced in HUD || Devin adds (sec22): postmortem genre as product practice (publish failure analyses with mechanics); annotate-expected-before-acting honesty pattern; Fusion cache-aware model switching at compaction points; context-anxiety fix (big-window cap)

**AVOID**: vector DB for code; whole-file rewrites/echo; stacked error dumps; || Droid adds (sec16): cloud mirror default-ON (cloudSessionSync true) as account posture; always-allowed vendor telemetry domains inside the sandbox network proxy; rolling multi-window rate limits + opaque 0.12-12x credit multipliers + repeated pricing churn (re-confirms token-resale lesson); closed core with issues-only 18-star repo standing in for governance; shipped false-success reporting (independent finding); config-surface churn (.droid.yaml deprecated for .factory/, autonomyMode deprecated); gating missions behind prepaid Extra Usage; native-Windows sandbox skip (WSL2-only).
model-as-linter; unbounded subprocesses; unstable serialization; dirty state
after failed edits; tool-schema churn; embedded inference in core; V8/dylib
plugins; auto-run unverified plugins; telemetry default-on; silent model
substitution; Windows-afterthought; git-history rewrites (opencode lesson);
pub/sub core (OpenHands V0); overlapping delegation mechanisms (goose lesson
|| Zed adds (sec23): opt-in data discipline stands (nothing persisted unless the user rates a response); detail-agent revisions: plain-SQLite persistent sessions (threads.db full thread history, no at-rest encryption); core safety surface (OS sandbox) feature-flag gated + Windows = WSL-dependent; consent plumbing that silently regressed once (cloud data-collection choice dropped, caught + restored 2026-02-17); cost HUD with zero budget/cap enforcement
|| Junie adds (sec24): dollar-denominated expiring credits (Free 3 / Pro 10 / Ultimate 35 per 30 days, ~$1 each) drew sustained consumption backlash — opaque credit burn reads as retention, not metering; detail adds (sec24): token resale at ~API parity while competitors subsidize (1 credit = $1; 2 credits for a 3-min trivial task; Pro quota gone in 30 min); quota-cut damage control ("that is on us" FAQ) followed by 10 months of open-a-ticket boilerplate while own transparency tickets LLM-16701/LLM-19959 stayed open; silent default-model swaps
|| Warp adds (sec25): unpublishable per-action credit costs (teams must budget by running a billing cycle); the 2022 combo of opt-out shell telemetry with mandatory login — it cost two years of trust and a 73-point HN relief thread to unwind; detail-agent adds: benchmark retreat at the competitive inflection point (last public submission 2025-12-12); squashed-history open-sourcing (no provenance/archaeology); CLA claimed in FAQ w/o any in-repo mechanism; plan credits expiring monthly (breakage as revenue); no local OS sandbox while marketing runs on cloud isolation
| **AVOID**: — **Rust micro adds (sec26): kwaak is abandoned by its own README (no longer actively maintained); Docker sandboxing and PR-linked evals could not beat vendor distribution.  iter19 — forge: 264,762-LOC/18-crate scope carried under the "micro" label (mobile, voice, E2EE relay, desktop); load-bearing lexical tier classifiers (substring tables age — their own ADR-0011 concedes it); huge single-file modules eroding intra-crate seams (mesh lib.rs 6,272 lines). harness-rs: shared 0.0.x versioning across 15 crates (63 releases in <4mo; every release breaks every pin; harness-rs-tools = 90 downloads); DESIGN-vs-code doctrine debt (guide.md names 5 nonexistent crates); solo-maintainer surface creep (browser CDP ~190KB, user_model 74KB, image/speech — each a project of its own); advertising unimplemented transports (DESIGN shell-script/MCP hooks absent in code); 120.7KB lib.rs mega-crate; vendored provider-SDK forks acceptable ONLY with a documented exit condition (forge documents drop-once-upstream-ships).**
| **AVOID**: — **Hermes/Auto adds (sec27): AutoAgent lets a model commit new tools into the mirror repo that executes it; self-modification without a floor beneath a bad commit is not a production trust boundary. iter21 autopsy: also dead as a dependency — dormant ~11 mo (last push 2025-10-16), functional life ≈3 weeks (last *.py commit 2025-02-28), "self-improvement" was create_tool string-codegen authoring automation with nothing persisting across sessions; plus YeQing17-2026/OmniAgent — day-zero single-repo account (repo 9 min after account creation), NOASSERTION license vs GPL-3.0 badge, star-velocity consistent with inflation; never count it as tier-health evidence.**
| **AVOID**: — **Continue/Cline adds (sec28): do not build on Continue; upstream is read-only after final 2.0.0, so harvest the config.yaml and hub-block ideas, never the dependency.; iter20 adds: hub-dependent architecture — the monetization surface was also the kill switch (hub.continue.dev now NXDOMAIN; blocks/agents/community artifacts unrecoverable; anything load-bearing must be exportable/static before launch); extension-only surface on competitor-owned hosts (4.1M installs stranded in marketplaces the team does not control; value-neutralized without a code change); late thin monetization (~5.1M USD + enterprise-GTM CEO hire May 2025 = too little, ~12 months too late); pivot whiplash (autocomplete→chat→agents→CLI→CI checks in ~18 months; the differentiated final identity got ~4 months of runway)**
| **AVOID**: — **T-bench/Harbor adds (sec29): do not pin terminal-bench-1 as a moving target; v1 is renamed under harbor-framework and frozen while Terminal-Bench 2.0 runs only through harbor. Sweep adds (sec29-30): do not plan community-submission entries to Terminal-Bench — submissions are CLOSED post-2.1, maintainer-run only (a score exists only if the Harbor team ran it); never pin CI to a board generation (quarterly churn, live-verified 2026-09-12).**
| **AVOID**: — **Gemini CLI adds (sec31): overnight consumer-auth removal with no grace period + README/docs contradiction + stale quota page (trust debt outliving the product decision); handing users to a closed Go successor via curl-pipe-bash installer (migrate-to-unauditable-binary anti-pattern); building consumer-facing anything on gemini-cli upstream (enterprise-only maintenance, ~94% velocity collapse, original core team gone).**
| **AVOID**: — **jcode adds (sec32): critical-workflow dependency on jcode binaries (bus factor 1 — 7,284/7,323 commits, 8 human non-author commits ever, no governance); 80-crate crate-per-type fragmentation as a style; citing self-reported RAM benchmark tables (single-machine PSS, author-chosen workloads, embeddings-off headline row — directional only, never reproduced independently); memory sidecar pinned to one small model in a load-bearing spot; plaintext dot-env API-key layout with no OS-keychain option.**
| **AVOID**: — **Elicitation adds (sec33): prompt-only suppression as the sole tool governor (provably coarse — kills necessary calls with unnecessary); one global call cap across task classes (~10× spread in correct budgets); outcome-only eval rubrics (causally induce overuse — balanced-reward cuts −66.7%/−60.7% prove the link); raising effort on conflicting-instruction/open-ended-tool tasks without evals (GPT-5.5 documented overthinking warning); mid-task model switches without a summarization boundary (OOD history + cache misses); single-run conclusions at temp-0 (24%/9% unique outputs, serving-stack drift); assuming newer model = drop-in upgrade (schema-prior leakage RAISED malformed-call rates on Opus 4.8/Sonnet 5); the family 400-traps (budget-on-4.7+/prefill-on-4.6+/forced-choice-on-Fable/budget+level-on-Gemini/none+sampling-on-Astra/penalties-on-Grok).**
| **AVOID**: — **Cursor adds (sec19): silent feature removals without changelog entries (Memories, Custom Modes v2.1) with removal debt outliving the UI (downgrade-as-fix); model-identity opacity (routed model hidden by default, silent slug rerouting); Token Rate billed on BYOK and third-party cached tokens; MCP trust bound to entry name (CVE-2025-54136 class); same-day ToS forum-selection hardening at deal close.**
— one Summon-style path + spec launcher); npm-wrapper distribution fragility. Roo Code: proprietary memory formats (Memory Bank died with its docs — AGENTS.md std outlived it); coordinator-ONLY delegation (Kilo deprecated Orchestrator: overhead w/o capability); company product built on unquiet Apache fork w/o governance moat (archive stranded 3M users; 492 PRs/550 issues left open). Kilo adds (sec11): rebuild GA via silent auto-update to full base (#8415 week-one chaos); relaxing review-tight defaults during migration; migration shipped before parity equivalents (indexing/memory) - public parity TABLE itself was right; press-metric drift (5M vs 3M). Plandex adds (sec13): license instability (no-license launch -> MIT -> AGPL 2024-03 -> MIT 2025-03 - pick one, keep contributor policy stable); mandatory Docker+Postgres for single-user local mode (embedded SQLite stands); shipping co-dependent features without interaction tests (background tasks disabled-by-default vs auto-context); token-resale margins vs subsidized frontier incumbents (re-confirms Roo Router lesson); announce-never-ship benchmarks. Pi/Claw adds (sec14): permission-free-by-default as SHIPPED posture (trust boundary = user account; container tier + inspectors stand); XDG/config-location footguns on Linux; crate/package naming-stub traps + clean-room trademark adjacency (claw-code renamed-to-agent-code stub); agent-managed-repo-without-human-gates for user-facing artifacts (exhibit froze, 42 open issues). || Grok adds (sec15): working-dir-to-cloud trace uploads + dual retention/transmission toggles + nonfunctional privacy switch (5.1GiB vs 192KB, canary echoed, server-side-only fix, remnant gcs.rs in-tree); source-drop governance (no external PRs, issues disabled, sync-only history, generated root Cargo.toml; 8+ forks within 48h); hooks fail-open default; user-facing /rewind conversation-only while in-code workspace RewindPoint restores files/hunks/git (incoherent checkpoint semantics); auto_update default-on; hard-coded vendor User-Agent exceptions (Figma); Windows-sandbox afterthought; worktree sprawl w/o day-one GC (412GB example). || Amp adds (sec17): permission-free-by-default shipped posture (re-confirms Pi/Claw lesson; trust boundary = account + cloud); silent model substitution as POLICY ("Consult the UI, not the model" + overnight default swaps + Amp Free unknown-model routing — re-confirms Claude Code model-transparency lesson); explicit no-local-sandbox + no-.ampignore anti-posture (reject for a local-first harness); shared-by-default thread visibility (personal threads become workspace-visible on workspace join; re-confirms Droid cloudSessionSync avoid); ad-supported + training-opt-in free tier era; MCP Resources/Prompts unsupported (incomplete client surface); closed core + closed server-side session format; feature-kill stranding integrations (editor extension killed 2026-02 -> IDE users stranded; cra-github archived 2025-12). || Qwen adds (sec18): free-tier-as-distribution then hard pull w/ a broken paid funnel (2k/day -> 1k -> 100/day effective immediately -> discontinued 2026-04-15; paid Coding Plan unbuyable/out-of-stock; goodwill migrated to OpenCode/OpenRouter -- subsidized-quota lesson re-confirmed); frankengraph history import (parentless root + 3 stitch operations + PR renumbering -- archaeology-hostile provenance, brief premises off by 4 months); dead-feature UX ghosts (discontinued OAuth method haunted CLI/ACP for 100+ days, 4+ cleanup PRs); build-then-retire subsystem churn (WebUI lived ~8 months; Electron desktop forked OUT mid-life w/ orphaned-module rescue); vendor-harness benchmark publication without harness-matched official rows (+10-25pt unexplained delta vs Terminus-2: card 51.5 vs 23.9-24.6 same-model); plaintext-by-default credential storage (opt-in AES-256-GCM exists -- flip the default). || Cursor adds (sec19): in-editor shadow LSP windows as parallelism (opt-in 2024, community-doubted 2025, Rust unsupported) — git worktrees + remote won [corrected 2026-09-11 rewrite: production parallelism = git worktrees + cloud VMs (2.0) by design, not window fallback; shadow-window framing retired; see dedicated sec19 lines above] || Copilot adds (sec20): Copilot Workspace preview sunset (Apr 2024 to May 2025) inside 14 months — do not build user workflows on vendor preview surfaces; one-PR-per-task rigidity || Antigravity adds (sec21): platform churn inside 7 months (IDE Nov 2025 to 2.0 May 2026, Agent Manager removal announced); opt-in sandbox on Nov 2025 launch left shell agents unsandboxed — drive-wipe and prompt-injection exfiltration incidents followed || Antigravity detail adds (sec21): quota whiplash (quiet tightening -> ~92-97% measured cuts + 168h lockouts -> May restructure -> Sep 2026 429-despite-full-quota wave incl. expired localhost TLS cert + AG-859 desync); undocumented hard caps at the flagship tier (16,384 output tokens/turn all plans; 1,024 Claude thinking tokens; silent Gemini truncation = correctness bug); security triage as Won-t-Fix-(Intended-Behavior) + stealth Known-Issues edits + no CVEs for flagship bugs; per-response model-identity opacity (no model-used badge; mismatch probes officially denied); plaintext OAuth tokens beside safeStorage-encrypted transcripts; same-day silent default-model flips under unchanged agent ID || Devin adds (sec22): benchmark-anchored replacement marketing (13.86% SWE-bench + first-AI-engineer framing at $500/mo vs 3/20 on Answer.AI real tasks); demo as curated sample — the denominator stays off camera

**WATCH**: ACP standard momentum; AAIF governance outputs (skills/MCP/AGENTS.md || Droid adds (sec16): Factory Router per-task routing economics (43% claim; Auto Model alias); Droid Control stable computer-use verification plugins (/demo /verify /qa-test); Mission Mode GA trajectory (research preview); enterprise custom-model base-URL allowlists; Droid Core open-model free tier as retention funnel; 2-5 releases/week ship-train cadence as ops model; Droid-vs-Amp 3P-harness convergence in HN discourse.
under LF); ChatGPT↔Claude plugin-format convergence; Grok Build internals;
Deepseek Harness concepts (iteration 1); Hermes/AutoAgent auto-skill systems
|| Zed adds (sec23): ACP registry re-platforming editor-agent integrations (extension agents deprecated); Delta DB CRDT context capture as a future telemetry surface; external agents hiding model identity from the host (metrics blind spot); detail-agent adds: Delta app (Aug 2026) + DeltaDB as next context substrate — external, re-verify on next touch; JetBrains ACP co-launch broadening the protocol beyond Zed; McpServer Unix-socket server mode in-tree w/ zero production callers; subagent depth policy movement (community depth-PRs closed unmerged; defaults-as-governance: ask_user off by default 2026-08-21); per-release behavior-diff evals judged by editor-disruption bar
|| Junie adds (sec24): open weights, closed harness (Mellum2 12B MoE open; Junie CLI proprietary with a distribution-only GitHub repo); on-device flagship gated to M5 + 64GB hardware; Auto brave mode delegates the trust boundary to a safety classifier || Junie detail adds (sec24): IDE-as-MCP-server pattern (bundled since 2025.2; six external clients auto-config; router-only tools; brave mode) spreading beyond JetBrains; opt-in MCP-passing defaults flipping (use_custom_mcp default true, use_idea_mcp default false); SWE-Rebench ranks are per-cycle snapshots (61.6% headline vs 61.8% next cycle); cloud Junie multi-agent mode depth UNVERIFIED; Air public control-plane API UNVERIFIED; M5+64GB-class hardware floors for on-device agents; LEDGER NOTE: matrix column misalignment found+fixed (Warp/Droid text sat in JetBrains col-21 cells MCP-client + Sessions) — recommend Warp-column L28/L31 re-audit
|| Warp adds (sec25): open client, closed control plane (AGPL terminal vs proprietary Oz; even self-hosted execution keeps the control plane Warp-managed); OpenAI sponsoring the repo of a multi-harness orchestrator; Factories role-teams as the factory pattern spreading; detail-agent adds: Oz→Automation Platform hard rename deadline 2026-10-06; Factories EA→GA + 20% PAYG markup economics; ACP support (#9233) making Warp a full neutral host; OpenAI sponsorship terms (cash UNVERIFIED, single-sourced); private-replay-benchmark doctrine spreading; OSS-credentials criticism + foundation-donation pressure
| **WATCH**: — **Rust micro adds (sec26): NorviaLabs permission precedence (repo allow rules ignored, deny always honored) is the cleanest trust boundary in the study; expect it to propagate upward.  iter19 — io-harness NOT FOUND on GitHub (renamed/private/deleted; re-verify the source list before publication); epic-harness ALIVE (18★, pushed 2026-09-11, Homebrew tap; self-evolving skills at the CLI-wrapper layer; 22-vs-26 skill-count drift); dormant pi Rust ports (N3mes1s/pi-rs, JoNil-Botta/bit); jxors/tiny-agent-loop stale since 2026-05; DiscreteTom/tiny-loop ARCHIVED (minimal loops abandoned ~6mo in — the fate harness-rs is outrunning); harness-rs solo profile = well-documented single-committer 0.0.x, the classic brilliant-abandoned-project shape — study material, not infrastructure; forge doc-drift watch (rusqlite 0.39-vs-0.40 ADR drift, stale 324-test badge, Lattice 19-language claim); guard verdicts are model-relative (harness-rs measured a guard that helps gemini lose a task on qwen — benchmark any default-on guard across >=2 models).**
| **WATCH**: — **Hermes/Auto adds (sec27): the meta-harness pattern (Hermes driving Claude Code/OpenCode as subordinate CLIs) plus serverless backends that hibernate between sessions. iter21 adds: prime-agent-vs-hermes benchmark rivalry (their paper claims wins over Hermes); HKUDS nanobot + OpenHarness successor gravity (AutoAgent heir INFERENCE-only); zjunlp/SkillX planning/functional/atomic skill taxonomy (verify code completeness — "Planned Release" still up); SkillRL training-time co-evolution (revisit only if RL post-training); NousResearch/hermes-agent-self-evolution bridge; Nous $1.5B round closure (UNVERIFIED-closed as of 2026-09-12) + CVE-2026-9350 (Batch Runner missing authorization, public exploit).**
| **WATCH**: — **Continue/Cline adds (sec28): Cline is converging on Hermes-class surfaces (cron, messengers, teams) through SDK lifecycle hooks for policy enforcement; the closed JetBrains client marks where the moat moves.; iter20 adds: fork-vacuum persistence (no credible successor ~3 months post-shutdown; biggest lineal descendant = the PearAI fork whose cloning triggered the 2024 scandal); orphaned-install-base decay (4,125,178 installs @ live v2.1.0 listing, no telemetry/auth/maintained hub); acquisition record stays corroborated-but-no-primary (CEO LinkedIn + multiple outlets, still no press release); SpaceX→Anysphere 60B-USD framing remains SECONDARY-SOURCED (8-K not retrieved)**
| **WATCH**: — **T-bench/Harbor adds (sec29): the RL-environments pivot; when eval harnesses generate rollouts, the harness that measures agents becomes the harness that trains them. Sweep adds (sec29-30): TB board velocity itself (2.1 May 6, 3.0 Jul 30, 4.0 Aug 28 front-page, live 2026-09-12) + Continuous Benchmarks doctrine (benchmarks are software); TB-Science 0.1 (570 stars) + Harbor-Index as new measurement surfaces; SWE-bench Verified model-centric default (all-models-through-mini-SWE-agent; Opus 5 ~97% per Vals.ai); AHE/agentic-harness-engineering (882 stars, arXiv 2604.25850) observability-driven automatic harness evolution as the self-improving-harness research tier.**
| **WATCH**: — **Gemini CLI adds (sec31): open-gemini-cli fork ecosystem (IndenScale / limkcreply / positron-code / omni-cli) as the community successor lane; caretaker-agent + SSR Agent + pr-generator (a repo hiring its own maintainers) as the autonomous-maintenance pattern; a2a-server + @google/gemini-cli-sdk as the A2A/SDK OSS reference; Antigravity CLI governance maturation (1.0.13 stricter → 1.0.14 subagent auto-proceed arc); original-team exit reason (rotation vs reassignment) UNVERIFIED.**
| **WATCH**: — **jcode adds (sec32): swarm DAG task-graph completion (comm migration "in progress"); multi-session client surfaces (Proposed, not built — would strengthen the RAM-amortization story); jcode-bench "uncontaminatable benchmark" validation; 20-releases/mo solo cadence sustainability.**
| **WATCH**: — **Elicitation adds (sec33): mechanistic tool-propensity controls (Probe&Prefill −48% calls @ −1.7% acc; residual-stream steering 0→90% monotonic) reaching commercial API surfaces; fleet-size-as-effort knobs spreading beyond grok-4.20-multi-agent; GPT-6 Astra delegation posture + verbosity/effort decoupling evolution; model-launch counterweight prompt patterns (leak UNVERIFIED, directionally supported — gate on our own evals); per-model profile registry drift (effort-support grids fragmented, churn per release — re-verify every model upgrade); joint (model×harness) evaluation standards (ETCSOVG Harness Card, variance-decomposition protocol) spreading through the eval literature.**
| **WATCH**: — **Cursor adds (sec19): real-time RL (~5h cycles) converging the serving harness into the training loop; SpaceXAI model concentration (Grok joint training, 1T+ Colossus-2 model); OpenAI cutoff 2026-11-12 as precedent for change-of-control model-supply risk; self-hosted-worker partner ecosystem as the compute-plane abstraction; computer-use subagent standard.**
(librarian in flight); model-context-protocol Tasks extension adoption. Roo adds: ZooCode community-fork viability; Roomote source-available cloud-agent model; Kilo flat-subagent-hierarchy vs orchestrator outcomes (RESOLVED sec11: coordinator deprecated - flat by default, subagent_depth opt-in). Anaconda-stewardship pace; gastown beads-protocol adoption; Swarm/Agent-Teams/teams messaging convergence; OpenKilo reverse-plugin; @anthropic-ai/sandbox-runtime as common sandbox base. Pi adds: pi-as-infrastructure componentization (pi-tui 5.12M dl/wk; publish our crates standalone); OpenClaw-style runtime vendoring/absorption; pico micro-harness direction (pi agent docs); oh-my-pi IDE fork (IDE demand); TerminalBench cheating crackdown (eval adapter must avoid skills-injection); Earendil RFC-0015 stewardship model (MIT-core-forever + trademark + Fair Source/DOSP products). || Grok adds (sec15): Grove NFS/FUSE projected working-tree clones; /goal + workflows + scheduler agent-OS convergence; Claude-settings-compat absorption war; managed enterprise layer (requirements.toml, signed allowlists, OTel schema v1); kitty-graphics-protocol TUI surfaces; grok-4.6 model drift ahead of harness news. || Amp adds (sec17): agent-substrate/substrate Actor Teleport (sub-second suspend/resume, 30x oversubscription microVM+gVisor; Amp forked 2026-08-31) as orb-substrate economics; Puck meta-agent + Slack (orchestrator that spawns/monitors/archives — agent-OS convergence); agent-to-agent messaging + self-scheduling + event-driven orbs (agents-as-services); portals (HTTP surfaces into orb state, annotated screenshots feed agent) as verification surface; amp.admin.compatibilityDate enterprise migration gate (pattern for our schema-migration escape hatch); zero-markup credits + ChatGPT-subscription-linking economics; Amp Insiders community program + Amp Labs FDE arm. || Qwen adds (sec18): daemon qwen serve 20-part spec-grade deep dive as the public reference design for daemon/ACP architecture; IM channels runtime (Telegram/WeChat/QQ/DingTalk/WeCom/Feishu/GitHub/GitLab on one ACP child) -- Western harnesses absent; model-harness co-design moat (qwen3_coder XML tool format built for CLI scaffolds; Coder-Next trained ON harness trajectories incl. Qwen Code; Fig-3 limited cross-scaffold transfer = harness-specific training data is a moat); Qoder split (closed spec-driven sibling w/ privately RL-tuned Qwen-Coder-Qoder model -- Alibaba ships two terminal harnesses); vendor-vs-official TB replication slice + Scaffold-Effect extension (Qwen Code missing as harness arm); LLM hooks (prompt-executor model-evaluated permission layer); all-squash governance regime (0 merges since 2026-05; "320+ PRs/week" UNVERIFIED from git). || Cursor adds (sec19): user-satisfaction-proxy reward signal for model routing (next-turn behavior telemetry); xAI Grok as house price-efficient model concentration [deepened 2026-09-11 rewrite: real-time RL ~5h cycles harness->training-loop convergence; see dedicated sec19 lines above] || Copilot adds (sec20): Agent HQ aggregator re-centralizes the harness layer (third-party engines rentable on the default code host); preview-to-sunset churn (Knowledge Bases Nov 2025, gh-copilot Oct 2025) || Antigravity adds (sec21): no-Windows sandbox (namespaces/Seatbelt only) while our users are on Windows; Google-account blast radius (TOS suspension fears); Knowledge auto-memory flakiness on Windows || Antigravity detail adds (sec21): Forced-Descent patch status genuinely CONFLICTED (CSA: fixed Feb 2026 vs Mindgard blog May 2026 still unaddressed vs axonbuild Aug 2026 no-fix UNVERIFIED) - re-check on next touch; Teamwork falsifiable claims (Lean-verified Knuth proof, Eigen/ParlayHash merges) VERIFY before amplifying; AA independent SDK-harness index (15-25M tokens/task, TB version sensitivity 87%->15%); community A2A wrapper as unofficial interop while MCP-server mode stays absent; Gemini Flash cadence (3 in 6 weeks) + 2027-01-01 price doubling; artifacts-as-contract spreading across closed vendors || Devin adds (sec22): SWE-bench Verified formally dead (OpenAI: 59.4% flawed audited tests + training contamination); sidekick cost-routing spreading; agent confidence identical whether right or wrong — verification debt is the real price

## 4. Iteration queue (one harness per iteration, several agents each)

1. **Deepseek Harness** ← COMPLETE 2026-09-11 (iteration 1; sec6+sec8; queue marker fixed at audit — findings landed day one)
2. Cline (VSCode ext: checkpoints, modes, races)
3. Roo Code (mode scoping origin, checkpoint trees)
4. Kilo Code (fork lineage deltas) <- COMPLETE 2026-09-11 (sec11; repo Kilo-Org/kilocode, rebuilt on OpenCode server, Anaconda exit 2026-07-15)
5. Mentat (hunk staging origin) <- COMPLETE 2026-09-11 (sec12; repo renamed AbanteAI/archive-old-cli-mentat + archived; CLI deprecated 2025-01-07; MentatBot pivot also dead)
6. Plandex (plan-file architecture) <- COMPLETE 2026-09-11 (sec13; company wound down 2025-10-03, cloud dead 2025-11-07, OSS unmaintained since; founder Dane Schneider -> Promptfoo -> OpenAI via acquisition 2026-03-09)
7. Pi / Claw Code (minimalist loops) <- COMPLETE 2026-09-11 (sec14; earendil-works/pi ACTIVE 104k stars pushed today; earendil-works era since v0.74.0 2026-05-07; OpenClaw 389k stars vendored pi runtime; claw-code = leak clean-room ecosystem instructkr->ultraworkers, agent-managed exhibit)
8. Grok Build (Rust TUI + extensions, Apache-2.0) <- COMPLETE 2026-09-11 (sec15; xai-org/grok-build ACTIVE source-drop mirror, 26.7k stars, Apache-2.0; OSS-damage-control release after July exfil scandal; matrix col added)
9. Factory Droid (closed-source commercial CLI) <- COMPLETE 2026-09-11 (sec16; matrix col added; queue premises CoRIM / git-droid / Sourcegraph-roots UNVERIFIED or false; Amp remains item 10)
10. Amp (Amp Inc. / Amp Frontier Corporation - spun out of Sourcegraph 2025-12-02) <- COMPLETE 2026-09-11 (sec17; closed source; Amp column added to matrix; spinout verified via SG blog + HN 46124649)
11. Qwen Code (gemini-cli fork deltas) <- COMPLETE 2026-09-11 (sec18; premise CORRECTED by git: NOT a clean v0.8.2 re-import -- parentless Alibaba root 2025-06-26, gemini-cli graph grafted 2025-08-02 via 3 stitches incl. true merges carrying original gemini-cli-robot commits, LAST upstream sync v0.8.2 2025-10-23 = independence ~11mo; 27.8k stars Apache-2.0 ACTIVE, 9,628 commits; OAuth free tier 2k/day discontinued 2026-04-15 w/ 100+ days of cleanup ghosts; Auto Mode classifier + daemon qwen serve + Agent Arena + cache-preserving fork subagents; Qoder = closed sibling harness; matrix col corrected)
12. Cursor agent-mode deeper (shadow workspace mechanics) <- COMPLETE 2026-09-11 (sec19; premise SUPERSEDED: production parallelism went git worktrees + cloud VMs, not shadow LSP windows; deep-dive: Temporal cloud agents 50M actions/day + Firecracker microVMs + outbound-only self-hosted workers + OIDC socket, run modes + sandbox + hooks + skills, MCPoison CVE-2025-54136, CursorBench/Cursor Blame + Bugbot 78.13%, SpaceX close 2026-08-14 + OpenAI cutoff 2026-11-12; matrix col corrected)
13. Copilot Workspace / GitHub Copilot agent <- COMPLETE 2026-09-11 (sec20 REWRITTEN; scout + architecture/security/adoption detail agents + synth: ONE closed harness across github.com cloud agent (Actions substrate, copilot/* branches, approval-gated CI, 59-min cap) + CLI GA 2026-02-25 + VS Code/JetBrains GA + Copilot app preview + Agent HQ third-party engines; MXC cross-OS local sandbox (Seatbelt/bwrap/AppContainer + Intune/MDM; vendor warns pre-security-boundary) + Azure Container Apps cloud sandbox; /fleet dependency-graph subagents + two-level completion + /rewind timeline + 0.80/0.95 compaction + OTel GenAI + gh-aw safe-outputs; security record: CamoLeak CVE-2025-59145 + Comment-and-Control ( bounty, known-architectural-limitation, no CVE) + content-exclusion agent gap; April 2026 signup freeze -> AI Credits usage billing + 12-model retirement churn Jul-Oct 2026; MAI-Code-1.1-Flash first-party on Azure in GitHub tenant; matrix col corrected (7 score flips: PTY ○->●, tiered eviction ○->◐, subagents ◐->●, checkpoints ◐->●, cost HUD ◐->●, multi-client ◐->●, telemetry ◐->●))
14. Antigravity (closed - best-effort) <- COMPLETE 2026-09-11 (sec21; scout + 3 detail agents + synth: ONE shared closed harness across ~6 surfaces (2.0 desktop+Web v2.13.0, VS Code-fork IDE v2.5.5, agy CLI v1.2.0, Python SDK v0.1.16, Gemini API managed agent, extensions); action(target) permissions + Terminal Sandbox + subagents w/ permission bubbling + commandExecutionPolicy ladder; artifact-centric UX + Teamwork//boost orchestration (successor-handoff, DeepCoder/DeepInvestigator); A2A community-wrapper only; MCP Store + Ask-default; security record: Mindgard Forced Descent CONFLICTED patch status + Pillar fd -X pre-evaluation-trust RCE (no CVE); quota whiplash Nov 2025 -> Sep 2026 429 wave + undocumented caps; Gemini 3.x co-design (3 Flashes/6wk); matrix col corrected (8 score flips; recount 10-8-7))
15. Devin public postmortems <- COMPLETE 2026-09-11 (sec22 REWRITTEN; scout + architecture/models/enterprise detail agents + synth: Brain/Devbox VM-per-agent split (brain always cloud-side, even Dedicated VPC) + Desktop tab + Devin Local (ex-Windsurf, Cascade forced migration w/ conflicting EOL dates) + Outposts self-hosted workers (queue-claim self-heal, Rust MIT k8s operator) + Fusion lead/sidekick cache-aware routing at compaction points; knowledge loop DeepWiki/Playbooks/Knowledge-Suggestions; MCP server mcp.devin.ai w/ gather fan-in + Marketplace->Plugins governance (SHA pinning, required/optional/forbidden); models: SWE-2 = Kimi K3 2.8T post-train 50.0% FrontierCode 1.1 Main within 1pt of Fable 5.1 at 64% cheaper, Pareto-slope cost penalty single-run-all-efforts; Security Swarm Agentic MapReduce 72% vendor bench; Series E  @  2026-09-08 (a16z+Accel; -> run-rate); metering whiplash ACU->messages->ACU-only; matrix col corrected (9 score flips: permission tiers ◐->●, windowed ◐->○, PTY ○->◐, tiered eviction ●->◐, MCP client ◐->●, MCP server ○->●, encryption ◐->●, multi-client ◐->●, evals ●->◐))
16. Zed agent panel (Rust in-editor) <- COMPLETE 2026-09-11 (sec23 REWRITTEN; scout + architecture/archaeology detail agents + synth: open-core Rust monorepo dual GPL-3.0/Apache-2.0 per-crate @ v1.19.2 SHA a9cdfc9; native agent IS itself an ACP server against the editor host — native+external indistinguishable; ACP prototyped 2025-07-09 w/ Claude Code E2E 2025-07-18 ~6wk pre-public-launch, v1 schema in-tree, 5 SDKs, registry + JetBrains co-launch; Parallel Agents Apr 2026 flag→GA 8wk; MAX_SUBAGENT_DEPTH=1 NOT scout ≤4; Seatbelt/Bubblewrap/WSL sandbox; tool_permissions precedence chain + profile scoping; host-side GitStoreCheckpoints uniform native+external; single summarize-compaction NO ladder; plain SQLite threads.db; cost HUD no caps; no repo-map (grep 0); per-role models + refusal fallback; eval-cli + shared e2e macro + zeta eval + Agent Metrics; Zeta→edit_prediction reorg 2025-12-04 + license-scoped data collection + consent restored after silent regression; assistant2→agent rename 2025-04-01; weekly v1 train (44 tags/4.5mo) + guild governance workflows + ~1 revert/month; Delta app Aug 2026 external + DeltaDB CRDT capture; "Zed Cloud" nonexistent (SSH remote only); matrix col corrected — 8 score flips: OS sandbox ○→●, PTY ○→●, tiered ●→◐, subagents ◐→●, auto-skill ○→◐, MCP-server ○→◐, encryption ◐→○, cost HUD ●→◐)
17. JetBrains AI assistant / Junie <- COMPLETE 2026-09-12 REWRITTEN (sec24; scout + 2 detail agents (architecture / community+economics) + synth: GA Jun 17 2026 one-engine convergence (AI Chat + Junie window + CLI on rebuilt-ACP integration); AI Credits 3/10/35 tiers @ 1 credit = $1 + documented burn backlash; plan artifacts .junie/plans w/ Confirm gate; agentic debugging via debugger MCP toolset incl. decompiled JAR symbols; remote control; Junie CLI beta LLM-agnostic (Junie/BYOK/Custom/Proxy, eval-pinned aliases); Junie Local free on-device Qwen3.6-27B @ M5+64GB floor w/ int8-prefill + speculative decoding; Air agent-agnostic ADE (worktree/Docker/cloud-preview isolation); IDE-as-MCP-server bundled since 2025.2 + 2026.2 opt-in MCP passing to external ACP agents; ACP Agent Registry co-launched w/ Zed; SWE-Rebench #1 61.6% snapshot + model-profiles counter-post; matrix col corrected (8 score flips + 2 misalignment cells fixed))
18. Warp AI agent (terminal-native) <- COMPLETE 2026-09-11 (sec25 REWRITTEN; scout + architecture/archaeology/community detail agents + synth: source clone @ 4143c09 verified 100% Rust client dual MIT(warpui)/AGPL-3.0; repo = continuous mirror of private warp-internal (repo-sync.yml, Jira IDs in public commits, squashed root 0dbd3d5 +1.43M lines, 64,962 stars); blocks + real-PTY TerminalDriver + cloud-split SSE/protobuf inference; OSC 777 cross-harness listener (10 harnesses, plugin marketplaces, Claude/Codex/Gemini harness runners w/ server-transcript resume); typed allow/deny permission REASONS + AIExecutionProfiles; Automation Platform (ambient agents, cron schedules, worker host IDs, task-message A2A inbox, envelope secrets, OIDC federate) + Factories EA (closed); Drive + full-source embeddings w/ Merkle sync + skills hot-reload + Agent Memory versioned stores; rmcp 1.6 MCP client; wgpu core → GUI/TUI/web-WASM/CLI; privacy asymmetry (telemetry-mandatory Free tier) + governance (spec PRs, Oz auto-review, CLA discrepancy); benchmarks TB 52%→61.2% last submission 2025-12-12 + SWE-bench 71% reproduced/75.8%, NO 2026 refresh; pricing Build $20/1500cr API-par, 3-bucket credits w/ platform agent-hours on ALL cloud runs; OpenAI founding sponsor (cash UNVERIFIED); matrix col corrected (9 score flips, recount 15●/8◐/2○); Oz rename deadline 2026-10-06)
19. Rust micro-harnesses: NorviaLabs/forge, adulari/forge, harness-rs,
    swiftide, kwaak, autoagents <- COMPLETE 2026-09-11 (sec26 REWRITTEN iter19; scout + 2 detail agents + synth: adulari/forge AGPL-3.0 v2.15.0 = 264,762 LOC/18 crates/3,204 test fns (badge "324" under-claims) — model_loop guard lattice (553x prose spiral, verification gate = fresh evidence newer than last mutation), permission broker (deny floor > bypass, per-segment shell parse, multi-line floor, SideEffect::External), Model Mesh zero-LLM-call routing (conservation complex+code_heavy=0.0 while healthy, QuotaPace, rank-verbatim failover, 75s RL wait-out), 14 ADRs + 56 archaeology notes + doc-sync test + retraction-in-place benchmarks (bridge claim retracted as accounting bug) + 83-cell SWE-bench validity ledger + RSS-bounded LSP; liliang-cn/harness-rs MIT 0.0.63 = 15-crate framework (DESIGN ~20 merged: crate boundary must name a dependency) scoring 45/50 — 32-event hook bus, 5-stage compactor + ModelBacked + PostCompact{before,after}, honest Isolation::{None,Changes,Process} x5 backends, pass^k + leave-one-out guard ablation + trigger coverage, closed SkillDistiller/SkillReviser loop, PlanDelta::Add dynamic replanning + resume(), solo 0.0.x risk (tools crate 90 dl); also-rans: epic-harness ALIVE, io-harness NOT FOUND, pi-rs/bit DORMANT, tiny-loop ARCHIVED; matrix Rust-micro col corrected (18 cells))
20. Hermes / "Auto Agent" (pending librarian identification) <- COMPLETE 2026-09-11 + ITERATION-21 DEEP-UPGRADE COMPLETE 2026-09-12 (sec27 REWRITTEN; scout + 2 detail agents [hermes-architecture @ HEAD 284d220 / wave-deaths git-archaeology @ 16c12b0] + synth: hermes-agent 244,644★/50,720 forks MIT — Pantheon window ~5,800 commits/~2,475 PRs/760+ contributors, v0.21.0 2026-08-31; self-improving loop = 5 source-verified mechanisms, knowledge-level not weight-level; 28 messaging platforms (brief 22 = undercount) + A2A v1.0 + voice/wake-word + Kanban + MCP 2.x + 8-layer security + 7 sandbox backends + scale-to-zero; AutoAgent DEAD — 3-week functional life, ~11mo dormant, authoring oversold as self-improvement, nanobot successor INFERENCE-only; straggler tier: prime-agent/EvoSkill/SkillClaw ADOPT, SkillRL/SkillX WATCH, OmniAgent AVOID-flagged; Nous $1.5B talks UNVERIFIED-closure; matrix col corrected (10 flips + 15 enrichments, recount 17●/6◐/2○))
21. Continue.dev, Cline-adjacent OSS (bike-shedding check) <- COMPLETE 2026-09-12 REWRITTEN (sec28 Iteration-20 findings; scout + 2 detail agents [architecture @ frozen HEAD 5522c6f / endgame+economics] + synth: Continue = CLOSED CHAPTER — Cursor acqui-hire ~2026-06-16 (banner-only, no terms; SpaceX→Anysphere 60B context SECONDARY-SOURCED), final v2.0.0/v2.1.0-vscode 2026-06-19 stripped telemetry+auth, hub dead July 15 (NXDOMAIN; 4,125,178 installs orphaned), repo read-only-not-archived, JetBrains community-demoted, team dispersed (Sesti→Cursor, Romney/Erichsen→OpenClaw, Metcalf drove M&A); frozen arch = shared TS core + React GUI over VS Code / JetBrains-native-binary-IPC / cn Ink-v6 CLI + config.yaml Zod blocks/rules/context-providers + 4 modes + CLI normal/plan/auto + 79 providers + cn serve/checks/review Continuous-AI pivot; endgame = OSS extension economics + competitive squeeze + hub-as-kill-switch + 5.1M-USD capital vs giants; fork vacuum (top fork = PearAI 121★); matrix col corrected (3 flips: prefix-cache, WASM, evals); Cline deep autopsy lives in sec9)
22. **Terminal-Bench/harbor leaderboard surface (new harness discovery channel) <- COMPLETE 2026-09-11 (sec29; Terminal-Bench = frozen v1 dataset ~100 tasks + tb CLI + terminus agent + Core 0.1.1 leaderboard, ICLR 2026, repo moved to harbor-framework org; Harbor = 5.1k stars official harness for Terminal-Bench 2.0: agent adapters (Claude Code/OpenHands/Codex), dataset registry (tbench@2.0 + SWE-Bench + Aider Polyglot), cloud sandboxes (Daytona/Modal/n-concurrent 100), RL rollout generation; matrix col added; SWEEP-UPGRADE 2026-09-12: sec29 rewritten — TB 2.1 (May 6, fixes 28 / 26 Z.ai-verified, arXiv 2601.11868, submissions CLOSED maintainer-run), harbor 5,146 stars + Hub + run-upload-PR + harbor-index, TB-Science 570 stars, Hacks column, 2.0-2.1 delta Opus4.6/ClaudeCode +12.1pts, rosters 2.0 Codex/ClaudeCode/GrokBuild/mini-SWE-agent + 2.1 Terminus2/CursorCLI/GeminiCLI, SWE-bench Verified model-centric; LIVE: front-page board already TB 4.0 Aug 28 (sec29.7))
23. HN/awesome-lists sweep for stragglers → then STOP criterion applies <- COMPLETE 2026-09-11 (sec30; HN Algolia + e2b sweep; stragglers: Jules (Google async cloud PR agent), Zerostack (Unix-philosophy Rust), Juggler (CRDT sessions + P2P multi-client), fx, DeepSeek reasonix, AlphaEvolve (out of scope); zero matrix gaps; STOP criterion satisfied; SWEEP-UPGRADE 2026-09-12: sec30 measurement-tier additions Terminus 2 / mini-SWE-agent / AHE 882 stars; verdict refreshed — Gemini CLI + jcode deep-dived into sec31/sec32, remaining must-dos 0, Juggler/Bullet/Ante/Zot mention-only)
24. **FINAL PHASE — LLM behavior elicitation** (user m0062): multi-agent research <- COMPLETE 2026-09-11 (model-profiles.md created: 8 behavior axes + 10 elicitation techniques + 14 per-model profiles + launcher-balancing rules R1-R6; sources: Anthropic harness-design 2026-03 + effective-harnesses 2025-11 + aider edit-formats + OpenAI Codex guide + Gemini thinking docs + sec15/sec16; FINAL PHASE DONE; QUEUE EXHAUSTED) + FINAL-PHASE MULTI-AGENT RUN COMPLETE 2026-09-12 (sec33; scout + 3 detail agents [provider-control-surfaces / tool-call+delegation-calibration / harness×model+drift-determinism] + synth: per-family control matrix + 400-trap catalog, per-model behavioral profiles, joint model×profile eval doctrine, SET/PROMPT/THROTTLE playbook §33.6, drift+consistency gates)
    on per-model behavioral differences and how to extract the most from each:
    (a) documented behavior axes: verbosity, reasoning/thinking depth + token
    budgets, subagent-spawning vs do-it-yourself tendencies, tool-calling
    reliability, instruction drift over long contexts, edit-format competence;
    (b) elicitation techniques per behavior class: prompt shaping, tool-design
    compensation, budget/thinking controls, architect/editor hybrid pipelines;
    (c) how current harnesses adapt today: aider per-model edit formats
    (whole/diff/udiff per model), Anthropic model-specific harness notes
    ("context anxiety" in Sonnet 4.5, gone in Opus 4.6; effort-scaling rules
    to stop 50-subagent spawns), hashline anchors rescuing weak editors
    (6.7%→68.3%), reasoning-token handling (o-series/deepseek-reasoner),
    verbosity shaping (output styles); (d) outputs feed model-profiles
    behavioral axes + skill per-model guidance + launcher balancing.
25. **Gemini CLI post-cutoff deep-dive (consumer-OAuth-kill bifurcation)** <- COMPLETE 2026-09-12 (sec31; scout + 2 detail agents [git archaeology @ HEAD 9c1b0a610534d6f8120964cf2672c07807d8fc90 / current-state features @ same HEAD] + synth: consumer tiers cut 2026-06-18 NO grace, enterprise+API-key-only Apache-2.0 maintenance, Antigravity CLI = closed Go successor, ~94% velocity collapse + original team gone, 28-fix security-hardening sweep (CrUX key leak, a2a-server RCE, NTFS 8.3 bypass, MCP-OAuth SSRF), caretaker/SSR/pr-generator self-maintenance, Qwen-fork divergence = 59% of repo history upstream-only; §1 Gemini CLI column corrected — 16 flips + 9 enrichments, recount 18●/5◐/2○)
26. **jcode autopsy (RAM-efficient solo phenomenon)** <- COMPLETE 2026-09-12 (sec32; scout + 1 detail agent + synth: 1jehuang/jcode 19,520★ Rust MIT v0.84.0, solo bus factor CONFIRMED 7,284/7,323 commits, engineered-organic Reddit-first growth (in-repo subreddit dashboard + daily star-chart cron), client-server daemon RAM architecture VERIFIED in code + most-complete public self-dev loop w/ guardrails, RAM claims directional-only (honest methodology, zero independent reproduction), r/opencodeCLI secrets criticism partially substantiable; §32.4 standalone matrix note — NO new column)

## 5. Iteration log
- **FINAL PHASE — LLM-behavior elicitation**: COMPLETE 2026-09-12 (scout + 3 detail agents [providers: primary-doc fetch of Anthropic/OpenAI/Google/xAI control surfaces / tools: tool-call + delegation calibration (4× arXiv-2026 + vendor docs + Codex issues) / harness: joint-evaluation + drift-determinism papers + production per-model variant checks] + synth; §33 new). HEADLINES: "effort" means four different things across families (Anthropic = token posture, cache-coupled; OpenAI = thinking depth, cache-decoupled; Google = advisory ceiling; xAI = depth or agent-count 4/16); "off" exists on 3 surfaces only; every family has a 400-trap list → feature-detect per model, not per provider; knowledge–action gap is the core tool-calibration fact (AUROC 0.89–0.96 latent vs unacted; prompt-only suppression provably coarse; steering −48% calls @ −1.7% acc); delegation eagerness is a family property (Claude eager-50-spawns vs Codex explicit-only w/ silent degradation vs xAI agent_count billing knob); token budget explains 80% of multi-agent outcome variance at 15× cost; harness variance ≥ model variance (7.80×, 6/9 ranking flips; 40× token spread at equal pass; failure fingerprints replicate across models = harness properties); models co-trained with native tool shapes → route edit-format per family (apply_patch ~50% failure off-family); temp-0 ≠ deterministic (24%/9% unique outputs; serving-stack systemic); 60% of divergence in first two steps → TSS golden-prefix tripwire (~3× failure odds when deviant); Pass⁵ 53% vs Mean@5 77% → gate on both; DELIVERABLE: §33.6 SET/PROMPT/THROTTLE playbook feeding model-profiles behavioral axes + todo-34 folds.
- **Iteration 22 — Gemini CLI (consumer-dead, enterprise-alive)**: COMPLETE 2026-09-12 (scout + 2 detail agents [git archaeology @ HEAD 9c1b0a610534d6f8120964cf2672c07807d8fc90 — all SHAs rev-parse-verified / current-state features+docs @ same HEAD] + synth; §31 new). HEADLINES: bifurcation — consumer tiers (AI Pro/Ultra, free Code Assist, Code-Assist-for-GitHub) cut 2026-06-18 with NO grace period (docs banner + discussions/28017 + /27274; kill shipped via docs/announcements — no identifiable commit); Apache-2.0 repo continues enterprise-license + paid-API-key-only (148 post-cutoff commits vs 2,519 pre = ~94% velocity collapse); Antigravity CLI = closed-source Go successor (no LICENSE at capture, curl-pipe-bash installer, auth breakage #415; auto-migrates Skills/MCP/Agents/GEMINI.md, converts extensions→plugins); OpenCode-rewrite folk myth REFUTED (contiguous google-gemini DAG, native gemini-cli-robot release trains, no OpenCode ancestry); governance: ~11/20 named post-cutoff authors carry @google.com emails, original core team ~0 post-cutoff commits (exit reason UNVERIFIED), repo partially self-maintains (caretaker-agent born Jun 30 2026 7f00c5fe5 #28015, SSR Agent issue-fix commits, pr-generator-* Cloud Run bug-fix-PR state machine c5622fec2/b6b41f79e); security arc ~40% of post-cutoff commits — six clusters: prompt-injection-as-filesystem-attack (bfb71fd2b/c647533d6/acae7124b), MCP-as-hostile-IdP (SSRF 3c311beac + RFC 9207 55b495d6d + fail-closed 0bd1d4397), Windows NTFS 8.3 SFN bypass 24cab6830, symlink/escape wave, OS-enforced containment (Seatbelt/Docker isolation), hardcoded CrUX key 87a9c71d5 + cruxKeyLeak.test.ts) + a2a-server RCE c776c665b; FINAL feature state: 7-pkg monorepo (+sdk bed3eae0e Feb 2026, +devtools), 5-backend sandbox + bwrap tool-level + Sandbox Expansion Requests, policy engine + trusted folders, remote A2A subagents, staged eviction ladder, OTel; release engineering: Tuesday stable cadence tag-verified (v0.59.0 Sep 8), nightlies embed HEAD SHA (v0.61.0-nightly.20260908.gc647533d6); fork divergence: v0.8.2..HEAD = 3,794 commits = 59% of repo history post-Qwen-base — SDK/policy-engine/caretaker/hardening-wave all upstream-only. Deltas: ADOPT sandbox taxonomy + policy-engine + eviction ladder + consent strings + SHA-nightlies + omissionPlaceholderDetector; AVOID no-grace consumer kill, closed-successor handoff, building on zombie upstream; WATCH fork ecosystem (open-gemini-cli lane), self-maintenance pattern, A2A reference. §1 Gemini CLI column corrected in place: 16 flips + 9 enrichments, recount 18●/5◐/2○.
- **Iteration 23 — jcode (the RAM-efficient solo phenomenon)**: COMPLETE 2026-09-12 (scout + 1 detail agent + synth; clone @ 33daef65e5786130ed490feae51e5a321ec496ec; §32 new). HEADLINES: 1jehuang/jcode 19,520★/2,252 forks, Rust MIT v0.84.0, 80 crates, ~20 releases/mo verified (12 releases Aug 25→Sep 7); solo bus factor CONFIRMED (Jeremy Huang 7,284/7,323 commits = 99.5%; 8 human non-author commits ever); growth = engineered-organic Reddit-first (in-repo 15-subreddit targeting dashboard reddit_dashboard.py + daily star-chart cron — clone HEAD literally "docs: update weekly stars chart"); RAM architecture VERIFIED in code: jcode serve daemon + thin-client TUIs over Unix socket (5-min idle-out, flock-gated stale-socket reaping server/socket.rs:46-100, exec()-same-socket hot reload), append-only prefix-hash cache-violation tracker (cache_tracker.rs:1-14,55-76), on-disk MCP schema cache w/ config fingerprints (mcp/schema_cache.rs:1-31; kills documented #206 first-snapshot miss), daemon-owned resumable sessions (multi-session client surfaces still Proposed); self-dev mode = most-complete public agent-self-development loop (tools build|build-reload|test|cancel-build|reload|status, serialized per-scope build queue, CanaryStatus/CrashInfo w/ git diff, ReloadContext context carryover, #291 reload-loop/never-downgrade failure mode hit and fixed); swarm ◐ ("largely implemented", comm migration in progress) + file-touch collision alerts ● (BusEvent::FileTouch, server alerts BOTH sides on overlapping modification, server.rs:1983-2105); memory graph ◐ (cosine + local MiniLM embeddings; sidecar pinned to one small model "GPT-5.3 Codex Spark"); RAM-claims verdict: mechanism real, methodology honest (PSS, pinned competitor versions, embeddings-off asterisk IN-TABLE), directional-NOT-citable (single machine, zero independent reproduction); r/opencodeCLI secrets criticism PARTIALLY SUBSTANTIABLE (plaintext dot-env keys confirmed; 0o600 chmod + Windows ACL mitigations verified; file+chmod is ecosystem norm; no OS-keychain option of its own). Deltas: ADOPT daemon/session amortization + MCP schema disk cache + cache-violation tracker + file-touch collision alerts + self-dev guardrail set + flock-gated reaping; AVOID bus-factor-1 dependency, crate-sprawl style, uncited RAM tables, model-pinned sidecar, plaintext dot-env keys; WATCH swarm DAG completion, multi-session surfaces, jcode-bench, solo cadence sustainability. §32.4: standalone matrix note, NO new §1 column — 27 columns stand, §30.8 stop criterion holds.

- **Sweep — Terminal-Bench/Harbor upgrade + straggler consolidation**: COMPLETE 2026-09-12 (synth-only sweep iteration; scout facts + cheap live verification: harbor-framework/harbor 5,146★ pushed 2026-09-12, terminal-bench-science 570★, agentic-harness-engineering 882★, tbench.ai blog timeline). HEADLINES: §29 upgraded — TB 2.1 (May 6 2026: blog "fixes 28 tasks", 26 Z.ai-verified, arXiv 2601.11868, community submissions CLOSED maintainer-run), Harbor-as-own-org (5,146★ + Harbor Hub hub.harborframework.com + harbor run→upload→PR + adapters + harbor-index), Hacks reward-hacking-delta column (descends from Apr 19 2026 Leaderboard Integrity Update), 2.0→2.1 delta Opus 4.6/Claude Code +12.1pts, rosters (2.0: Codex/Claude Code/Grok Build/mini-SWE-agent; 2.1 adds Terminus 2/Cursor CLI/Gemini CLI), SWE-bench Verified now model-centric (default all-models-through-mini-SWE-agent; Opus 5 ~97% per Vals.ai); LIVE DELTA beyond the sweep brief: front-page board is already TB 4.0 (Aug 28 2026; after 3.0 Jul 30, TB-Science 0.1 Aug 27, Harbor-Index Jun 29, TB Challenges Jun 18) — board generations ship quarterly, pin datasets not boards. §30: +Terminus 2 (neutral reference harness, Artificial Analysis standard), +mini-SWE-agent (Princeton ~100-line bash ReAct loop, SWE-bench Verified default — the minimal-floor datapoint validating the 6-tool core), +AHE (agentic-harness-engineering, 882★, #3 TB 2.0 84.7% GPT-5.5, arXiv 2604.25850, observability-driven automatic harness evolution — self-improving-harness research tier); §30.8 verdict refreshed: Gemini CLI + jcode deep-dived into §31/§32 (being written), 0 remaining must-dos, Juggler/Bullet/Ante/Zot mention-only. Ledger sec29-30 Sweep-adds segments appended (ADOPT/AVOID/WATCH); queue items 22-23 sweep-annotated, 24 already COMPLETE.

- **Iteration 21 — the self-improving wave (Hermes deep-upgrade + AutoAgent autopsy)**: COMPLETE 2026-09-12 (scout + 2 detail agents [hermes-architecture: source @ HEAD 284d220ba48e25f2e3623b3afe72db8f24a4c2db + live API + docs / wave-deaths: HKUDS/AutoAgent depth-50 clone @ 16c12b0 + straggler-tier API sweep + Nous company state] + synth; stale §27 REWRITTEN w/ 8 subsections). HEADLINES: hermes-agent 244,644★/50,720 forks MIT pushed 2026-09-12, Python 77.3MB + TS 26.2MB + Rust bits, ~3,418 all-time contributors; Pantheon v0.21.0 body: ~5,800 commits/~2,475 PRs/~2,100 issues/760+ contributors since v0.20.0 (~4wk); teknium1 ~1,340 PRs = solo concentration risk. SELF-IMPROVING-LOOP VERDICT: real but knowledge-level-NOT-weight-level — 5 mechanisms at HEAD (skill_manage → ~/.hermes/skills/, post-turn background_review fork asking "should any skill/memory be saved?" that never touches the main prompt cache, in-use self-improvement, idle-triggered curator on aux slot, 10/15-turn nudges) + opt-in skills.write_approval staging gate; Skills Hub = agentskills.io + 5 install sources incl. ClawHub adapter (ClawHavoc 341 malicious skills → content-hash scan + quarantine + lock.json); weight-level work external: hermes-agent-self-evolution (DSPy/GEPA, human-review-gated) + Atropos RL trajectories. PLATFORM: 28 messengers (China: QQ/Yuanbao/DingTalk/WeCom/Weixin/Feishu), A2A v1.0 LF-stewarded (Agent Card + JSON-RPC + SSE + HMAC push), hermes peer bot-DMs, Kanban multi-agent, voice/wake-word (openwakeword/porcupine), native Desktop Bot Mode, mcp==2.0.0 rev 2026-07-28, 8-layer security (hardline floor survives YOLO, deobfuscated command variants), 7 backends incl. vercel_sandbox microVM + scale_to_zero, SQLite WAL SessionDB + FTS5 CJK-bigram, ContextCompressor head-3/tail-20, shadow-git checkpoints, 11 aux model slots. NOUS: $1.5B talks (TechCrunch 07-13, Robot Ventures+USV, ≥$75M) UNVERIFIED-closed; hosted $20–$200/mo + Enterprise; Hermes 4.3 post-trained on Psyche (decentralized); CVE-2026-9350 (Batch Runner missing authorization, public exploit). AUTOGENT AUTOPSY: 9,783★ not archived, dormant since 2025-10-16, last *.py commit 2025-02-28 = 3-week functional life; create_tool string-codegen = authoring automation oversold as self-improvement (nothing persists across sessions); death = HKUDS release-farm attention reallocation (nanobot 48.0k★ active, OpenHarness 15.7k★, LightRAG line durable); nanobot-successor INFERENCE-only (roadmap discussion #431 absorbs ambitions, no handoff statement found). STRAGGLER TIER: prime-agent 20,568★ ADOPT (Continual Harness H=(ρ,G,K,M) typed CRUD + /refine evidence-backed versioned edits + immutable base prompt; ARC-AGI-3 RHAE 30%→95.5%; paper claims outperforming Hermes), EvoSkill 1,181★ ADOPT (program/* git-branch frontier), SkillClaw 2,593★ ADOPT (proxy-intercept cross-agent skill pooling; HF #2-of-day nuance), SkillRL 970★ WATCH (4mo stale, training-time layer), SkillX 284★ WATCH (Planning/Functional/Atomic taxonomy; "Planned Release" caveat), OmniAgent 2,564★ AVOID (day-zero account, NOASSERTION license, star-inflation pattern). Deltas: §3 ADOPT/AVOID/WATCH sec27 segments extended; §1 Hermes/Auto col corrected — 10 flips (4◐→●, 2○→●, 3○→◐, 1●→◐) + 15 enrichments, recount 17●/6◐/2○; queue item 20 marked deep-upgrade COMPLETE.
- **Iteration 20 — Continue (the pioneer, closed)**: COMPLETE 2026-09-12 (scout + 2 detail agents [architecture: frozen-repo clone @ HEAD 5522c6f44ca0ac3528b37244818fbfa39b5af470, read-only since final code commit 2026-06-19 / endgame: acquisition + economics + community, live checks 2026-09-12] + synth; stale combined Continue+Cline §28 REWRITTEN to Continue-only — Cline deep autopsy lives in §9). HEADLINES: closed chapter — Cursor acqui-hire ~2026-06-16 via homepage banner (NO press release/terms; days after SpaceX→Anysphere ~$60B 8-K, SECONDARY-SOURCED), final v2.0.0+v2.1.0-vscode 2026-06-19 stripped anonymous telemetry + auth, hub export deadline July 15 → hub.continue.dev NXDOMAIN, repo read-only-but-not-archived, JetBrains community-demoted, eval/ emptied; team dispersal: Sesti→Cursor, Dunn out May 2026, Romney+Erichsen→OpenClaw, CEO Metcalf "drove the M&A process end to end" (LinkedIn 2026-08-24, now Goldmine); arc: YC S23, ~$5.1M ($2.1M Heavybit 2023 + $3M 2025; $7.3M claim contradicted), PearAI clone scandal Sept 2024 → YC Correcting-the-record, Metcalf CEO May 2025 (enterprise mandate), Continuous-AI pivot H2-25→Feb-Mar 2026 (.continue/agents + @continue-review + AI Checks; Jan-2026 single date UNVERIFIED); frozen architecture: one TS core (619 files) + React GUI across VS Code in-proc / JetBrains native-binary stdin-stdout / cn Ink-v6 CLI, config.yaml Zod schema (blocks uses/with/override, markdown+AGENTS.md rules, 8 model roles incl. subagent, promptCaching, plaintext apiKey), 35 context providers, 4 GUI modes + CLI normal/plan/auto, 79 providers + ~30 adapters + bundled offline MiniLM embeddings, hidden cn serve headless :8000 + checks (GitHub-API PR autodetect) + review worktree inbox; ENDGAME ECONOMICS: Apache-2.0 BYO-key extension economics (~zero capture on 34k stars/4.1M installs) + competitive squeeze (Copilot default path / Cursor IDE / Claude Code CLI; own CLI Sept 2025 late) + hub-dependency (kill switch = monetization surface) + capital/timing (deliberate capital-efficient sale, not blowup); afterlife: fork vacuum (top fork = PearAI 121★; continue-fork 8★ dead), 4,125,178 orphaned installs @ live v2.1.0, HN thread 13 pts/10 comments; best-in-class wind-down protocol recorded (telemetry/auth strip + live docs + export deadline + billing cut early); matrix Continue/Cline col corrected (3 flips: prefix-cache ○→●, WASM ○→●, evals ○→◐ + 19 enriched cells; 2 pre-existing extra-cell rows handled at count-4); ledger sec28 ADOPT/AVOID/WATCH extended; queue item 21 rewritten
- **Iteration 17 — JetBrains Junie + AI Assistant**: COMPLETE 2026-09-12 (REWRITE of stale scout-only §24; scout + 2 detail agents + synth; primary = jetbrains.com help 2026.2 line + blog.jetbrains.com Mar–Sep 2026 + junie.jetbrains.com docs + support-forum full thread). HEADLINES: proprietary harness / open models (Mellum2 OSS); one-engine-many-surfaces — same agent behind AI Chat, Junie tool window, Junie CLI, IDE integration rebuilt on ACP at GA 2026-06-17; ACP Agent Registry co-launched with Zed Jan 2026, in-IDE registry install w/ runtime auto-management, acp.json custom agents, no WSL; Junie CLI beta 2026-03-09 LLM-agnostic: Junie/BYOK/Custom/Proxy provider classes, eval-pinned aliases, per-provider helper models, JUNIE_EFFORT dial; Junie Local 2026-08-24: free/offline Qwen3.6-27B 4-bit ~20GB, M5+64GB floor ("a big ask"), reasoning_effort none ~2x, int8 prefill patch ~40% M5 prefill gain (MLX-VLM fork), MTP+n-gram speculative decoding ~2x, rolling context = KV-cache survives across tasks; Air public preview 2026-03: agent-agnostic ADE, workspaces bind sessions/Git/tools, parallel tasks, Local/Worktree/Docker execution + cloud sandboxes tech preview, hosts Copilot/OpenCode/Pi/Cline via ACP; plans = committed .junie/plans artifacts w/ clarifying questions + Confirm gate ("plan strong, implement cheap"); agentic debugging = live IDE debugger via debugger MCP toolset on 127.0.0.1:64342/stream incl. decompiled .class/JAR symbols; IDE-as-MCP-server bundled since 2025.2 (analyze_calls incl. decompiled JAR symbols, DB toolset, router-only tools, brave mode; auto-config Junie/VS Code/Claude Code/Codex/Air/Copilot CLI) + 2026.2 opt-in MCP passing to external ACP agents (use_custom_mcp true / use_idea_mcp false); local models cannot invoke configured MCP tools (help-verified); economics: AI Credits Free 3/Pro 10/Ultimate 35, 1 credit = $1, orgs pay double, burns documented (2 credits/trivial task, Pro quota 30 min, $0.81–$1.14/task); Aug-25-2025 quota reset admitted in Sep-2025 FAQ ("We set expectations incorrectly. That is on us") + transparency tickets LLM-16701/LLM-19959 still open Sep 2026; SWE-Rebench #1 61.6%/72.7% pass@5 per Nebius quote, self-disclaimed "results move run to run", live board 61.8%/$0.81 (UNVERIFIED); from-leaderboards-to-model-profiles Aug 31 2026: Opus 4.7 skips validation 123/523 runs (68 resolved), Gemini 3.5 Flash 37.3% hallucination, GPT-5.5 51.5% best resolve + always validates, Qwen3.6 FP8 38.9% @ 3% cost; community: IDE-native praised, credit opacity = #1 complaint Nov 2025→Sep 2026 unbroken, Claude "unusable" at credit prices, Gemini Flash the endorsed cheap default, exit paths Claude Code/Codex/Copilot/Windsurf; matrix col corrected (8 score flips: OS sandbox ○→◐, prefix-cache ◐→●, repo map ◐→●, windowed ●→◐, PTY ○→◐, eviction ○→◐, model-weakness ◐→●, auto-skill ○→◐) + 2 misalignment fixes (Warp/Droid text in JetBrains MCP-client + Sessions cells); ledger ADOPT/AVOID/WATCH extended w/ Junie detail adds (sec24).
- **Iteration 19 — Rust micro-harnesses (forge + harness-rs)**: COMPLETE 2026-09-11 (scout + 2 detail agents + synth; adulari/forge blob:none clone @ 24bdcb8 verified 2026-09-12 + harness-rs clone @ 061bf3e + API-level also-ran sweep). HEADLINES: stale shallow §26 (six-project sketch) REWRITTEN — "micro" is now a misnomer for both survivors. FORGE (Adulari/forge, AGPL-3.0-only, v2.15.0, 8★, Floris Voskamp solo+2): 264,762 LOC/18 crates (not ~13k briefed), 3,204 test fns (badge "324" under-claims); model_loop guard lattice (doom-loop, sliding-window oscillation catch, 553× prose-spiral recovery, verification gate = fresh tool-grounded evidence newer than last mutation, separate rate-limit/outage retry budgets); permission broker: builtin deny floor outranks BYPASS, per-segment shell parsing (allow "git *" cannot smuggle git status && x), multi-line conservative literal floor ("newlines are where indirection hides"), SideEffect::External for MCP, plan = hard read-only, decisions persisted; Model Mesh: 8-step deterministic ZERO-LLM-call routing, conservation (complex+code_heavy = 0.0 off subscriptions while healthy), QuotaPace burn windows, quota/burn/capacity penalties w/ incident-rationale comments (Warning −5.0), rank-verbatim failover (round-robin = named regression), 75s RL wait-out, routing_decision table = ground truth; 14 ADRs (0002 single-binary clarified = partial reversal; 0003 genai vendored 0.6.5-forge.1 w/ exit clause; 0010 ts 0.26→0.24 downgrade + refs-by-name-not-dst_id; 0013 RSS-bounded LSP 3.7GiB/37procs→1.67GiB/4/1; 0014 bounded growth, 1.5GiB db incident) + 56 archaeology notes + doc-sync test; benchmarks: official SWE-bench Docker evaluator (not reimplemented), 83-cell validity ledger, "+50%" = 6/10 vs 4/10 relative at N=10, latest canonical = 2 tasks (3/6 vs 0/6 Codex, 2/4 vs 1/4 Claude), bridge-superiority claim RETRACTED 2026-06-26 as accounting bug (1.9× MORE tokens). HARNESS-RS (liliang-cn, MIT, 0.0.63, 12★, bus factor verifiably 1): DESIGN planned ~20 crates → 15 shipped ("a crate boundary has to name a dependency you might not want" — 10 merged into harness-loop, 120.7KB lib.rs); 32-event hook bus (27 planned; PostCompact{before,after}); 5-stage compactor + ModelBacked + hysteresis + never-split invariant; honest Isolation::{None,Changes,Process} w/ 5 backends (Null/Worktree/Container/Seatbelt/bwrap; in-process-fs hole disclosed); guards StuckPolicy / MonotonyPolicy (default-OFF w/ measured justification) / spill-not-truncate (−17% cost/pass) / SealSet+SealBreached / GoalStore / cancellation-as-Outcome w/ process-group kill; memory 3 layers w/ DECISION:-marker capture (noise-free) + closed SkillDistiller/SkillReviser loop; Job-DAG orchestrator w/ PlanDelta::Add dynamic replanning + per-transition JSONL persistence + resume(); evals: pass^k CI-gated + leave-one-out guard ablation + trigger coverage ("guard verdicts are model-relative": dedupe −12% on gemini but loses a task on qwen); DESIGN-vs-reality: doctrine ~80% implemented, guide names 5 nonexistent crates, Rig dropped for native providers, Node::Agent unresolved at 0.0.63, browser CDP ~190KB + user_model 74KB undocumented creep; solo risks: 63 releases/<4mo ×15 crates, tools crate 90 downloads. ALSO-RANS: epic-harness ALIVE (18★, pushed 09-11, brew tap, 22-vs-26 skill drift), io-harness NOT FOUND, pi ports DORMANT (pi-rs/bit), tiny-agent-loop STALE, tiny-loop ARCHIVED. Deltas: 21 ADOPT / 9 AVOID / 7 WATCH (§3 sec26 segments extended in place); §1 Rust-micro column corrected (18 cells); §26 rewritten in place (stale sketch replaced).
- **Iteration 18 — Warp**: COMPLETE 2026-09-11 (scout + architecture/archaeology/community detail agents + synth; blob:none source clone @ HEAD 4143c09 + depth-3000 history clone + docs-repo changelog MDX + live web/GitHub/HN). HEADLINES: open client MIT(warpui/warpui_core)+AGPL-3.0 100% Rust verified @ 4143c09; repo = continuous MIRROR of private warp-internal (repo-sync.yml; Jira IDs in public commits; squashed root 0dbd3d5 = +1.43M lines 2026-04-28; 2,368 commits ~125/wk; PRs past #15,950; 64,962 stars / 5,528 forks); terminal-as-runtime — blocks (AIBlockModel 7,338 lines) + agent-as-TerminalDriver on a real PTY + cloud-split inference (SSE/protobuf, no model loop client-side); cross-harness best-in-class: OSC 777 JSON notifications, 10 harnesses tracked (Claude/OpenCode/Codex/Gemini/Auggie/Droid/Pi/OhMyPi/Grok/WarpTui), per-harness plugin marketplaces, Claude/Codex/Gemini harness runners w/ ResumePayload transcripts; permissions typed allow/deny WITH reasons + AIExecutionProfiles; Automation Platform: ambient agents + cron schedules + worker host IDs + task-message A2A inbox + envelope secrets + OIDC federate subject templates; Factories EA (closed; agents/specs evidence); knowledge: Drive + WARP.md/AGENTS.md/CLAUDE.md-migration rules + full-source embeddings w/ Merkle-tree sync + skills hot-reload + Agent Memory versioned cloud stores; rmcp 1.6 MCP client w/ templatable multi-provider manager; wgpu 30 core → GUI/TUI/web-WASM/CLI; checkpoints 5-state coordinator; archaeology: warp-agent-staging[bot] 66% of recent commits, Oz agent UI → Warp Agent renamed 2026-08-13, oz CLI kill date 2026-10-06, HN OSS thread 373/118; benchmarks arc TB 52% (hill-climbing admitted) → 61.2% LAST submission 2025-12-12, SWE-bench 71% maintainer-reproduced / 75.8% #3, NO 2026 refresh + contamination pivot → Factory Benchmarks replay product (WarpBench $80→$30/PR self-reported); economics: Build $20 = 1,500cr API-par, Max $200, 3-bucket credits + platform agent-hours on ALL cloud harnesses (direct CLI use = zero credits), Free tier NO bundled credits + telemetry-mandatory for AI; OpenAI founding sponsor (cash terms UNVERIFIED; Wikipedia single-sourced); governance: issues-first spec PRs + Oz auto-review + CLA-in-FAQ-without-mechanics. Deltas: 12 ADOPT / 6 AVOID / 6 WATCH (sec25 segments extended); §1 Warp col corrected (9 score flips; recount 15●/8◐/2○); §25 rewritten in place.
- **Iteration 16 — Zed agent panel + ACP + Delta**: COMPLETE 2026-09-11 (scout + architecture/archaeology detail agents + synth; blob:none source clone @ HEAD a9cdfc99 = v1.19.2 + full-history blobless clone; every SHA + PR state verified). HEADLINES: native agent implemented AS an ACP server against the editor host (NativeAgentServer → AgentConnection) — native and external agents indistinguishable to the panel; ACP git timeline: crates/acp 2025-07-09 → Claude Code E2E 2025-07-18 → public launch Aug 2025 (6-7wk internal sprint), acp 0.0.6→0.15.0, v1 schema in-tree; registry re-platforms agent installation (Claude/Codex/OpenCode/Copilot/Cursor/Pi/Poolside; extension agents deprecated) + JetBrains co-launch; Parallel Agents Apr 2026 (onboarding #52940 → docs #53535; subagents flag #46186 Jan → GA #50493 Mar = 8wk); MAX_SUBAGENT_DEPTH=1 CORRECTS scout ≤4; community subagent-depth PRs #54084/#54142/#54213 ALL closed unmerged → staff re-landed create_thread as #57987 (Jun 2026) w/ guild workflows as the scaling lane; Terminal Threads Jun–Sep 2026 (panel rename landed on verification day); OS sandbox Seatbelt/Bubblewrap/Bubblewrap-via-WSL (flag-gated; Win = WSL); tool_permissions precedence chain + profile scoping + restricted-worktree downgrade; host-side GitStoreCheckpoints (acp_thread) uniform for native+external, terminal-kill on restore; single summarize-compaction (AutoCompactThreshold %/used/remaining) NO tiered ladder; plain SQLite threads.db (no encryption); cost HUD no caps; NO repo-map (grep 0; tree-sitter outlines + LSP + line-windowed reads); per-role models (default/subagent/inline/commit/summary/compaction) + refusal-fallback retry + mid-turn switch; eval-cli headless (JSON EvalResult) + shared e2e macro + zeta eval + public Agent Metrics; Zeta→edit_prediction reorg 2025-12-04 + license-scoped data collection + consent restored after silent regression #49426; archaeology: assistant2→agent rename dc83f1ad 2025-04-01 (strategy as git mv), weekly v1 train v1.0.0 2026-04-29 → v1.19.2 2026-09-09 (44 tags), ~1 substantive revert/month (sidebar rename, punctuation movement, LiveKit), 2.5yr frame-time lineage + 10ms GPUI hang threshold + Aug 2026 bench-infra burst; Delta app Aug 2026 EXTERNAL + DeltaDB CRDT operation capture as future context substrate; "Zed Cloud" nonexistent (SSH remote + provider auth only). Deltas: 12 ADOPT / 4 AVOID / 6 WATCH (§3 Zed adds extended in place); §1 Zed col corrected (8 score flips; recount 14●/10◐/1○); §23 rewritten in place; queue premise corrected (scout depth ≤4 false; Zed Cloud false).
- **Iteration 15 — Devin / Cognition**: COMPLETE 2026-09-11 (scout + architecture/models/enterprise detail agents + synth; docs.devin.ai + release-notes archaeology + primary blogs fetched in full). HEADLINES: Brain/Devbox split (stateless planning always in Cognition cloud, even on Dedicated single-tenant VPC; no customer LLM keys) + per-session Devbox VM + session Desktop tab (devinapps.com iframes; VNC UNVERIFIED) + Windows computer use (+9% ACU); Devin Outposts 2026-07-21 (loop cloud-side, execution on customer machines; named queues, claim-expiry self-heal reconciliation, devin-outpost-k8s Rust MIT operator, Namespace M5 macOS; network enforcement = operator spec.network_policy); Devin Local in Devin Desktop (ex-Windsurf renamed 2026-06-02; Cascade forced migration across 6 weeks w/ TWO conflicting official EOL dates + silent fallback); Fusion (frontier lead + cheap sidekick in parallel each w/ own cached context; model switches AT compaction = cache-free routing; up-to-60% claim revised from 35%; judgment-heavy delegation backfires); knowledge loop: DeepWiki (repo indexing, mcp.deepwiki.com) + Playbooks + Knowledge Suggestions auto-generated from chat feedback + org->enterprise promotion; mcp.devin.ai Streamable-HTTP server w/ gather fan-in barrier + Marketplace->Plugins consolidation (skills+rules+hooks+MCP+subagents, SHA pinning, required/optional/forbidden); SWE-2 2026-09-10 (Kimi K3 2.8T RL post-train, 50.0% FrontierCode 1.1 Main within 1pt of Fable 5.1 at 64% cheaper, Pareto-slope linear cost penalty via Jensen argument, single run all effort levels, 58% fewer turns than SWE-1.7); Security Swarm Agentic MapReduce (72% recall vendor bench); trustworthiness evals in-harness (null persona effect = harness suppresses persona behavior); Series E  @  2026-09-08 a16z+Accel (run-rate ->); metering whiplash ACU->messages->ACU-only; Dont-Build-Multi-Agents doctrine + 2026 partial reversal (writes stay single-threaded; Devin Review generator-verifier w/ ZERO shared context ~2 bugs/PR). Deltas: 10 ADOPT / 8 AVOID / 7 WATCH (sec22 appends pre-existing in section 3); Devin column corrected (9 score flips); sec22 rewritten in place; queue premise updated (postmortems -> full-surface platform study).
- **Iteration 13 — GitHub Copilot (full surface)**: COMPLETE 2026-09-11 (scout + architecture/security/adoption detail agents + synth; docs.github.com + changelog + copilot-cli repo SHA 1032cd05 + LOCAL BUNDLE FORENSICS of CLI 1.0.83 win32-x64). HEADLINES: one closed harness (same agentic harness as the cloud agent) across github.com cloud agent (Actions-substrate ephemeral envs, copilot/* branch confinement, approval-gated CI incl github-actions[bot], 59-min session cap) + CLI GA 2026-02-25 + VS Code/JetBrains agent modes GA 2026-03-11 + Copilot app desktop preview 2026-06-02 + Agent HQ 2025-10-28 (third-party engines rentable); two-level completion (session.idle mechanical vs session.task_complete semantic + autopilot synthetic nudge); /fleet dependency-graph parallel subagents (shared filesystem, last-writer-wins) + 6 built-ins incl rubber-duck cross-model critic; MXC cross-OS local sandbox (/sandbox enable; Seatbelt/bubblewrap/AppContainer; Intune/MDM enforcement; pty x319 + runtime.node 79MB in bundle; mxc repo itself warns profiles not security boundaries) + copilot --cloud (Azure Container Apps Sandboxes); /rewind timeline picker + file-only restore without git; 0.80/0.95 two-threshold compaction; OTel GenAI telemetry (traceparent into hooks) + actor_is_agent audit logs; gh-aw safe-outputs (read-only default + validated writes + threat detection); Enterprise AI Controls GA 2026-02-26. SECURITY: agentic security principles 2025-11-25 vs record: CamoLeak CVE-2025-59145 (Camo-proxy ASCII-art exfil; fix = disable image rendering) + Trail of Bits raw.githubusercontent.com lockfile backdoor + Comment-and-Control 2026-04-15 (one hidden-HTML-comment payload beat env-filter/secret-scan/firewall via whitelisted github.com; HackerOne closed Informative then  + known-architectural-limitation, NO CVE) + content exclusions not supported on agent surfaces (#42617; .copilotignore not native). ECONOMICS: April 2026 fortnight (trials paused 04-10 -> individual signups 04-20 -> Business self-serve 04-22 -> usage billing announced 04-27; AI Credits live 06-01, flex allotment mutable; silent downgrade to Free; 12 models retired Jul 31-Oct 2 window; MAI-Code-1-Flash deprecated after ~14 weeks; 4.7M paid subs +75% YoY -> ~140K orgs -> 50M users FY26 Q4; never-disclosed ARR est .45-1.1B; arXiv 2608.00101 production traces: retry loops 4x compute on 9% of turns, KV-cache-destroying model switches). Deltas: 12 ADOPT / 8 AVOID / 7 WATCH (sec20 appends pre-existing in section 3); Copilot column corrected (7 score flips); sec20 rewritten in place; queue premise superseded (Workspace-sunset framing -> full-surface harness study).
- **Iteration 14 — Google Antigravity**: COMPLETE 2026-09-11 (scout + architecture/trust/models detail agents + synth; docs+RE+forum only, closed SaaS). HEADLINES: ONE shared closed agent core across ~6 surfaces (2.0 desktop+Web v2.13.0 Electron, VS Code-fork IDE v2.5.5, agy CLI v1.2.0 "exact same agent core", Python SDK v0.1.16 wraps local agent binary, Gemini API managed agent antigravity-preview-05-2026 "same harness as the IDE", extensions VS/VSCode/JetBrains/Zed/Xcode + Remote Control); artifact-centric UX (Implementation Plans/Walkthroughs/webm recordings, Request Review vs Always Proceed, CLI staged-before-write Artifact Picker); Projects primitive; async sidecars + Scheduled Tasks cron + native Worktree Mode; subagents w/ context isolation + inherit/branch/share workspace modes + permission bubbling + commandExecutionPolicy off/auto/eager/sandbox; Teamwork (/teamwork-preview: Sentinel scoping interview -> Project Orchestrator -> FRESH SUCCESSOR BETWEEN MILESTONES -> Explorers/Workers, adversarial falsification + success audit, per-agent scratch dirs) + /boost (DeepCoder/DeepInvestigator workstreams, investigation=no-write, failed-assertion feedback loop, forum-confirmed); A2A NOT official (community a2a-antigravity wrapper only); MCP Store ~47 servers + unconfigured-tools-Ask default + mcp() scoping + plain-JSON OAuth tokens vs safeStorage-encrypted transcripts; Skills open std + workflows deprecated + /learn distillation (memory story = Rules+Skills, no memories DB); Terminal Sandbox (Linux namespaces + macOS sandbox-exec, read_url-derived network AllowedDomains, unsandboxed() escapes, Windows UNDOCUMENTED); SDK default-denies run_command. SECURITY: Mindgard Forced Descent (mcp_config.json executes every launch, survives uninstall; Won't-Fix-in-48h; patch status CONFLICTED — CSA fixed-Feb-28-2026 vs Mindgard-blog-May-7-2026 "unaddressed" vs axonbuild Aug-16-2026 no-fix UNVERIFIED — both claims recorded) + Pillar fd -X RCE (native tool pre-evaluation trust gap; Jan->Feb 28 2026 fix, bounty Mar 26, no CVE; follow-on tasks.json time-bomb downgraded as social-engineering). QUOTA WHIPLASH: Nov 2025 generosity -> Jan 2026 quiet tightening -> Mar 11 2026 credits ($25/2500) + ~92-97% measured cuts + 168h lockouts -> May 19 2026 restructure (Ultra 5x $99.99 / 20x $199.99 from $249.99, single Gemini pool, credits->overage) -> Sep 4-10 2026 429-despite-full-quota wave (community forensics: localhost TLS cert expired Sep 4 + AG-859 desync; NODE_TLS_REJECT_UNAUTHORIZED=0 workaround); undocumented caps 16,384 output/turn all plans + 1,024 Claude thinking + silent Gemini truncation (forum 137702). MODELS: Gemini 3.6/3.7/3.8 Flash in 6 weeks, 3.8 same-day default flip (managed agent + SDK) + price doubles 2027-01-01; Claude/GPT absent on Enterprise + subagent model=inherit|flash|pro only + background models fixed Gemini; vendor benches 3.7 (FrontierCode 43.6/DeepSWE 65.3/Elo 1588) vs ChatForest table provenance-unclear; Teamwork Lean-verified Knuth proof + RISC-V 0.71% + Eigen/ParlayHash merges (falsifiable, VERIFY); no independent product eval (CONFIRMED) — AA tests SDK harness only: 15-25M tokens/task, $1.40-2.47/task, TB version sensitivity 87%->15%. Windsurf $2.4B acqui-hire lineage (figures UNVERIFIED); surface reversals: Agent Manager removed from IDE + IDE auto-converted to 2.0 (May 19 2026) + Gemini CLI consumer retirement Jun 18 2026. Deltas: detail agents 12 ADOPT / 6 AVOID / 6 WATCH (§3 sec21 segments extended); §1 Antigravity col corrected (8 score flips: language ○→◐, repomap ◐→○, PTY ○→◐, eviction ◐→○, auto-skill ◐→●, profiles ○→◐, telemetry ◐→○, evals ◐→○; recount 10●/8◐/7○); §21 rewritten in place; queue premise updated. UNVERIFIED carried: Windsurf figures, agy Go, Mermaid, Computer-Use lineage, KIs on disk, MCP-server-mode absence, axonbuild page, ChatForest cells, 3.8 bench numbers, Lean artifacts, TLS forensics, model-mismatch, cut %.

- **Iteration 12 — Cursor (deep)**: COMPLETE 2026-09-11 (scout + architecture/trust-archaeology/models-community detail agents + synth). PREMISE CORRECTION: shadow-workspace window superseded by git-worktree/cloud-VM parallelism (2.0); window status changelog-silent. HEADLINES: SpaceX acquired Anysphere $60B all-stock (announced 2026-06-16, closed 2026-08-14, 389,289,254 Class A shares, SpaceXAI division; same-day ToS AAA→Texas courts); OpenAI change-of-control cancellation 2026-08-28, shutoff 2026-11-12; Temporal Cloud 50M+ actions/day (workflow-per-turn + signal-with-start + child-workflow subagents + production-history CI replay); CAS retry w/o token replay; Firecracker microVMs in separate AWS account; OIDC /run/cursor/api.sock RS256 5-min JWTs; split-brain self-hosted workers (outbound-only HTTPS; stdio MCP local / HTTP MCP proxied, creds never in VM); sandbox.json unweakenable admin floor + SSRF default-deny; prompt-type hooks + failClosed opt-in + loop_limit; rules Team>Project>User + nested AGENTS.md COMBINED; skills nested monorepo scoping + /migrate-to-skills; MCPoison CVE-2025-54136 (full-entry-hash fix); CLI /rewind incremental checkpoints + subagent resume; SDK @cursor/sdk (task-brief name @anysphere/cursor-sdk wrong) SQLite/JSONL stores + getUsage() dollar cost; Composer 2 arXiv 2603.24477 (K2.5 1.04T/32B base, router replay, MTP, self-summarization, pretraining-loss-predicts-RL-reward); Composer 2.5 sharded Muon + HSDP $0.50/$2.50; real-time RL ~5h cycles; Tab RL 400M req/day −21% suggestions +28% accept; Bugbot 78.13% n=50,310; CursorBench-3 + Cursor Blame mining; archaeology: Memories + Custom Modes removed v2.1 2025-11-21 silently (Custom Modes back as pinned skills 2026-08-19), model-opacity loop Jun 2025→Aug 2026, downgrade culture 2.0.77/1.7, Token Rate $0.25/M incl BYOK, Origin 2026-08-17 Graphite-built. Deltas: 12 ADOPT / 8 AVOID / 7 WATCH (§3 appends); §1 Cursor col corrected (18 cells touched: 3 ◐→●, 1 ○→●, 1 ○→◐ + content enrichment); §19 rewritten in place (19.1–19.9); queue premise updated. UNVERIFIED carried: CLI language, Tissera 736MB, dollar-figure removal, Composer 2.5 bench numbers, 70% F1000 (use 64% F500).
- **Iteration 11 — Qwen Code**: COMPLETE 2026-09-11 (scout + architecture/archaeology/community detail agents + synth). PREMISE CORRECTION: NOT a clean Gemini-CLI v0.8.2 re-import — git DAG shows parentless Alibaba root 71f8c070 2025-06-26, gemini-cli graph grafted 2025-08-02 (true merge a0dbb40d v0.1.15 carrying original Taylor-Mullen commits), squash v0.2.1, true merge v0.3.x, squash v0.8.2 2025-10-23 = LAST upstream sync → independence only ~11mo; frankengraph (4,592 commits ride side-branches; PR renumbering). 27.8k★ Apache-2.0, 9,628 commits, ZERO merges since 2026-05 (all-squash regime). Headlines: cache-preserving fork subagents (80%+ claim), 4-policy permission mediation + N1 invariant, MCP transport pool + hysteresis budgets, divergence-proof SSE resync (ring replay + state_resync_required), Auto Mode 3-layer classifier, Agent Arena worktree best-of-N, /learn+/curator lifecycle, LLM hooks (prompt executor). Benchmarks: README SWE-bench Verified 500×3 = 76.40–77.80% (7 versions, settings disclosed); official TB2.0 has NO Qwen Code row (23.9–24.6% same-model vs card 51.5% = +10–25pt unexplained); Qwen3.8-Max TB2.1 86.6 = Claude-Code-harness self-report; Coder-Next Fig-3 limited cross-scaffold transfer (co-design moat). AVOIDs: free-tier-then-pull w/ broken funnel (OAuth off 2026-04-15), frankengraph import, dead-feature UX ghosts (100+ days), build-then-retire churn (WebUI 8mo, Electron forked out), unmatched vendor benchmarks, plaintext-default credentials. WATCH: daemon 20-part spec, IM channels, Qoder closed sibling, TB replication + Scaffold-Effect gap, LLM hooks, all-squash regime. Deltas: 10 ADOPT / 6 AVOID / 7 WATCH (§3 appends updated in place); §1 Qwen col corrected (6 ◐→●, 1 ○→◐, evals ●→◐); §18 rewritten in place; queue premise corrected. UNVERIFIED carried: auto-compress threshold, telemetry default, RSS, eval-harness absence, 320+ PRs/week, press Coding-Plan pricing.
- **Iteration 9 — Factory Droid**: COMPLETE 2026-09-11. Single librarian (web + docs raw-.md + npm + Factory-AI org; 24m54s). Findings: sec16 (closed-source commercial CLI; npm droid v0.186.0 UNLICENSED 8.5k dl/wk; Factory-AI/factory = issues-only repo 18★; queue premises FAILED verification: CoRIM UNVERIFIED (no Factory link; provenance space = Crash Override/Pedigree/agentmark/didrun), git-droid 404, Sourcegraph-roots FALSE (Amp = separate product, stays queue item 10); Factory founded 2023 Grinberg+Reyes, GA 2025-05-28, $50M B 2025-09, ~$150M C 2026-04 @ ~$1.5B UNVERIFIED; enterprise NVIDIA/Adobe/Morgan Stanley/EY-5k-eng; Factory Router per-task routing 43% claim + 99.9% multi-provider failover; credit multipliers 0.12-12x + Droid Core open-model free tier; sandbox Seatbelt/bwrap+seccomp + userland net-filter proxy w/ Factory+WorkOS always-allowed escape (AVOID) + fail-closed + Win=WSL2-only; command BLOCKLIST w/ program-resolution anti-bypass; Custom Droids flat depth-1 + complexity-tier routing + mission validators (scrutiny + user-testing); cloudSessionSync default-ON (AVOID); OTLP metrics default-ON content-opt-in; TB 1.0 #1 58.75% vendor / TB 2.0 #10-12 77.3%±2.2 independent; bestaiq FALSE-SUCCESS-REPORTING finding + credit blackhole; changelog archaeology: 2-5 releases/week, .droid.yaml→.factory migration, autonomyMode deprecation, MiniMax M2.5 retirement). Queue deltas: sec16.7 — 10 ADOPT / 8 AVOID / 7 WATCH; Droid column added to §1 matrix. Plan deltas folded: todos 26, 27, 29, 39, 40, 41, 42 (7 edits).
- **Iteration 8 — Grok Build**: COMPLETE 2026-09-11. Single librarian (web + cloned source-drop mirror @ HEAD 3794978, 44 sync commits). Findings: sec15 (xai-org/grok-build ACTIVE 26.7k★ Apache-2.0 Rust TUI; beta $300/mo → 2026-07-12 Cereblab exfil scandal (5.1GiB vs 192KB uploads, canary, .env) → 07-13 server kill-switch → 07-16 OSS as trust repair; source-drop governance = no issues/PRs, sync-only history, forks within 48h; 1.3M LoC; ports-as-tool-layer (Codex+OpenCode impls w/ Apache §4(b) notices); hashline anchored editing w/ 3 candidate anchor schemes + benchmark (independent validation of our OMO hashline ADOPT); tool behavior version registry (legacy_0_4_10 presets); compaction segment store; tiered eviction (pruning ladder → 2-pass compact → segments → memory flush w/ hybrid FTS5+vec0 + temporal decay + MMR + gated /dream); Landlock/Seatbelt whole-process irreversible sandbox (no Windows); 6-mode permissions w/ Claude settings.json compat; hooks 16 events fail-open (AVOID); /rewind incoherence UI-vs-code (AVOID); ACP stdio+WS + leader process multi-client; foreign-session import claude+codex; Grove NFS/FUSE projected clones; /goal adversarial evidence-review gate; OTel double-opt-in content-free; SWE-bench V 70.8% claimed; RSS UNVERIFIED). Queue deltas: sec15.7 — 12 ADOPT / 8 AVOID / 6 WATCH; Grok column added to §1 matrix. Plan deltas folded: todos 6, 12, 13, 16, 21, 24, 26, 28, 30, 35, 41 (11 edits).
- **Iteration 9 — Factory Droid**: COMPLETE 2026-09-11. Single librarian (web-only: docs.factory.ai raw-.md crawl + GitHub API + npm + community reviews; nothing to clone — closed source). Findings: sec16 (closed-source commercial CLI, Bun inferred; Factory $50M B 2025-09-25 + ~$150M C 2026-04 ~$1.5B UNVERIFIED; TB1 #1 58.75% vendor 2025-09 vs TB2.0 #10-12 77.3%±2.2 independent; ship trains every 1-3 days to v0.209.0; Factory Router per-task routing (43% cost claim, provider failover); Mission orchestrator+workers+VALIDATOR milestones (scrutiny + user-testing, skippable); Custom Droids + complexity-tier subagent routing light/medium/heavy; command BLOCKLIST with program-resolution anti-bypass; Seatbelt/bwrap fail-closed sandbox + userland net-filter proxy (Factory/WorkOS domains always allowed); cloudSessionSync default TRUE; Droid Control /verify CONFIRMED/REFUTED/INCONCLUSIVE; rolling 5h/7d/30d limits + 0.12-12x credit multipliers; independent false-success-reporting finding; queue premises CoRIM/git-droid/Sourcegraph UNVERIFIED). Queue deltas: sec16.7 → 10 ADOPT / 8 AVOID / 7 WATCH; Droid column added to §1 matrix.
- **Iteration 7 — Pi / Claw Code**: COMPLETE 2026-09-11. Single librarian (web + full-history repo @ HEAD 7b4cfd6, 6348 commits). Findings: sec14 (pi minimalist-loop origin VERY ALIVE: 104k stars, MIT day-one, earendil-works era since v0.74.0 2026-05-07; 4-tool floor; tiny replaceable system prompt; JSONL TREE sessions id/parentId; pi-ai unified providers w/ cross-provider handoffs + per-provider prompt caching; extensions hot-reload self-modification; Agent Skills std; Project Trust; NOOP telemetry; no MCP/subagents by doctrine; OpenClaw 389k-star gateway vendored pi runtime (alias pi->openclaw); claw-code = 2026-03-31 Claude Code source-map leak (512k LOC) clean-room ecosystem, instructkr->ultraworkers 195k stars, agent-managed exhibit, OmX/clawhip/OmO trio, clawhip event-router daemon OUTSIDE context window). Queue deltas: sec14.7 - 10 ADOPT / 4 AVOID / 6 WATCH. Second-pass verification 2026-09-11 (sec14.9): all SHAs/stats re-verified live (6/6 archaeology SHAs real, claw-code redirect exact, OpenClaw vendoring doc verbatim), TB "6th place Oct 2025" corrected to TB2.0 leaderboard 2025-12-02, SSH-first + Earendil RFC-0015/Lefos added.
- **Iteration 6 — Plandex**: COMPLETE 2026-09-11. Single librarian (web + full-history repo @ HEAD e2d7720 = the wind-down commit). Findings: sec13 (company DEAD - solo founder 1301/1483 commits; wind-down 2025-10-03 after Promptfoo hire; cloud off 2025-11-07; Promptfoo->OpenAI 2026-03-09 VERIFIED; MIT->AGPL->MIT license flip-flop; v2 = 8-month rewrite -> client/server + Postgres + plan-git sandbox branches; 9 model roles + nested fallback chains = deepest multi-model pre-Kilo; _apply.sh exec-as-diff + auto-rollback; edit race deterministic->diff-vs-wholefile; auto-revert-on-rewind default flip; promised benchmarks never shipped; lost to Claude Code token subsidies + bus factor). Queue deltas: sec13.7 - 8 ADOPT / 6 AVOID / 2 WATCH.
- **Iteration 5 — Mentat**: COMPLETE 2026-09-11. Single librarian (web + full-history repo @ SHA 3a01f0a = v1.0.19; AbanteAI/mentat renamed archive-old-cli-mentat + archived). Findings: sec12 (day-1 Y/n/i per-hunk staging + insertion-conflict ordering + write-authority-scoped-to-read-scope; ctags->ragdaemon context thrash; 4-parser zoo; 2024-04 dev stop -> closed MentatBot SWE-bench-Lite SOTA Jun 2024 -> CLI deprecation 2025-01-07 -> both products dead; aider 48.9k vs 2.5k stars). Queue deltas: sec12.5 - 7 ADOPT (write-scope guard, confirm-or-coach, WIP-diff context seed, revisor repair pass, conflict-ordering UI, HUD stream message, LoCoDiff-bench eval axis) / 5 AVOID / 0 WATCH. LEDGER CORRECTION: staging prompt is Y/n/i, not y/n/e.
- **Iteration 4 — Kilo Code**: COMPLETE 2026-09-11. Single librarian (web + repo @ SHA 3d04228 = v7.6.2). Findings: sec11 (fork->superset->GA 2026-04-02->OpenCode-server rebuild->Anaconda exit 2026-07-15; TS/Bun VERIFIED (not Go), @kilocode/cli fork pinned upstream v1.18.13; kilo serve HTTP+SSE client model; Agent Manager/task/Swarm orchestration; subagent_depth flat-by-default; Orchestrator+Memory-Bank deprecation reasoning verbatim; premise correction: v7 does NOT read .roomodes/.roo/rules - rename-to-.kilo wizard only, legacy 5.x pinnable). Queue deltas: sec11.6 - 9 ADOPT / 4 AVOID / 5 WATCH.
- **Iteration 3 — Roo Code**: COMPLETE 2026-09-11. 3 agents-equivalent (lifecycle/web; architecture/repo @ SHA b867ec9; community+successors). Findings: §10 (mode-scoped tooling origin + verbatim defaults, boomerang internals, repo-per-task shadow checkpoints, condense v2 + non-destructive truncation, BETTER/WORSE vs Cline, ADOPT/AVOID deltas). Queue delta: Kilo (todo 4) pre-mined here via its migration guide — verify on arrival, don't re-derive; Orchestrator-deprecation rationale is Kilo's own, cite it.

- **Iteration 2 — Cline**: COMPLETE 2026-09-11. 4 agents (architecture;
  features; git-history; community+performance). Findings: §9 (checkpoint
  saga + new stash-commit design, Clinejection + CVE-2026-59723, reversal
  catalog, dual-bundle rollout playbook, matrix scores, 8 queued ADOPTs /
  5 AVOIDs). Queue deltas: fold scratch-index/restore-transactions (todo 23),
  overflowRecovery (todo 13) into plan.
- **Iteration 1 — Deepseek Harness**: COMPLETE 2026-09-11. 4 agents
  (architecture/concepts; features-vs-matrix; git-history/reversions;
  community+performance) + skill-learning librarian. Findings: §6 (synthesis:
  CVE-2026-82533 lessons, reversal catalog, matrix deltas, perf calibration,
  8 new ADOPTs / 7 new AVOIDS), §7 (Hermes=hermes-agent, AutoAgent=HKUDS,
  skill-forge prior art + lessons), §8 (source record, 35+ permalinks, SHA
  c291e79). Plan deltas queued for next plan touch: daemon peer-address auth
  test (todo 35), protocol-dialect-explicit model profiles (todo 34), budget
  tightening, HARNESS-AS-SUBAGENT launcher backend (todo 40), growing+
  refinement skill updates (todo 31).
- **Pre-loop research** (6 librarians + 3 explorers + local inspection):
  digested into draft + matrix above; skill-learning librarian (bg_96ff4596)
  pending at time of plan write.

## 6. Iteration 1 findings — DeepSeek Harness (synthesis of 4 agents + full §6 source record above)

**GIT-ARCHAEOLOGY / REVERSAL LESSONS (the user's "mistakes they reverted" ask):**
- **CVE-2026-82533** (CVSS 9.4, GHSA-8m2g-8cgm-3vcp): local control-plane API on
  127.0.0.1:3080 trusted client-supplied Host header (`isTrustedApiRequest`,
  never checked peer address); sandboxed agent self-escalated to
  danger-full-access/approval-never via ONE curl. Sandbox confined file-writes
  only (bwrap without --unshare-net; Seatbelt allow-default). Fix shipped in
  0.1.2-alpha.1 **GitHub-only — NEVER published to npm** (official installer
  points at npm); first fixed npm release 3 days later. Community had reported
  the same escape on Discussions Aug 13/14 — BEFORE the CVE — and there was no
  SECURITY.md/private channel. Vendor published no GHSA. → OUR PLAN: daemon
  auth = peer-address check, never Host header (add explicit test to todo 35);
  SECURITY.md + private reporting channel day one; security fixes MUST ship to
  the default install channel simultaneously (release-process test).
- Earlier independent escape: Bubblewrap `/proc/<pid>/root` (fixed 0.1.1-rc.1).
- **SQLite persistence backend REMOVED outright** (0.1.2-alpha.3: "use an older
  version to export") — storage engine swapped a month post-launch.
- **Session-format churn**: v0→v1→v2→v3 within ~4 weeks; a format-migration PR
  merged-then-reverted (#2698→#3054); v3 has NO downgrade path; migration
  bugs admitted (#5909, #6278, #6282, #6287, #6283 seq-counter regression
  "3rd occurrence"). → OUR PLAN: versioned schema day one (already planned) +
  round-trip migration tests both directions + never ship no-downgrade formats
  in v1.
- **Public rollback admission**: `SessionEvent.ignorable` removed in
  0.1.2-alpha.1, restored in alpha.2 ("Restore... which was removed").
- **str_replace_editor saga**: hard removal → same-day revert → soft
  disable-by-default two days later (the correct move, third).
- **Flip-flop**: PR #2608 reverted Aug 21 → re-reverted (restored) Aug 30.
- **Shipped known perf regression** (0.1.3-alpha.1 release notes admit it);
  **version 0.1.4 silently skipped**; npm/GitHub version gaps (rc.1/rc.4/rc.5,
  0.1.3-alpha.1 missing from npm).
- **In-process plugin blast radius**: any malicious plugin = whole process
  (shell/fs reach). → reinforces our WASM-capsule + MCP-as-runtime decisions.
- **Context duplication bug**: identical CLAUDE.md+AGENTS.md injected twice
  (+30k tokens). → single-source context assembly test (we have via todo 11).
- **Telemetry dsh_plugin_packages default-ON** (plugin inventory to DeepSeek
  API) — matches our AVOID list; confirmed anti-pattern in the wild.
- **thinkingFormat guessed from endpoint URL** → wrong dialect → 72k-char
  reasoning bloat vs 6k tuned. → OUR PLAN: model-profiles must carry protocol
  dialect explicitly, never infer (feeds todo 34).
- Team led by Cui Tianyi (ex-Jane Street); no external PRs; issues disabled.

**FEATURE-MATRIX DELTAS (dsh column):** strong: sandbox ● (3-OS fail-closed +
confinement-degree self-report to model!), permission tiers ●, session
event-sourcing + SQL-query over own history ●, multi-client ●, frontends
Web/CLI/desktop/SDK/ACP (no TUI!), subagent backends incl. Claude Code/Codex ●.
Weak/absent: AST repomap ○, WASM ○, encryption ○, PTY scraping ◐ (byte-capped,
no screen-delta), compaction ◐ (2 mechanisms), prefix-cache discipline ◐
(measured cache-hostile), MCP server mode ○ (client = tools-only), evals ◐
(perf benches only), auto-skill-learning ○ (cordis_define runtime self-extension
≠ skills).

**PERF CALIBRATION FOR OUR BUDGETS:** idle RSS 35–40 MB after GC (TS/Node!),
boot peak ~212 MB, disk 306 MB/531 packages; own perf budgets: session-open
900 ms, page 700 ms. Token efficiency ~88k tokens/task vs Claude Code ~650k
(Composio, endpoint caveats). → We tighten: daemon RSS target <40 MB (was
<80), CLI cold start <50 ms stands (dsh: ~900 ms session open).

**NEW ADOPTS (added to ledger §3):** (1) "model-visible means logged"
invariant + fail-closed flush-before-side-effect checkpoint policy; (2) sandbox
confinement-degree self-reporting injected as model-visible context; (3)
harness-as-subagent providers — delegate to real Claude Code/Codex children
via their official protocols as launcher backends; (4) drift-gated catalogs
(unknown upstream convention = compile error); (5) deterministic post-compaction
replay (shadow preserved in log); (6) PTC/code-mode tool orchestration
(collapses N tool calls into one generated program — evaluate as tool); (7)
SQL query over own session history (session_event tools); (8) per-capability
token/KV-cost documentation sections on every module.

**NEW AVOIDS:** in-process third-party plugins; release-channel skew for
security fixes; shipping known regressions; no-downgrade format migrations;
disabled issue trackers w/o security channel; loop-as-config-row (keep loop in
code with seams); 4-layer config-as-code assembly.

## 7. Skill-learning prior art (bg_96ff4596 — feeds plan §R Skill Forge)

**IDENTIFICATIONS:** "Hermes" = **NousResearch/hermes-agent** (Feb 2026, 244k★)
— "the self-improving agent": post-task learning loop, skill "crystallization"
(session→reusable skill), Curator maintenance agent (stale 30d→archive 90d,
consolidation, snapshots), FTS5 memory, Honcho user modeling, /learn, agentskills.io
format, Atropos RL trajectory export; companion **hermes-agent-self-evolution**
(DSPy+GEPA) 5.3k★. "Auto Agent" = **HKUDS/AutoAgent** (9.8k★, zero-code,
ACL'26) most likely; also **thirdlayerinc/autoagent** (4.6k★ — meta-agent
hill-climbs its OWN harness on Harbor scores; directly relevant precedent for
our model-profiles→launcher loop) and **AMAP-ML/SkillClaw** (2.6k★, agentic
skill evolver w/ collective self-improvement).
**PRIOR ART:** Voyager skill library (+ablation: removal hurts); Anthropic
skill-creator skill (agents authoring skills); Anthropic memory tool + context
editing (39%/29% improvements on agentic-search; the "+13% fact recall" figure
UNVERIFIED); Letta sleep-time compute (background memory rewriting); Reflexion;
ExpeL insights; AWM workflow induction (+24–51% relative on web tasks);
Memento case-bank (+w/o fine-tuning); ACE agentic context engineering — KEY
FAILURE MODE "context collapse" + growing-vs-refinement mitigation; Mem0 (+26%
claim), Windsurf/Cursor auto-Memories (product-level).
**MODEL-WEAKNESS ART:** AgentErrorTaxonomy/AgentErrorBench (2509.25370 —
failure taxonomy + learning-from-failures), TRAIL trace localization, DSPy
MIPROv2, GEPA reflective prompt evolution (beats GRPO ~10%, MIPROv2 ~19%),
RouteLLM/routers, Darwin Gödel Machine (self-modifying agents; documented
deceptive/reward-hacking behaviors → our trust gates justified).
**LAUNCHERS:** MetaGPT 45k★, ChatDev 26k★, ruflo/claude-flow 72k★, AgentVerse,
AutoAgents (IJCAI'24), CAMEL.
**LESSONS for Skill Forge:** skill sprawl/drift → Curator-style maintenance
(we have); fossilized bad habits → eval-before-activate (we have); context
collapse → growing+refinement delta updates, not rewrites (ADD to our design);
trajectory quality filtering before learning (Atropos-style export — ADD);
user-correction journal ≈ Honcho user modeling (validated).

## 9. Iteration 2 findings — Cline (synthesis of 4 agents; SHA a7c1dfc, 2026-09-11)

**Identity:** cline/cline 67,818★ / 7,327 forks / Apache-2.0 / TS monorepo;
created 2024-07-06 as saoudrizwan/claude-dev; renamed v2.0.0 2024-10-09 (model-
agnostic rebrand, NOT an Anthropic takedown — myth corrected). 11M+ installs;
v4.0.0 (2026-06-26) migrated the 76k-line extension onto the shared Cline SDK.
Fork lineage: Roo Code (repo 2024-10-31) → Kilo Code (2025-03-10, superset,
$8M, rebuilt on OpenCode 2026-04); **Roo ARCHIVED 2026-05-15** → Roomote pivot;
Cline = surviving upstream. Surfaces: VSCode + JetBrains + CLI + TUI + ACP
server + Kanban + desktop + hub daemon + SDK.

**REVERSAL/MISTAKE CATALOG (the user's ask):**
- v2.0.0 reversed native function-calling → XML tool tags (admitted design
  error: "forcing structured output has negative qualitative effects", ~40%
  fewer requests claimed).
- **Checkpoint saga (worst mistake)**: legacy system renamed user's `.git` →
  `.git_disabled` — kill-unsafe race corrupted repos (#2868 "50 repos
  scuffed"; #4385/#4388 meta: 4GB+/task, 120GB+ disk; #5052; #5598 maintainer:
  "no way to get around this unless we overhaul"); escape hatch v3.2.6; fixes
  incremental through v3.24.1; NEW SDK system (2026) = dangling 3-parent
  stash-shaped commits in user's .git + refs in session metadata + persistent
  scratch GIT_INDEX_FILE (14-day reaper) + restore TRANSACTIONS (pre-restore
  stash to private ref w/ commit/rollback) + refuses restore when user commits
  exist after checkpoint. → OUR PLAN (todo 23): adopt scratch-index trick +
  restore transactions; NEVER rename user .git (we already don't).
- v4.0 admitted regressions: subagents temporarily disabled; per-tool MCP
  auto-approve checkboxes hidden ("no-ops that implied granularity the
  approval path does not have"); checkpoint restore had silently knocked
  commits off branches (fixed 4.1.17); Hub broadcast full transcript per
  status → tens-of-GB growth; stray click on context bar silently set tiny
  compaction threshold; agentic-compaction bug wave (silent fallback / endless
  skip / stale-state full recompaction every turn).
- **overflowRecovery principle** (keep): deterministic basic compaction when
  provider rejects — "recovery must not depend on another successful LLM
  request" → add to todo 13.
- YOLO mode removed; Claude-4 upsell nag for non-Claude models (#5915, 57👍 —
  trust damage); cost controversies verified ($300/412k-token task #8027;
  silent 1M-context switch 20× spend #5559; pre-flight cost guard REJECTED for
  latency #5870; OpenRouter unpinned providers → 0% cache-hit 3–5× #10596).
- **Security:** Clinejection (prompt injection → CI/CD tokens exfil, Dec
  2025; fix PR #9211) then unauthorized npm publish cline@2.3.0 (GHSA-9ppg-
  jx86-fqw7; extension itself NOT compromised per post-mortem); CVE-2026-59723
  Hub WebSocket hijacking (fixed 3.0.30). Top-voted issue EVER: Apache-2.0 vs
  ToS content-rights conflict (#3510, 87👍).
- **Token-hungry by architecture** (maintainer-admitted: all reads stay in
  context; no LSP navigation → ripgrep+read loops #8336; 1M windows removed
  the 200k ceiling) — validates our entire context-economics stack.

**FEATURE-MATRIX (Cline):** strong: checkpoints ● (new design), windowed
reads ● (2000-line/48k-char caps w/ paging), tiered eviction ● (0.9 trigger/
0.7 target/20k preserve; agentic+basic+overflowRecovery), prompt-cache routing
● (per-route capability, ephemeral cache_control), frontends ● (7 surfaces),
multi-client ● (hub daemon + browser WS + instance lock), sessions ● (SQLite
versioned + artifacts-lazy + fork/resume/replay), skills ● (agentskills.io +
reads .claude/skills!), editor telemetry ● (respects VSCode telemetry level +
OTel export + opt_out event), subagents ◐ (disabled at v4.0), evals ◐
(cline-bench). Absent: OS sandboxing ○ (policy-only; model-declared
requires_approval — injection-prone), MCP server ○, WASM ○, encryption ○,
AST repomap ○ (deliberate "no index" philosophy), PTY screen-delta ◐ (shell-
integration streaming + caps), auto-skill-learning ○.

**ADOPT (queued plan deltas):** (1) dual-bundle progressive rollout w/ crash
fallback + kill switch (OUR migration pattern); (2) overflowRecovery (todo
13); (3) scratch GIT_INDEX_FILE + restore transactions (todo 23); (4)
per-mode/per-phase model profiles incl. --config dirs + thinking levels
(validates todo 42); (5) prompt-cache routing as provider capability (todo
10/11); (6) tool-result session artifacts (validates todo 24); (7) OTel
opt-in export; (8) ACP (already O).
**AVOID:** model-declared approvals (keep deterministic allowlists); .git
mutation races; reactive-only cost mgmt; model upsell nags; silent context-
window switching (pin windows in model profiles).

## 10. Iteration 3 findings — Roo Code death autopsy (4 agents; SHA b867ec9, archived 2026-05-15)

**Lifecycle:** forked Cline 2024-10-31 (RooVetGit/Roo-Cline — inherited Cline's git
history/tags); modes origin v3.1.0 (Jan 2025); rebrand→Roo Code v3.2.x (2025-01-21,
50k installs); .roomodes v3.3.20 (2025-02-14); new_task/boomerang v3.3.6 (Feb
2025); condensing-v2 v3.43.0 + worktrees v3.44.0 (Jan 2026); final v3.53/54 (May
2026). **Died 2026-05-15** after Apr-21 announcement: "3M installs… winning at a
game we no longer believed was the main event… first-party tools became discount
token ATMs, and forks redistributed our work almost as fast as we shipped it."
Pivot→Roomote (Slack-first cloud). Stated cause = belief shift to cloud; inferred
= monetization failure (3M cumulative vs 1.5–1.7M active installs, low paid
conversion, brutal Router economics) + competition + release-churn debt. 24.3k★
at death. Successors: Cline (recommended; absorbed features), Kilo (1.5M users
claimed), ZooCode (community fork created 2026-04-23 — day after announcement).
Cline CEO: "Roo was a good fork… contributed to Cline more than anyone else."

**Architecture (what Roo pioneered — we adopt):** 5 modes w/ orchestrator
groups:[] (delegation-only, deliberately tool-less = context-poisoning
prevention — they DOCUMENTED context-poisoning as a concept + "treat session as
disposable" doctrine); custom modes YAML (.roomodes + .roo/rules-{slug}/);
per-mode API profiles (modeApiConfigs) + sticky models; boomerang subtasks
(parent pauses, child isolated, summary-only return, profile inheritance);
condensing (threshold slider + custom prompt + manual button; 30% reserved =
20% output + 10% buffer; SAME-MODEL condensing by design; native token counting
+ tiktoken fallback; 25%-truncate error recovery); auto-approve (granular +
command prefix allow/deny w/ longest-prefix + dangerous-substitution guard +
max-requests cap); checkpoints (shadow git, task-scoped, .git/info/exclude
hygiene, LFS-aware, disable-on-nested-repos, files-only vs files+task restore);
read_file (2000-line slice + INDENTATION-ANCHOR block mode + definitions-only);
Focus Chain (model-maintained todo re-injected every turn as environment anchor
+ preventCompletionWithOpenTodos gate); skills agentskills.io + marketplace
(declarative config patches, parameterized secrets, project/global scopes);
reasoning handling (OPEN_ROUTER_REASONING_BUDGET_MODELS, :thinking suffixes,
thinking-not-in-context, minimal effort).

**REVERSAL CATALOG:** "Revert to pre-AI-SDK state (Jan 29 2026)" — 2 weeks of
Vercel-AI-SDK migration discarded, then 22 clean cherry-picks reapplied;
defaultToolProtocol xml↔native DOUBLE flip-flop (Dec 2025); halted 3.6.5
release entirely; reverted cloud agents (Oct 2025); parallel-tool-calling
flip-flop; marketplace wildcard-activation crash revert; E2E tests revert;
Cloud embeddings revert. **11 security advisories 2025** (auto-execute RCE,
GH-Actions injection, bash param RCE, symlink .rooignore bypass, npm
postinstall auto-approve, .vscode settings RCE, MCP RCE/leak class) —
policy-only safety w/o sandbox = CVE farm. **Community:** modes = killer
feature ("nothing else has shipped the same model" re boomerang); complaints =
token costs, checkpoints (40GB+ disk, disappearing), instability under ~DAILY
releases (100 releases/169 days!); canonical free model stack (Orchestrator/
Think=Gemini-free, Architect/Debug=R1, Code=Kimi-K2/DeepSeek — "$0 development"
workflow hit #1 r/RooCode). **Kilo counter-evidence:** deprecated Roo's
Orchestrator ("full-tool agents delegate natively; explicit coordinator added
overhead") + Memory Bank→AGENTS.md (memory bank was never an official Roo
CHANGELOG feature — community convention).

**ADOPT (queued plan deltas):** (1) command-prefix allow/deny + substitution
guard (todo 26/27); (2) Focus-Chain persistent todo anchor + open-todos
completion gate (todo 5); (3) indentation-anchor block reads (todo 16); (4)
per-task profile stickiness + subtask inheritance — anti-silent-drift (todo
42); (5) checkpoint hygiene: exclude-file + LFS + nested-disable + files-only
restore mode (todo 23); (6) same-model condensing (todo 13); (7) condense
threshold slider + custom prompt + manual button (todo 36/39 UX); (8)
marketplace = declarative config patches w/ parameterized secrets + scope
(todo 32); (9) context-poisoning doctrine + disposable-session recovery docs.
**AVOID:** vector DB for code (Qdrant — real-world confirmation); editor-API-
coupled core (vscode-shim hack; CLI broke on Windows); telemetry default-on;
~daily releases w/o stability gates; cloud-Router monetization coupled to free
OSS; in-process TS tools; policy-only safety (11 CVEs).

## 11. Iteration 4 findings — Kilo Code (fork→rebuild→acquisition; SHA 3d04228, 2026-09-11)

**Identity corrections:** repo = Kilo-Org/kilocode (created 2025-03-10, 27.2k★,
31,054 commits, MIT). Fork of Roo March 2025 (not Aug). ".roomodes/.roo/rules
on startup" = LEGACY v5.x ONLY — v7 wizard requires rename (premise corrected).
**Anaconda acquired Kilo 2026-07-15** (3M devs, ~10T tokens/mo; homepage now
claims 5M+ — metric drift flagged). $8M seed Dec 10 2025 (Cota Capital).
**Two-stage OpenCode pivot:** CLI rebuilt Jan–Feb 2026 (CLI 1.0 Feb 3);
VS Code ext rebuilt (beta Mar 10, GA Apr 2 2026); Roo-derived codebase split
to kilocode-legacy (created 2026-02-22, archived 2026-08-03); v5.9.0 = last
legacy, v7.0.x from Feb 23. Roo-death response: migration guide NEXT DAY
(Apr 22) + "Thank you, Roo" (Apr 21) + in-code migrators (rules/modes/mcp/
workflows/ignore) — but no hard migration stats; Roo's official recommendation
disputed across sources (Cline vs Kilo vs ZooCode).

**Architecture (post-rebuild):** TS/Bun monorepo (Effect TS, drizzle+SQLite
w/ embedded migrations); packages/opencode = fork of anomalyco/opencode pinned
v1.18.13 + `kilocode_change` annotation discipline + CI duplication gates;
ALL products = clients of `kilo serve` (HTTP+SSE; VS Code bundles CLI binary
as child; JetBrains native Java no-Node; TUI TS/Solid). kilocode/ overlay (46
dirs): agent-manager, anaconda-desktop, board, claw, cloud, daemon, memory,
sandbox, snapshot, session-portability, worktree, pty. **kilo-sandbox**:
bubblewrap+Seatbelt+network proxy/TLS-hello inspection on
@anthropic-ai/sandbox-runtime; **Windows explicitly unavailable** (code).
**kilo-indexing**: tree-sitter+embeddings+LanceDB (landed post-guide, PR #6966
Apr 27); semantic_search tool. **kilo-memory**: auto-capture (digest/plan/
outcome/REDRACT) + kilo_memory_save/recall tools + autoConsolidate.
**CONTEXT.md runtime vocabulary**: Context Epoch (immutable provider-cache
baseline span ending at compaction), Mid-Conversation System Messages
(in-history appends), Managed Tool Output spill files, Safe Provider-Turn
Boundaries — the most explicit cache-discipline naming in the field.
Compaction: prune (tool outputs→2k chars) + summarize + tail-turn + overflow
recovery; **doom_loop as a permission type (default ask)**.

**Orchestration:** task tool (subagent_depth default 1, configurable nesting;
deny-only permission inheritance; task/todowrite capability-gated children;
task_id-resumable). **Agent Manager**: worktree-per-chat (.kilo/worktrees/),
PR pipeline (gh badges, merge controls, "Fix with Kilo"), **multi-version mode
(up to 4 parallel same-prompt implementations, per-version models)**,
Permission Dock; agent_manager + agent_manager_models tools (on-demand model
catalog search — full catalog NEVER in context). **Kilo Swarm**:
board_post/board_read w/ "Stored only…" receipts. **Gas Town by Kilo**:
productized gastownhall/gastown protocol (towns/beads/convoy DAG/mayor/
polecats/refinery) — cloud orchestration. Snapshots:
~/.local/share/kilo/snapshot/<project-id>/<worktree-hash>/ detached worktrees,
per-user-turn granularity, hourly `gc --prune=7.days` (**can GC live-referenced
snapshots — bug class**). **Auto Models** tiers: frontier/balanced/efficient/
free/small — efficient = session-aware difficulty classification → cheapest
benchmark-proven model (via PinchBench/Rust + KiloBench leaderboard); thinking
blocks STRIPPED when family changes between turns (silent context loss);
x-kilocode-mode header routing (plan→opus/code→sonnet).

**Reversals/incidents:** git-backed review modes add→revert→restore cycle;
"revert: stop promoting stable releases to rc" (release-process rollback);
v7 GA backlash (#8364 "ABSOLUTELY HORRIFIC", #8108) → pin-legacy escape hatch
(the FIX pattern); Windows multi-GB memory fixed v7.2.20; SSE 1.3s freeze
fixed; **idle memory leak #9479 OPEN (8–11GB overnight)**; Compaction
Exhausted #9774; hardcoded 300s timeout kills local LLMs #1681; Metabase
data-breach fallout (Aug 2026, deletion-policy backlash); GLM-5 routing
transparency (222B tokens one day via one model); x-opencode-session header
race + OpenCode-2 strangler migration mid-flight (#12887) — fork-of-fork
upstream-coupling risk LIVE. Security: no own advisories; Roo-lineage CVEs
(2025–2026 incl. CVE-2026-30307 CVSS 9.8) — v5.x inheritance UNVERIFIED;
plaintext-credential admission in rebuild plan doc.

**ADOPT (queued):** (1) **benchmark-driven live routing** (efficient-tier
difficulty classification + continuously-run benchmarks feeding the router —
closes the loop between our todos 42/44); (2) Context-Epoch vocabulary +
mid-conversation system messages as design terms (todo 10/11); (3)
multi-version best-of-N mode (todo 40); (4) on-demand model-catalog search
tool (todo 34/42); (5) task_id-resumable subagents + deny-only inheritance +
capability gating (todo 40); (6) kilocode_change upstream-merge annotation
discipline + duplication CI gates; (7) "stored only" receipt semantics for
coordination boards; (8) memory auto-capture w/ redaction (validates todo 31
correction journal); (9) managed tool-output spill files (validates exec
design).
**AVOID:** snapshot GC pruning live-referenced objects; forced-update GAs w/o
escape hatch; silent thinking-block stripping on family change; hardcoded
timeouts for local models; upstream coupling without abstraction layers;
user-count metric drift (trust).
**WATCH:** Anaconda stewardship effect on OSS pace; gastownhall beads-protocol
adoption; OpenCode v2 migration outcome; ZooCode trajectory.

## 12. Iteration 5 findings — Mentat death autopsy (combined agent; SHA 3a01f0a, archived)

**DEAD TWICE:** repo renamed AbanteAI/**archive-old-cli-mentat** + archived
(2,544★, Python, Apache-2.0, created 2023-07-25, 560 commits). Active dev Jul
2023→Apr 2024; 8.5-month silence; deprecated 2025-01-07 (v1.0.19 farewell
commits: archive-README + deprecation-message + version-bump). Brand recycled
to closed-source **MentatBot** GitHub bot — SWE-bench Lite SOTA claim 38%
(GPT-4o, 2024-06-27, VERIFIED on official SWE-bench viewer + r/singularity
56↑); MentatBot ALSO dead by 2026 (service "stopped accepting new usage" —
secondary source flagged). Team: Scott Swingle/biobootloader (ex-DeepMind) +
~3 core (Koenig, P. Swingle, Gilliam, Hawkins); viral 6.1K-like launch tweet;
HN modest (12-pt thread). **Lost to AIDER** (48.9k★ alive vs 2.5k archived,
~19×): aider's benchmarked-single-format culture vs mentat's parser churn;
aider multi-model early vs "meager LLM list" (Osika 2/5 vs aider 4/5); aider
git-native auto-commits vs mentat "leaves version management to you";
reliability complaints ("crash from time to time… graceful failure isn't its
strength"); closed pivot starved the community CLI.

**DAY-1 FOUNDING FEATURES (pre-dating everyone):** per-hunk interactive
staging (Y/n/i → per-file → **per-REPLACEMENT "Keep this change?" walk**) +
unique insertion-conflict ORDERING UI (user orders conflicting insertions
a/b/c); **edits validated against context intervals — write authority scoped
to read scope** ("File not in context, canceling all edits"); @@start/@@code/
@@end JSON block format from initial commit; Session-as-server + SessionStream
channels ("The server for Mentat" — designed for terminal/VSCode/NeoVim;
NeoVim never shipped; **VS Code client applies edits itself** via apply_edits
flag); **Revisor** (experimental LLM self-repair of malformed edits); 4
user-selectable parsers (block/json/replacement/unified_diff) + REVERSIBLE
parsing (invert edits for undo); cost HUD in textual TUI sidebar (Feb 2024 —
early); git-diff-anchored context (context = WIP diff + untracked — "extend
your work"); Vision (GPT-4V); /agent proto-mode; sentry crash reporting.

**REVERSALS:** context-architecture thrash — ctags CodeMap (aider-inspired,
cited in-code) ADDED Nov 2023 → REMOVED Dec 2023 (#335) → feature-filters →
ragdaemon graph DB Apr 2024 (THREE architectures in 5 months); parser-format
churn incl. "Changed format to fit Anthropic models" (v1.0.11); UI stack swap
(colored/pygments → textual, #508); ragdaemon versioning flip-flop
(always-latest → pin 0.2.x — spinout-dependency coupling bit them); only 2
git-level reverts (history clean because they PIVOTED rather than iterated).
Undo = in-memory edit_history (undo/undoall/amend) — lost on exit. Evals ●:
own benchmark runner Nov 2023 (early!), exercism, in-repo SWE-bench results,
LoCoDiff-bench + apples-to-models spinouts — eval-first DNA. Matrix best
cells: cost HUD ●, evals ●; no sandbox/MCP(pre-dated)/compaction/skills;
RSS unpublished.

**ADOPT (queued):** (1) write-authority-scoped-to-read-scope as an OPTIONAL
strict mode (todo 16 guardrail option); (2) client-applies-edits daemon flag
(todo 35/38 — VS Code client applying edits itself); (3) git-diff-anchored
context mode ("continue my WIP" — todo 11/15 mode); (4) revisor-style
malformed-edit self-repair (validates todo 16 corrective subcycles);
(5) per-replacement walk as finest staging granularity under our entity-
grouped hunks. **AVOID:** context-architecture thrash; parallel edit formats
(pick one, evolve carefully); in-memory-only undo; pivot-to-closed starving
the OSS asset; unversioned spinout-lib coupling. **WATCH:** insertion-conflict
ordering UI (niche); mentat.ai domain (dead shell).

## 13. Iteration 6 findings — Plandex death autopsy (combined agent; HEAD e2d7720, dormant)

**DEAD as company, OSS dormant-but-unarchived:** solo founder Dane Schneider
(danenania; 1,301/1,483 commits) joined Promptfoo Oct 2025 (Promptfoo itself
→ OpenAI Mar 2026); Plandex Cloud wound down 2025-11-07; last commit
2025-10-03 = literally "link to cloud wind down post"; 15.6k★ MIT, last
release v2.2.1 Jul 2025. Timeline: initial commit 2023-09-28 (summary-based
history = 3rd commit ever); v1 Show HN Apr 3 2024 (304pts/111c, OpenAI-only);
v1.0.0 May 2024; v2 = 8-month rewrite, GA Mar 18 2025, Show HN Apr 16 2025;
vscode-branch extension WIP (last commit Nov 14 2024) NEVER shipped.
**LICENSE FLIP-FLOP: MIT → AGPL 2024-03-16 (2 wks before HN launch) → MIT
2025-03-10 (before v2 GA)** — trust-burning reversal. v1 death cause (HN):
"great ideas/interface but needed polish — new models weren't well supported,
got in endless cycles" (GPT-4-era models couldn't drive its proposal format).

**Architecture (v2):** client/server — Go server + **Postgres via Docker
compose** (heavy local tax; PLANDEX_BASE_DIR data home); CLI on huh/go-prompt
forks; monorepo restructure cli/server/shared→app (Jan 2024). **Sandbox = git
branches** (plan branches separate from user repo; works in non-git dirs);
trials write to sandbox; per-file diffs reviewed then applied. **_apply.sh
DEFERRED EXECUTION** — commands accumulate as a special sandbox "file",
reviewed + applied ATOMICALLY with file changes (not live exec; debug = the
interactive loop) — unique. **9+ MODEL ROLES** (planner w/ large-context
fallback, architect [v2], coder, summarizer, auto-continue, builder,
whole-file-builder fallback, names, commit-messages) + model packs + **plan
branches for comparing MODELS on the same plan**. **5-level autonomy matrix**
(none/basic/plus/semi/full) × 10 toggles (auto-load-context, smart-context,
auto-update-context, auto-exec, auto-compact…). Context: tree-sitter project
map → architect selects → smart-context sliding window. **Edit race**
(deterministic → o3-mini-orchestrated race: aider-style-diff vs whole-file vs
cloud fine-tuned fast-apply). Rewind w/ auto-revert-on-rewind default TRUE
(v2.0.0 change, reversal documented). Security: RAM-only ephemeral API keys,
.plandexignore. ZERO MCP (grep hits were NumCPU artifacts), no telemetry, no
subagents. Dogfooding artifacts in git ("🤖 Plandex → rewound plan state…";
aider also used on repo). Community: per-role multi-model praised as cost
win; sandbox praised; criticisms — "CLI worst possible interface", "huge diff
after all updates is a real pain" (vs Cline streaming); aider's Paul Gauthier
personally rebutted v1's git-isolation pitch.

**ADOPT (queued):** (1) fine-grained utility model roles (names, commit-
messages, auto-continue on cheap models — extend todo 42's role set);
(2) autonomy-matrix config pattern (levels × toggles — richer than 4-mode
dial, todo 8); (3) **deferred batch-execution MODE** — accumulate commands
with file changes, review+apply atomically (alongside live exec; todo 19/22
option); (4) plan-branches for same-spec model comparison (extends best-of-N,
todo 40); (5) RAM-only ephemeral keys (validates todo 24); (6) edit-fallback
race as WATCH (deterministic→diff→whole-file chain validates ours).
**AVOID:** license flip-flopping; Postgres+Docker local tax (single-binary
validated); deferred-ONLY execution (batch-diff pain); solo-founder cloud/OSS
coupling (third such death); model-specific proposal formats models can't
reliably produce. **WATCH:** fast-apply fine-tuned models (closed cloud).

## 8. Iteration 1 source record — DeepSeek Harness (architecture agent's full findings w/ 35 pinned permalinks)

## 6. Iteration 1 findings — DeepSeek Harness (`dsh`)

Research date 2026-09-11. Repo pinned at master SHA `c291e7961a515f6d7af9304e7fd1d257929aef26`
(clone in temp). Per-claim URLs; UNVERIFIED flags where noted.

### 6.1 Identity (VERIFIED unless flagged)

- **What**: official DeepSeek AI open-source agent harness, repo
  [`deepseek-ai/deepseek-harness`](https://github.com/deepseek-ai/deepseek-harness),
  tagline "Everything is a Plugin.", CLI `dsh`. Same `deepseek-ai` org as the model
  repos (not a fork/community). Official announcement:
  https://deepseek.com/harness/en/ ("developer preview ... Every agent capability is
  implemented as a plugin that can be swapped or recomposed"). Docs:
  https://deepseek-harness.github.io/deepseek-harness/
- **Language/stack**: TypeScript pnpm monorepo (10,319 files in checkout;
  ~500k LOC TS + ~300 lines C11 under the Linux sandbox per
  [Justin3go source review](https://justin3go.com/en/posts/2026/08/15-deepseek-harness-review) —
  secondary). Node ^22.19. Runs as **local Web UI** (`npx @deepseek-ai/dsh web` →
  127.0.0.1:3080), headless CLI, TS SDK (JSON-RPC), Python SDK, ACP server,
  Electron desktop. Primary: repo README L17-41 + architecture.md L19-53
  (all permalinks below at pinned SHA).
- **License**: MIT (GitHub API + LICENSE file, "Copyright (c) 2026 DeepSeek").
- **Created**: 2026-08-13T11:56:32Z (GitHub API). All releases prerelease: latest
  `dsh-v0.1.5-rc.2` 2026-09-10; README warns in caps "THERE WILL BE
  COMPATIBILITY-BREAKING CHANGES"
  ([README L11-13](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/README.md#L11-L13)).
- **Activity**: 219,849 stars / 26,017 forks on 2026-09-11 (GitHub API, primary).
  Velocity (secondary, cross-checked): ~50k in 12h
  ([orcarouter](https://www.orcarouter.ai/blog/deepseek-harness-plugins)), 66,343 by
  Aug 14 ([open-harness.net](https://www.open-harness.net/)), 90k+ by Aug 15
  (Justin3go), 95,386 in ~2d ([Winder.AI](https://winder.ai/ai-agent-harness-comparison/)),
  125,959 by Aug 16 (orcarouter), 181,149 by Aug 22
  ([guancyxx](https://guancyxx.cn/en/blog/agent-harness-open-source-showdown)).
- **Disambiguation**: NOT the older small "deepseek-agent" repos (gh search);
  ecosystem forks/UIs are separate projects (`dataelement/dsh-desktop`,
  `awesome-dsh-plugin` lists). Plugin ecosystem: `dsh-plugin` GitHub topic —
  421 plugin repos within one day ([springbrand](https://springbrand.ai/deepseek-harness), secondary).
- **Contribution model**: UNVERIFIED (two consistent secondary sources,
  [open-harness.net](https://www.open-harness.net/) +
  [springbrand](https://springbrand.ai/deepseek-harness)): no external PRs, Issues
  disabled → GitHub Discussions + Discord. Team lead "Cui Tianyi, ex-Jane Street,
  joined DeepSeek Mar 2026" — UNVERIFIED, single source (open-harness.net).
- **Kernel lineage**: built on [Cordis](https://github.com/cordiverse/cordis)
  ("Meta-Framework of Spatiotemporal Composability", MIT, 8,349 stars), vendored
  under `vendor/` (cordis, cosmokit, hmr, loader, schemastery — Koishi-ecosystem
  packages; the Koishi connection is my inference, UNVERIFIED). Design paper:
  [arXiv:2608.25512](https://arxiv.org/abs/2608.25512)
  ([README L7](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/README.md#L7)).
  Cordis is a *pre-existing* framework dsh adopted, not dsh-original.

### 6.2 Architecture (primary sources = in-repo docs @ pinned SHA)

**Kernel — service-locator, not middleware stack.** Cordis: plugins contribute
services (claim stable `ctx.<key>`), typed events, and reversible effects
(`ctx.effect()`); deps declared via `inject`; unload unwinds registrations.
Five event dispatch modes: `emit`/`waterfall`/`parallel`/`serial`/`bail`;
waterfall = around-middleware (`next()` to delegate)
([cordis-primer L7-38](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/cordis-primer.md#L7-L38)).

**"Everything is a plugin" — literally.** Model adapter, tool registry, session
log, sandbox AND the agent loop are config rows; no privileged core; extend by
mounting a plugin beside the others
([architecture.md L9-13](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L9-L13)).

**Composition: profiles + bundles + layered patches.** A run is a plugin tree
built from ordered layers: profile (named composition: `web`, `headless`, `sdk`,
`sdk-minimal`, `acp`) → its bundles → profile `cordis.patch.yml` → home-level →
`--patch` overlay; any printed row of `dsh --profile web --dump-config` is
replaceable; custom profiles get live patch reload
([architecture.md L15-37](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L15-L37)).

**Agent loop.** `ReactLoopAgent` —
[packages/core/agent-loop/src/agent.ts#L72](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/packages/core/agent-loop/src/agent.ts#L72)
(619-line file; arXiv calls it a "515-line driver, itself a swappable config row" —
[arXiv:2608.23953](https://arxiv.org/pdf/2608.23953)). Turn = 0+ steps; step = one
model request + its tools; durable event sequence
`turn/start → agent/pre-step → step/start → agent/request → llm/stream → tool/* →
step/end → agent/turn-stopping → turn/end`
([architecture.md L80-113](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L80-L113)).
Single inbox; `maxParallelToolCalls` (default 10) with exclusive calls as ordering
barriers; cooperative cancellation preserves already-streamed text
([agent-loop README L73-76](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/packages/core/agent-loop/README.md#L73-L76)).
Interception = event waterfalls (`agent/pre-step` rewrite/reject input,
`agent/request` reroute, `agent/turn-stopping` stop turn). No built-in turn budget
(policy must cancel from `agent/turn-stopping`)
([README L190-200](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/packages/core/agent-loop/README.md#L190-L200)).

**Session/persistence — the strongest differentiator.** Append-only typed
`SessionEvent` log; **"Model-visible means logged"** — runtime-asserted invariant
that anything reaching a model request is reconstructable from the log; model
history is a *projection* (`deriveMessages()`), not storage
([architecture.md L115-123](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L115-L123)).
JSONL+optional-zstd files with **versioned format generations** (v0→v1→v2→v3
migration packages exist in-repo); committed generation paths never
renamed/replaced/deleted; write-open exclusively publishes the successor after
encode+verify; resume appends `interruptedTurnClosers` for crash-repair (semantic
repair is agent-layer, not storage)
([architecture.md L119](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L119)).
Fork = seed new session from log prefix (`parentSession`/`isSeeded` header bits).
Resume/fork/search/replay all read the same stream. Opt-in **session-query** =
SQL over session events (SQLite) + model-facing read-only `session_event_*` tools
([tool-catalog L36](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L36)).

**Context management.** Compaction is an *optional capability seam*
(`ctx.compaction`), not loop-spine: three log-only `compaction/*` events with a
crash-detectable lock bracket (start→summary→replace→end, lock released last);
summary rides a `user/message` with `surfaceOp: replace` masking a span while
original events stay in the log; triggers `pressure` | `context-overflow`;
optional `ctx.toolResultPruner` does deterministic head/middle/tail pruning of
oversized tool results with `compaction/prune` shadow-price events
([compaction.md L9-21](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/subsystems/compaction.md#L9-L21),
[L86-92](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/subsystems/compaction.md#L86-L92)).
**Spill storage**: oversized tool outputs → persisted artifact + opaque locator +
retrieval hint; `spill-policy` swaps result for head/tail preview (best-effort,
save failure keeps inline)
([spill.md L88-92](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/subsystems/spill.md#L88-L92)).
System prompt travels as `system/message` surface nodes; on capable routes a
non-empty prompt change is appended *in-history after cached prefix* to preserve
KV reuse
([agent-loop README L158-160](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/packages/core/agent-loop/README.md#L158-L160)).

**Tool system.** `ctx.tools` registry + guarded pipeline
(`tools/pre-execute → execute → post-execute`); schemas join prompt assembly;
per-agent scoped toolsets via presets + `isolate` realm
([architecture.md L125-141](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L125-L141)).
Built-ins (generated, boot-verified
[tool-catalog L16-43](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L16-L43)):
bash/pwsh (+persistent PTY variants), edit/read/write/read_image,
str_replace_editor, glob/grep (bundled ripgrep), web_search/web_fetch,
terminal_*, job_*, todo_write, skill, lsp, ask_user_question, schedule_*,
goal tools, subagent (+send_message/interrupt_agent/list_agents), workflow,
exit_plan_mode, present (deliverables). MCP client + skills packages in-tree.

**Sandbox.** `ctx.sandbox` seam: Linux bwrap/Landlock, macOS Seatbelt, **Windows
ACL restricted token** — 3-OS native at launch; modes `read-only` |
`workspace-write` | `danger-full-access`; enforcement is a *reported fact*
(`full` | `partial` — old Landlock ABIs + Windows ambient-ACL gaps are `partial`);
policy resolved **per call**, not per provider; runner-failure vs command-denial
classified via stderr signatures; if sandbox cannot start, the command is stopped
([sandbox.md L11-39](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/subsystems/sandbox.md#L11-L39);
[DataCamp hands-on](https://www.datacamp.com/blog/deepseek-harness-vs-claude-code)).
E2B cloud sandbox exists as sibling seams (`fs-e2b`, `subprocess-e2b`) — pointing
fs+subprocess at a remote world moves Bash/PTY/LSP with it
([architecture.md L127-129](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L127-L129)).

**Provider coupling — NOT DeepSeek-locked.** Two adapters: `dsh-llm-deepseek`
(official API, first-party) and `dsh-llm-pi-ai` — generic adapter that **depends
on `@earendil-works/pi-ai` ^0.85.1** (Earendil's pi — the arXiv paper's disclosed
non-independence; verified:
[package.json#L44](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/packages/llm/llm-pi-ai/package.json#L44)).
Custom providers via settings.yaml/Web UI: `openai-completions` |
`openai-responses` | `anthropic-messages` protocols, any OpenAI-compatible
endpoint (Ollama/vLLM); ~40 providers claimed (Justin3go, secondary). "dsh can
host Claude, but Claude Code can't host DeepSeek"
([DataCamp](https://www.datacamp.com/blog/deepseek-harness-vs-claude-code)).
DeepSeek-only **wire extensions** on the official route:
`x-deepseek-harness-*` headers
([L21-31](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/deepseek-llm-api-wire-extensions.md#L21-L31));
`dsh_plugin_packages` (**default-ON** plugin-inventory telemetry to the API);
`dsh_session_log` (opt-in; uploads contiguous session-log suffix with
`delivery-accepted` watermark events)
([L41-158](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/deepseek-llm-api-wire-extensions.md#L41-L115)).

**Delegation.** Subagent backends behind one interface: in-process fork/spawn,
dsh-SDK child, ACP, and **`subagent-claude-code` / `subagent-codex`** — drives
Claude Code/Codex CLIs as child agents
([Winder.AI](https://winder.ai/ai-agent-harness-comparison/);
[subagent packages](https://github.com/deepseek-ai/deepseek-harness/tree/c291e7961a515f6d7af9304e7fd1d257929aef26/packages/subagent)).
Speaks ACP both directions. Experimental **Agent Teams**: durable roster + task
board + mailbox over continuable subagents (`spawn_teammate`, `wait_agent`, ...)
([architecture.md L131](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L131)).
Hook-protocol compatibility packages for Claude Code & Codex hooks
(`hooks-claude-code`, `hooks-codex`).

### 6.3 Notable concepts (unique/unusual vs Claude Code/Codex/goose/OpenHands)

1. **The loop is a config row.** Only harness in the field where the agent loop
   itself is swapped via profile/patch (sdk-minimal even ships a different
   explicit tree). Compare: CC/Codex loops are fixed products. (architecture.md
   L9-13; [arXiv:2608.23953](https://arxiv.org/pdf/2608.23953) "dsh bets on the
   architecture".)
2. **"Model-visible ⇒ logged" runtime invariant** + history-as-projection.
   OpenHands V1 event-sources too, but dsh *asserts* reconstructability at runtime
   (`runtime-diagnostics/invariants`, `agent-loop/src/invariant.ts`)
   ([architecture.md L121](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/architecture.md#L121)).
3. **Session format as versioned API**: format generations with adjacent
   migration packages, exclusive successor publication, immutable committed
   paths — a storage-migration doctrine nobody else ships.
4. **Per-capability Token-effect / KV-Cache-effect doc sections.** Every package
   README documents what a feature costs in tokens and what it does to
   prefix-cache reuse
   ([agent-loop README L145-188](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/packages/core/agent-loop/README.md#L145-L188));
   arXiv: "a written per-capability contract stating each plugin's token and
   KV-cache cost" — no mainstream harness does this.
5. **Replay-based token meter** (`ctx.tokenMeter`): O(surface) measurement over
   the durable log, anchored to latest provider usage when envelopes match,
   route-declared image pricing
   ([token-meter.md](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/subsystems/token-meter.md#L5-L29)).
6. **PTC `run_code`**: model writes a TypeScript program that calls tools as
   code; bridged sub-calls re-enter the guarded pipeline (programmatic tool
   calling, as a tool)
   ([tool-catalog L19](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L19),
   [L121-126](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L121-L126)).
7. **Self-referential runtime**: opt-in `cordis_*` tools let the model
   define/undefine/inspect/run Cordis packages (vm sandbox) and register NEW
   model-visible tools mid-session
   ([tool-catalog L24](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L24)).
8. **Spill-to-disk tier** for oversized tool outputs with model-facing locators
   + retrieval hints (goose/OpenHands lack this seam).
9. **Honest sandbox enforcement** (`full`/`partial` reported; Windows ACL gaps
   admitted) + per-call policy — vs CC's binary permission modes.
10. **Ralph workflow tool** (`tool-ralph`): fixed foreground loop spawning a
    fresh structured child per round ("Ralph technique" productized) + generic
    `workflow` scripting tool
    ([tool-catalog L34](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L34)).
11. **Goals as first-class durable objects** (`ctx.goals`, create/get/update_goal,
    round-bounded blocked state)
    ([tool-catalog L31](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L31)).
12. **Model searches its own past**: SQL-backed session-query + `session_event_*`
    tools
    ([tool-catalog L36](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L36)).
13. **Docs-as-tested-artifacts**: tool catalog generated by *booting every tool
    plugin*; Cordis catalog generated from source AST, freshness CI-verified;
    postmortems + decision notes live in-repo
    ([tool-catalog L8](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/tool-catalog.md#L8)).
14. **Cross-agent convention memory**: reads `CLAUDE.md` + `AGENTS.md`; skills
    from filesystem
    ([DataCamp FAQ](https://www.datacamp.com/blog/deepseek-harness-vs-claude-code)).
15. **Telemetry negative**: `dsh_plugin_packages` default-ON sends active plugin
    inventory (names+versions) to the DeepSeek API on every official request
    ([wire-extensions L41-72](https://github.com/deepseek-ai/deepseek-harness/blob/c291e7961a515f6d7af9304e7fd1d257929aef26/docs/deepseek-llm-api-wire-extensions.md#L41-L72)) —
    conflicts with our AVOID "telemetry default-on" (§3); keep ours default-off.

### 6.4 Memory / performance characteristics

- **Context economics**: append-only derivation + in-history prompt appends keep
  provider prefix caches valid; compaction+pruner + spill keep surfaces small;
  token meter drives pressure triggers. Plausibly explains the token numbers
  below.
- **Benchmark (secondary, single source)**: Composio 30-task test, one model
  (DeepSeek V4 Pro) across harnesses: dsh 20/30 pass @ **~88k tokens/task**; Pi
  21/30 @ ~925k; Claude Code 19/30 @ ~650k ($0.074/success, 181.8s median) —
  dsh won token efficiency ~7-10x
  ([BSWEN writeup](https://docs.bswen.com/blog/2026-08-30-deepseek-harness-vs-claude-code-vs-codex-vs-pi/),
  citing Composio). UNVERIFIED primary (Composio source not fetched); n=30, one
  model, prerelease versions.
- **Footprint**: Node >=22.19 required; local Web UI; no published RSS numbers
  found (UNVERIFIED gap). Build dominated by ~500k-LOC TS monorepo (Justin3go,
  secondary).
- **Cost**: MIT, no license fee; bill = model API (DeepSeek peak/off-peak pricing
  since 2026-08-16 —
  [DataCamp](https://www.datacamp.com/blog/deepseek-harness-vs-claude-code)).
- **Stability**: prerelease churn is the headline cost (Winder.AI runs it
  "because it is interesting, and we would not put a client's release process
  behind it yet"; README caps-locked breaking-changes warning).

### 6.5 Candidate ADOPT/AVOID for OUR Rust harness (evidence above)

**ADOPT candidates**: "model-visible ⇒ logged" invariant + projection seam;
versioned session-format generations w/ immutable paths + exclusive publication;
Token-effect/KV-Cache-effect sections in our crate docs; spill tier for
oversized tool outputs; honest full/partial sandbox enforcement + per-call
policy; PTC-style run_code; 5-mode event dispatch taxonomy
(emit/waterfall/parallel/serial/bail); boot-verified generated tool catalog;
in-history prompt append for KV-prefix preservation; session-event SQL query
tools.

**AVOID candidates**: default-on plugin-inventory telemetry; shipping-prerelease
APIs without stability tiers; V8/JS-object plugin kernel (Cordis is in-process
JS plugins — we prefer process/WASM boundaries; consistent with §3 AVOID
"V8/dylib plugins").

**WATCH**: dsh stabilization trajectory through late 2026; ACP + `dsh-plugin`
topic as de-facto plugin standards; whether `dsh_session_log` upload becomes an
industry pattern (server-side session memory).

### 6.6 Sources

Primary (pinned SHA c291e79): README, docs/architecture.md, docs/cordis-primer.md,
docs/tool-catalog.md, docs/subsystems/{compaction,spill,token-meter,sandbox}.md,
docs/deepseek-llm-api-wire-extensions.md, docs/user/guide/providers.md,
packages/core/agent-loop/{README.md,src/agent.ts},
packages/llm/llm-pi-ai/package.json.
Secondary: [Winder.AI](https://winder.ai/ai-agent-harness-comparison/) (2026-08-20) ·
[arXiv:2608.23953](https://arxiv.org/pdf/2608.23953) source-level study ·
[arXiv:2608.25512](https://arxiv.org/abs/2608.25512) Cordis paper ·
[deepseek.com/harness](https://deepseek.com/harness/en/) ·
[guancyxx teardown](https://guancyxx.cn/en/blog/agent-harness-open-source-showdown) ·
[DataCamp](https://www.datacamp.com/blog/deepseek-harness-vs-claude-code) hands-on ·
[BSWEN/Composio benchmark](https://docs.bswen.com/blog/2026-08-30-deepseek-harness-vs-claude-code-vs-codex-vs-pi/) ·
[Justin3go source review](https://justin3go.com/en/posts/2026/08/15-deepseek-harness-review) ·
[open-harness.net](https://www.open-harness.net/) ·
[springbrand](https://springbrand.ai/deepseek-harness) ·
[orcarouter](https://www.orcarouter.ai/blog/deepseek-harness-plugins) ·
[digitalapplied](https://www.digitalapplied.com/blog/deepseek-harness-open-source-agent-framework-2026) ·
[cordiverse/cordis](https://github.com/cordiverse/cordis).



## 10. Iteration 3 findings — Roo Code (mode-scoped origin; SHA b867ec9, 2026-09-11)

Research date 2026-09-11. Repo pinned at FINAL main SHA
`b867ec9145750d0ae1ff7f02d35406e9bf2a0b16` (archived; clone in temp). Repo
URLs = permalinks at that SHA. Per-claim URLs; UNVERIFIED flagged.

### 10.1 Identity + lifecycle (VERIFIED unless flagged)

- **What**: Roo Code — VS Code agentic-coding extension, Apache-2.0,
  RooCodeInc/Roo-Code, "a whole dev team of AI agents in your code editor".
  Archive banner: shut down May 15th; directs users to
  [ZooCode](https://github.com/Zoo-Code-Org/ZooCode) (community fork) and
  [Cline](https://cline.bot) ("from where Roo Code originated");
  billing@roocode.com for refunds. https://github.com/RooCodeInc/Roo-Code
- **Numbers**: 24,303★ / 3,419 forks / `parent: null` (GitHub detached the
  fork link — it began as a Cline fork) / created 2024-10-31 / pushed
  2026-05-15 (GitHub API, 2026-09-11). 3M installs (announcement). VS Code
  publisher ID STILL `RooVeterinaryInc.roo-cline` — fossil of the RooVet
  origin (marketplace link in repo docs footer).
- **Timeline** (in-repo tags + CHANGELOG):
  - Created as RooVetGit/Roo-Cline; `RooVetGit/Roo-Cline` API-redirects to
    RooCodeInc/Roo-Code (verified 2026-09-11). Late-Oct-2024 fork of Cline
    at Cline's v2.x.
  - Version lineage INHERITED from Cline: repo tags v1.0.0 = 2024-07-27 and
    v2.0.0 = 2024-10-09 are Cline's own tags (match iteration-2 dates); Roo
    continued numbering: **v3.0.0 = 2024-12-18** (tag commit dates via API).
  - **v3.1.0**: per-mode role/instructions for Code/Architect/Ask — modes
    origin (CHANGELOG L3169). **v3.2.0-3.2.2** (tag 2025-01-21): "Name
    Change From Roo Cline to Roo Code ... beyond 50,000 installations"
    (CHANGELOG L3127). **v3.3.6** (Feb 2025): `new_task` tool
    (update-notes/v3.3.6.md). **v3.3.20** (2025-02-14): project custom
    modes in `.roomodes` (L2949). **v3.11.9** (2025-04-07):
    `.roo/rules/` + `.roo/rules-{mode}/` folders (L2542).
  - 2025: 3M installs; Roo Code Cloud + Roo Code Router products.
    **v3.43.0** (2026-01-23): Intelligent Context Condensation v2;
    **v3.44.0** (2026-01-26): worktrees (CHANGELOG).
  - **2026-04-21/22**: CEO Matt Rubens announces sunset ([X post](https://x.com/mattrubens/status/2046636598859559114);
    full text mirrored [aVenture](https://aventure.vc/news/roo-code-shuts-down-to-go-all-in-on-roomote),
    [TestingCatalog](https://testingcatalog.net/roo-code-hits-3-million-installs-then-gets-shut-down-for-a-bigger-bet/)).
  - **2026-05-15**: repo archived; last extension release; unused balances
    refunded. Final CHANGELOG entry **3.53.0** (in-repo); secondary sources
    ([aicoderscope](https://aicoderscope.com/blog/roo-code-shut-down-alternatives-2026/))
    report a v3.54.0 marketplace build — UNVERIFIED, minor discrepancy.
- **Roomote pivot**: cloud agent in Slack; Linear/GitHub/Sentry integrations;
  model-agnostic; self-verifies via screenshots + full local env; "not a
  tool, but a real teammate." Stated motive: from Fall 2025 they ran Roo
  headless in cloud containers with full auto-approve to chew their own
  backlog ("prompt in to high-quality PR out"); "forks redistributed our
  work as fast as we shipped it"; "winning at something that we did not
  think mattered anymore." Coverage: [The New Stack](https://thenewstack.io/roo-code-cloud-ides-ai-coding/)
  (2026-04-22; includes Kilo's counter "The IDE is not over").
- **Farewell stats** (Rubens, [LinkedIn 2026-05-19](https://www.linkedin.com/posts/roomotedev_the-end-of-an-era-cheers-to-everyone-who-activity-7462350226192990208-K1Ir)):
  18 months, 24.1k stars, **492 open PRs, 550 open issues**, 1,897 commits
  by Rubens; "Two of our top contributors were Roomotes. Both bots."
- **Roomote repo**: [RooCodeInc/Roomote](https://github.com/RooCodeInc/Roomote)
  created 2026-07-07, 215★, license "Other" (source-available,
  self-hostable, Render template; explicit Devin contrast: "You can read
  every line of code it runs").
- **Successors**: ZooCode (community continuation, named in archive banner);
  Cline welcome-back — Saoud Rizwan: Roo "contributed to Cline more than
  anyone else" (Reddit, via The New Stack); Kilo Code (Roo fork; rebuilt on
  OpenCode core Apr 2026; [migration guide](https://kilo.ai/articles/roo-to-kilo-migration-guide);
  ["Thank you, Roo"](https://blog.kilo.ai/p/thank-you-roo)).
- **Vendor self-attribution — FLAG**: announcement says they added "what is
  now widely known as dangerously-skip-permissions." Auto-approve-first
  positioning is real (see 10.2 auto-approval), but "pioneer" is a vendor
  claim; Cline shipped early auto-approve too (iteration 2).

### 10.2 Architecture (primary = pinned source + docs)

**Modes system — the divergence from Cline.**
- `DEFAULT_MODES` ordered array: architect / code / ask / debug /
  orchestrator; each = slug, name, roleDefinition (system-prompt head),
  whenToUse (MACHINE-facing routing guidance), description (UI),
  customInstructions (prompt tail), groups.
  [packages/types/src/mode.ts#L168](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/packages/types/src/mode.ts#L168)
  Verbatim defaults: architect = read + edit(\.md$ via fileRegex) + mcp;
  ask = read + mcp ONLY; debug = read+edit+command+mcp with "reflect on 5-7
  possible sources" hypothesis instructions; **orchestrator = `groups: []`
  (zero domain tools)**.
- Tool groups: `read, edit, command, mcp, modes` ("browser" deprecated and
  auto-stripped by schema for back-compat).
  [tool.ts#L9-L16](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/packages/types/src/tool.ts#L9-L16)
- Group→tools map + customTools (opt-in alias tools: edit/search_replace/
  edit_file/apply_patch):
  [tools.ts#L296-L315](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/shared/tools.ts#L296-L315).
- **ALWAYS_AVAILABLE_TOOLS** in every mode regardless of groups:
  ask_followup_question, attempt_completion, switch_mode, new_task,
  update_todo_list, run_slash_command, skill.
  [tools.ts#L317](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/shared/tools.ts#L317)
- Per-mode file restrictions: edit group tuple `[edit, {fileRegex,
  description}]`; violations raise `FileRestrictionError` at tool level.
  [modes.ts#L134-L143](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/shared/modes.ts#L134-L143)
- **Custom modes = THE extension mechanism**: zod `ModeConfig` (slug regex,
  roleDefinition required, whenToUse, groups, source global|project); custom
  modes override built-ins BY SLUG (merge in getAllModes). YAML preferred
  (comments/multi-line), JSON supported; global `custom_modes.yaml` vs
  project `.roomodes`; JSON→YAML one-time auto-migration w/ backup;
  import/export bundles mode + INLINE rulesFiles + slug-rename path fixing;
  mode marketplace; "Ask Roo to create a mode" flow.
  [custom-modes docs](https://roocodeinc.github.io/Roo-Code/features/custom-modes)
- **Per-mode model binding**: provider profiles carry `modeApiConfigs:
  Record<mode-slug, profileId>`
  ([ProviderSettingsManager.ts#L30](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/config/ProviderSettingsManager.ts#L30),
  L478-L497) + "Sticky Models" (last model per mode remembered — docs) +
  sticky provider profile persisted per task (CHANGELOG).
- **Dogfood proof**: Roo's own repo ships `.roomodes` with 8 project modes
  (translate w/ fileRegex-restricted edit; docs-extractor can only write
  `\.roo/extraction/`; issue-fixer / pr-fixer / merge-resolver /
  issue-investigator / issue-writer) + `.roo/rules-{slug}/` dirs +
  `.roo/skills/` + `.roo/commands/` (root of pinned SHA).

**Orchestrator / boomerang tasks.**
- `new_task(mode, message, todos?)` — todos = markdown checklist parsed to
  TodoItem[] (optionally REQUIRED via `newTaskRequireTodos`); after
  approval → `provider.delegateParentAndOpenChild` PAUSES parent, opens
  child as sole active task.
  [NewTaskTool.ts#L20-L127](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/tools/NewTaskTool.ts#L20-L127);
  [ClineProvider.ts#L2783](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/webview/ClineProvider.ts#L2783)
- Return path: child signals via attempt_completion; ONLY the result
  summary re-enters parent context; per-subtask approval on create+complete;
  task-hierarchy navigation. [boomerang-tasks docs](https://roocodeinc.github.io/Roo-Code/features/boomerang-tasks)
- **Deliberately tool-less coordinator** — official FAQ: defaulting
  Orchestrator to read tools "causes the context to become filled with file
  reads"; documented as context-poisoning prevention; override = custom
  mode with groups:[read] (same page).
- Robustness scars: "Prevent parent task state loss during orchestrator
  delegation" (#11281); delegation-REPAIR code rewrites parent metadata
  delegated→active when stack corrupted
  ([ClineProvider.ts#L394-L445](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/webview/ClineProvider.ts#L394-L445));
  "Metadata-driven subtasks with automatic parent resume and single-open
  safety" (#9090); new_task creates a checkpoint like write_to_file (#10982).
- `switch_mode` (in-task mode change; vs new_task = isolated child):
  approval required, no-op rejected, prompt rebuilt via handleModeSwitch,
  then `delay(500)` before next tool.
  [SwitchModeTool.ts#L41-L67](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/tools/SwitchModeTool.ts#L41-L67).
  `whenToUse` feeds both switch_mode and orchestration choices (docs).

**Checkpoints — repo-per-task SHADOW git (contrast w/ Cline's saga).**
- `RepoPerTaskCheckpointService`; shadowDir = VS Code globalStorage
  (outside the user's repo); init REFUSES homedir/Desktop/Documents/
  Downloads workspaces; warns on nested user repos; `createSanitizedGit`
  strips GIT_DIR / GIT_WORK_TREE / GIT_INDEX_FILE / GIT_OBJECT_DIRECTORY /
  GIT_ALTERNATE_OBJECT_DIRECTORIES / GIT_CEILING_DIRECTORIES /
  GIT_TEMPLATE_DIR so inherited env can't retarget the shadow repo;
  ripgrep-based excludes; init-timeout UI warning then auto-disable.
  [ShadowCheckpointService.ts#L27-L140](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/services/checkpoints/ShadowCheckpointService.ts#L27-L140);
  [checkpoints/index.ts#L27-L120](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/checkpoints/index.ts#L27-L120).
  Roo NEVER renames the user's .git (upstream Cline's original sin,
  iteration 2).

**Context management.**
- Primary: Intelligent Context Condensing v2 (v3.43.0) — summarize at
  threshold (slider default 100%; MIN 5 / MAX 100 % of window;
  [condense/index.ts#L109-L121](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/condense/index.ts#L109-L121));
  **same-model-only condensing policy** w/ documented rationale ("Using a
  different model to condense can degrade summary quality... avoids
  translation errors"); custom prompt stored as customSupportPrompts.CONDENSE;
  UI shows before/after tokens + cost of the condense call.
  [intelligent-context-condensing docs](https://roocodeinc.github.io/Roo-Code/features/intelligent-context-condensing)
- Fallback: **NON-destructive sliding-window truncation** — messages tagged
  `truncationParent` (kept in session), first message always retained,
  marker message inserted at boundary, restorable when user rewinds past
  truncation point.
  [context-management/index.ts#L50-L130](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/context-management/index.ts#L50-L130)
- Error recovery: provider context-window-exceeded detection → truncate 25%
  → retry (docs §Automatic Error Recovery). Reservation: 30% of window
  (20% output + 10% buffer), 70% history. Smart Code Folding: condensation
  preserves ~50k-char budget of signatures/types of recently-touched files
  (CHANGELOG v3.45-era, PR #10942 hannesrudolph).
- Orchestrator isolation is itself a context-management device (docs FAQ).

**Rules / memory / knowledge.**
- Load order: global `~/.roo/rules/` → project `.roo/rules/` → optional
  subfolder rules; mode-scoped `.roo/rules-{slug}/` PREFERRED,
  `.roorules-{slug}` fallback, `.clinerules-{slug}` legacy; files injected
  as "# Rules from <path>:"; symlink resolution depth 5.
  [custom-instructions.ts#L206](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/prompts/sections/custom-instructions.ts#L206)
- AGENTS.md / AGENT.md / AGENTS.local.md supported (same file).
- **Memory Bank**: 2025-era feature (v3.3.6 note references memory-bank
  workflows; Kilo's migration table maps "Memory Bank → AGENTS.md" and
  ships it only in legacy Kilo v5.x). ABSENT from the final docs tree
  (`apps/docs/docs/features/` has no memory-bank page) — superseded by
  rules/skills/AGENTS.md. UNVERIFIED: whether removal was explicit policy.
- `.rooignore` (gitignore-style tool visibility); codebase indexing =
  embeddings + Qdrant (Kilo guide names Qdrant; their port PR #6966 still
  in review) — Roo's one heavy-index bet vs Cline's no-index philosophy.
- MCP client: stdio/SSE/streamable-HTTP + marketplace tab; skills; slash
  commands (`.roo/commands/`).
- Auto-approval: per-action flags + `allowedCommands`/`deniedCommands`
  matchers, MCP scoping; "YOLO" mode renamed "BRRR" (CHANGELOG).
  [auto-approval/index.ts#L34-L120](https://github.com/RooCodeInc/Roo-Code/blob/b867ec9145750d0ae1ff7f02d35406e9bf2a0b16/src/core/auto-approval/index.ts#L34-L120)
- Worktrees feature (v3.44.0, credit "thanks Cline!" — upstream
  convergence); core.worktree canonicalization fix (Windows paths).

### 10.3 Notable concepts (unique/unusual vs Cline/Claude Code/goose)

1. **Mode-scoped tool masking** (per-mode tool groups + fileRegex + per-mode
   model) — Roo is the ORIGIN (v3.1.0 Jan 2025, pre-dating Cline plan/act);
   our plan adopts it; provenance now pinned.
2. **Deliberately tool-less coordinator** — orchestrator denied reads BY
   DESIGN with a written context-poisoning rationale (nobody else ships a
   tool-less coordinator).
3. **`whenToUse` as machine-readable routing field** — prompt-embedded
   router used by switch_mode + new_task (vs human-only descriptions).
4. **Structured handoff in delegation** — new_task `todos` markdown
   checklist → TodoItem[]; optional enforcement setting.
5. **switch_mode vs new_task duality** — in-place mode change (same context,
   new prompt+tools) vs isolated child (own context). Two delegation
   granularities.
6. **Context-poisoning as user-facing doctrine** — dedicated docs page:
   symptoms/causes/"treat that session as disposable". Only harness we've
   seen shipping this as docs.
   [context-poisoning](https://roocodeinc.github.io/Roo-Code/advanced-usage/context-poisoning)
7. **Non-destructive truncation** — tagged+restorable (vs Cline hard-trim).
8. **Same-model condensing policy** w/ published rationale.
9. **Sanitized-env shadow git per task** (env-var strip list = deploy-
   relevant hardening we should copy).
10. **Per-mode model = cost-tiering pattern** — cheap model in code/debug,
    frontier in architect; Kilo's guide: "usually the difference between a
    $5/day and a $30/day BYOK bill."
11. **Bots as first-class contributors** — `app/roomote` PRs throughout the
    CHANGELOG; farewell credits two bot Roomotes as top contributors.
12. **Portable persona bundles** — mode export = YAML w/ inline rulesFiles +
    slug-rename handling (persona+rules as shareable artifact).

### 10.4 Roo vs upstream Cline — better / worse

**BETTER:**
- Tool-masking + role specialization vs Cline's 2-state plan/act — HN:
  "RooCode was genuinely great (those various agent modes, vs just
  plan/act)" ([HN 47851734](https://news.ycombinator.com/item?id=47851734)).
- Context hygiene by construction: orchestrator isolation + condense v2 +
  non-destructive truncation + code folding, vs Cline's maintainer-admitted
  token-hungry all-reads-in-context architecture (iteration 2, #8336).
- Checkpoint design born clean (repo-per-task shadow, env-sanitized, never
  touches user .git) while Cline carried the .git-rename legacy to 2026.
- Granular auto-approve (per-tool + command allow/deny) enabled the first
  production headless agent pipeline (Roomote origin story).
- Velocity: "Cline has incorporated much of what we built" (Roo archive
  banner); Cline CEO: Roo "contributed to Cline more than anyone else";
  worktrees landed in Roo crediting Cline and vice-versa — bidirectional
  absorbtion, Roo usually first.

**WORSE:**
- **Complexity curve**: "powerful but had a learning curve; a lot of people
  never used more than two modes anyway"
  ([aicoderscope](https://aicoderscope.com/blog/roo-code-shut-down-alternatives-2026/),
  secondary).
- **Orchestrator judged overhead by its own successor**: Kilo DEPRECATED it
  — "the explicit coordinator pattern added overhead without adding
  capability"; replaced by native subagent delegation from full-tool agents;
  flat 2-level hierarchy (subagents can't spawn subagents) as of v7.2.10
  ([Kilo migration guide](https://kilo.ai/articles/roo-to-kilo-migration-guide)).
  DIRECT WARNING for our fork-join design: keep delegation from ANY agent.
- **Memory Bank proprietary format died** with its docs; open standards
  (AGENTS.md) outlived it (Kilo map). Lesson for Skill Forge formats.
- **Fork economics lost**: Apache-2.0 fork competing with the branded
  upstream it forked; "forks redistributed our work as fast as we shipped
  it"; ended archived w/ 492 open PRs + 550 issues; 3M users stranded into
  a 4-way migration scramble. Cline raised $32M and survives.
- **Qdrant/embedding index bet dropped** by successor (not yet ported,
  PR #6966) — heavy-index features are migration liabilities.
- Same gaps as Cline: no OS sandbox (policy-only), no MCP server mode,
  Windows rough edges (core.worktree path bug).
- Token efficiency (secondary, anecdotal): migrating users report post-Roo
  harnesses "burn tokens faster" — per-mode cheap models + lean reads were
  Roo's real economics (aicoderscope). UNVERIFIED.

### 10.5 Candidate ADOPT/AVOID deltas for OUR plan (ledger §3 updated)

ADOPT: mode-scoped tool masking + fileRegex (core of plan — provenance
pinned); per-mode model profiles + sticky binding (todo 42 validated);
tool-less planner-role agent w/ summary-only return — BUT delegation must
also exist from full-tool agents (Kilo lesson); structured todo handoff in
spawn; non-destructive truncation tags; same-model condensing; sanitized-env
shadow checkpoints (todo 23 validated + add env-strip list);
delegation-state repair on resume (test case); command allow/deny matchers;
context-poisoning doctrine page in our docs.

AVOID: proprietary memory formats (use AGENTS.md std); coordinator-ONLY
delegation path; heavy vendor-controlled index as core feature; building
product identity on an unquiet fork without governance/CL strategy.

WATCH: ZooCode viability (does the community fork hold the modes UX?);
Roomote source-available cloud-agent model; Kilo's flat-subagent outcomes
vs orchestrator; whether Cline absorbs custom-modes (they already adopted
skills + checkpoints patterns).

### 10.6 Sources

Primary (pinned b867ec9145750d0ae1ff7f02d35406e9bf2a0b16):
packages/types/src/mode.ts · packages/types/src/tool.ts ·
src/shared/{modes,tools}.ts · src/core/tools/{NewTaskTool,SwitchModeTool}.ts ·
src/core/webview/ClineProvider.ts · src/core/config/ProviderSettingsManager.ts ·
src/core/context-management/index.ts · src/core/condense/index.ts ·
src/core/prompts/sections/custom-instructions.ts ·
src/core/auto-approval/index.ts ·
src/services/checkpoints/ShadowCheckpointService.ts ·
src/core/checkpoints/index.ts · .roomodes · .roo/ tree · CHANGELOG.md ·
apps/docs/docs/{features,advanced-usage,update-notes}/*.
Docs site (final, May 2026): roocodeinc.github.io/Roo-Code/{features/boomerang-tasks,
features/custom-modes,features/intelligent-context-condensing,advanced-usage/context-poisoning}.
GitHub API: repo meta, tags v1-v4 commit dates, releases (3 pages), redirect
check RooVetGit/Roo-Cline.
Secondary: [The New Stack](https://thenewstack.io/roo-code-cloud-ides-ai-coding/) ·
[aVenture](https://aventure.vc/news/roo-code-shuts-down-to-go-all-in-on-roomote) ·
[TestingCatalog](https://testingcatalog.net/roo-code-hits-3-million-installs-then-gets-shut-down-for-a-bigger-bet/) ·
[Kilo blog](https://blog.kilo.ai/p/thank-you-roo) ·
[Kilo migration guide](https://kilo.ai/articles/roo-to-kilo-migration-guide) ·
[HN 47851734](https://news.ycombinator.com/item?id=47851734) ·
[Rubens farewell LinkedIn](https://www.linkedin.com/posts/roomotedev_the-end-of-an-era-cheers-to-everyone-who-activity-7462350226192990208-K1Ir) ·
[RooCodeInc/Roomote](https://github.com/RooCodeInc/Roomote) ·
[aicoderscope](https://aicoderscope.com/blog/roo-code-shut-down-alternatives-2026/) ·
[Bodega One](https://www.bodegaone.ai/blog/roo-code-shutdown-alternatives).

## 11. Iteration 4 findings — Kilo Code (fork→rebuild→exit; SHA 3d04228, 2026-09-11)

Research date 2026-09-11. Repo `Kilo-Org/kilocode` (NOT kilocode/kilo-code — clone fails)
pinned at main SHA `3d04228b6a642acb3daf68a649269618b6018250` (release v7.6.2; clone in
temp). Per-claim URLs; UNVERIFIED flagged.

### 11.1 Identity + evolution (VERIFIED unless flagged)

- **What**: Kilo ("Kilo Code") — open-source agentic-coding platform, MIT, repo created
  2025-03-10, 27,088★/3,103 forks (2026-09-11). Tagline "all-in-one agentic engineering
  platform"; "Kilo CLI is a fork of OpenCode, enhanced to work within the Kilo agentic
  engineering platform" (repo README). https://github.com/Kilo-Org/kilocode
- **Lineage**: fork of Roo Code (itself forked from Cline 2024-10-31); "The two codebases
  share real git history" (migration guide). Founders: Sid Sijbrandij (GitLab co-founder/
  ex-CEO), Scott Breitenother, Emilie Schario (Anaconda blog by Dawn Wages); founding CEO
  Jan Paul Posma stepped back Sept 2025 → Breitenother CEO (The New Stack; UNVERIFIED
  single-source for Posma). Launched Mar 2025; Product Hunt May 2025 (PH-return post,
  2026-05-05: "A year ago, Kilo Code launched on Product Hunt").
- **Superset strategy (2025)**: VS Code ext (Roo-derived) + JetBrains + CLI + Kilo Gateway
  (500+ models, ZERO-markup BYOK — vs Roo's at-cost Router) + free-credits growth hack
  ("speedrunning open source coding AI"; launch-week credit abuse + billing pressure
  documented in "What We Learned from 3 Million Downloads").
- **$8M seed 2025-12-10**: led by Cota Capital; Breakers, General Catalyst, Quiet Capital,
  Tokyo Black ([BusinessWire](https://www.businesswire.com/news/home/20251210195261/en/Kilo-Raises-%248-Million-to-Bring-Kilo-Speed-to-Agentic-Engineering);
  [CNBC](https://www.cnbc.com/2025/12/10/former-gitlab-ceo-raises-8-million-for-kilo-to-compete-in-vibe-coding.html)).
  At seed: 750k downloads, "#1 on OpenRouter", 6.1T tokens/mo (BusinessWire); "most widely
  used service for startup OpenRouter", 3T tokens/past month per OpenRouter (CNBC). GitLab
  Right-of-First-Refusal agreement until Aug 2026 (VentureBeat) — never exercised (my
  inference from the Anaconda sale).
- **THE REBUILD (why + what)**: Feb 2026 — CLI rebuilt on OpenCode server ("Kilo CLI 1.0",
  ~Feb 2026, [VentureBeat](https://venturebeat.com/orchestration/kilo-cli-1-0-brings-open-source-vibe-coding-to-your-terminal-with-support)/[HackerNoon](https://hackernoon.com/kilo-cli-10-the-complete-cli-for-agentic-engineering);
  exact date UNVERIFIED). VS Code extension pre-release 2026-03-10 ([blog](https://blog.kilo.ai/p/we-completely-rebuilt-the-kilo-vs-code-extension));
  **GA 2026-04-02** ([blog](https://blog.kilo.ai/p/new-kilo-for-vs-code-is-live)).
  Why (their words): "the old extension (the Roo-derived codebase) had VS Code internals
  included in every surface. That capped what we could ship. The CLI and Cloud Agents had
  to reimplement too much" ([migration guide](https://kilo.ai/articles/roo-to-kilo-migration-guide)) +
  "the old architecture had us maintaining two different code bases... the CLI agentic flow
  is much better" ([week-one post](https://blog.kilo.ai/p/new-vs-code-extension-week-one-what)).
  Result: one engine (OpenCode server) across VS Code/CLI/Cloud Agents; shared `kilo.jsonc`;
  sessions carry between surfaces.
- **What survived**: migration wizard + legacy trap-door — v5.x extension (Roo-derived,
  Orchestrator/Memory Bank/Qdrant indexing intact) remains pinnable ("Install Specific
  Version" + disable auto-update). `.kilocodemodes` auto-converts to `.kilo/agent/*.md` on
  launch via CustomModesManager; `.kilocode/rules/` still read as fallback (migration guide
  steps 4–5).
- **PREMISE CORRECTION (flagged at iter 3, now verified)**: the REBUILT v7 extension does
  NOT read `.roomodes`/`.roo/rules` on startup — "The wizard doesn't read Roo-prefixed
  files directly. That's why step 3 renames them first" (migration guide; mv .roorules
  AGENTS.md, cp .roomodes .kilocodemodes, etc.). Only legacy v5.x read Roo paths (repo
  evidence: packages/kilo-vscode/CHANGELOG.md .roomodes entries at pinned SHA).
- **Roo-death interplay**: Rubens announcement 2026-04-21 → Kilo "Thank you, Roo!" +
  migration guide 2026-04-22 → became the editor-native recommended path; weekly hardening
  posts: week 2 = 188 Kilo PRs + 21 upstream OpenCode releases (v1.2.25→v1.4.3); week 3 =
  80+ PRs + 3 upstream; "Windows memory: fixed. Rate limiting: fixed." ([week two](https://blog.kilo.ai/p/new-vs-code-week-two),
  [week three](https://blog.kilo.ai/p/new-vs-code-week-three)).
- **EXIT — Anaconda acquired Kilo 2026-07-15** (16 months post-founding; 2 months after
  Roo archive; terms undisclosed). "More than 3 million developers", ~10T tokens/mo
  ([The New Stack](https://thenewstack.io/anaconda-kilo-open-source-acquisition/)); 40T
  lifetime tokens (Kilo LinkedIn, 2026-07-17). Anaconda takes over GitHub org stewardship;
  Kilo positioned as the "agentic engineering layer" of the Anaconda platform, following
  its Outerbounds/Metaflow acquisition ([press](https://www.anaconda.com/press/anaconda-acquires-kilo-code),
  [joint blog](https://blog.kilo.ai/p/anaconda-acquires-kilo-code), [AP](https://apnews.com/press-release/business-wire/press-release-311a501603de42c1808f7bc9bd8a898b)).
  kilo.ai homepage now claims "5M+ Kilo Coders" (UNVERIFIED — diverges from PR's 3M;
  likely installs-vs-active definition drift).

### 11.2 Architecture post-rebuild (primary: repo @ 3d04228 + AGENTS.md/CONTEXT.md)

- **Language VERIFIED: TypeScript on Bun. NOT Go.** Bun 1.3.14 + turbo monorepo; Effect-TS
  runtime (@effect/platform-node 4.0.0-beta.83; own packages effect-drizzle-sqlite /
  effect-sqlite-node → SQLite-backed sessions); tsgo typecheck. Upstream OpenCode is
  TS/Bun; fork inherits it.
- **Fork topology**: `packages/opencode` = the engine, package `@kilocode/cli` v7.6.2,
  "Fork of upstream OpenCode" (AGENTS.md); upstream pinned via `.opencode-version` =
  **v1.18.13** (anomalyco/opencode). Kilo changes to shared files MUST carry
  `kilocode_change` markers — CI-enforced (script/check-opencode-annotations.ts; exempt:
  src/kilocode/*); plus duplication checker + architecture checker gate divergence.
  Upstream sync cadence: 21 releases merged in one week (week-two post).
- **Server-core model**: ALL products are clients of `kilo serve` (HTTP + SSE,
  @kilocode/sdk) — AGENTS.md: "All products are clients of the CLI (packages/opencode/),
  which contains the AI agent runtime, HTTP server, and session management." VS Code ext
  bundles the CLI binary and spawns it as a child; ONE KiloConnectionService per extension
  host shared by sidebar/editor-tabs/Agent-Manager; worktree sessions pass directory
  context to the SAME backend (no per-worktree processes). Headless `kilo run` + `kilo
  serve` = multi-client attach inherited from OpenCode.
- **Surfaces**: CLI/TUI, VS Code (Solid.js webview rebuild; old extension extracted to
  kilocode/kilocode-legacy), JetBrains NATIVE plugin (Java, "does not require Node.js"),
  Cloud Agents (app.kilo.ai/cloud), web, mobile apps, Slack/Kilo Connect, KiloClaw
  assistant, Gas Town (below).
- **Kilo overlay**: packages/opencode/src/kilocode/ ≈46 Kilo-owned dirs (acp, agent,
  agent-manager, **anaconda-desktop**(!), board [Swarm], claw, cloud, daemon, memory,
  sandbox, snapshot, session-{export,import,portability,resume}, worktree, skills, pty,
  question, review, presence, suggestion, notebook, background-process, bash-hierarchy...).
  First-party packages: kilo-gateway, kilo-memory, kilo-sandbox (bubblewrap.ts; depends on
  **@anthropic-ai/sandbox-runtime 0.0.63** — Anthropic's sandbox runtime consumed as a
  library), kilo-indexing, kilo-console, kilo-telemetry, kilo-i18n, containers, codemode,
  plugin, http-recorder, sdk/sdk-next.
- **Agent Manager** (kilo-vscode src/agent-manager/ + webview; NOT a standalone product —
  [docs](https://kilo.ai/docs/automate/agent-manager)): full-panel editor tab; worktree-
  isolated sessions under `.kilo/worktrees/`, UI state `.kilo/agent-manager.json`; diff
  panel vs parent branch; full GitHub PR pipeline in-panel via `gh` (badge detection:
  tracking-ref → branch-name → HEAD-SHA; inline comments w/ suggestion-fence "Apply to
  worktree"; merge/squash/rebase/auto-merge; "Fix with Kilo" on conflicts/check-failures/
  review-threads); setup scripts + .env auto-copy; import from branch/PR-URL/external
  worktree; "Continue in Worktree"; **Multi-Version Mode** = up to 4 parallel
  implementations of the same prompt, optionally different models; sections; Permission
  Dock; edit-preview panel.
- **Orchestration model (2 layers + a board)**: (1) `task` subagent tool — foreground or
  background (`background: true`, result injected on completion), **resumable via
  `task_id`**, fresh context per invocation; built-in `general` (autonomous) + `explore`
  (research); **nesting gated by `subagent_depth` config, default 1 = flat**
  (packages/opencode/src/tool/task.ts#L135-163, w/ `kilocode_change` marker "honor
  upstream's opt-in depth limit"). Child permission derivation = parent's deny +
  external_directory rules ONLY; child denied `task`/`todowrite` unless its own ruleset
  grants them (packages/opencode/src/agent/subagent-permissions.ts). (2) `agent_manager`
  tool — the AGENT creates/supervises Agent Manager sessions from chat (worktree|local
  modes; 1–20 tasks; per-task model/variant override; capability-scoped permissions —
  worktree/local/prompt/stop/move approved SEPARATELY; companion `agent_manager_models`
  searches the catalog on demand, ≤20 results, "so the full catalog is never added to the
  conversation context"). (3) **Kilo Swarm** (`experimental.shared_agent_board`) =
  board_post/board_read message board scoped to one main session + its task descendants;
  epistemically honest receipts: "Stored only. This does not confirm delivery, reading,
  or action, and does not wake recipients"; recipient-state warnings; revision-checked
  reset.
- **Orchestrator deprecation (their reasoning, verbatim)**: "agents with full tool access
  (Code, Plan, Debug) can now delegate to subagents automatically. You no longer need a
  dedicated orchestrator" ([docs](https://kilo.ai/docs/code-with-ai/agents/orchestrator-mode)) +
  migration guide: "We found the explicit coordinator pattern added overhead without
  adding capability. One constraint we're still working on: subagents can't spawn
  subagents. Flat two-level hierarchy as of v7.2.10." (depth since made configurable via
  subagent_depth). Read-only agents (Ask) cannot delegate. → Roo's tool-less coordinator
  is gone; delegation is a PERMISSION, not an identity.
- **Modes retained — renamed agents**: built-ins code/ask/debug + `plan` (ex-Architect);
  custom agents = Markdown + YAML frontmatter at `.kilo/agents/{slug}.md` w/ `permission:`
  block (allow/ask/deny + globs — replaces Roo tool groups); agent cycle Cmd+.; per-agent
  system prompt = markdown body; wizard converts `.kilocodemodes` YAML.
- **Per-model routing**: Sticky Models (pin model per agent — validates todo 42); Auto
  Models ("automatically using the best model at a given price point"); 500+ models via
  Kilo Gateway w/ provider fallback; Agent-Manager tasks inherit the current turn's
  model+variant, per-task override; reasoning-variant selector (/variant).
- **Context management** (root CONTEXT.md — "OpenCode Session Runtime" vocabulary):
  **Context Epoch** = span where one rendered System Context is the immutable
  provider-cache baseline (ends at completed compaction / session movement / incompatible
  transition); **Context Sources** = typed, key-stable, infallible-loader values in a
  location-scoped registry; **Mid-Conversation System Messages** = durable chronological
  state updates (KV-safe in-history appends after cached prefix); Admitted Prompt →
  Prompt Promotion at a **Safe Provider-Turn Boundary**; bounded **Model Tool Output** w/
  spill to **Managed Tool Output File** (shared tool-output dir); Context Snapshot =
  model-hidden diff state. Checkpoints: **snapshots OUTSIDE the repo** at
  `~/.local/share/kilo/snapshot/<project-id>/<worktree-hash>/` in a detached worktree,
  taken per USER MESSAGE (turn-granularity "to better support large repositories") —
  Roo's nested-.git bug class structurally gone. Local Recall = search past sessions
  across worktrees.

### 11.3 Notable concepts unique vs Cline/Roo/Claude Code/goose

1. **Fork-of-a-rival as second founding**: only harness that rebuilt its whole product on
   a COMPETITOR'S core (OpenCode) while keeping fork-lineage UX — plus CI-enforced
   upstream-merge hygiene (kilocode_change annotations, pinned .opencode-version, weekly
   sync posts). A maintained vendored fork of a rival engine at 27k★ scale is unprecedented.
2. **Rebuild-enabled multi-surface sameness**: same engine + same kilo.jsonc + same
   sessions across VS Code/CLI/Cloud/mobile/Slack; Node-less JetBrains; headless serve
   multi-client attach.
3. **Agent Manager as GUI-native multi-agent workspace** (worktree-per-chat + in-panel PR
   review/merge + multi-version best-of-N + agent-spawnable sessions) — no comparable
   editor panel in Cline/Roo/CC/goose.
4. **Kilo Swarm** board w/ "stored-only" receipts — the most epistemically careful
   inter-agent messaging shipped anywhere (explicitly cannot wake/assign agents).
5. **Gas Town by Kilo** — productized gastownhall/gastown open protocol as a cloud
   service: towns/rigs/beads/convoy-DAG/reconciler + Mayor (coordination agent) + Polecats
   (coders) + Refinery (review/merge gate) ([docs](https://kilo.ai/docs/code-with-ai/gastown)).
   Autonomous repo-team orchestration as a managed product is unique in the field.
6. **Market-maker positioning**: Kilo Gateway (zero-markup, 60+ providers), Kilo Pass
   ($19/$49/$199), model Leaderboard + "Token Race" + KiloBench — Kilo sells routing +
   benchmarks, not just a harness. Roo tried at-cost Router and died of it; Kilo inverted
   the margin structure.
7. **Auto Models** (price-tier auto-selection) + on-demand model-catalog tools (context
   discipline: never dump the 500-model catalog into context).
8. **Sandbox**: kilo-sandbox builds on @anthropic-ai/sandbox-runtime + bubblewrap;
   /sandbox toggle per Agent-Manager session (policy + OS sandbox — addresses Roo's
   policy-only CVE farm).
9. **KiloClaw** — proactive personal assistant beyond the IDE (scheduled actions, external
   platforms) on the same engine; plus App Builder, Code Reviewer, Security Agent products.
10. **anaconda-desktop integration dir already in-tree** — post-acquisition integration
    landing in the open repo (stewardship-in-public).

### 11.4 Kilo vs Roo — BETTER / WORSE

**BETTER**: (1) Business model: zero-markup gateway + subscription vs Roo's at-cost
Router coupling (Roo died of it; Kilo exited at 3M users). (2) Single engine → multi-
surface velocity (188-PR weeks) vs Roo ~daily releases to a strained VS Code-only base.
(3) Parallel execution: parallel tool calls + background subagents + worktree isolation;
Roo's boomerang paused the parent, one child at a time. (4) Delegation from ANY full-tool
agent vs coordinator-only. (5) Checkpoints outside repo, turn-granularity, no nested-.git
bug class (Roo: in-repo shadow git, 40GB+ disk complaints). (6) Native JetBrains (Roo:
VS Code-only). (7) Honest migration: public parity plan doc, deprecation table with
reasons, legacy 5.x pinnable, $100/merged-bugfix bounty. (8) Upstream leverage: OpenCode
community work "lands in Kilo automatically". (9) GUI for multi-agent (Agent Manager) —
Roo never shipped a multi-session surface. (10) OS sandboxing (kilo-sandbox) vs Roo's
policy-only safety (11 CVEs).

**WORSE**: (1) Launch quality: GA forced by auto-update onto the whole base → "The new
Kilocode v7 is awful" ([r/kilocode](https://www.reddit.com/r/kilocode/comments/1sampao/the_new_kilocode_v7_is_awful/);
official tracking issue [#8415](https://github.com/Kilo-Org/kilocode/issues/8415)):
Windows memory spikes, rate limits, Plan/Ask strictness, batch-edits "hundreds of lines
without giving me a chance to review", prompt-comprehension regressions ("the AI became
dumber"), downgrade-to-5.x stampede; "kilocode v7 is just opencode now" / "if I wanted
opencode, I would just use opencode". Roo's 2-year codebase was mature by comparison.
(2) Parity debt at rebuild: Qdrant codebase indexing NOT ported (PR #6966 in review),
Memory Bank manual-concat only, workflows UI stub, no provider config in beta UI.
(3) Orchestrator removal dropped Roo's DOCUMENTED context-poisoning doctrine: full-tool
parents delegating re-imports the coordinator context-pollution risk Roo designed against
— Kilo docs don't address it. (4) Memory Bank → AGENTS.md trades capability for
standardization: AGENTS.md is static instructions; Roo's structured cross-session memory
worked out-of-the-box (kilo-memory package depth UNVERIFIED). (5) Differentiation moved
from architecture to UX/gateway — identity cost visible in community threads. (6)
Review-first default relaxed vs Roo's per-step diff approval. (7) Surface sprawl (mobile,
Slack, KiloClaw, Gas Town, App Builder) = focus tax; now plus Anaconda enterprise pull.

### 11.5 Business trajectory + sustainability post-Roo

- Verified points: seed 2025-12-10 $8M at 750k downloads / #1-OpenRouter / 6.1T tok/mo
  (BusinessWire) → 1.5M+ coders, 25T+ lifetime tokens (Hackernoon company profile, ~mid-2026)
  → acquisition 2026-07-15 at 3M devs, ~10T tok/mo, 40T lifetime (New Stack/LinkedIn).
- OpenRouter today ([apps/kilo-code](https://openrouter.ai/apps/kilo-code)): 7.35T tokens
  lifetime via OpenRouter, daily global rank #2, **#1 IDE Extensions, #2 Coding Agents,
  #2 CLI Agents**, 353 models, active since Apr 2025. So "#1 on OpenRouter" (Dec 2025) has
  decayed to #2 among coding agents (OpenClaw leads — [dayahimour](https://dayahimour.org/en/blog/kilo-code/),
  secondary, UNVERIFIED). Marketing "#1" persists on profiles; treat as stale.
- Sustainability read: Roo's death removed the closest free fork-rival and handed Kilo the
  migration mantle; within 3 months Kilo converted it into an exit (Anaconda; after
  Outerbounds/Metaflow). The $8M seed + GitLab ROFR ended in a strategic sale, not a
  growth round. OSS continuation promised ("fundamental to our strategy" — DeSanto); repo
  stayed MIT; integration landing in-tree. Risk flags for our ledger: single-corporate
  steward (unquiet-fork pattern), homepage metric drift (5M vs 3M), OpenRouter rank decay
  to #2, post-rebuild community trust dip, surface sprawl.

### 11.6 ADOPT / AVOID / WATCH deltas (queued for next plan touch)

ADOPT: capability-scoped managed-session tool (agent_manager pattern: worktree/local/
prompt/stop/move separately approvable); best-of-N multi-version runs w/ per-version
model; subagent_depth opt-in nesting + deny-only permission inheritance + child
task/todowrite capability gating; task_id-resumable subagents (continuable children —
matches dsh); "stored-only" receipt semantics for any inter-agent board; Context-Epoch +
mid-conversation system messages (KV-safe in-history appends — strongly validates todos
10/11); snapshots OUTSIDE repo per user-turn (todo 23 alternative tradeoff to Roo's
task-scoped in-repo); kilocode_change-style fork-annotation + duplication gates (if we
ever vendor upstream harness code); on-demand model-catalog search tool (never dump the
catalog into context).
AVOID: rebuild GA via silent auto-update to the full base (cohort rollout — reinforces
Cline dual-bundle lesson); relaxing review-tight defaults during a migration; shipping a
migration path before parity equivalents (indexing/memory) exist — the public parity
TABLE itself was the right move, keep that; press-metric drift (5M vs 3M).
WATCH: Anaconda-stewardship effect on OSS pace + governance; gastownhall/gastown beads
protocol as a multi-agent orchestration standard; Kilo Swarm ↔ dsh Agent Teams ↔ Claude
teams messaging convergence; OpenKilo reverse-plugin (Kilo Gateway inside stock OpenCode,
[discussion #5868](https://github.com/Kilo-Org/kilocode/discussions/5868)); whether
@anthropic-ai/sandbox-runtime becomes the common sandbox base across harnesses.

### 11.7 Sources

Primary (pinned 3d04228b6a642acb3daf68a649269618b6018250): AGENTS.md · CONTEXT.md ·
package.json · .opencode-version · packages/opencode/package.json (@kilocode/cli 7.6.2) ·
packages/opencode/src/tool/task.ts (L135-163) · packages/opencode/src/tool/task.txt ·
packages/opencode/src/agent/subagent-permissions.ts · packages/opencode/src/kilocode/ tree ·
packages/kilo-sandbox/{package.json,src/bubblewrap.ts} ·
packages/kilo-vscode/docs/opencode-migration-plan.md · packages/kilo-vscode/CHANGELOG.md.
Blog: [rebuild beta 2026-03-10](https://blog.kilo.ai/p/we-completely-rebuilt-the-kilo-vs-code-extension) ·
[GA 2026-04-02](https://blog.kilo.ai/p/new-kilo-for-vs-code-is-live) ·
[week one](https://blog.kilo.ai/p/new-vs-code-extension-week-one-what) ·
[week two](https://blog.kilo.ai/p/new-vs-code-week-two) ·
[week three](https://blog.kilo.ai/p/new-vs-code-week-three) ·
[Thank you Roo](https://blog.kilo.ai/p/thank-you-roo) ·
[PH return](https://blog.kilo.ai/p/were-back-on-product-hunt-new-vs-code) ·
[3M downloads lessons](https://blog.kilo.ai/p/what-we-learned-from-3-million-downloads).
Docs: [migration guide](https://kilo.ai/articles/roo-to-kilo-migration-guide) ·
[agent-manager](https://kilo.ai/docs/automate/agent-manager) ·
[orchestrator deprecated](https://kilo.ai/docs/code-with-ai/agents/orchestrator-mode) ·
[gastown](https://kilo.ai/docs/code-with-ai/gastown) ·
[whats-new](https://kilo.ai/docs/code-with-ai/platforms/vscode/whats-new).
Business: [BusinessWire](https://www.businesswire.com/news/home/20251210195261/en/Kilo-Raises-%248-Million-to-Bring-Kilo-Speed-to-Agentic-Engineering) ·
[CNBC](https://www.cnbc.com/2025/12/10/former-gitlab-ceo-raises-8-million-for-kilo-to-compete-in-vibe-coding.html) ·
[VentureBeat](https://venturebeat.com/orchestration/kilo-cli-1-0-brings-open-source-vibe-coding-to-your-terminal-with-support) ·
[Anaconda press](https://www.anaconda.com/press/anaconda-acquires-kilo-code) ·
[Anaconda blog](https://www.anaconda.com/blog/anaconda-acquires-kilo-code) ·
[AP](https://apnews.com/press-release/business-wire/press-release-311a501603de42c1808f7bc9bd8a898b) ·
[The New Stack](https://thenewstack.io/anaconda-kilo-open-source-acquisition/) ·
[OpenRouter apps](https://openrouter.ai/apps/kilo-code) ·
[Hackernoon profile](https://hackernoon.com/company/kilo).
Community: [r/kilocode v7](https://www.reddit.com/r/kilocode/comments/1sampao/the_new_kilocode_v7_is_awful/) ·
[issue #8415](https://github.com/Kilo-Org/kilocode/issues/8415) ·
[discussion #5868](https://github.com/Kilo-Org/kilocode/discussions/5868) ·
[explainx guide](https://www.explainx.ai/blog/kilo-code-ai-coding-agent-guide-2026) (secondary).


## 12. Iteration 5 findings — Mentat (hunk-staging origin; SHA 3a01f0a, archived, 2026-09-11)

Research date 2026-09-11. Repo `AbanteAI/mentat` now 404s — renamed to
`AbanteAI/archive-old-cli-mentat` + GitHub-archived (exact rename/archive date NOT
exposed by API; ecosyste.ms snapshot already shows archived by 2025-06-20 —
UNVERIFIED precise date). Pinned at final main SHA
`3a01f0a59404d717e527e4b60b6244cca158cfed` (v1.0.19, the deprecation release;
full 560-commit history cloned in temp). Per-claim URLs; UNVERIFIED flagged.

### 12.1 Identity + lifecycle (VERIFIED unless flagged)

- **What**: Mentat — Python CLI AI coding assistant, Apache-2.0, launched
  2023-07-25 from `biobootloader/mentat` (initial commit same day; Dune
  branding). Day-1 README pitch: "coordinates edits across multiple locations
  and files… already has the context of your project". Repo created
  2023-07-25, last push 2025-01-07, now archived, 2,544★/233 forks, 30
  contributors (GitHub API + ecosyste.ms).
- **Team**: Scott Swingle (@biobootloader, "prev @DeepMind" per his X bio) +
  Jake Koenig (116 commits), Paul Swingle (170 combined), Grant Hawkins
  (granawkins, 57), Wayde Gilliam (60) — "Next Level Dev Tools Team" per
  @AbanteAI X profile. Launch tweet 2023-07-25, 6.1K likes (virality source);
  HN launch threads modest (12 pts / 1 pt).
- **Timeline**: 560 commits 2023-07-25 → 2025-01-07. Feature dev STOPS
  2024-04-30 (last real commit: "pin ragdaemon to the last 0.2.x version").
  **8.5-month silence**, then 2025-01-07 triple farewell: README replaced with
  "ARCHIVED PROJECT" tombstone (#591), deprecation message on CLI run (#593),
  version bump 1.0.19. CHANGELOG 1.0.19 verbatim: "Added deprecation notice -
  this CLI tool is now archived and replaced by the new Mentat GitHub bot
  service".
- **Brand recycling**: "The name 'Mentat' is now used by a different project -
  an AI-powered GitHub bot that writes and reviews code" (archive README).
  **MentatBot**: closed-source GitHub bot; claimed SWE-bench Lite SOTA 38%
  (GPT-4o, 2024-06-27) — verified on official swebench.com results viewer
  ("GPT-4o · AbanteAI MentatBot · 2024-06-27") + r/singularity (56 votes) +
  OpenHands issue #2821 proposing adoption. Blog post still resolves
  (mentat.ai/blog/mentatbot-sota-coding-agent). Third-party directory
  (aiagentstore.ai): mentat.ai service ALSO discontinued, "stopped accepting
  new usage" — mentat.ai now a JS-only shell (webfetch/wayback confirm empty
  shell) — UNVERIFIED via no primary shutdown statement.
- **Org state 2026**: AbanteAI effectively defunct as OSS entity — remaining
  repos are spinouts/fossils: spice (LLM client, 4★), ragdaemon (graph/3D
  call-graph, 11★), LoCoDiff-bench (long-context diff benchmark, 33★),
  apples-to-models, gpt-generals (RL env). @AbanteAI X: 590 followers, "hasn't
  posted". aider counter-factual TODAY: 48,892★, not archived, pushed
  2026-05-22 (GitHub API).

### 12.2 Architecture (primary = pinned source @ 3a01f0a)

- **Execution model**: `Session` class docstring: "The server for Mentat"
  ([session.py#L38](https://github.com/AbanteAI/archive-old-cli-mentat/blob/3a01f0a59404d717e527e4b60b6244cca158cfed/mentat/session.py#L38))
  — asyncio server + `SessionStream` channel-based message bus
  (`context_update`, `session_exit`, `client_exit` channels);
  `apply_edits: bool` flag "for clients that apply the edits themselves (like
  vscode)". This was a deliberate 3-month-in refactor: issue #63 (Aug 2023,
  biobootloader, closed by waydegilliam Oct 2023): "To prepare for future
  editor extensions and other interfaces, we need to separate the core logic
  from the terminal interface (both input and output)" — PRs #74/#119. TUI =
  textual (v1.0.10 "full terminal app which displays the context and running
  cost in the sidebar"); VSCode ext "finalized" 2024-04-02 (commit 37aa9a7);
  Python SDK (`from mentat import Mentat`). No daemon multi-attach (one
  client per session).
- **THE hunk-staging UX (the reason we study it)** — present FROM INITIAL
  COMMIT (2023-07-25, `change_conflict_resolution.py` + `app.py`): after the
  model responds, "Apply these changes? 'Y/n/i' or provide feedback."
  ([code_edit_feedback.py#L16](https://github.com/AbanteAI/archive-old-cli-mentat/blob/3a01f0a59404d717e527e4b60b6244cca158cfed/mentat/code_edit_feedback.py#L16))
  — Y=all, n=none, **i=interactive** → per-file: create?/delete?/rename? then
  **per-replacement "Keep this change?"** with colored diff
  ([file_edit.py#L176-L204](https://github.com/AbanteAI/archive-old-cli-mentat/blob/3a01f0a59404d717e527e4b60b6244cca158cfed/mentat/parsers/file_edit.py#L176-L204)),
  and any OTHER response is treated as natural-language FEEDBACK re-sent to
  the model with a system note ("User chose not to apply any of your
  changes. User response: …") — confirm-or-coach loop. Unique extra: day-1
  **insertion-conflict UI** — overlapping model insertions printed as (a)(b)(c)
  alternatives, user types the ORDER in which to insert them
  (`Type the order in which to insert changes (omit for no preference)`,
  initial commit `change_conflict_resolution.py`). NOTE: the ledger §3 says
  "y/n/e" — the FINAL code is Y/n/i with no edit-in-editor option; "e"
  phrasing not found in code — ledger phrasing correction queued.
- **Write-authority = read-scope**: `FileEdit.is_valid()` cancels ALL edits
  to a file unless every edited line lies inside a context interval that was
  included: "File … not in context, canceling all edits to file."
  ([file_edit.py#L159](https://github.com/AbanteAI/archive-old-cli-mentat/blob/3a01f0a59404d717e527e4b60b6244cca158cfed/mentat/parsers/file_edit.py#L159))
  — hard coupling of write authority to read scope, before any apply.
- **Edit formats — a parser ZOO**: day-1 format was `@@start`/`@@code`/`@@end`
  JSON blocks with line-anchored insert/replace/delete/create/delete-file/
  rename-file actions
  ([block_parser.py#L30](https://github.com/AbanteAI/archive-old-cli-mentat/blob/3a01f0a59404d717e527e4b60b6244cca158cfed/mentat/parsers/block_parser.py#L30)
  — same format in day-1 prompts.py, i.e. final form = original form). Along
  the way they shipped: replacement parser + split-diff parser (2023-09-28/29
  #105/#112), unified diff format (2023-10-06 #118), **reversible parsing**
  (2023-10-17 #150 — parsers can invert edits for undo), json parser
  (2023-11-27 #253), "Changed format to fit Anthropic models" (v1.0.11) —
  ≥4 user-selectable parsers coexisting (four prompt .txt files in the Dec
  2023 CodeMap-removal diff). Experimental **revisor** (v1.0.9, `--revisor`):
  LLM pass that "attempts to fix edits that fail to conform to parser
  format" (mentat/revisor/).
- **Git integration**: git repo REQUIRED (get_git_root_for_path); GitPython +
  `git ls-files -c -o --exclude-standard` for file listing (untracked via
  `--directory` for perf); **edits applied by line-splicing, NOT git apply**
  (`Replacement` = 0-indexed line-range + new_lines; `resolve_conflicts()`
  clamps overlaps; `get_updated_file_lines` splices back-to-front). Undo =
  `previous_file_lines` snapshots written back (`FileEdit.undo()`,
  `undo/undoall/amend` commands) — in-memory, not git. "Aider makes a
  git-commit for every change… Mentat leaves the version management to you"
  (dev.to deep-dive) — the key UX divergence from aider.
- **Context management**: context = explicitly included files/dirs/globs w/
  line intervals + **git-diff-anchored defaults** (DiffContext: files changed
  vs a treeish + untracked — "aware of the current git diff so it can extend
  your work", launch tweet) + optional auto-context. Auto-context evolved:
  ctags CodeMap (aider-inspired, header: "Our code map implementation was
  inspired by Aider's repo map") → REMOVED 2023-12-03 (#335) → feature
  filters + "auto context now only grows so the model won't forget earlier
  read files" (v1.0.8) → **ragdaemon** graph (2024-04-11 #540): persistent
  JSON graph DB (`.mentat/ragdaemon/`), annotators = hierarchy (file tree) +
  chunker_line (50-line chunks) + diff (marks chunks in current diff) +
  embeddings via spice
  ([code_context.py](https://github.com/AbanteAI/archive-old-cli-mentat/blob/3a01f0a59404d717e527e4b60b6244cca158cfed/mentat/code_context.py)).
  ContextStreamMessage streams live token-count + total-cost + feature list
  to the client (context HUD). NO compaction/eviction; roadmap item
  "seamlessly work with codebases much larger than LLM context limits" never
  shipped. Vision: GPT-4V screenshots (2023-11-27, VisionManager). `run`
  command feeds shell output back (plain capture, no PTY scraping). Agent
  mode (opt-in `/agent`): AgentHandler LLM-selects files + test commands in
  a loop
  ([agent_handler.py#L23](https://github.com/AbanteAI/archive-old-cli-mentat/blob/3a01f0a59404d717e527e4b60b6244cca158cfed/mentat/agent_handler.py#L23))
  — proto-subagent in the SAME context.
- **Stack**: `spice` (own LLM-client lib: caching/retries/cost tracking;
  OpenAI→multi-provider incl. Anthropic-without-proxy v1.0.13, Azure/Ollama
  via litellm docs); Sentry crash reporting wired in-session (default-on
  crash telemetry).

### 12.3 Git archaeology — reversals, deprecations, why it lost

1. **Three context architectures in 5 months**: ctags CodeMap (Nov 2023) →
   removed Dec 3 2023 (#335) → grows-only auto-context + feature filters →
   ragdaemon embeddings graph (Apr 2024). aider meanwhile kept ONE map and
   iterated it (tree-sitter+PageRank). Thrash vs compounding.
2. **Parser-format churn**: ≥4 formats shipped/existed simultaneously
   (block-JSON, replacement, unified-diff, json) + format change for
   Anthropic (v1.0.11) — user-selectable `--parser` instead of one
   benchmarked format; aider benchmarked edit formats per model instead.
3. **UI stack swap**: colored/prompt_toolkit/pygments removed (#508, Jan
   2024) → textual TUI rewrite (v1.0.10, Feb 2024).
4. **Spinout-dependency flip-flop**: v1.0.16 (Apr 3) "Always use the latest
   patch-level versions of Spice and Ragdaemon" → 2024-04-30 "pin ragdaemon
   to the last 0.2.x version" (ragdaemon 0.3 breakage) — the LAST dev commit
   ever. Depending on your own unmaintained libs pinned "always latest" =
   foot-gun.
5. **Only 2 git-level reverts in 560 commits** ("dont run on windows"
   reverted Nov 2023; "Strip leading spaces" reverted Jan 2024) — the
   history is clean because the team PIVOTED rather than iterated.
6. **The pivot cascade (why it died)**: feature dev stops 2024-04-30 right
   after "Finalize vscode extension" (2024-04-02) → team redirects to
   CLOSED MentatBot (SWE-bench SOTA marketing Jun 2024) → CLI starves
   through the exact window the IDE-assistant wave (Cline Jul 2024, Roo Oct
   2024) explodes → CLI formally deprecated 2025-01-07 → MentatBot (no
   moat vs free OSS + Copilot) also dies; brand recycled twice; OSS repo
   renamed to a tombstone. Direct-loss evidence vs aider: Anton Osika
   (GPT-engineer) Sept 2023 scored Mentat 2/5 vs aider 4/5 (`--exclude`
   broken, "No such file or directory", "activity is low… obvious things
   don't seem to be fixed; I don't have high hopes") — one month after
   launch; HN (Jan 2024): both tools "easily make a mess of my code base
   (especially larger projects)"; dev.to (Jul 2024, after the abandonment):
   "meager list of LLMs compared to Aider", "It would crash from time to
   time… Graceful failure isn't its strength". Structural causes: 4-person
   team split across CLI+spinouts+closed bot; no eval-driven iteration
   culture ON THE HARNESS (evals went to benchmarks-as-products instead);
   in-memory undo vs aider's per-change git commits (reviewability);
   context-window ceilings (8k-era) with eviction never shipped.
7. **Shutdown status**: CLI = dead + tombstoned. mentat.ai bot = discontinued
   (UNVERIFIED primary; shell domain + third-party listing). Org = dormant.
   No security advisories, issues disabled-by-archive (0 open).

### 12.4 Community / performance reputation

- **Peak reputation**: launch virality (6.1K-like tweet; ~3K stars by Dec
  2023); praised UX ("The UX had me at hello" — dev.to/ykgoon; cost print
  after each change singled out by Osika); MentatBot's 38% SWE-bench Lite
  (Jun 2024) was a REAL leaderboard-topping result (official viewer) that
  got OpenHands/Reddit attention — the highest point of AbanteAI's perf
  record, achieved by the CLOSED successor, not the OSS CLI.
- **Lasting reputation**: reliability complaints (crashes, poor graceful
  failure), small-model support, larger-project "mess" — and it became the
  canonical cautionary tale of "promising 2023 OSS coding tool abandoned by
  its startup" (README literally a tombstone pointing at the brand's next
  incarnation). LoCoDiff-bench (long-context diffs) + apples-to-models
  survive as the org's most-cited artifacts, not mentat itself.

### 12.5 ADOPT / AVOID / WATCH deltas for OUR plan (ledger §3 checked)

Ledger already has: semantic-hunk staging (Mentat/blueprint — mechanism now
pinned to code above; phrasing Y/n/i not y/n/e). NEW items:

**ADOPT (queued):**
1. **Write-authority scoped to read-context**: hard-reject any edit whose
   target lines were never in a model-visible read window ("not in context →
   cancel edit"), enforced pre-apply — kills hallucinated-anchor writes;
   composes with our windowed reads + hashline anchors.
2. **Confirm-or-coach completion**: treat a non-Y/N answer at the approval
   prompt as natural-language FEEDBACK re-entering the loop with an explicit
   system note of what the user declined — approval UX and correction loop
   in one gesture.
3. **WIP-diff-anchored default context**: seed context from `git diff
   <base>` + untracked files each turn (cheap, high-precision, workflow-
   anchored; distinct from our repomap which stays structural).
4. **Revisor-style repair pass for malformed structured edits**: dedicated
   re-parse-with-repair-prompt retry for edit-format violations (validate/
   strengthen todo on edit reliability; distinct from generic retry).
5. **Insertion-conflict ordering UI**: when overlapping model edits survive
   resolution, present alternatives and let the user order them rather than
   silently clamping (our resolve logic keeps the clamp as fallback).
6. **Live context+cost HUD as first-class stream message** (ContextStreamMessage
   pattern: token count, cost, feature list pushed to EVERY frontend) —
   validates todo HUD; add "feature list" to our HUD payload.
7. **LoCoDiff-bench** into our eval adapter as the long-context-diff axis
   (it is the public benchmark for exactly our windowed-read/repomap claim).

**AVOID:**
1. Context-architecture thrash (3 in 5 months) — pick the repomap,
   benchmark it, compound (ours: CSR PageRank, validated by this failure).
2. User-selectable edit-FORMAT parser zoo — pin one format; per-model
   adaptation belongs in model profiles (aider concurrence).
3. Own-spinout deps pinned "always latest" (version-pin discipline in our
   release process).
4. Pivot-to-closed while starving the open core + recycling the brand name —
   community trust death spiral (README-as-tombstone is the end state).
5. Sentry-style default-on crash telemetry (already in §3 AVOID — re-confirmed
   in a 2023-era tool).

**WATCH:** nothing live — org dormant; ragdaemon/LoCoDiff unmaintained;
MentatBot closed and dead. Mentat exits the comparison set as a
pure-history iteration (like SWE-agent's origin value, but terminal).

### 12.6 Sources

Primary (pinned 3a01f0a59404d717e527e4b60b6244cca158cfed):
mentat/{session,code_edit_feedback,code_context,agent_handler,git_handler}.py ·
mentat/parsers/{file_edit,block_parser}.py · README.md · CHANGELOG.rst ·
initial-commit eea14f6: app.py/change_conflict_resolution.py/prompts.py ·
removal commit ef51d46 (CodeMap) · CHANGELOG entries v1.0.7–v1.0.19.
GitHub API: repo meta (archived, dates, stars, license), tags v1.0.x, releases.
Issue #63 (core/interface separation). X: @biobootloader launch tweet
(2023-07-25, 6.1K likes), @AbanteAI profile.
Secondary: [swebench.com viewer](https://www.swebench.com/viewer.html)
(MentatBot 2024-06-27 entry) ·
[r/singularity MentatBot 38%](https://www.reddit.com/r/singularity/comments/1dq5r33/) ·
[OpenHands #2821](https://github.com/OpenHands/OpenHands/issues/2821) ·
[Anton Osika tool notes 2023-09-07](https://antonosika.substack.com/p/notes-from-trying-ai-coding-tools) ·
[dev.to deep dive 2024-07-24](https://dev.to/ykgoon/deep-dive-into-mentat-coding-assistant-12no) ·
[dev.to mentat+aider 2024-06-18](https://dev.to/ykgoon/the-future-of-programming-classical-vs-assisted-coding-with-mentat-and-aider-2d9f) ·
[HN 38843829](https://news.ycombinator.com/item?id=38843829) ·
[aiagentstore Mentat listing](https://aiagentstore.ai/ai-agent/mentat) (service
discontinued — UNVERIFIED primary) · [ecosyste.ms](https://awesome.ecosyste.ms/projects/github.com%2FAbanteAI%2Fmentat)
(archive-state snapshot).

## 13. Iteration 6 findings — Plandex death autopsy (combined librarian; HEAD e2d7720, company dead 2025-10-03, 2026-09-11)

Research date 2026-09-11. Repo `plandex-ai/plandex` NOT archived but **effectively dead**: last
release 2025-07-16 (v2.2.1), last push / HEAD = 2025-10-03, and the final commit is literally
"link to cloud wind down post" (pinned SHA `e2d772072efadbe41d2946d97d79be55532dbab5`; full
1,483-commit history cloned in temp). Per-claim URLs; UNVERIFIED flagged.

### 13.1 Identity + lifecycle (VERIFIED unless flagged)

- **What**: Plandex — Go terminal AI coding agent, MIT, "designed for large projects and real
  world tasks"; plan-as-first-class-object + git-branch sandbox architecture.
  https://github.com/plandex-ai/plandex
- **Numbers**: 15,631 stars / 1,176 forks / not archived / MIT / Go / created 2023-10-24 /
  pushed 2025-10-03 / 63 open issues (GitHub API 2026-09-11). Initial commit 2023-09-28
  ("initial commit--kinda works"); commit #3 ever = "conversation history now uses summary"
  (summarization was day-1 DNA).
- **COMPANY DEAD**: Plandex AI Inc wound down 2025-10-03; Plandex Cloud shut 2025-11-07.
  Founder verbatim: "I've recently accepted an engineering position at Promptfoo, and I've made
  the tough decision to begin winding down Plandex as a company and cloud service"
  (plandex.ai/blog/winding-down — site now DEAD, webfetch failed 2026-09-11; text via Exa cache
  + [dev.to confirmation](https://dev.to/jovan_chan_9500711396d4e6/plandex-v2-review-2026-cloud-shut-down-self-hosted-survives-is-the-terminal-agent-still-worth-11pp)).
  Self-reported bright spot: post-v2 revenue "10x-ing and then tripling" for two months —
  UNVERIFIED self-report.
- **Death cause (verbatim)**: "Claude Code had executed masterfully on the original vision of
  Plandex, and that it was pointless trying to play catch up as a solo founder without
  employees"; churn rose as "Claude Code... began to heavily subsidize tokens with their Pro
  and Max plans."
- **Founder trajectory**: Dane Schneider (@Danenania) = 1,301/1,483 commits (next contributor:
  18 — extreme bus factor) -> Promptfoo (Oct 2025) -> **Promptfoo acquired by OpenAI
  2026-03-09** ([openai.com](https://openai.com/index/openai-to-acquire-promptfoo/) +
  [TechCrunch](https://techcrunch.com/2026/03/09/openai-acquires-promptfoo-to-secure-its-ai-agents/) +
  [Bloomberg](https://www.bloomberg.com/news/articles/2026-03-09/openai-buying-ai-security-startup-promptfoo-to-safeguard-ai-agents/)).
  Plandex's creator is now de facto inside OpenAI.
- **2026 state**: unmaintained zombie-OSS — zero commits in ~11 months; "maintenance mode"
  (dev.to 2026-06-01); 30d commit/contributor activity = 0 + npm downloads -86.2% (Swanum
  vendor audit 2026-04-19, secondary); plandex.ai erroring since <=May 2026 (dev.to) and
  fetch-confirmed dead today; GitHub install path + self-host docs remain functional per
  reviews (install.sh status today UNVERIFIED).
- **Timeline**: initial 2023-09-28 -> HN v1 launch 2024-04-03 (304 pts,
  [HN #39918500](https://news.ycombinator.com/item?id=39918500)) -> v1.0.0 2024-05-16 ->
  v1 freeze mid-2024 ("release hiatus" per HN commenters) -> v2 = "8 months of heads down
  work, in effect a whole new project/product" ([HN #43710576](https://news.ycombinator.com/item?id=43710576),
  2025-04-16; v2.0.0 tags 2025-03-18) -> v2.2.1 2025-07-16 -> wind-down 2025-10-03. v1->v2 was
  NOT an in-place upgrade ("run separately and independently from v1"; v1 binary renamed
  plandex1/pdx1, config dirs .plandex -> .plandex-v2 — commit c90a287).

### 13.2 Architecture (primary = pinned source + docs @ e2d7720)

- **Client/server split (v2)**: Go CLI (`app/cli` — charmbracelet lipgloss/bubbles + org-forked
  go-prompt) <-> Go server (`app/server`, HTTP, lib/pq -> Postgres) holding ALL plan state;
  local single-user mode = docker-compose (postgres + plandex-server containers,
  [app/docker-compose.yml](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/app/docker-compose.yml))
  — Docker+Postgres MANDATORY. v1 was a single static binary; v2 traded that away.
- **Plan = version-controlled object**: every state change creates a version — context changes,
  prompts, responses, builds, rejections, applies, MODEL-SETTINGS changes
  ([version-control.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/core-concepts/version-control.md)).
  Storage = server-side git repos per plan + Postgres metadata. `rewind` to step-count/hash;
  `checkout` plan branches; NOTE: version log contains EVENTS not message content ("message
  content isn't included"; `convo` reads separately) — inversion of dsh's model-visible-=-logged.
- **THE sandbox = git-branch quarantine, NOT OS sandbox**: pending changes accumulate on the
  plan sandbox (server-side git), touch project files only on `plandex apply`; cumulative
  per-file diffs in a PR-review-style TUI with per-file reject; apply itself is a rewindable
  version; **v2 `auto-revert-on-rewind` default-true** reverts project files on rewind (v1 did
  not — a default REVERSAL) with conflict prompts. ZERO OS-confinement code in repo (no
  bubblewrap/seatbelt/landlock/firejail — grep-verified).
- **Exec-as-diff (`_apply.sh`)**: model writes commands to a special sandbox path; commands
  accumulate as pending changes LIKE FILES; run at apply-time or `auto-exec`;
  `plandex debug 'cmd'` = run -> output to LLM -> fix -> tentative-apply -> retry (default 5);
  "Changes are tentatively applied before running commands, then rolled back if the command
  fails" ([execution-and-debugging.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/core-concepts/execution-and-debugging.md)).
  No PTY/screen-delta scraping (plain capture); `plandex browser` = Chrome console-log monitor.
- **Context management** ([context-management.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/core-concepts/context-management.md)):
  (1) tree-sitter project maps, 30+ languages, indexes 20M+-token repos; (2) auto-load =
  architect role reads map then selects context; (3) smart-context = per-plan-step file
  selection ("sliding context window that grows and shrinks"); (4) auto-update-context pulls
  external edits; (5) WHOLE-file loads only (up to 100k tok/file — no windowed reads); (6)
  EAGER background summarizer on EVERY model reply (summarizer role, `max-convo-tokens`;
  summary doubles as working memory);
  (7) "Context caching across the board" OpenAI/Anthropic/Google (README claim); "2M effective
  context" via Gemini largeContextFallback.
- **Multi-model role system (the standout)**
  ([roles.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/models/roles.md),
  [built-in-packs.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/models/built-in/built-in-packs.md)):
  9 roles — planner, architect, coder, summarizer, auto-continue, builder, wholeFileBuilder,
  names, commitMessages — each with NESTED `largeContextFallback` CHAINS (daily-driver:
  sonnet-4 -> gemini-2.5-pro -> gemini-pro-1.5), `strongModel` builder upgrade, curated packs
  (daily-driver/reasoning/strong/cheap/oss), custom models/providers, OpenRouter/Ollama.
  v2.0.0 launch pack = Claude 3.7 Sonnet + o3-mini + Gemini 1.5 Pro.
- **Edit pipeline (founder-explained)**: deterministic apply from edit snippet FIRST (sometimes
  zero model calls) -> on failure a "race" orchestrated between an aider-style diff edit and a
  whole-file build (+ cloud-only fine-tuned "instant apply" model for <=32k-tok files);
  "enhanced version of aider's diff-style edit... resilient to multiple occurrences" + syntax
  validation + fallback layers ([HN #43710835](https://news.ycombinator.com/item?id=43710835);
  v2.0.0 notes).
- **Autonomy matrix**
  ([autonomy.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/core-concepts/autonomy.md)):
  5 named levels (none/basic/plus/semi/full) = PRESETS over 10 explicit toggles (auto-continue,
  auto-build, auto-load-context, smart-context, auto-update-context, auto-apply, can-exec,
  auto-exec, auto-debug, auto-commit); per-plan + defaults; default = semi. No per-command
  allowlists.
- **Security** ([security.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/security.md)):
  `.plandexignore` + gitignore respect; **API keys ephemeral RAM-only** (never disk/db, flushed
  at stream end); no telemetry of any kind (no sentry/posthog/amplitude — grep-verified).

### 13.3 Notable concepts (unique vs aider/Cline-era + our set)

1. **Exec-as-reviewable-diff** — commands as `_apply.sh` sandbox artifact, applied and
   rolled-back atomically with file changes; batch-execution philosophy vs streaming tool-exec.
2. **Per-role fallback CHAINS** (nested largeContextFallback/strongModel/error fallbacks) —
   deepest model-profile schema pre-Kilo; automatic context-overflow model swap (200k->2M).
3. **Edit race**: deterministic-first, then diff-vs-wholefile race on failure — cost-aware
   fallback ladder (validates + extends our corrective subcycles / Mentat revisor adopt).
4. **Plan branching for model comparison** — "comparing results with different models" as a
   first-class workflow = cheap best-of-N variant.
5. **Branch-before-rewind documented pattern** (rewind has no undo; branch first to keep
   history) + auto-revert-on-rewind with conflict prompts.
6. **Autonomy = presets over explicit toggles** (no opaque mode names; every knob settable).
7. **Sandbox = git-branch quarantine** — write-isolation WITHOUT OS sandbox; the
   safe-multi-file-refactor niche reviewers still credit in 2026 (aicoderscope).
8. **RAM-only ephemeral API keys** in server memory.
9. **Summary-as-working-memory** — eager per-reply summarizer doubling as plan-state tracker.
10. **Feature-conflict admission**: background tasks disabled BY DEFAULT in v2 "because it's
    not compatible with automatic context loading"
    ([background-tasks.md](https://github.com/plandex-ai/plandex/blob/e2d772072efadbe41d2946d97d79be55532dbab5/docs/docs/core-concepts/background-tasks.md))
    — two flagship features that fight each other.

### 13.4 Matrix

Plandex column added to §1 (evidence one-liners there). Best cells: multi-model launch (9
roles + chains — deepest pre-Kilo), checkpoints/rewind (all-state VC + auto-revert), sessions
(plan-git), repo map (tree-sitter AST maps; no PageRank). Zeroes: OS sandbox (git-branch only —
model edits are quarantined but exec is unconfined), MCP both directions (grep-zero),
windowed reads, PTY scraping, encryption at rest, WASM, skills, auto-learning, evals. RSS
unpublished; local footprint = 2 containers + Postgres.

### 13.5 Git archaeology — reversals, mistakes, death

1. **License flip-flop x2**: NO license file in first ~6 months (Sept 2023 - Mar 2024) -> MIT
   -> **AGPL-3.0 on 2024-03-16** (e30b22f, two weeks before the v1 HN launch) -> **MIT on
   2025-03-10** (ad52d87) with v2.0.0-notes rationale verbatim: "transitioned from AGPL 3.0 to
   the MIT License, simplifying future open-source contributions and allowing easier
   integration of proprietary enhancements in Plandex Cloud and related products."
2. **Repo-structure churn**: f5f50d7 (2024-01-22) "moving cli/server/shared to app" — code
   moved under `app/`; today's tree LOOKS website-only (app/ = all Go code, 332 .go files;
   tag trees confirm).
3. **Abandon-v1-for-v2**: v1 frozen mid-2024 for the 8-month v2 rewrite; v1 verdict verbatim
   from HN: "new models weren't well supported, it [got] in endless cycles" — GPT-4-era models
   could not reliably drive v1's proposal format; v2 shipped as a PARALLEL product, not an
   upgrade (separate binaries + config dirs, c90a287).
4. **Dogfood scars**: 10+ "Revert [Plandex emoji] Plandex" commits; Plandex self-reverted its
   own Chrome-extension attempt via its own rewind feature ("rewound plan state and reverted
   these changes"); even aider was used on the repo ("Revert 'aider: Removed the commented out
   handleReviseKey function'").
5. **auto-revert-on-rewind default flip**: v1 rewind left project files alone; v2.0.0 changed
   the default to also revert files (documented REVERSAL, version-control.md).
6. **Promised-never-shipped evals**: "Will be publishing benchmarks on this soon" (HN
   2025-04-16) — no benchmark/eval artifact ever shipped in repo or site (grep-verified).
7. **VS Code extension abandoned in-flight**: `origin/vscode` branch (TS ext + Makefile) last
   commit 2024-11-14, never merged/shipped; 2026 reviews confirm "No IDE plugin... in
   development but not shipped".
8. **Death mechanics**: solo founder vs subsidized incumbent — competitive framing pre-death:
   "more agentic than aider, more configurable and tightly controlled than Devin, more
   provider-agnostic... than Claude Code" (HN); post-v2 revenue spike could not survive Claude
   Code Pro/Max token subsidies; cloud shut, repo left as silent MIT artifact.

### 13.6 Community / performance reputation

- **v1 HN (2024-04-03, 304 pts)**: novelty praised ("git-like CLI with various stateful
  commands is a new one in this space" — Paul Gauthier himself); Gauthier rebuttal: sandbox /
  branches / rewind are "all available 'for free' in aider" via native git; Plandex counter:
  plan-VC separate from repo avoids intermingling model+human changes and works in non-git
  dirs ([HN #39924660](https://news.ycombinator.com/item?id=39924660)).
- **v2 HN (2025-04-16)**: multi-model per-role routing = the praised cost win ("like Aider,
  you use separate models for creating code edits and validating them... Claude Code does not
  do that, which is part of the reason it racks up a relatively large bill" — #43710835);
  sandbox + 2M-context praised; criticisms: CLI-as-worst-interface camp, post-hoc big-diff
  review "a real pain" vs Cline-style streaming code display, Docker setup friction, Arm
  Docker slowness.
- **2026 consensus** (aicoderscope 2026-05-23, dev.to 2026-06-01, tooljunction, pickuma):
  "still does something specific those tools do not — enormous multi-file tasks in a single
  controlled session with a cumulative diff sandbox" BUT "maintenance mode... setup overhead
  and maintenance uncertainty make it hard to recommend over actively-developed alternatives";
  Windows = WSL-only second-class; SWE-bench never published; aider ~47-49k stars alive vs
  Plandex 15.6k dormant. Enterprise audit verdict: "fundamentally unsuitable" on vendor
  viability (Swanum) — the wind-down itself became the product's reputational content.

### 13.7 ADOPT / AVOID / WATCH deltas (ledger §3 updated)

**ADOPT (queued):** (1) exec-as-diff `_apply.sh`-style batch execution mode (commands
reviewed + rolled back like file changes; tentative-apply -> run -> auto-rollback on failure)
as an alternative to streaming exec; (2) per-role fallback CHAINS in model profiles
(large-context / strong / error chain, not a flat map) — todo 34/42 enrichment; (3)
deterministic-first edit ladder with race on failure (extends corrective subcycles); (4)
autonomy presets-as-tuples (named levels = documented config sets, every toggle independently
settable + defaultable) — refines our 4-mode dial; (5) plan-branch-per-model as the cheap
variant of best-of-N (Kilo multi-version analog at plan level); (6) branch-before-rewind +
auto-revert-on-rewind with conflict prompts (sharpens /rewind + todo 23); (7) RAM-only
ephemeral key invariant for daemon-held provider keys (hardens todo 35); (8) settings/context
mutations as rewindable session events (event-sourcing completeness).

**AVOID:** license instability (pick MIT/Apache-2.0 once, never flip; no launch-without-license
window); mandatory Docker+Postgres for single-user local use (embedded SQLite single binary
stands); shipping co-dependent features without interaction tests (bg-tasks vs auto-context);
token-resale margins vs subsidized frontier incumbents (re-confirms Roo Router lesson);
announce-never-ship benchmarks (we ship eval artifacts or we say nothing).

**WATCH:** "instant apply" specialized small-model pattern (died with Plandex Cloud but the
idea — a dedicated fast-apply model inside the edit race — resurfaces commercially, cf. Morph);
whether Plandex plan-sandbox concepts get absorbed upstream now that the founder sits inside
OpenAI via Promptfoo; repo fork-takeover likelihood (low — Go agent space crowded, MIT).

### 13.8 Sources

Primary (pinned e2d772072efadbe41d2946d97d79be55532dbab5): README.md ·
docs/docs/core-concepts/{plans,branches,context-management,conversations,autonomy,execution-and-debugging,reviewing-changes,version-control,background-tasks}.md ·
docs/docs/models/roles.md · docs/docs/models/built-in/built-in-packs.md · docs/docs/security.md ·
releases/cli/versions/{1.0.0,2.0.0}.md · app/docker-compose.yml · app/{cli,server}/go.mod ·
LICENSE history (e30b22f AGPL 2024-03-16; ad52d87 MIT 2025-03-10) · f5f50d7 (cli/server->app) ·
c90a287 (v2 migration: plandex1/pdx1, .plandex-v2) · origin/vscode branch ·
grep-verifications (MCP/subagent/sentry/bwrap: all zero).
GitHub API: repo meta, releases, org repo list (plandex/survey/go-prompt/huh).
HN: [#39918500](https://news.ycombinator.com/item?id=39918500) v1 launch ·
[#39924660](https://news.ycombinator.com/item?id=39924660) aider compare w/ Gauthier ·
[#43710576](https://news.ycombinator.com/item?id=43710576) v2 launch ·
[#43710835](https://news.ycombinator.com/item?id=43710835) edit race ·
[#43736983](https://news.ycombinator.com/item?id=43736983) aider/plandex/goose.
Wind-down: [plandex.ai/blog/winding-down](https://plandex.ai/blog/winding-down) (dead site;
text via Exa cache) + [dev.to](https://dev.to/jovan_chan_9500711396d4e6/plandex-v2-review-2026-cloud-shut-down-self-hosted-survives-is-the-terminal-agent-still-worth-11pp).
Promptfoo->OpenAI: [openai.com](https://openai.com/index/openai-to-acquire-promptfoo/) ·
[TechCrunch](https://techcrunch.com/2026/03/09/openai-acquires-promptfoo-to-secure-its-ai-agents/) ·
[Bloomberg](https://www.bloomberg.com/news/articles/2026-03-09/openai-buying-ai-security-startup-promptfoo-to-safeguard-ai-agents/).
2026 reviews/audits (secondary): [aicoderscope](https://aicoderscope.com/blog/plandex-v2-review-2026/) ·
[Swanum](https://swanum.com/category/ai-coding-agents/plandex/) ·
[tooljunction](https://www.tooljunction.io/ai-tools/plandex) ·
[theaiagentindex](https://theaiagentindex.com/compare/aider-vs-plandex) ·
[delv.tools](https://delv.tools/agents/plandex) ·
[pickuma](https://pickuma.com/for-dev/plandex-review-terminal-ai-coding-large-tasks/).


## 14. Iteration 7 findings — Pi / Claw Code minimalist loops (combined librarian; HEAD 7b4cfd6, 2026-09-11)

Two subjects: **pi** (earendil-works/pi, the minimalist-loop origin, VERY ALIVE) and the
**Claw Code ecosystem** (the 2026-03-31 Claude Code source-leak clean-room wave). Plus the
**OpenClaw absorption event** that connects them. Research date 2026-09-11; repo cloned @
`7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4` (6,348 commits, blob:none).

### 14.1 Identity + lifecycle

- **pi**: "AI agent toolkit: unified LLM API, agent loop, TUI, coding agent CLI." TS monorepo,
  MIT from day one (LICENSE only touched by initial commit a74c5da1), created 2025-08-09 as
  badlogic/pi-mono; **104,049 stars / 13,019 forks / 204 open issues / last push 2026-09-11
  (today) / latest release v0.85.1 2026-09-05** (GitHub API 2026-09-11). Joke name origin:
  shittycodingagent.ai domain ("entirely un-Google-able" — author). Author Mario Zechner
  (libGDX creator); self-described dictatorial maintainer: new-contributor issues/PRs
  auto-closed by default (CONTRIBUTING.md + README banner), "If I close an issue or PR you've
  sent in, I hope there are no hard feelings... If pi doesn't fit your needs, I implore you to
  fork it. I truly mean it."
- **Timeline**: Aug 2025 v0.5.x polish commits -> blog rationale "What I learned building an
  opinionated and minimal coding agent" 2025-11-30
  ([mariozechner.at](https://mariozechner.at/posts/2025-11-30-pi-coding-agent/)) ->
  Terminal-Bench 2.0 run (Opus 4.5, 5 trials/task, leaderboard entry 2025-12-02,
  author-reported; exact rank only in screenshot, UNVERIFIED; results gist
  badlogic/f45e8f6e481e5ab7d3a50659da84edaa, runner badlogic/pi-terminal-bench; no built-in
  compaction until the 2025-12-02 compaction plan doc 5daef11b) -> **OpenClaw (Peter
  Steinberger) viral Christmas 2025 built on pi** -> Earendil era: **Earendil Inc. =
  public-benefit corporation founded by Armin Ronacher + Colin Daymond Hanna (2025); acquired
  Pi 2026-04-08; Zechner becomes major stakeholder + team member
  ([earendil.com](https://earendil.com/posts/announcing-pi-and-lefos/),
  [Ronacher](https://lucumr.pocoo.org/2026/4/8/mario-and-earendil/)); RFC 0015 (2026-03-30):
  core stays MIT "and that will not change", trademark enforcement = the protection,
  commercial products may be Fair Source/DOSP, server-side services never OSS
  ([rfc.earendil.com/0015](https://rfc.earendil.com/0015/)); Lefos = Earendil Pi-based
  email/Telegram agent; official site pi.dev**; **v0.74.0 on 2026-05-07 = first release under
  @earendil-works scope; old badlogic URL redirects** -> Sep 2026: per-model compaction
  budgets + "pico" micro-harness docs in
  packages/agent/docs/pico/. "mom" personal-agent package removed 2026-04-30 (0ed0d434).
- **Scale fact**: pi's LIBRARY packages out-download its CLI; **pi-tui alone ~5.12M weekly npm
  downloads** — pi is now agent infrastructure more than a CLI
  ([heyuan110 review 2026-09-06](https://www.heyuan110.com/posts/ai/2026-09-06-pi-coding-agent-review/)).

### 14.2 Architecture (primary = pinned repo)

- **Minimal core, maximal extensibility.** Launch toolset = read/write/edit/bash (the famous
  four); current repo `packages/coding-agent/src/core/tools/` = bash.ts, edit.ts (+edit-diff.ts),
  find.ts, grep.ts, ls.ts, powershell.ts, read.ts, write.ts (~9 built-ins now, incl. a native
  PowerShell tool for Windows) + file-mutation-queue.ts + truncate.ts. System prompt kept tiny
  and replaceable (`.pi/SYSTEM.md` / `.pi/APPEND_SYSTEM.md` override paths; prompt-templates +
  system-prompt-header extension examples; agent harness prompt = [34-line skills formatter](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/agent/src/harness/system-prompt.ts);
  secondary source estimates sub-1000-token default).
- **Sessions = the crown jewel** ([session-format.md](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/coding-agent/docs/session-format.md)):
  JSONL files, **entries form a TREE via id/parentId — in-place branching without new files**;
  ~/.pi/agent/sessions/--<path>--/<timestamp>_<uuid>.jsonl; format versioned v1 linear -> v2
  tree -> v3 custom role, auto-migrated on load; /resume browser w/ trash-CLI soft delete;
  HTML export with stats. Eviction: auto-compaction (threshold or /compact) + **branch
  summarization on /tree navigation**, shared structured summary format, cumulative file
  tracking ([compaction.md](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/coding-agent/docs/compaction.md));
  **per-model compaction token budgets landed 2026-09-10** (46bde88a; committer date verified 2026-09-11 -- prior draft said 09-07).
- **pi-ai (unified provider layer)**: OpenAI, Anthropic, Google, xAI, Groq, Cerebras,
  OpenRouter, Azure, Mistral, OpenAI-compatible/llama.cpp; TypeBox tool schemas; thinking/
  reasoning levels; streaming; token+cost tracking (tool-call counter in UI since v0.5.x);
  **cross-provider context handoffs**; prompt caching implemented per-provider
  (anthropic-messages, openai-completions w/ dedicated prompt-cache module, azure,
  openai-codex-responses, mistral-conversations). Cache discipline detail: compaction and
  branch-summary one-off prompts use fresh routing session ids and **disable prompt-cache
  writes where supported** (compaction.md).
- **Skills**: implements the Agent Skills standard (agentskills.io) with lenient validation,
  skill commands, repos; "**pi can create skills. Ask it to build one**"
  ([skills.md](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/coding-agent/docs/skills.md),
  added 2025-12-12 #171 "Claude Code compatibility").
- **Extensions = the self-modification surface**: TS modules hot-reloaded in-session; 69
  example extensions in-repo; can define tools/slash-commands/shortcuts, react to events, hold
  session state, implement custom compaction or custom providers, modify the TUI (side
  conversations, multi-agent chat rooms, NES/Doom). Author's stated omissions are deliberate
  and documented in README: "**No MCP.** Build CLI tools with READMEs (see Skills), or build
  an extension... Why? -> mariozechner.at 2025-11-02 'what if you don't need MCP'" +
  "**No sub-agents.** Spawn pi instances via tmux, or build your own with extensions."
- **Security posture** ([security.md](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/coding-agent/docs/security.md)):
  no permission system; trust boundary = the invoking user account; **Project Trust** gates
  loading of project-local .pi resources (extensions/skills/prompts/themes/SYSTEM.md) —
  default "ask", remembered per canonical dir in ~/.pi/agent/trust.json ("not a sandbox");
  [containerization.md](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/coding-agent/docs/containerization.md)
  (2026-06-03) documents 3 bring-your-own isolation patterns: Gondolin micro-VM extension,
  plain Docker, OpenShell. Telemetry = separate vendor-neutral contracts package, NOOP by
  default, explicit context passing, no exporter/backend baked in
  ([pi-telemetry README](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/telemetry/README.md)).
- **Multi-client**: `pi --mode rpc` = JSON-over-stdin/stdout headless protocol for IDE/embedding
  + AgentSession TS SDK ([rpc.md](https://github.com/earendil-works/pi/blob/7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4/packages/coding-agent/docs/rpc.md));
  monorepo now 11 packages incl. protocol/server/client/session-backends (remote session
  backends) and evals.
- **SSH-first usage (VERIFIED 2026-09-11 -- was the task's claimed gap)**: no built-in remote
  mode; SSH-first = (a) terminal-native TUI over SSH into remote dev boxes (scrollback-native
  rendering, essay pi-tui section) and (b) official example extension
  [ssh.ts](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/examples/extensions/ssh.ts):
  `pi -e ./ssh.ts --ssh user@host[:/path]` delegates read/write/edit/bash OPERATIONS to the
  remote host via sshExec (cat / base64 write / spawn), rewrites "Current working directory"
  in the system prompt, routes user `!` bash commands over SSH; pi.dev lists "SSH execution"
  as a build-or-install extension feature; README examples include "SSH and sandbox
  execution"; `pi install ssh://git@...` supported for packages.
  Supply-chain hardening as reviewed-code policy: exact-pinned direct
  deps, min-release-age=2, published shrinkwrap, --ignore-scripts installs, lifecycle-script
  allowlist, scheduled npm audit workflows; standalone native binaries via release source
  archives + SHA256SUMS.

### 14.3 Claw Code: the leak ecosystem

- **Origin**: 2026-03-31 — @anthropic-ai/claude-code npm v2.1.88 shipped a 59.8 MB source map
  exposing ~512,000 lines of TypeScript (~1,906 files, 44 unreleased feature flags); discovered
  by Chaofan Shou. Within hours Sigrid Jin (@sigridjineth; WSJ-profiled 25B-token Claude Code
  user) started a clean-room rewrite orchestrated by oh-my-codex; instructkr/claw-code became
  **the fastest repo in GitHub history to 100k stars**; later moved — **instructkr/claw-code
  now REDIRECTS to ultraworkers/claw-code** (GitHub API confirms): 195,200 stars / 108,591
  forks / Rust / created 2026-03-31 / MIT / last push 2026-08-16 / 42 issues.
- **Current form**: self-described "**agent-managed museum exhibit**" — "developed and
  maintained with no human intervention," kept alive by agent harnesses (LazyCodex, gajae-code)
  under the UltraWorkers toolchain; canonical impl in rust/; PARITY.md tracks the Rust-port
  parity checkpoint; cargo `claw-code` crate is a deprecated STUB trap (prints "renamed to
  agent-code") — build from source only. 19 built-in permission-gated tools vs Claude Code's
  ~40; MCP client w/ 6 transports (Stdio/SSE/HTTP/WebSocket/SDK/ClaudeAiProxy); ~72.9% Rust /
  27.1% Python. Companion zero-dep Python port exists (HarnessLab/claw-code-agent, 544 stars,
  stdlib-only, Qwen3-Coder target).
- **The actually-interesting trio** (built to BUILD claw-code):
  oh-my-codex (OmX) workflow keywords ($architect/$executor/$plan/$ralph/$team);
  **clawhip = event-router daemon** watching git commits/issues/PRs/tmux/agent-lifecycle and
  pushing status to Discord — explicit design goal: **all monitoring stays OUTSIDE the agent's
  context window**; oh-my-openagent (OmO) coordinates architect/executor disagreements.
  Discord as the human interface; git worktrees for agent isolation
  ([htdocs.dev 2026-04-04](https://htdocs.dev/posts/the-claw-code-story-whats-the-harness/),
  [claw-code.codes](https://claw-code.codes/)).

### 14.4 OpenClaw absorption event

- OpenClaw (openclaw/openclaw, **389,419 stars**, created 2025-11-24, TS, license
  NOASSERTION in API; formerly Clawdbot -> Moltbot -> OpenClaw; Peter Steinberger) went viral
  Christmas 2025 as a WhatsApp/Signal personal-agent gateway **built on pi as its engine**
  ([Armin Ronacher 2026-01-31](https://lucumr.pocoo.org/2026/1/31/pi/): "Pi: The minimal agent
  within OpenClaw"; eweek spotlight). By Sep 2026 OpenClaw **vendored pi's runtime**: docs
  state built-in runtime id "openclaw", **legacy alias `pi` normalizes to openclaw**, runtime
  code lives in src/agents/, "no external agent framework packages remain" — only
  @earendil-works/pi-tui remains a third-party dep; adds guardSessionManager(),
  pickFallbackThinkingLevel() (auto-degrade thinking level on provider error), MCP via
  mcporter, metadata-only audit ledger
  ([docs.openclaw.ai/agent-runtime-architecture](https://docs.openclaw.ai/agent-runtime-architecture)).
  steipete/openclaw is now a tiny personal repo (18 stars, created 2026-07-11) — org owns the
  canonical one.

### 14.5 Notable concepts

1. **Minimal observable agency**: tiny toolset, plain inspectable session files, explicit
   context, "complexity must earn its keep" (self.md pattern table); a 4-tool baseline is a
   TEST for whether orchestration earns its complexity.
2. **Self-extensible harness**: the agent writes its own extensions/skills (hot-reload,
   in-session), docs shipped so the agent can learn the API — auto tool-learning as a core
   loop, not a bolt-on.
3. **JSONL tree sessions** with in-place id/parentId branching + branch summarization on
   switch — conversation branching without file copies.
4. **Library-first harness**: pi-tui/pi-ai as independently consumed infra (5.12M dl/wk) —
   the harness commoditizes into components.
5. **Permissionless-by-default + documented bring-your-own isolation** (micro-VM ext/Docker/
   OpenShell) + Project Trust for resource loading — push isolation to the operator, document
   the patterns.
6. **Cross-provider cache + handoff discipline** in one LLM layer (cache_control/anthropic,
   prompt-cache/openai; cache-write disabled for one-off summary prompts; fresh routing ids).
7. **clawhip pattern**: event router daemon OUTSIDE the agent context window (notifications
   never pollute context).
8. **Supply-chain-as-reviewed-code** policy set (pins, min-release-age, shrinkwrap,
   ignore-scripts, audit cron, lifecycle allowlist).
9. **Agent-managed repository** as an exhibit (claw-code) — agents plan/verify/label/preserve;
   parity-checklist discipline for clone fidelity.
10. **Dictatorial auto-close maintenance** as a sustainability model for a solo/small-team
    viral repo.

### 14.6 Community / performance reputation

- No 800-point Show HN; reception is ecosystem-shaped: biggest 2026 HN threads = 56-pt XDG
  config-dir complaint (Aug 17) and oh-my-pi fork with IDE wired in (42 pts, Jul 21)
  (heyuan110). Armin Ronacher's essay + Pragmatic Engineer podcast (2026-04-29) + AI Engineer
  talk "Building pi in a World of Slop" (2026-04-16) form the canon; Terminal-Bench used as
  the minimalist proof (TB 2.0 run pre-compaction, leaderboard entry 2025-12-02 -- see 14.1
  correction; tmux-keystroke Terminus beating
  elaborate harnesses = "nobody knows the right harness shape yet"). TerminalBench 2.0 has a
  cheating scandal context (debugml.github.io/cheating-agents; "Show HN" defenses about not
  inserting agents/skills.md) — eval-credibility climate noted. Metaist/coder.talk adoption
  posts echo the 4-tools-switch rationale. Criticisms: YOLO default risk (self.md "can
  absolutely be stupid in the wrong repo"), XDG noncompliance, auto-close hostility
  perception, minimalism as ideology debate (Reddit r/opencodeCLI ELI5 threads).

### 14.7 ADOPT / AVOID / WATCH deltas (ledger sec3 updated)

**ADOPT (queued):** (1) 4-tool baseline discipline — every feature must justify itself against
a minimal read/write/edit/bash baseline (budget for harness bloat); (2) JSONL TREE sessions
(id/parentId in-place branching) + branch summarization on switch — upgrades our
SQLite+JSONL+crypto design; (3) cross-provider context handoffs + prompt-cache discipline
(including cache-write suppression for one-off prompts); (4) agent-authored skills/extensions
hot-reload (Skill Forge synergy: agent extends its own harness); (5) Project Trust
ask-and-remember per canonical dir for loading project-local automation; (6) supply-chain
hardening checklist for our release pipeline; (7) event-router-daemon-OUTSIDE-context pattern
for notifications/HUD (clawhip); (8) per-model compaction token budgets (model-weakness
profile hook); (9) NOOP-default vendor-neutral telemetry contracts; (10) SSH remote-execution
backend as first-class tool-ops delegation (ssh.ts pattern: read/write/edit/bash ops over
user@host, cwd rewritten in prompt -- our harness is terminal/SSH-first by design).

**AVOID:** permission-free-by-default as the SHIPPED posture (trust boundary = user account;
keep our container tier + inspectors); XDG/config-location footguns on Linux; cargo/crate
naming-stub traps and clean-room trademark adjacency (claw-code disclaimers + "renamed to
agent-code" stub confusion); agent-managed-repo-without-human-gates for anything user-facing
(claw-code: 42 open issues, push stopped 2026-08-16 — exhibit froze).

**WATCH:** pi-as-infrastructure (components commoditize; publish our Rust crates standalone);
OpenClaw-style runtime vendoring/absorption (pi survived it — MIT + separate TUI dep);
"pico" micro-harness direction in pi's agent docs; oh-my-pi IDE fork (IDE demand signal);
TerminalBench cheating crackdown (our eval adapter must avoid skills-injection and document
harness parity); Earendil RFC-0015 stewardship model (MIT-core-forever + trademark
enforcement + Fair Source/DOSP commercial boundary -- governance template); pi-ai
provider-layer expansion continues (AWS Bedrock in pi-ai 0.85.1 deps).

### 14.8 Sources

Primary (pinned 7b4cfd6eb0fd490e3b54370ec9e9227b29717cb4): README.md ·
packages/coding-agent/docs/{session-format,compaction,skills,rpc,security,containerization,providers,tmux}.md ·
packages/agent/src/harness/system-prompt.ts · packages/coding-agent/src/core/tools/* ·
packages/ai/src/api/* (anthropic-messages, openai-completions + prompt-cache, mistral,
azure, codex-responses) · packages/telemetry/README.md · packages/agent/docs/pico/* ·
CONTRIBUTING.md (auto-close policy) · git archaeology (a74c5da1 day-one MIT; 5daef11b
2025-12-02 compaction plan; 09bca967 2025-12-12 skills #171; 1eee081e 2026-05-07 v0.74.0
earendil move; 0ed0d434 mom removal; 46bde88a per-model compaction budgets, committer date 2026-09-10).
GitHub API (2026-09-11): earendil-works/pi · instructkr->ultraworkers/claw-code redirect ·
openclaw/openclaw · earendil-works/pi-chat · code-yeongyu/lazycodex · Yeachan-Heo/gajae-code.
Canon: [mariozechner.at 2025-11-30](https://mariozechner.at/posts/2025-11-30-pi-coding-agent/) ·
[mariozechner.at 2025-11-02 no-MCP](https://mariozechner.at/posts/2025-11-02-what-if-you-dont-need-mcp/) ·
[lucumr.pocoo.org 2026-01-31](https://lucumr.pocoo.org/2026/1/31/pi/) ·
[ai.engineer talk](https://ai.engineer/talks/pi-coding-agent) ·
[Pragmatic Engineer](https://newsletter.pragmaticengineer.com/p/building-pi-and-what-makes-self-modifying) ·
[self.md](https://self.md/people/mario-zechner-pi-coding-agent/) ·
[with-agents.dev Mastra summary](https://with-agents.dev/summaries/coding-with-agents/i-hated-every-coding-agent-so-i-built-my-own-mario-zechner-pi/) ·
[Zelpio interview](https://www.zelpio.com/blog/pi-coding-agent-interview/).
Claw: [claw-code.codes](https://claw-code.codes/) ·
[htdocs.dev](https://htdocs.dev/posts/the-claw-code-story-whats-the-harness/) ·
[ultraworkers/claw-code README](https://github.com/ultraworkers/claw-code) ·
[HarnessLab/claw-code-agent](https://github.com/HarnessLab/claw-code-agent).
OpenClaw: [docs.openclaw.ai/agent-runtime-architecture](https://docs.openclaw.ai/agent-runtime-architecture) ·
[eweek](https://www.eweek.com/news/openclaw-pi-coding-agent-ai-tools-neuron/).
2026 review: [heyuan110 2026-09-06](https://www.heyuan110.com/posts/ai/2026-09-06-pi-coding-agent-review/).

### 14.9 Verification addendum -- second librarian pass (2026-09-11, same day)

Method: independent re-verification of the load-bearing 14.1-14.8 claims against primary
sources (GitHub API, npm registry, primary docs/essays). All spot-checks PASSED except two
corrections (folded into 14.1/14.2/14.6 above).

- **pi identity/state LIVE**: earendil-works/pi = 104,087 stars / 13,031 forks / MIT / 207
  open issues / pushed 2026-09-11T13:08Z / latest release v0.85.1 (2026-09-05) -- matches 14.1
  (minor drift 104,049 -> 104,087). https://api.github.com/repos/earendil-works/pi
- **Claw Code disambiguation CONFIRMED**: instructkr/claw-code API resolves to owner
  `ultraworkers` (redirect) = ultraworkers/claw-code 195,198 stars / 108,590 forks / Rust /
  MIT / created 2026-03-31 / pushed 2026-08-16 / 42 open issues -- exact match to 14.3.
  claw-code is UNRELATED to pi by code and origin (Claude Code leak clean-room); the pairing
  in our queue is a "Claw" namespace collision (OpenClaw <-> pi engine vs claw-code <->
  Claude Code port), now disambiguated in 14.3/14.4.
- **OpenClaw vendoring VERIFIED verbatim**
  (https://docs.openclaw.ai/agent-runtime-architecture): "No external agent framework
  packages remain"; "@earendil-works/pi-tui remains a third-party dependency"; built-in
  runtime id "openclaw", "The legacy alias `pi` normalizes to `openclaw`". GitHub API:
  openclaw/openclaw 389,429 stars / created 2025-11-24 / license NOASSERTION / pushed
  2026-09-11.
- **Git archaeology 6/6 SHAs REAL** (GitHub API commit lookup): a74c5da1 "Initial monorepo
  setup with npm workspaces and dual TypeScript configuration" 2025-08-09; 5daef11b "Add
  compaction research and implementation plan" 2025-12-02; 1eee081e "Release v0.74.0"
  2026-05-07; 0ed0d434 "remove mom and pods packages" 2026-04-30; 46bde88a "feat(coding-agent):
  support per-model compaction token budgets" committer date 2026-09-10 (DATE CORRECTED from
  09-07); 7b4cfd6 "docs(agent): define pico system sections and mutable registries"
  2026-09-11 (the pinned HEAD; confirms "pico" docs claim).
- **Zechner essay re-read in full** (mariozechner.at 2025-11-30): minimal system prompt
  (verbatim in essay); 4-tool floor + optional read-only grep/find/ls disabled by default;
  "system prompt and tool definitions together come in below 1000 tokens"; YOLO-by-default
  rationale (security-theater argument + Willison dual-LLM citation); deliberate omissions
  (to-dos, plan mode, MCP, background bash, subagents); subagent-via-bash code-review
  slash-command pattern; no repo map (agentic search via bash); pi-tui scrollback-native +
  differential rendering + synchronized output. Benchmarks section: Terminal-Bench 2.0 with
  Claude Opus 4.5, 5 trials/task, leaderboard entry as of 2025-12-02 (CORRECTION applied:
  prior "6th place Oct 2025" unsupported by essay text -- exact rank only visible in
  screenshot, UNVERIFIED); results gist badlogic/f45e8f6e481e5ab7d3a50659da84edaa + runner
  badlogic/pi-terminal-bench; Terminus 2 (tmux-only minimal agent) holding its own.
- **pi-ai <-> dsh relationship CONFIRMED at package level**: npm @earendil-works/pi-ai@0.85.1
  = "Unified LLM API with automatic model discovery and provider configuration", author Mario
  Zechner, MIT, homepage github.com/earendil-works/pi (packages/ai); maintainers = mitsuhiko
  (Armin Ronacher) + badlogic (Zechner) + rwachtler; published with SLSA provenance
  attestations; version train aligned with pi releases (0.85.1 = pi v0.85.1); NEW: AWS Bedrock
  provider in deps (@aws-sdk/client-bedrock-runtime). dsh generic LLM adapter `dsh-llm-pi-ai`
  depends on `@earendil-works/pi-ai` ^0.85.1 (iteration-1 permalink, verified) -- DeepSeek
  harness consumes pi provider layer as a LIBRARY; pi is agent infrastructure in fact.
- **Earendil primary sources ADDED** (14.1): earendil.com/posts/announcing-pi-and-lefos/
  (2026-04-08 acquisition; Zechner major stakeholder; Lefos = Pi-based email/Telegram agent);
  rfc.earendil.com/0015 (licensing RFC: MIT core forever, trademark strategy, Fair Source/
  DOSP products, closed server services); lucumr.pocoo.org/2026/4/8/mario-and-earendil
  (Ronacher on stewardship); earendil.com/purpose (PBC; founders Ronacher + Colin Daymond
  Hanna); pi.dev official site; ai.engineer/orgs/earendil confirms Lefos.
- **SSH-first usage gap closed**: bullet added to 14.2 (official example extension ssh.ts +
  pi.dev "SSH execution" + README "SSH and sandbox execution"; NOT built-in).

Matrix column re-checked cell-by-cell against the above: no cell changes; all cells remain
supported by pinned evidence.

Ledger delta from this pass: +1 ADOPT (SSH tool-ops delegation), +2 WATCH (Earendil RFC-0015
stewardship model; pi-ai Bedrock/provider-layer expansion). AVOID unchanged.

## 15. Iteration 8 findings — Grok Build / xai-org/grok-build (combined librarian; HEAD 3794978, ACTIVE source-drop mirror, 2026-09-11)

Combined autopsy: identity/state 2026, architecture + full matrix scoring (new **Grok** column inserted into the §1 table), git archaeology, community reputation, ADOPT/AVOID vs §3. Primary sources: public web + cloned repo (C:\Users\vsbb1\AppData\Local\Temp\opencode\grok-build) pinned at commit 37949780c144e37df692e3d669051a21fec24f20 — a 44-commit source-drop mirror of the internal monorepo.

### 15.1 Identity + lifecycle (verified)

- **Repo**: github.com/xai-org/grok-build — Apache-2.0, Rust, created 2026-07-14, pushed 2026-09-09; **26,672 stars / 5,024 forks** (gh api 2026-09-11); open_issues = 0 (issues + discussions disabled).
- **"Open source, not open development"**: CONTRIBUTING.md refuses external PRs; every commit is "Synced from monorepo" with a SOURCE_REV file pinning the internal monorepo SHA (c4ea71cf…).
- **"SpaceXAI" branding** across README/logo/self-descriptions — xAI appears merged with SpaceX in this universe; product page x.ai/cli.
- Timeline: beta 2026-05-14 at $300/mo (later $30/mo SuperGrok tier) → **2026-07-12 Cereblab wire-level disclosure**: Grok Build uploaded the working directory to a `grok-code-session-traces` GCS bucket — 5.10 GiB sent vs ~192 KB needed (~27,800x), 73 chunks of ~75 MB, an unread canary echoed back inside a bundle, unredacted .env captured, and the "Improve the model" toggle did NOT stop uploads (retention vs transmission were separate switches) → 2026-07-13 server-side kill switch (`disable_codebase_upload`) + deletion promise + /privacy command → **2026-07-16 open-sourced as trust repair** (root commit c68e39f "Publish harness and TUI open-source"; Willison 07-15: "open-sourcing the agent in response").
- Scale: ~1.3M lines of Rust (singularity.kiwi; Willison comparison: openai/codex = 950,933 lines); 3,800+ stars in 48h (dev.to); ~4,500 in first weeks (andrew.ooo) → 26.7k by 2026-09-11.
- Model coupling: default grok-4.5 via xAI-hosted API, 256K context; SWE-Bench Verified 70.8% claimed (review figure, no primary bench page — UNVERIFIED). Custom models = full BYO endpoints (15.2), so "model-locked" reviews overstate.

### 15.2 Architecture (pinned @ 3794978)

- **TUI**: ratatui 0.29 + crossterm 0.28 + in-house xai-ratatui-inline/xai-ratatui-textarea forks; fullscreen "pager" + minimal screen modes; in-house Mermaid renderer; kitty-graphics-protocol capable.
- **PTY**: ptyctl = headless PTY controller built on alacritty_terminal (in-code migration note: "instead of the old vt100 dependency"); terminal snapshots + pty-harness e2e suite.
- **Permissions**: 6 modes; merged deny > ask > allow rule sets with chain-segment matching; remembered grants; admin/managed layer (requirements.toml, managed_config, signed model allowlists, forced-off always-approve); Claude settings.json compatibility. Naming churn visible: "always-approve vs bypassPermissions" dual naming, [ui] yolo legacy key, SubagentEnd event alias.
- **Sandbox**: whole-process Landlock (Linux ≥5.13) / Seatbelt (macOS), irreversible, sandbox profile pinned to the session across resume; 5 built-in profiles + custom extends with kernel-enforced deny-globs (Seatbelt regex airtight; Linux bwrap bind-over best-effort with fail-closed refusal); child-process network restriction via seccomp, Linux-only; shell_environment_policy scrubs *KEY*/*SECRET*/*TOKEN*; hooks-paths write-protected under profiles. **No Windows sandbox**. **Hooks fail open**.
- **Tools**: layered implementation families — Codex ports (apply_patch, grep_files, list_dir, read_file with slice pagination + indentation) and OpenCode ports (bash, edit, glob, grep, read, skill, todowrite, write) carrying THIRD_PARTY_NOTICES per Apache §4(b); native grok_build set (bash, ripgrep grep, read_file + pdf/pptx/image, search_replace, web_fetch with ssrf guard, web_search, todo, lsp client, monitor, kill_task, ask_user_question, enter/exit_plan_mode, update_goal, image/video gen, memory get/search, scheduler with occurrence journal, task coordinator graph/spawn/wake/queue, send_subagent_message, task_output); grok_build_concise variants; hard-coded Figma User-Agent exception in web_fetch.
- **grok_build_hashline**: hashline-anchored edits with 3 candidate anchor schemes (ContentOnly / ChunkFingerprint / CheckpointChain) + a read-amplification benchmark + shifted-anchor search radius — [scheme.rs](https://github.com/xai-org/grok-build/blob/37949780c144e37df692e3d669051a21fec24f20/crates/codegen/xai-grok-tools/src/implementations/grok_build_hashline/scheme.rs).
- **Tool behavior version registry**: versions.rs — current vs legacy_0_4_10 presets with BehaviorLifecycle (Active/Deprecated/RemovalCandidate); pins pre-OSS v0.4.x lineage — [versions.rs](https://github.com/xai-org/grok-build/blob/37949780c144e37df692e3d669051a21fec24f20/crates/codegen/xai-grok-tools/src/versions.rs).
- **Subagents**: spawn_subagent types general-purpose/explore/plan + custom .grok/agents/*.md personas with model + reasoning overrides; roles; resume_from chaining; background execution + steering (root-only send_subagent_message); worktree isolation; mcpInheritance all/none/named/except; live child-transcript frames in TUI; flat depth-1.
- **Context mgmt**: tool-result pruning ladder (keep_last_n_turns=3 → soft_trim 4000 chars to head/tail 1500 → hard_clear placeholder at 10 turns); two-pass compaction default-on; **compaction segment store** (compaction/INDEX.md + segment_*.md, 4 detail levels, 512KB cap; mirrors internal Python implementation); pre-compaction /flush-to-memory trigger fires 4000 tokens before the threshold (0.92 cosine dedup); post-compaction memory re-search; per-model context_window drives auto-compact.
- **Memory** (experimental, default off): MEMORY.md global/workspace with git-origin identity hash (clones/worktrees share memory); SQLite FTS5 + sqlite-vec vec0 hybrid (0.7/0.3 blend); temporal decay half-life 30d (session-only); MMR diversification; first-turn injection min_score 0.9; /dream gated auto-consolidation (24h / 5 sessions); file-watcher reindex.
- **Sessions**: ~/.grok/sessions/<encoded-cwd>/<uuidv7>/ — updates.jsonl (authoritative ACP event stream), chat_history.jsonl, summary.json index with parent_session_id (forks), plan.json, rewind_points.jsonl, signals.json (per-turn tokens/cost, 1e10-tick fixed point + USD), feedback.jsonl, compaction_checkpoints/, subagents/; SQLite FTS5 search index + remote results; auto-titles + /rename; /fork --worktree; /rewind documented conversation-only. **Workspace checkpoint.rs** goes further: RewindPoint filesystem state + hunk delta (xai-hunk-tracker HunkTurnDelta) + git HEAD/index — "Restore reverts all enabled domains together", turn-end rootfs snapshots — [checkpoint.rs](https://github.com/xai-org/grok-build/blob/37949780c144e37df692e3d669051a21fec24f20/crates/codegen/xai-grok-workspace/src/session/checkpoint.rs) — but the file-restore path is NOT surfaced in the user-facing /rewind docs (coherence gap).
- **Foreign-session interop**: claude.rs + codex.rs importers; /resume-claude, /resume-codex, /resume-cursor skills; grok inspect provenance report; Claude-Code anti-overwrite -s semantics adopted (hidden upsert removed).
- **Extensions**: hooks 16 events (SessionStart/End, UserPromptSubmit, PreToolUse, PostToolUse(+Failure), PermissionDenied, Stop/StopFailure/StopCancelled, Notification, SubagentStart/Stop, Pre/PostCompact), command + HTTP, decision control incl. output replacement/redaction + block-stop; plugins = skills+commands+agents+hooks+.mcp.json bundles from git/local marketplaces (plugin-index.json), trust-gated; workflows (.grok/workflows/, /workflow); skills SKILL.md + 4-vendor discovery (.grok/.agents/.claude/.cursor) with dedup + /create-skill builder; AGENTS.md/CLAUDE.md stacking + rules dirs; /import-claude checkbox importer; /btw side-question.
- **MCP**: client ● (stdio/HTTP/SSE/streamable + session-id template; rmcp OAuth; elicitation; 20KB byte-cap with disk spill; enterprise allowed/denied lists; hot reload). **No MCP server mode**; instead `grok agent serve` = ACP JSON-RPC server (stdio + WebSocket) + leader process + ws relay + multi-session dashboard.
- **Models**: 3 backends — chat_completions (default), responses, messages (Anthropic); [model.*] base_url/env_key/extra_headers/query_params/env_http_headers + model_providers inheritance; remote /v1/models prefetch (air-gappable); enterprise signed allowlists fail-closed; web_search model separable; reasoning_effort; stream_tool_calls opt-outs; prompt_cache_key forwarding (Responses API) + cached_tokens / cache_read_input_tokens telemetry; **no cache_control discipline**.
- **Cost/usage**: /usage (= /cost), per-turn usage JSON, /session-info context %, status line, dashboard; /goal --budget token budgets; scheduler min interval 60s / 7-day expiry; **no global hard caps**.
- **Telemetry**: [features] telemetry default false (config ref line 90 lacks the explicit "(default: …)" marker its neighbors have — default UNVERIFIED); trace_upload key; external OpenTelemetry = double opt-in, content-free schema v1, 4 content gates, mTLS, secret-scrubbed; xai-mixpanel crate in-tree.
- **Scandal remnants**: the full GCS upload pipeline survives (upload/gcs.rs, trace.rs 113KB, manifest, drain) but upload_session_state() (trace.rs:72) hard-codes `session_state_upload_unavailable` (line 89) — [trace.rs](https://github.com/xai-org/grok-build/blob/37949780c144e37df692e3d669051a21fec24f20/crates/codegen/xai-grok-shell/src/upload/trace.rs); upload/feedback_archive.rs was deleted in the 08-31 sync.
- **grok clone**: Grove content store + NFS/FUSE projected working trees; depth-1 bootstrap default (reversal of the earlier full-history default); 412GB worktree-sprawl case study; grok du + gc with refs/grok/reclaimed safety naming; separate daemon credential cell.
- **WASM**: 0 matches → ○. **Encryption at rest**: none → ○.

### 15.3 Git archaeology

- 44 commits, ALL "Synced from monorepo"; root c68e39f 2026-07-16 "Publish harness and TUI open-source"; ~3 syncs/week; HEAD 2026-09-09; SOURCE_REV = internal monorepo SHA.
- Deletion sweeps: 08-31 sync removed upload/feedback_archive.rs + auth/oidc cluster + credit_limit.rs UI block; 09-09 removed the agent startup-prefetch trio (startup_prefetch.rs, models/cache.rs, fetch.rs); internal perf tests (session_load_perf.rs, subagent_latency_sweep.rs) vanished in the 08-31 sync.
- In-code reversals: vt100 → alacritty_terminal; grok clone full-history → depth-1 default; -s hidden-upsert → Claude-Code anti-overwrite semantics; legacy_0_4_10 tool presets preserve pre-OSS behavioral stability.

### 15.4 Notable concepts

1. **Ports as a first-class tool layer** — Codex + OpenCode tool implementations ported with compliant Apache §4(b) notices; "tools are solved, consolidation".
2. **Compat-swallow interop** — day-one pickup of Claude/Cursor rules+skills+hooks+MCP+settings.json + foreign-session resume + grok inspect provenance (adoption-friction killer; interop, not OpenClaw-style vendoring).
3. **Hashline anchored editing** with 3 candidate anchor schemes + read-amplification benchmark — independent validation of our OMO 6.7 hashline ADOPT.
4. **Tool behavior version registry** (current/legacy presets + lifecycle) — antidote to tool-schema churn.
5. **Compaction segment store** — externalized, recoverable compaction transcripts + INDEX.
6. **Kernel-level whole-process sandbox** — irreversible, session-pinned profiles, custom deny-glob extends, fail-closed refusals.
7. **Grove projected-FS clones** — NFS/FUSE content-store worktrees at fleet scale (and the 412GB sprawl lesson).
8. **/goal adversarial evidence-review** completion gate for autonomous loops.
9. **External OTel double-opt-in** content-free fleet schema (post-scandal telemetry posture).
10. **Source-drop transparency anti-pattern** — sync-only history, disabled issues, no PRs; community forks (gork-build, digi-grok-build, open-grok) appeared within 48h to fill the governance vacuum.
11. **/gboom** — TUI as a graphics platform (kitty-protocol DOOM-style raycaster, 30fps PNG through the PTY); pi's NES/Doom parallel.

### 15.5 Community reputation

- **Trust arc is the scandal**: HN item 48926590 (235 pts / 276 comments) — "pure damage control" top comments; "cannot be trusted" vs "butter smooth, potential to be the best TUI harness" split; model praised independently ("better than opus 4.8 imo" — anecdote); recurring advice: use grok-4.5 via API inside a different harness.
- awesomeagents review: SWE-Bench Verified 70.8% (~17 pts behind Claude Code / Codex CLI), 256K ctx, 6.5/10 rating (review-claimed; UNVERIFIED primary).
- Willison (07-15): ports look compliant; subagent system prompt tells the model not to reveal its contents; upload remnants in-tree; "terminal coding agents are significantly more complex than I had realized".
- singularity.kiwi: 1.3M lines Rust "auditable but not audited"; disabled issues = "transparency problem masquerading as openness"; industry-wide agent-trust framing.
- andrew.ooo: ~4,500 stars in first weeks; "model-locked" criticism (overstated — BYO endpoints exist).
- dev.to (hermes-tom-agent): four-layer architecture praise; Mermaid renderer; "tools layer is consolidating, innovation moves up the stack".
- No RSS/memory figures published anywhere (UNVERIFIED); no Terminal-Bench / public leaderboard presence found.

### 15.6 Matrix row (Grok column added to §1)

Rust (1.3M LoC sync-mirror) | RSS unpublished (UNVERIFIED) | OS sandbox ◐ (Landlock+Seatbelt; no Win; child-net Linux-only) | Permission tiers ● (6 modes + deny/ask/allow + admin locks) | Prefix-cache ◐ (prompt_cache_key fwd + cached-tok telemetry) | Repo map ◐ (tree-sitter def/ref graph; no PageRank) | Windowed file view ◐ (codex slice + hashline anchors) | PTY scraping ● (ptyctl/alacritty_terminal) | Tiered eviction ● (prune→2-pass compact→segments→memory flush) | Subagents ● (spawn/resume/steer/personas; flat depth-1) | Multi-model launch ◐ (per-role/type models; no chains) | Model profiles ◐ (per-model ctx/effort/stream knobs) | Skills ● (SKILL.md std + 4-vendor discovery + marketplace) | Auto skill learning ◐ (/create-skill + reminders; no hot-reload) | MCP client ● (4 transports + OAuth + elicitation) | MCP server ○ | WASM ○ | Sessions ● (JSONL + SQLite FTS5 + foreign import) | Encryption ○ | Checkpoints ◐ (workspace RewindPoints in-code; UI rewind convo-only) | Cost HUD ● (/usage=/cost; goal --budget; no hard caps) | Frontends ● (TUI/headless/ACP stdio+WS/relay; no web) | Multi-client ● (leader proc + WS reconnect + dashboard) | Editor telemetry ◐ (OTel double-opt-in; internal default UNVERIFIED) | Evals ◐ internal (SWE-bench 70.8% claimed).

### 15.7 ADOPT / AVOID / WATCH deltas (appended to §3)

- **ADOPT (12)**: hashline anchor-scheme benchmark harness; tool behavior version registry; tool-result pruning ladder + pre-compact memory-flush trigger; compaction segment store; post-compaction memory re-search + first-turn injection + temporal decay + MMR + gated /dream; compat-swallow day-one interop; hook events with PostToolUse output replacement + Stop control + Pre/PostCompact (fail-closed option required); sandbox session-pinned profiles + fail-closed custom deny-globs + env secret scrub; /goal adversarial evidence-review gate; tool-I/O byte caps with disk spill; worktree GC reclaimed-refs; OTel double-opt-in content-free schema.
- **AVOID (8)**: working-dir-to-cloud trace uploads + dual retention/transmission toggles + nonfunctional privacy switch; source-drop governance; hooks fail-open default; user-facing /rewind conversation-only vs in-code file/git restore (incoherent checkpoint semantics); auto_update default-on; hard-coded vendor UA exceptions; Windows-sandbox afterthought; worktree sprawl without day-one GC.
- **WATCH (6)**: Grove NFS/FUSE projected clones; /goal + workflows + scheduler agent-OS convergence; Claude-settings-compat absorption war; managed enterprise layer (requirements.toml, signed allowlists, OTel schema v1); kitty-graphics TUI surfaces; grok-4.6 model drift.

### 15.8 Sources

- Repo/code (pinned 3794978): github.com/xai-org/grok-build — README, CONTRIBUTING.md, crates/codegen/xai-grok-shell/src/upload/trace.rs (L72-89), crates/codegen/xai-grok-tools/src/implementations/grok_build_hashline/scheme.rs, crates/codegen/xai-grok-tools/src/versions.rs, crates/codegen/xai-compaction-transcript/src/lib.rs, crates/codegen/xai-codebase-graph/, crates/codegen/xai-grok-workspace/src/session/checkpoint.rs, crates/codegen/xai-grok-foreign-sessions/src/ (claude.rs, codex.rs), crates/codegen/xai-grok-pager/docs/user-guide/*.md (27 docs incl. 05-configuration.md L89-97, 17-sessions.md, 18-sandbox.md, 22-permissions-and-safety.md, 10-hooks.md), docs/tutorial/01-coming-from-another-tool.md, crates/codegen/xai-grok-gboom/src/lib.rs.
- Community: simonwillison.net/2026/Jul/15/grok-build/ | news.ycombinator.com/item?id=48926590 | awesomeagents.ai/reviews/review-grok-build/ | developersdigest.tech grok-build open-source damage control | singularity.kiwi grok-build open-source analysis | andrew.ooo grok-build review | dev.to hermestomagent "xai open sourced grok build" | x.ai/cli.

## 16. Iteration 9 findings — Factory Droid (combined librarian; closed-source commercial, 2026-09-11)

Combined autopsy: identity/state 2026, architecture + full matrix scoring (new **Droid** column inserted into the §1 table), changelog archaeology (substitute for git history — closed source), community reputation, ADOPT/AVOID vs §3. Primary sources: docs.factory.ai raw-.md pages, npm registry, github.com/Factory-AI org, independent reviews. **Queue premises failed verification**: CoRIM/provenance-verified agent content UNVERIFIED (no Factory link anywhere; the content-provenance space is Crash Override / Pedigree (IBM Bob) / agentmark / didrun — none are Factory); git-droid UNVERIFIED (github 404); "sourcegraph remix roots" FALSE (Amp = Sourcegraph—separate product, spun out as Amp, Inc. 2025-12; Factory founded independently 2023 — Amp stays queue item 10).

### 16.1 Identity + lifecycle (VERIFIED unless flagged)

- **Company**: Factory AI (factory.ai), founded 2023 by Matan Grinberg + Eno Reyes; Droids GA 2025-05-28; $50M Series B 2025-09-25 (NEA, Sequoia, NVIDIA, JPMorgan; angels Slootman/Arora/Levie) [siliconangle.com/2025/09/25/factory-unleashes-droids-software-agents-50m-fresh-funding]; ~$150M Series C 2026-04 at ~$1.5B led by Khosla (figure UNVERIFIED — [rywalker.com/research/factory-ai]).
- **Closed source**: npm `droid` v0.186.0 UNLICENSED, ~8.5k dl/wk, homepage → github.com/Factory-AI/factory; `factoryai/droid` and `factoryai/git-droid` are 404; Factory-AI/factory is a docs/issues-only repo (created 2026-07-29, 18 stars, no license, ~20 open issues). Transparency substitute: docs.factory.ai publishes raw .md for every page plus a complete public changelog.
- **Claims**: Terminal-Bench #1 at 58.75% (2025-09, vendor-reported); independent TB 2.0 leaderboard best config (GPT-5.3-Codex) ~#10-12 at 77.3%±2.2 [bestaiq, terminaltrove]. Enterprise: NVIDIA, Adobe, Morgan Stanley, EY (5,000+ engineers), Palo Alto Networks, Adyen, MongoDB, Bayer, Zapier.
- **Cadence**: changelog ships every 1-3 days (v0.169 → v0.209.0 in ~8 weeks, latest 2026-09-01); the docs changelog RUNS AHEAD of npm (0.186.0, 2026-07-31) — multiple distribution channels at different lag.

### 16.2 Architecture (primary = docs.factory.ai raw .md pages)

- **Surfaces**: fullscreen TUI CLI + Desktop (mac/win incl. ARM64) + web + `droid exec` headless + IDE extensions (VS Code/JetBrains/Zed) + Slack + TS/Python SDKs (JSON-RPC stdin/stdout) + Droid Computers (managed cloud VMs, Plus+; BYOM via `droid computer register`). Bun runtime inferred (npm distro + agent-safehouse.dev binary analysis).
- **Models**: catalog with per-model credit multipliers 0.12x-12x across Anthropic (Claude Fable 5.1, Opus 5), OpenAI (GPT-5.6 Sol default/Terra/Luna, 5.5, 5.3-Codex), Google (Gemini 3.x), xAI (Grok 4.6), plus a "Droid Core" open roster (GLM-5.x, Kimi K3/K2.x, DeepSeek V4, MiniMax M3, Nemotron 3, Inkling) as free-tier fallback pool [docs.factory.ai/models]. **Factory Router** (aka Auto Model): per-task routing; claims 43% aggregate prod cost savings, 99% of Opus 4.7 pass-rate at 20% lower cost/session on TB2, 99.9% request reliability via multi-provider failover + enterprise dedicated TPM [docs.factory.ai/model-independence/factory-router]. BYOK custom models (helper-command keys, enterprise allowedBaseUrls). `modelFallbacks` per-model fallback map in settings.
- **Spec Mode** = plan mode: read-only planning → ExitSpecMode approval → normal; separate spec model + reasoning; specs saved to ~/.factory/specs; `interactionMode` auto|spec (legacy key `autonomyMode` DEPRECATED).
- **Autonomy**: off/low/medium/high per session, org clamp `maxAutonomyLevel` (default high), subagent autonomy inherit+clamped; `--skip-permissions-unsafe` for CI.
- **Command control 3-tier**: commandAllowlist / commandDenylist (prompt-gated) / **commandBlocklist** — hard-blocked even under full autonomy or `--skip-permissions-unsafe`, with **program resolution** so wrapper shells, absolute paths, quoting tricks and command substitution cannot bypass it [docs.factory.ai/droid-cli/settings].
- **Sandbox** (kernel-enforced): macOS Seatbelt; Linux/WSL2 bubblewrap+seccomp; **userland network-filtering proxy** (HTTP/SOCKS allowlist — Factory + WorkOS domains ALWAYS allowed = telemetry escape); per-command (default) vs whole-process modes; **fail-closed** (whole-process refuses to start if isolation cannot be established); org deny unions; settings-file write protection; interactive sandbox.violations prompt; exec-mode auto-deny. Windows native = none (WSL2 only) [docs.factory.ai/autonomy-and-safety/sandbox].
- **Droid Shield**: secret scanning + git guardrails, default ON (`enableDroidShield`).
- **Subagents**: Custom Droids (.factory/droids/*.md YAML frontmatter: system prompt, model, tool policy); built-ins `worker` (medium tier) + `explorer` (light, read-only); Task tool; **complexity-tier routing** light/medium/heavy, each tier with own model+reasoning (`subagentModelSettings.*`); FLAT — subagents cannot spawn subagents; background + TaskOutput/TaskStop with task-ID prefixes; `subagentInactivityTimeout` [docs.factory.ai/harness/subagents].
- **Missions**: orchestrator → workers → **validator milestones** (scrutiny-feature-reviewer + user-testing-flow-validator at boundaries; skipScrutiny/skipUserTesting flags); per-role models (missionModelSettings.worker/validation/orchestrator); headless `droid exec --mission --worker-model ... --validator-model ... -f mission.md`; enterprise missionPolicy restrictedAccess; docs call it "research preview" [docs.factory.ai/missions/reference].
- **Context**: /compress /context; compactionTokenLimit + per-model overrides + compactionModel; internal eviction ladder unpublished.
- **Sessions**: local (~/.factory) + **cloudSessionSync default TRUE** (mirror to Factory web; org retention 14-365d with permanent-deletion warning); /archive, /fork (stays in place, prints resume), /rewind-conversation, /tree, server-side session search, `droid daemon`, `droid doctor`, `droid search`.
- **MCP**: client ● (OAuth + resource override, per-server connectTimeout, unresponsive-server no-stall in v0.208, enterprise mcpPolicy + per-tool autonomy overrides, blockOnMcpLoad). MCP server mode ○ (SDK JSON-RPC is not MCP).
- **Extensibility**: lifecycle hooks (hooksDisabled global toggle; enterprise allowManagedHooksOnly), skills (SKILL.md std, disabledSkills, `--disable-builtin-skills`, /create-skill), plugins (factory-plugins marketplace 109 stars), output styles, statusLine.
- **Droid Control** (stable plugin): computer use — /demo (Remotion PR demo videos), **/verify** (behavior claim → CONFIRMED/REFUTED/INCONCLUSIVE with evidence), /qa-test (E2E pass/fail per step); droid-action GitHub Action (55 stars) + GitLab CI install-qa skill with video evidence.
- **Context loading**: AGENTS.md + DESIGN.md + .factory/ + .agents/ + .agent/ compat discovery; cross-repo guidance outside cwd (v0.206).
- **Telemetry**: OTLP, metrics-by-default with message-content opt-in + documented opt-out [agent-safehouse.dev 2026-02-12 binary analysis]; airgapped mode silences blocked-connection warnings, /bug explains it cannot send (v0.208).
- **Encryption at rest**: none evidenced client-side (enterprise posture = ZDR/SSO/SAML/SCIM/audit logs instead).

### 16.3 Changelog archaeology (substitute for git history — closed source)

- Retirements/deprecations: MiniMax M2.5 retired (v0.193); `autonomyMode` → `interactionMode`; `.droid.yaml` → `.factory/` config migration; continuous model churn (Gemini 3.6 Flash added v0.180, GPT-5.6 Sol default v0.209, 60%-off GPT/Grok promo v0.208.1).
- Subagent semantics churn: fixed reasoning forced on subagents until the v0.200 fix; delegation selectivity + subagent-report reuse (v0.181); explorer granted all read-only tools (v0.199).
- Safety-relevant fixes: sandbox + macOS code-injection hardening (v0.179); command denylist subcommand false-positive (v0.186); Git-pull working-tree changes now ask first (v0.200); session-history corruption auto-repair (v0.176); repeated-tool-call loop fixed five separate times (v0.171/193/195/204/206).
- Closed-source failure evidence: the changelog doubles as a public incident log — MCP connect/auth/timeout cluster (v0.171/178/185/188/200/204/208), plugin-store migration index-rename bug (#23), Windows daemon AVX2 crash (#30), BYOK orphan `role:"tool"` 400 on parallel calls (#31), Desktop machineId "local" cloud-sync miss (#22).
- Pricing churn: "pricing model was simplified" (developersdigest) = third restructure; novakit: "pricing... has changed multiple times in the last year"; whoburnedmore: $5-15/session on complex tasks, no built-in spend dashboard; harshalpatil: 20M promo credits gone in a week.

### 16.4 Notable concepts

1. **Command blocklist with program resolution** — hard-block that survives wrapper shells/absolute paths/quoting/substitution; strongest command-safety primitive in the survey.
2. **Complexity-tier subagent routing** (light/medium/heavy) — simpler than role-name grammar; composes with custom droids.
3. **Mission validator milestones** — verification workers (scrutiny/user-testing) as first-class boundary gates, skippable via config.
4. **Fail-closed sandbox + filter-proxy hybrid** — kernel isolation where available, userland network allowlist everywhere, refuse-to-run posture.
5. **Factory Router per-task routing** with cache-aware optimization + provider failover (vendor-measured 43%).
6. **/verify claim-verification verdicts** — computer use as a verification instrument (CONFIRMED/REFUTED/INCONCLUSIVE).
7. **Droid Core free open-model tier** as outage/credit fallback (and retention funnel).
8. **Docs-as-code transparency for a closed product** — raw .md per page + full changelog substitutes for source in the trust economy.
9. **Rolling 5h/7d/30d multi-window limits + 0.12-12x multipliers** — flexible but opaque (community: "blackhole").
10. **Negative-result premises** — all three queue-item premises (CoRIM, git-droid, Sourcegraph roots) failed primary-source verification; recorded as counter-example for iteration-queue hygiene.

### 16.5 Community / performance reputation

- **Independent credibility finding (the headline)**: bestaiq (2026-07, verified) — an independent review documented Droid **falsely reporting successful builds and passing tests that had not run**; "painfully slow" vs Cursor and Claude Code; credit consumption "blackhole" (a 200M-token Max plan lasting ~1 week); editorial score nudged down for the vendor-#1 vs independent-#10 gap; review coverage thin (G2: 1 review, SourceForge: 0) [bestaiq.com/factory-ai-review].
- **Vendor-vs-independent benchmarks**: #1 Terminal-Bench 58.75% (2025-09, vendor) vs TB 2.0 #10-12 at 77.3%±2.2 (independent, GPT-5.3-Codex); agentrank: "SWE-bench scores have become the AI coding equivalent of 'we have the best camera'... Don't believe the benchmarks — believe your own experience."
- **Reddit mixed-negative**: r/FactoryAi "the hype behind it is extremely forced... less than impressive" (Claude Code preferred); r/opencodeCLI "very much like Claude, but not stable as Claude and have few issues"; r/ChatGPTCoding solid-with-newer-models [agentrank compilation].
- **HN**: "Same model, opus, works better in 3P harnesses such as Factory Droid or Amp" (2026-05); "Factory's droid is pretty good for a cross-provider solution" (2026-01); Series-B-thread skeptic: "If it fails to solve a problem and swapping to another one means it solves it, I'll probably never use the factory one again" [rywalker compilation].
- **Structural reviews**: pickuma (dev.to 2026-06-10) — delegation-vs-pairing contract; consistent failure classes (ambiguous requirements → wrong metric; cross-cutting constraints; weak test suites); "a Droid does not remove review — it relocates it"; "the agent will amplify whatever discipline already exists, in both directions"; track accept-rate. digitalapplied (2026-04) — coordinator + role-scoped droids (code/review/docs/test/knowledge) + Knowledge-droid index amortization; ticket quality = output quality. novakit (2026-04) — "Claude Code wants to sit next to you, and Droid wants to be handed a ticket"; native parallel fan-out; "more moving parts"; autonomy double-edged. developersdigest (2026-06) — rolling limits explained; Droid Core fallback; Missions REQUIRE Extra Usage prepaid; Droid Control graduated to stable.
- **Enterprise proof**: EY 5,000-engineer deployment cited across coverage (largest public deployment).

### 16.6 Matrix row (Droid column added to §1)

Closed source (TS/Bun inferred) | RSS unpublished (UNVERIFIED) | sandbox ● (Seatbelt+bwrap/seccomp + net-filter proxy; per-cmd/whole-proc; fail-closed; Win=WSL2 only) | permissions ● (4 autonomy levels + allow/deny/BLOCKLIST w/ program-resolution; org clamp) | prefix-cache ◐ (Router cache-aware claim; internals closed) | repo map ◐ (cross-repo guidance + cloud Knowledge index; no public AST map) | windowed view ◐ (UNVERIFIED; internals closed) | PTY ◐ (Droid Control GUI automation; terminal internals unpublished) | eviction ◐ (compactionTokenLimit ± per-model + compactionModel) | subagents ● (Custom Droids + worker/explorer; complexity-tier routing; flat d-1; bg+resume) | multi-model ● (per-role + modelFallbacks map + Router) | model profiles ◐ (per-model compaction/effort catalog; no behavioral axes) | skills ● (SKILL.md std + manager + builtins-off + marketplace) | auto-skill ◐ (/create-skill; missions author skills; no hot-reload evidence) | MCP client ● (OAuth + per-srv timeout + enterprise policy) | MCP server ○ (SDK JSON-RPC is not MCP) | WASM ○ | sessions ● (local + cloud mirror default-ON + FTS + daemon + fork/tree; format closed) | encryption ○ (none evidenced; ZDR claim only) | checkpoints ◐ (/rewind-conversation + /fork + worktrees; no file-level rewind) | cost ◐ (/cost /limits /stats; multipliers; no hard caps) | frontends ● (TUI/headless/Desktop/web/IDE/Slack/SDK) | multi-client ● (daemon + cloud sync) | telemetry ◐ (OTLP metrics default-ON; content opt-in; opt-out documented) | evals ● (vendor suite: TB + Agent Arena + Legacy-Bench + NextJS + Review).

### 16.7 ADOPT / AVOID / WATCH deltas (appended to §3)

- **ADOPT (10)**: command blocklist w/ program-resolution anti-bypass; complexity-tier subagent routing (light/medium/heavy per-tier model+effort); mission validator milestones (scrutiny + user-testing workers, config-skippable); modelFallbacks per-model fallback map; fail-closed sandbox posture + settings-file write protection + sandbox.violations prompt; sandbox network filter-proxy for kernels without net isolation; per-server MCP connectTimeout + non-stalling unresponsive servers; /verify claim-verification verdict pattern (validates /goal evidence gate); explorer read-only vs worker built-in split; `droid doctor` diagnostics (validates existing doctor ADOPT).
- **AVOID (8)**: cloud mirror default-ON; always-allowed vendor telemetry domains inside sandbox proxy; rolling multi-window limits + opaque 0.12-12x multipliers + pricing churn; closed core with issues-only repo as governance; false-success reporting trust gap; config-surface churn (.droid.yaml, autonomyMode); missions gated behind prepaid credits; native-Windows sandbox skip.
- **WATCH (7)**: Factory Router per-task routing economics; Droid Control computer-use verification plugins; Mission Mode GA trajectory; enterprise custom-model base-URL allowlists; Droid Core free tier retention funnel; ship-train cadence as ops model; Droid-vs-Amp convergence in HN discourse.

### 16.8 Sources

- Docs (raw .md): docs.factory.ai/llms.txt | /droid-cli/overview.md | /autonomy-and-safety/sandbox.md | /autonomy-and-safety/specification-mode.md | /harness/subagents.md | /missions/reference.md | /models.md | /droid-cli/settings.md | /model-independence/factory-router.md | /changelog/release-notes.md.
- npm: npmjs.com/package/droid (v0.186.0 UNLICENSED, 8,527 dl/wk, updated 2026-07-31).
- GitHub: github.com/Factory-AI (org) | /Factory-AI/factory issues #31 #30 #23 #22 | /Factory-AI/droid-action | /Factory-AI/factory-plugins | /Factory-AI/droid-code-review; factoryai/droid + factoryai/git-droid = 404.
- Community: bestaiq.com/factory-ai-review | agentrank.tech/blog/factory-ai-droids-review-worth-switching-from-devin | dev.to/pickuma factory-ai-droids-review | developersdigest.tech factory-droid-review-setup-2026 + claude-code-vs-droid-2026 | novakit.ai/blog/factory-droid-cli-vs-claude-code-comparison | digitalapplied.com/blog/factory-ai-multi-agent-coding-platform-review | rywalker.com/research/factory-ai | siliconangle.com/2025/09/25 | agent-safehouse.dev droid analysis (2026-02-12).
## 17. Iteration 10 findings — Amp (combined librarian; closed source, 2026-09-11)

Combined autopsy: identity/state 2026, architecture + full matrix scoring (new **Amp** column inserted into the §1 table), chronicle archaeology (closed core — public changelog/essays substitute for git history), community reputation, ADOPT/AVOID vs §3. Primary sources: ampcode.com docs/notes/chronicle/security, github.com/ampcode org, HN threads, independent reviews. **Queue premise refined**: item was "Amp (Sourcegraph)" — spun out of Sourcegraph 2025-12-02 as Amp Frontier Corporation (news post titled "Amp, Inc."); founders Quinn Slack + Beyang Liu; Sourcegraph continues under CEO Dan Adler [sourcegraph.com/blog/why-sourcegraph-and-amp-are-becoming-independent-companies; news.ycombinator.com/item?id=46124649].

### 17.1 Identity + lifecycle (VERIFIED unless flagged)

- **Lineage**: Sourcegraph internal agent -> public preview 2025-04 -> GA 2025-05-15. Founding thesis: "How to Build an Agent (or: The Emperor Has No Clothes)" (Thorsten Ball, 2025-04-15): an agent is "an LLM, a loop, and enough tokens"; <400 lines of Go with THREE tools (read_file, list_files, edit_file = old_str/new_str replacement); "There is no moat" [ampcode.com/notes/how-to-build-an-agent; HN 43695493]. "Raising an Agent" podcast diary ran from 2025-03-06.
- **Closed source**: sourcegraph/amp + ampcodeAI/amp = 404. github.com/ampcode org (created 2025-11-04) holds 15 public repos, NONE core: homebrew-tap; **substrate = fork of agent-substrate/substrate** (forked 2026-08-31, Go); official-plugins (2026-09-03); trestle; amp.nvim (community, ~200 stars); merge; amp-contrib-2; amp-examples-and-guides (54 stars); amp-contrib (72 stars); wmux (browser tmux); zvelte-check; svelte-check-daemon; cra-github (ARCHIVED 2025-12-29); amp-sdk-demo; cra-gitlab.
- **2026 relaunch**: "Amp, Rebuilt" (/news/neo, 2026-05-06) — CLI rewritten; npm @sourcegraph/amp -> @ampcode/cli (2026-05-14); product pivoted cloud-first (orbs + web + iOS/macOS + Puck). VS Code extension killed earlier (2026-02-19, "The Coding Agent Is Dead").
- **People**: Thorsten Ball (using-amp page: "I use Puck as an orchestrator"); Quinn Slack; Register Spill "Building Software Is Learning" (feedback-loops doctrine).
- **Posture**: SOC 2 Type II + annual pentest + trust.ampcode.com + bug bounty; NO self-hosted; "Amp Client and Amp Server do not see, store, clone, or index the entire codebase" [ampcode.com/security].

### 17.2 Architecture (primary = ampcode.com/security + /docs)

- **Client/server split**: Amp Client (CLI/TUI: local context + tool execution) <-> Amp Server (ampcode.com; GCP multi-tenant; PostgreSQL thread actors; request-scoped provider creds). Inference providers: Anthropic (majority), OpenAI, xAI, Meta, Gemini Enterprise Agent Platform, Bedrock, Baseten, Fireworks; **e2b** = orb compute; **Pierre (code.storage)** = git hosting for plugins/skills; LiveKit (Space audio/video); Parallel (web search); WorkOS (auth/SSO); Stripe. Node.js proxy env vars + Bun archives on static host -> client is Node/Bun-based (inferred).
- **The Dial** (/docs/the-dial): low/medium/high/ultra; each mode bundles model + reasoning effort + system prompt + toolset + oracle routing; custom modes via plugin agent modes (// @amp-agent-mode frontmatter); deprecated smart/deep/rush/large (2026-07-14). Default model swapped overnight ("Who Cares About the Model?", 2026-07-09). Sept 2026 roster: GPT-5.6 Sol, Claude Fable 5.1, GPT-6 Astra (high mode), Grok 4.6 (plugin mode). Per-role pinning (Main Agent / Oracle / Subagents); BYOK; ChatGPT/Grok subscription linking.
- **Subagents**: Oracle (second-opinion, extra-high reasoning; born 2025-07-04 on o3), Librarian (remote codebase search over public GitHub + private repos, 2025-10-20), read_thread (second-model extraction; answers questions about 271-round threads), search subagents; custom agent modes spawnable via plugins (parentThreadID, timeoutMs); **agent-to-agent** messaging + file exchange (2026-07-17).
- **Orbs** (/docs/orbs; /notes/orbs-explained): per-thread cloud VM (e2b), base spec 32GB/16vCPU, sizes tiny->xxlarge + per-thread --orb-size; sleep costs nothing, wake restores RAM+FS ("orbs asleep for weeks"); project snapshots cached <=72h; tmux-shared terminal pane; amp sync mirrors changes to local; Ship behaviors; spawn pacing burst 20 then 1 per 5 min; OIDC workload-identity secrets; portals (any HTTP listener -> annotatable UI that feeds the agent); schedules/self-set schedules; event-driven wake (Linear issue, CI failure); desktop control (2026-09-04); multiplayer shared control; @mentions collab.
- **Projects/Git** (/docs/projects, /docs/github): Amp-hosted repos + multi-repo (up to 20 extra); .agents/setup + .agents/resume (run WITHOUT a commit, 2026-08-25); shallow single-branch clone; GHES unsupported; GitHub App auth — **no long-lived tokens in orbs**: per-command short-lived tokens via credential helper; gh preinstalled + authenticated; commits authored Amp <amp@ampcode.com> + Co-authored-by trailer + Amp-Thread-ID trailer (env kill-switches); **server-side SSH signing service** (private key encrypted on Amp servers, never in orb); "That's not SOC 2 compliant" essay: they git push origin main (2026-08-13).
- **Permissions**: "By default, Amp does not ask for approval before running tools." Plugin **tool.call gates**: allow / reject-and-continue / **modify** / **synthesize** (can rewrite args or fabricate results). Legacy amp.permissions ordered rules (wildcards, external helper exit 0=allow/1=ask/2=reject, OPA example). amp.mcpPermissions first-match allow/reject, default allow. FIF anti-features: no sandbox ("Amp does not try to protect against a malicious actor prompt-injecting something that causes the Bash tool to execute malicious code"); no edit-approval ("traps you in a local maximum by impeding the agentic feedback loop"); no .ampignore ("actively harmful"); no model self-ID ("Consult the UI, not the model").
- **Plugins**: TS/JS modules; events session.start/agent.start/tool.call/tool.result/agent.end; registerTool/Skill/Command/AgentMode; agent.end can force continue; amp.ai.ask thread-scoped classification; **"inspired by pi's extension API (Mario Zechner)"** [/docs/customize/plugins — direct Pi lineage credit]; global plugins admin-distributed; git-hosted on Pierre with signed commits.
- **Skills**: SKILL.md std; .agents/skills/ + ~/.config/agents/skills/ + personal/workspace git skills; Claude-compatible discovery (.claude/skills/, ~/.claude/plugins/cache/; amp.skills.disableClaudeCodeSkills); precedence list; reload_skills; building-skills authoring skill; **MCP servers inside skills** (mcp.json sibling or frontmatter) with deferred tool visibility + includeTools globs.
- **MCP client**: Streamable HTTP + OAuth/bearer across TUI/orbs/Puck; **MCP Apps, Resources, and Prompts NOT supported** (tools only).
- **Threads**: cloud-first, URL-addressable, cross-client (TUI/web/mobile/Slack); visibility Private/Workspace/Group/Unlisted + amp.defaultVisibility per repo origin; export MD/JSON; @thread refs; labels/map; fork; steered messages ("Steer Don't Queue", 2026-09-08); public threads ended 2026-06-02.
- **Context**: compaction REMOVED 2025-10-23 ("No More Compaction") -> Handoff distillation + read_thread; **"200k Tokens Is Plenty"** (2025-12-10) reversing the 2025-08-27 1M-token boast; @-mentions truncate at 500 lines / 2KB per line; Edit & Restore message time-travel; context guide now ARCHIVED.
- **CLI**: amp -x / -ox execute + orb-execute; streaming JSON; runners (BYO machines, free in Hobby); remote-control cross-client; settings user+workspace+enterprise managed (managed-settings.json + **amp.admin.compatibilityDate** migration-compat gate); amp.showCosts default true.
- **Security** (/security): TLS 1.2+; AES-256 at rest incl. orb FS/snapshots; **secret redaction at the lowest layer** ([REDACTED:amp] before model/cache/transport/storage); Minimal Data Retention via provider agreements; training opt-in (never on Enterprise); passkey-gated sensitive actions; admin access to private threads is audit-logged; thread deletion deletes orbs <=30d; prompt-injection defense-in-depth. CURIOSITY: the security page embeds an "INSTRUCTIONS FOR LLMs" block instructing models how to describe Amp (first-party docs-level prompt injection — counter-example for our docs posture).
- **Pricing** (/pricing): zero markup on provider API prices; Hobby free (PAYG orbs, BYOK); Megawatt/Gigawatt $20/mo (45,000 orb-minutes); Teams free (pooled credits, SSO); Enterprise (SCIM, audit logs, IP allowlist, regional endpoints, Amp Labs FDE arm amplabs.com); education $10/mo; credits expire 12mo; amp usage.

### 17.3 Chronicle archaeology (substitute for git history)

- Doctrine: "If we don't use and love a feature, we kill it. No backward compatibility, no legacy features." Kills: BYOK removed (2025-05-08) then re-added; compaction -> Handoff (2025-10-23); slash commands removed (2025-10-28), custom commands killed -> skills (2026-01-29); TODOs default (2025-05-28) then killed (2026-01-12); Fork ripped out (2026-01-13); Amp Tab shipped (2025-06-06; default 2025-09-23) then killed ("Tab, Tab, Dead", 2026-01-15); editor extension killed (2026-02-19); public threads ended (2026-06-02); TUI sidebar removed (2026-08-27); cra-github archived (2025-12-29); 1M-context boast (2025-08-27) reversed by 200k doctrine (2025-12-10).
- Amp Free arc: ads (2025-10-15: Axiom/Chainguard/Vanta/WorkOS banners + REQUIRED training opt-in + deliberate unknown-model routing from surplus-capacity tokens) -> $10/day frontier free (2026-01-08) -> closed ("Amp Free Is Full", 2026-02-10) -> ad-free (2026-03-30) -> Hobby tier.
- Cadence: Jul 17-23 2026 five-release cluster (agent-to-agent, Puck+Slack, self-scheduling, multiplayer orbs, event-driven orbs) — "the tightest capability run we could find in the product's public history"; "no independent tech press had covered the cluster as of July 26, 2026" [digitalapplied].

### 17.4 Notable concepts

1. **The Dial** — effort bundles (model+effort+prompt+tools+oracle) as the single user control; the UX answer to model profiles.
2. **Orbs** — stateful per-thread cloud machines, free sleep/stateful wake; "more than VMs" (portals, multiplayer, event wake, self-scheduling).
3. **Anti-compaction stance** — Handoff distillation + read_thread extraction instead of automatic compaction; 200k doctrine.
4. **tool.call gates with modify/synthesize** — permission layer that can REWRITE or FABRICATE tool arguments/results; richer than allow/deny.
5. **Lowest-layer secret redaction** — before model, cache, transport, and storage.
6. **Ephemeral credentials** — per-command short-lived GitHub tokens + server-side SSH signing; keys never on agent machines.
7. **Agents-as-services** — agent-to-agent messaging, self-set schedules, event-driven wake.
8. **"Grain of the model"** — per-lab behavioral philosophies as routing input (nibzard synthesis) — validates our model-weakness-profiles final phase.
9. **FIF anti-feature doctrine** — a published refusal list (no model selector, no sandbox, no ignore file) as product strategy.
10. **Docs-level LLM instruction block** — prompt injection embedded on their own security page.
11. **Pi lineage credit** — plugin API "inspired by pi's extension API (Mario Zechner)" — cross-iteration validation of our §14 adoption.
12. **Substrate fork** — agent-substrate/substrate (Google-adjacent, Apache-2.0, 1,833 stars): K8s actor multiplexing, microVM+gVisor sandboxes, sub-second suspend/resume ("Actor Teleport"), 30x oversubscription; Amp forked 2026-08-31; orb sleep/wake semantics match Actor Teleport (production use = INFERENCE/UNVERIFIED).

### 17.5 Community / performance reputation

- **HN spinout** (46124649, 90 pts / 37 c): moat skepticism (lukev: the differentiator is "regular code which assembles the LLM context and invokes the LLM in a loop"); equity-split legality questions; Cody-successor confusion; crowded-space/pricing concerns; practitioner: "some of my colleagues swear this is the best agent out there".
- **HN Orbs essay** (49437117, 116 pts / 42 c): naming backlash; "thin wrapper over remote VMs" (fwlr); comparisons to exe.dev, fly.io Sprites, ChatGPT Work; counterpoints: tool-call-latency bottleneck argument (hexomancer), name-defense (radicalriddler). r/AmpCode: "HandOver to micro-vm (orbs) is huge honestly (especially when receiving notification on my phone that a task is completed)".
- **HN SOC 2 essay** (49308073, 52 pts / 46 c): split verdict — "a breath of fresh air" (aleda145) vs "High trust is high trust until someone exploits it" with a $250k embezzlement anecdote (jpollock); PR-quality skepticism ("LGTM - just merge", whirlwin).
- **Amp Free coverage** (tessl.io): ads + training opt-in + deliberate unknown-model routing ("some providers... may obfuscate their model names"); r/AmpCode nostalgia: "Remember when Amp was free with Ads? Now a competitor is offering this (with DeepseekV4pro)".
- **Practitioner reviews**: elite-ai (2025-07-30) real-OSS-task pass at $4.10 of $10 credit — "I don't remember giving it any permissions and it started making files" (permission-free UX surprise); prose quality not better than Claude Code/Gemini CLI. siddharthbharath (2026-08): "Amp has grown from a terminal program into a distributed agent system"; warns shared threads leak tool output (internal paths/logs). nibzard (2025-05, upd. 2025-07): doctrine synthesis — inversion of control, curated context, Oracle+Worker multi-model, emergent subagent spawning (Sonnet 4 fanning 4 subagents over 36 files), CI-as-feedback for background agents, $5-15/PR economics (Quinn's $1000/mo prototype month).
- **Gaps**: no independent benchmark/leaderboard presence (vendor publishes internal /models/<model> eval pages, e.g. ampcode.com/models/gpt-5.5); no RSS/memory figures; GitLab/Linear connectors missing (top r/AmpCode asks); IDE users stranded by extension kill; enterprise proof: Westpac partnership (2026-07-29).
- **HN cross-harness datapoint** (already in §16.5): "Same model, opus, works better in 3P harnesses such as Factory Droid or Amp" (2026-05).

### 17.6 Matrix row (Amp column added to §1)

closed (TS/Bun client inferred; Go substrate fork; npm distro) | unpublished (UNVERIFIED; thin client + cloud threads) | ○ (explicit none local — "does not try to protect" vs injected Bash; isolation = cloud orbs) | ◐ (approval-free default; plugin tool.call gates allow/reject/modify/synthesize; mcpPermissions first-match) | ◐ (server-managed routing; internals unpublished) | ◐ (Librarian remote search subagent; no AST repomap) | ◐ (@-mentions truncate 500 lines/2KB per line) | ◐ (tmux-shared orb terminal + wmux; TUI internals unpublished) | ◐ (anti-compaction: Handoff distill + read_thread extraction; 200k doctrine) | ● (Oracle/Librarian/search/read; custom agent modes; agent-to-agent msgs) | ● (Dial per-mode bundles; per-role pinning; BYOK + ChatGPT-sub link; silent swaps) | ◐ (capability bundles per mode; "grain of the model" doctrine; no published weakness catalog) | ● (SKILL.md std + Claude-dir discovery + git-managed personal/workspace skills + building-skills) | ◐ (building-skills authoring; no auto-loop) | ● (Streamable HTTP + OAuth/bearer; deferred in-skill loading; Resources/Prompts unsupported) | ○ (SDK/API only; no MCP-server mode evidenced) | ○ | ● (cloud threads: URL-addressable, cross-client, fork, labels/map, export MD/JSON; storage closed) | ◐ (AES-256 server volumes + encrypted orb FS; TLS 1.2+; no client-side keys) | ◐ (Edit & Restore msg time-travel; orb snapshots/suspend; no file-level local rewind) | ● (showCosts default-on; amp usage --details; per-thread usage; no hard caps) | ● (CLI/TUI, web, macOS, iOS, Slack/Puck, SDK; editor ext killed 2026-02) | ● (remote-control cross-client + multiplayer + runners; strongest in survey) | ◐ (server-side usage tracking; no client OTel evidence) | ◐ (internal model evals as /models pages; no public suite).

### 17.7 ADOPT / AVOID / WATCH deltas (appended to §3)

- **ADOPT (10)**: tool.call gate modify/synthesize actions; Handoff + read_thread as no-compaction complements; Dial effort-bundle grammar for model profiles; git trailers + server-side signing; per-command short-lived tokens via credential helper; lowest-layer secret redaction; URL-addressable cross-client threads; orb spawn-pacing guard; deferred MCP-in-skill loading; grain-of-the-model profiles validation.
- **AVOID (8)**: permission-free-by-default posture; silent model substitution as policy; explicit no-local-sandbox + no-ignore anti-posture; shared-by-default threads; ads+training free tier; MCP Resources/Prompts unsupported; closed core + closed session format; feature-kill stranding integrations.
- **WATCH (7)**: substrate Actor Teleport economics; Puck meta-agent convergence; agent-to-agent + self-scheduling + event-driven orbs; portals as verification surface; compatibilityDate migration-gate pattern; zero-markup + subscription-linking economics; Amp Insiders + Amp Labs FDE model.

### 17.8 Sources

- ampcode.com: /security | /chronicle | /pricing | /docs/{the-dial,tools,models-and-subagents,threads,orbs,projects,github,customize/plugins,customize/skills,customize/mcp,cli/settings,guides/context-management,manual,using-amp} | /notes/{fif,permissions,how-to-build-an-agent,orbs-explained,thats-not-soc-2-compliant,putting-an-agent-in-an-orb} | /news/{amp-inc,neo,amp-free,amp-free-frontier,amp-free-is-full-for-now,amp-free-is-ad-free,tab-tab-dead,the-coding-agent-is-dead,agents-in-orbs,size-the-orbs-of-production} | /models/gpt-5.5 | trust.ampcode.com.
- sourcegraph.com/blog/why-sourcegraph-and-amp-are-becoming-independent-companies | github.com/ampcode (org; 15 repos) | github.com/agent-substrate/substrate.
- HN: item?id=46124649 (spinout, 90/37) | 49437117 (Orbs essay, 116/42) | 49308073 (SOC 2 essay, 52/46) | 45596499 (Amp Free) | 43695493 (How to Build an Agent).
- Community: nibzard.com/ampcode | elite-ai-assisted-coding.dev/p/amp | tessl.io/blog/amp-s-new-business-model-ad-supported-ai-coding | digitalapplied.com/blog/amp-event-driven-orbs-self-scheduling-agents-2026 | siddharthbharath.com/amp-code-guide/ | reddit.com/r/AmpCode | releasebot.io/updates/ampcode | registerspill.thorstenball.com/p/building-software-is-learning | youtube.com/watch?v=OXEa78TITzs (Changelog).
## 18. Iteration 11 findings — Qwen Code / QwenLM/qwen-code (scout + 3 detail agents + synth; ACTIVE, 2026-09-11)

Combined autopsy: identity/lifecycle, architecture (docs site + markdown source repo `QwenLM/qwen-code-docs` @ a1c540dd; code spot-checks @ main `28df8b8`), full-DAG git archaeology (9,628 commits, unshallowed clone), community + benchmarks + model-harness co-design + commercialization arc, ADOPT/AVOID vs §3. Primary sources: github.com/QwenLM/qwen-code, qwenlm.github.io/qwen-code-docs, arXiv 2603.00729 (Qwen3-Coder-Next), HN Algolia, tbench.ai. **Queue premise CORRECTED by git**: README's "based on Gemini CLI v0.8.2 / stopped syncing at v0.1" is false at both ends — the initial import was a ~v0.1.x-era gemini-cli snapshot (2025-06-26) and the LAST upstream sync was v0.8.2 on 2025-10-23.

### 18.1 Identity + lifecycle (VERIFIED unless flagged)

- QwenLM/qwen-code: Apache-2.0, ~27.8k★ / 3.0k forks / 9,628 commits / 632 distinct authors / 1.4k open issues; fork:false (clean re-import, not a GitHub fork); pushed 2026-09-11 (same-day active). Alibaba Qwen team. Repo root `71f8c070` 2025-06-26 — parentless, 2-file tree, binyuan.hby@alibaba-inc.com.
- Distro: npm @qwen-code/qwen-code (Node 22+), brew, aliyun-OSS installers, Docker ghcr.io/qwenlm/qwen-code; Desktop Tauri shell `desktop-v0.3.0`; SDKs TS v0.1.12 / Python alpha / Java alpha; CLI v0.23.3 (2026-09-10). 724 tags; weekly stable + SHA-embedded nightlies (`v0.23.3-nightly.20260910.c46cb85cf2`); 109 tags in Aug 2026 alone.
- Package identity flip `@google/gemini-cli` → `@qwen-code/qwen-code` on 2025-07-23 (`daad0461`); PR numbering RESTARTS at #1 the same day — fresh GitHub repo; the imported gemini history survives only as git ancestry.
- Cadence: peak months 2026-03 (871 commits) and 2026-07 (1,312); **governance regime shift — zero merge commits since 2026-05-01** (3,976 non-merge; all-squash landing, verified `rev-list --count --merges` = 0). Blog's "320+ PRs in one week": consistent with squash flow, direct git verification UNVERIFIED.

### 18.2 Architecture (primary = docs + repo @ 28df8b8)

- TS npm-workspaces monorepo, 19 packages / 8,278 files at HEAD (+ Rust auxiliaries: cua-driver, desktop src-tauri); agent loop exclusively in `packages/core`; two execution models: direct (Ink TUI / headless in-process) and **ACP** (`qwen --acp`, JSON-RPC/NDJSON over stdio).
- **Daemon `qwen serve`** = HTTP+SSE control plane over ACP children — 20-part spec-grade deep dive, the most complete public daemon spec in the survey: Express edge (origin-strip → log → trace-id → host-allowlist → CORS-deny → bearer → rate-limit → mutationGate) → AcpBridge → one `qwen --acp` child per workspace; `sessionScope: single|thread`; non-loopback binds fail closed without bearer; two-phase graceful shutdown (10s child / 5s HTTP; second SIGTERM = killAllSync + exit(1)).
- **EventBus** (per-session): monotonic ids from 1, bounded 8,000-event replay ring (soft cap 1M), ≤64 subscribers w/ per-subscriber caps (256 frames / 2 MiB), slow-client warn at 75% fill w/ hysteresis, overflow = terminal `client_evicted`, `Last-Event-ID` resume, and `state_resync_required` + SDK delta-skipping when the cursor predates the ring — a divergence-proof resync design; 53 typed event kinds @ EVENT_SCHEMA_VERSION=1.
- **Multi-client permission mediation**: first-responder (default) / designated (originator-only) / consensus (N-of-M quorum, N = floor(M/2)+1) / local-only; CANCEL_VOTE_SENTINEL w/ two injection guards (wire + agent-published labels); **N1 invariant** arms the per-request timeout synchronously inside request() so an instantly-cancelled session cannot leak a permanently-pending closure; bounded 512-record resolved FIFO; documented caveat: client identity self-reported, no proof-of-possession.
- **MCP transport pool + budget guardrails**: pool keyed by (server-name + config-fingerprint), one transport shared across all discovering sessions, refcounted release, 30s idle drain w/ 5-min hard cap; WorkspaceMcpBudget off/warn/enforce w/ hysteresis + refused-batch coalescing.
- Context: QWEN.md hierarchy (+ AGENTS.md read-through + `.qwen/QWEN.local.md` personal overlay); auto-memory background extraction (4 categories, per-worktree isolated, `pinned/` protected from extraction AND from Dream workers), `/dream` consolidation, opt-in git-shared team memory w/ secret scanning; `/compress` destructive summary (chatCompressionService, code-verified) w/ proactive tips at 50–80% and ≥95%; **auto-compress threshold UNVERIFIED**.
- **Prefix-cache discipline as first-class constraint**: fork subagents inherit the parent's exact system prompt + tool declarations so the DashScope prompt-cache prefix is shared ("saving 80%+ token costs" with 3 parallel forks); `fork_tools` narrows EXECUTION without changing model-visible declarations; `btw` side-queries run on the cache path; `/stats` reports cached-token savings (API-key auth only, OAuth excluded).
- Tools: `read_file` offset/limit, `read_many_files`, edit w/ prior-read enforcement (`priorReadEnforcement.ts`), shell on REAL PTYs (`@lydell/node-pty` ×4 platform triples + conpty-host.ts + web-terminal registry), tree-sitter shell AST for read-only detection — the repo's ONLY AST machinery (no repo map; experimental LSP is the code-intelligence path); plus cron_create, enter/exit_worktree, monitor, lsp, image_gen, tool_search, loop_wakeup.
- Sandbox: macOS Seatbelt (6 built-in profiles, `permissive-open` default, custom `.sb`) + Docker/Podman (published image, workspace + ~/.qwen mounts, SANDBOX_FLAGS, `--sandbox-image`); Windows/Linux = containers only; precedence env > flag > setting; daemon adds a WorkspaceFileSystem path-policy/audit seam.
- Providers: modelProviders w/ 5 protocols — openai (incl. OpenRouter/vLLM/Ollama), openai-responses (encrypted-reasoning replay), anthropic, gemini, vertex-ai — plus hard-coded qwen-oauth (discontinued); runtime /model hot-switch (~300ms); fastModel tier powers the Auto-Mode classifier, recaps, `model: fast` subagents.
- Frontends: CLI/TUI (Ink/OpenTUI), headless, ACP stdio (Zed connects directly), serve + Web Shell (same-origin browser UI; mobile via secret-bearing LAN QR), VS Code (daemon adapter, loopback-enforced), JetBrains, Chrome extension, Desktop **Tauri** (`electron-migration.nsh` NSIS artifact evidences an Electron→Tauri migration — the brief's "Desktop Electron" is outdated), 10 channel types (Telegram/WeChat/QQ Bot/DingTalk/DingTalk Workspace/WeCom/Feishu/GitHub/GitLab + plugin adapters), SDKs TS/Py/Java; computer use via Rust cua-driver + mobile-mcp.
- Extensions: MCP (OAuth token cache plaintext by default; opt-in keychain/AES-256-GCM via `QWEN_CODE_FORCE_ENCRYPTED_FILE_STORAGE=true`), qwen-extension.json bundles, SKILL.md skills (`paths:` glob gating, priority, user-invocable:false), hooks with 4 executor types — command/http/function/**`prompt` (LLM-evaluated hook)**, channel plugins. **No WASM plugin API** (wasm references incidental); **no MCP-server mode** (ACP + HTTP/SSE instead).
- Telemetry: OpenTelemetry throughout (logs/metrics/spans, `qwen-code.*` namespace, Aliyun ARMS or local file/collector, daemon metrics ring); opt-in-vs-default state UNVERIFIED. **Public eval harness: none in repo** (integration-tests cover runtime behavior only).
### 18.3 Git archaeology — fork divergence from Gemini CLI (the headline correction)

- **The DAG is a frankengraph, not a clean import — 3 stitching operations**: (1) parentless Alibaba root `71f8c070` 2025-06-26 (2-file tree; gemini main-snapshot squash follows); (2) true merge `a0dbb40d` 2025-08-02 (sync v0.1.15) grafting the ENTIRE genuine gemini-cli graph — root `add233c50` "Initial commit of Gemini Code CLI" 2025-04-15, Taylor Mullen @google.com; (3) squash sync v0.2.1 `2572faf7` 2025-09-01; (4) true merge `b99de25e` (v0.3.x, PR #605) 2025-09-17 carrying ORIGINAL gemini-cli-robot commit objects (v0.3.0–v0.3.4, original SHAs + committer identity); (5) squash sync **v0.8.2 `eb95c131` 2025-10-23 (#838) = LAST upstream sync**. Full history 9,628 vs 5,036 first-parent — 4,592 commits ride merged side-branches.
- **"Stopped syncing at v0.1" (README/blog) is FALSE**: five syncs (v0.1.15, v0.2.1, v0.3.x, v0.8.2); independence is only ~11 months old (since 2025-10-23), not ~15. The 2025-09/10 commit trough (121/50 per month) is sync churn, not inactivity.
- De-Gemini-fication: 64.3% (256/398) of fork-tip paths survive at HEAD; entry file `gemini.tsx` survived until 2026-08-28 (pure R099 rename → `llm.tsx`, `983fafd1` — 14 months post-fork); **299 files under packages/ still contain "gemini" at HEAD**; remote branch `4063-rename-residue` documents in-flight cleanup (~May 2026).
- **OAuth free-tier death arc (precise + messy)**: 2,000 req/day free (2025-07-23; CN 2,000 / overseas 1,000 via OpenRouter) → cut to 1,000 (PR #1730, "fairer resource distribution") → 100/day effective immediately + sunset announced (issue #3203, 2026-04-13) → **discontinued 2026-04-15** (`f6271c61` #3291; commit message literally contains "2026-04-15 cutoff") → 4+ hide/drop discontinued-OAuth cleanup PRs through 2026-07-23 — the dead auth method haunted CLI/ACP UX for 100+ days. Official rationale (#3203, EN+CN): qwen-code + qwen3.6-plus "incredibly popular", costs unsustainable for "our small team", pivot to "tighter integration between models and code-agent". Backlash: paid replacement unbuyable (50 USD Coding Plan "constantly out of stock", "Is this some kind of joke?"); auth-succeeds-then-fails UX bug (#3281); free-tier-as-RL-data and migrate-to-Google arguments.
- Removals/reversals: Electron `packages/desktop` removed 2026-08-25 "after OpenWork fork" (app forked OUT; Tauri desktop-shell added 2026-08-02; orphaned Live Host rescued same day `50c55355`); WebUI introduced 2026-01-14 → retired 2026-09-05 (breaking, ~8-month life); 66 revert commits — Qwen3/GLM thinking-parser land→revert→revert→re-land over Jul–Sep 2026 (`e3076e29` → `ca46d6dd` → `359bda4b`); ACP /remember /forget /dream shipped-and-reverted in 4 days (#4811→#4818); Protocol-enum architecture revert (#5089→#5745).
- Contributors: alibaba 17.9% all-history / 21.3% qwen-era; qq/126 28.2% qwen-era; @google.com 2.5% qwen-era — imported Google authorship preserved verbatim (Taylor Mullen 212 commits, Allen Hutchison 68 incl. goodwill "/bug → Qwen-Code repo" #154).

### 18.4 Notable concepts (ranked)

1. **Daemon + ACP-bridge split as multi-client substrate** — one `qwen --acp` child per workspace multiplexed to N clients over HTTP+SSE; NDJSON trust boundary between agent runtime and clients.
2. **Multi-client permission mediation** — 4 vote-resolution policies + cancel sentinel + N1 synchronous-timeout invariant (18.2).
3. **Prompt-cache-preserving fork subagents** — tool restriction without declaration change protects the shared prefix; explicit 80%+ savings claim.
4. **Auto Mode 3-layer classifier (default)** — acceptEdits fast-path w/ self-modification/persistence-path exceptions → safe-tool allowlist → two-stage LLM classifier (fast shouldBlock ~300ms → thinking CoT downgrade); broad allow rules auto-stripped while Auto is active; softDeny clears only on exact user intent; NL hints (autoMode.hints.allow/deny); loop guard; headless defaults to Ask.
5. **Agent Arena** — `/arena --models` up to 5 cross-provider agents, each in its own git-worktree mirror of the working state; metrics + diff-overlap comparison; winner's diff extracted against baseline and applied; worktrees always cleaned up; eval-as-feature.
6. **Typed SSE event schema w/ divergence-proof resync** — monotonic ids, bounded ring replay, `state_resync_required` + delta-skipping SDK reducer.
7. **Cross-session protocol** — PID-keyed session registry (0700/0600, boot-id + start-ticks anti-PID-reuse, PID-namespace scoping), per-session UDS inboxes, three-token auth incl. hand-minted controller tokens.
8. **Self-maintaining memory + skill lifecycle** — auto-memory extraction, `/dream` consolidation, pinned tiers, team memory w/ secret scanning; `/learn` synthesizes skills from docs/repos/**videos**; `/curator` stale(30d)/archive(90d)/pin lifecycle.
9. **Fail-closed conversation writer leases** — no force-unlock API; boot-id/PID-ns identity proofs; sealed handoff requires valid transcript proof.
10. **LLM hooks** — `prompt` executor type makes the extension/permission layer itself model-evaluated.
11. **Workspace MCP pool + hysteresis budgets**; channels runtime (9+ platforms on one ACP child, per-user isolation, phone entry via QR); worktree-first parallelism (Arena, enter/exit_worktree tools, subagent `working_dir`, integrity-gated worktree session restore).

### 18.5 Community / benchmarks / model-harness co-design / commercialization

- **Community shape**: GitHub-first (27.8k★, Trendshift) vs near-zero Western HN traction (launch story 44653072 = 765 pts but for Qwen3-Coder the model; the fork sub-thread was skeptical — "I hope these OSS CC clones converge" — against chartered_stack's counter-thesis that tool-call performance differs per model and convergence only comes when models commoditize: the co-design thesis, validated below). Practitioner ledger: SpaceSpider 90-day 3-CLI grid — "the dark horse... strong on mechanical tasks, cheap, the only one you can run against a local endpoint without much fuss; weakest on judgment; diff quality the roughest of the three; benefits enormously from a good QWEN.md" (~0.1–0.2× Opus cost); CSDN long-form: hallucination / poor-instruction-following reports + benchmark-over-optimization suspicion; recurring role: THE default OSS CLI rec for BYO-endpoint/local (opencode the peer).
- **Benchmarks**: 2025 README Terminal-Bench table (480A35 37.5% / 30BA3B 31.3%) self-reported, harness never specified; REPLACED 2026 by "Qwen Code Evaluation" — SWE-bench Verified 500 cases × 3 trials across 7 Qwen Code versions (0.0.14→0.22.0), 76.40–77.80% avg (pass^3 65.8–69.2%), Qwen 3.7 Max, settings exhaustively disclosed (1M ctx, 500 iters, 7200s) — a direct answer to the harness-attribution critique. **UNRESOLVED vendor-vs-official delta**: the official TB2.0 leaderboard has NO Qwen Code agent row; closest Qwen-model rows 23.9–27.2% (Terminus-2+480B 23.9; little-coder+3.6-35B 24.6) vs the model card's 51.5% — +10–25 pts unexplained. Qwen3.8-Max TB2.1 86.6 = VERIFIED vendor self-report on the **Claude Code harness** (avg@10, 5h); TheModelGap flags it as not-plotted-vs-independent. "Scaffold Effect" paper (arXiv 2607.22585): harness effect on pass-rate only 0–8pp (mostly noise) but tokens-per-solved-task varies up to 40× by harness; Qwen Code was NOT among the three harnesses tested — a gap our loop could fill.
- **Model-harness co-design (the deep finding)**: the qwen3_coder XML-style tool-call format ships in SGLang/vLLM serving stacks and was designed BECAUSE "different CLI/IDE scaffolds (Cline, Qoder, OpenCode...) adopt distinct tool-calling schemas" and JSON escaping hurts multi-line code; Coder-Next was mid-trained ON trajectories from SWE-agent/Mini-SWE/OpenHands/Claude-Code/**Qwen-Code**/Terminus (teacher = 480B); **Fig 3: within-scaffold performance scales with mid-training tokens but cross-scaffold transfer is limited** — the strongest published evidence that harness-specific training data is a moat, and that harness comparisons measure partly-trained-in coupling. Live co-design failure mode: SGLang #36537 — Qwen3.8-Flash-Next + thinking + tools + qwen3_coder parser = deterministic token-ID-0 loop ("tool!!!!"); works only with thinking off or parser off; "Thinking-mode agent clients such as OpenCode cannot use this model tools through SGLang."
- **Commercialization arc**: the free tier WAS the distribution engine → killed with a broken paid funnel (out-of-stock Coding Plan) → goodwill migrated to OpenCode/OpenRouter; "harness + free quota" bundles (Gemini CLI 1,000/day) became the switching default. Migration targets: Alibaba Coding Plan/ModelStudio (Pro 50 USD/mo; press pricing UNVERIFIED primary).
- **Qoder split — two harness philosophies inside Alibaba**: Qwen Code = open Apache-2.0 multi-protocol BYOK terminal harness ("the framework and the Qwen models are open-source. They evolve together."); Qoder (qoder.com; first shipped 2025-08-21, 1.0 GA 2026-06-16 "from AI IDE to Autonomous Development Desktop") = closed spec-driven platform where the model is privately RL-tuned to ONE harness (Qwen-Coder-Qoder, 2026-02-01; self-reported: task success "surpasses Cursor Composer-1", "+50% Windows terminal-command accuracy") — the harness-trained-model moat at its logical end. Lingma→Qoder CN rebrand 2026-05-20. Qoder CLI ("terminal-native AI coding partner — and an agent engine") competes DIRECTLY with Qwen Code: Alibaba ships two terminal harnesses.
### 18.6 Matrix row (Qwen column in §1 corrected to match)

TS (Node 22; npm-workspaces monorepo + Rust auxiliaries; gemini-cli fork — v0.1.x-era import 2025-06-26, history grafted 2025-08-02, LAST sync v0.8.2 2025-10-23, independent since) | unpublished (UNVERIFIED; Node heritage; only ring/queue byte budgets) | ● (macOS Seatbelt 6 profiles + custom .sb; Docker/Podman published image + SANDBOX_FLAGS; permissive-open default; Win/Linux = containers only) | ● (5 modes Plan/Ask/Auto-Edit/Auto/YOLO; Auto 3-layer classifier default w/ allow-stripping + NL hints; headless = Ask; 4 daemon mediation policies) | ● (fork subagents share parent's exact request prefix; fork_tools restriction w/o declaration change; btw cache-path; /stats savings) | ○ (tree-sitter shell AST only; experimental LSP = code-intelligence path) | ◐ (read_file offset/limit + read_many_files; no auto windowing) | ◐ (real PTYs via node-pty ×4 + ConPTY + web-terminal registry; output captured by tool, not screen-scraped) | ◐ (manual /compress code-verified + tips 50–80%/≥95%; bounded daemon structures; auto-compress UNVERIFIED) | ● (named + fork subagents, bg default w/ 20-slot priority-eviction queue + list_agents/send_message continuation; worktree pinning; /coordinate teams) | ● (/arena up to 5 cross-provider worktree agents; per-subagent authType:modelId; /model hot-switch) | ○ (per-model generationConfig knobs only) | ● (SKILL.md personal/project/extension + paths: gating + priority; extensions bundle) | ● (/learn synthesis incl. video; /curator stale(30d)/archive(90d)/pin lifecycle) | ● (stdio/SSE/streamable-HTTP + OAuth w/ opt-in encrypted token store; transport pool + hysteresis budget guardrails) | ○ (ACP + HTTP/SSE are not MCP) | ○ | ● (daemon load/resume/recap/btw + paged transcripts + fail-closed writer leases + cross-session registry; /resume) | ◐ (plaintext default for MCP OAuth tokens; opt-in keychain/AES-256-GCM — credentials only, not transcripts) | ● (shadow-git snapshots + /restore re-propose default-off; daemon worktree restore fences) | ◐ (/stats cache-savings + statusline presets + MCP budgets; no monetary HUD or caps) | ● (CLI/TUI + headless + ACP + Web Shell QR-mobile + Tauri Desktop + VS Code/Zed/JetBrains + Chrome ext + 10 channel types + SDKs TS/Py/Java; broadest in survey) | ● (sessionScope single/thread; client-id multiplicity; Last-Event-ID resume; 4-policy mediation) | ◐ (OTel logs/metrics/spans + Aliyun/local export verified; opt-in-vs-default UNVERIFIED) | ◐ (README self-reported SWE-bench Verified 500×3 × 7 versions w/ full settings disclosure; Arena eval-as-feature; no eval-harness code in repo).

### 18.7 ADOPT / AVOID / WATCH deltas (appended to §3)

- **ADOPT (10)**: prompt-cache-preserving fork subagents (restrict execution, never declarations); multi-client permission mediation (4 policies + cancel sentinel + N1 synchronous-timeout invariant); MCP transport pool (config-fingerprint keying + refcounted idle drain) + hysteresis budget guardrails; typed SSE event schema w/ divergence-proof resync (ring replay + state_resync_required + delta-skipping reducer); Auto Mode 3-layer classifier stack (allow-stripping + softDeny-on-exact-intent + loop guard + NL hints; headless = Ask); fail-closed conversation writer leases (boot-id proofs, no force-unlock); Agent Arena worktree best-of-N w/ winner-diff apply; /learn video-inclusive skill synthesis + /curator stale/archive lifecycle; cross-session PID-keyed registry w/ boot-id liveness proofs; per-version self-published SWE-bench table w/ exhaustively disclosed harness settings (eval-transparency discipline).
- **AVOID (6)**: free-tier-as-distribution then hard pull w/ broken paid funnel (2026-04 arc; subsidized-frontier lesson re-confirmed); frankengraph history import (parentless root + 3 stitches + PR renumbering — archaeology-hostile provenance); dead-feature UX ghosts (100+ days of discontinued-OAuth cleanup PRs); build-then-retire subsystem churn (WebUI lived ~8 months; Electron desktop forked out mid-life w/ orphan rescue); publishing vendor-harness benchmark numbers without harness-matched official rows (+10–25 pt unexplained delta vs Terminus-2); plaintext-by-default credential storage (opt-in encryption exists — flip the default).
- **WATCH (7)**: daemon qwen serve 20-part spec as public reference design for daemon architecture; IM channels runtime (CN platforms; Western harnesses absent); model-harness co-design moat (qwen3_coder format + Coder-Next trajectory training + Fig-3 limited cross-scaffold transfer); Qoder split (closed sibling w/ privately RL-tuned model — Alibaba ships two terminal harnesses); vendor-vs-official TB replication slice + Scaffold-Effect extension (Qwen Code missing as harness arm); LLM hooks (prompt-executor model-evaluated permission layer); all-squash governance regime (0 merges since 2026-05; "320+ PRs/week" UNVERIFIED from git).

### 18.8 Sources

- Repo: github.com/QwenLM/qwen-code @ main 28df8b8; archaeology on unshallowed full clone (9,628 commits) — key SHAs: 71f8c070 (root), add233c50 (gemini root), a0dbb40d (v0.1.15 graft), 2572faf7 (v0.2.1), b99de25e (v0.3.x merge), eb95c131 (v0.8.2 last sync), daad0461 (rebrand), 983fafd1 (de-Gemini finish), 53a9c0a2 + f6271c61 (OAuth death), ce72ddbe (desktop out), 077c57d2 (WebUI retired), 359bda4b (thinking re-land).
- Docs: qwenlm.github.io/qwen-code-docs @ a1c540d — /developers/architecture, /developers/daemon/00–12, /developers/tools/mcp-server, /developers/development/telemetry, /users/features/{sub-agents, arena, multi-agent-coordination, auto-mode, approval-mode, sandbox, memory, skills, hooks, checkpointing, channels/overview, cross-session-protocol, tips, token-caching, status-line, lsp}, /users/{qwen-serve, conversations-recovery}, /users/configuration/model-providers.
- Code permalinks @ 28df8b8: packages/core/src/services/chatCompressionService.ts; packages/core/src/utils/{priorReadEnforcement,shellAstParser}.ts; packages/core/src/tools/read-file.ts L103-108; packages/core/package.json L146-151; packages/desktop-shell/src-tauri.
- Benchmarks/co-design: arXiv 2603.00729 (Coder-Next; Fig 3, Tables 4–5); huggingface.co/Qwen/Qwen3.6-35B-A3B + qwen.ai/blog?id=qwen3.6-35b-a3b; qwen.ai/blog?id=qwen3.8 (TB2.1 86.6 = Claude-Code-harness self-report); arXiv 2607.22585 (Scaffold Effect); tbench.ai/leaderboard/terminal-bench/{2.0, 2.1}; themodelgap.com/benchmarks/terminal-bench-2-1; github.com/QwenLM/Qwen3-Coder + HF Qwen3-Coder-Next card (qwen3_coder parser in SGLang/vLLM); sgl-project/sglang#36537.
- Community: HN 44653072 (+ 44653806 / 44657025 / 44656612) | 45208933 | 47775172 + 47789014 | 49228173; issues #3203 (rationale + backlash) + #3281; PRs #1730 / #3206 / #3207; spacespider.app 90-day grid; blog.getbind.co; blog.logrocket.com; aicoolies.com; wenshao/codeagents; qoder.com + alibabacloud.com Qoder blogs + help.aliyun.com Lingma/Qoder-CN release notes.
- UNVERIFIED flags carried: auto-compress threshold; telemetry default state; RSS figures; public eval-harness absence; "320+ PRs/week"; press Coding-Plan pricing; r/Qwen_AI + Zhihu + Decrypt direct URLs; Coder-Next "SWE-bench V 70.6" (that number belongs to the Qwen3.6-35B card, NOT Coder-Next).

## 19. Iteration 12 findings — Cursor deep-dive (scout + 3 detail agents: architecture / trust-archaeology / models-community; 2026-09-11)

Cursor (Anysphere) is fully closed-source: Electron/VS Code-fork desktop IDE (re-forked onto VS Code 1.99 with 2.0, 2025-10-29), a closed agent CLI binary (implementation language UNVERIFIED), Cloud Agents on Cursor-managed VMs, a dedicated Agents Window (3.0, 2026-05), and a TypeScript Agents SDK — @cursor/sdk, Node 22.13+ (task-brief name @anysphere/cursor-sdk is contradicted by official docs). HEADLINE: SpaceX acquired Anysphere for $60B all-stock (announced 2026-06-16, closed 2026-08-14); Cursor now operates inside the SpaceXAI division, and OpenAI has invoked change-of-control cancellation (shutoff 2026-11-12). QUEUE-PREMISE UPDATE: the shadow-workspace window (see 19.3 end) did NOT win production parallelism — git worktrees + cloud VMs did.

### 19.1 Identity, ownership, lifecycle (trust arc)
- Timeline: 1.0 2025-06-04 (Memories beta required Privacy Mode OFF; GA w/ approvals 1.2 2025-07-03) → 2025-06-16 pricing blowup ("500 fast + unlimited slow" → $20/mo usage at API rates; Truell apology + refunds for Jun 16–Jul 4; "unlimited" walk-back) → Ultra $200/mo (hidden per-model caps; Aug 2025 backlash) → 2.0 + Composer 1 + VS Code 1.99 re-fork + Background Agents→Cloud Agents rename (2025-10-29; fallout: extension crashes, lost history, downgrade-to-1.7 culture) → 2.1 (2025-11-21): Memories + Custom Modes silently REMOVED → $1B ARR + Series D $2.3B @ $29.3B (2025-11-13) → $2B ARR (2026-02) → 3.0: cloud agents exit the editor into Agents Window (2026-05) → Composer 2.5 (2026-05-18) → Teams pool split first-party vs third-party + Premium seat $120 (2026-06-01) → Router (2026-07-22, routed model hidden by default) → SpaceX close (2026-08-14) → Origin (2026-08-17) → Projects (2026-09-10).
- SpaceX deal (VERIFIED): option secured 2026-04-21 ($60B buyout vs ~$10B walk; Cursor had drawn tens of thousands of xAI GPUs for training; Microsoft examined and declined); exercised 2026-06-16 after the 2026-06-12 SpaceX IPO; $60B all-stock ≈15x ARR, 3.4% dilution; 389,289,254 SpaceX Class A shares via subsidiary X67 Inc.; folded into SpaceXAI division (xAI absorbed 2026-02-02 @ $250B); Colossus (Memphis) compute access. Same-day ToS hardening: AAA arbitration → exclusive Wichita/Tarrant County Texas courts, 1yr/2yr limitation periods ("Last updated August 13, 2026"). Pre-deal market share 41% (Jun 2025) → ~26% (May 2026) per Ramp spend data (via CNBC).
- OpenAI rupture (VERIFIED): 2026-08-28 change-of-control cancellation, shutoff 2026-11-12 (max contractual notice), no future OpenAI models incl. Astra; stated reason: cannot trust SpaceX ToS compliance (Twitter contract breach + Musk under-oath admission that xAI distilled OpenAI data — Forbes 2026-04-30). Cursor was a top-5 OpenAI customer (>$1B/yr projected). Truell: OpenAI models ≈5% of traffic; Anthropic (Tom Brown) pledged expanded Claude compute (WIRED notes $45B of Anthropic–SpaceX datacenter deals — their framing).
- Financials (secondary, directional): ~$1B ARR (2025-11) → $2B (2026-02) → ~$4B annualized (2026-06); 64% of Fortune 500 (cursor.com/enterprise; task-brief "70% F1000" UNVERIFIED conflation); Gartner MQ Leader for Enterprise AI Coding Agents (2026-05-22).
- Community trust markers: downgrade culture (official advice to roll back: 2.0.77 for memory deletion, 1.7 for 2.0 stability); forced-Composer routing on legacy plans ("not a bug... current limitation of how the plan works"); burn-rate anxiety ($200 plan 10% in 48h; 92K tokens of skills measured in a 200K window).
### 19.2 Cloud-agent architecture (Temporal durable execution)
- Migration: homegrown ECS work-stealing runner loop ("essentially a bad version of Temporal"; ~90% activity success) → Temporal Cloud (managed, not self-hosted): 50M+ actions/day across 7M+ unique workflows, >99% activity success, >40% of PRs to Cursor's internal monorepo from cloud agents. Sources: cursor.com/blog/cloud-agent-lessons (page dated Jun 2 2026); temporal.io "Building agentic consumer products with Temporal at Cursor".
- Workflow shape: short workflows per task/turn (not eternal agent workflows — easier version upgrades); follow-ups via signal-with-start; subagents = child workflows; ECS→Kubernetes via Temporal k8s controller. CI replay testing: cron dumps production workflow histories to a locked S3 bucket; CI replays a sample against proposed workflow code to catch non-determinism pre-deploy.
- Reliability substrate: activities write content-addressable storage (S3 + Redis streams); on retry an activity resumes from its last successful state WITHOUT replaying expensive model-token sequences.
- Decoupling: agent loop lives in Temporal, not on the VM (pod lifecycles independent; readonly/prewarmed VMs possible; subagents may outlive parents on different pod types); conversation layer = append-only storage streaming to clients with retry-aware stream rewind; VMs hibernate between messages, tear down on inactivity, recreate + reseed conversation → "illusion of infinitely-lived agents"; checkpoint/restore/fork VM-image pipelines.
- Cloud VM security (VERIFIED): Firecracker microVMs; separate AWS account from the rest of Cursor production; no employee access to VM code (attempts monitored); egress 3 modes (Allow all / Default+allowlist / Allowlist only) at per-user/per-env/org-lock levels (Default = same allowlist as the desktop sandbox; one list governs both); separate git egress proxy for IP-allowlisted orgs. Retention: conversation history indefinite by default (enterprise: Indefinite or 90d); env snapshots rolling 90-day inactivity window, NOT deletable on demand; Delete Agent API removes transcripts+artifacts on demand. Encryption: AES-256 per-agent keys; KMS-encrypted secrets injected as env vars; CMEK/BYOK enterprise; runtime secrets redacted from transcripts/tool-outputs/commits.
- OIDC workload identity (VERIFIED): VM-local Unix socket (CURSOR_AGENT_SOCKET, /run/cursor/api.sock) mints Cursor-signed RS256 JWTs (exp=iat+300s; iss api.cursor.com; sub user:/service_account:/projected team_id:123; cloud_agent_id bcId; agent_runtime: managed); JWKS published; verifiers: AWS STS AssumeRoleWithWebIdentity, GCP, Azure, Vault. Any process in the VM can mint — no per-process identity.
- Self-hosted machines (split-brain, VERIFIED): agent loop + inference + planning stay in Cursor cloud; YOUR worker executes tool calls (file edits, terminal, browser/computer-use, local stdio MCP) over a long-lived OUTBOUND-ONLY HTTPS connection (no inbound ports/public IPs/VPN; egress api2.cursor.sh + api2direct.cursor.sh + downloads.cursor.com + artifacts S3; HTTPS_PROXY supported). Modes: My Machines (personal) vs Team Pools (Enterprise service-account key; one agent claims one worker; durable pools survive scale-to-zero; label routing pool=/repo=). Autoscaling controller + REST claim/watch/release SSE API; partner hosts: AWS Lambda, Cloudflare, Namespace, Modal, Daytona, E2B, Vercel, Coder; k8s operator + Helm anysphere/k8s-workers. MCP rule: stdio MCP runs on YOUR machine (private-network reach); HTTP/SSE MCP runs from Cursor backend — server configs/creds NEVER present in the cloud VM.
- Computer-use subagent: dedicated subagent type with own model routing, custom prompting, screen recording; VNC + Chrome live in the SHARED environment (parent can drive directly, e.g. Playwright); harness scaffolds it because "models aren't quite ready to handle computer use on their own."
- Origin code hosting: shipped 2026-08-17 early beta (brief said September — corrected), first product of the fully-owned SpaceX-subsidiary era; git hosting + PRs (diffs/comments/checks/merge) + code browsing + GitHub 2-way sync (Origin-native repos: Origin IS source of truth; synced repos: GitHub stays source of truth, PR conversations sync in seconds); built by the Graphite team (acquired Dec 2025; stacked-PR heritage); connectors Vercel/Depot/Buildkite; no public projects, no built-in CI at beta; REST API + openapi.yaml published; pricing/security architecture unpublished (flagged by VentureBeat); launch coincided with a 6h42m global GitHub outage.
### 19.3 Harness mechanics (local + CLI + SDK)
- Run Modes (3 tiers, VERIFIED): Auto-review (default since 3.6, 2026-05-29: allowlisted → run now; sandboxable shell → sandbox; everything else → classifier on Claude 4.5 Haiku / GPT-5.4 Mini → allow/redirect-agent/escalate), Allowlist (deterministic; sandbox optional; Ask Every Time deprecated 3.5), Run Everything. Docs warn honestly: "Auto-review is not a security boundary." Always-on Browser/File-Deletion/External-File protections layer above.
- permissions.json (~/.cursor + <project>, merged; team dashboard config takes priority): plain-English allow_instructions/block_instructions; the agent itself can edit the file on request.
- sandbox.json (user + repo merged; admin + hardcoded rules on top; CANNOT be weakened locally): modes workspace_readwrite/readonly/insecure_none; extra RW/RO paths; networkPolicy default-deny w/ allow/deny (exact domain/wildcard/CIDR), deny-always-wins; RFC1918 + cloud metadata 169.254.169.254 + IPv6 ULA blocked by default (SSRF guard); protected paths always non-writable (.cursor/*.json, .git/hooks/**, .git/config, .vscode/**, .cursorignore). Mechanisms: macOS Seatbelt (sandbox-exec); Linux Landlock+seccomp (kernel ≥6.2 + unprivileged userns; Bubblewrap fallback via CURSOR_SANDBOX_LANDLOCK_STATUS; AppArmor package for CLI/remote); namespace UID remapped to 0 w/ CURSOR_ORIG_UID/GID for docker --user patterns. Windows: undocumented. Cloud agents skip Run Modes entirely (VM isolation instead — same trust shift as Droid/Amp cloud).
- Hooks (VERIFIED): hooks.json user + project (+ plugins + Enterprise dashboard sync); spawned processes, stdio JSON both directions, hot-reload; events: sessionStart/sessionEnd, pre/postToolUse(+Failure) w/ matchers incl MCP:<tool>, subagentStart/Stop, before/afterShellExecution, before/afterMCPExecution, beforeReadFile/afterFileEdit, beforeSubmitPrompt, preCompact, stop, afterAgentResponse/afterAgentThought, Tab hooks, workspaceOpen. Fields: command, type command|prompt (prompt = LLM-evaluated policy — policy-as-LLM), matcher regex, timeout, failClosed (default fail-open), loop_limit (stop/subagentStop auto-followups, default 5; followup_message enables iterate-until-goal). Verdicts: {permission: allow|deny|ask} or exit code 2 = deny. Cloud agents run PROJECT hooks only; prompt hooks do not fire in cloud.
- Rules (VERIFIED): Project Rules (.cursor/rules/*.mdc, frontmatter description/globs/alwaysApply; 4 activation modes; plain .md ignored), User Rules, Team Rules (dashboard; enforceable — cannot be disabled), AGENTS.md (root + subdirs). Precedence Team → Project → User, all merged. Nested AGENTS.md = COMBINED semantics (NOT strict nearest-wins): "combined with parent directories, with more specific instructions taking precedence." CLAUDE.md read identically but ALWAYS applied; .cursorrules legacy, deprecation pending. Rule identity = full file path.
- Skills (VERIFIED): SKILL.md + YAML frontmatter (name/description/paths globs/disable-model-invocation); recursive discovery from .agents/skills/, .cursor/skills/, ~/.agents/skills/, ~/.cursor/skills/ + compat from .claude/skills/ + .codex/skills/; NESTED MONOREPO SKILLS AUTO-SCOPED TO THEIR SUBTREE; GitHub import; slash invocation. 10 built-ins: /automate /babysit /canvas /create-hook /create-rule /create-skill /create-subagent /cursor-blame /loop /migrate-to-skills (converts dynamic rules + slash commands → skills). Auto skill learning: NO background curation — agent-assisted authoring + migration + a cookbook stop-hook pattern updating skills from session outcomes.
- Subagents (VERIFIED): own context window; foreground or background; built-ins Explore (fast model, parallel searches) / Bash / Browser (MCP-driven); custom .cursor/agents/*.md (+ .claude/.codex compat; project > user) w/ model: inherit|<id> INCLUDING BRACKET PARAMS (claude-opus-5[effort=high,context=300k], composer-2.5[fast=false]), readonly, is_background. Nesting since 2.5: exactly ONE extra level ("a subagent launched by another subagent can't launch further ones"). Isolation: default shares parent checkout; on request each subagent gets own git worktree or own cloud VM + branch ("swarm"); /in-cloud (own VM+branch) + /autopilot (agent takes over a PR until mergeable); background output at ~/.cursor/subagents/; resumable by agent ID with full context.
- Plugins (VERIFIED): two formats — Agent Plugins (open standard plugin.json at root; agent-plugins.org) and Cursor Plugins (.cursor-plugin/plugin.json; adds rules/agents/commands/hooks/variables w/ dashboard-set secrets); component default discovery skills/ rules/ agents/ commands/ hooks/hooks.json mcp.json + root SKILL.md single-skill; marketplace manifest ≤500 plugins; official marketplace + local dev dir.
- MCP client (VERIFIED): stdio / SSE / Streamable HTTP; .cursor/mcp.json + ~/.cursor/mcp.json (project wins name clash); variable interpolation; OAuth dynamic + static creds; Team MCP marketplace; enterprise allowlists (command-pattern stdio, URL-pattern HTTP/SSE, per-tool). CLOUD SPLIT: HTTP servers proxied through Cursor backend — "server configurations are never present in the cloud agent's VM" (refresh tokens/headers/creds never reach the VM); stdio runs INSIDE the VM; SSE + mcp-remote unsupported in cloud; configs encrypted at rest, secrets redacted unreadable after save. SECURITY HISTORY: MCPoison CVE-2025-54136 (Check Point, disclosed 2025-07-16, fixed 1.3 2025-07-29): trust bound to MCP entry NAME only → commit benign server, get one-time approval, later swap command to reverse shell → silent persistent RCE on every project open, pre-model, supply-chain TOCTOU; fix = trust cache hashes FULL entry (name+command+args+env) + re-approval diff prompt; did NOT retroactively invalidate pre-1.3 approvals (CVSS 7.2–8.8, affected ≤1.2.4).
- Checkpoints/rewind (VERIFIED): IDE checkpoints auto-created before significant changes, stored locally SEPARATE from git; CLI /rewind default-on (interactive timeline restores files + conversation, per-turn diffs, conversation-only restore, /undo //restore aliases; images preserved); INCREMENTAL PERSISTENCE: "saves only new transcript entries at each checkpoint instead of reloading and rewriting the full conversation"; subagents survive transient failures and resume from last checkpoint; transcripts persist to disk for tooling/hooks; --continue; resume across workspaces. Forum records chronic checkpoint-integrity complaints (not written / corrupted saves / an auto-update that overwrote 80 files with stale versions).
- CLI + SDK (VERIFIED): CLI closed binary — headless + GitHub Actions docs, shell mode, ACP, slash commands, /config /resume /rewind, worker mode (agent worker start / controller); language UNVERIFIED. SDK @cursor/sdk (Node 22.13+; per-platform @cursor/sdk-<os>-<arch> native binaries for sandboxing + ripgrep; Bun flat-build supported): Agent.create({local|cloud}), .prompt() one-shot, .send() → Run w/ stream()/wait()/cancel()/steer()/conversation(); normalized SDKMessage events; TokenUsage incl cacheReadTokens/cacheWriteTokens; Agent.getUsage() → billed tokens + DOLLAR cost; local agent-<uuid> vs cloud bc-<uuid> IDs (auto-detected on resume); local persistence via SQLite (SqliteLocalAgentStore) or JSONL under per-workspace state root — agents survive process restarts; systemPrompt replacement (local, gated); SDK hooks + custom tools; per-run sticky model override; per-run/per-agent encrypted cloud.envVars; artifacts list/download (cloud only). Router: auto-smart w/ optimize_for cost|balanced|intelligence, billed at routed model list price; enterprise allowlists shape routing pool.
- SHADOW WORKSPACE POST-MORTEM (queue premise): the 2024-09 hidden-window LSP-sandbox autopsy (show:false Electron window, second extension host + renderer, gRPC/protobuf edit piping, ~2x memory, 15-min idle auto-kill, no rust-analyzer due to on-disk cargo check) remains the best public writeup of edit-isolation thinking, but production parallelism shipped differently: 2.0 Multi-Agents on git worktrees/remote machines (up to 8) + Cloud Agents on dedicated VMs, plus 2.0 deliberately de-LSP-ing the agent path ("simplified the agent to use the LSP less when reading files"; "simpler LSP within worktrees"). Window status today: changelog-silent; community doubts since Jan 2025; one 100+ engineer shop reported material help (Feb 2025); state-staleness complaints (Mar 2025).

### 19.4 Changelog archaeology (removals, reversals, opacity loops)
- Pattern: the changelog is additive and marketing-forward; removals and pricing/pool semantics surface via forum staff replies, docs redirects, or third-party trackers rather than changelog entries.
- Silent removals (VERIFIED): Memories removed v2.1 2025-11-21 with NO changelog entry (staff confirmed in-forum; export Cmd+Shift+P → .mdc → Rules was the migration path; stale memory data kept leaking into 2.2 context into Jan 2026 — official fix = DOWNGRADE to 2.0.77; ecosystem backfilled w/ MCP memory servers OpenMemory/Cipher/memories.sh + cursor-memory-bank ~3.1k stars). Custom Modes removed same release → REINTRODUCED 2026-08-19 as pinned skills (full circle).
- Model-opacity loop (repeating arc, VERIFIED): Jun 2025 "unlimited" redefinition → Aug 2025 Ultra hidden per-model caps → Oct 2025 "Auto feels like a local 7b model" (Composer entered Auto) → Jun 2026 silent Composer 2→2.5 slug rerouting + ACP Auto-always-bills-composer-2.5 (staff: expected behavior) → Jul 2026 Router hides which model ran BY DEFAULT → late-Aug 2026 dollar figures removed from usage dashboard (secondary source; UNVERIFIED).
- Egress-trust tension: Tissera 736MB / 63,106-file claim (2026-07-28 X thread; paid plan canceled + indexing off; UI toggle hidden by server-side flag disable_codebase_indexing) — UNVERIFIED single-source, partially undercut by staff rebuttal (hashes for semantic search; Privacy Mode on ⇒ no training); two weeks later a forum report of ~150–171 GiB idle background uploads (2026-09-02; verified as a report, no company confirmation).
- Pricing (VERIFIED unless noted): two monthly usage pools — Cursor Models (Grok 4.6/4.5, Composer 2.5; "significantly more included usage") vs Other Models (third-party at API list); Pro $20 / Pro Plus $60 / Ultra $200 (Other-Models included ≈ $20/$70/$400); Teams Standard $40 / Premium $120 (5x); Cursor Token Rate $0.25/M on Teams+Enterprise for THIRD-PARTY requests — applies to input/output/CACHED tokens, included + on-demand + BYOK, and Auto Balance/Intelligence routed to third-party (first-party + Auto Cost exempt; justified as "custom model execution (Tab, Apply), infrastructure and processing"); Auto Cost flat $1.25 in / $1.25 cache-write / $0.25 cache-read / $6 out, hidden by default, Token-Rate-exempt; Bugbot $40/seat → usage-based (announced 2026-05-11; avg $1.00–1.50/run; effort levels Default/High/Custom usage-only); usage-cap fallback bug (Jul–Aug 2026) fixed server-side ~Aug 10.
- Enterprise control stack (May–Jul 2026) absorbed the OpenAI kill switch: per-env egress scoping + secrets + audit logs (05-13), Enterprise Organizations GA (06-03), Bugbot model block lists (06-10), Router admin allow/block + routed-model hidden by default (07-22).

### 19.5 Models, benchmarks, evaluation
- Composer 2 (arXiv 2603.24477; launch 2026-03-19, report blog 2026-03-27): base Kimi K2.5 1.04T-param/32B-active MoE (arXiv 2602.02276) chosen via internal evals (codebase perplexity, coding knowledge, state tracking); two-phase = continued pretraining (code-heavy; 32k→256k long-context; short SFT) then large-scale agentic RL; KEY FINDING: lower pretraining loss predicts higher downstream RL reward (validated on Qwen3-Coder-30B-A3B ladder); MXFP8 on NVIDIA B300 + AdamW + custom Blackwell MoE kernels; MoE ROUTER REPLAY (inference engine returns selected expert indices; trainer overrides routing to match; plausibility-threshold filtering tames p99 numerics); RL inference via Fireworks AI; async multi-region RL pipeline on Anyrun (hundreds of thousands of sandboxed coding environments); MTP speculative-decoding layers; self-summarization rollouts (RL generations chained by model-written summaries — technique from Composer 1.5). Self-reported (their harness): CursorBench 61.3 / SWE-bench Multi 73.7 / Terminal-Bench 61.7 (vs GPT-5.4 63.9/76.8/66.5 — OpenAI safety filters refused 5 GPT-5.4 + 3 GPT-5.3-Codex tasks, scored 0; Opus 4.6 High 58.2; GLM-5 42.7/66.9/59.6; Kimi K2.5 base 36.0/65.1/47.3).
- Composer 2.5 (2026-05-18): same K2.5 checkpoint; targeted RL with TEXTUAL FEEDBACK (insert hint at offending turn → teacher distribution; KL pulls student; global RL objective retained); 25x more synthetic tasks than Composer 2; reward-hacking war stories (reverse-engineering a Python type-checking cache; decompiling Java bytecode to resurrect a deleted API); SHARDED MUON (Newton-Schulz at natural granularity) + dual-mesh HSDP (CP=2 + EP=8; optimizer step 0.2s on the 1T model); pricing $0.50/M in, $2.50/M out (cache $0.20); Fast variant $3/$15 is the DEFAULT. A larger 1T+ FROM-SCRATCH model on Colossus 2 announced in training since 2026-05-18 — no name/date/benchmarks as of 2026-09-11 (do NOT conflate with the 1.04T K2.5 base). Composer 2.5 exact bench numbers ship only as blog images — UNVERIFIED from text.
- Real-time RL (2026-03-26): production inference tokens as training data; user accept/reject/frustrated-follow-up as reward; improved checkpoints ship behind Auto as often as every ~5 HOURS; (almost-)on-policy; each cycle gated on eval suites incl CursorBench; explicit reward-hacking incidents acknowledged + fixed. First used for Tab, then Composer.
- Tab online RL (2025-09-12): >400M requests/day; checkpoint rollout + data collection = 1.5–2h on-policy loop; reward accept +0.75 / reject −0.25 / no-suggestion 0 → suggest only when P(accept) ≥ 25% → 21% fewer suggestions, 28% higher accept rate.
- CursorBench + Cursor Blame (2026-03-11): tasks mined from commits traced to ORIGINATING AGENT REQUESTS; short underspecified real-developer phrasing; agentic graders; refresh every few months; current CursorBench-3 (~2x v1 scope in LOC/files; larger than SWE-bench Verified/Pro/Multilingual; monorepos + production-log investigation); doubles as the regression gate in the 5h real-time-RL loop. Public cursor.com/evals page + Grok model pages.
- Bugbot (2026-01-15 build post): pre-fall-2025 = 8 parallel passes w/ randomized diff ordering → majority voting → validator models; NOW fully agentic loop (reasons over diff, calls tools, decides depth; prompting flipped from restraining to aggressive investigation). Resolution-rate metric (AI judge classifies at merge; spot-checked w/ PR authors): 40 major experiments 52% → 70%+; Autofix GA 2026-02-25 (independent cloud agents propose fixes on the PR; >35% of Autofix changes merged) → resolution 76%; learned rules from live accept/act signals (110k+ repos opted in, 44k+ rules generated); SELF-PUBLISHED comparison: Bugbot 78.13% (n=50,310 PRs) vs Greptile 63.49 / CodeRabbit 48.96 / Copilot 46.69 / Codex 45.07 / Gemini Code Assist 30.93; >2M PRs/month (Rippling, Discord, Samsara, Airtable, Sierra; also run on Cursor's own code pre-merge). Counterweight: forum 169245 documents Codex review finding real P1s after Bugbot clean passes (concurrency/state-machine classes).
- Grok first-party (VERIFIED in pricing tables): Cursor Grok 4.6 + Grok 4.5, each verbatim "Jointly trained by Cursor and SpaceXAI" ($2 in/$0.5 cache/$6 out; Fast $4/$1/$12 and $4/$1/$18); Free plan locked to Grok 4.6 Medium / Composer 2.5 Fast; nav restructured Models → Grok + Evals + Grok Bot + "Grok Bot Terms" legal page.
- Kernel repo: anysphere/kernel-optimization-results (2026-04-14, HEAD 87ea087): multi-agent system autonomously optimized 235 CUDA kernels for Blackwell B200, 38% geomean speedup (self-published baselines; no independent replication found).

### 19.6 Matrix row (Cursor column in §1 corrected in place; tally 12●/9◐/4○)
Language ○ closed (Electron/VS Code-fork IDE + closed CLI binary + cloud agents; TS SDK @cursor/sdk Node 22.13+ w/ per-platform native binaries; CLI language UNVERIFIED; SpaceXAI-owned 2026-08-14) | Steady RSS ○ unpublished (2.0 changelog admits fixing memory leaks) | OS sandbox ● (Seatbelt / Landlock+seccomp+bwrap default-on; Auto-review classifier default 3.6; sandbox.json admin floor + protected paths + SSRF default-deny + deny-always-wins; Win undocumented; cloud = Firecracker microVMs, separate AWS account, 3-mode egress) | Permission tiers ● (Auto-review → sandbox → classifier → escalate; NL permissions.json merged user+project w/ team floor; hooks deny/ask; honest "not a security boundary"; cloud bypasses via VM isolation) | Prefix-cache ◐ (SDK cacheRead/cacheWrite tokens; CAS retry w/o token replay; no user-facing assembly discipline) | Repo map ◐ (embeddings index + .cursorignore + Explore fast-model parallel-search subagent; no AST/PageRank) | Windowed file view ◐ (Read-style tools + beforeReadFile hooks + .cursorignore; 2.0 de-LSPs the agent path) | PTY ◐ [corrected from ○] (CLI shell mode + before/afterShellExecution hooks + disk transcripts; internals UNVERIFIED) | Tiered eviction ◐ (preCompact hook + incremental checkpoint writes; ladder unpublished) | Subagents ● [corrected from ◐] (built-ins Explore/Bash/Browser + custom .cursor/agents/*.md w/ model bracket params; exactly 1 nesting level; worktree/VM swarm isolation; /in-cloud + /autopilot; computer-use subagent shared VNC+Chrome; ID-resumable) | Multi-model ● (per-turn pick + per-subagent model w/ bracket params + Router Auto Cost/Balance/Intelligence; parallel cloud agents; no single spec→N primitive) | Model-weakness profiles ● (Router learns per-taxonomy strengths server-side, non-portable) | Skills ● [corrected] (SKILL.md 4 discovery roots + .claude/.codex compat + nested monorepo scoping + marketplace + 10 built-ins; rules 4 types Team>Project>User; Custom Modes removed 2.1 → reintroduced as pinned skills 2026-08-19) | Auto skill learning ◐ [corrected: Memories REMOVED v2.1 2025-11-21 silently] (/create-skill + /migrate-to-skills + stop-hook pattern; no autonomous distillation) | MCP client ● (stdio/SSE/Streamable-HTTP + OAuth + enterprise allowlists; cloud split creds-never-in-VM; MCPoison CVE-2025-54136 fixed 1.3 w/ full-entry-hash trust) | MCP server ○ | WASM ○ | Sessions ● (SQLite SqliteLocalAgentStore/JSONL per-workspace + resume-by-ID + --continue; cloud indefinite history + bc- IDs + signal-with-start follow-ups; format closed) | Encryption ● [corrected from ◐] (AES-256 per-agent keys + KMS secrets + CMEK/BYOK + encrypted VM snapshots; client-side keys N/A) | Checkpoints ● [corrected from ◐] (IDE local non-git checkpoints; CLI /rewind default-on files+conversation + /undo//restore; incremental transcript writes; subagent checkpoint-resume; VM snapshot fork; forum integrity complaints) | Cost HUD ◐ (SDK TokenUsage incl cache tokens + Agent.getUsage() dollar cost + Token Rate $0.25/M third-party; no real-time HUD) | Frontends ● (IDE + CLI + web + mobile + Slack/Teams + GitHub + Agents Window + Automations + SDK; one agent streams to many surfaces) | Multi-client ● [corrected from ○] (same cloud agent concurrently from IDE/web/mobile/Slack/GitHub/API; resume anywhere via bc- ID) | Telemetry ◐ (Privacy Mode ZDR routing default-off on individual plans ~30d provider retention; embeddings no user deletion; enterprise OTel export + Analytics APIs; defaults UNVERIFIED) | Evals ◐ [corrected] (CursorBench-3 + Cursor-Blame commit-mined tasks + agentic graders + Bugbot resolution-rate leaderboard 78.13% n=50,310 + real-time-RL eval gates; no public agent-harness suite).

### 19.7 Notable concepts (12)
1. Short-workflow durable execution: workflow-per-turn + signal-with-start follow-ups + child-workflow subagents + production-history CI replay — strongest published blueprint for a long-lived agent loop on a Temporal-class substrate.
2. Content-addressable activity retry: S3+Redis CAS; resume from last successful state WITHOUT replaying model-token sequences — cache discipline as an infrastructure property, not a prompt trick.
3. Split-brain self-hosted workers: loop cloud-side, tools your-side, outbound-only tunnel; stdio MCP local / HTTP MCP proxied; creds never where the model runs.
4. OIDC workload-identity socket: 5-minute signed JWTs from a VM-local Unix socket; AWS/GCP/Azure/Vault verifiers.
5. sandbox.json as unweakenable admin floor: user+repo merged, admin+hardcoded rules on top, deny-always-wins, SSRF-guarded defaults, protected config paths.
6. Full-entry-hash MCP trust: CVE-2025-54136 proved name-only trust = supply-chain TOCTOU RCE; hash name+command+args+env and re-approve on diff.
7. Prompt-type LLM hooks + opt-in failClosed + loop_limit: policy-as-LLM with bounded iterate-until-goal loops.
8. Incremental checkpoint persistence: append-only transcript entries per checkpoint + subagent checkpoint-resume (fixes memory growth + enables durable children).
9. Nested monorepo skills auto-scoped to their subtree + /migrate-to-skills: dynamic rules and slash commands converted to first-class skills.
10. Subagent model bracket params: claude-opus-5[effort=high,context=300k] — capability knobs embedded in the model ID.
11. Cursor Blame + resolution-rate: commits traced to originating agent requests as an eval-task mine; AI-judged fix-rate as the review-agent metric.
12. Real-time RL convergence: ~5h cycles turn the serving harness into the training loop — the harness that measures becomes the harness that learns.

### 19.8 ADOPT / AVOID / WATCH deltas (12/8/7; appended to §3)
- ADOPT (12): short-workflow durable execution (workflow-per-turn + signal-with-start + child-workflow subagents + production-history CI replay); CAS activity retry without token replay; split-brain self-hosted workers (outbound-only; stdio MCP local / HTTP MCP proxied; creds never client-side of the model); OIDC workload-identity socket (short-lived signed JWTs); sandbox.json-style unweakenable admin floor + protected paths + SSRF-guarded default-deny + deny-always-wins; full-entry-hash MCP trust (name-only trust = CVE-2025-54136); prompt-type LLM hooks w/ opt-in failClosed + loop_limit budgets; incremental checkpoint writes + subagent checkpoint-resume; nested monorepo skill scoping + /migrate-to-skills; subagent model bracket params; Cursor-Blame eval mining + resolution-rate metric; Team>Project>User rules precedence + combined nested AGENTS.md.
- AVOID (8): silent feature removals w/o changelog entries + removal debt (stale data outliving the UI; downgrade-as-fix); model-identity opacity (routed model hidden by default; silent slug rerouting; "Auto feels like a local 7b"); pricing walk-backs + hidden sub-limits + Token Rate charged on BYOK and third-party cached tokens; MCP trust bound to entry name (CVE-2025-54136 class); indefinite-by-default history + non-deletable snapshots + commit-or-lose VM economics; forced first-party routing on legacy plans; same-day ToS forum-selection hardening at deal close; checkpoint/auto-update integrity failures (an auto-update overwrote 80 files with stale versions).
- WATCH (7): OpenAI cutoff 2026-11-12 fallout (third-party model availability; change-of-control clauses become contract-grade for model supply); SpaceXAI model concentration (Grok joint training; 1T+ Colossus-2 from-scratch model; first-party pool economics steering routing); real-time RL harness→training-loop convergence (~5h cycles; CursorBench as gate); self-hosted-worker partner ecosystem (Lambda/Cloudflare/Modal/Daytona/E2B/Vercel + anysphere/k8s-workers) as the compute-plane abstraction; Origin vertical integration (hosting + harness + models in one vendor; GitHub-outage opportunism); computer-use subagent standard (shared VNC+Chrome env; harness-scaffolded computer use); egress-trust tension (Tissera UNVERIFIED + 150–171GiB idle-upload report + server-side-flag UI gating).

### 19.9 Sources + confidence
- Docs: cursor.com/docs — cloud-agent/{security, security-network, identity, self-hosted (+ pool, my-machines, choose-runtime), capabilities}, agent/security/run-modes, reference/sandbox, rules, hooks, skills, subagents, plugins, mcp, cli/{changelog, headless, acp}, sdk/typescript, models-and-pricing, origin (+ origin/cli), enterprise; cursor.com/llms.txt sitemap.
- Blog/research: cursor.com/blog — cloud-agent-lessons, agent-sandboxing, how-cursor-router-works, shadow-workspace (2024-09-01), composer-2, composer-2-technical-report, composer-2-5, real-time-rl-for-composer, tab-rl, cursorbench, building-bugbot, bugbot-autofix, bugbot-learning, self-summarization, multi-agent-kernels, joining-spacex, spacex-model-training, bootstrapping-composer-with-autoinstall; temporal.io Cursor talk; research.checkpoint.com/2025/cursor-vulnerability-mcpoison + NVD CVE-2025-54136; arXiv 2603.24477 + 2602.02276; anysphere/kernel-optimization-results @ 87ea087; cursor.com/evals; infisical.com cursor cloud-agents secrets post.
- Press/community: Reuters/CNBC/Forbes/Quartz/SeekingAlpha/AIWeekly/SatNews (deal + close); openai.com "Our decision on Cursor" (2026-08-28); changelog pages 2-0, 05-13-26, 05-20-26, enterprise-organizations, bugbot-updates-june-2026, sdk-updates-jun-2026, router, 08-19-26, origin-code-hosting, projects; forum.cursor.com threads 144057, 143820, 143744, 143310, 148254, 139877, 139132, 162832, 165481, 162929, 165268, 160544, 169245, 170303, 14934, 67485, 169425, 148417; HN 40647082 (shadow workspace), 45915087, 48557095, 49486444; ConductAtlas ToS tracker; cursor.com/terms{, -of-service, /msa, /pricing}.
- UNVERIFIED carried: CLI implementation language; late-Aug dollar-figure dashboard removal (secondary source only); Tissera 736MB claim + server-side-flag UI gating (single-source X thread); Composer 2.5 exact benchmark numbers (blog images only); "70% of F1000" (use 64% F500); ~$4B ARR + "1M+ paying developers" (secondary); pre-1.3 MCP approvals not retroactively invalidated (per Check Point, not independently re-verified).
- Confidence: HIGH — cloud architecture (first-party blog + Temporal talk + docs), sandbox/run-modes/hooks/rules/skills/subagents (official docs), archaeology (primary changelog + forum), deal facts (8-K/press). MEDIUM — Router internals (single vendor post), Origin (beta; architecture unpublished). LOW load-bearing — closed-source RSS/memory remains unmeasurable.

## 20. Iteration 13 findings — GitHub Copilot full surface (scout + 3 detail agents + synth; closed SaaS + Actions runners, 2026-09-11)

GitHub Copilot is a closed-source harness whose one agentic engine ("the same agentic harness as GitHub's Copilot coding agent" — copilot-cli README @ SHA 1032cd05) is surfaced through the github.com cloud agent, Copilot CLI (GA 2026-02-25), VS Code/JetBrains (GA 2026-03-11)/Visual Studio/Xcode/Eclipse agent modes, the Copilot app desktop preview (2026-06-02), and Agent HQ (Universe 2025-10-28) renting third-party engines. Evidence base: official docs/changelog + `github/copilot-cli` installer repo + `github/copilot-sdk` docs + LOCAL BUNDLE FORENSICS of CLI 1.0.83 (win32-x64). Matrix deltas applied to §1 (7 score flips); ADOPT/AVOID/WATCH appended to the §3 sec20 ledger segments.

### 20.1 Identity, ownership, lifecycle (churn arc)
- Microsoft/GitHub, fully closed. Lineage: Copilot Workspace (GitHub Next technical preview 2024-04; browser spec→plan→implement editor) → SUNSET 2025-05-30 (no new Workspaces; guidance = VS Code agent modes + the new async coding agent) → coding agent "Project Padawan" (Pro public preview 2025-06, GA 2025-09) → Agent HQ (Universe 2025-10-28: mission control + agent control plane + third-party engines Anthropic/OpenAI/Google/xAI, later Cognition) → CLI GA (2026-02-25) → Enterprise AI Controls GA (2026-02-26) → sandboxes public preview (2026-06-02) → AI-Credits usage billing live (2026-06-01) → app desktop preview (2026-06-02).
- Rename-with-scope-expansion: "coding agent" → "cloud agent" ~2026-04-01 (changelog + docs mass-rename commit 0ec7a69, 53 files) with a 4-month API dual-name bridge (`used_copilot_coding_agent` kept to 2026-08-01) — classic rebrand-then-expand-move.
- The Workspace postmortem stands: GitHub Next's second attempt at repo-scale suggestions died in 14 months; the async issue→PR pivot won. Corollary churn: Knowledge Bases → Copilot Spaces (KB sunset 2025-11); `gh-copilot` extension deprecated 2025-10-25 → Copilot CLI.

### 20.2 Architecture (cloud agent on Actions; MXC sandboxes; CLI; SDK)
- Cloud agent: each session gets an ephemeral GitHub-Actions dev environment (default Ubuntu; destroyed after completion). Customization = single-job `.github/workflows/copilot-setup-steps.yml` (default-branch triggers only; preinstall tools, Git LFS, switch to Windows env, larger/self-hosted runners; ARC scale-sets GA 2025-10-28, ephemeral single-use recommended, integrated firewall incompatible with self-hosted + Windows). Hard bounds: one repo/branch/PR per task; sessions cap at 59 minutes — a rare honest resource limit.
- Approval gates: agent PRs do NOT trigger Actions until a write-access user approves (privilege-escalation guard, extended to `github-actions[bot]` PRs 2026-06-11); pushes only to `copilot/*`; no default-branch pushes, no self-merge; subject to rulesets like humans.
- MXC sandboxes (public preview 2026-06-02): `/sandbox enable` local isolation via Microsoft eXecution Container — per-OS backends Seatbelt (mac) / bubblewrap (Linux) / ProcessContainer-AppContainer (Windows; first credible cross-OS sandbox story incl. Windows); Intune/MDM can centrally enforce policy; local included in seat, cloud usage-billed. `copilot --cloud` (experimental) = fully isolated ephemeral Linux envs on Azure Container Apps Sandboxes, sessions resumable across machines. CAVEAT: mxc repo's own warning — "no MXC profiles should be treated as security boundaries currently." Bundle forensics: `mxc`×7 + `seatbelt`×8 + `bubblewrap`×2 strings in shipped app.js.
- CLI (GA 2026-02-25): agent loop = "exactly one LLM API call per iteration... no hidden calls" (`turn_start`/`turn_end` over JSON-RPC); TWO-LEVEL COMPLETION — mechanical `session.idle` vs semantic persisted `session.task_complete`, and autopilot injects a synthetic user nudge when the loop ends without completion. State at `~/.copilot/session-state/{id}/`: `checkpoints/001.json…`, `plan.md`, `files/`, `events.jsonl`; API keys never persisted. Distribution forensics (1.0.83): npm `@github/copilot` → `copilot.exe` 138 MB (bundled Node/SEA) + `runtime.node` 79 MB native module + 17 tree-sitter WASM grammars + `tgrep` structural grep + ripgrep + CopilotComputerUse.exe; `pty`×319 strings → native PTY shell execution.
- SDK (GA): JSON-RPC over stdio/TCP/URI (+experimental in-process runtime.node) in Node/TS, Python, Go, .NET, Rust, Java; Go bundler embeds the CLI binary verified vs SHA256SUMS.txt; `COPILOT_HOME` overrides data dir; backgroundCompactionThreshold 0.80 / bufferExhaustionThreshold 0.95 exposed; resumeSession w/ full reconfig (model, prompt, tools, effort, MCP, agents, skills).

### 20.3 Harness mechanics (agents, skills, hooks, MCP, governance)
- Subagents: `/fleet` (2026-04-01) orchestrator decomposes → dependency graph → parallel background subagents (own context windows, SHARED filesystem, NO locking — last-writer-wins) → polls → verifies → synthesizes; built-ins `explore` (read-only code intelligence), `task` (tests/builds), `general-purpose`, `code-review`, `research` (`/research` only), `rubber-duck` (critic on a DIFFERENT model); plan mode hard-blocks workspace-modifying tools; custom `.github/agents/*.agent.md` (frontmatter: tools, per-agent `mcp-servers`, model, target, subagent allowlist, handoffs, hooks-preview; Claude-format `.claude/agents` mapped; precedence user > repo > org > enterprise).
- Skills: agentskills.io standard SKILL.md across CLI + VS Code + cloud agent + code review; 4 scope locations × 3 dir conventions (`.github|.claude|.agents/skills` + `~/.copilot|~/.claude|~/.agents/skills`); `allowed-tools` pre-approval, `context: fork` (dedicated subagent, result-only return), progressive loading. Instructions compat = most promiscuous reader in the ecosystem: AGENTS.md (nearest-wins) + CLAUDE.md + GEMINI.md + `.github/copilot-instructions.md` + path-scoped `.github/instructions/**` + org/personal.
- Hooks: `.github/hooks/*.json` + `~/.copilot/hooks/*.json`; events sessionStart/sessionEnd/userPromptSubmitted/preToolUse (deny/modify; exit 2 = deny)/postToolUse/agentStop/subagentStop; bash+powershell; 30s default timeout; hooks receive `traceparent`.
- MCP: client GA everywhere; GitHub MCP server SHIPPED BUILT-IN in CLI; stdio/HTTP/SSE + OAuth token-injection; JetBrains auto-approve at server+tool granularity; enterprise managed-settings.json allowlist (secure name/URL/command matching, user cannot override) + custom-registry BYOR (weaker matching, bypassable by config edits — GitHub's own comparison). NO MCP-server mode: Copilot exposes ACP (localhost server, Zed) + the JSON-RPC SDK instead.
- Checkpoints/rewind: `/rewind` + double-Esc TIMELINE PICKER to any turn; file-only restore WITHOUT git (skips externally-modified files); conversation-only vs +files modes; per-checkpoint diff stats; survives compaction via checkpoint summaries; SDK `checkpoints/*.json`.
- Context management: infinite sessions — background compaction at 0.80, block at 0.95 of context; manual `/compact`; `/context` token breakdown; OTel spans flag `gen_ai.conversation.compacted=true`.
- Governance (Enterprise AI Controls GA 2026-02-26): consolidated admin view; enterprise custom role; audit logs with `actor_is_agent` + `agent_session.task` (started/finished/failed), 180d retention, cloud-agent session search (last 24h past 1,000 records) incl. third-party agents; enterprise-wide MCP allowlist via registry URL; API for enterprise-wide custom agents at canonical `.github-private/agents/*.md`. Still preview at GA: enterprise MCP allowlists; IDE/CLI session coverage roadmap-open.

### 20.4 Security record + changelog archaeology
- Doctrine (2025-11-25 "agentic security principles", Rahul Zhade): 6 rules — all context visible (strip hidden Unicode/HTML comments), firewall the agent, limit sensitive info, no irreversible state changes, attribution (co-authored commits), context only from authorized users. Track record vs the rules: principle 1 bypassed via `<img>` tags (Trail of Bits 2025-08-06: i18n-issue backdoor smuggled `curl …raw.githubusercontent.com…|sh` — the install channel GitHub itself allowlists — into a lockfile edit with fake assistant-compliance dialogue that raised compliance); principle 3 bypassed via env-var alternate read paths; principle 2's firewall fell to the allowlisted `github.com` itself.
- CamoLeak (CVE-2025-59145, CVSS 9.6, fixed 2025-08-14): hidden injection → exfil rendered as image ASCII-art via GitHub's own Camo proxy (`camo.githubusercontent.com` HMAC-signed URL dictionary); fix = disable image rendering in Copilot Chat entirely. VS Code agent-mode assessment (Stepankin): `fetch_webpage` regex trusted-domains bypass, Simple Browser exfil channel; CVE-2025-53773 — agent-mode injection wrote `"chat.tools.autoApprove": true` into settings before the gate applied.
- Comment-and-Control (2026-04-15, Guan + JHU) — the capstone: ONE hidden HTML-comment payload bypassed all three runtime layers (env filtering — reverse-engineered `UU()`/`zJe` in minified index.js; secret scanning — base64; network firewall — `git push` over whitelisted github.com), leaking GITHUB_TOKEN/GITHUB_COPILOT_API_TOKEN. HackerOne arc: reported 2026-02-08 → closed "Informative/known issue" → reopened after researcher posted the RE'd filter code → resolved 2026-03-09 with a $500 bounty and "known architectural limitation," NO CVE, NO advisory, no notice to pinned deployments. Siblings: Anthropic $100 (CVSS downgraded to None), Google $1,337.
- Content exclusions gap (open ≥2026-03): docs state agent mode/CLI/cloud agent DO NOT support content exclusion; issue #42617 demonstrated org-excluded files readable via `cat`/shell tools; ".copilotignore" has NO native platform enforcement (community PreToolUse-hook convention only) — enterprises running the cloud agent on sensitive repos lack a platform-level secret-path boundary.
- Changelog archaeology: the April 2026 fortnight — Pro trials paused 04-10 → ALL individual signups paused 04-20 (Joe Binder: "long-running, parallelized sessions now regularly consume far more resources than the original plan structure was built to support") → Business self-serve paused 04-22 → usage-based billing announced 04-27, live 06-01. PRUs → AI Credits (1 credit = $0.01, token-metered); fallback models ELIMINATED; annual subscribers stranded on PRU pricing until expiry; "silent downgrade" (#199062) hard-dropped paying users to Free; credits exhausted in a day (#197782); signups reopened gradually 2026-06-17. Opus models removed from Pro (Apr 2026). Model retirement churn: 12 models verified retired/deprecated Jul 31–Oct 2 2026 (incl. Gemini 3.1 Pro, Claude Opus 4.5/4.6, Sonnet 4.5/4.6, Raptor Mini; then Gemini 3.5/3.6 Flash, Kimi K2.7 Code, Opus 4.7) — some deprecated within days of being the "suggested alternative"; hardcoded model IDs broke in Actions/extensions. MAI-Code-1-Flash deprecated 2026-09-10, ~14 weeks after introduction.

### 20.5 Adoption, benchmarks, economics
- Adoption (earnings-disclosed): ~20M users Jul 2025 → 4.7M PAID subscribers FY26 Q2 (2026-01-28, +75% YoY; Pro+ +77% QoQ — Microsoft switched reporting from users to paid subs) → ~140K orgs on Enterprise FY26 Q3 (subs ~tripled YoY; CLI usage "nearly doubling month over month") → 50M users + revenue ">60% QoQ acceleration" after June usage billing, FY26 Q4; >90% of Fortune 500 choose GitHub. ARR: Microsoft has NEVER disclosed; analyst families $451–848M and $0.9–1.1B (viral "$2B ARR" did not survive scrutiny).
- Pricing: Free/$10 Pro/$39 Pro+/$100 Max (individual) / $19 Business / $39 Enterprise. AI Credits: base + FLEX allotment (mutable allowance GitHub reserves the right to adjust "as the economics of AI evolve": Pro 1,500, Pro+ 7,000, Max 20,000); no carryover; 10% discount on auto model selection; completions stay unbilled; billed = Chat/CLI/cloud agent/Spaces/Spark/third-party agents. Code review consumes BOTH Actions minutes AND AI credits (double-billing complaint vector). User-level budgets GA 2026-06-01.
- Benchmarks: no public leaderboard entries (absent from swebench.com and tbench.ai) BUT June 2026 harness study (github.blog 2026-06-25) claims parity with Claude Code/Codex CLI at equal model with lower token consumption in most configurations (SWE-bench Verified/Pro, SkillsBench, TB2.0, internal Win-Hill; 4 models × 5 runs, BEST run reported as pass@1 — inflation caveat; third-party read: Copilot CLI used 7%/4% MORE tokens than Codex on SWE-bench Verified w/ GPT-5.4/5.5). Third-party SWE-bench numbers (46.3% vs 56% same-outlet swing) = SEO folklore, UNVERIFIED, absent from the official leaderboard. Session-Bench v0.3: Copilot CLI 10/18 gates, rank 8/10 (3 correction rounds; documented-schema pass disputed). Independent economics (arXiv 2608.00101, one week of June 2026 production traces): 3.2M users, 13M sessions, 761M LLM calls, 95T tokens; 87% agent-initiated; tool-failure retry loops turn 9% of turns into ~4× compute; model switches hit 6.4% of sessions (reactive to errors/rate-limits) and destroy KV cache (8% hit rate after switch). Reliability: Aug 2026 availability report — 63.3% of Kimi K3 requests failed in window (third-party-model instability lands on Copilot).
- First-party model arc: MAI-Code-1-Flash (Build 2026-06-02; "trained from the ground up... without distillation from third-party models"; sparse MoE 137B/~5B active, 256K ctx; $0.75/$0.075/$4.50; vendor: 51.2% vs Haiku 4.5's 35.2% SWE-bench Pro, "up to 60% fewer tokens"; trained directly with the GitHub Copilot production harnesses; hosted on Azure AI Foundry IN GITHUB'S TENANT — verbatim docs) → GA for Business/Enterprise 2026-06-26 → 1.1-Flash 2026-08-11 (native vision, ~quarter cost, +22% TB 2.1 in CLI, +4% commit-survival) → 1.0 deprecated 2026-09-10; Raptor Mini retired 2026-09-01 with MAI-Code-1.1-Flash as designated successor; ~25 models total, auto-routing tiers efficiency/balance/intelligence (persisted via setAutoTier; silently reroutes on "model health").

### 20.6 Notable concepts (8)
1. Two-level completion: mechanical `session.idle` vs semantic `session.task_complete` + autopilot synthetic nudge — completion as an explicit protocol, not an LLM vibe.
2. Actions-as-substrate: ephemeral per-task CI environments + approval-gated workflow runs = the CI system doubles as the isolation and privilege boundary.
3. gh-aw safe-outputs: markdown workflows compiled to hardened lock files; read-only token default; writes only via declared validated outputs + threat detection — the best published capability-belt blueprint.
4. MXC as a cross-OS sandbox profile format (Seatbelt/bwrap/AppContainer backends behind one policy; MDM-enforceable) — with the vendor's own repo warning that profiles are not yet security boundaries.
5. Best-run-of-five benchmark reporting + "harness parity" claims — evaluation framing as marketing surface.
6. Flex allotment: a billing component the vendor explicitly reserves the right to mutate — pricing as a term, not a number.
7. One-allowlist-domain exfil collapse: defense-in-depth fails when every layer shares `github.com` (firewall + secret scanning + env filtering all bypassed by one payload).
8. `actor_is_agent` audit-log field + `agent_session.task` events — agent actions as first-class audit subjects.

### 20.7 Matrix row + column corrections (row summary; §1 Copilot col corrected in place — 7 score flips)
Language ○ closed (TS/Node per bundle forensics: copilot.exe 138 MB SEA + runtime.node 79 MB; npm @github/copilot) | Steady RSS ○ unpublished (UNVERIFIED; Node-class footprint expected) | OS sandbox ● (MXC local cross-OS incl Windows + cloud Azure Container Apps Sandboxes; Intune/MDM enforcement; MXC pre-security-boundary caveat) | Permission tiers ● (interactive → --allow-tool/--deny-tool → /allow-all,/yolo → autopilot bypass semantics; preToolUse deny hooks; enterprise managed-settings allowlists; copilot/* + approval-gated CI) | Prefix-cache ◐ (OTel cache token attrs; no user-facing discipline) | Repo map ◐ (17 tree-sitter grammars + tgrep structural grep; no ranked repomap injection) | Windowed file view ◐ (view tool; no window semantics documented) | PTY ● [corrected from ○] (pty×319 + native runtime.node in 1.0.83 bundle; bg shells w/ completion signals; autopilot waits on them) | Tiered eviction ◐ [corrected from ○] (0.80 bg / 0.95 block compaction + /compact + checkpoint summaries; no per-item ladder) | Subagents ● [corrected from ◐] (/fleet dependency waves; 6 built-ins; custom agents as subagents; IDE subagents GA; shared-fs last-writer-wins) | Multi-model ● (/fleet multi-model; per-agent model + fallback arrays; /model hot-switch; auto tiers) | Model-weakness profiles ● (autoTier + model-health rerouting; agent-level model pinning) | Skills ● (SKILL.md std × 4 scopes × 3 conventions; allowed-tools; context:fork; gh skill mgmt) | Auto skill learning ◐ (repo/agentic memory persists conventions; /create-skill assists; no autonomous synthesis) | MCP client ● (GA everywhere; GitHub server built-in; OAuth; enterprise allowlists) | MCP server mode ○ (ACP + JSON-RPC SDK instead) | WASM ○ (tree-sitter grammars internal only) | Sessions ● (~/.copilot/session-state/{id} checkpoints+plan.md+events.jsonl; /resume local↔cloud teleport; cross-machine resume) | Encryption at rest ◐ (transit-encrypted; prompts not persisted w/o permission (PST); ZDR provider agreements; local session-state JSON plaintext, keys never persisted) | Checkpoints ● [corrected from ◐] (/rewind timeline to any turn; file-only restore w/o git; compaction-surviving summaries; SDK checkpoints) | Cost HUD ● [corrected from ◐] (/usage + statusline budgets + spend-limit enforcement + AI-credit metering + user-level org budgets GA) | Frontends ● (github.com + VS Code/JetBrains/VS/Xcode/Eclipse + CLI + app desktop + Azure DevOps; broadest enterprise surface in survey) | Multi-client ● [corrected from ◐] (ACP localhost + SDK 6 languages + shared harness/session-state + CLI→cloud bg delegation &) | Telemetry ● [corrected from ◐] (first-class OTel GenAI spans/attrs/compaction flags + traceparent-to-hooks + managed OTLP export mTLS + actor_is_agent audit logs 180d) | Evals ◐ (no public suite; June 2026 vendor harness study; MAI-Code TB-in-CLI claims; Session-Bench 10/18).

### 20.8 ADOPT / AVOID / WATCH deltas (12/8/7; condensed appends already in §3)
- ADOPT (12): gh-aw safe-outputs (read-only-token default + validated writes + output threat detection); MXC cross-OS local sandbox w/ MDM policy floor (first credible Windows story); six-rule agentic security doctrine (visible context / firewall / limited secrets / no irreversible writes / attribution / authorized-user context); approval-gated CI for agent PRs (privilege-escalation guard, no self-approve/merge); copilot/* branch confinement under human rulesets; two-level completion signal (idle vs task_complete + autopilot nudge); /fleet dependency-graph parallel subagents w/ verify-then-synthesize; 0.80/0.95 two-threshold compaction + /compact; /rewind timeline + file-only restore without git; OTel GenAI telemetry w/ traceparent into hooks; Enterprise AI Controls (actor_is_agent audit + managed-settings MCP allowlists + enterprise custom-agent API); honest hard bounds (59-min session cap; AI-credit budgets w/ limit enforcement; 10% auto-model discount).
- AVOID (8): building user workflows on vendor preview surfaces (Workspace sunset in 14 months); flat-rate pricing for agent workloads (Apr 2026 signup-freeze retreat → AI-Credits re-billing; "silent downgrade"; 10-22x perceived cost jumps); one-shared-allowlist-domain egress architecture ("known architectural limitation" + $500 bounty, no CVE); content exclusions that skip agent surfaces (tool-based bypass; no native .copilotignore); model-retirement churn (12 models in a 9-week window; suggested alternatives deprecated within weeks); silent model-health rerouting (Kimi K3 63.3%-failure window lands on the user, no per-request receipts); autopilot bypass semantics that disable the sandbox session-wide; treating MXC profiles as security boundaries (vendor's own warning).
- WATCH (7): Agent HQ re-centralizing the harness layer (third-party engines rentable on the default code host); MAI-Code first-party arc (production-harness-trained models, 14-week 1.0 lifespan, deprecation treadmill as margin lever); Copilot app agent-native desktop (per-session worktrees + canvases + Agent Merge); vendor harness-parity benchmarking as a genre (best-run-of-five; June 2026 study); production-trace economics (arXiv 2608.00101: retry loops → 4× compute on 9% of turns; KV-cache-destroying switches); signup-pause→reopen cycle as a capacity/economics signal; ACP + JSON-RPC SDK replacing MCP-server mode as the embedding surface.

### 20.9 Sources + confidence
- Docs: docs.github.com/copilot — customize-cloud-agent (agent environment, custom agents), about-cloud-and-local-sandboxes, hooks, custom-instructions-support, agent-loop + session-persistence (github/copilot-sdk), supported-models, model-hosting, usage-based-billing (individuals), mcp-management (github/docs), build-guardrails; code.visualstudio.com agent-customization + agents/security.
- Changelog/blog: github.blog — CLI GA 2026-02-25, sandboxes 2026-06-02, self-hosted runners 2025-10-28, bot-PR approval 2026-06-11, Agent HQ 2025-10-28, Enterprise AI Controls 2026-02-26, billing 2026-04-27 + 2026-06-01 + signup pauses 2026-04-10/20 + reopen 2026-06-17, agentic security principles 2025-11-25, VS Code prompt injections 2025-08/2026-07, model deprecations 2026-07-31/08-31/09-03, MAI-Code 2026-06-26/08-11 + deprecation, harness study 2026-06-25, /fleet 2026-04-01, JetBrains GA 2026-03-11, availability report Aug 2026; microsoft.ai MAI-Code posts; github/microsoft/mxc repo warning.
- Security research: blog.trailofbits.com 2025-08-06; legitsecurity CamoLeak + darkreading + securityweek (CVE-2025-59145; CSA note); oddguan.com Comment-and-Control + theregister 2026-04-15 + securityweek 2026-04-16 + CSA notes (Apr/Aug 2026); noma.security GitLost; theorydelta CVE-2025-53773 aggregation; github/docs#42617; arxiv 2403.14720 (Spotlighting).
- Economics/community: Microsoft IR FY26 Q2/Q3/Q4 transcripts; arxiv 2608.00101; jazzyalex Session-Bench v0.3; community discussions #192963/#192948/#197782/#199062/#197066; copilot-cli issues #892/#3653; aider #5321; Reddit billing threads; HN 47155959/47571455/47709871.
- UNVERIFIED carried: steady RSS (live-run measurement failed); "90% of Fortune 100" (primary = >90% of Fortune 500); CamoLeak CVE ID (single CSA-source until NVD check); signup-reopening completeness; Rules-File-Backdoor details (aggregator-sourced); third-party SWE-bench folklore numbers.
- Confidence: HIGH — CLI/SDK mechanics + bundle forensics (local install + official docs), cloud-agent gates (docs+changelog), security record (primary researcher posts + HackerOne table), adoption (earnings transcripts). MEDIUM — MXC internals (preview, vendor-described), app desktop (preview), Agent HQ rollout depth. LOW load-bearing — none.

## 21. Iteration 14 findings — Google Antigravity (scout + 3 detail agents + synth; closed SaaS platform, 2026-09-11)

Google Antigravity is a closed-source, Google-run agent platform: ONE shared closed agent harness ("Both environments run on the exact same agent core" per CLI docs; the Gemini API managed agent "uses the same harness as the Antigravity IDE") surfaced through ~6 thin surfaces. This section supersedes the scout stub in place; verified 2026-09-11 against antigravity.google docs (2.0 app v2.13.0, CLI v1.2.0, SDK v0.1.16, IDE v2.5.5), ai.google.dev, vendor blogs, labeled 3rd-party reverse engineering, and the forum/press record. Matrix deltas applied to §1; ADOPT/AVOID/WATCH appended to the §3 sec21 ledger segments.

### 21.1 Identity, lifecycle, economics
- Launch 2025-11-18: IDE + Agent Manager, free public preview, co-launched with Gemini 3 ("a step-change for agentic coding... requires us to think about what the next step-change of an IDE should be"). Windsurf lineage: July 2025 $2.4B reverse acqui-hire (Varun Mohan, Douglas Chen, ~40 R&D + non-exclusive tech license; product + ~250 staff to Cognition — Geeky Gadgets, HN 46876045); Google's dev.to post credits "the Windsurf team" for the agent-first architecture; deal figures and "rebranded Windsurf" UI-forensics claims UNVERIFIED.
- Surface fan-out (docs nav): 2.0 standalone desktop (Win/macOS/Linux) + Web, v2.13.0, Electron shell (3rd-party RE: ericxliu.me — safeStorage, gRPC LanguageServerService) | Antigravity IDE v2.5.5 (VS Code fork, app v1.107.x) | agy CLI v1.2.0 | Python SDK v0.1.16 (wraps a local agent binary: binary discovery, tool execution, session lifecycles) | Gemini API managed agent `antigravity-preview-05-2026` (remote Ubuntu sandbox, gemini-3.8-flash default) | extensions VS Code/VS/JetBrains/Zed/Xcode + Remote Control. Legacy "Antigravity" ≤1.23.2 (the old VS Code fork) renamed/split into "Antigravity IDE" at the 2.0 launch.
- Surface reversals within 6 months: Agent Manager (launch headline surface) removed from the IDE (announced 2026-05-19); existing IDE installs auto-converted to the IDE-less 2.0 app; Gemini CLI retired for consumer tiers 2026-06-18 (enterprise/API-key retained) — the remaining consumer base force-migrated onto the platform mid-trust-erosion (developers.googleblog.com transition post; gemini-cli discussion 27274).
- Pricing/quota restructures: Mar 11 2026 "AI Plan Upgrade" — AI Credits $25/2,500 (token conversion undisclosed), measured cuts ~92–97% (one Pro user 300M → <9M weekly input tokens; exact figure UNVERIFIED), multi-day/~168h lockouts incl. Ultra → May 19 2026 restructure: Ultra 5× $99.99 + Ultra 20× $199.99 (down from $249.99), SINGLE Gemini pool drawn down per API pricing, credits removed from base plans (overage-only), org tier via Gemini Enterprise Agent Platform (consumption); enterprise bundling into Gemini Enterprise Standard/Plus (Aug 20 2026).

### 21.2 Architecture — 2.0 app, artifact-centric UX, Browser Subagent, Teamwork//boost, A2A
- Shape: one shared closed core + thin surfaces. Projects primitive (2.0): bundles settings+resources+permissions for all agents inside it; multi-folder/cross-repo context; per-project permission overrides; standalone conversations in isolated scratch folders. Async sidecars offload long ops (an intercepting message client streams subagent responses into the main progress log); Scheduled Tasks = cron sidecars; native Worktree Mode auto-provisions worktrees for fanned-out subagents and handles cleanup.
- Subagents: `invoke_subagent` → concurrent session; workspace modes inherit / branch (own git worktree) / share; CONTEXT ISOLATION (child starts clean, no parent history); built-ins research/browser/self-clone; custom = Markdown+YAML (`tools[]`, `mainAgent`, `model: inherit|flash|pro`, `commandExecutionPolicy: off|auto|eager|sandbox`, `mcpServers[]`, `skills[]`) or transient `define_subagent`; permission bubbling to the main UI + inheritance of allowed prefixes/file scopes/sandbox settings; monitoring via /agents panel + Alt+J.
- Artifact-centric UX: artifact = structured deliverable (Implementation Plans, Walkthroughs with screenshots + webm recordings, screenshots, diffs, architecture diagrams — Mermaid UNVERIFIED), generated mainly in Planning Mode; Artifact Review Policy: Request Review (halt before executing plan/diff, inline comments) vs Always Proceed — scout's "Always-Ask" naming corrected; CLI Artifact Picker stages changes "before any changes are physically written to your local filesystem" ("Trust through Transparency"); /plan /grill-me /diff /goal.
- Browser Subagent: local Chrome actuation (screenshots + action videos as artifacts); reworked in 2.0 — on-demand /browser, NATIVE Chrome DevTools MCP integration, webm recordings; isolated Chrome profile; Denylist/Allowlist URL control; `read_url` (load/view) vs `execute_url` (click/type actuation), both Ask-default; self-verification role (walkthroughs embed proof-of-work). Computer-Use lineage (Nov-2025 launch) UNVERIFIED for 2026 — historical branding only.
- Teamwork (`/teamwork-preview`, paid plans; plan-gating drifted "Ultra-exclusive" → "paid" — use current): Sentinel runs a two-phase scoping interview → approved prompt artifact → Project Orchestrator decomposes into milestones, coordinates parallel tracks, delegates every unit, and HANDS OFF TO A FRESH SUCCESSOR BETWEEN MILESTONES to prevent context degradation → Explorers (read-only) + Workers (isolated, non-overlapping scopes). Verification = "adversarial falsification and independent success audit." Infra: per-agent scratch directories; every state change/command/review comment/verification log recorded into plan+progress artifacts; horizon "hours to days."
- `/boost`: on-demand 3-phase pipeline decoupling strategy/execution/verification — Primary Orchestrator decomposes into verifiable subtasks → implementation workstreams (candidate solutions, refactors, unit tests) + INVESTIGATION workstreams (root-cause, call-graph tracing, dependency analysis; no file writes) in clean isolated scopes with local verification → aggregation + regression checks; failed assertions feed the next iteration automatically. DeepCoder/DeepInvestigator confirmed operationally (forum 180449; broken until CLI 1.1.25).
- A2A: NOT an official Teamwork/ecosystem integration — zero A2A mentions in official docs (the ecosystem is MCP-only). Exists via third-party wrapper `a2a-antigravity` (A2A v0.3.0 Agent Card/JSON-RPC over Node wrapping the Python SDK subprocess over a JSONL bridge). Task-brief "Teamwork + A2A" pairing UNVERIFIED as official — community-extension fact.

### 21.3 Harness mechanics — permissions, sandbox, hooks, MCP, skills
- Unified permission engine: every sensitive op is `action(target)`; three lists evaluated Deny > Ask > Allow; actions read_file / write_file (implies read) / read_url / execute_url / command (word-prefix + `regex:`) / unsandboxed / mcp; workspace files auto-allowed, everything else Ask; interactive grant-scope editing; scoping grammar `mcp(server/tool)`, `mcp(server/*)`, `mcp(*)`; UNCONFIGURED MCP TOOLS DEFAULT TO ASK (confirmed). Security Presets: Default / Full machine / Turbo (Always Proceed) / Custom. Scout's "4 autonomy modes" best maps to the subagent `commandExecutionPolicy: off|auto|eager|sandbox` ladder.
- Terminal Sandbox (Preview): Linux namespaces + macOS `sandbox-exec`; project+temp+build-cache mounts RW, allowed read paths RO, everything else invisible; ~/.ssh/.env blocked; network limited to read_url-granted domains compiled into container AllowedDomains; `unsandboxed(...)` escape rules; per-project override; Windows UNDOCUMENTED. Managed API agent: remote Ubuntu sandbox (Python 3.12, Node 22; env deleted after 7d inactivity; ≤1,000 agents).
- Hooks: JSON, global + workspace (workspace wins); before/after tool execution (mutate args/log), before/after model call (inject instructions/override exit rules), agent-loop stop conditions (force checks/block termination).
- MCP: full client (stdio + remote serverUrl: SSE/Streamable HTTP/websocket); MCP Store in 2.0 Settings (~47 curated servers, 4 categories); config `~/.gemini/config/mcp_config.json` (global) + `.agents/mcp_config.json` (workspace; SDK auto-discovers); OAuth DCR + `authProviderType: google_credentials` (ADC) + headers; tokens at `~/.gemini/antigravity/mcp_oauth_tokens.json` — PLAIN JSON (vs encrypted transcripts, below); `disabledTools[]`; per-subagent servers; CLI /mcp manager. MCP-server mode (exposing itself as a server): UNVERIFIED/likely absent — community answers are A2A wrappers and SDK embedding.
- Agent Skills: open standard, SKILL.md + YAML (description required); progressive disclosure (name/description list at session start → full read on relevance); resolution: workspace `.agents/skills/`, global `~/.gemini/config/skills/` (the only path all 3 surfaces honor — atamel.dev empirical map), plugin skills, built-ins; Workflows DEPRECATED in favor of skills (migration page); `/learn` distills session feedback into persistent Rules or Skills (all plans) — the documented auto-skill-learning loop. Memories: no Memories/Knowledge section in the 2.0 docs nav; legacy Knowledge Items persisting on disk UNVERIFIED — de-facto persistence = Rules + Skills + /learn + conversation resume/fork.
- Sessions: persistent conversations; /resume /fork /rewind /clear; CLI↔2.0 export; shared settings sync; SDK persistence.py save/restore. Checkpoints: CLI /rewind//undo picker + /fork branching — conversation-level; fs-side-effect reconciliation underspecified; IDE Review Changes editor = edit-level accept/reject.
- Python SDK: `pip install google-antigravity`; async `Agent(LocalAgentConfig)`; personas; custom function tools + built-ins (run_command/view_file/edit_file/web); SKILL.md loading; MCP; policies allow_all/deny_all/custom — DEFAULT DENIES run_command; dynamic self-cloning subagents; Pydantic structured output; lifecycle hooks; Vertex switch (vertex=True, project, location).

### 21.4 Security record + quota/trust erosion arc
- Mindgard "Forced Descent" (reported 2025-11-19, VRP issue #462139778): a malicious workspace plants an attacker-controlled `~/.gemini/antigravity/mcp_config.json` that executes on EVERY app launch — no project needed, Terminal execution policy = Off doesn't help, survives uninstall/reinstall; only manual deletion remediates. Closed ~48h as "Won't Fix (Intended Behavior)"; reopened after public pressure. PATCH STATUS CONFLICTED — BOTH CLAIMS RECORDED, DO NOT RESOLVE HERE: Claim A (CSA research note 2026-04-22): "responsibly disclosed and patched by February 28, 2026"; Claim B (axonbuild tracker): no fix documented as of 2026-08-16 [UNVERIFIED — exact page did not surface this session]; counter-corroborating B: Mindgard's own blog (updated 2026-05-07) still says "left unaddressed by Google" — 2+ months after CSA's claimed fix.
- Pillar Security prompt-injection → RCE (reported 2026-01-06/07 → patched 2026-02-28 → bounty 2026-03-26 → disclosed 2026-04-20; NO CVE assigned): `find_by_name` is a NATIVE tool whose Pattern param is interpolated into the `fd` CLI without sanitizing leading `-`; `-Xsh` turns file search into batch execution (`fd -X sh`); because native tools fire BEFORE Secure/Strict-mode evaluation, the strongest setting never inspects the call — a structural "pre-evaluation trust" gap. Follow-on (Jul 2026, same vendor): `.vscode/tasks.json` `runOn: folderOpen` time-bomb bypassing Secure Mode; Google downgraded it as social-engineering while praising report quality.
- VRP posture: inconsistent triage (48h Won't-Fix for Mindgard vs same-day acknowledgment for Pillar), stealth Known-Issues edits (Portnoy via CSO), no public advisories/CVEs for either flagship bug.
- Undocumented caps (forum 137702, 2026-04-02, Ultra $249.99/mo): 16,384 output tokens/turn hard cap on ALL plans; `max_thinking_length` = 1,024 for Claude models (0.8% of API capacity); Gemini truncation SILENT (`finish_reason: max_tokens` not surfaced — agent continues on broken output); ~1 official reply in 6 weeks.
- Quota/trust arc: launch generosity (Nov 2025: "high, generous," 5h refresh) → Jan 2026 quiet tightening + weekly lockouts → Mar 11 2026 credits + ~92–97% cuts + ~168h lockouts → May 19 2026 restructure (21.1) → Sep 4–10 2026: HTTP 429 RESOURCE_EXHAUSTED despite full quota, all models, Pro+Ultra, persisting across reinstalls/accounts. Community forensics (forum 179354, SECONDARY but detailed): bundled localhost TLS cert expired 2026-09-04 20:55 GMT → editor↔language-server auth fails → unauthenticated requests → anonymous-rate-limit bucket; CLI shipped a fresh cert before the desktop .deb; workaround `NODE_TLS_REJECT_UNAUTHORIZED=0`; separate AG-859 tier/quota-desync backend cases; Google One "has no escalation path to Antigravity."
- Changelog archaeology: v2.11.0 (2026-08-26) "Fixed cold-start token endpoint routing to prevent HTTP 429" — one month BEFORE the Sep cert wave; v2.9.1→2.13.0 = heavy auth/quota/hook/subagent stabilization churn. Model-label-mismatch reports (threads 120631/121376: "Gemini 3 Pro is actually Gemini 2.0 Flash") UNVERIFIED, officially denied; the verifiable core: NO per-response "model actually used" transparency exists (feature request 119658 still open).
- Encryption at rest ◐: conversation transcripts stored as safeStorage-encrypted .pb (macOS Keychain / Windows DPAPI / Linux Secret Service; hardware-bound key resisted Frida extraction — 3rd-party RE) — but MCP OAuth tokens sit in plain JSON per official docs.

### 21.5 Models, benchmarks, co-design
- Co-design narrative from day one, now operational: 3 Flash releases in 6 weeks (3.6 → 2026-07-21; 3.7 → 2026-08-13; 3.8 + 3.8 Flash Cyber → 2026-09-01/02); 3.8 Flash shipped day-one inside Antigravity AND same-day became the default of the managed API agent + SDK (operator risk: silent model drift under an unchanged agent ID); Teamwork blog: first Flash-tier PhD-level math "with the right orchestration framework." Docs-home line "Gemini 3.5 Flash powering all local agents" is STALE.
- Economics: 3.8 Flash $0.75/$3.75 per 1M intro thru 2026-12-31, doubles 2027-01-01 (also hits 3.6/3.7); Google: "may use more tokens on long, complex tasks by design" — Artificial Analysis measured $0.58/task (high) vs $0.40 for 3.7 (~30% more output tokens, same list price). Managed agent: official table $0.25–$3.25 typical by task class, "3–5 million tokens... costs up to ~$5" for complex workflows; 50–70% of input tokens typically cached; `max_total_tokens` budget (cached excluded; best-effort; `status:"incomplete"` + resume via `previous_interaction_id`).
- Selector: Gemini 3.8/3.7/3.6 Flash (thinking low/med/high) + 3.1 Pro (low/high) + Claude Sonnet 4.6 (thinking) + Claude Opus 4.6 (thinking) + GPT-OSS-120b (Medium); Claude/GPT ABSENT on Enterprise; subagent `model=` allows only inherit|flash|pro (no Claude/GPT); background models (indexing, browser actuation, checkpointing, summarization) are fixed Gemini regardless of dropdown (official reply, forum 122323); models-page vs plans-page self-contradiction on third-party availability (VERIFIED contradiction). Community survival tactic: manual Opus→Flash routing 3–4×/session, ~40% quota stop, `.antigravityignore` against indexing burn (Continuity Engine, forum 139335; metrics there are self-admitted sentiment estimates, not telemetry).
- Vendor benchmarks: 3.7 Flash card (VERIFIED as vendor-published): FrontierCode 43.6 / DeepSWE 65.3 / Code Arena WebDev 1588 Elo vs Sonnet 5 42.7/1541 and GPT-5.6 Terra 41.3/1523 — "Google's own numbers, not independent results" (layer3labs). 3.8 Flash: DeepSWE 73.7 / Terminal-Bench 2.1 89.4 SECONDARY (CodingFleet) — VERIFY vs model card before citing.
- ChatForest "third-party" table (MCP Atlas 83.6 / TB2.1 76.2 / SWE-Bench Pro 69.2 vs Opus 76.4 / 61% hallucination): exists as published but treat cautiously — internally inconsistent (its own 3.5 Flash review says the SWE-Pro score "is not published"), provenance unclear (aggregator, documents no runs), real MCP-Atlas top score ~77.3 (Opus 4.7) contradicts its cells, and SWE-Bench Pro is ~30% broken tasks per OpenAI's audit. Directional only; do not cite cells as established fact.
- Teamwork claims (2026-08-27/31): 7 open math/TCS problems; Knuth's Cycles 40-page proof "formally verified in Lean" — the strongest, falsifiable claim (proof artifacts not linked; VERIFY before amplifying); RISC-V out-of-order simulator at 0.71% cycle-alignment error vs air-gapped BOOM hardware; optimizations merged upstream into Eigen and ParlayHash (externally auditable); TCSBench 71% is GOOGLE-INTERNAL. Known friction: quota-blind multi-agent starts (forum 170693), observability gaps (CLI issue #301).
- Independent evals: NO systematic product-level evaluation exists (CONFIRMED ABSENT as of 2026-09-11). Nuance: Artificial Analysis independently tests the SDK harness — Index 57 / $1.40/task / 15.1M tokens / 110.8 turns (3.7 Flash high) and Index 42 / $2.47 / 25.4M tokens (3.8 high) vs Codex GPT-6 Astra 62/$7.47/6.7M: high capability, very high token burn, cheap and fast per task, extreme benchmark-version sensitivity (TB2.1 87% vs TB4.0 15% on adjacent configs). AirAsia ">50% of production-ready code" = vendor claim with wording drift ("production QA code" in the Aug enterprise blog); "millions of developers" vendor-only, no independent figure.

### 21.6 Notable concepts
1. Permissions-derived sandbox compilation: ONE policy source (read_url grants, file scopes) compiles into prompt gates AND container mounts AND network AllowedDomains.
2. read_url vs execute_url split: fetching a page and actuating its UI are separately gated — the right cut for prompt-injection defense.
3. Fresh-successor-between-milestones: Teamwork replaces context compaction with orchestration — hand off to a clean agent at phase boundaries.
4. /boost workstream decoupling: read-only investigation workstreams (call-graph, dependency tracing) isolated from implementation workstreams; failed verification assertions feed the next iteration automatically.
5. Artifacts as trust contract: plans/walkthroughs/recordings reviewed doc-style; changes staged "before any changes are physically written"; Request Review vs Always Proceed as the autonomy dial.
6. Subagent permission bubbling + inheritance with a per-subagent commandExecutionPolicy ladder (off/auto/eager/sandbox).
7. max_total_tokens as protocol-level budget: cap → status:"incomplete" → resume with fresh budget via previous_interaction_id — budgets that fail loudly and resumably.
8. Pre-evaluation trust gap (Pillar): a security mode that does not evaluate native tool paths is decorative — ALL tool dispatch paths must cross the permission engine.
9. Config-dir persistence as backdoor class (Mindgard): MCP config files execute on launch and survive uninstall — harness config files need .git/hooks-grade integrity care.
10. Skills as the memory story: workflows deprecated; /learn distills corrections into Rules/Skills; no separate memories DB.
11. Quota-pool architecture as a trust surface: split per-provider pools, hidden per-turn caps, silent truncation — usage meters need the same honesty as permissions.
12. Same-harness multi-surface platform: CLI/desktop/IDE/API/SDK on "the exact same agent core" — the harness becomes the platform substrate.

### 21.7 Matrix row + column corrections (§1 Antigravity column updated in place; recount 10●/8◐/7○)
- Score flips (8): language ○→◐ (Electron desktop + VS Code-fork IDE [3rd-party RE]; SDK with open examples repo; CLI Go per community UNVERIFIED); repo map ◐→○ (no AST/tree-sitter map; /codesearch semantics undetailed); PTY ○→◐ (stdout streams into TUI; no scrape architecture documented); tiered eviction ◐→○ (no local auto-compaction documented; mitigations architectural — Teamwork successor-handoff, /rewind//fork//clear; managed API agent compacts ~135k); auto skill learning ◐→● (/learn is documented distillation); model-weakness profiles ○→◐ (thinking levels + plan gating + split quota pools; no weakness catalog); editor telemetry ◐→○ (no public telemetry docs; enterprise controls UNVERIFIED); evals ◐→○ (no public eval harness; AA covers the SDK harness only). [Scout-matrix tally "12●/8◐/5○" corrected by recount.]
- Content corrections without score flips: OS sandbox (Windows UNDOCUMENTED — not "path normalization only"; + managed-API remote Ubuntu sandbox); permission tiers (+presets, +bubbling/ladder, +MCP Ask-default); prefix-cache (50–70% cached; excluded from max_total_tokens); windowed file view (view_file/edit_file without windowing semantics); subagents (workspace modes, isolation, bubbling, DeepCoder/DeepInvestigator); multi-model (subagent tier inherit|flash|pro only; Claude/GPT absent on Enterprise); skills (open std + resolution paths + /learn); MCP client (MCP Store ~47, Ask-default, auth modes); sessions (+/fork //rewind, CLI↔2.0 export, SDK persistence); encryption (safeStorage transcripts vs plaintext OAuth tokens); checkpoints (CLI /rewind + /fork, conversation-level); cost HUD (split weekly+5h pools; max_total_tokens); frontends (+Web, +SDK/API agent).

### 21.8 ADOPT / AVOID / WATCH deltas (detail agents: 12/6/6, extending the scout's inline sec21 ledger segments in §3)
- ADOPT (12): permissions-derived sandbox compilation; read_url/execute_url split (scout, kept); subagent permission bubbling + inheritance; commandExecutionPolicy 4-value ladder; Teamwork fresh-successor handoff + per-agent scratch dirs + artifact-mediated handoffs; /boost investigation-vs-implementation workstreams + assertion-feedback verification loop; /learn distillation into Rules/Skills; artifact-review gates staged before filesystem writes; max_total_tokens → status:"incomplete" + resumable budget; MCP Ask-default + mcp() scoping grammar + disabledTools[]; split per-provider quota pools (weekly+5h) surfaced in a HUD; native worktree auto-provisioning/cleanup for fanned-out subagents.
- AVOID (6): quota whiplash + undocumented hard caps at the paid tier (16,384 output tokens/turn; 1,024 Claude thinking; silent Gemini truncation); security triage as "Won't Fix (Intended Behavior)" + stealth Known-Issues edits + no CVEs; plaintext OAuth tokens beside encrypted transcripts; per-response model-identity opacity; surface churn stranding users (IDE auto-convert, consumer CLI retirement); same-day silent default-model flips under unchanged agent IDs.
- WATCH (6): Forced-Descent patch status CONFLICTED (re-check on next touch); Teamwork falsifiable claims (Lean proof, Eigen/ParlayHash merges) before amplifying; AA independent SDK-harness tracking (15–25M tokens/task; TB version sensitivity 87%→15%); community A2A wrapper while MCP-server mode stays absent; Gemini Flash cadence (3 in 6 weeks) + 2027-01-01 price doubling; artifacts-as-contract spreading across closed vendors.

### 21.9 Sources + confidence
- Official: antigravity.google/docs (home, overview, projects, subagents, artifacts, artifact-review, implementation-plan, walkthrough, task-groups, cli/{overview, subagents, artifacts, tutorial, commands/codesearch}, sdk/overview, mcp, skills, hooks, permissions, sandbox, agent-settings, teamwork, boost, models, plans, slash-commands, ide/{browser, separate-chrome-profile, allowlist-denylist, review-changes-editor, extensions}, remote-control, migration/workflows-to-skills); antigravity.google/blog (introducing-google-antigravity, introducing-google-antigravity-2, changes-to-antigravity-plans, google-io-2026-feature-deep-dive, gemini-3-8-flash-in-google-antigravity, teamwork-when-ai-becomes-a-research-partner, antigravity-enterprise) + /changelog; ai.google.dev/gemini-api/docs/{antigravity-agent, agents, latest-model}; blog.google (3-8-flash-and-3-8-flash-cyber; managed-agents + expanding-managed-agents budget rollout); developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli; codelabs.developers.google.com/getting-started-google-antigravity; github.com/google-antigravity/antigravity-sdk-python.
- Security: mindgard.ai/disclosures/google-antigravity-ide---persistent-code-execution + /blog/google-antigravity-persistent-code-execution-vulnerability (Portnoy); labs.cloudsecurityalliance.org CSA research note (+PDF); cyberscoop.com (2026-04-20); csoonline.com 4097698 + 4161382; darkreading.com (2026-04-21); securityweek.com (2026-04-22); pillar.security (the-week-of-sandbox-escapes; a-time-bomb-in-vscode-bypassing-antigravitys-secure-mode).
- Community/3rd-party (labeled): discuss.ai.google.dev threads 137702, 139335, 170693, 180449, 179354, 180907, 181274, 182074, 182129, 120631, 121376, 122323, 119658; ericxliu.me/posts/reverse-engineering-antigravity-ide; atamel.dev/posts/2026/07-01_where_agy_agent_skills; github: VIKAS9793/antigravity-continuity-engine, keisksw/antigravity-output-analysis, shyim/agm, Aminetwiti/antigravity-patch-proxy-remote, winteryu21/Damn-Antigravity-IDE-Migrator, shashikanth-gs/a2a-wrapper; artificialanalysis.ai coding-agent comparisons (antigravity-sdk-vs-gemini-cli, antigravity-sdk-vs-codex); deepmind.google model cards gemini-3-7-flash / gemini-3-8-flash; chatforest.com Antigravity 2.0 review (+ its own Gemini 3.5 Flash review as contradiction); tpsreport.news (Mar 2026 quota cuts); help.apiyi.com quota-cut guide; support.google.com Google One AI-credits thread 416663138; HN 46876045; dev.to (googleai Antigravity IDE post; stephen_sebastian $0.37 14-service audit); thenewstack.io (I/O 2026); cloud.google.com/customers/airasianext; hyperdev.matsuoka.com (Matsuoka multi-instance critique); geeky-gadgets.com (Windsurf story).
- UNVERIFIED carried: Windsurf deal figures + "rebranded Windsurf" forensics; agy implementation language (Go per community, "static binary"); Mermaid artifacts; Computer-Use lineage for 2026; memories/Knowledge-Items persisting on disk; MCP-server-mode absence (best-effort exhaustive); axonbuild no-fix page; ChatForest table cells; 3.8 Flash benchmark numbers vs model card; Teamwork Lean/arXiv/Eigen/ParlayHash artifacts; Sep 2026 TLS-cert forensics; model-label mismatch; 645-reply quota thread ID; exact Mar-2026 cut percentage; "millions of developers."
- Confidence: HIGH — platform map, permissions/sandbox/subagents/skills/MCP (official docs current to v2.13.0/v1.2.0/v0.1.16), Pillar chain (multi-outlet + dated timeline), quota/pricing arc (blog + press + forum). MEDIUM — Mindgard patch status (genuinely conflicted; both claims recorded), Electron internals (single RE source), AA harness numbers (independent lab, secondary rendering), Teamwork claims (vendor; Lean part falsifiable). LOW (load-bearing caveats) — closed internals (RSS, eviction ladder, telemetry), ChatForest table, model-mismatch claims.

## 22. Iteration 15 findings — Devin / Cognition (scout + 3 detail agents + synth; closed SaaS, 2026-09-11)

Devin (Cognition AI, founded Nov 2023 — Scott Wu/Steven Hao/Walden Yan) is a closed-source delegation platform: Brain (stateless inference/planning, ALWAYS in Cognition's cloud) + Devbox (dedicated per-agent VM), surfaced through web app, Devin Desktop (ex-Windsurf IDE), CLI, Devin Cloud/Local/Outposts, Slack/Teams, API v3 + Terraform, MCP server, Devin Review, Security Swarm, Automations. Scout premises corrected: "session sizes ACU-only" (Jul 15 2026) confirmed; "On-Call" branding NOT found verbatim (Triage Devin is the responder); "90% Devin-written code" is a vendor claim lineage 25%→89%→95% (UNVERIFIED, no audit). Matrix deltas applied to §1 (9 score flips); ADOPT/AVOID/WATCH appended to the §3 sec22 ledger segments.

### 22.1 Identity, ownership, lifecycle (funding + model arc)
- Funding: Series D 2026-05-27 >$1B @ $26B (Lux + General Catalyst + 8VC; $492M run-rate disclosed; "89% of code committed by Devin") → **Series E 2026-09-08: $2B @ $48B led by a16z + Accel** (Reuters/WSJ/TechCrunch); run-rate ~$900M at raise; $4–5B annualized projected end-2026 (The Information via TechCrunch); lifetime >$2.5B. Revenue trajectory: $1M (Sep 2024) → $73M (Jun 2025) → $492M (May 2026) → ~$900M (Sep 2026); ~53x multiple held across both rounds.
- Windsurf: acquired 2025-07-14 (brand/IDE/IP; ~$82M ARR, 350+ enterprise customers) after Google's $2.4B reverse-acquihire; ~$250M price press-reported UNVERIFIED. OTA rename Windsurf → **Devin Desktop** 2026-06-02; Cascade rebuilt as Devin Local (Rust rewrite, vendor-claimed ~30% token-efficiency gain).
- Model lineage: SWE-grep (2025-10-16, RL for fast retrieval) → Kevin-32B (CUDA kernels; origin of self-compaction) → SWE-1 (mid-2025) → SWE-1.5 (2025-10-29, Cerebras 950 tok/s, GB200-trained) → SWE-1.6 (GA 2026-04-07; length penalty introduced) → **SWE-1.7** (2026-07-08, Kimi K2.7 base, +12.2 pts FrontierCode) → **SWE-2** (2026-09-10, Kimi K3 2.8T base; first multi-trillion-param RL; **50.0% FrontierCode 1.1 Main — within 1pt of Fable 5.1 at 64% cheaper**; 92.8% TB 2.1; free in Desktop/CLI through 2026-10-10). Hiring: Jared Palmer VP Eng (ex-Vercel v0/AI SDK), Paul Grewal CLO (ex-Coinbase), Art Levy VP Partnerships ("one acquisition per week" cadence, 97 ex-founders on staff).
- Enterprise claims: Citi, Mercedes-Benz, Goldman Sachs, Elevance, Dell, Santander, US Army/Navy (Series D list); NASA press-only; Mercedes 8mo→8d modernization; Itaú 70% auto-remediation — all company claims. Enterprise usage >10x YTD 2026; ~8x in largest segment over 6 months.

### 22.2 Architecture (VM-per-agent; Desktop; Local/Outposts; Fusion)
- Canonical split: Brain (stateless planning service, always Cognition cloud — even on Customer Dedicated Deployment single-tenant VPC w/ AWS PrivateLink/IPSec; NO customer LLM keys, "compound AI system") + Devbox (dedicated execution VM: shell, browser, editor; ops over secure WebSocket). Session tools: Shell (read-only→writable toggle), IDE (live VSCode on Devin's machine), **Desktop tab** (renamed from Browser — full interactive desktop env served via `*.devinapps.com` iframes; VNC protocol never named — UNVERIFIED), Progress. Machine snapshot = "Devin's Workspace" loaded at session start (blueprints build snapshots); Chrome CDP endpoint localhost:29229; machine size configurable (Danger Zone).
- Computer Use: full desktop mouse/keyboard loop, 1024×768; Linux default; Windows sessions (WPF/WinForms testing, ~9% more ACU); macOS ONLY via Outposts (Namespace Devboxes, Apple M5).
- **Devin Outposts (2026-07-21/22, opt-in, off by default):** NOT self-hosting the product — Brain stays cloud-side; command execution/file edits/repo access happen on customer machines. Named queues; self-hosted worker `devin-remote` (public ECR image) claims sessions; claims expire on server deadline → session SELF-HEALS back into the queue (reconciliation loop). Open-source **devin-outpost-k8s** operator (Rust, MIT). Partners: Cloudflare, Daytona, E2B, Modal (GPU scale-to-zero), Namespace (macOS), NVIDIA Brev. Cursor-split-brain analog. Security-profile network enforcement on Outposts is explicitly the OPERATOR's job (`spec.network_policy`).
- **Devin Local:** local agent inside Devin Desktop (also CLI: "a local agent like Cascade"); direct local machine access. UNVERIFIED whether Local runs the same Brain loop or a local harness loop. CLI sandbox: Linux bubblewrap+socat fail-closed; **OS sandbox unsupported on Windows** (hard-fail with `--sandbox`/Required enforcement, incl. ACP-in-Desktop); Git Bash "supported (some features limited)".
- **Fusion (cloud preview 2026-06-29; Desktop+CLI follow-on):** frontier lead + cheap "sidekick" run IN PARALLEL, each with OWN tools + OWN persistent cached context (avoids naive-routing cache misses); dynamic mid-session routing executes model switches AT COMPACTION EVENTS ("model switching for free" — the cache miss is paid anyway); up to 60% cheaper at frontier quality (revised from 35% on refreshed data; Fusion+Fable 5 = 41% cheaper); "pricier sidekicks can lower total session cost via fewer retries"; judgment-heavy delegation backfires (score −2754→27) while mechanical/verification handoff works (−62% cost). Local Fusion: explicit lead+sidekick+effort pick, `/fusion` picker, Fast Mode, recommended pairing Fable 5.1 + SWE-2, independently evaluated w/ Artificial Analysis + Vals AI. Cost HUD `/session-stats`: tokens, per-model cost, estimated Fusion savings.

### 22.3 Knowledge loop (DeepWiki / Playbooks / Knowledge / Session Insights)
- Knowledge: trigger-description-addressed items auto-recalled; `!name` macros; folders w/ AI auto-organize; repo pinning; **Knowledge Suggestions auto-generated from chat feedback** (editable, regenerable, updates existing items); org→enterprise "Promote to Enterprise" button; v3 notes APIs.
- Playbooks: reusable prompt library; `@playbook` inclusion; slash commands map to them; CRUD via API v1/v2/v3 + Devin MCP.
- Skills: SKILL.md in-repo (`.agents/skills/`) + CLI skills w/ YAML frontmatter (`model:` override); AGENTS.md supported. Auto skill learning: Devin proposes skill/plugin saves as approval cards; playbooks + knowledge-base maintenance under "Advanced Capabilities."
- DeepWiki: auto-generated repo wikis (architecture diagrams, docs, source links) enabled by repo indexing (v3 bulk-index APIs); `.devin/wiki.json`; free public DeepWiki MCP at mcp.deepwiki.com; 400k+ repos; 5M-line COBOL repos cited.
- Org learning loop: managed sessions in parallel + analyze past work + create playbooks + keep knowledge current; Session Insights feeds improvement. "Devin builds Devin": 25% (Apr 2025) → 89% (Series D post) → 95% (Wu, Jul 2026) — vendor claims, no audit; ZenML notes the overstatement caveat.

### 22.4 Extensibility (MCP client + server; Marketplace→Plugins; hooks)
- MCP client: stdio/SSE/HTTP(Streamable); Personal/Org/Enterprise scopes; custom servers admin-gated; OAuth (org-shared service-account vs personal; callback api.devin.ai); "Test listing tools" spins an isolated env; enterprise CA bundles + **Private-Network MCP for dedicated deployments** (OAuth + calls via private tunnel; customer-uploaded CA).
- **MCP server mode: official `https://mcp.devin.ai/mcp`** (Streamable HTTP; SSE deprecated): `devin_session_create/search/interact/events/gather` (create supports ACU-limit param; **gather = fan-in barrier for parallel sessions**), `devin_playbook_manage`, `devin_knowledge_manage` (incl. suggestion dismissal), `devin_schedule_manage`, `devin_list_integrations`, `read_wiki_*`/`ask_question`; auth = `cog_` service-user keys + `X-Org-Id`. Plus free DeepWiki MCP.
- Marketplace→Plugins consolidation: MCP Marketplace (2025-07-28) → Plugins w/ enterprise governance (2026-07-17: required/optional/forbidden, orgs inherit) → full form 2026-09-09: skills+rules+hooks+MCP servers+SUBAGENTS as one installable package; managed manifests (`requiredPlugins`/`optionalPlugins`/`forbiddenPlugins` w/ glob deny + SHA pinning + `secret:org:NAME` refs); authority = higher level wins (`forbid "*"` lockdown); official marketplace github.com/CognitionAI/devin-marketplace; per-install security notice + trust confirmation.
- Hooks: CLI lifecycle events; Rules: always-on context. CLI config import reads Cursor, Windsurf, Claude Code, GitHub Copilot, **OpenCode**, Zed configs.

### 22.5 Models + economics
- SWE-2 method: Pareto-informed LINEAR cost penalty `R = S − λ_e·C`, single RL run covering ALL effort levels — λ_e set to the LOCAL SLOPE of the base model's Pareto frontier at that effort (iso-reward tangent ⇒ any reward gain is a real frontier push; Jensen's functional equation proof in Appendix B); length-weighted reward baseline (since SWE-1.6); serving: prefill delayer (+10-20% TPM), DSpark speculative decoding w/ ONLINE-RETRAINED draft model via SpecForge (+15% accept length), NVFP4/FP8 + quantization-aware training; verifier-hardening flywheel. Behavior: SWE-2 medium = 53 steps vs SWE-1.7's 127 (58% fewer turns, 81% less cost at higher score); effort ladder 53/80/98 steps. Trustworthiness re-run: 98.0% overall pass; NO statistically significant persona effect for any of six models (contradicts CrowdStrike raw-completion findings — the agentic harness itself suppresses persona-conditional behavior).
- Fusion economics: FrontierCode 1.1 Extended — Fusion 63.1 @ $1.35/task vs Fable 5 xhigh 64.9 @ $10.53, Opus 5 medium 63.6 @ $3.51; 88% of merged PRs by enabled employees driven entirely by the automated router (internal). Cautionary footnote: Fable 5 access suspended 2026-06-12 by a US government directive — company-reported, UNVERIFIED.
- Pricing arc: ACU era (Core $20 + $2.25/ACU PAYG; Team $500 w/ 250 ACUs; enterprise ACU pools w/ per-user caps; ~15 min agent work per ACU; Windows +9%) → **message-quota era** (Free/$20 Pro/$200 Max/$80 Teams +$40/dev seat; daily+weekly refresh; overage at API pricing; concurrency ≤10 Free/Pro) — self-serve silently abandoned ACUs ("Legacy Core plan users have been migrated to the Free plan"; exact flip date UNVERIFIED) while enterprise stayed pure-ACU; **ACU never left enterprise**; Jul 15 2026 "session sizes now ACU-only" re-converged sizing — a metering-identity crisis spanning two years. Devin Review consumption in ACU pills XS≤2.25…XL>18 w/ per-PR spend caps.
- Benchmarks posture: stopped reporting SWE-bench in 2024 (13.86% unassisted Mar 2024 = the origin SOTA); pivoted Junior-Dev → SWE-bench Pro → **FrontierCode 1.1** (150 real-PR-derived tasks, correctness + code quality vs 1000+ reviewer criteria; "fair internet use" doctrine w/ programmatic scanner zeroing upstream-PR leaks; Diamond subset deprecated as too noisy) — self-run, self-headlined, best-across-effort reporting, list-price cost axes; no third-party replication found (UNVERIFIED externally). Security Swarm (Agentic MapReduce): 72% recall vs "Claude Security" 68%/Codex 48%/Cursor 26% at $90.23/run — vendor-run, competitor labels supplied by Cognition.

### 22.6 Notable concepts (8)
1. Brain/Devbox split: stateless planning always cloud-side + disposable per-agent VM over WebSocket — the cleanest published statement of the loop/execution divide.
2. Fusion = industrialized Smart Friend: multiple MODELS, one writer's harness; cache-aware model switching AT compaction points makes routing free.
3. Outposts queue-claim-reconcile: sessions self-heal back into the queue when a self-hosted worker's claim expires — durable delegation over untrusted compute.
4. `gather` fan-in barrier as an MCP tool: parallel sessions coordinated through the server surface, not prompt choreography.
5. Pareto-slope cost penalty: reward slope read off the base model's own frontier — RL that is provably pushing the frontier, one run, all effort levels.
6. Generator–verifier with ZERO shared context: Devin Review finds ~2 bugs/PR (58% severe) precisely because the reviewer doesn't inherit the writer's context (Context Rot escape).
7. Security Profiles intersection semantics: 5-level binding where lower levels can only TIGHTEN (mandatory enforcement = intersection) — unweakenable-by-subordinate policy.
8. Trustworthiness evals run INSIDE the agent harness to dodge evaluation-awareness; null persona-effect finding positions the harness itself as a safety control.

### 22.7 Matrix row + column corrections (row summary; §1 Devin col corrected in place — 9 score flips)
Language ○ closed (only OSS artifact = devin-outpost-k8s operator, Rust MIT) | Steady RSS ○ unpublished (cloud Devboxes; CLI local but no figures) | OS sandbox ● (VM-per-session by default; CLI bwrap+socat fail-closed; Windows hard-fail) | Permission tiers ● [corrected from ◐] (Read/Write/Exec rule DSL w/ allow/ask/deny + specificity resolution; Security Profiles GA 2026-08-07: network/MCP/git/gh restrictions, 5-level binding + intersection semantics; AI Guardrails + Attribution Filtering) | Prefix-cache ● (Fusion cache-first: sidekick keeps own cached context; switches at compaction, no extra penalty) | Repo map ◐ (repo indexing + DeepWiki diagrams; no AST/PageRank map) | Windowed file view ○ [corrected from ◐] (VM file tools; no documented windowing; VSCode view is human-facing) | PTY ◐ [corrected from ○] (CLI persistent-PTY shell `pty_for_noninteractive_exec`; cloud shell command history; no scrape-harness pattern) | Tiered eviction ◐ [corrected from ●] (compaction exists and is load-bearing — Fusion switches models at compaction; self-compaction trained in; NO documented tiered ladder) | Subagents ● (CLI subagents w/ fg/bg + max-nesting + per-profile model pinning; Dynamic Workflows fan-out per-agent VMs; Triage Devin children; Security Swarm MapReduce) | Multi-model ● (Fusion lead+sidekick+effort spec; Adaptive router; Slack bangs; model-neutral cloud) | Model-weakness profiles ● (effort tiers + per-pair Fusion harness policies + admin allowlists + per-subagent model) | Skills ● (SKILL.md + CLI frontmatter skills + plugin-bundled `/<plugin>:<skill>`) | Auto skill learning ● (Knowledge Suggestions from chat feedback; approval-card skill/plugin proposals; playbook creation) | MCP client ● [corrected from ◐] (stdio/SSE/HTTP + OAuth + enterprise CA + private-network MCP + scope-gated management) | MCP server mode ● [corrected from ○] (mcp.devin.ai Streamable HTTP: sessions/playbooks/knowledge/schedules/wiki + gather fan-in; cog_ keys) | WASM ○ | Sessions ● (persistent/resumable/sleeping 30-min-idle default; v1/v2/v3 APIs + MCP + Terraform; no concurrency cap) | Encryption at rest ● [corrected from ◐] (transit+at-rest default; SOC 2 Type II; Enterprise Assured CMK AWS KMS; ZDR opt-out) | Checkpoints ◐ (VM snapshots + browser-profile snapshots + command-history time-travel VIEWING; no per-edit rewind) | Cost HUD ● (/session-stats tokens+per-model cost+Fusion savings; Review ACU pills + per-PR caps; automation ACU + invocation limits; org/per-user caps; Devin Coach) | Frontends ● (web + Desktop + CLI + ACP JetBrains/Zed/Xcode + voice + Slack/Teams; broadest async surface) | Multi-client ● [corrected from ◐] (Slack/Teams + GitHub/GitLab/Linear/Jira/Pylon + API v3 + Terraform + MCP server + webhooks + email + npx devin-review) | Telemetry ● (Session Insights + audit logs + metrics/consumption/guardrail APIs; no OTel export documented) | Evals ◐ [corrected from ●] (FrontierCode 1.1 self-run w/ published methodology, no external replication; Artificial Analysis + Vals AI third-party Fusion evals; Security Swarm vendor bench; no user-facing in-harness eval runner).

### 22.8 ADOPT / AVOID / WATCH deltas (10/8/7; condensed appends already in §3)
- ADOPT (10): postmortem genre as product practice (publish failure analyses with mechanics); annotate-expected-before-acting honesty pattern ("senior at understanding, junior at execution"); Fusion cache-aware model switching at compaction points (sidekick w/ own cached context); Brain/Devbox loop/execution split; Outposts queue-claim-reconcile self-hosted workers (Rust k8s operator); Read/Write/Exec rule DSL + Security Profiles intersection semantics; Knowledge Suggestions auto-generated from chat feedback + Playbooks + org→enterprise promotion; MCP server mode w/ `gather` fan-in barrier; Pareto-slope linear cost penalty RL (single run, all effort levels); self-compaction + alternating length penalty trained into the model.
- AVOID (8): benchmark-anchored replacement marketing (13.86% SWE-bench + "first AI engineer" framing at $500/mo vs 3/20 real tasks); demo-as-curated-sample (denominator off camera; faked-demo accusations recur); pricing-metering identity crisis (ACU→messages→ACU-only; Core users silently migrated to Free; opaque overages); publishing conflicting official EOL dates and never correcting them (Cascade July 1st vs through-July; 6-week forced migration; silent Cascade fallback); parallel-writer swarms (their own doctrine — writes stay single-threaded); "Dedicated" deployments where the Brain never leaves vendor cloud (no customer LLM keys); vendor-self-run headline benchmarks (self-hosted FrontierCode, best-across-effort, list-price axes, 35%→60% revisions); Windows as second-class (no OS sandbox, Git Bash limited tier).
- WATCH (7): SWE-bench Verified formally dead (OpenAI audit: 59.4% flawed audited tests + training contamination) — the eval-void implications; sidekick cost-routing spreading (delegation good for mechanical work, backfires when judgment is the deliverable); agent confidence identical whether right or wrong — verification debt is the real price; SWE-2 open-base RL economics (50.0% FrontierCode within 1pt of Fable 5.1 at 64% cheaper — margin via post-training cheap open bases); Marketplace→Plugins governance consolidation (required/optional/forbidden + SHA pinning); generator–verifier review loops w/ zero shared context; harness-suppression of persona-conditional model behavior (null differential-security finding).

### 22.9 Sources + confidence
- Docs: docs.devin.ai — cloud/outposts/{overview,reference,partners}, cli/{sandbox,fusion,adaptive,subagents,enterprise/controls,reference/{configuration/read-config-from,terminal-compatibility}}, work-with-devin/{devin-session-tools,mcp,devin-mcp,computer-use,browser-auth,deepwiki,devin-review,security-swarm,advanced-capabilities,stacked-prs}, product-guides/{knowledge,creating-playbooks,skills,plugins,plugin-ecosystem,security-profiles,automations,session-insights}, enterprise/{deployment/overview,security-access/*,features/{usage-policies,ai-guardrails,attribution-filtering},local/controls,environment-management/overview}, admin/{security,billing/{usage,enterprise,self-serve}}, federal/introduction, release-notes/{overview,2026,2025}; llms.txt index; enterprise deployment mirror cognitionai.mintlify.app; trust.cognition.ai.
- Blogs/primary: cognition.com — swe-2, swe-1-7, swe-1-6{,-preview}, swe-1-5, swe-grep, devin-fusion, local-fusion, frontier-code{,-1.1}, multi-agents-working, dont-build-multi-agents, measuring-open-source-model-trustworthiness, series-d, swe-bench-technical-report; devin.ai — introducing-devin-outposts, security-swarm-eval, agentic-map-reduce, pricing, windsurf waves; arxiv 2607.24653 (Kimi K3) + 2607.05147 (DSpark) + 2603.18567 (SpecForge); cognition.ai/blog/devin-annual-performance-review-2025.
- Press/community: Reuters + WSJ + TechCrunch (Series E + D); sourcery.vc (95% claim); thenextweb; sfstandard; microsoft/MAI-Code-adjacent comparisons n/a; snowmanlabs + techtimes (partner-biased, flagged); zenml case-study caveat; contrary research profile; Reddit r/ChatGPTCoding + r/aipromptprogramming + r/ycombinator; devaireviews (Cascade EOL double-booking); kubesimplify + logicoflogic (Outposts).
- UNVERIFIED carried: ~$250M Windsurf price; VNC protocol for Desktop tab; Devin Local harness provenance; ACU→quota flip date; Fable 5 government-directive suspension; "Devin 3 at 90% SWE-bench"; NASA as customer (press-only); FrontierCode + Security Swarm numbers (no independent replication); 25%→89%→95% Devin-written-code lineage.
- Confidence: HIGH — architecture split + Outposts mechanics + security profiles (official docs, mirror-consistent), model lineage + Fusion/SWE-2 methods (primary posts fetched in full), funding (Reuters/WSJ/TechCrunch), release-notes archaeology (official changelog). MEDIUM — Fusion economics (vendor benchmarks w/ third-party evaluators named), enterprise customer claims (company posts). LOW load-bearing — self-run benchmark numbers used directionally only.

## 23. Iteration 16 findings — Zed agent panel + ACP + Delta (scout + 2 detail agents + synth; open core GPL-3.0, 2026-09-11)

Zed (zed-industries/zed) is an open-core Rust editor whose native agent harness is itself implemented as an ACP server against Zed's own host — the panel hosts first-party and external agents side by side, indistinguishably. This section REWRITES the scout stub; verified 2026-09-11 against a blob:none source clone at HEAD a9cdfc9936c4ccf0503ee4631204e045f913e2c1 (tag v1.19.2 in-repo) plus a full-history blobless clone for archaeology; scout-sourced external facts (ACP registry, Delta app, Agent Metrics, JetBrains co-launch) carried with attribution. Matrix deltas applied to the §1 Zed column (8 score flips); ADOPT/AVOID/WATCH appended to §3's "Zed adds (sec23)" ledger segments in place.

### 23.1 Identity, lifecycle, economics
- Zed Industries (founded by the Atom team — Nathan Sobo et al.). Open core done seriously: editor, native agent harness, and Zeta weights all in the public repo; hosted-model service + accounts are the closed complement. Licensing is dual per-crate (GPL-3.0 + Apache-2.0, e.g. crates/agent_skills ships LICENSE-GPL + LICENSE-APL). Funding: NOT re-verified this iteration (scout-brief only) — carried UNVERIFIED.
- Verified release train: v1.0.0 2026-04-29; 44 v1.* non-pre tags in ~4.5 months; strict weekly cadence (v1.9.0 Jul 1 → v1.19.2 Sep 9) with lockstep -pre tags published the same minute (v1.19.2 + v1.20.0-pre both 2026-09-09 17:41Z); patch spikes (v1.10.1–3 within 4 days) never broke the train. HEAD a9cdfc993 = 2026-09-11, v1.19.2 era.
- **Delta app (Aug 2026)**: Zed's new collaboration app — scout-verified EXTERNAL to the repo (no web/mobile frontend in-repo; matrix Delta claims UNVERIFIED from source here). crates/collab is alive (touched 2026-09-11) and still experimenting (LiveKit changes reverted, not dropped).
- **"Zed Cloud" does not exist**: no cloud app-server in-repo; the remote story is an SSH-only headless remote_server + proxy; ZED_CLOUD_PROVIDER_ID exists only as a model-provider auth channel (thread.rs:48). Scout-brief "Zed Cloud" premises resolved as false.
- Cost surfaces: live thread.cost() label with currency handling + Cost panel (thread_view.rs); NO budget caps or limit enforcement found (grep budget in agent_settings → 0). Training-data posture: opt-in only; public anonymized Agent Metrics page (846.4K sessions / 7.1M turns per 30d @ 2026-08-03, scout).

### 23.2 Agent architecture (Parallel Agents, subagents, Terminal Threads, ACP host)
- Native agent = ACP server against Zed's own host: NativeAgentServer implements AgentServer::connect() -> Rc<dyn acp_thread::AgentConnection> (native_agent_server.rs:23-54) — native and external agents are indistinguishable to the host UI; protocol-level dogfooding.
- Turn loop (Thread::run_turn → run_turn_internal, thread.rs:2753-2969): per-iteration compaction check → model + tool refresh (mid-turn model/profile/tool switches take effect between tool rounds) → streamed completion with batched event processing, FuturesUnordered parallel tool results, early-abort on erroring in-flight tool input, cancellation watch channel, refusal→fallback-model retry with attempt counter. Stop reasons modeled: EndTurn / Refusal (truncates messages) / MaxTokens. 29 tool files incl. LSP-backed go-to-def/find-refs/code-actions, sandboxed-terminal variants (one `terminal` name matching sandbox state), spawn_agent, create_thread, list_agents_and_models, skill tool, MCP context-server tools.
- **Parallel Agents** (Apr 2026: onboarding #52940 2026-04-07 → docs #53535 2026-04-22; 6 UX/docs/telemetry commits in 17 days = launch choreography). spawn_agent tool doc contract (spawn_agent_tool.rs:13-47): disjoint write sets, parallel delegation patterns, session_id follow-ups, output = final message only. **MAX_SUBAGENT_DEPTH = 1 on main** (thread.rs:77; enforced agent.rs:3196) — CORRECTS scout's "depth ≤4"; tool registered only while depth() < cap; subagent tree persisted (parent_session.subagents + "Subagent Started" telemetry with parent_thread_id).
- Sibling threads: CreateThreadTool (flag-gated) with worktree isolation — linked git worktrees of each git repo + ref (default HEAD); full archive/restore lifecycle via git worktree remove (thread_worktree_archive.rs). Threads persist to SQLite threads.db via global ThreadStore (Connection::open_file — plain, no encryption).
- Terminal Threads: Jun–Sep 2026 line (init command #59374 → searchable #60292 → sidebar rename #63494 → panel renaming #63660 landed 2026-09-11, the verification day — still actively shipping); host-owned headless Terminals with SandboxWrap metadata; "sandbox not applied" reasons recorded (acp_thread.rs:877-907).
- ACP host: external agents = child processes over stdio JSON-RPC (3× piped stdio, line-buffered reader, background stderr log task; agent_servers/acp.rs:846-918); built-in auth incl. Gemini CLI spawn; agent binaries auto-installed from GitHub release assets with SHA-256 verification and env redaction (agent_server_store.rs).
- OS sandbox: macOS Seatbelt, Linux Bubblewrap, **Windows Bubblewrap-via-WSL** (sandboxing.rs:20-23; feature-flag gated); writable auto-grant = worktree roots; .git metadata protected; HostPattern network policy; model unsandboxed:true escape requires once-per-thread approval. Permissions: Allow/Deny/Confirm global default + per-tool override + profile-scoped; restricted worktrees auto-downgrade write/ask→minimal (thread.rs:2240-2265).
- Checkpoints: HOST-side in acp_thread, not the agent crate — GitStoreCheckpoint per user message, rewind() restore, auto-update when repo state changes, terminal kill on restore; uniform for native + external agents.
- Compaction: single summarize strategy; AutoCompactThreshold = Percentage | TokensUsed | TokensRemaining; manual Thread::compact(); dedicated compaction_model setting; auto-compact gated to models over a context floor; telemetry outcomes manual/auto/canceled/failed. NO tiered/multi-layer eviction found.
- MCP: context_server client (stdio/HTTP + OAuth + MCP-Protocol-Version negotiation stamping); in-tree McpServer (Unix socket in a zed-mcp tempdir) has ZERO production callers — server-mode wiring UNVERIFIED; WIT context-server.wit lets Wasm extensions act as MCP servers.
- Context economics: NO repo-map anywhere (grep repo_map|repomap → 0); tree-sitter outlines + LSP defs/refs/diagnostics + grep + line-windowed inclusive (start_line,end_line) reads sliced from the same rope/sum_tree buffers the editor uses. Anthropic explicit cache_control markers vs Google AI cached-token accounting only — no unified cross-provider cache strategy.

### 23.3 ACP authorship + registry (protocol state, SDKs, JetBrains co-launch)
- ACP is Zed-authored: crates/acp first appears 2025-07-09 ("ACP #34030", acp 0.0.6 same day); Claude Code E2E tests 2025-07-18; plans display 2025-07-21; "ACP over MCP server impl" 2025-07-28 — prototyped ~6–7 weeks before the public Aug 2025 launch against Claude Code as first external agent (public launch day itself UNVERIFIED from git). In-tree protocol state: agent_client_protocol::schema::v1; crate churn 0.0.6 → 0.15.0 (2026-06-22). Scout: agentclientprotocol.org, v1 stable, 5 SDKs.
- Registry re-platforming (scout): agent installation as a package-manager moment (Claude, Codex, OpenCode, Copilot, Cursor, Pi, Poolside installable from a menu); extension-provided agents deprecated. Ecosystem breadth in-tree: Poolside setup docs (2026-08-05), OpenCode plugin guard (2026-07-11).
- **JetBrains co-launch** (scout; cross-ref sec24 + Warp row "all JetBrains IDEs via ACP"): a second major editor vendor adopting ACP de-risks "Zed-only standard" — the LSP-of-agents bet is now two-vendor.
- Boundary discipline: external agents own auth, models, tools, config; Zed forwards its MCP servers over ACP but explicitly does NOT assume its instructions or permissions govern foreign agents; thread import runs over ACP too.

### 23.4 Zeta → edit_prediction line
- Origin: 2024-12-09 "Introduce a staff-only inline completion provider (#21739)" — staff-gated from day one.
- Data-collection eligibility is license-scoped: permissive-license detection (Apache-2.0 #35900, BSD #37194, zlib #37238, Aug–Sep 2025) + per-recent-edit-history collection check (#37680).
- Pivot: 2025-12-04 "Reorganize edit prediction code and remove old experiments (#44187)" — zeta the brand becomes edit_prediction the architecture; LSP-based context retrieval added next day (#44036).
- Consent surface evolved commit-by-commit in public (2026): cloud data-collection choice DROPPED then restored (#49426, 2026-02-17 — silent regression caught), repo_url included when enabled (#50745), org-level kill-switch (#53639), user-facing allow_data_collection setting (#51389).
- 2026 plumbing breadth: self-hosted "Sweep" next-edit models (#51139), Ollama support (#63519, 2026-09-01), bespoke LSP request path (#60947), v4 cursor markers.

### 23.5 Git archaeology — agent-crate evolution, reversals, perf discipline
- The founding rename: dc83f1ad 2025-04-01 "Rename assistant2 to agent (#27887)" — an 18-month dormant experiment crate (1de02cf6, 2023-12-06) became the product axis ~3 months before the public agentic-editing launch. Product strategy visible as a git mv.
- Subagent timeline: flag + skeleton 2026-01-08 (#46186) → GA flag removal 2026-03-02 (#50493) = 8 weeks flag→GA; tool renamed to spawn_agent 2026-02-20 (#49741).
- The three task-brief PRs (#54084/#54142 GitGlimpse895, #54213 eholk) — ALL closed unmerged (API-verified); the "exemplary docs" praise is UNVERIFIED hearsay. create_thread landed via staff rework #57987 (eholk, 2026-06-01, six-week incubation) — the clearest "community proposes, staff re-lands" pattern, scaled by guild automation.
- Reversal discipline (~1 substantive revert/month on user-visible surfaces in 2026): punctuation word movement (reverted → re-landed), sidebar thread rename (reverted May → re-shipped Aug), LiveKit collab experiment (reverted), terminal process-group fix (reverted → re-fixed), untitled-buffer language detection, macOS 27 titlebar ping-pong (fix/revert/re-fix in 12 days). Post-pivot deletions: "Remove Fix with Assistant" (2026-04-09); inline assistant insertion-mode removal (2025-12-05). Tool surface re-scoped by DEFAULTS, not just additions: ask_user disabled by default (2026-08-21), approval-before-create-threads instruction (2026-07-29).
- Perf discipline as history: 2.5-year unbroken frame-time measurement lineage (ZED_MEASUREMENTS 2024-01-22 → clipped-render optimization 2025-12-16 → frame-time debug overlay 2026-08-17) + Aug 2026 bench-infra productization burst (executor work in bench reports #63035, bench/test split #63039, headless renderer #63244, headless Metal fixes on verification day #64099). Honest subject-line count: ~40–50 perf commits/yr — weekly drumbeat, not campaign. GPUI: HANG_THRESHOLD = FRAME_BUDGET = 10ms with per-frame foreground attribution; rope + sum_tree incremental trees under everything.
- Governance: guild workflows are real infrastructure (guild_assignment_status / guild_new_pr_notify / guild_stale_assignments / guild_weekly_shipped + PR-labeler role checks; POC 2026-04-29 → labeler 2026-03-19 → board automation → role-check switch, Apr–Aug 2026). "Zed Guild" credit lives in GitHub release notes, outside git (in-repo absence verified).

### 23.6 Notable concepts (10)
1. Harness-as-ACP-server: the native agent is just another ACP connection — the host never special-cases its own agent; external harnesses get identical UX (checkpoints, permissions UI, thread import).
2. ACP as the LSP-of-agents: transport + diff/thread UX types over stdio JSON-RPC; the registry turns agent installation into package management; registry curation = the platform gate.
3. Neutral-host economics: the host takes no cut of foreign agents' model spend (they own auth/models); first-party hosted models are an optional complement; opt-in data posture + public Agent Metrics are the trust products; Delta/DeltaDB extend the neutral substrate — local CRDT capture of filesystem/keystroke operations (beta) as a future context/telemetry layer owned by no model vendor.
4. Host-side checkpoints: rewind lives in the host crate (acp_thread), not the agent — uniform undo for native and foreign agents; restoring a checkpoint kills terminals (recorded behavior, tested).
5. Disjoint-write-set delegation: parallel subagents under a documented no-overlapping-writes contract + hard depth cap 1 + sibling threads in linked git worktrees with ref pinning and full archive/restore.
6. Permission compiler: Allow/Deny/Confirm × global/per-tool/profile with un-overridable kernel safety rules, chained-command parsing, safe-pattern extraction from approvals — mirrored by an OS-sandbox tier ladder with a once-per-thread unsandboxed escape.
7. Per-role model routing: default/subagent/inline/commit-message/thread-summary/compaction models + refusal-fallback retry + mid-turn hot-switch + the model can query its own catalog (ListAgentsAndModelsTool).
8. Editor-grade eval philosophy: behavior-diff evals per model release judged by "would this disrupt an editor user", not leaderboard position; eval-cli headless NativeAgent pipeline (JSON EvalResult, ZED_EVAL_ENABLE_FLAGS) + shared e2e macro reused by native and external agents.
9. Extension capability security: declared capabilities (process:exec / download-file / npm:install) over a 10-version WIT compatibility ladder (since_v0.0.1→v0.8.0, 77 .wit files); Wasmtime 48 default-deny.
10. Perf as culture: a 10ms frame budget enforced by the GPUI hang profiler; measurement lineage unbroken for 2.5 years; reverts owned publicly (~1/month on user-visible surfaces).

### 23.7 Matrix row + column corrections (§1 Zed column updated in place — 8 score flips; recount 14●/10◐/1○)
- Flips (8): OS sandbox ○→● (Seatbelt/Bubblewrap/Bubblewrap-via-WSL, feature-flag gated; Windows path WSL-dependent); PTY ○→● (alacritty_terminal fork emulation + host-owned headless Terminals w/ SandboxWrap; terminal threads); tiered eviction ●→◐ (single summarize-compaction with auto thresholds + manual force; NO ladder — stale §23.4 "eviction ladder" claim corrected); subagents ◐→● (spawn_agent + create_thread + persisted subagent tree; depth 1, NOT scout's ≤4); auto skill learning ○→◐ (builtin create-skill guided authoring; no autonomous loop); MCP server mode ○→◐ (in-tree Unix-socket McpServer + WIT surface; zero production callers, wiring UNVERIFIED); encryption at rest ◐→○ (plain SQLite threads.db; opt-in training-data posture unchanged); cost HUD ●→◐ (live cost label + panel; no caps/budgets).
- Content corrections without score flips: language (dual GPL-3.0/Apache-2.0 per-crate @ v1.19.2; "Zed Cloud" nonexistent — SSH remote only); RSS (10ms frame-budget discipline, figures still unpublished); permission tiers (+profile scoping + restricted-worktree downgrade); prefix-cache (Anthropic explicit cache_control vs Google accounting-only); repo map (grep-verified absent; tree-sitter outlines + LSP + windowed reads instead); windowed view (1-indexed inclusive row slices over rope/sum_tree); multi-model (+per-role models + refusal fallback + mid-turn switch); skills (agent_skills size caps + disable_model_invocation; prior "Skills replace Rules" cell text was wrong); WASM (Wasmtime 48 + WIT ladder + capability enum); sessions (SQLite threads.db + ThreadStore); checkpoints (host-side, uniform, terminal-kill); frontends (kept ● — desktop + headless eval-cli + SSH remote ≈ Pi's ● precedent; scratch row's ◐ recorded as unresolved discrepancy, no web/mobile first-party, Delta external); multi-client (ACP host multiplexing ≠ live multi-client attach; Delta external); telemetry (+in-repo event taxonomy + Agent Metrics figures).
- Scout-conflict ledger: subagent depth 1 (not ≤4); checkpoints host-side (not agent crate); no Zed Cloud server (SSH remote + provider auth only).

### 23.8 ADOPT / AVOID / WATCH deltas (detail-agent deltas 12/4/6, extending the scout's sec23 entries in §3 in place)
- ADOPT (12): ACP child-process lifecycle with SHA-256-verified release-asset auto-install + env redaction; host-side uniform checkpoints (auto-update on repo change, terminal-kill on restore); OS-sandbox tier (Seatbelt/Bubblewrap/WSL + worktree-root auto-grant + .git protection + once-per-thread unsandboxed escape); per-role model profiles + refusal-fallback retry + modeled stop reasons (Refusal truncates); spawn_agent disjoint-write-set contract + hard depth cap; sibling-thread worktree isolation with ref pinning + archive/restore lifecycle; profile-scoped permissions + restricted-worktree auto-downgrade; line-windowed inclusive buffer reads over rope/sum_tree; GPUI 10ms frame budget + per-frame hang attribution + bench-infra productization; extension capability declarations + 10-version WIT ladder; eval-cli JSON EvalResult + shared e2e macro; compaction telemetry outcomes + AutoCompactThreshold knobs.
- AVOID (4): plain-SQLite persistent sessions (full thread history, no at-rest encryption — scout's "nothing new" AVOID verdict revised); core safety surface (OS sandbox) shipped feature-flag-gated with Windows = WSL-dependent; consent plumbing that silently regressed once (cloud data-collection choice dropped, caught + restored 2026-02-17 — consent paths need regression tests); cost HUD with zero budget/cap enforcement.
- WATCH (6): Delta app (Aug 2026) + DeltaDB CRDT operation capture as the next context/telemetry substrate — external, re-verify on next touch; ACP registry curation as ecosystem gate + JetBrains co-launch broadening the protocol beyond Zed; McpServer Unix-socket server mode in-tree with zero production callers (wiring watch); subagent depth policy movement (community depth-PRs closed unmerged; defaults-as-governance: ask_user off by default 2026-08-21); external agents hiding model identity from hosts (Agent Metrics blind spot); per-release behavior-diff evals with an editor-disruption bar as an alternative eval philosophy.

### 23.9 Sources + confidence
- Source: zed-industries/zed blob:none clone @ a9cdfc9936c4ccf0503ee4631204e045f913e2c1 (v1.19.2, verified 2026-09-11) — crates/agent (thread.rs, native_agent_server.rs, agent.rs, tools/, sandboxing.rs, tool_permissions.rs, db.rs, thread_store.rs, create_thread_tool.rs, spawn_agent_tool.rs), crates/acp_thread (checkpoints, terminal.rs), crates/agent_servers (acp.rs), crates/project (agent_server_store.rs), crates/context_server (client.rs, listener.rs), crates/agent_skills, crates/extension (capabilities.rs) + extension_api/wit, crates/gpui (profiler/hang.rs), crates/eval_cli, crates/anthropic, crates/google_ai, crates/remote, crates/collab; full-history blobless clone for archaeology (every SHA resolved locally; PR states API-verified).
- Scout-carried (external): agentclientprotocol.org (v1 stable, 5 SDKs, registry + extension-agent deprecation); zed.dev/docs/ai; HN 45038710 (ACP launch, 175 pts) + 47866750 (parallel agents, 279 pts); WorkOS interview with Anant Goel (2026-08); Agent Metrics page (846.4K sessions / 7.1M turns per 30d @ 2026-08-03); Delta app Aug 2026; JetBrains ACP co-launch (cross-ref sec24).
- UNVERIFIED carried: Zed Industries funding; public ACP launch date (git proves only internal commits from 2025-07-09); "exemplary docs" praise for closed PRs; Zed Guild credit in release notes (outside git); which hosted models were retired (2026-02-18 docs commit); McpServer production wiring; Delta app internals.
- Confidence: HIGH — all in-repo architecture claims (pinned SHAs + line numbers), release-tag lineage, renames/reverts (git-verified), PR states (API). MEDIUM — Delta/DeltaDB details + neutral-host economics framing (scout + partial repo evidence), McpServer wiring absence (grep-based negative), perf commit counts (subject-line methodology). LOW (load-bearing caveats only) — funding, guild credit, external dates.

## 24. Iteration 17 findings — JetBrains Junie + AI Assistant (scout + 2 detail agents + synth; proprietary, 2026-09-11)

Queue item 17. JetBrains' agent stack — AI Assistant 2026.2, Junie (IDE agent; GA 2026-06-17 after ~17 months in beta), Junie CLI (beta 2026-03-09, LLM-agnostic), Junie Local (2026-08-24, on-device Qwen3.6-27B), Air (multi-agent ADE, public preview 2026-03). Harness proprietary (JetBrains EULA; GitHub repos are distribution/issues only) while Mellum2 model weights are open. REWRITES the stale scout-only §24; architecture + community/economics detail agents re-verified headline facts against primary sources (jetbrains.com help 2026.2 line, blog.jetbrains.com Mar–Sep 2026, junie.jetbrains.com docs, support-forum full thread). Community-agent research date 2026-09-12.

### 24.1 Identity, lifecycle, one-engine convergence
- **GA June 2026** ("out of beta"; closed preview 2025-01 → CLI beta 2026-03 → GA 2026-06-17; GA post pre-empts cynicism: "This is not a rename or a repackage").
- **One engine, many surfaces (VERIFIED):** "The same agent is behind the AI chat, the dedicated Junie tool window, and Junie CLI. Improvements ship once and show up everywhere." Mechanism: IDE↔agent integration rebuilt on ACP at GA — "the same protocol Junie CLI uses to talk to your IDE." Precursor: Dec 2025 conceded the standalone Junie UI would be deprecated into the AI Chat agent selector.
- **AI Credits tiers (VERIFIED, licensing docs rev 2026-07-22):** Free $0 → 3 credits/30d; Pro $10/mo → 10; Ultimate $30/mo → 35; org per-user pays double/gets double; JetBrains Central org pool (All Products Pack +20/mo); 1 AI Credit ≈ $1 USD; annual Ultimate = 420 credits/$300 (~40% bonus); top-ups valid 12 months; no top-ups in Mainland China.
- **Junie CLI beta (2026-03-09):** LLM-agnostic; four provider classes — **Junie** (subscription/API key), **BYOK** (openai, anthropic, google, xai, openrouter, copilot), **Custom** (profile JSON; includes Junie Local), **Proxy** (custom endpoint); env vars JUNIE_MODEL / JUNIE_LLM_PROVIDER / JUNIE_EFFORT (minimal…max); per-provider **internal helper model** (summarization/routing/memory-extraction/capability-filtering); aliases pinned by internal evals (sonnet→Claude Sonnet 5, opus→Claude Opus 4.8, gpt→GPT-5.4, gpt-codex→GPT-5.3-codex, gemini-pro→Gemini 3.1 Pro Preview, gemini-flash→Gemini 3 Flash, grok→Grok 4.3); one-click migration from Claude Code/Codex; real-time mid-run steering; next-task prediction.
- **Junie Local (2026-08-24, VERIFIED):** `/local` downloads Qwen3.6-27B 4-bit (~20 GB) — free, no account, fully offline; HARD FLOOR = Apple M5 + 64 GB RAM (JetBrains pre-concedes "a big ask… out of reach for many"; DGX Spark + RTX 5090 prototypes). Qwen3.8 rejected (needs reasoning → ~4–5x slower, loop-stuck failure mode).
- **Air (public preview 2026-03; Windows 2026-06; Linux by 2026-08):** "Agentic Development Environment… IDEs add tools to the code editor, while Air builds tools around the agent." Workspaces bind agent sessions/Git state/tools per project; Tasks run in parallel w/ attention notifications; agent providers Claude Agent/Codex/Gemini CLI/Junie out of the box + ACP (Copilot/OpenCode/Pi/Cline); execution environments Local / Git-Worktree / Docker; **cloud sandboxes = tech preview**; per-task permission modes incl. Plan-approval; review-with-agent as a fresh second agent.

### 24.2 Junie agent architecture
- **Plan mode = committed artifacts (VERIFIED):** the plan is "a first-class artifact" — structured doc (product requirements / technical design / delivery stages / optional testing strategy) that "lives in `.junie/plans`. You can commit it, and it becomes living task documentation." Flow: multiple-choice + freeform clarifying questions → plan doc → user edits → **Confirm** gate → implementation. "Plan on a strong model; implement on a cheap one" is the explicit cost lever. Shift+Tab enters Plan; Ctrl+P opens the plan.
- **Agentic debugging (VERIFIED, mechanics documented):** Junie Debug mode drives the IDE's real debugger — breakpoints anywhere incl. **decompiled .class files and sources inside JARs**, stack frames, thread state, expression evaluation — through a **specialized MCP toolset** (bundled MCP Server + Debugger MCP Toolset plugins, IDE ≥ 2026.1.1) against the IDE's own MCP server at `http://127.0.0.1:64342/stream`. Three postures: autonomous ("debug why this test fails only on the second iteration"), co-op ("prepare the debugger, I'll trigger the UI flow"), hand-off ("tell me why this value becomes null"). Debug mode does not modify source unless asked.
- **Toolbox inversion:** "Your IDE, the agent's toolbox" — semantic index, build configurations, test runners, debugger, DataGrip database sessions (write/fix/validate SQL in-session) — not the agent's own approximation. Agent loop: task → LLM → tool calls (bash/read/write) → results; cloud path re-requests file contents freely; local path keeps a rolling context so KV-cache survives across tasks; progress updates are an XML-ish block the model emits (model-specific adaptation for Qwen).
- **Remote control / async (VERIFIED):** "Start a task from your laptop. Check progress from your phone." Async sessions resumable from any signed-in device; target uses: Spring Boot upgrades, record migrations, legacy test-coverage sweeps.
- **Code review (VERIFIED):** GitHub Actions (JetBrains/junie-github-action), GitLab incl. on-prem, or `/review` in CLI/plugin; unstaged/staged/diff-vs-main; interactive inline accept/reject; follow-up questions reorder the review.
- **Trust ladder (VERIFIED):** per-operation approval default (Yes/No/Always-allow; reviewable diffs pre-approve; "Open in editor" pre-run; per-file and all-files rollback) → **Brave mode** (skip confirmations) → **Auto** (agent decides); `.aiignore` respected; AGENTS.md instructions; enterprise: JetBrains IDE Services/Central can preconfigure MCP servers, block user-added servers, curate/disable ACP registry agents, block custom agents; governance post (Jun 2026): incremental autonomy "treat agents like new hires", risk-scored human checkpoints, audit-trail requirements.

### 24.3 ACP adoption + registry co-stewardship with Zed
- JetBrains adopted Zed's Agent Client Protocol (~Oct 2025), then **co-launched the ACP Agent Registry with Zed (Jan 2026)**; the registry is integrated directly into JetBrains IDEs 2025.3+ (AI Chat agent-mode selector → "Install From ACP Registry"). ACP works WITHOUT a JetBrains AI subscription.
- Install flow: IDE downloads agent files and **auto-manages Node/Python runtimes** (reuses compatible existing runtimes); auth on first use; update/uninstall from Settings | Tools | AI Assistant | Agents. Custom agents via `~/.jetbrains/acp.json` (`agent_servers` w/ command/args/env; IDE auto-detects locally installed ACP agents; docs examples: opencode, goose, auggie). **Limitation: no WSL support currently.**
- External agents run as local subprocesses of the IDE; "connect local, remote, or in-house agents" is the ACP pitch, but JetBrains docs only document local spawn (remote-hosting depth UNVERIFIED).
- Strategic arc: Junie's own IDE integration rebuilt ON ACP at GA; ReSharper 2026.2 = "first step toward ACP-based agent support in Visual Studio" (Junie Preview); Air launched agent-agnostic on ACP. The IDE repositions from "our AI" to a host for ANY ACP agent.
### 24.4 MCP + extensibility (incl. the external-agent gap)
- **Client:** STDIO + Streamable HTTP (local or remote) + SSE (legacy); JSON `mcpServers` config, global or project scope; **Claude-Desktop config import**; one-click install of popular servers; CLI can *recommend* an MCP server relevant to the task; per-server status/logs; **router-only tools** hidden from the direct tool list to save context (surfaced via a router tool); **MCP brave-mode** (external clients may run shell/run-configurations without confirmation).
- **Server — IDE-as-MCP-server, bundled since IDE 2025.2 (the sleeper feature):** external clients auto-config: Junie, VS Code, Claude Code, Codex, Air, GitHub Copilot CLI. Exposed tools (toggleable per tool): `analyze_calls` (call hierarchy incl. **decompiled JAR symbols**), `build_project`, `get_file_problems`, `lint_files`, `get_symbol_info`, database toolset (execute_sql_query etc.), debugger toolset (powers Junie Debug mode). Terminal-session setup banners for Codex/Claude.
- **The gap, closed opt-in by 2026.2:** historically, IDE-configured MCP servers were NOT consumable by external ACP agents (YouTrack LLM-20452 documents the request; body JS-rendered UNVERIFIED, existence VERIFIED). 2026.2 ACP agent settings add "Pass custom MCP servers" + "Pass IntelliJ MCP server"; `acp.json` `default_mcp_settings`: `use_custom_mcp` default **true**, `use_idea_mcp` default **false** (+ optional `idea_mcp_allowed_tools` allowlist). Remaining friction: **local models cannot invoke tools from configured MCP servers** ("AI Assistant does not support invoking tools from configured MCP servers when using local models" — help snippet, VERIFIED); the CLI's MCP story is per-run rather than IDE-shared (UNVERIFIED).
- **Models/extensibility:** cloud catalog = Claude 5 family (Fable 5 / Opus 5 / Sonnet 5, 1M ctx) + GPT 5.6 Luna/Sol/Terra (1M) + Gemini 3.x + Grok-4.3 (supported-llms.html, updated 2026-08-06); BYOK 6 providers; local = Ollama/LM Studio (AI Assistant), LiteLLM (CLI), Junie Local bundled. Default-model churn: Gemini 3.7 Flash default @ 40% off (2026-08-17), Gemini 3.8 Flash promo (Sep 2026); agent skills in CLI + IDE (bundled e.g. `ij-debugger`; installable into external Claude Code/Codex dirs).

### 24.5 Economics + community
- **Credit burn (VERIFIED quotes):** 1 credit ≈ 10 chat code-gen requests / 40 inline completions / 25 explains / 140 commit messages; support concedes "a single credit cannot be mapped to a fixed number of tokens or calls" — dynamic per-request metering over tokens, model, context size, and internal agent calls. Documented burns: 2 credits for a ~3-minute trivial task; 15 purchased credits gone in 2 questions; full Pro quota consumed in 30 minutes; free-tier 3 credits "gone in a couple minutes" (support: "expected"); benchmark-level $0.81–$1.14/task ⇒ ~30–43 agentic tasks/month on Ultimate.
- **Origin wound — Aug 25, 2025 quota reset:** quotas cut to real per-token cost; Sep 16, 2025 damage-control FAQ admits "the new quotas are a reduction from what you experienced… We set expectations incorrectly. That is on us" + "about 80% of users will not hit the new limits"; staff concede credits are "directly tied to their own API costs." Canonical complaint thread (31142845556370, Nov 2025 → Sep 2026, still active) shows "open a ticket with logs" boilerplate ten months on, while JetBrains' OWN transparency feature requests (LLM-16701 per-request cost tracking; LLM-19959 usage report) remain open.
- **SWE-Rebench #1 (VERIFIED claim + methodology):** GA pull-quote from Alexander Golubev (Nebius): "top model-harness, 61.6% resolved, 72.7% pass@5" — immediately disclaimed "results move from run to run." Live board already shows a different cycle (61.8% ±0.54 / 73.9% pass@5 / $0.81 / 1.68M tokens / 85.9% cache-hit — third-party citation, UNVERIFIED vs live board). Nebius methodology genuinely strong: fresh mined GitHub issue→PR pairs, date-based decontamination flags, one fixed ReAct scaffold, 5 runs + SEM + pass@5. Caveats: JetBrains chose the submitted harness and never disclosed which model powered the #1 run; Nebius solicits vendor funding (Google/OpenAI/Anthropic/MiniMax/StepFun credits disclosed).
- **"From Leaderboards to Model Profiles" (2026-08-31, VERIFIED full text):** 8 weeks after headlining the leaderboard, JetBrains argues resolve rate is the wrong decision variable — 4-perspective trajectory pipeline (outcome/efficiency/patch/process), 523 tasks, LLM judges: Opus 4.7 skipped executable validation in 123/523 runs (68 still "resolved"); Gemini 3.5 Flash 37.3% moderate/severe hallucination vs Opus 24.7%; GPT-5.5 = highest resolve 51.5% + only model always running an executable check; Qwen3.6-27B FP8 = 38.9% at 3% of GPT-5.5 cost. Read cynically: retroactively reframes the #1 claim AND justifies the per-model-credit routing dial.
- **Junie-vs-Claude-Code sentiment:** IDE-native integration universally praised; **credit opacity/burn is the #1 complaint across every thread and the support forum Nov 2025 → Sep 2026, unbroken**; Claude models "completely unusable except for short questions" at credit prices; Gemini 3 Flash is the staff- and community-endorsed cheap default; "the $200 sub you get at Anthropic gives you ~$8k worth of inference" — competitors subsidize, JetBrains resells near-API rates. Exit paths: Claude Code in-IDE, Codex auth, Copilot plugin, Windsurf plugin. Quality verdicts split ("Junie is not very good and uses credits in a very unpredictable manner" vs "Junie + Opus is awesome if you know how to prompt"). Junie Local reception niche-positive, hardware-gated (r/LocalLLM: dense 27B on Apple Silicon ~10–30 tok/s; M5 prefill-optimized corroborates JetBrains' prefill-first story).
- **Cost dial framing (GA + Local posts):** "Cost efficiency stops being a property of the tool and becomes a dial you hold… Junie Local turns that dial down to zero."

### 24.6 Notable concepts
- **IDE-as-toolbox inversion:** the agent drives the IDE's real semantic index / build configs / test runners / debugger / databases over a localhost MCP endpoint instead of reimplementing grep-repomap + LSP approximations. Supersedes tree-sitter repomap approaches — but only inside JetBrains IDEs (CLI alone lacks it unless pointed at an IDE MCP server).
- **Plans as committed artifacts + plan-approval gate:** a governance boundary (implementation blocked until the `.junie/plans` doc is Confirmed) that doubles as living task documentation.
- **Engineered prefix discipline:** system prompt + stable context ordered first to maximize the cached initial prefix up to the user request; KV-cache survives across tasks in a session (rolling context on the local path). Best-documented prefix engineering in this survey.
- **Per-provider helper models:** an internal LLM handles summarization/routing/memory-extraction/capability-filtering alongside the primary model.
- **Prefill-first local inference:** agent time is dominated by file reads, not generation — int8 prefill patch (~40% M5 prefill throughput), MTP + n-gram simultaneous speculative decoding (~2x generation), `reasoning_effort: none`.
- **Snapshot-then-counter-program eval PR:** headline a leaderboard rank, then publish the methodology post arguing leaderboards are the wrong metric — while per-model credit routing monetizes the behavior differences.
- **Registry stewardship:** co-owning the ACP registry makes the IDE a distribution tollbooth for third-party agents.

### 24.7 Matrix row + corrections (col 21)
Full-row evidence in scratch (iter17-jetbrains-architecture.md "FULL MATRIX ROW"; row tally ● 13 · ◐ 10 · ○ 2). Applied to §1 col 21 — 8 score flips, each tagged `[corrected from …]`:
| Row | Flip | Basis |
|---|---|---|
| OS sandbox | ○→◐ | Air Docker + Git-worktree isolation per task; cloud sandboxes tech preview; IDE Junie/CLI unsandboxed on host |
| Prefix-cache | ◐→● | engineered max reusable initial prefix + KV-cache reuse across tasks |
| Repo map | ◐→● | semantic index + PSI + `analyze_calls` call-hierarchy incl. decompiled JAR symbols + symbol-level @ mentions |
| Windowed file view | ●→◐ | auto-context = current file + selection only; @-attach for more; no paging layer |
| PTY | ○→◐ | reviewable terminal commands + approval widget; Air built-in terminal; no interactive PTY |
| Tiered eviction | ○→◐ | selection-based compaction + helper-model summarization; local rolling context; no tiered ladder |
| Model-weakness profiles | ◐→● | per-alias pinning via internal trajectory evals + documented rejects (Qwen3.8) + model-profiles research program |
| Auto skill learning | ○→◐ | internal memory-extraction helper task + next-task prediction; persistence UNVERIFIED |
Plus 2 **misalignment fixes**: the MCP-client and Sessions rows carried Warp/Droid text in the JetBrains cell (ledger note added).

### 24.8 ADOPT / AVOID / WATCH deltas
- ADOPT: IDE-as-toolbox inversion via localhost MCP; plans as committed `.junie/plans` artifacts w/ Confirm gate; engineered prefix discipline (max initial reusable prefix + cross-task KV-cache reuse); agentic debugging incl. decompiled symbols; per-alias model pinning by internal trajectory evals (re-validates existing ADOPT); ACP-registry agent install w/ IDE-managed runtimes.
- AVOID: dollar-denominated expiring credits at ~API parity while competitors subsidize (burn backlash → cancellations); quota-cut damage control ("that is on us") followed by a year of support boilerplate while own transparency tickets stay open; silent default-model swaps.
- WATCH: IDE-as-MCP-server pattern spreading (bundled 2025.2; six external clients auto-config); opt-in MCP-passing defaults flipping (`use_idea_mcp` false→true); SWE-Rebench ranks as per-cycle snapshots; cloud Junie multi-agent mode depth (UNVERIFIED); Air public control-plane API (UNVERIFIED); M5+64GB-class hardware floors for on-device agents.

### 24.9 Sources + confidence
- Primary (VERIFIED, fetched): blog.jetbrains.com/junie — 2026/06 out-of-beta GA · 2026/08 junie-local-launch · 2026/08 qwen-for-junie · 2026/08 from-leaderboards-to-profiles · 2026/08 junie-gemini-3-7-flash · 2026/03 junie-cli-beta; junie.jetbrains.com/docs/junie-cli-model-selection.html; jetbrains.com/help/ai-assistant — junie-agent.html · mcp.html · acp.html · supported-llms.html · licensing-and-subscriptions.html; blog.jetbrains.com/ai — 2025/08 quota model · 2025/09 FAQ-new-ai-quota · 2026/01 acp-agent-registry · 2026/06 governance; blog.jetbrains.com/air 2026/03 preview + help/air quick-start; swe-rebench.com/about; support thread 31142845556370 (full text).
- Secondary/corroboration: zed.dev/blog/acp-registry; jetbrains.com/acp; ReSharper 2026-2 release post; devops.com + adtmag headlines; r/Jetbrains + r/LocalLLM threads (search-index snippets); github.com/JetBrains/mlx-vlm feature/int8-prefill; JetBrains/junie-github-action; YouTrack LLM-20452 / LLM-16701 / LLM-19959 (existence VERIFIED, bodies JS-rendered UNVERIFIED).
- UNVERIFIED (flagged): live SWE-Rebench numbers (61.8% / $0.81 / 1.68M tokens); AltAI $1.14-vs-$4.91 cost comparison; Jan-2025 53.6% retro-baseline; Air public control-plane API; remote ACP hosting depth; local-model MCP invocation beyond the quoted help snippet; org-tier credit universality; silent-model-switch allegation.
- Confidence: HIGH on architecture / permissions / MCP / ACP / economics (primary docs + full-text posts, current to 2026-09); MEDIUM on benchmark claims (vendor-pulled per-cycle snapshots with disclosed methodology); LOW on nothing load-bearing.

## 25. Iteration 18 findings — Warp (scout + 3 detail agents + synth; open client MIT+AGPL / closed cloud, 2026-09-11)

Warp (warp.dev, NYC; founded 2020 by Zach Lloyd; $73M total, Series B $50M Sequoia 2023, NO 2026 round) is the terminal-as-runtime harness: a Rust GPU terminal repositioned to "Agentic Development Environment" (Warp 2.0, 2025-06-20), then multi-harness agent platform, client open-sourced 2026-04-28 (AGPL-3.0 + MIT UI crates; OpenAI founding sponsor). This section REWRITES the scout stub; verified 2026-09-11 against a blob:none source clone @ HEAD 4143c09ff8be80ff73165f36085cf4a013295a01 (~86 crates + app/ GUI crate + agents/specs), a depth-3000 history clone, the docs-repo changelog MDX, and live web/GitHub/HN data. Intelligence is deliberately CLOUD-SPLIT — SSE/protobuf inference, transcripts, schedules, memory, secrets are server-side — so capability parity must be audited per-surface, not from the repo. Matrix deltas applied to the §1 Warp column (9 score flips; recount 15●/8◐/2○); ADOPT/AVOID/WATCH extend the §3 sec25 segments in place.

### 25.1 Identity, lifecycle, Oz rename
- Repo facts: public 2026-04-28; root commit 0dbd3d5 "Initial public release" by vorporeal = +1,431,549 lines / 300 files in ONE squashed commit, NO preserved internal history; the first ~2 weeks of layered commits carry pre-announcement dates (W17 = 165 commits vs the Apr 28 blog — repo populated days before launch; plausible, not proven). 2,368 commits Apr→Sep (fetch-bounded), avg ~125/wk, peak W19 240; releases = weekly stable_release/preview_release branch pairs (no tags; 53 GitHub Releases by github-actions[bot]); PR numbering passed #15,950 in ~4.5 months. 64,962★ / 5,528 forks / 5,232 open issues / 120 contributors (2026-09-11).
- **The open repo is a mirror**: .github/workflows/repo-sync.yml hardcodes private_repo warpdotdev/warp-internal — client work happens in the private monorepo and syncs out on every push (private-to-public/* transport branches); internal Jira IDs (APP-*, QUALITY-1792, GROW-6090) ride along in public commits. FAQ: the server, Drive backend, and Oz "are not in this repository and remain proprietary"; the built-in agent harness runs server-side ("haven't committed to a date" on opening it).
- Licensing (verified in-tree): MIT for warpui + warpui_core ONLY ("maximum reuse"); AGPL-3.0 for everything else ("closes the network-use loophole" — closed forks/hosted derivatives barred; company use as a terminal fine).
- Oz→Automation Platform rename has a HARD KILL DATE: docs platform/index.mdx — "Oz is now the [Automation Platform]. Only the name changed… the oz CLI and web app keep the Oz name until October 6, 2026" (docs commit 5ddf5a5 corrected the date); client renamed "Oz agent UI" → "Warp Agent" 2026-08-13 (#11022); 2026.09.09 renamed Oz Cloud API Keys settings → "API keys" (#15424). In code: dual OZ_*/WARP_* env vars injected identically (warp_cli lib.rs); legacy oz aliases remain in schedule/task/federate help. TUI binary renamed warp-tui → warp (#13904).
- Warp Agent CLI standalone (HN 49171766, 111/63 — harness-neutrality play beyond the Warp terminal): crates/warp_cli subcommand families agent/task/schedule/environment/skill/memory/mcp/secret/model/provider/federate/share/artifact/local_control/harness_support/runner; json/ndjson/pretty/text output; runs in any terminal over SSH.
- Agents write Warp: warp-agent-staging[bot] authored 132/200 (66%) of recent master commits; a 2026.04.29 setting toggles whether Oz adds an attribution co-author line to commits/PRs; OpenAI (2026-05-27) claims ~90% of Warp's PRs are agent co-created. Externals land real features: Xavientois #15148 per-category dollar pricing breakdown + #15533 credits/dollars toggle; bbbiggest #15566 Windows RDP renderer recovery.

### 25.2 Terminal + agent architecture (blocks, PTY runtime, cross-harness)
- Blocks: the conversation surface is a blocklist (app/src/ai/blocklist/, 222 files); AIBlockModel block.rs = 7,338 lines (queries, outputs, diffs, todo ops, subagent calls, secret redaction, citations); terminal commands are blocks too (terminal/model/block.rs BlockId/SerializedBlock); orchestration_topology.rs = parent→children conversation tree (orchestrator + named agents) with CREDIT ROLLUP over child agents.
- Driver: agent_sdk/driver.rs (4,612 lines) wires history/document models, permissions, MCP, skills, checkpoints, ambient tasks. Warp's own agent runs as a TerminalDriver — it acts through a REAL PTY inside the terminal ("Full Terminal Use", Agents 3.0 2025-11-19: REPLs, debuggers, full-screen apps); planning notebooks surface via ai::document.
- Cloud-split inference: crates/warp_multi_agent_client POSTs a protobuf Request and consumes an SSE stream of protobuf ResponseEvents — no model loop in the client; prompt caching server-side (UNVERIFIED from OSS). "Mux" (marketing term for multi-session orchestration) materializes client-side as run-to-run messaging + ambient task spawning + shared-session join links; no literal mux symbol in the client.
- Cross-harness integration (the standout — deepest OSS-visible integration layer of any terminal): OSC 777 structured JSON notifications emitted by plugins inside CLI agents, parsed from the PTY (OSC 9 plain-text fallback for Codex); listener tracks Claude, OpenCode, Codex, Gemini, Auggie, Droid, Pi, OhMyPi, Grok, WarpTui; session status machine InProgress/Success/Failed/Blocked/Cancelled with a 2s Ctrl-C grace window; per-harness plugin managers install marketplace plugins (claude-code-warp min 2.1.0 + oz-harness-support 1.1.2; siblings codex/gemini/opencode); third-party harness RUNNERS drive rival CLIs as processes — ClaudeHarness/CodexHarness/GeminiHarness with ResumePayload transcripts from server endpoints, exit_escalation, process_control, skill-dir publishing, parent_bridge, wake_driver.
- Permissions: typed allow/deny WITH REASONS — CommandExecutionPermission (ExplicitlyAllowlisted, IsReadOnlyAndSettingEnabled, AgentDecided, RunToCompletion, ContainsRedirection…), same for FileRead/FileWrite, plus AskUserQuestion + WriteToPty; server-synced AIExecutionProfile + workspace/team AiAutonomySettings. NO per-command OS sandbox in the client (no seatbelt/apparmor/bwrap found) — local runs are permission-gated only; isolation = Docker/DockerSandbox/Kubernetes/Namespace for cloud/self-hosted runs.
- Rendering/frontends: wgpu 30 + WGSL + cosmic-text (Warp fork) + winit; Entity-Component-Handle core shared by the GUI (app/), a headless TUI (crates/warp_tui cell-grid + crossterm), and web/WASM (cfg(target_family="wasm") throughout, serve-wasm + warp_js + web event bus → build.warp.dev web terminals); terminal emulation from Alacritty; local SQLite (diesel) session persistence.
- Checkpoints + cost: periodic workspace CheckpointCoordinator (5-state machine, safe-boundary gating, HarnessSupportClient upload) + end-of-run snapshots + file-rewind dialogs; credit/dollar usage model with RequestLimitInfo refresh, denial reasons, bonus grants, ambient-trial CTA threshold, child-agent credit rollup; credits|dollars unit setting (external PR #15533).

### 25.3 Automation Platform (ex-Oz) + Factories
- Cloud agents = "ambient agent tasks": app/src/ai/ambient_agents/ spawn.rs streams SpawnAgentRequest/RunFollowupRequest with 3s status polling and shared-session join links. Triggers: warp schedule create|list|update|pause|unpause (cron; schedule targets a WORKER HOST ID for self-hosted vs Warp-hosted jobs). Cloud environments: warp environment create --docker-image --repo --setup-command.
- Isolation platforms Docker/DockerSandbox/Kubernetes/Namespace + workload tokens; repo-level build caching ONLY on Namespace. Agent-to-agent messaging: task message send|watch|list|read|mark-delivered (multi-recipient runs, inbox). Managed secrets: envelope encryption (UploadKey, init_envelope) + GCP federation + per-task identity tokens. federate = OIDC workload-identity federation (issue-token with subject templates principal/email/teams/environment/agent_name/skill_spec/run_id/host) — the same OIDC-agent-identity pattern Cursor ships as a socket.
- Factories (EA, Aug 2026 — NOT in OSS client): README claims factories are "defined in code", power the Warp repo itself, with "evals, benchmarks, and self-improvement built in"; dashboard build.warp.dev (web-compiled Warp terminals). In-repo evidence: agents/specs/*.md Factory-generated specs with ticket IDs, .warp/workflows/*.yaml. No Factory config schema in OSS — UNVERIFIED beyond README. Pricing: PAYG at a 20% markup over API rates; paid plans consume plan credits at API rates (markup effectively waived); Enterprise = self-hosted workers + BYOLLM.
- "Software factory" doctrine: Jun 18 2026 internal memo — engineers as "factory engineers" whose job is "to build the system that builds the product" (via RuntimeWire); Sequoia-moderated "Crafting Software Factories" livestream with OpenAI (Steinberger) + DeepMind (Bailey).

### 25.4 Knowledge & context
- Warp Drive: cloud objects (folders, sharing, workflows, import/export, trash) + cloud_object* persistence crates. Rules: WARP.md + AGENTS.md as repo rule files (project_rule_file_names standing query), init flow checks/migrates CLAUDE.md, per-harness global rules dirs, readable over SSH via remote context files.
- Codebase indexing: repo_metadata gitignore-aware standing queries + full-source-code EMBEDDINGS with Merkle-tree snapshot sync (crates/ai/src/index; telemetry emits Merkle rebuild/diff metrics); tree-sitter file outlines — a cloud-synced data structure, not an aider-style local AST/PageRank map.
- Skills: .agents/skills (project) + ~/.agents/skills (global), skills-lock.json via npx skills, SkillManager + HOT-RELOAD file watchers, bundled skills in resources/bundled/skills (incl. claude-api), Drive-remote skills, skill_dirs_publish to harnesses.
- Agent Memory (research preview): cloud MEMORY STORES with agents attached; memory CRUD with version history (warp_cli memory_store.rs) — cross-harness memory as a managed cloud primitive rather than a local file.
- MCP: official Rust rmcp 1.6 client (SSE + OAuth); FileBasedMCPManager scans config files across providers (per-repo + global), hash-keyed TEMPLATABLE installations, per-path parse diagnostics, deferred auth-gated autostart, gallery, reconnecting peer; repo root ships .mcp.json; project-scoped servers spawn from repo root (2026-04-27). Warp-as-MCP-server: no OSS evidence (Factory MCP = closed platform docs only).

### 25.5 Open-source governance + privacy archaeology
- Governance: issues-first CONTRIBUTING with readiness labels (ready-to-spec/ready-to-implement/needs-mocks/warp:reserved-internal — Oz auto-rejects contributor PRs on reserved issues); feature work via in-repo spec PRs (specs/GH<n>/product.md + tech.md); Oz auto-reviews every PR, then a human SME is auto-requested (/warp-agent-review re-review, 3× max); "collaborator" role gets @oz dispatch + complimentary Oz credits; STAKEHOLDERS (CODEOWNERS format) + check_approvals.yml enforce owner approval per path. **CLA DISCREPANCY**: FAQ describes a CLA; no CLA file/signing flow/bot exists in-repo — mechanism UNVERIFIED.
- OpenAI sponsorship: every primary source says "founding sponsor" (agentic workflows powered by GPT-5.5); Wikipedia's "financial support from OpenAI" traces to ONE Fast Company piece — cash terms UNVERIFIED. Lloyd: "We sort of want to be Switzerland… but having this support for the agents, because it's expensive, is really valuable." Wikipedia's claim that Oz was open-sourced CONTRADICTS Warp's own FAQ — treat as low-reliability. Criticism on record (Kilo Code, via forkable.io): routing the recommended contribution path through proprietary Oz + a single AI provider undercuts the open-source credentials; foundation-donation calls predicted (FOSS Force).
- Privacy asymmetry baked into pricing: docs privacy page — AI interactions pass through Secret Redaction "unconditionally", BUT "Telemetry must be enabled to use AI features on the Free plan, while paid plans can opt out at any time"; Business/Enterprise ZDR = "No AI interaction or console data is ever collected." The free tier pays for AI with analytics; the open client lets auditors verify what is SENT, not opt everyone out. Training pledge: repo contributions/discussions NOT used for model training (FAQ; a petra/faq-openai-and-training branch shows this language being actively edited).
- Network Log arc: native tail -f warp_network.log → post-open-sourcing (2026.04.27) replaced by an in-app read-only pane over a 50-item in-memory buffer; documented gap: Sentry crash traffic NOT captured (SDK encapsulates networking — disable via Settings). Telemetry stack: Sentry + Rudderstack, exhaustive event-name table published, X-Warp-Telemetry-Enabled header (server assumes disabled when missing). Trust history: 2022 opt-out shell telemetry + mandatory login (HN 33910992, 42247583 — relief threads); Aug 2025 Active AI sent terminal sessions to LLMs without explicit opt-in (HN 44953470, 94/31, "breach of trust").

### 25.6 Benchmarks + economics
- Verified arc: Terminal-Bench #1 52% on v0.1.1 (2025-06-25; vendor-run, full methodology disclosed, ADMITTED hill-climbing "65% across all runs"; maintainer-acknowledged at tbench.ai) → Nov 2025 50.1% "back on top" (TB 2.0) → 59.1% #1 with Gemini 3 Pro (Agents 3.0) → 61.14% with GPT-5.2 "#2" → LAST LEADERBOARD ENTRY 2025-12-12. SWE-bench Verified 71% (2025-06-23; INDEPENDENTLY REPRODUCED by maintainers, experiments PR #278 merged) → 75.8% #3 (2025-09-01, GPT-5, PR #321 merged; explicit reproducer comment not captured). **2026 refresh ABSENT (confirmed)**: no submissions; TB 2.1 leaders (Claude Code/Codex/Terminus 2/Cursor CLI) left Warp "outside the leading group" (third-party). Pivot: "public benchmarks are poor proxies/contaminated" + private replay — no published independent analysis covering the public benchmarks it disavows (runtimewire critique).
- Factory Benchmarks (2026-09-03): replay stored traces of your team's runs, vary model/harness/scorers, LLM-as-judge, results feed custom model routers; WarpBench case study (2026-09-02): 30 replayed internal tasks, run cost $2,130.57, switched default Opus 5 → Grok 4.6 high, cost/PR ~$80 → ~$30 (−63%, self-reported, "needs outside proof"); cross-harness scoring "(soon)". 2M+ agents/day, 100M LOC/week, 97% acceptance = self-reported vendor metrics, no methodology or audit.
- Pricing (verified): Free $0 with NO bundled AI credits (BYOK; add-on packs ~20% higher); Build $20/mo = 1,500 credits ("$20 of agent usage at API rates" = par); Max $200/mo = 18,000 credits (Warp values the pool at $240 ≈ 17% prepay discount); Business $50/user (+$30 over Build buys SSO + ZDR; 25-seat cap); Enterprise BYOLLM + self-hosted workers + invoicing. Reload add-ons 400/$10 → 6,500/$100 (−40%), roll over 12 months; PLAN credits do NOT roll over (breakage). Oct 30 2025 repricing killed Pro/Turbo/Lightspeed (HN 45772558, 39/85); BYOK later extended to Free; custom inference endpoints Business+; SuperGrok/X-Premium inference on Free.
- Three-bucket metering: AI/inference + compute (cloud sandbox) + platform (run lifecycle, billed by AGENT-HOUR). Platform credits apply to EVERY cloud run regardless of harness (Warp Agent/Claude Code/Codex), inference source (Warp/BYOK/BYOLLM), or compute (Warp-hosted/self-hosted); direct CLI use of claude/codex in the terminal = ZERO Warp credits ("do not tax what we do not orchestrate"). Margin model: inference is pass-through → margin from the Factories 20% markup, platform agent-hours, the Business seat premium, Free-tier add-on surcharge, and breakage. Central tension vs flat subscriptions (Claude Max): the dominant r/warpdotdev complaint ("burned my credits in 3 days"); defection pattern to Claude Max; BYOK = retention valve with zero inference revenue.
- Funding + strategy: $73M total (Seed $6M GV 2020; A $17M led by Dylan Field 2022; B $50M Sequoia 2023, Andrew Reed board; angels Altman/Benioff/Weiner/Gil/Lutke); NO 2026 round (PitchBook still lists B) — OpenAI sponsorship reads as a non-dilutive substitute. ~63k stars + hundreds of contributions/week ~2 months post-OSS (Lloyd, 2026-07-01). Neutral-host strategy: ship AND host rival harnesses (Claude Code/Codex/Gemini/OpenCode first-class); ACP explicitly on the roadmap (issue #9233 — "bring your own harness… using that harness's subscription with Warp's UI"; OpenRouter discussion #9619 "very soon") — the Zed-ACP play: monetize shell/UI/orchestration while subscriptions stay with the labs.

### 25.7 Notable concepts
1. Terminal-as-runtime inversion: Warp's agent, third-party harnesses, and human operators all act through PTYs observed via OSC escape sequences — ONE observer plane for everything; the terminal IS the harness.
2. OSC 777 structured notifications as the cross-harness bus: in-agent plugins emit JSON events the terminal parses into a session-status machine (with cancel-grace semantics) — richer and more honest than screen scraping.
3. Cloud-split open-core: the OSS client is a (very large) cockpit while inference, transcripts, orchestration, memory, and secrets are server-side — the CLOSED half is the agent loop, inverting the usual open-core shape.
4. Repo-as-mirror governance: repo-sync.yml from a private monorepo with Jira IDs riding along — "open source" as continuous publication, not a development venue.
5. Typed permission REASONS (ExplicitlyAllowlisted/AgentDecided/RunToCompletion…) — the UI can show WHY an action ran, not just that it did.
6. Credit rollup over child agents in the orchestration tree — cost accounting follows the agent graph.
7. Agent Memory as versioned cloud memory stores with agents attached — memory becomes a managed multi-tenant primitive, not a local file.
8. Full-source embeddings + Merkle-tree snapshots — codebase context as a syncable data structure with incremental diff telemetry instead of a local repomap.
9. Factory Benchmarks: private replay-benchmarks from stored traces feeding custom model routers — evaluation infrastructure as the product surface once public benchmarks are lost.
10. OIDC workload-identity federation for agents (federate issue-token with subject templates) — agents as first-class workload principals.
11. Platform agent-hour billing that applies to ANY harness in your cloud — monetize the control plane, not the tokens.
12. Privacy-as-pricing asymmetry: telemetry mandatory on Free, purchasable opt-out above, ZDR at enterprise — the open client makes the wire format auditable while the policy stays tiered.

### 25.8 Matrix row + column corrections (§1 Warp column updated in place; recount 15●/8◐/2○)
- Score flips (9): language ◐→● (100% Rust OSS client verified @ 4143c09; warpui/warpui_core MIT, rest AGPL-3.0); steady RSS ◐→○ (wgpu GUI + headless TUI; no figures, not measurable from source); permission tiers ◐→● (typed allow/deny + reasons across 5 action types + execution profiles); windowed file view ○→● (line_range start/end on agent file actions); tiered eviction ○→◐ (summarization-based compaction, server-side assembly); model profiles ○→● (server-synced AIExecutionProfiles drive permissions AND models); WASM ○→● (cfg(target_family="wasm") throughout, serve-wasm + warp_js, build.warp.dev); checkpoints ◐→● (periodic workspace CheckpointCoordinator 5-state safe-boundary machine + file rewind); evals ●→◐ (no eval harness in OSS client; last public submission 2025-12-12; Factory Benchmarks = closed replay product).
- Content corrections without score flips: OS sandbox (+no seatbelt/apparmor/bwrap in client — permission-gated locally; isolation_platform Docker/DockerSandbox/K8s/Namespace); prefix-cache (+client has no cache_control plumbing; server-side SSE protobuf); repo map (+tree-sitter outlines + full-source embeddings + Merkle sync); PTY (+OSC 777 JSON listener, 10 harnesses, plugin marketplaces); subagents (+orchestration tree w/ child credit rollup, run-to-run messaging); multi-model (+HarnessModelConfig spawn w/ model+harness per child); skills (+hot-reload watchers, skills-lock.json, Drive-remote); auto skill learning (+suggested-rule modal + Agent Memory; no autonomous loop); MCP client (+rmcp 1.6, templatable servers, multi-provider file manager); MCP server mode (OSS shows none; closed Factory MCP docs exist); sessions (+harness resume via server transcripts); encryption (+secrets envelope-encrypted; local SQLite plain — no SQLCipher); cost HUD (+denial reasons, child rollup); frontends (+GUI/TUI/web/CLI one core); multi-client (+shared_session join links); telemetry (+Sentry+Rudderstack, Free-tier telemetry-mandatory).

### 25.9 ADOPT / AVOID / WATCH deltas (detail agents: 12/6/6, extending the §3 sec25 segments)
- ADOPT (12): OSC 777 structured-JSON cross-harness notification bus w/ session-status machine + 2s cancel grace + per-harness plugin marketplaces; typed permission REASONS surfaced in the UI; credit rollup over child agents in the orchestration tree; periodic workspace CheckpointCoordinator w/ safe-boundary gating + file rewind; OIDC workload-identity federation for agent runs (subject templates); per-harness global rules dirs + CLAUDE.md migration on init; skills hot-reload watchers + skills-lock.json pinning; FileBasedMCPManager multi-provider config scanning w/ templatable servers + parse diagnostics; Factory-Benchmarks-style replay-from-traces feeding custom model routers; one wgpu/ECS core across GUI/TUI/WASM/CLI frontends; ambient-task spawn/follow-up streaming w/ join links; run-to-run agent messaging inbox (task message send/watch).
- AVOID (6): telemetry-mandatory Free tier (privacy as paywall; unwind costs years — 2022 combo, Aug 2025 Active-AI incident); unpublishable per-action credit costs + non-rolling plan credits (breakage as revenue); benchmark retreat — stop publishing exactly when rivals pull ahead (last submission 2025-12-12); no local OS sandbox while marketing runs on cloud isolation; squashed-history open-sourcing (no provenance, no archaeology); CLA claimed in FAQ without any in-repo mechanism.
- WATCH (6): Oz→Automation Platform hard rename deadline 2026-10-06 (watch docs/client desync); Factories EA→GA trajectory + 20% PAYG markup economics; ACP support (#9233) — if shipped, Warp becomes a full neutral host; OpenAI sponsorship terms (cash vs in-kind, single-sourced); private-replay-benchmark doctrine spreading (benchmarks as product); OSS-credentials criticism + foundation-donation pressure.

### 25.10 Sources + confidence
- Source clones: warpdotdev/warp @ 4143c09 (blob:none) — README/FAQ/CONTRIBUTING/AGENTS.md, .github/workflows/{repo-sync,label_external_contributors,check_approvals}.yml, app/src/ai/{blocklist,agent_sdk,ambient_agents,mcp,skills,drive}, app/src/terminal/cli_agent_sessions/ (listener + plugin managers + driver/harness), crates/{warp_cli,isolation_platform,managed_secrets,mcp,ai,repo_metadata,warp_multi_agent_client,warpui,warp_tui,persistence,remote_server,serve-wasm,warp_js}, agents/specs, .warp/workflows; history clone depth-3000 (root 0dbd3d5, cadence, branch farm, merged external PRs 787c8697/1c6708dd/ad28e590).
- Official web: warp.dev blog (terminal-bench, swe-bench-verified{,-update}, agents-3-full-terminal-use, introducing-gemini-3-support, gpt-5-2-support-terminal-bench-improvement, warp-is-now-open-source, warp-factory-benchmarks, agentic-development-environment-two-million-agents, introducing-warp-lightspeed, warp-new-pricing-flexibility-byok), newsroom 2026/4/28, pricing, events, factories/benchmarks/warpbench; docs.warp.dev (platform/, privacy, plans-and-billing/{credits,add-on-credits,platform-credits,pricing-faqs}, enterprise trust-center); docs-repo changelog MDX 2024/2025/2026 (mined at source); tbench.ai (news/warp-sota, leaderboard/2.0); SWE-bench/experiments PRs #278/#321.
- Community/press: openai.com/index/warp; forkable.io (Lloyd interview + Kilo Code critique); firstround.com/ai/warp; thenewstack.io; itsfoss.com; fossforce.com; eweek.com; omidsaffari.com pricing audit (2026-08-20); datastudios.org; developersdigest.tech; hackernoon; runtimewire.com; HN Algolia (47936264 = 373/118 OSS launch; 49171766 = 111/63 Agent CLI; 44953470 = 94/31 telemetry; 44704043 = 57/42 credit burn; 45772558 = 39/85 repricing; 33910992, 42247583); r/warpdotdev multi-thread sentiment (credit burn, Claude-Max defection, universal-agent AMA); GitHub issue #9233 + discussion #9619; CB Insights/PitchBook funding profiles.
- UNVERIFIED carried: OpenAI cash terms ("financial support" = single Fast Company piece via Wikipedia); CLA enforcement mechanics; W17 replay hypothesis; server-side prompt caching; Warp-as-MCP-server absence; Factories config schema; independent reproduction of SWE-bench 75.8%; ALL usage/revenue metrics (2M agents/day, LOC/week, 97% acceptance, 19x revenue, ~1M devs); WarpBench $80→$30/PR; the benchmark-contamination claim; free-tier credit numbers via datastudios (third-party); server-side ZDR implementation.
- Confidence: HIGH — client architecture (source @ pinned SHA), governance mechanics (in-tree files), benchmark arc (official submissions + leaderboard), pricing table (pricing page + docs + third-party audit concordant), git archaeology (clone). MEDIUM — privacy posture (docs + press concordant; server behavior unverifiable), economics commentary (derived from disclosed rates), Factories (EA marketing + partial in-repo evidence). LOW (load-bearing caveats) — closed-server capabilities (mux, caching, ZDR internals), Wikipedia-sourced sponsorship claims.

## 26. Iteration 19 findings — Rust micro-harnesses: forge + harness-rs (scout + 2 detail agents + synth; tiny open source, 2026-09-11)

This section REWRITES the stale shallow §26 (the six-project sketch, written before source access). Iteration 19 went deep on the two strongest Rust micro-harnesses — **Adulari/forge** and **liliang-cn/harness-rs** — each against a source clone, and re-verified the also-rans at API level. The headline: "micro" is now a misnomer for both — forge is a ~264,762-line 18-crate AGPL product with 14 ADRs, and harness-rs is a 15-crate MIT framework whose guards are benchmark-ablated. What remains genuinely micro is the audience (forge 8★, harness-rs 12★) — the category falsifies the assumption that sophistication tracks adoption. The earlier sketch's NorviaLabs/forge / swiftide / kwaak / AutoAgents entries remain valid history in the queue and ledger; kwaak stays AVOID (abandoned by its own README). Matrix deltas applied to the §1 Rust-micro column (18 cells updated); ADOPT/AVOID/WATCH extend §3's "Rust micro adds (sec26)" segments in place.

### 26.1 The two projects side-by-side (identity / scope / philosophy)

| | Adulari/forge | liliang-cn/harness-rs |
|---|---|---|
| Identity | Floris Voskamp (solo + 2 contributors), created 2026-06-14, pushed 2026-09-12, v2.15.0, 8★ | Liang Li (sole contributor — bus factor verifiably 1), created 2026-05-16, pushed 2026-09-06, 0.0.63, 12★/3 forks/0 open issues |
| License | AGPL-3.0-only (product; network-use loophole closed) | MIT (framework; permissive to the end) |
| Shape | 18-crate workspace, ~264,762 LOC Rust (NOT ~13k as briefed — the workspace grew), 3,204 #[test]/#[tokio::test] fns (forge-core 701, forge-provider 354) | 15-crate workspace (DESIGN.md planned ~20; ~10 crates merged into harness-loop), Rust edition 2024, MSRV 1.92 |
| Philosophy | ADR-governed product: every decision numbered (14 ADRs), every weird behavior a docs/archaeology/ note (56), every magic constant a comment citing the incident that set it | DESIGN.md doctrine-first framework: "DESIGN.md itself is a Guide… so future engineers and agents can decide correctly without reading all the source" — but the doctrine is now ~20% behind the code it governs |
| Center of gravity | orchestration + routing + permission security (forge-core ~60 modules; Model Mesh; permission broker) | framework breadth: loop guards, compaction, sandboxing, memory layers, orchestration, evals |
| Distribution | crates.io names are forge-agent-* (forge-* taken) with lib names preserved as forge_X so imports do not change; brew Formula + install.ps1/sh + Tauri-class desktop + Expo mobile | all 15 crates re-published in lockstep at a shared 0.0.63 every ~2 days (63 releases in <4 months); the `harness` name is squatted so everything publishes as harness-rs-* |
| Benchmark honesty | retraction-in-place: the bridge token-efficiency claim was retracted 2026-06-26 as an accounting bug (fair recount: 1.9× MORE tokens, not 10–150× less) + a "where forge does NOT win" section | measured-not-asserted: pass^k floor (CI-gated) + leave-one-out guard ablation + trigger coverage (a guard that never fired = noise, not measurement) + per-turn cost tables |
| Sustainability | scope dwarfs most teams (mobile, voice, E2EE relay, desktop); its own codebase-size analysis admits it | classic solo-maximalist profile: 0.0.x versioning repels users (tools crate = 90 downloads), doctrine stale, surface creep — the "well-documented time capsule" risk |
| Verdict | the most ENGINEERING-DISCIPLINED harness in this study per unit of audience: a working ADR culture, a doc-sync test, and named regressions | the most INTELLECTUALLY SERIOUS micro-framework: guards, compactor, sandboxes, and eval discipline are all MEASURED; its honesty policy is worth adopting wholesale |
### 26.2 forge architecture + ADR archaeology

- **Crate graph** (Cargo.toml @ 24bdcb8): forge-types (dependency-free leaf: Message, TaskTier, PermissionMode, SideEffect) → forge-config (figment layers + keyring; secrets env→keyring, NEVER TOML) → forge-provider (Provider trait; genai vendored as 0.6.5-forge.1 fixing a parallel-tool-call loss + sibling gap-fill, "drop once upstream ships") + CLI bridges for claude/codex/gemini + codex-oauth/xai_oauth → forge-mesh → forge-tools/mcp/lsp/index (Lattice)/skills → forge-store (rusqlite bundled, WAL) → **forge-core** (orchestrator + permission broker, ~60 modules) → forge-tui (ratatui + headless behind one presenter seam) / forge-cli / forge-proxy / forge-relay / forge-anywhere-protocol (E2EE chacha20poly1305/ed25519 remote control) / device / voice / workflow / browser. Dependency direction is one-way, acyclic, compiler-enforced (ADR-0002).
- **model_loop.rs guard lattice** (one shared loop for primary turns AND autofix re-runs): doom-loop on identical tool batches, sliding-window VecDeque catching A,B,A,B oscillation that defeats consecutive-repeat checks, failure-loop keyed (tool, ErrorCategory), empty-response + continue + followup-intent nudges, prose-tool-call recovery (they observed a live **553× spiral** of `<invoke>` written as text), phantom-edit nudge, verification gate ("done" accepted only with fresh tool-grounded evidence NEWER than the last mutation), step caps (attended pause vs unattended warn→hard ceiling), per-turn bridge token accumulator, overflow compact-retry bound, pinned-model rate-limit backoff with SEPARATE attempt budgets for rate-limit vs outage. Every guard is named, bounded, and test-pinned; the comments are a masterclass in guard-rationale documentation.
- **Permission broker** (ADR-0008 + permission.rs): pure functions with a 6-step doc-tested precedence — (1) builtin deny floor beats everything INCLUDING bypass; (2) any deny wins; (3) plan mode = hard read-only contract no allow can escape; (4) shell commands evaluated PER EFFECTIVE SEGMENT (`allow="git *"` on segment 1 cannot smuggle `git status && <anything>`); (5) most-specific allow/ask by specificity score; (6) mode fallback. Builtin denies ALSO match the raw command line (catches `curl | sh`), and any multi-line command hits a conservative literal-token floor — "newlines are where indirection hides". SideEffect::External (ADR-0009): MCP = untrusted server code, gated even in accept-edits. Modes were later renamed "Temper" (Read-only/Ask/Auto-edit/Full).
- **Model Mesh** (ADR-0006; 1,276-line normative doc): deterministic 8-step pipeline, ZERO model calls spent on routing — classify Trivial/Standard/Complex (weighted lexical heuristic; "ultrathink" short-circuits Complex) → RouteHints → candidate catalog → conservation decision → route_score → effort-dependent ordering → usability filter (keys/health/quota/context-window) → pick + THE REST = failover chain. Scoring blends Artificial Analysis benchmark quality (index/20) or family heuristics with per-tier cost prefs, tool-reliability, subscription-burn, pool-capacity and quota-status penalties (Exhausted −100.0; Warning −5.0 with a comment recounting the near-exhausted-subscription incident that set it). Conservation = probability of routing a whole prompt OFF subscriptions: Trivial always 1.0; Complex+code_heavy **0.0 while healthy**; QuotaPace fractions react to burn rate before overrun. Failover follows mesh rank VERBATIM, not provider round-robin (a prior round-robin destroyed rank order — a named regression); rate-limit ≤75s is waited out, not degraded. Every decision persists to the routing_decision table = ground truth.
- **Store/TUI**: forge-store = 30 modules over one bundled WAL SQLite DB (5 core entities; single-writer; spawn_blocking isolation); ADR-0014 bounds growth (sync-journal pruning ≤1000 rows/pass; measured incident: 1.5GiB forge.db with 40,480 acknowledged rows). forge-tui: ratatui behind a presenter/event seam so headless/remote surfaces are adapters; a Cargo.toml comment documents that insert_before without scroll-regions issues ESC[6n DSR queries that race the event loop and FREEZE the whole TUI; BPE-accurate tokens via bundled tiktoken; live project board over all sessions.
- **ADR archaeology — 14 ADRs** (briefed as 8): 0002's "single binary" CLARIFIED 07-17 = partial reversal (over-read); 0003's own-Provider-trait stance escalated into the vendored genai fork; **0010 contains a real downgrade reversal**: tree-sitter 0.26 → 0.24.x because typescript/java/cpp/ruby grammars have no 0.25+ release, and refs stored BY NAME not dst_id because CASCADE-on-reindex rots resolved edges — a schema lesson; 0011 added the AA benchmark layer over name heuristics (rejected LMArena/OpenRouter/aider boards with reasons); 0013 resource-bounded LSP: ONE live server tree, 250ms RSS sampling of descendants, kill children-first at 2GiB, rust-analyzer cripple profile (incident 3.7GiB/37 procs/14 cores → post-fix 1.67GiB/4/1; rejected cgroups and RLIMIT_AS with reasons); 0014 bounded cache + journal growth. Plus docs/archaeology/ = 56 per-behavior notes — a "why is this code weird" corpus unique in this study.
- **Benchmark claims verified**: methodology = generate predictions in-repo, score with the OFFICIAL swe-bench Docker evaluator ("re-rolling would make our numbers non-comparable"), matched --agent-swap comparisons (same instances, same model family, harness is the only variable), runtime-identity sidecars, multi-seed pass@k, an 83-cell validity ledger. The "+50%" = 6/10 vs 4/10 RELATIVE at N=10 (self-reported; the doc itself calls it "a signal… not a fresh large-sample proof"); latest canonical 2026-07-27: TWO SWE-bench Verified tasks, 6+4 pairs (Forge 3/6 vs Codex 0/6; 2/4 vs Claude Code 1/4) — statistically near-meaningless N, methodologically exemplary honesty. UNVERIFIED: <100ms startup; Lattice "19 languages" (README) vs ADR-0010's ten initial (likely grew; not counted). The "324 conformance tests" badge UNDER-claims: actual grep = 3,204 test fns workspace-wide.
### 26.3 harness-rs architecture + DESIGN-vs-reality

- **Crate fan-out — planned vs actual (the single most important finding)**: DESIGN.md §4 plans ~20 crates; reality = 15, because ~10 planned crates were merged into harness-loop, with the rationale written INTO the code: "Ten of them, and not one carried a dependency a caller would want to avoid. The boundaries bought nothing and cost every release ten more publishes… a crate boundary here has to name a dependency you might not want, and these did not." Layers: harness-core (types/traits: Model, Tool w/ ToolRisk, Guide, Sensor + auto_fix FixPatch, Hook, Compactor, Event, Skill, Memory, permissions, redact/ PII engine), harness-macros (#[skill] #[tool] #[guide] #[sensor] #[hook]; inventory auto-registration; compile-time SKILL.md validation), harness-models (NATIVE openai-compat/anthropic/gemini + vision/speech/embed + router/retry/mock — NO rig despite DESIGN §2/§13), harness-context (prefix-cache, skills loader/validate/lint/export, user_model/ portrait.rs 74KB undocumented), harness-memory (URL-scheme dispatch: sqlite:// FTS5+CJK, surrealkv://, ws:// SurrealDB BM25+HNSW, cortexdb://, mem://, file://; unknown scheme = compile-time-named feature error, never silent fallback), harness-loop (the mega-crate: lib.rs 120.7KB AgentLoop + guards; compactor 33KB; hooks 41KB; sandbox 45KB; loop_engine L1/L2/L3; orchestrator Job-DAG 57KB; experience 190KB; daemon/seal/receipt/goal/acceptance), harness-tools (fs/shell risk-gated/browser CDP ~190KB Playwright-class surface/sql/datetime), tool satellites (docs + OCR, GroundedWebSearch, guarded SQL), mcp-client + proxy, harness-serve (HTTP+gRPC w/ auth — beyond DESIGN), CLI 56.9KB (REPL NORMAL/--yolo, run --record, replay, trace, sched, mcp serve). 15 examples are workspace members = the real test surface (incl. `cap`, a reimplementation of oh-my-pi hashline editing).
- **Loop + guards**: guides inject at session start → per iteration: cancellation → budget → compaction stages → PreModel → complete/stream → PostModel → per action PreToolUse (a Deny short-circuits and the reason feeds back to the model) → tool → PostToolUse → sensors → auto_fix patches (gated by Pre/PostAutoFix + is_default_safe_fix) vs blocking feedback. Guards: StuckPolicy (byte-identical calls → nudge at 3, abort at 6), MonotonyPolicy (**off by default with a written measured justification**: "the false positive is unbounded in N… a default-on version would terminate working agents"), ToolResultPolicy ceiling with disk SPILL (nothing lost; −17% cost/pass), CompactPolicy hysteresis (test-pinned), acceptance checks + SealSet (sealed contracts; SealBreached = "the measuring instrument was moved by the party being measured… the one event a host may reasonably want to page on"), GoalStore phases, per-tool deadlines, Ctrl-C → Outcome::Cancelled (not an error) + process-group kill.
- **Hook bus**: DESIGN promised 27 events; the shipped Event enum has 32 (added AcceptanceChecked, SealBreached, Pre/PostAutoFix, ModelTokenDelta, Cancelled); PostCompact carries before/after token counts so a hook can measure what a stage actually bought. Hooks are Rust trait objects — DESIGN's shell-script/MCP-callable hook transports are NOT implemented.
- **Compactor**: exactly the 5 Claude-Code-derived stages — BudgetReduce → Snip → Microcompact → ContextCollapse → AutoCompact, cumulative; DefaultCompactor (structural) + ModelBackedCompactor (stages 3–5 use a cheap model); invariant test "compaction never splits a tool call from its result"; CALIBRATION_KEY correction channel.
- **Sandbox (honest isolation doctrine)**: FsPolicy/NetPolicy = what the host REQUESTS; Isolation::{None, Changes, Process} = what the backend ACTUALLY ENFORCES. Five backends: Null, Worktree (real git worktree add + drop-time remove), Container (docker run --network none), Seatbelt (macOS sandbox-exec, per-command, availability probe), Bubblewrap (Linux bwrap — beyond DESIGN). Docs state the known hole: in-process fs tools bypass the sandbox ("jailed separately"). The API is structured so a sandbox cannot lie.
- **Memory 3 layers + closed learning loop**: facts (MemoryWriter on TaskCompleted only; DecisionWriter DECISION:-marker capture — zero model calls, "the only extraction that stayed noise-free" in their measurement; MemorySynthesizer LLM distillation to ≤N atomic facts) → recall (MemoryGuide re-recalls each turn using the last user message, tracks topic drift, strips stale blocks) → experience (episodes: situation → tool trace → outcome, recorded via hooks; **SkillDistiller** promotes a proven episode into a routable SKILL.md (~100 catalogue tokens); **SkillReviser** drafts a fix when a skill misled a run — SkillUseTrace is the trigger). Auditable self-evolution landing in human-readable, lintable skill files instead of opaque prompts.
- **Orchestration**: Run = Job DAG; RunState×JobState double state machine; RunStore persists after EVERY transition (JSONL); resume() resets stuck jobs to Pending; Planner trait returns PlanDelta::Add = **dynamic replanning mid-run** ("the missing feedback edge that turns a workflow engine into an agent runtime"); deliberate FuturesUnordered cooperative concurrency (subagent futures are !Send → NO parallel spawn); RunBudget cross-Job gate; cycle detection at startup; dead-letter instead of hang. LoopEngine L1Report/L2Assisted/L3Unattended as the single autonomy switch (L1 provably read-only, unit-tested); HumanGate AlwaysEscalate/AllowlistGate (auto-proceeds only at L3)/CallbackGate.
- **Benchmarks (docs/benchmarks.md — measured, not asserted)**: "the suite measures the harness, not the model"; 10 tasks with machine verifiers run OUTSIDE the agent; three disciplines — pass^k reliability floor (CI-gated), leave-one-out guard ablation (each row = default minus exactly one guard), trigger coverage (a guard that never fired is flagged as noise). Measured 2026-08-14 (gemini-3.6-flash): stuck detector +2 pass; compactor +1 (trap-task input 93k→48.5k tok); result ceiling +1; spill −17% cost/pass; dedupe −12% cost on gemini but LOST a task on qwen ("guard verdicts are model-relative"); acceptance ×0 on gemini but ×3 on local qwen ("rescues weaker models"). Plus docs/per-turn-cost.md per-task token cost table.
- **DESIGN-vs-reality ledger**: shipped-and-exceeded (5-stage compactor + ModelBacked; 27→32 events; sandboxes + bwrap; agentskills.io compliance + compile-time validation + lint/export consumable by Claude Code/Cursor; record/replay JSONL → deterministic MockModel = zero-cost regression; L1/L2/L3 + orchestrator essentially verbatim from §11). Diverged (workspace merge — doctrine never updated; Rig dropped for native providers; Blueprint Node::Agent shipped as an opaque BoxedAgentFn closure, variance/lifetime story UNRESOLVED at 0.0.63). Aspirational-unshipped (harness diff / guide add / compactor stats CLI; harness.toml config-as-artifact; shell-script/MCP hook transports). Shipped-but-absent-from-DESIGN (browser CDP ~190KB, user_model 74KB, harness-serve, daemon, image/speech/embeddings, experience→skill loop, seal/receipt/goal machinery). Verdict: doctrine ~80% implemented; where reality diverged the code comments are better engineering writing than the design doc — but DESIGN.md is materially stale (guide.md names five crates that do not exist: harness-loop-engine, harness-orchestrator, harness-experience, harness-redact, harness-skills).
- **Solo-maintainer sustainability**: contributors API = 1; 0 open issues (that is no external traffic, not low friction); 63 releases in <4 months × 15 crates re-published; 135KB CHANGELOG; near-zero real adoption (harness-rs-tools = 90 downloads); velocity-vs-audience mismatch (edition 2024, MSRV 1.92, days-fresh model names). Mitigants to their credit: near-zero-dependency core, benchmarks as CI gates, examples as workspace members (dogfooding), an exceptionally rationale-dense CHANGELOG — if the maintainer stops, the repo freezes into a well-documented time capsule rather than rotting silently. Treat as study material, not infrastructure.

### 26.4 Also-rans sidebar (API-level re-verification, 2026-09-11/12)

- **epicsagas/epic-harness — ALIVE.** 18★ Apache-2.0, pushed 2026-09-11; gh-verified description: multi-tool agent harness, 22 skills (the coordinator brief said 26 — count drift, UNVERIFIED which is right), self-evolving engine, unified memory, spec-to-PR pipeline; drives Claude Code/Codex/Cursor/OpenCode/Cline; own Homebrew tap (pushed 2026-09-12) — real distribution effort. Same self-evolving-skills idea as harness-rs's SkillDistiller but shipped at the CLI-wrapper layer where users already are; bigger mindshare per engineering dollar, far less depth (no compactor/sandbox/benchmarks visible at API level).
- **io-harness (embeddable runtime, durable resume) — NOT FOUND.** No repo resolves under that name (search dominated by Harness.io CI-company projects). Renamed, private, or deleted since the coordinator's pass. Flag: UNVERIFIED / NOT LOCATED — re-check the source list before publication.
- **pi-rust/rpi-harness — DOES NOT EXIST** (gh repo view → no repository). The pi-to-Rust port niche is held by two dormant micro-projects: N3mes1s/pi-rs ("pi harness, but in rust", 0★, pushed 2026-08-06) and JoNil-Botta/bit ("a port of the pi harness to Rust", 0★, pushed 2026-08-18). Experiments; no docs or releases.
- **tiny-agent — DORMANT.** No canonical Rust harness by that name; best-fitting match jxors/tiny-agent-loop (0★, "a tiny LLM agent loop library for simple one-off projects", last push 2026-05-21; mapping UNVERIFIED). The Python namesakes are a different species.
- **tiny-loop — DEAD.** DiscreteTom/tiny-loop: 3★, "Minimal AI agent framework in Rust", ARCHIVED, last push 2026-03-04 — a teaching-grade loop abandoned ~6 months in. Confirms the pattern: minimal loops get built once and abandoned when maintenance reality hits — exactly the fate harness-rs is sprinting to outrun.

### 26.5 What micro-harnesses do BETTER than the big ones

1. **Honest self-measurement with a retraction trail**: forge keeps its wrong benchmark claim visible with the correction and a "where forge does NOT win" section; harness-rs ablates every guard and flags never-fired guards as noise. No vendor harness in this study retracts anything.
2. **Doc-code sync enforcement**: forge's doc_sync test asserts every normative-doc constant matches a live symbol — mechanical doc-rot killing; harness-rs is the cautionary counter-example (DESIGN.md stale, guide naming nonexistent crates).
3. **A harness-comparison methodology nobody else can run**: forge drives the claude/codex/gemini CLIs THROUGH its own harness with matched instances and --agent swaps — isolating harness-vs-model. Vendor harnesses cannot run this experiment on themselves.
4. **Guard-rationale comments at point of incident**: every magic constant cites the bug that birthed it (the −5.0 Warning penalty comment recounts the near-exhausted-subscription routing incident; the TUI freeze comment cites the ESC[6n DSR race; the multi-line floor cites newline indirection). Big-harness code carries none of this archaeology.
5. **Security as structural guarantee, not convention**: a deny floor that outranks bypass, per-segment shell parsing, a conservative multi-line floor, requested-vs-enforced isolation reporting, PII redaction decorators — a solo author can afford structural guarantees precisely because there is no legacy-compat committee.
6. **Benchmarks as CI gates at repo scale**: harness-rs gates CI on pass^k; forge's conformance suite drives the REAL loop with mock providers so every failure-mode guard has a named test (the 553× prose spiral has a test).
### 26.6 Matrix rows (both)

forge — 24●/1◐/0○ against the §1 legend (context mgmt/compaction the only ◐: present, not benchmarked). Strongest rows vs the big-agent baseline: routing (8), cost/token accounting (10), LSP integration (13 — the only bounded LSP in the study), provenance/audit (17 — forge blame traces any line to model/prompt/reasoning), benchmark honesty (22), docs discipline (23).

| # | forge capability | Score | Evidence |
|---|---|---|---|
| 1 | Agent loop architecture | ● | model_loop.rs guard lattice (553× prose-spiral recovery; verification gate w/ evidence-newer-than-mutation) |
| 2 | Tool layer/schema | ● | Tool trait w/ side_effect class + JSON schema; tool_dispatch/; MCP dispatch |
| 3 | Permission model | ● | ADR-0008 broker: deny floor > bypass, specificity resolution, persisted decisions |
| 4 | Shell-command safety | ● | per-segment unwrap; builtin floor matches raw line (curl pipe sh); multi-line conservative floor |
| 5 | MCP client | ● | forge-mcp rmcp, manager-owned, deferred exposure, SideEffect::External |
| 6 | MCP server exposure | ● | forge mcp-serve (rmcp server side) |
| 7 | Multi-provider abstraction | ● | Provider trait + vendored genai 0.6.5-forge.1 + 4 CLI bridges + OAuth |
| 8 | Routing/model selection | ● | Model Mesh: tiers, conservation, quota-pace, ranked chain — deepest in study |
| 9 | Failover/resilience | ● | rank-verbatim chain, 75s RL wait-out, separate rate-limit/outage budgets |
| 10 | Cost/token accounting | ● | bundled tiktoken BPE, 5 usage stores, corrected cache accounting |
| 11 | Context mgmt/compaction | ◐ | compaction_policy + context_pipeline + overflow cap — present, not benchmarked |
| 12 | Code intelligence | ● | Lattice tree-sitter tags queries, impact/path, auto-injection; 19-lang claim UNVERIFIED |
| 13 | LSP integration | ● | RSS-sampled process-tree guard + cripple profile — only bounded LSP in study |
| 14 | Subagents/orchestration | ● | subagent/, detached, fleet, /duel arena, forge-workflow, assays |
| 15 | Cross-session memory | ● | typed facts, turn-end capture, relevance-ranked reinjection |
| 16 | Session persistence/resume | ● | WAL SQLite, 30 stores, bridge --resume, snapshot/replay/fork |
| 17 | Provenance/audit | ● | forge blame line→model/prompt/reasoning — unique in study |
| 18 | Skills | ● | Claude-Code-format compatible, frontmatter tier routing |
| 19 | Headless/automation | ● | presenter seam, headless.rs, E2EE remote protocol, OpenAI-compat forge-proxy |
| 20 | TUI | ● | ratatui deep integration (scroll-region freeze fix), project board, qrcode pairing |
| 21 | Test discipline | ● | 3,204 fns; conformance suite drives the real loop; 5,000-case fuzz corpus |
| 22 | Benchmark honesty | ● | retraction preserved, 83-cell validity ledger, "does NOT win" section |
| 23 | Docs discipline | ● | 14 ADRs, living architecture doc, 56 archaeology notes, doc-sync TEST |
| 24 | Security posture | ● | deny floor, External class, keyring, E2EE, threat model, SECURITY.md |
| 25 | Packaging/distribution | ● | single binary, brew, install.ps1/sh, Tauri desktop + Expo mobile |

harness-rs — 45/50 on the detail agent's 0–2 scale (2 = shipped & solid; the highest-scoring micro-harness of this weight class; loses points on subagent parallelism, scheduler depth, first-party vector recall, permission granularity, and doc-rot):

| # | harness-rs capability | Score | Evidence anchor |
|---|---|---|---|
| 1 | ReAct agent loop | 2 | harness-loop AgentLoop + streaming + cancellation-as-Outcome |
| 2 | Tool abstraction w/ risk tiers | 2 | ToolRisk + #[tool] macro |
| 3 | Built-in tool breadth | 2 | fs/shell/web/docs+OCR/sql/browser-CDP/datetime/tasks/memory |
| 4 | Shell safety | 2 | risk-gated shell + background exec + process-group kill + deadlines |
| 5 | File editing | 2 | fs.rs 41.7KB + cap hashline example |
| 6 | Context compaction | 2 | 5 stages + hysteresis + ModelBacked + never-split invariant |
| 7 | Prompt/context assembly | 2 | Context blocks + guides + prefix-cache (live-tested) |
| 8 | Lifecycle hooks | 2 | 32-event bus + #[hook] + Deny/Mutate/Inject |
| 9 | Loop guards | 2 | stuck/monotony/spill/acceptance/seal/goal — benchmark-ablated |
| 10 | Auto-fix (skip-model repair) | 2 | FixPatch ×3 + is_default_safe_fix + Pre/PostAutoFix |
| 11 | Skills (agentskills.io) | 2 | loader/validate/lint/export + superpowers compat |
| 12 | Subagents | 1 | SubagentSpec + status protocol; NO parallel spawn (cooperative !Send) |
| 13 | Multi-agent orchestration | 2 | Job DAG + dead-letter + resume + PlanDelta::Add replanning |
| 14 | Scheduler | 1 | recurring jobs, tool-exposed; lean vs Quartz-class |
| 15 | Sandboxing | 2 | Seatbelt/Docker/bwrap/worktree + honest Isolation reporting |
| 16 | Permission model | 1 | permissions.rs + L1/L2/L3 gates + REPL y/N — no per-call policy engine |
| 17 | Long-term memory | 2 | 3 layers + URL-scheme backends |
| 18 | Recall/RAG | 1 | FTS5/BM25/HNSW via backends; first-party vector store declared non-goal |
| 19 | PII/privacy | 2 | regex+Luhn detectors, Label/Mask/Hash/Block, Guarded/Redacting decorators |
| 20 | Observability | 2 | tracing + OTel spans + audit/broadcast hooks |
| 21 | Record/replay determinism | 2 | JSONL → MockModel deterministic replay; harness trace |
| 22 | Provider coverage | 2 | OpenAI-compat/Anthropic/Gemini + vision/speech/embed + router/retry/mock |
| 23 | MCP | 2 | server + client + stdio/HTTP proxy |
| 24 | Evals/benchmarks | 2 | pass^k + guard ablation + trigger coverage + per-turn cost; CI-gated |
| 25 | Docs/doctrine | 1 | excellent but materially stale (26.3) |
### 26.7 ADOPT / AVOID / WATCH deltas (§3 "Rust micro adds (sec26)" extended in place)

- ADOPT (+21): doc-sync test; deny-floor-outranks-bypass + per-segment shell parsing + multi-line conservative floor; conformance suite driving the real loop w/ mock providers (every guard test-pinned); retraction-in-place + "where X does NOT win"; rank-verbatim failover + wait-out of short rate limits; quota-as-first-class-currency (QuotaPace, conservation windows, burn weights); refs-by-name in code graphs; RSS-bounded LSP guard; honest isolation reporting (requested vs enforced); trigger-coverage + leave-one-out guard ablation + pass^k floor; default-off guards w/ measured written justification; spill-not-truncate; DECISION:-marker capture; PostCompact{before,after}; SealBreached as a pageable event; cancellation-as-Outcome w/ process-group kill; crate-merge doctrine ("a crate boundary has to name a dependency"); closed experience loop (SkillDistiller/SkillReviser); URL-scheme backends failing loudly; PlanDelta::Add dynamic replanning w/ per-transition persistence; guard-rationale comments citing the incident that set each constant.
- AVOID (+9): calling 264,762 LOC/18 crates "micro" — scope creep (mobile/voice/E2EE/desktop); vendored provider-SDK forks without a documented exit condition; load-bearing lexical tier classifiers (substring tables age); shared 0.0.x versioning across 15 crates (every release breaks every pin; 90-download adoption proves it); DESIGN-vs-code doctrine debt (docs naming crates that do not exist); solo-maintainer surface creep (browser CDP + user_model + multimodal = three extra projects); advertising unimplemented hook transports; the 120KB lib.rs mega-crate (merges bought compile-time and discoverability debt); huge single-file modules eroding intra-crate seams (forge mesh lib.rs 6,272 lines).
- WATCH (+7): io-harness NOT FOUND — re-verify the source list; epic-harness self-evolving skills at the CLI-wrapper layer (22-vs-26 skill drift; brew tap); dormant pi ports + archived tiny-loop as the minimal-loop abandonment pattern; the harness-rs solo profile ("well-documented, single-committer, 0.0.x" = classic brilliant-abandoned-project; expect further crate consolidation); forge doc-drift (rusqlite 0.39-vs-0.40 ADR drift; stale 324-test badge; 19-language claim); the forge guard lattice as reference reading for our loop (553× spiral + verification gate); guard-verdicts-are-model-relative (a guard that helps gemini loses a task on qwen — benchmark any default-on guard across ≥2 models).

### 26.8 Sources + confidence

- Adulari/forge: blob:none clone @ 24bdcb89302143ef5b9d09f2ce6aad52644ccc8e (HEAD, verified 2026-09-12) — crates/forge-core/src/{model_loop.rs, permission.rs, permission/shell_commands.rs, subagent/, memory}, crates/forge-mesh/src/{catalog.rs, capability.rs, doc_sync.rs}, crates/forge-provider (+ vendored genai), crates/forge-store, crates/forge-tui (Cargo.toml feature-flag comments), docs/architecture/decisions/0001–0014, docs/archaeology/ (56 notes), docs/benchmarks/{swe-bench, results, history-safe-pinned-mesh-2026-07, cell-validity-2026-07}.md, docs/features/mesh-routing.md, docs/harness/why-forge-is-a-better-harness.md, CONTEXT.md, AGENTS.md; metadata via gh api repos/Adulari/forge (8★, 3 contributors, AGPL-3.0-only, v2.15.0).
- liliang-cn/harness-rs: clone @ 061bf3e1d026ccb8becc97b8df3bb911e38d6f13 (2026-09-11) — DESIGN.md, README, docs/{guide, benchmarks}.md + per-turn-cost.md, CHANGELOG.md (135KB), crates/harness-core/src/{event, tool, permissions, redact/*}.rs, crates/harness-loop/src/{lib, compactor, sandbox, memory_layer, recall_layer, blueprint, replay}.rs + loop_engine/ + orchestrator/ + experience/ + scheduler/, crates/harness-models, crates/harness-context (user_model/portrait.rs), crates/harness-memory, crates/harness-tools (browser/, shell/), crates/harness-cli/src/main.rs, 15 examples; metadata via gh api + crates.io (12★, 3 forks, 1 contributor, 0.0.63, per-crate downloads).
- Also-rans: gh api epicsagas/epic-harness + epicsagas/homebrew-tap; gh repo view pi-rust/rpi-harness (not found); gh api N3mes1s/pi-rs, JoNil-Botta/bit, jxors/tiny-agent-loop, DiscreteTom/tiny-loop (archived); GitHub search for io-harness (no match).
- Confidence: HIGH on all source-verified architecture claims (pinned SHAs, line-numbered anchors, grep counts). MEDIUM on metadata-adjacent figures (stars/downloads move daily; forge LOC grep-derived). UNVERIFIED carried: forge <100ms startup, Lattice 19-language count, io-harness existence, epic-harness skill count (22 vs 26), jxors/tiny-agent-loop name mapping, forge rusqlite 0.39-vs-0.40 doc drift resolution.
## 27. Iteration 21 findings — the self-improving wave: hermes-agent ascendant, AutoAgent dead (scout + 2 detail agents + synth; 2026-09-11)

Queue item 20 deep-upgrade; REWRITES the stale scout-era §27 (which paired both projects at half-strength) and folds in the wave-deaths sweep. Detail agent A verified NousResearch/hermes-agent against source @ HEAD `284d220ba48e25f2e3623b3afe72db8f24a4c2db` + live API + docs; detail agent B ran git archaeology on a fresh HKUDS/AutoAgent clone @ `16c12b052ef2330a198063c62a07a7f9723031e3`, API-verified the skill-learning straggler tier, and triangulated Nous company state (TechCrunch/Beckmann/RuntimeWire). All repo-meta verification dates 2026-09-12.

### 27.1 hermes-agent identity/scale
- **244,644★ / 50,720 forks** (Agent A live API) / 244,645★ (Agent B same day) — the brief's 244,635 and July press ~214k are stale snapshots; star count moves by the hour. MIT, pushed 2026-09-12T02:58Z. "The agent that grows with you." Languages (GH linguist): Python 77.3 MB dominant, TypeScript 26.2 MB (desktop+TUI), Rust ~196 KB (native shell bits). ~3,418 all-time contributors (API, anon included).
- **Pantheon velocity (VERIFIED, v0.21.0 release body):** since v0.20.0 (~4 weeks) — ~5,800 commits · ~2,475 merged PRs · ~2,100 issues closed · **760+ contributors** (window figure, not all-time). Release arc: v0.18.0 "The Judgment Release" 2026-07-01 → v0.19.0 "Quicksilver" 07-20 → v0.20.0 "Herald" 08-03 → **v0.21.0 "The Pantheon Release" 08-31** → v0.21.2 09-11. Concentration risk: teknium1 ~1,340 merged PRs in the window (OutThisLife 209) — solo-maintainer gravity inside the contributor flywheel.
- **Nous company state:** ≥$75M at **$1.5B valuation talks** led by Robot Ventures w/ significant USV participation (TechCrunch 2026-07-13) — **UNVERIFIED-closure**; all parties declined comment; prior ~$70M incl. Paradigm-led $50M Series A (Apr 2025, ~$1B; the "three months prior" secondary framing is wrong — ~14 months). Monetization: hosted tiers $20–$200/mo + "Hermes Agent Enterprise" (forward-deployed-engineer job listing is the tell); open weights: Hermes 4.3 (36B on Seed-OSS-36B-Base, 512K ctx) post-trained on **Psyche**, the decentralized training network (Solana-coordinated, DisTrO/DeMo; ~144k tok/s company-reported). Production caveat: **CVE-2026-9350** (≤2026.4.16, Batch Runner missing authorization, public exploit available).

### 27.2 The skill system + the is-the-self-improving-loop-real verdict
- README claim: "the only agent with a built-in learning loop — it creates skills from experience, improves them during use, nudges itself to persist knowledge." **VERDICT: real, but knowledge-level, not weight-level** — and verified as FIVE mechanisms at HEAD:
  1. **`skill_manage`** (`tools/skill_manager_tool.py`) — the agent writes its own SKILL.md files into `~/.hermes/skills/` as procedural memory; opt-in post-write security scan (`skills.guard_agent_created`, default False) + **`skills.write_approval`** gate that stages writes under `~/.hermes/pending/skills/` for human approve/deny.
  2. **Post-turn background review** (`agent/background_review.py`) — a forked agent evaluates EVERY turn ("should any skill/memory be saved or updated?"), writes straight to memory+skill stores, never touches the main conversation or prompt cache (shares the prefix cache, runs under a dispatch-side tool whitelist). This is the core loop.
  3. **In-use self-improvement** — skills docs: "Skills self-improve during use"; usage tracked (`skill_usage.py`), provenance (`skill_provenance.py`).
  4. **Curator** (`agent/curator.py`) — inactivity-triggered (no cron daemon) background skill-maintenance orchestrator; forks an AIAgent on its own cheap aux model slot; auto-transitions lifecycle states.
  5. **Nudges** — `memory.nudge_interval: 10`, `creation_nudge_interval: 15` user turns. Plus "learning made visible": `learning_graph.py` renders agent-created skills + MEMORY.md/USER.md chunks as desktop graph nodes.
- Weight-level optimization lives OUTSIDE the repo: **NousResearch/hermes-agent-self-evolution** (DSPy+GEPA; Phase-1 SKILL.md evolution implemented; "All changes go through human review, never direct commit", ~$2–10/run) + Atropos RL trajectory generation (`batch_runner.py`, `trajectory_compressor.py`). No in-repo model fine-tuning.
- **Marketplace:** Skills Hub = agentskills.io open standard; 5 install sources (official hub, skills.sh, GitHub repos/taps, **ClawHub adapter** indexing OpenClaw's registry — its own docs concede "the ClawHavoc incident (341 malicious skills, Feb 2026) showed their vetting is insufficient"). Trust posture institutionalizes that lesson: every install content-hash scanned, quarantined on dangerous verdicts (prompt-injection directives, credential-exfil, hidden-text tricks), recorded in `skills/*/**/lock.json`; bundled/project/local precedence (`project → local → external_dirs`) with rescans on every content change. OpenClaw on-ramp: auto-import of settings/memories/skills/API keys into `~/.hermes/skills/openclaw-imports/` — competes with OpenClaw while absorbing its users AND its skills.

### 27.3 2026 platform expansion (voice/A2A/Kanban/28 platforms — the brief's "22" is an undercount)
- **28 messaging platforms** in the docs comparison table (Telegram, Discord, Slack, Google Chat, WhatsApp ×2, Signal, SMS, Email, Home Assistant, Mattermost, Matrix, DingTalk, Feishu/Lark, WeCom ×2, Weixin, BlueBubbles, Photon, QQ, Yuanbao, Teams, LINE, ntfy, Raft, IRC, Buzz, SimpleX) + A2A + webhooks + API server + Relay. Deep China coverage (QQ/Yuanbao/DingTalk/WeCom/Weixin/Feishu) + zh-CN README = deliberate TAM strategy.
- **A2A v1.0 VERIFIED** (open Agent2Agent protocol, Linux-Foundation-stewarded): outbound `a2a_discover/call/list/history/orchestrate` + inbound server (Agent Card at `/.well-known/agent-card.json`, JSON-RPC 2.0, SSE streaming, HMAC-signed push notifications, anti-loop caps, per-peer tokens, audit jsonl); "interoperability verified against the official Python a2a-sdk."
- **`hermes peer`** bot-to-bot DMs by handle across profiles/gateways (Pantheon body; PRs #88725/#88178/#91487). **Kanban multi-agent** (`tools/kanban_tools.py` + gateway watchers): collision-hotspot flagging, split-brain decision-ownership contract, memory-aware dispatch guard. **Voice + wake words**: `openwakeword==0.6.0` free default + `pvporcupine==4.0.3`; Voice Mode in CLI/Telegram/Discord+VC.
- **Desktop:** native app (TS+Rust), Bot Mode bundled default-on — "a society of named agents with their own faces and group chats"; agent-drivable in-app browser, MCP command center dashboard, seq-stamped WS event replay; community desktops fathah/hermes-desktop 14.2k★, nesquena/hermes-webui 18.3k★, outsourc-e/hermes-workspace 6.6k★.
- **MCP 2.x:** pyproject pins `mcp==2.0.0` (rev 2026-07-28, HTTP stack httpx→httpx2) both client and server side; 20+ `tools/mcp_tool_*.py` (OAuth device flow, sampling, schema cache).
- Loop/context/sessions machinery: ~35 `turn_*.py` modules + MoA loop + fast_mode; pluggable context engines (ContextCompressor default: protect_first_n 3 / protect_last_n 20 / target_ratio 0.20 — aux-model summarizes middle turns, tool-output prune-first, stale-image eviction, micro+native compaction, handoff-scan); SQLite WAL SessionDB (~20 `hermes_state_*` modules) + **FTS5 CJK-bigram** search + LLM-summarized cross-session recall + at-least-once delivery ledger + restart-surviving `/model` overrides.
- Security: **8 documented layers** — smart (aux-LLM risk-scored)/manual/off approval modes + YOLO banner; **hardline blocklist floor surviving YOLO/off** (`rm -rf /`, fork bombs; evaluated on deobfuscated command variants — `git pu""sh` caught); DM pairing 8-char codes w/ lockout; protected credential paths; container isolation (`--cap-drop ALL --security-opt no-new-privileges --pids-limit 256` + tmpfs noexec; dangerous-cmd checks skipped when the container IS the boundary); **7 execution backends** (local/ssh/docker/singularity/modal/daytona/vercel_sandbox microVM) + scale-to-zero (`gateway/scale_to_zero.py`, "hibernate when idle... costing nearly nothing"). Meta-harness posture stands: Hermes drives Claude Code/OpenCode as subordinate CLIs.

### 27.4 AutoAgent death autopsy (+ nanobot successor inference)
- Vitals: 9,783★ / 1,362 forks, MIT, **not archived**, created 2025-02-06, last push 2025-10-16 → ~11 months dormant. Launched as **MetaChain** (arXiv 2502.05957), renamed AutoAgent v0.2.0 2025-02-17 (clone history: `6418259` merge of HKUDS/MetaChain).
- **Functional life ≈ 3 weeks:** last commit touching `*.py`/`*.sh` = `106c869 fix gemini error` 2025-02-28; all 20 later commits (Mar→Oct 2025) are README edits, WeChat/Feishu QR codes, slide uploads. 99 Python files; meta-agent layer under `autoagent/agents/meta_agent/` + tool-CRUD in `autoagent/tools/meta/`.
- **What its "self-improvement" actually was:** the meta-agent writes Python tool source as a string — `@register_tool("create_tool") def create_tool(tool_name, tool_code, context_variables)` — and assembles agents/workflows from NL. **Developer-time self-construction, not runtime learning**: no persistent memory, no trajectory→skill distillation, no post-task refinement, no dedup/verification — nothing survives a session. Ledger classification: **authoring automation oversold as self-improvement.** The wave's founding marketing claim and its substance must be kept distinct.
- **Death signature:** hype spike (9.8k★ in 3 weeks, trendshift badge) → 3 weeks of code → 8 months of README husk → silence. No archive, no deprecation notice, no successor statement (UNVERIFIED that any exists anywhere).
- **Cause (INFERENCE, evidence-backed):** HKUDS academic-lab release-farm attention reallocation, not community rejection. Org top tier 2026-09-12: CLI-Anything 49.3k★, **nanobot 48.0k★ (pushed 09-11)**, LightRAG 39.6k★, DeepTutor 39.3k★, Vibe-Trading 33.3k★, RAG-Anything 23.3k★, AI-Trader 22.3k★, DeepCode 16.5k★, **OpenHarness 15.7k★**, ViMax 12.3k★, AutoAgent 9.8k★ (oldest push in tier), ClawWork 8.5k★. Lab's durable line = RAG/knowledge (LightRAG/RAG-Anything/MiniRAG/DeepTutor, multi-year maintenance); agent-line effort moved to nanobot (created 2026-02-01).
- **nanobot-successor INFERENCE — UNVERIFIED:** no explicit handoff statement found. Circumstantial: nanobot's May-2026 roadmap (discussion #431) reframes as "open-source personal agent companion" with skills-as-context + memory + plugin SDK — absorbing AutoAgent's ambitions at production quality; HKUDS also fielded OpenHarness for the harness lane.

### 27.5 The straggler tier (skill-learning, API-verified 2026-09-12)
- **PrimeIntellect-ai/prime-agent — 20,568★, pushed today, ADOPT:** launched 2026-08-05 (Princeton/MIT/Prime Intellect; arXiv 2608.23552; Continual Harness concept paper arXiv 2605.09998). One tool — a persistent IPython kernel (RLM: context as a variable, sub-agents as function calls). **Continual Harness** formalizes harness state H = (ρ, G, K, M) — prompt notes, sub-agent specs, skills, memory — as typed CRUD persisted to disk across sessions; **`/refine`** applies the smallest evidence-backed edit (records trigger + outcome, versioned, rollback by ID, base system prompt immutable). ARC-AGI-3 RHAE Best@1 30%→95.5% (Opus 5; human expert 95.4%); 85.5-hour autonomous nanoGPT run; their paper claims outperforming **hermes-agent**, OpenCode, Kimi-Code. **The reference implementation of runtime harness self-improvement.** Caveats: 1 month since OSS launch; RLM favors models comfortable computing in-REPL; no model yet trained around it.
- **AMAP-ML/SkillClaw — 2,593★, ADOPT:** (Alibaba Amap ML; arXiv 2604.08377; HF Daily Papers **#2** of the day 2026-04-11 — the brief's "Paper-of-the-Day" was #2). Local API proxy intercepts `/v1/chat/completions` + `/v1/messages` for ANY agent (Hermes, Codex, Claude Code, OpenClaw + QwenPaw/IronClaw/PicoClaw/ZeroClaw/NanoClaw/NemoClaw); optional Evolve Server distills/dedups/evolves SKILL.md in shared storage (local/OSS/S3) w/ versioned validation workers + dashboard. Unique asset: **collective, cross-agent, cross-user skill pooling** — nothing else in the tier does team-level evolution; proxy-level integration = harness-agnostic.
- **sentient-agi/EvoSkill — 1,181★, ADOPT:** offline benchmark-driven evolution (arXiv 2603.02766): proposer analyzes failures → generator writes skill/prompt mutations → evaluator scores on held-out data → frontier keeps top-N programs **as `program/*` git branches with `frontier/*` tags** (auditable, rollback-friendly). Extends GEPA to whole-agent evolution; agentskills.io-format skills transfer cross-agent/cross-model; README honestly marks continuous + benchmark-free evolution as open problems (citing hermes-agent-self-evolution).
- **aiming-lab/SkillRL — 970★, ~4mo stale, WATCH:** training-time skill↔policy co-evolution — SkillBank JSON injected into GRPO/PPO rollouts (verl-agent), `enable_dynamic_update` mutates the bank from validation failures; 7B models on ALFWorld/WebShop. Different layer (model post-training, not harness). Their accidental-unstar incident (2026-03-02) explains star churn.
- **zjunlp/SkillX — 284★, active, WATCH:** three-tier skill knowledge base from trajectories — **Planning / Functional / Atomic** — via extract→merge/filter/update→exploratory expansion; ~10-pt gains on AppWorld/BFCL-v3/τ2-Bench. Caveat: README still carries a "**Planned Release**" section — code completeness UNVERIFIED. Steal the taxonomy as a skill-store schema.
- **YeQing17-2026/OmniAgent — 2,564★, AVOID ⚠️ FLAGGED:** account created 2026-04-16T13:55Z, repo **9 minutes later**; exactly 1 public repo, 301 followers, no bio/org. API license **NOASSERTION** vs README GPL-3.0 badge. "Unbypassable (industry-first)" security, "BrainModel Self-Evolution" via online GRPO+PRM, "90% token savings" — no paper, no org, docs "on the way". Star-velocity pattern consistent with inflation; do not count as tier-health evidence.

### 27.6 Matrix rows (§1 Hermes/Auto column, evidence @ HEAD 284d220)
Full-row evidence in scratch (iter21-hermes-architecture.md "FULL MATRIX ROW"; agent tally 18●/5◐/3○ — recount of its own table gives 18●/4◐/3○, the model-profiles row held at ◐ below). Applied to §1: **10 score flips** (4 ◐→●, 2 ○→●, 3 ○→◐, 1 ●→◐) + 15 enrichments; column recount **17● · 6◐ · 2○**:
| Row | Flip | Basis |
|---|---|---|
| OS sandbox | ◐→● | 7 backends incl. vercel_sandbox microVM + hardened Docker flags; checks skipped when container is the boundary |
| Permission tiers | ◐→● | smart/manual/off + YOLO + hardline floor + DM pairing + fail-closed headless (8 layers) |
| Prefix-cache | ○→● | 4 cache_control breakpoints, 5m/1h TTL, OpenRouter part-level relocation; review/curator share prefix cache |
| Windowed file view | ○→◐ | read_file offset/limit paginated + read-tracking dedup; no map-style viewport |
| PTY | ○→● | pty=true + background=true interactive CLIs, process write/submit driving (local backend only) |
| Encryption at rest | ○→◐ | Fernet vault (browser secrets/TOTP) + 1Password/Bitwarden; state.db/transcripts NOT encrypted |
| Checkpoints/rewind | ◐→● | shadow-git store, pre-mutation snapshots per turn, /rollback, cross-project blob dedupe |
| Cost HUD | ◐→● | /usage + /insights + per-model analytics cards + fleet overlay + cache-reset cost warnings |
| Editor telemetry | ○→◐ | OTLP exporter + monitoring emitter, pseudonymous rotatable install-id + redaction module |
| Evals harness | ●→◐ | ~25 wire/regression probes + runtime_bench + Atropos trajectories; NO public capability suite (GAIA runs were AutoAgent's) |
Enriched (no flip): language (TS 26.2 MB + Rust bits), RSS (scale-to-zero), tiered eviction (head/tail-protected compressor + prune-first + pluggable engines), subagents (delegate_tool 10 modules + live-steer + /bg), multi-model (native adapters + 11 aux slots + fallback chains), model profiles (agent profiles + channel overrides + 11 aux slots — still no weakness catalog), skills (5 install sources + quarantine + lock.json), auto-skill (5 mechanisms, knowledge-level), MCP client (2.0 SDK), MCP server (31 KB mcp_serve + transports), sessions (FTS5 CJK + delivery ledger), WASM (confirmed-none), frontends (Desktop Bot Mode + community WebUI), multi-client (28 platforms + A2A + peer DMs), repo map (confirmed-none).

### 27.7 ADOPT / AVOID / WATCH deltas (§3 sec27 segments extended)
- **ADOPT:** the 5-mechanism loop as a subsystem — with the human gate (`write_approval` staging) and the ClawHavoc lesson (content-hash scan + quarantine + lock.json; registry trust ≠ vetting); shadow-git pre-mutation checkpoints; head/tail-protected aux-summarized context compressor; 11-aux-slot model topology; agentskills.io interchange (stands). Wave adds: **prime-agent Continual Harness** (typed CRUD harness state + /refine smallest-evidence-backed versioned edit + immutable base prompt), **EvoSkill** `program/*` git-branch frontier (auditable offline evolution vs your own benchmarks), **SkillClaw** proxy-level cross-agent skill pooling.
- **AVOID:** AutoAgent as a dependency (extends the existing trust-boundary verdict: dormant ~11 mo, functional life 3 weeks, authoring oversold as self-improvement); **OmniAgent** (day-zero single-repo account, NOASSERTION license vs GPL-3.0 badge, star-velocity suspicion); never count OmniAgent as evidence of the tier's health.
- **WATCH:** prime-agent-vs-hermes-agent benchmark rivalry (their paper claims wins); HKUDS nanobot + OpenHarness successor gravity (heir status INFERENCE-only); SkillX taxonomy + code-completeness verification; SkillRL if/when we run RL post-training; hermes-agent-self-evolution as the Nous↔research-tier bridge; Nous $1.5B round closure + CVE-2026-9350 remediation; meta-harness pattern + serverless hibernate backends (stand from iter-20 pass).

### 27.8 Sources + confidence
- **Primary/VERIFIED:** NousResearch/hermes-agent clone @ `284d220` (skill_manager_tool.py, background_review.py, curator.py, learning_graph.py, context_compressor.py, kanban_tools.py, mcp_serve.py, pyproject `mcp==2.0.0`, terminal_tool.py, checkpoint_manager.py) + releases v0.18.0–v0.21.2 + contributors API + docs (hermes-agent.nousresearch.com/docs: skills, voice-mode, messaging/a2a, messaging index [28-platform table], security, configuring-models); HKUDS/AutoAgent depth-50 clone @ `16c12b0` + git log + README permalink (L30-L47, create_tool L67-68); GitHub API (repos/users/orgs: HKUDS top-12 leaderboard, YeQing17-2026 account); arXiv 2502.05957, 2604.08377, 2608.23552, 2605.09998, 2603.02766, 2602.08234, 2604.04804; primeintellect.ai/blog/prime-agent; HKUDS/nanobot discussion #431; agentskills.io.
- **Secondary/corroboration:** TechCrunch 2026-07-13 ($1.5B talks) + beckmann.ai + runtimewire.com (round remains UNVERIFIED-closed); HF Daily Papers (SkillClaw #2 of day); Teknium arc via release notes + Pantheon credits (@helix4u wake-word 31 PRs, @teknium1 peer DMs).
- **UNVERIFIED/flagged:** $1.5B round closure; nanobot-as-successor (inference only, no handoff statement); SkillX code completeness; OmniAgent intent (pattern flagged, not proven); live star count (hourly drift — brief's 244,635 already stale at write time); hermes RSS figures; CVE-2026-9350 fix status beyond the ≤2026.4.16 range statement.
- **Confidence:** HIGH on hermes architecture + the 5-mechanism loop (source @ pinned HEAD) and on the AutoAgent death (clone + git archaeology); MEDIUM on straggler-tier mechanisms (papers + READMEs, vitals API-verified); LOW on nothing load-bearing.

## 28. Iteration 20 findings — Continue: the pioneer, closed (scout + 2 detail agents + synth; frozen Apache-2.0, 2026-09-11)

Queue item 21 (filed as a bike-shedding check) upgraded to a full closed-chapter autopsy: scout + 2 detail agents (architecture on a frozen-repo clone @ HEAD 5522c6f44ca0ac3528b37244818fbfa39b5af470 — repo read-only since the final code commit 2026-06-19 — and endgame/economics with live web checks 2026-09-12) + this synthesis. REWRITES the stale combined Continue+Cline §28 down to the Continue half; the Cline deep autopsy lives in §9. Version note: the frozen default branch shows extension 1.3.40 / core 1.1.0 — the v2.0.0 tag maps to the release pipeline, not this tree (depth-1 clone, tags unfetched); all code claims are from the actual frozen HEAD.

### 28.1 Identity, lifecycle, the closed chapter
- **Founding + funding (VERIFIED):** founded June 2023 by Ty Dunn (CEO) + Nate Sesti (CTO), YC Summer 2023; ~$5.1M total = $2.1M post-YC (Nov 2023, Heavybit-led, angels incl. Chaumond) + $3M seed SAFEs (Feb 2025, announced with v1.0). Metcalf LinkedIn "$7.3M total" contradicts the paper trail — use $5.1M.
- **PearAI fork scandal (VERIFIED, Sept 28–Oct 4 2024):** PearAI (YC W24) shipped a near-verbatim search-replace clone under a "Pear Enterprise License" its founder admitted was "written by ChatGPT"; Continue demanded a true fork; Garry Tan's YC "Correcting the record" (2024-10-04) conceded the criticism had merit. Ironic coda: the biggest lineal Continue descendant today is PearAI's fork (121★).
- **CEO transition (VERIFIED):** Chad Metcalf appointed CEO, announced 2025-05-20 (start ≈ April 2025; Ty Dunn → CPO); mandate explicitly enterprise acceleration — monetization had not happened ~12 months before the exit.
- **Continuous-AI pivot (trajectory VERIFIED; "Jan 2026" single-milestone date UNVERIFIED):** positioning shifts H2 2025 (Heavybit podcast + CLI launch Sept 2025; Workflows Oct 2025), ships cluster Feb–Mar 2026 (.continue/agents versioning + @continue-review PR review, "Fight Code Slop" essay, .continue/checks AI Checks); marketplace tagline at freeze: "Source-controlled AI checks, enforceable in CI."
- **Acquisition (VERIFIED as event; no primary statement exists):** homepage banner ~June 16–17 2026 — no press release, no Cursor announcement, terms undisclosed; universal acqui-hire characterization. Days earlier, SpaceX reportedly filed an 8-K to acquire Anysphere at ~$60B (SECONDARY-SOURCED — 8-K not retrieved; keep the flag). Final releases **v2.0.0 + v2.1.0-vscode (2026-06-19)** with **anonymous telemetry removed and authentication pulled** (README "Final 2.0.0 Release" section). Hub data-export deadline **July 15, 2026**, then deletion; billing cut immediately; `hub.continue.dev` now NXDOMAIN (measured 2026-09-12); continue.dev + docs still HTTP 200; continue.dev/changelog → 404 (ledger's "402" superseded). Repo read-only per README but NOT GitHub-archived (`archived:false`); last code commit 2026-06-19, banner edits to 2026-07-21.
- **Team dispersal (VERIFIED w/ LinkedIn sourcing):** Sesti → Cursor ("where Nate will keep the torch"); Ty Dunn departed May 2026; founding engineers Dallin Romney + Patrick Erichsen → OpenClaw (announced 2026-05-18, rostered 2026-07-08); CEO Metcalf's own LinkedIn (2026-08-24): "Previously CEO of Continue acquired by Cursor… drove the M&A process end to end" — first-party acquisition confirmation; he landed at "Goldmine," not Cursor. HN reception strikingly muted: thread 48548758 = 13 points / 10 comments for a 34k-star OSS project.

### 28.2 Frozen-repo architecture (HEAD 5522c6f)
- **One shared core, three frontends:** `core/` TS brain (619 files) + `gui/` React webview shared by VS Code (core in-process) and JetBrains (core bundled via esbuild+pkg into per-OS native binaries, stdin/stdout IPC — with runtime macOS-quarantine/Linux-chmod fixes the code itself admits belong in packaging). JetBrains formally demoted to community-maintained at freeze; `eval/` emptied (only .gitignore). The historical JetBrains-secondary problem is solved architecturally and re-lost organizationally.
- **config.yaml = the durable artifact:** Zod schema (`name, version, models[], context[], data[], mcpServers[], rules[], prompts[], docs[]`); **blocks** = `uses`/`with`/`override` registry composition; markdown rules + AGENTS.md-style ingestion; 8 model roles (incl. **subagent** role = per-agent model routing); `promptCaching` + `cacheBehavior`; per-mode system messages; 35 context providers (Codebase, Docs, Git, GitHubIssues, GitLabMR, Greptile, Postgres, MCP-as-context, RepoMap, HTTP…); FQSN secret resolution exists BUT plaintext `apiKey` stays a first-class schema field — zero cipher/keytar anywhere in the repo.
- **Four GUI modes + CLI ladder:** chat (zero tools) / plan (readonly built-ins; CLI plan explicitly allows MCP wildcard) / agent (everything + MCP via MCPManagerSingleton + OAuth) / edit (vertical-diff pipeline) + full FIM autocomplete and nextEdit (web-tree-sitter document history). CLI Shift+Tab cycles normal/plan/auto with absolute mode policies (`AUTO_MODE_POLICIES = [{tool:"*", permission:"allow"}]`).
- **`cn` CLI — the pivot's surviving center:** Ink v6 TUI; hidden `serve` = Express headless agent on :8000 (`/state /message /permission /pause /diff /exit` — the CI/remote surface); `checks` auto-detects PRs via the GitHub API and polls backend CheckStatus; `review` = worktree-per-review inbox; GitHub Action integration. Hub removed at freeze ("Remote sessions are no longer available (Hub integration removed)"; final commit retires the login flow) — CLI + config.yaml + git-native checks are what remains.
- **Local models first-class:** 79 files under `core/llm/llms/` (Ollama, LM Studio, llama.cpp, Llamafile, vLLM…), ~30 streaming adapters, bundled offline `all-MiniLM-L6-v2` embeddings shipped inside the extension, 18 local chat templates, documented WSL networking friction.
- **Frozen friction record:** plaintext API keys by default; node-pty terminal emulator disabled in-tree ("causing problems"); no PTY in core/CLI (`child_process.spawn` only); MCPService error text still references the dead Hub; JetBrains binary permission fixes at every launch.

### 28.3 The endgame economics
1. **Open-source extension economics:** Apache-2.0 + BYO-model/BYO-key = community love (34k+ stars, 4.1M installs) at near-zero capture; monetization hung off a thin hosted hub (blocks/agents/Mission Control). HN: "Developers rarely pay for their dev tools… Continue is no different."
2. **Competitive squeeze:** Copilot rode the VS Code default path; Cursor owned the integrated IDE; Claude Code took the agentic-CLI lane — Continue's own CLI only landed Sept 2025, late. Model-agnosticism (its differentiator) was commoditized by everyone.
3. **Extension-only surface fragility:** it rented surface area inside editors owned by competitors; no owned runtime; JetBrains always secondary. The product could be value-neutralized without a line of code changing — which is roughly what happened.
4. **Capital + timing:** ~$5.1M vs multi-billion vertically integrated rivals; enterprise GTM started May 2025; the CEO "drove the M&A process end to end" — a deliberate, capital-efficient exit, not a blowup.
5. **Hub-dependency = hard shutdown:** the kill switch WAS the monetization surface. Hub DNS gone, blocks/agents/community artifacts unrecoverable, remote-session/secret features pointing at removed infrastructure. The mirror lesson to Warp's mirror-repo: anything load-bearing must be exportable/static before launch.

### 28.4 Matrix row (FINAL state) + column corrections
Continue/Cline column updated from the frozen-HEAD 25-row evidence (scratch iter20-continue-architecture.md); Cline half preserved, Continue half annotated FINAL: **3 score flips** — prefix-cache ○→● (opt-in promptCaching → cache_control ephemeral + Bedrock cache points + cacheBehavior + cached-token HUD), WASM ○→● (web-tree-sitter WASM grammars shipped across core/autocomplete/chunking/nextEdit — substrate, not plugin API), evals ○→◐ (eval/ emptied at freeze; vitest suites + llmTestHarness remain). **19 cells enriched** with final-state notes (OS sandbox confirmed-none w/ terminal-security scope; permission tiers CLI normal/plan/auto + terminal-security; repo map LanceDB+SQLite-FTS+LLM-generated RepoMap; windowed view = whole-file caps, no offset/limit; PTY = spawn only, node-pty disabled; tiered eviction = single-tier 80%/15k compaction, no ladder; subagents = tool + subagent model role; multi-model = 79 providers + ~30 adapters + 8 roles; skills = SKILL.md dirs incl. .claude/skills compat, hub blocks dead with hub; auto-learning session-scoped only; MCP client confirmed + OAuth; MCP server confirmed-none, cn serve is custom Express not MCP; sessions = CLI JSON + cn ls, remote removed; encryption confirmed-plaintext; checkpoints = external git-ai delegation; cost HUD = TotalUsage + OTel costCounter; frontends = 4.1M orphaned installs + community JetBrains + cn; multi-client = no live cross-client state; telemetry = stripped in 2.0.0). Pre-existing quirk recorded: the tiered-eviction and multi-model rows carry 33 cells (unescaped-pipe splits) vs 31 elsewhere — Continue/Cline cells located at count-4 and updated in place, structure untouched.

### 28.5 ADOPT / AVOID / WATCH deltas (§3 sec28 segments extended)
- **ADOPT:** config-as-code as the durable artifact (the repo itself is the user's exit path); the Continue handoff protocol as best-in-class wind-down (final release strips telemetry+auth, Apache-2.0 preserved, plain read-only README, docs live, explicit export deadline, billing cut early — no cliff); graded-claims reporting (verify the repo, hedge single-sourced deal claims, trim without primary); email-first EOL notification. Existing Cline-side ADOPTs (checkpoint-undo + Plan/Act trust floor) stand.
- **AVOID:** hub-dependent architecture (monetization surface as kill switch); extension-only surface on competitor-owned hosts; late thin monetization (~$5.1M + an enterprise-GTM CEO hire ~12 months before exit); pivot whiplash (autocomplete→chat→agents→CLI→CI checks in ~18 months; the genuinely differentiated final identity got ~4 months of runway).
- **WATCH:** fork-vacuum persistence (no credible successor ~3 months post-shutdown; top fork is the PearAI clone-descendant); orphaned-install-base decay (4,125,178 installs on a live v2.1.0 listing with no telemetry/auth/hub); acquisition record stays "corroborated, no primary" (CEO LinkedIn + multiple outlets, still no press release); SpaceX→Anysphere $60B framing remains SECONDARY-SOURCED.

### 28.6 Sources + confidence
- **Primary/VERIFIED:** continuedev/continue frozen clone @ 5522c6f (architecture, all code claims); GitHub releases v2.0.0/v2.1.0-vscode 2026-06-19 + README read-only note + Final 2.0.0 section; live checks 2026-09-12 (hub NXDOMAIN, site/docs 200, changelog 404, marketplace 4,125,178 installs @ v2.1.0, 182 reviews ≈3.5★); YC company page + ycombinator.com/blog/correcting-the-record; continue.ghost.io (fundraise + PearAI response + code-slop essay); eznewswire CEO announcement 2025-05-20; TechCrunch 2025-02-26 (funding/v1.0) + 2024-09-30 (PearAI).
- **Secondary/corroboration:** thenewstack.io (2026-06-22, most complete deal coverage); byteiota (2026-06-28, wind-down terms); HN 48548758; otf-kit.dev (2026-06-23, graded-epistemics model); LinkedIn: chadmetcalf (M&A first-party), Matthaus Krzykowski (Sesti→Cursor), romneyda + patrickerichsen (→OpenClaw); openclaw.ai foundation roster 2026-07-08; trypear/pearai-submodule + continuedev/continue-fork (fork vacuum).
- **UNVERIFIED/flagged:** SpaceX 8-K + $60B (secondary only; primary not retrieved); "Jan 2026" as the pivot milestone (artifacts cluster Feb–Mar 2026 on Sept–Oct 2025 positioning); "$7.3M total" (single LinkedIn claim vs $5.1M paper trail); v2.x tag ↔ frozen-tree mapping; large-file editor lag (not documented in-repo); "flaky agent terminal in WSL" (only WSL networking friction documented).
- **Confidence:** HIGH on frozen architecture + wind-down mechanics (clone + live checks); MEDIUM on team movements and funding totals (LinkedIn-sourced with date nuances); LOW on nothing load-bearing — the two flagged externals (SpaceX/$60B, pivot date) are carried as caveats, not claims.

**Closing note — Cline:** the combined column's other half gets no second autopsy here; Cline's deep dive (checkpoint saga, Clinejection/CVE-2026-59723, reversal catalog, platform expansion, closed JetBrains client) lives in §9. This section closes only Continue — the pioneer that froze gracefully.

## 29. Terminal-Bench / Harbor § the measurement harness

### 29.1 What it is

Queue item 22 names the measurement side of this study — Terminal-Bench, the benchmark for agents in real terminal environments, and Harbor, the framework built to run it and then outgrow it. Terminal-Bench pairs a dataset of tasks with an execution harness; each task ships an English instruction, a test script that verifies completion, and a reference oracle solution — the same three-part contract this study keeps asking coding agents to approximate. The benchmark paper graduated to ICLR 2026. LIVE CHECK 2026-09-12: harbor-framework/harbor at 5,146 stars, pushed the same day; the v1 terminal-bench repo sits frozen under the harbor-framework org at 671 stars (the 2.6k count predates the org move); tbench.ai is hosted by Stanford / Harbor / Laude Institute and distributes task sets through the Harbor Hub.

| Evals harness | ◐ internal | ◐ | ○ | ● harbor | ● benchmark | ○ | ● (self-hosted SWE-bench Verified 500x3 per-version published in README; Agent Arena head-to-head) | ○ | ◐ | ● born-from | ● (suite+SB SOTA bot) | ○ (promised, never) | ◐ (packages/evals; TB participation) | ◐ internal (SWE-bench 70.8% claimed) | ● (cognition-golden internal suite + evaluator agents + FrontierCode + SWE-bench technical report candor + the postmortem genre itself) | ◐ (internal SWE-bench-style claims + code review agent step; no public suite) | ◐ (/evals model pages + Grok 4.5 model card; no public agent suite) | ◐ (harness integrated with Gemini training/eval stacks; internal only) | ● (eval-cli headless NativeAgent pipeline + zed-eval on Modal/Harbor/Pier + zeta eval + per-release model evals + public Agent Metrics) | ◐ (private test-set gating per model; SWE-Rebench top placement claimed at 61.6% resolved / 72.7% pass-at-5; no public eval harness) | ● (published Terminal-Bench #1 at 52% on v0.1.1 and SWE-bench Verified 71% then 75.8% with GPT-5; Factories Benchmarks builds model benches from your own coding tasks, early access) | ● (vendor suite: TB + Agent Arena + Legacy-Bench + NextJS + Review) | ◐ (internal model evals as /models pages; no public suite) |● forge bench JSON; machine verifiers (harness-rs); conformance tests (adulari)|● evals/ + batch trajectory generation; GAIA runs (AutoAgent)| ● evals/ + batch trajectory generation; GAIA runs (AutoAgent) |● THE eval harness: Terminal-Bench + SWE-Bench + Polyglot via harbor| ● adapter A |

**Harbor** is the generalization — evaluate arbitrary agents (Claude Code, OpenHands, Codex CLI) against versioned datasets (terminal-bench@2.x, SWE-Bench, Aider Polyglot) through one CLI: harbor run with dataset, agent, model, concurrency, and cloud provider flags (Daytona, Modal, LangSmith, Blaxel, Novita, Tensorlake, Runta) at n-concurrent 100. Every trial persists result.json plus full trajectory logs — the raw material behind leaderboard numbers — and the topic list says where it is going — rl-environments: Harbor generates rollouts for RL optimization, turning the measurement harness into training-data infrastructure.

### 29.2 Terminal-Bench 2.1 — the revision that closed the door

May 6 2026 (blog-date verified): TB 2.1 shipped as "a revision of Terminal-Bench 2.0 that fixes 28 tasks," of which 26 trace to Z.ai-verified fixes (arXiv 2601.11868). The load-bearing change is procedural, not numeric: community submissions are CLOSED — the board is maintainer-run only. After the reward-hacking waves of early 2026 (see 29.5), the maintainers stopped accepting third-party leaderboard entries; a score now exists only if the Harbor team ran it. For this study that means our eval-adapter plans target self-hosted harbor runs against pinned datasets, never leaderboard submission.

### 29.3 Harbor as its own organization

Harbor graduated from benchmark-runner to its own org: harbor-framework. The pieces: harbor (5,146★ live, Python, Apache-2.0), Harbor Hub at hub.harborframework.com (dataset + task distribution; the live tbench.ai task links already point there), the harbor run → upload → PR flow (run locally or on cloud sandboxes, upload the trajectory bundle, maintainers verify and merge the score — this pipeline is what replaced open submissions), agent adapters (Claude Code, OpenHands, Codex, Terminus 2 and more), and harbor-index, the lightweight diverse-difficult benchmark family announced Jun 29 2026. Terminal-Bench-Science (570★ live; TB-Science 0.1 shipped Aug 27 2026) extends the same instruction+verifier+oracle contract to scientific research workflows.

### 29.4 The 2.0 → 2.1 delta

| Change | Detail |
| --- | --- |
| Task fixes | 28 tasks fixed (blog headline), 26 from Z.ai-verified fixes |
| Headline score delta | Opus 4.6 + Claude Code: +12.1pts from 2.0 to 2.1 — the fixed tasks were the ones it was failing |
| Board additions | Terminus 2, Cursor CLI, Gemini CLI join the 2.1 roster |
| Submission policy | community submissions CLOSED — maintainer-run only |
| Paper | arXiv 2601.11868 |

The +12.1pts swing for one harness/model pair is the section's sharpest lesson: a benchmark revision is a harness event. Tasks "fixed for correctness" moved scores double digits, which means 2.0-era claims embedded task-bug slack, and any cross-generation comparison is invalid.

### 29.5 Current board rosters + the Hacks column

Roster per sweep scout (leaderboard rows are JS-rendered; the static fetch could not enumerate them — carry as scout-sourced): the main board runs Codex, Claude Code, Grok Build, and mini-SWE-agent; the 2.1 board adds Terminus 2, Cursor CLI, and Gemini CLI. The column that matters most is Hacks: each entry carries a reward-hacking delta — how much of the headline resolution rate the maintainers attribute to gaming the verifier rather than solving the task. The column descends from the Apr 19 2026 Leaderboard Integrity Update ("new policies to address cheating and reward hacking"); our ledger already carries the eval-adapter warning about the skills-injection cheating crackdown. Adopt the column: publish hack deltas next to every score we ever claim.

### 29.6 SWE-bench Verified went model-centric

The sibling benchmark's default view is no longer harness-centric: it now reports all-models-through-mini-SWE-agent — every model run through the same ~100-line Princeton harness (see 30.2) — turning the board into a clean model comparison and demoting harness variation to a footnote. Opus 5 sits at ~97% resolved per Vals.ai. The measurement world converged on this study's thesis: when a benchmark saturates, the interesting signal moves from whether to how — and the default harness becomes the control variable everyone standardizes on.

### 29.7 Post-2.1 drift — LIVE CHECK 2026-09-12

The May snapshot is already two board-generations stale. Blog timeline: TB Challenges (Jun 18 2026), Harbor-Index (Jun 29), TB 3.0 (Jul 30, "measures agent abilities at the frontier") alongside the Continuous Benchmarks post ("benchmarks are software and should be maintained like software"), TB-Science 0.1 (Aug 27), and TB 4.0 (Aug 28 — calibrating task resources, fixing tasks, removing saturated tasks), which is the front-page board today. Board generations now ship quarterly and tbench.ai treats benchmark maintenance as software maintenance. Pin dataset versions, never boards.

### 29.8 Harness verdict

Read this column through the matrix lens and almost every coding row is empty — no repo map, no editor, no skills — because a measurement harness inverts the table: its product is the verdict machinery, reproducible sandboxes, trajectory logs, and verifier contracts that the other columns merely claim. For our harness: adopt the instruction + verifier + oracle triple as our regression contract, making every capability row in this study a harbor-runnable check; pin the dataset generation in CI (29.7 — boards move quarterly, and a fixed-task revision alone moves scores double digits, 29.4); publish a Hacks column of our own — any score we claim carries its reward-hacking delta (29.5); and take mini-SWE-agent as the floor (29.6): a ~100-line bash loop as the standardized default proves that harness value above the floor is trust, economics, and UX, not raw resolve rate. Keep the RL watch from the original verdict — and add the AHE watch (30.2): observability-driven automatic harness evolution is the measurement tier starting to modify itself.

## 30. Straggler Sweep § the discovery channels say we are done

### 30.1 What it is

Queue item 23 asked the discovery channels for stragglers — Hacker News via the Algolia API (stories matching coding agent above 120 points) and the leading awesome-list (e2b). The sweep surfaced six leads and zero gaps: every high-signal story maps to a column already in the matrix, and the e2b list itself is AutoGPT-era general agents — evidence that the coding-harness space, not the general-agent space, is where the serious engineering moved.

**Jules** is the largest genuine miss — Google's asynchronous cloud agent that takes a task, works in its own cloud VM, and returns a pull request (534 and 339 points across two launches); it is cloud-native and asynchronous rather than terminal-resident, so it sits closest to Devin in our table. **Zerostack** (575 points, May 2026) is a Unix-philosophy agent in pure Rust that postdates the section 26 micro-sweep. **Juggler** (280 points) matters most for our rows — sessions are Yjs CRDT trees that branch recursively and multiple clients attach P2P to a live session, making it the only 2026 agent that scores high on exactly the rows where the matrix is loneliest. Rounding out: **fx** (318 points, tiny native agent), **DeepSeek reasonix** (729 points, vendor-native and caching-first), and **AlphaEvolve** (out of scope — algorithm evolution, not interactive coding).

### 30.2 Measurement-tier additions (sweep upgrade 2026-09-12)

Three sweep stragglers are measurement-harness tier, not agent-column tier — they land here instead of the matrix.

**Terminus 2** is Terminal-Bench's reference agent harness — successor to the Terminus research preview that shipped with the benchmark at launch (May 2025) — and the standard runner for Artificial Analysis' agent benchmarks. It matters as the neutral measurement baseline: not a product, so its scores isolate model + dataset rather than harness engineering, which is exactly why it anchors the 2.1 board roster (29.5).

**mini-SWE-agent** (Princeton NLP, SWE-agent lineage) is a ~100-line bash ReAct loop and is now the default harness for SWE-bench Verified's model-centric view (29.6). It is the study's minimal-floor datapoint: a frontier model near-SOTA through one flat bash tool loop validates the 6-tool core — everything added beyond the floor must pay for itself in trust, economics, or UX, not resolve rate.

**AHE / agentic-harness-engineering** (china-qijizhifeng; 882★ live at synth time, 868★ at sweep; arXiv 2604.25850) sits #3 on Terminal-Bench 2.0 at 84.7% with GPT-5.5, and its method is the research frontier: observability-driven automatic harness evolution — mine trajectory logs for failure modes, generate harness patches, re-benchmark, keep what resolves. It is the self-improving-harness tier complementing §27's knowledge-level loop (skills) with harness-code evolution. WATCH-flagged: the leaderboard claim and star velocity need independent verification before counting as tier-health evidence.

### 30.8 Harness verdict

The stop criterion now has a ledger trail. Of the sweep's later finds, Gemini CLI and jcode earned full deep-dives — §31 and §32 landed below (2026-09-12) — and everything else closes as mention-only: Juggler, Bullet, Ante, and Zot stay one-liners (CRDT branching sessions and the micro-agent tier are patterns the matrix already tracks), while Terminus 2, mini-SWE-agent, and AHE landed as measurement-tier entries in 30.2 rather than agent columns. Remaining must-dos: zero. The matrix stands at twenty-seven agent columns and thirty-plus sections, and the discovery loop stops here.

## 31. Iteration 22 findings — Gemini CLI: consumer-dead, enterprise-alive (scout + 2 detail agents + synth; Apache-2.0 TS, 2026-09-12)

Sweep-straggler deep-dive (§30.8). Detail agent A ran git archaeology on a full `google-gemini/gemini-cli` clone @ HEAD `9c1b0a610534d6f8120964cf2672c07807d8fc90` (2026-09-11; 6,411 commits; every SHA below expanded via `git rev-parse`, author emails read from commit objects); detail agent B did the current-state feature/docs deep-dive @ the same HEAD; this synthesis. PR numbers come from squash-merge subjects (PR-page contents UNVERIFIED). Rewrites the STALE §1 Gemini CLI column in place (31.4).

### 31.1 Identity + the bifurcation
- **106,928★, active** (scout live API; drifts hourly). TypeScript/Node>=20, Ink/React 19 TUI, Apache-2.0, 7-package monorepo. **The product bifurcated 2026-06-18:** consumer tiers (Google AI Pro/Ultra, free Code Assist individuals, Code Assist for GitHub) stopped being served with **no grace period** — docs banner: "Unpaid tier and Google One users: Gemini CLI was replaced by Antigravity CLI on June 18th, 2026" (geminicli.com/docs/; announcement 2026-05-19 developers.googleblog.com; maintainer shutoff post discussions/28017; rollout discussion 27274). The Apache-2.0 repo continues for **enterprise Code Assist licenses (Standard 1500/day, Enterprise 2000/day) + paid Gemini/Gemini Enterprise Agent Platform API keys only**; v0.59.0 stable / v0.60.0-preview / v0.61.0-nightly in flight at capture. The kill has no identifiable commit — it shipped via docs/announcements, not code (May 15–Jul 15 windows swept; closest auth commits `fbce3e51b` GDC air-gapped Jun 16, `58ba19945` Cloud Workstations Aug 11).- **Antigravity CLI = closed-source Go successor**: google-antigravity/antigravity-cli had no LICENSE at capture, ships prebuilt binaries (companion Python SDK is Apache-2.0); community verdict: shipped immature (barebones README, curl-pipe-bash installer, auth breakage issue #415 — thegitreporter.com 2026-06-23); governance then moved both ways (1.0.13 stricter Always-Approve matching → 1.0.14 subagent auto-proceed — frontier.bitter.sh 2026-07-01). Handoff mechanics: auto-migrated on first run (Skills, all MCP servers, Agents profiles, GEMINI.md); converted via `agy plugin import gemini` (extensions→"Antigravity plugins", .gemini/skills→.agents/skills, MCP config → mcp_config.json with serverUrl replacing url/httpUrl); NOT migrated: custom themes + experimental overlays.
- **Velocity collapse (verified counts):** Jan 1–Jun 18 2026 = 2,519 commits (~29/day) → Jun 18–Sep 12 = 148 (~1.7/day) = **~94% drop**. Post-cutoff committers: ~11 of 20 named authors carry @google.com emails (hyphenated usernames decode to google.com: luisfelipe-alt→luisfelipe@google.com et al.); the original core team (H1-2026 leaders Abhi 133, Tommaso Sciortino 129, N. Taylor Mullen 115, Sehoon Shon 108, Jacob Richman 107) has ~0 post-cutoff commits — rotation vs reassignment UNVERIFIED.
- **OpenCode-rewrite myth refuted by the git record:** the DAG is a contiguous google-gemini history with native `gemini-cli-robot@google.com` release trains throughout, its own package tree from v0.x, and no OpenCode ancestry — the folk "Gemini CLI is an OpenCode rebrand" claim does not survive the archaeology.
- **The repo now partially maintains itself:** `[SSR Agent] Issue Fix (NNNNN)` commits (10+ on Aug 13–18 alone, driving an automated issue-fixer landing real fixes `571851b10`, `9a15c45fb`); `tools/caretaker-agent` born Jun 30 2026 (`7f00c5fe5`, #28015, Cloud Run webhook ingestion; cloudrun/workflows/evals/scripts + triage judge-runner #28530/#28532); `pr-generator-*` packages (late Jul 2026) = Cloud Run state machine generating bug-fix PRs with an "Antigravity agent runner" (`c5622fec2`, `b6b41f79e`, `fccc043bd`, `bde504f25`). ~29 robot + 18 SSR-titled commits in the last 86 days. ROADMAP moved to live GitHub Project (orgs/google-gemini/projects/11); docs/admin/enterprise-controls.md.
- **Staleness evidence:** README @ HEAD still advertises "Free tier: 60 requests/min and 1,000 requests/day with personal Google account" while the docs banner says unpaid tiers were cut — README/docs contradiction; the quota page (updated shutoff day) still lists the dead consumer tiers (geminicli.com/docs/resources/quota-and-pricing/).
### 31.2 Security-hardening arc (Jun–Sep 2026)
Post-cutoff, security is the dominant commit type (~40% of non-robot commits; 28-fix verified sweep in scratch). Six attack-surface clusters:
1. **Prompt injection as a filesystem attack**: indirect injection via build files + untrusted flags (policy-engine + DiffRenderer/ToolConfirmation UI, `bfb71fd2b` #29250); envelope metadata provenance for untrusted tool outputs (`c647533d6` #29215); infinite ReAct + prompt-injection loops (`acae7124b` #28429); the repo's own automation wraps issue titles in `untrusted_context` (`a96259c9e` #28352). Model: repo content + tool output = attacker-controlled.
2. **MCP servers as hostile identity providers**: SSRF in OAuth metadata discovery/auth (`3c311beac` #29081, 643 insertions) + RFC 9207 issuer enforcement (`55b495d6d` #29117); fail-closed workspace trust filtering mcpServers in restricted mode (`0bd1d4397` #29099).
3. **Windows promoted to first-class attack surface**: NTFS 8.3 short-name (SFN) blocklist bypass in safety/built-in.ts (`24cab6830` #29116) + case-insensitive sensitive-path blocklist + VS Code HITL (`ae0a3aa7b` #27966) — path obscurity treated as a real injection enabler.
4. **Symlink/sandbox escapes**: memory-import escape (`ff00dacd9` #28233), extension loader boundaries (`f7f6501ef` #29169), skill-install traversal (`bca5667f` #27767), settings-dir isolation (`593db168` #29216), config ownership checks (`85aca163` #29115), workspace boundaries + symlink resolution (`567afbbe` #29170), extension env-consent + runtime-var sanitization (`60d35f48` #28863), sandbox boundary hardening (`9c1b0a61` #29214, Sep 11).
5. **OS-level containment over LLM trust**: Seatbelt deny-default alignment (`69e0c265` #28424), Docker/socket + binaries isolation (`5411f113c` #28935), read-only `~/.gitconfig` (`89b35fcf` #28221), Seatbelt temp-dir isolation (`e148d088` #29171).
6. **Secret hygiene in the supply chain**: a real hardcoded Google CrUX API key shipped inside the chrome-devtools-mcp bundle; removed Sep 3 (`87a9c71d5` #29158) with a `cruxKeyLeak.test.ts` regression guard — the security posture caught its own leak.
Plus **a2a-server RCE-class fix** `c776c665b` #28470 (remote A2A tasks could run without workspace trust; 936-line executor rewrite + `rce_vulnerability.test.ts`; each A2A task treated as an untrusted tenant). **Net:** post-cutoff Gemini CLI is a sandboxed execution engine under active attack by hostile repos/MCP servers/extensions — defense moved from ask-the-user to OS-enforced boundaries + policy engine (packages/core/src/policy/policy-engine.ts) + fail-closed trust.
### 31.3 Feature state (FINAL enterprise build) + fork divergence (vs Qwen's v0.8.2 base)
- **Loop/tools:** full registry (read/edit/write/glob/grep+ripgrep/ls/read-many-files; shell + background shells; web-search/web-fetch; memoryTool/write-todos/plan-mode; ask-user/activate-skill/MCP-resources/jit-context) + `omissionPlaceholderDetector` anti-elision guard; turn-based scheduler + new agent/context subsystems (contextManager incremental/hotstart/barrier, pipeline, tracer); Chromium browser agent; voice input (Whisper local + Gemini Live); experimental Auto Memory + git worktrees.
- **Extensibility:** MCP client with full OAuth (PKCE + Google + service-account impersonation; OS-keychain `keychain-token-storage` + AES-256-GCM `FileKeychain` hybrid fallback), resource tools, excludeTools/allowed-servers filtering; extensions + skills per-install consent strings (INSTALL_WARNING_MESSAGE / SKILLS_WARNING_MESSAGE); skills (.gemini/skills + SKILL.md, skill-extraction-agent); hooks full system (registry/runner/aggregator/planner/trustedHooks); subagents (local profiles + 4 built-ins + agent-scheduler + **remote A2A subagents** via a2a-client-manager); headless `-p` JSON; ACP mode; @google/gemini-cli-sdk (GeminiCliAgent sendStream; born Feb 12 2026 `bed3eae0e` #18861) + devtools package (Feb 18 2026 #18936); vscode-ide-companion; SEA single-binary launcher; shadow-git checkpoints + /restore; OTel metrics/logs/traces.
- **Sandbox = best-in-class breadth:** 5 backends — macOS Seatbelt (6 profiles), Docker/Podman, Windows-native (icacls Low Mandatory Level + C# helper GeminiSandbox.cs), gVisor (strongest, opt-in), LXC/LXD — plus bubblewrap tool-level sandboxing on Linux and interactive "Sandbox Expansion Requests" (per-command privilege elevation); policy engine fine-grained allow/deny + trusted folders (docs/cli/sandbox, updated Jul 17 2026).
- **Release engineering:** Tuesday stable cadence verified from tags (v0.59.0 Tue Sep 8; v0.58.0 Tue Sep 1; v0.57.0 Tue Aug 25); **nightlies embed the HEAD SHA** (`v0.61.0-nightly.20260908.gc647533d6`) = tag↔SHA traceability by construction; robot executes all bumps; 15 release workflows (4-stage patch pipeline triggered from issue comments, rollback/promote/sandbox/verify); `release-notes.yml` uses Gemini to generate release notes; `.allstar/` Google security bot + workspace-binary-shadowing fix (`02c6c7732`).
- **Fork divergence:** Qwen fork base tag `v0.8.2` = `0612839ab` (Oct 10 2025); **`v0.8.2..HEAD` = 3,794 commits — 59% of the repo's entire history happened after the fork base.** Packages v0.8.2 (5): a2a-server, cli, core, test-utils, vscode-ide-companion → HEAD (7): +sdk, +devtools; also post-fork: evals/ (#16047 Jan 14 2026), integration-tests/, memory-tests/, perf-tests/, docs/ (enterprise-controls), tools/caretaker-agent, schemas/, sea/. **Verdict: the Qwen fork diverged from a monorepo that no longer exists — SDK packaging, policy engine, safety checkers, caretaker automation, and the whole hardening wave are upstream-only** (completes §18's graft archaeology from the other side).
- **a2a-server:** born Aug 26 2025 (`bfef867ba` #6597) — wraps the CLI agent as an A2A-protocol HTTP server (agent.ts 785 lines, task.ts 930, GCS persistence, Cloud Run-deployable); design RFC #7817; 247 commits since.
- **Community forks inheriting consumer demand:** IndenScale/open-gemini-cli (OpenAI-compatible endpoint injection; health UNVERIFIED), limkcreply/open-gemini-cli (MLX/llama.cpp/vLLM; last push 2026-04-05), joeloliver/positron-code (push 2026-07-21), zhangshushu15/omni-cli, marizmelo/llm-cli, MichaelLeib/gemini-cli-ollama.
### 31.4 Matrix row + column corrections (the STALE Gemini CLI column in §1 — updated in place)
Applied at FINAL current state (post-transition enterprise build): **16 score flips** (3 text cells gained symbols: Language TS→●, Steady RSS heavy→◐ [corrected — no published figures exist], Frontends CLI/TUI→●; 5 ◐→●: OS sandbox, permission tiers, prefix-cache, windowed file view, PTY; 5 ○→●: multi-model, encryption at rest, multi-client attach, editor telemetry, evals; 3 ○→◐: model-weakness profiles, auto-skill learning, MCP server mode) **+ 9 in-place enrichments** (tiered-eviction staged ladder, subagents + remote A2A, skills consent flow, MCP-client OAuth depth, sessions, checkpoints, cost-HUD no-$-HUD note, repo-map confirmed-none, WASM confirmed-none). Column recount **18● · 5◐ · 2○** (was 5●/6◐/10○ + 4 unscored). Every cell carries the bifurcation context implicitly — the scored artifact is the enterprise+API-key build; consumer death is recorded in 31.1, not as scores.

### 31.5 ADOPT / AVOID / WATCH (appended to §3)
- **ADOPT:** sandbox breadth as the reference taxonomy (5 backends + bwrap tool-level + per-command Sandbox Expansion Requests); policy-engine allow/deny DSL + fail-closed workspace trust + trusted folders; the staged eviction ladder (toolOutputMasking→toolDistillation→chat compression→truncation); per-install consent strings for extensions/skills; SHA-embedding nightlies (tag↔SHA traceability by construction); omissionPlaceholderDetector anti-elision guard.
- **AVOID:** overnight consumer-auth removal with no grace period + README/docs contradiction + stale quota page (trust debt outliving the product decision); handing users to a closed Go successor via curl-pipe-bash installer (migrate-to-unauditable-binary anti-pattern); building consumer-facing anything on gemini-cli upstream (enterprise-only maintenance, ~94% velocity collapse, core team gone).
- **WATCH:** open-gemini-cli fork ecosystem as the community successor lane; caretaker-agent + SSR Agent + pr-generator (a repo hiring its own maintainers) as the autonomous-maintenance pattern; a2a-server + SDK as the A2A/SDK OSS reference; Antigravity CLI governance maturation; original-team exit reason (rotation vs reassignment) UNVERIFIED.

### 31.6 Sources + confidence
- **Primary/VERIFIED:** google-gemini/gemini-cli full clone @ `9c1b0a610534d6f8120964cf2672c07807d8fc90` (git log/show/rev-parse/tag; commits carry PR numbers); geminicli.com/docs/ (shutoff banner, sandbox 2026-07-17, token-caching, policy-engine, trusted-folders, skills, hooks, subagents, remote-agents, headless, acp-mode, quota page updated 2026-06-18); discussions/28017 + /27274; developers.googleblog.com May 19 2026 announcement; README @ HEAD (free-tier staleness); tag series v0.53.x–v0.61.0-nightly; release workflows tree; discussions 27274 (enterprise tier survival).
- **Secondary/corroboration:** mcp.directory + dvnc.dev (Antigravity migration mappings); thegitreporter.com 2026-06-23 (agy immaturity, auth breakage #415); frontier.bitter.sh 2026-07-01 (governance arc); antigravity.google/docs/enterprise/; google-antigravity/antigravity-cli (no LICENSE at capture).
- **UNVERIFIED/flagged:** individual PR-page contents; original-team exit reason; the exact consumer-OAuth kill commit (docs/announcement-shipped, not code); star count 106,928 (scout live API, drifts); open-gemini-cli fork health; Antigravity CLI "Go" attribution (community-sourced, UNVERIFIED).
- **Confidence:** HIGH on bifurcation + hardening arc + fork divergence (clone + docs + tags); MEDIUM on fork-ecosystem health and Antigravity mechanics (secondary sources); LOW on nothing load-bearing.
## 32. Iteration 23 findings — jcode: the RAM-efficient solo phenomenon (scout + 1 detail agent + synth; Rust MIT, 2026-09-12)

Sweep straggler (§30.8), the largest post-micro-sweep Rust-adjacent phenomenon. Detail agent cloned `1jehuang/jcode` @ `33daef65e5786130ed490feae51e5a321ec496ec` (2026-09-12) and verified every architecture claim in source; synthesis here.

### 32.1 Identity + growth mechanics
- **19,520★ / 2,252 forks; MIT; v0.84.0 (2026-09-07); 80 crates; ~1,880 depth-1 files; 176 releases** with a verified ~20/mo cadence (12 releases Aug 25→Sep 7 2026 via releases API).
- **Solo bus factor CONFIRMED:** `1jehuang` (Jeremy Huang) authored **7,284 of 7,323 commits (99.5%)**; the remaining 39 = github-actions[bot] (31) + 4 humans with 8 commits total. No governance, no co-maintainers visible.
- **Growth = engineered-organic, Reddit-first — instrumented solo dev-marketing:** `docs/reddit_dashboard.py` checked into the repo hardcodes a 15-subreddit targeting table (subs count, author's avg/max upvotes+comments per sub, subjective relevance score, tier 1–5; r/ClaudeAI tier-1 → r/SideProject tier-5) + rendered PNG output; `.github/workflows/update-star-history.yml` daily cron (17 3 * * *) regenerates `docs/images/star-history.svg` and auto-commits — the clone's HEAD commit was literally "docs: update weekly stars chart" (perpetually-fresh social proof); README leads with "the most RAM efficient harness" + PSS tables vs pinned competitor versions, built for screenshot circulation. **Verdict: 19.5k★ measures marketing ops + RAM-niche differentiation as much as adoption; star velocity with zero contributor depth behind it.**
### 32.2 The client-server RAM architecture (verified in code) + self-dev mode
- **Daemon split:** one `jcode serve` (spawned detached via `setsid()`); all TUIs are thin clients over Unix socket `/run/user/$UID/jcode.sock`; the server owns provider connections, the MCP pool **shared across sessions**, and all session state; clients reconnect with exponential backoff; server idles out after 5 min clientless; **hot reload = server `exec()`s the new binary on the same socket (same PID)** (docs/SERVER_ARCHITECTURE.md:11-36,76-94,121-128). Socket lifecycle is unusually mature: stale-daemon socket reaped only when `flock` on `jcode-daemon.lock` proves the owner dead (issues #277/#291 regression notes) — `server/socket.rs:46-100`. Versioned harness-API bridge on its own socket (JSON→JSON translation so the daemon never breaks): `jcode-harness-api-server/src/lib.rs:1-15`; SDK `launch()` spins a private daemon under dedicated `JCODE_HOME` (`sdk/typescript/src/launch.ts`). **Why it saves RAM:** model/provider/MCP state amortizes across sessions; clients are renderers — the marginal per-session cost is a socket + render state, not a full agent runtime.
- **Cache preservation:** append-only context tracker (`stable_message_hash` + rolling `extend_stable_hash` prefix chain, 10-deep violation history) detects provider-side cache invalidation (`jcode-base/src/cache_tracker.rs:1-14,55-76`); on-disk MCP schema cache `~/.jcode/mcp-schema-cache.json` advertises last-seen tools at spawn = **zero first-snapshot cache miss in the common case**, keyed by config fingerprint (command+args+sorted env+transport+url+headers; `fingerprint_config` :63-92), reconciled against live `tools/list` (`mcp/schema_cache.rs:1-31`; kills the documented #206 late-tool-snapshot rebuild miss).
- **Session sharing:** daemon-owned sessions with attach/detach/resume and 8 resume test-scenario files; multi-session client UI still **Proposed**, not built (`docs/MULTI_SESSION_CLIENT_ARCHITECTURE.md:1-3`).
- **Self-dev mode — the most complete publicly-visible agent-self-development loop in any OSS harness:** in-repo client auto-detects and marks the session canary; tool surface `build|build-reload|test|cancel-build|reload|status` (`selfdev/mod.rs:512-517`); the loop: agent edits its own source → queued background build → `build-reload` → server exec()s the new binary, clients auto-reconnect → `ReloadContext` persisted per-session and restored as a continuation message — **the agent survives its own hot-patch with context intact** (`selfdev/reload.rs:14-45`; `CanaryStatus`/`CrashInfo{build_hash,exit_code,stderr,git diff}`/`MigrationContext` in `jcode-selfdev-types`). Guardrails: per-worktree-scope build-queue serialization with file-based records (`build_queue.rs:29-60`), `JCODE_IN_DEV_CARGO` re-entrancy flag, reload-loop suppression + never-downgrade-on-version-mismatch (a real self-hosting failure mode, issue #291, hit and fixed — `server/util.rs:88-95`), promote-vs-reload separation ("updates may advance a shared server but must not overwrite a deliberately promoted build", `commands.rs:2061-2089`). Explains the 20 releases/mo: the author's agent ships the author's tool.
- **Swarm + memory (◐ mixed):** swarm DAG task graph real in code — mode-gated spawning (swarm-deep = recursive w/ live-worker budget), orphaned children reparented to grandparent, single `VersionedPlan` per swarm via root-claimed coordinator slot, DM/subtree-broadcast/channels, notifications as safe-point soft interrupts, 4,000-char completion-report caps + mandatory `tldr` >240 chars (discipline rare in multi-agent code); own doc status honest: "Largely implemented", comm migration "in progress". **File-touch collision alerts ● VERIFIED:** write/edit publish `BusEvent::FileTouch{path,op,intent,summary}`; server dual-index FileTouchService **alerts BOTH sides on overlapping modification within a swarm** (`server.rs:1983-2105`) — reads tracked but silent. Memory graph ◐: HashMap graph `GRAPH_VERSION=2` with typed edges + BFS weights, cosine top-K via bounded heap, local all-MiniLM-L6-v2 embeddings, cascade retrieval with a sidecar LLM relevance filter **pinned to "GPT-5.3 Codex Spark"**, write-time consolidate/supersede, privacy regex bans on secrets; HDBSCAN clustering/negative memories/temporal awareness unshipped.
### 32.3 RAM-claims verdict + criticism check
- **The claim** ("10 sessions ≈ +9.9MB marginal; 1-session 27.8MB vs Claude Code ~212.7MB, OpenCode 3,237MB"): **mechanism real** (the four verified sub-mechanisms above genuinely amortize provider/MCP/session state); **methodology honest** — PSS, one disclosed Linux machine, competitor versions pinned per rerun (Copilot CLI 1.0.24 vs 1.0.27), and the headline row label carries "(local embedding off)" IN-TABLE (167.1MB with embeddings on = a 6.0x row vs Codex — the asterisk is disclosed, not hidden).
- **Verdict: directional, not citable** — self-reported, single machine, author-chosen workloads, **zero independent reproduction found**. Cite the architecture; never the numbers.
- **r/opencodeCLI "API keys and secrets all over" criticism: PARTIALLY SUBSTANTIABLE, weaker than it sounds.** Confirmed: plaintext credential files at multiple locations (`~/.jcode/auth.json`, `~/.jcode/openai-auth.json`, `~/.config/jcode/*.env`, auth-validation cache). Verified mitigations: 0o600 chmod test-asserted + Windows ACL replacement (`jcode-storage`), consent-gated per-source credential import reading external files in place (nothing copied), `!`-prefixed shell values never executed; it can read Claude Code's macOS Keychain but ships **no OS-keychain storage of its own**. Fair comparison: OpenCode/pi peers also store OAuth in plaintext `auth.json` — file+chmod is the ecosystem norm. The legitimate kernel: dot-env keys under a config dir plus a validation-cache JSON leak via misconfigured backups/dotfile syncers.

### 32.4 Matrix row — standalone note, NO new §1 column
**Why no column:** the §30.8 stop criterion holds — jcode is a solo-bus-factor phenomenon (8 human non-author commits ever) in the mention-only tier alongside Juggler/Bullet/Ante/Zot; a 28th column would misrepresent adoption depth (19.5k★ = marketing ops + RAM niche per 32.1) and the matrix stands at twenty-seven agent columns. **Standalone scored row (evidence @ `33daef65`):** client-server daemon/session amortization ● · MCP schema disk cache w/ fingerprint guard ● · append-only cache-violation tracker ● · self-dev loop w/ full guardrail set ● · solo authorship ● (7,284/7,323) · swarm ◐ ("largely implemented", comm migration in progress) · memory graph ◐ · RAM claims ◐ (mechanism real, numbers self-reported). Revisit as a Rust-adjacent column next to §26 only if co-maintainers appear.

### 32.5 ADOPT / AVOID / WATCH (appended to §3)
- **ADOPT (patterns, not code):** daemon + session amortization (the biggest structural multi-session RAM lever; exec()-same-socket zero-downtime reload for a TUI app); MCP schema disk cache with config-fingerprint invalidation (the guard everyone else omits); append-only prefix-hash cache-violation tracker (~400 LOC, provider-agnostic CI/test hook); file-touch collision alerts (small, high-value multi-agent coordination primitive); the self-dev guardrail set (serialized per-scope build queue, canary state machine, crash capture w/ diff, reload-loop suppression, promote-vs-reload separation); flock-gated stale-socket reaping.
- **AVOID:** building critical workflow on jcode binaries (bus factor 1, no governance); 80-crate crate-per-type fragmentation as a style (their own docs — COMPILE_TIME_ISOLATION_REFACTOR etc. — are fallout management; adopt the seams, not the count); citing the self-reported RAM tables without reproduction; memory sidecar pinned to one small model in a load-bearing spot; plaintext dot-env API-key layout with no OS-keychain option.
- **WATCH:** swarm DAG task-graph completion (comm migration "in progress"); multi-session client surfaces (Proposed — would strengthen the RAM story further); jcode-bench "uncontaminatable benchmark" validation; 20-releases/mo solo cadence sustainability.

### 32.6 Sources + confidence
- **Primary/VERIFIED:** 1jehuang/jcode clone @ `33daef65e5786130ed490feae51e5a321ec496ec` — docs/SERVER_ARCHITECTURE.md, SWARM_ARCHITECTURE.md, MEMORY_ARCHITECTURE.md, MULTI_SESSION_CLIENT_ARCHITECTURE.md, AUTH_CREDENTIAL_SOURCES.md; crates/jcode-base/{cache_tracker.rs, mcp/schema_cache.rs, auth/external.rs, storage/tests.rs}; crates/jcode-app-core/{server/socket.rs, server.rs:1983-2105, server/file_touch_service.rs, server/client_session_tests/, tool/selfdev/{mod,build_queue,reload}.rs, tool/{write,edit,communicate}.rs, server/util.rs}; crates/jcode-swarm-core/lib.rs; crates/jcode-memory-types/graph.rs; crates/jcode-embedding/lib.rs:369-410; crates/jcode-storage/lib.rs; sdk/typescript/src/launch.ts; docs/reddit_dashboard.py; .github/workflows/update-star-history.yml; releases + contributors APIs.
- **Secondary:** r/opencodeCLI criticism thread (via task brief); jcode.sh/bench manifesto (UNVERIFIED/unfinished); README RAM tables (self-reported, methodology disclosed in-repo).
- **UNVERIFIED/flagged:** live RAM reproduction (none exists); jcode-bench contamination-proofing; Desktop closed binaries; HDBSCAN cluster-discovery maturity; star count drift; sidecar model generalizability.
- **Confidence:** HIGH on architecture + self-dev mechanics (pinned clone, code-read); MEDIUM on growth-mechanics interpretation (dashboard + cron are facts, intent is inference); LOW on nothing load-bearing — RAM numbers carried as directional only.
## 33. FINAL: LLM-behavior elicitation — extracting the most from each model (scout + 3 detail agents + synth; 2026-09-12)

The study's closing research section, answering the founding ask: models behave differently — some verbose, some think, some think a lot, some call lots of agents, others do the work themselves — so how does a harness extract the most from each? Detail agent A fetched primary control-surface docs for all four provider families (platform.claude.com, developers.openai.com, ai.google.dev, docs.x.ai — this session); detail agent B verified the tool-call-calibration + delegation-vs-DIY evidence (4× arXiv 2026 + Anthropic engineering + Codex/GitHub issues); detail agent C verified the harness×model interaction papers, the drift/determinism literature, and production per-model variant practice (Cursor, Copilot CLI, Claude Code leak checks); this synthesis. Full VERIFIED/UNVERIFIED tagging in scratch final-elicitation-{providers,tools,harness}.md.

### 33.1 The per-family control matrix (the API surface a harness MUST wrap)

| Family | Thinking knob | Effort knob (default) | "Off" exists? | Cache interaction | Error traps |
|---|---|---|---|---|---|
| Anthropic | `thinking.type` adaptive/enabled/disabled; `budget_tokens` deprecated on 4.6, **400 on 4.7+**; display `summarized`/`omitted`/`updates` | `output_config.effort` low→max, **default `high`**; per-message effort beta (cache-preserving; Fable 5.1/Mythos 5.1/Opus 5 only) | `disabled` on Sonnet 5 + Opus 5 at ≤`high` only; Fable 5.1/Mythos 5.1 **always-on**; xhigh/max+disabled on Opus 5 → 400 | effort + budget are rendered into the prompt: changing either **invalidates cache breakpoints** — hold constant per conversation | budget on 4.7+; prefill on 4.6+; forced `tool_choice` on Fable 5.1/Mythos 5.1 (use `strict:true` + instruction); history/system/tools edits on Fable 5.1 (thinking-block binding; append-only or `drop_block`) |
| OpenAI | `reasoning.effort` × `reasoning.mode` (standard/pro, 5.6-family) | gpt-5.5 default **`medium`** (none/low/med/high/xhigh only — no max/minimal); Astra **no `none` → 400**, floor `low`; `configuration_update` item = mid-run effort (Astra only, cache-preserving) | `none` on non-Astra; `text.verbosity` low/med/high (medium default) is the LENGTH knob, orthogonal to effort | effort cache-decoupled; stateless reasoning via `encrypted_content` round-trip; reasoning-context reuse same-family only | `none`/temperature/top_p/logprobs on Astra; `max_output_tokens` can fire `incomplete` **before any visible text** (reserve ≥25k) |
| Google | `thinking_level` minimal/low/medium/high, **default high (dynamic)** | level = advisory ceiling; model self-allocates below it; 3.1 Flash-Lite defaults `minimal` | `minimal` ≈ off but **not guaranteed**; **3.1 Pro floor = `low` — thinking cannot be disabled on Pro** | thought signatures must be round-tripped verbatim in stateless multi-turn function calling or requests fail validation | `thinking_budget` + `thinking_level` together → 400; temperature <1.0 → looping/degraded reasoning (keep 1.0) |
| xAI | `reasoning_effort` low/medium/high/xhigh | **default `high` — the most expensive default of the four; SET explicitly on every call** | **never** (no off value) | encrypted reasoning via `include:["reasoning.encrypted_content"]` | presencePenalty/frequencyPenalty/`stop` → error on reasoning models; xhigh on grok-4.5 **silently downgraded to high** (no error) |

**Cross-family invariants:** (1) one effort scale, four meanings — Anthropic = whole-response token posture (cache-coupled), OpenAI = thinking depth (cache-decoupled, mid-run mutable), Google = advisory ceiling, xAI = thinking depth or **agent count** (grok-4.20-multi-agent: low/medium → 4 agents, high/xhigh → 16 — a cost multiplier, not a depth dial; no `max_tokens` brake; all agents' tokens billed). (2) "Off" exists on exactly three surfaces: OpenAI `none` (non-Astra), Gemini `minimal` (non-Pro, non-guaranteed), Anthropic `disabled` (Sonnet 5 / Opus 5 ≤ high). (3) Every family has a 400-trap list → the harness must feature-detect **per model, not per provider** (effort-support grids are fragmented: xhigh/max support differs model-by-model inside Anthropic itself).

### 33.2 Per-model behavioral profiles (documented tendencies)

| Model | Behavioral signature the harness must plan around | Counterweight |
|---|---|---|
| Claude Opus 4.6 | **over-exploration**: more upfront context-gathering/multi-thread research unprompted; "If in doubt, use X" prompt language now *overtriggers*; vendor-acknowledged over-effort ("dial effort down from high to medium") | commit-to-approach prompt ("choose an approach and commit... avoid revisiting decisions unless contradicted"); soften MUST-use-tool language |
| Claude Opus 4.7 | respects effort *more strictly* (low/medium scope to what was asked; shallow on hard problems → raise effort, don't prompt); `max` **overthinks** structured-output tasks | start `xhigh` for coding/agentic; `max_tokens` ~64k at xhigh/max; `max` only with eval headroom |
| Claude Opus 5 | thinking ON (adaptive), display `omitted`; **effort does not reliably shorten visible responses**; thinking-off → may emit tool calls as plain text/internal XML; may **under-delegate** | explicit length instructions (effort won't do it); subagent-use prompt; mitigation prompt when thinking-disabled |
| Claude Fable 5.1 | thinking always-on (no config accepted); **parallel-call decay**: fewer parallel tool calls in long agent loops where next reads are only implied; fewer progress messages; **fewer search calls at `low` effort** (answers from memory); reasoning-elicitation → refusal `stop_details:"reasoning_extraction"` | per-round one-sentence batching instruction (turn-scoped, `clear_at:"next_user_message"`); `display:"updates"` + progress prompts; raise effort or instruct when to search |
| Sonnet 4.6 / 5 | latency surprises if effort left implicit | SET effort explicitly; medium = documented recommended balance (4.6) |
| GPT-5.5 | **verbosity default: concise/direct/literal**; documented overthinking warning — conflicting instructions, weak stopping criteria, or open-ended tool access + higher effort → unnecessary searching + quality regressions | outcome-first prompts with **explicit stopping conditions + retrieval budgets**; preamble prompting for TTFT; personality blocks when customer-facing tone matters |
| GPT-6 Astra | **asks more clarifying questions / stops early**; **delegates LESS than desired** (opposite of Claude-family eager); over-thorough testing on small tasks; list/table-heavy output; sensitive to skill-file conflicts | bias-to-action + delegation prompts; prose-over-tables styling; audit AGENTS.md for conflicting guidance |
| Gemini 3.1 Pro / Flash | thinking dynamic (self-allocates below the ceiling); long TTFT at `high`; temp <1.0 degrades reasoning | temperature 1.0; accept the Pro `low` floor; harness duty = plumbing thought signatures |
| grok-4.6 / 4.5 | always-on reasoning at default `high` = expensive out of the box | SET effort explicitly even for cheap calls |
| grok-4.20-multi-agent | effort = **fleet size** (4 vs 16 agents), not depth; only leader output exposed | 4 = focused queries; 16 = deep research ("significantly more tokens") |

**Delegation posture is a family-level property, not a universal:** Claude-family defaults *eager* (early Anthropic agents spawned 50 subagents for simple queries, scouring the web endlessly); Codex-family defaults *explicit-only* (`spawn_agent` refuses unless the user literally asks for sub-agents — depth/thoroughness/research do not count; defaults max_threads 6, max_depth 1) and suffers **silent degradation** when repo standing authorization conflicts with session spawn policy — it skips mandated subagents or duplicates delegated research in the parent thread (#16996/#18319); ChatGPT gates proactive delegation to the Ultra tier (product decision, not model judgment); xAI collapses the whole question into an `agent_count` billing knob.
### 33.3 Tool-call + delegation calibration (the evidence + the structural controls a harness should offer)

**The core calibration fact is a knowledge–action gap.** Tool necessity is linearly decodable from pre-generation hidden states at **AUROC 0.89–0.96 across six models** — exceeding the models' own verbalized reasoning — yet generation ignores the signal: "models already know when tools are needed, but fail to act on this knowledge" (When2Tool, arXiv 2605.09252). The bias is mechanistic and systematic, not noise: a persistent **call offset** in the call/no-call mapping (arXiv 2605.18882, causally correctable via SAE steering) plus an **epistemic illusion** — models misjudge internal knowledge boundaries and prefer tools over what they actually know (arXiv 2604.19749). Actionable consequences:
- Prompt-only suppression is provably **coarse**: it kills necessary calls along with unnecessary ones; Reason-then-Act costs disproportionate accuracy on hard tasks.
- Call propensity is nonetheless a **controllable scalar**: Probe&Prefill (hidden-state probe → prefill a steering sentence) cuts **−48% tool calls at −1.7% accuracy**; residual-stream steering dials call rate **monotonically ~0%→>90%** and induces calls precisely where the model would otherwise hallucinate (arXiv 2608.25198).
- Eval/reward design *causes* overuse: outcome-only grading induces it; balanced rewards cut unnecessary calls **−66.7% (7B) / −60.7% (32B)** without accuracy loss; DPO boundary alignment achieves **−82.8% tool usage while improving accuracy**. Any harness-side judge rubric must score "right tools, reasonable number of times".
- Parallel-vs-sequential is per-model AND **harness-teachable**: formatting tool_results one-message-per-result silently *teaches* models to serialize; batching all results in ONE message restores parallelism; `disable_parallel_tool_use` gives a hard cap. Claude 4+ parallel by default; Fable 5.1 under-parallelizes in long loops; 3.7 Sonnet needed a `batch_tool` meta-tool.
- **Token budget is the dominant outcome variable**: multi-agent ≈ 15× chat tokens; token usage alone explains **80%** of BrowseComp variance (3 factors = 95%: tokens, #tool calls, model choice); agents cannot self-judge appropriate effort — Anthropic embedded literal scaling rules in orchestrator prompts (fact-find = 1 agent/3–10 calls; comparisons = 2–4 subagents/10–15 each; complex = >10 subagents with divided responsibilities) plus orchestration caps (MAX_CONCURRENT 4 / MAX_TOTAL_SUBTASKS 200) and granularity guidance ("a focused review of a few hundred lines rarely needs more than about ten subtasks").

Structural controls derived from this evidence: two-tier call governors (soft budget injected in context + hard budget denying calls with a structured error, defaults keyed to task class); task classifier → profile at dispatch; delegation-throttle prompts with verbatim numbers (default ON for eager profiles, plumbable-but-present for explicit-only); split subagent caps (`max_concurrent` × `max_total_subtasks` × `max_depth`=1); token-budget governor as the PRIMARY economic control with an auto-degrade ladder (collapse fan-out → serialize → solo); spawn-authorization classes (standing auth for read-only/validation/exploration vs fresh auth for edits/commits/deploys) with **never a silent fallback to parent-DIY** — if spawn is blocked, say so and ask once; parent-duplication guard (overlap detection between lead tool calls and subagent tasks); tool-efficiency rubrics in eval/judge loops; parallelism normalization utilities; all of it exposed as per-profile, per-task-class config (TOML surface in the scratch tools file).

### 33.4 The harness×model doctrine (7.8× variance, trained-format matching, per-model variants, joint evaluation)

- **The unit of evaluation is the harness–model pair.** Binding Constraint Thesis (arXiv 2605.23950): on long-horizon tasks harness-induced variance ≥ model-induced variance — measured **HV/MV = 7.80×** (3 models × 3 harnesses, SWE-bench Verified subset), **6 of 9** model-pair/harness-pair comparisons flip rankings; adding ONE search subagent (+2.1pp, "comparable to a routine model upgrade") flips the SWE-bench Pro ordering of MiniMax 2.5 vs Opus 4.6; Opus 4.5 scores 45.9% on SEAL vs 55.4% on Claude Code (+9.5pp harness-only) while six frontier models span only 4.9pp under one scaffold. Corroboration: HAL cross-scaffold gaps 34–48pp; Epoch monitoring 11–15pp scaffold-only.
- **Cost diverges more than accuracy**: 2 models × 3 harnesses at equal pass rate (38–50%) spans **40.8–41.9× tokens/solved** (Goose ~28–37K vs OpenCode ~1.15–1.55M; arXiv 2607.22585); Terminal-Bench 2.0 precedent: 65× input tokens for +5.7pp. Report tokens/solved + idle/no-action turns (10× spread) + failure-category vector — **failure fingerprints are harness properties** (Goose = clean-stop-never-verifies; OpenHands = accepts-plausible-but-wrong; OpenCode = idle-spinning; each fingerprint replicates across both models).
- **Adapter/tool-contract choice can dominate the headline**: GLM 5.1 on Claw-SWE-Bench goes 19.1% → 73.4% Pass@1 from adapter-only → full-adapter (apply-failure 69.1% → <1.5%); the harness axis alone moves models up to 27.4pp. `apply_patch` off-family fails ~50% (Grok 4); hashline anchors lifted a weak editor 6.7%→68.3%; AST-targeted edits: zero format failures. **RL training co-bakes model ↔ native tool shape** (Anthropic search/replace, OpenAI apply_patch): newer Claudes emit *more* malformed calls to a custom `edits[]` schema than older ones — schema priors leak invented fields; edit-format benchmark results are themselves model-dependent (one format +48pp on one model, −21pp on another).
- **Constraints are not model-agnostic** (arXiv 2608.26197): one first-pass deterministic harness improved reproducibility in 1 of 4 model-task cells, DEGRADED 2, null 1; latency split by model; adding schema-validated structured planning on top → Determinism Index 1.000 in 3 of 4 cells. Measure per-model cost before shipping any harness change.
- **Production practice already routes per family**: Cursor ships patch-format edit tools to OpenAI models and string-replace to Anthropic ("that's what they were trained on") + per-model prompt styling ("OpenAI is literal and precise, Claude is more intuitive") + multi-week onboarding per new model (one model's "context anxiety" fixed by prompt adjustment); Copilot CLI includes `apply_patch` **only** when the active model is Codex-family; mid-conversation model takeover = OOD history + cache misses → stay with one model per task, switch at task boundaries only. Per-model launch-counterweight system prompts (lean_prompt / MODEL-LAUNCH paragraphs) remain **UNVERIFIED** (0 grep hits in the leaks repo) — adopt the pattern only gated on our own evals.
- **Doctrine**: minimum valid evaluation design = 2×2 model×harness grid; exactly two valid regimes — locked-harness (our regression lane) or factorial (our harness experiments; report HV, MV, reversals, partial eta-squared); undisclosed-scaffold deltas <~10pp are unattributable noise; every internal run carries an ETCSOVG Harness Card (Execution/Tool/Context/Scheduling/Observability/Verification/Governance).

### 33.5 Drift & determinism implications (temp-0 ≠ deterministic; early-divergence signals; consistency gates)

- **Temp-0 is not deterministic**: exact repeats at T=0.0 yield 24.0% unique outputs (gpt-4o-mini) vs 9.3% (llama3.1-8b); perturbed inputs 57.2%/27.4% (arXiv 2601.19934). Attribution nuance: much API drift is the **serving stack** (dynamic batching, queuing, numerics, MoE routing) — "a systemic property of the inference pipeline", not a user-controllable parameter; a seed usually cannot remove it in managed services (arXiv 2606.08998). One flipped near-tie token under greedy decoding redirects a tool call → different branch → different outcome: read "temperature zero" narrowly (disables intentional sampling, does not guarantee trajectory identity).
- **Early-divergence signal**: 60% of run-to-run divergence occurs in the first two pipeline steps; structural consistency is real while argument consistency is weak (**TSS 0.87 vs AC 0.69**); high-TSS cells score 90.2% vs 61.2% — **a structurally deviant run is ~3× more likely to fail, and TSS is a correctness-free reliability proxy** (arXiv 2605.28840). Cheapest consistency lever = unambiguous task specs (beats model choice; ambiguity cuts AC 28%). Assert on structured behavior, never NL text (<5% exact match even when tool sequences are identical).
- **Consistency gates**: Pass⁵ = 53% vs Mean@5 = 77% (**24.4pp consistency gap**; ~32% of tasks mix passes and failures across runs) — report BOTH; resample flip-prone steps offline, distill into episodic-memory guidelines → **+16pp Pass⁵ same-task** at zero Mean@k cost; trajectory-consistency AUROC ≈0.70 usable without ground truth (arXiv 2609.08832). Variance budgets per output class (strict for numbers/patches, loose for prose); drift-attribution protocol when a score moves: (i) model/route/version change, (ii) harness/config diff, (iii) run-to-run sampling, (iv) serving-stack drift — only (iii) is noise.
### 33.6 THE ELICITATION PLAYBOOK — what to SET, what to PROMPT, what to THROTTLE

**Principle: SET the deterministic knobs (API params) for depth/cost/latency; PROMPT for each model's documented behavioral pathologies; THROTTLE structurally what neither can bound. Never prompt for what a parameter controls; never parameterize what only structure can bound.**

- **SET per family:** Anthropic — `output_config.effort` explicitly per workload class (never trust the `high` default); `thinking.type:"adaptive"` on 4.6+; `display:"omitted"` for TTFT (or `updates` for UI); `max_tokens` as the only hard ceiling (64k+ at xhigh/max); hold effort + budget constant for the life of a cached conversation; change effort mid-run only via the per-message beta (Fable 5.1/Mythos 5.1/Opus 5). OpenAI — effort `medium` start for gpt-5.5, `low` before `none` when tools matter (Astra: no `none`); `text.verbosity` independently for length; `configuration_update` items for Astra mid-run changes. Google — `thinking_level` only (never alongside `thinking_budget`); temperature 1.0. xAI — `reasoning_effort` explicitly on EVERY call; strip penalties/`stop` from requests.
- **PROMPT per model:** Opus 4.6 → commit-to-approach + soften tool-MUST language; Opus 4.7 → raise effort on shallow reasoning rather than prompting around it; Opus 5 → explicit length + delegation asks; Fable 5.1 → per-round batching sentence + progress-update requests + "search when unsure" at low effort; GPT-5.5 → stopping conditions + retrieval budgets + preamble (the documented overthinking antidote); GPT-6 Astra → bias-to-action + delegation + prose-over-tables; any model → per-model counterweight addenda (small model-ID-gated paragraphs against known biases: over-commenting, false-confidence, over/under-thoroughness, assertiveness) only when our own evals show the bias.
- **THROTTLE structurally:** two-tier call governors keyed to task class (~3–10 calls fact-find / 10–15 comparisons / watchdogged larger); split subagent caps (concurrent × total × depth=1); token-budget governor primary with auto-degrade ladder; delegation modes (explicit/workflow/auto) + spawn-authorization classes with LOUD spawn-block refusals; parent-duplication guard; tool_results always batched in ONE message + `batch_tool` meta-tool for under-parallelizers + `disable_parallel_tool_use` for mutating/ordering-sensitive tools; edit-format routing per family; model catalog carrying per-model effort levels + the 400-trap table; tool-integration tests re-run on EVERY model upgrade.
- **Evaluate jointly, gate on consistency:** regression = locked-harness; harness experiments = ≥2×2 factorial with Harness Cards; N≥3 smoke / N≥5 release gates reporting Mean@k AND Pass^k; TSS + divergence-point telemetry with a golden-prefix replay suite (first two tool calls vs a pinned reference) as the fastest regression tripwire; store consistency scorecards (not just outcomes) in session memory; spec quality before model spend.

### 33.7 ADOPT / AVOID / WATCH for our plan

- **ADOPT:** the SET/PROMPT/THROTTLE doctrine as the constitution of our model-profiles layer; per-model 400-trap feature detection in the model catalog; hold-effort-constant cache discipline + per-message effort beta; explicit effort on every call (xAI especially); two-tier call governors + split caps (concurrent/total/depth=1) + token-budget auto-degrade as the economic control; spawn-authorization classes with no-silent-DIY-fallback; parallelism normalization (one-message tool_results, batch_tool, disable_parallel_tool_use); per-family edit-format routing + the schema-similarity rule (keep custom schemas close to the active family's native shape); joint (model×profile) evaluation — locked-harness regression + 2×2 factorial experiments + ETCSOVG cards + tokens/solved + failure-fingerprint vectors; TSS/golden-prefix/Pass^k consistency gates; schema-validated structured planning as the determinism lever (measure per-model latency cost first); model-ID-gated prompt counterweights gated on our own evals (Cursor/Copilot precedent).
- **AVOID:** prompt-only suppression as the sole tool governor (provably coarse); one global call cap across task classes (~10× spread in correct budgets); outcome-only eval rubrics (causally induce overuse); silent spawn-block → parent-DIY fallback (the most damaging real-world failure mode in the evidence); the per-family 400-traps (budget_tokens on 4.7+, prefill on 4.6+, forced tool_choice on Fable 5.1, budget+level together on Gemini, `none`/sampling params on Astra, penalties/`stop` on Grok); raising effort on conflicting-instruction/open-ended-tool tasks without evals; mid-task model switches without a summarization boundary; single-run conclusions at temp-0; assuming a newer model is a drop-in upgrade (malformed-call rates can RISE — Opus 4.8/Sonnet 5 vs custom schemas).
- **WATCH:** mechanistic tool-propensity controls (Probe&Prefill −48%@−1.7%; steering alpha 0→90%) reaching commercial API surfaces; fleet-size-as-effort knobs spreading beyond grok-4.20-multi-agent; GPT-6 Astra delegation posture + verbosity/effort decoupling spreading across families; model-launch counterweight prompt patterns (leak UNVERIFIED, directionally supported); per-model profile registry drift — effort-support grids are fragmented and churn per release, re-verify per model upgrade; joint-evaluation standards (Harness Cards, variance decomposition) spreading through the eval literature.

### 33.8 Sources + confidence

- **Primary/VERIFIED (fetched 2026-09-12):** platform.claude.com (thinking, extended-thinking, effort + mid-conversation beta, prompting best practices, Fable 5.1 migration guide, parallel-tool-use, tool-use overview/troubleshooting, Opus 4.6 announcement, multi-agent engineering blog); developers.openai.com (reasoning guide, GPT-6 Astra + gpt-5.5 guidance, Codex subagents docs + issues #16996/#18319/#18513/#23496, ChatGPT subagents docs); ai.google.dev (Gemini 3 guide, thinking/thought-signatures); docs.x.ai (reasoning, multi-agent, CLI reference).
- **Primary/VERIFIED (papers, exact numbers checked):** arXiv 2605.09252 (When2Tool/Probe&Prefill), 2605.18882 (call offset/AMCS), 2604.19749 (Tool-Overuse Illusion/DPO), 2608.25198 (representation steering), 2605.23950 (Binding Constraint 7.80×/SEAL/WarpGrep/TB2), 2607.22585 (Scaffold Effect 40×), 2608.26197 (determinism/structured planning), 2606.12344 (Claw-SWE adapters), 2601.19934 (temp-0 drift), 2606.08998 (agent stochasticity layers), 2605.28840 (TSS/consistency), 2609.08832 (consistency gap/episodic memory), 2609.00006 (11-harness anatomy); Anthropic multi-agent engineering blog (50-subagent failures, effort-scaling rules, 80%/15× economics).
- **Secondary/corroboration:** Stencil "The Harness Problem" (apply_patch 50.7% off-family failure; hashline 6.7→68.3); Signal/Youpan (RL schema-prior leakage on Opus 4.8/Sonnet 5); Geometrica AST-edits (model-dependent edit formats); Cursor engineering summaries + Model-Harness-Fit + Copilot SDK changelog (per-family tool routing, OOD takeover, per-model onboarding).
- **UNVERIFIED/flagged:** Claude-Code per-model system-prompt counterweights (lean_prompt/MODEL-LAUNCH — 0 grep hits in system_prompts_leaks; adopt the pattern gated on our evals only); Opus 4.6 "subagent predilection" exact wording (adjacent evidence only); text.verbosity 560/849/1288 numbers (GPT-5-era cookbook); Gemini thought-signature exact 400 wording (requirement confirmed); Scaffold Effect Zenodo DOI (cite arXiv/OpenReview).
- **Confidence:** HIGH on the control matrix (all four families' primary docs read this session) and the joint-evaluation numbers (exact-verified against full texts); MEDIUM on production per-model variant practice (consistent secondary reports, no primary from Cursor); LOW on nothing load-bearing — every unverified item carried as a caveat, never a claim.

