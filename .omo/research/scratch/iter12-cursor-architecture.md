# Iteration 12 — Cursor (Anysphere) — Cloud-Agent Architecture & Harness Mechanics

- **Agent slice:** A (architecture) — cloud-agent infra, sandboxing/security, self-hosted machines, IDE/CLI harness mechanics, Origin, FULL matrix row
- **Date of research:** 2026-09-11
- **Target facts (verified):** Cursor is a closed-source VS Code fork (desktop IDE), a closed `agent` CLI binary, cloud agents on Cursor-managed VMs. Anysphere was acquired by SpaceX ($60B all-stock); deal formally closed **2026-08-14**; Cursor now operates inside the **SpaceXAI** division. Sources: https://venturebeat.com/infrastructure/cursor-launches-origin-code-hosting-platform-as-github-outage-exposes-opening-in-ai-coding-race , https://thenewstack.io/cursor-origin-github-alternative/
- **Legend:** ● = fully implemented, documented/verified; ◐ = partial or partially verified; ○ = absent or no evidence; UNVERIFIED = claim not confirmable from primary sources this pass.

---

## 1. Cloud Agents architecture (Temporal durable execution)

Source: **"What we've learned building cloud agents"** — Josh Ma, cursor.com blog (page dated Jun 2, 2026; third-party coverage dates the original post May 21, 2026): https://cursor.com/blog/cloud-agent-lessons

**VERIFIED findings:**
- **Migration from work-stealing to Temporal.** Original architecture: runner nodes on Amazon ECS picked up agents and looped them to completion; progress periodically snapshotted to S3; on node death another runner was elected and resumed. Cursor's own talk calls this "essentially a bad version of Temporal." Early beta ran at ~1 nine of reliability (~90%); the homegrown system's activity success was ~90%.
- **Temporal migration -> past two 9s.** Today: **50M+ Temporal actions/day across 7M+ unique workflows**; activity success >99%; **>40% of PRs merged to Cursor's internal monorepo come from cloud agents**. Cursor uses **Temporal Cloud** (not self-hosted); Cursor's own stateless workers run in AWS; frontend nodes are stateless and unassigned (any node handles any agent).
- **Workflow architecture:** moved from "eternal" agent workflows to **short workflows per task/turn** (easier version upgrades); follow-ups implemented via Temporal **signal-with-start** (signal if running, else start new workflow). Subagents are **child workflows**. ECS -> Kubernetes migration with the Temporal Kubernetes controller for worker lifecycle. **CI replay testing:** a cron dumps production workflow histories to a locked S3 bucket; CI replays a sample against proposed workflow code to catch non-determinism before deploy. Source (talk): https://temporal.io/resources/on-demand/building-agentic-consumer-products-with-temporal-at-cursor
- **Reliability substrate:** activities write to **content-addressable storage in S3 + Redis streams**; on retry, activities restart from last successful state **without replaying expensive model token sequences**. (First-party talk.)
- **Decoupling of agent-loop / machine-state / conversation-state.** Agent loop lives in Temporal, not on the VM -> pod lifecycles managed independently; readonly VMs and prewarmed VMs possible; subagents may outlive parents and run on different pod types. Conversation layer separated: **append-only storage mechanism** streams updates to web/desktop clients, with **retry-aware stream rewind** (client detects a retried step, rewinds, shows new data).
- **VM lifecycle:** workflow start checks for a reachable dedicated VM, else provisions one; VMs hibernated between messages and torn down on inactivity timers, then recreated + conversation reseeded on the next prompt -> "illusion of infinitely-lived agents." Pipelines exist to **checkpoint/restore/fork VM images** quickly and durably.
- **Computer-use subagent (VERIFIED):** dedicated subagent type with **own model routing, custom prompting, and screen recording**; **VNC + Chrome belong to the shared environment** (parent and subagent share them — parent can drive them directly, e.g., run a Playwright script). Agent controls when to invoke it; harness scaffolds it because "models aren't quite ready to handle computer use on their own."
- **Harness philosophy — "get out of the way":** deterministic harness logic progressively replaced by tools the agent controls (branch/PR tools instead of hardcoded multi-repo handling; GitHub CLI + auto-written large-output files instead of hardcoded CI-log plumbing). Cloud prompts tuned for **more autonomy** (blocking is expensive when the user isn't watching).
- **Self-healing environments:** roadmap of agents reporting missing secrets / blocked network / broken environments and self-repairing; **autoinstall** described in https://cursor.com/blog/bootstrapping-composer-with-autoinstall

## 2. Sandboxing & security

### 2a. Cloud (docs: https://cursor.com/docs/cloud-agent/security , https://cursor.com/docs/cloud-agent/security-network , https://cursor.com/docs/cloud-agent/identity)
- **Per-agent VM boundary**, "not a shared process sandbox"; one agent cannot see another's code/state. **Firecracker-based microVM infrastructure** for runtime workspaces. **Separate AWS account** from the rest of Cursor production. No Cursor employee access to VM code; access attempts monitored.
- **Egress controls:** three modes — Allow all / **Default + allowlist** / **Allowlist only**; per-user, per-saved-environment, and team-admin (org-lock) levels. The "Default" domains are the same allowlist as the desktop sandbox defaults; one allowlist governs both. Egress IP ranges published; separate **git egress proxy** for IP-allowlisted orgs (all git hosts).
- **Retention (VERIFIED):** conversation history kept **indefinite by default** (enterprise can cap: Indefinite or 90 days); **environment snapshots = rolling 90-day inactivity window**, extended another 90 days on each start/resume; snapshots cannot be deleted on demand. Delete Agent API removes transcripts+artifacts on demand.
- **Encryption:** secrets **KMS-encrypted at rest**, TLS in transit, injected as runtime env vars; data encrypted **AES-256 with per-agent keys**; **CMEK/BYOK** supported for enterprise. Runtime Secrets kept out of transcript/tool-output/commits; build-only secrets scoped to Docker build.
- **OIDC (VERIFIED):** cloud VMs expose a **local Unix socket (`CURSOR_AGENT_SOCKET`, default `/run/cursor/api.sock`)** to mint Cursor-signed **RS256 JWTs** (exp = iat+300s) with claims `iss=https://api.cursor.com`, `sub` (user:/service_account:/projected `team_id:123`), `aud`, `cloud_agent_id` (`bcId`), `agent_runtime: managed`, optional nonce; JWKS published; verifiers: AWS STS AssumeRoleWithWebIdentity, GCP, Azure, Vault. Tokens mint for the whole run — any process in the VM can mint; no per-process identity.
- **Environment config as code:** `.cursor/environment.json` (install/start lifecycle; Dockerfile or snapshot-based). Snapshots can capture `.env.local` if included (Infisical note: prefer `start`-phase secret fetch so nothing lands in the snapshot — https://infisical.com/blog/secure-secrets-management-for-cursor-cloud-agents ).

### 2b. Desktop/local agent sandbox (docs: https://cursor.com/docs/agent/security/run-modes , https://cursor.com/docs/reference/sandbox)
- **Run Modes (3 tiers, VERIFIED):** **Auto-review** (default since v3.6, 2026-05-29: allowlist runs immediately; other shell cmds sandbox when possible; rest reviewed by a **classifier on a small Cursor-managed model** — Claude 4.5 Haiku or GPT-5.4 Mini); **Allowlist** (deterministic; sandbox optional; "Ask Every Time" deprecated v3.5); **Run Everything** (no sandbox, no classifier). Docs explicitly warn: "Auto-review is not a security boundary."
- **permissions.json** (`~/.cursor/permissions.json`, `<project>/.cursor/permissions.json`, merged; team dashboard config takes priority): plain-English `allow_instructions`/`block_instructions`.
- **sandbox.json** (user + repo, merged, admin + hardcoded rules on top, cannot be weakened): modes `workspace_readwrite` / `workspace_readonly` / `insecure_none`; extra RW/RO paths; networkPolicy default-deny with allow/deny (exact domain / wildcard / CIDR), deny-always-wins; RFC1918 + cloud metadata (169.254.169.254) + IPv6 ULA blocked by default (SSRF guard). **Protected paths** always non-writable: `.cursor/*.json`, `.git/hooks/**`, `.git/config`, `.vscode/**`, `.cursorignore`, etc.
- **Mechanisms per OS:** macOS **Seatbelt** (`sandbox-exec`); Linux **Landlock + seccomp** (kernel >=6.2, unprivileged userns; Bubblewrap fallback surfaced via `CURSOR_SANDBOX_LANDLOCK_STATUS`), AppArmor profile package for CLI/remote; injected env vars `CURSOR_SANDBOX`, `CURSOR_ORIG_UID/GID`. Sandbox engineering blog: https://cursor.com/blog/agent-sandboxing
- **Extra always-on gates:** Browser Protection, File-Deletion Protection, External-File Protection.
- **Cloud Agents do not use Run Modes** — dedicated VM means no per-action approvals (documented).
- **MCPoison CVE-2025-54136 (VERIFIED):** Check Point Research, disclosed 2025-07-16, fixed in Cursor **1.3** (2025-07-29). Trust was bound to the MCP entry *name* in `.cursor/rules/mcp.json`, not command/args -> commit benign MCP, get one-time approval, later swap command (reverse shell) -> silent persistent RCE on every project open, pre-model, pure supply-chain TOCTOU. Fix: trust cache now hashes full entry (name+command+args+env) + re-approval diff prompt; did **not** retroactively invalidate pre-1.3 approvals. Sources: https://research.checkpoint.com/2025/cursor-vulnerability-mcpoison/ , https://nvd.nist.gov/vuln/detail/CVE-2025-54136 , https://vulnerablemcp.info/vuln/cve-2025-54136-cursor-mcpoison.html (affected <=1.2.4; CVSS 7.2-8.8)

## 3. Self-Hosted Machines (worker/pool model)

Sources: https://cursor.com/docs/cloud-agent/self-hosted , https://cursor.com/docs/cloud-agent/self-hosted/pool , https://cursor.com/docs/cloud-agent/self-hosted/my-machines , https://cursor.com/docs/cloud-agent/self-hosted/choose-runtime

- **Split-brain model (VERIFIED):** Cursor keeps the **agent loop, inference, planning** in its cloud; a **worker** (your machine, registered via `agent worker start`) executes tool calls: file edits, terminal, browser/computer-use, **local stdio MCP servers**. "Pools are an infrastructure-ownership choice. They do not move the agent loop out of Cursor's cloud."
- **Transport (VERIFIED):** worker opens a **long-lived outbound HTTPS** connection to Cursor backend; Cursor pushes tool calls over it. **No inbound ports, public IPs, or VPN tunnels.** Required egress: `api2.cursor.sh`, `api2direct.cursor.sh`, `downloads.cursor.com` (CLI updates + macOS computer-use install), `cloud-agent-artifacts.s3.us-east-1.amazonaws.com` (artifact upload; blockable -> artifacts fail but agent keeps working). `HTTPS_PROXY` supported.
- **Two modes:** **My Machines** (personal; browser login / user API key / user-scoped token; multiple agents can share one machine) and **Team Pools** (Enterprise; **service-account API key** auth; **one agent claims one worker at a time**; durable pools survive scale-to-zero; label routing `pool=` + `repo=`; repo-backed vs any-repo pools; `--clone-git-repos` mints GitHub tokens on claim).
- **Autoscaling:** shipped controller `agent worker controller` (`--spawn`, `--warm-idle`, `--all-pools`, claims via `POST /v0/private-workers/claim`); REST API for pools/queue (list/watch/claim/release SSE); partner hosts: AWS Lambda, Cloudflare, Namespace, Modal, Daytona, E2B, Vercel, Coder; k8s operator + Helm (anysphere/k8s-workers). Idle connection timeout default 1 hour. Health endpoints `/healthz`, `/readyz`, `/metrics` via `--management-addr`.
- **MCP transport rule on workers (VERIFIED):** stdio MCP runs on **your** machine (private-network reach); HTTP/SSE MCP runs from **Cursor backend** (Cursor manages OAuth/session).
- Admin toggles: Allow Self-Hosted Machines (opt-in) / Require Self-Hosted Machines (force routing).
- Third-party pattern (VERIFIED existence): Vercel Sandbox as compute plane via Self-Hosted Machines APIs — https://vercel.com/kb/guide/cursor-vercel-sandbox

## 4. IDE/CLI harness mechanics

### Checkpoints & rewind (VERIFIED)
- IDE: **checkpoints save snapshots of the codebase during agent sessions, created automatically before significant changes; stored locally and separate from Git** — restore any checkpoint from chat timeline / `Restore Checkpoint` button. Source: https://cursor.com/docs/agent/overview
- CLI: **`/rewind` on by default** — interactive timeline restores files + conversation to any earlier turn, per-turn file diffs, conversation-only restore, `/undo`//`/restore` aliases; rewind preserves images; **"The CLI now saves only new transcript entries at each checkpoint instead of reloading and rewriting the full conversation"** (memory-growth fix); subagents survive transient failures and **resume from their last checkpoint**; transcripts persist to disk for tooling/hooks; `--continue` resumes latest chat; resume across workspaces. Source: https://cursor.com/docs/cli/changelog

### Rules system (VERIFIED)
- Four types: **Project Rules** (`.cursor/rules/*.mdc`, frontmatter `description`/`globs`/`alwaysApply`; 4 activation modes: Always / Apply Intelligently / Apply to Specific Files / Manual @mention; plain `.md` in `.cursor/rules` is ignored), **User Rules** (settings or `~/.cursor/rules`; Agent/Chat only), **Team Rules** (dashboard; free-text + globs; enforceable — enforced rules cannot be disabled), **AGENTS.md** (root + subdirs).
- **Precedence: Team Rules -> Project Rules -> User Rules** — all merged; earlier sources take precedence on conflict. Source: https://cursor.com/docs/rules
- **Nested AGENTS.md = COMBINED semantics** (differs from a strict nearest-wins spec reading): "Instructions from nested `AGENTS.md` files are **combined** with parent directories, with more specific instructions taking precedence." Parent + child both applied; child wins only on direct conflict. `CLAUDE.md` read identically but **always applied** regardless of frontmatter. `.cursorrules` legacy, deprecation pending. Sources: https://cursor.com/docs/rules , https://cursor.com/help/customization/rules
- Rule identity = full file path (same-named rules in different dirs both apply).

### Hooks (VERIFIED) — https://cursor.com/docs/hooks
- `hooks.json` at `~/.cursor/hooks.json` (user) and `<repo>/.cursor/hooks.json` (project); also distributable via **plugins** and Enterprise dashboard (synced to team machines). Spawned processes, **stdio JSON** both directions; hot-reload on config change.
- Events: `sessionStart/sessionEnd`, `preToolUse/postToolUse/postToolUseFailure` (matchers incl. `MCP:<tool>`), `subagentStart/subagentStop`, `beforeShellExecution/afterShellExecution`, `beforeMCPExecution/afterMCPExecution`, `beforeReadFile/afterFileEdit`, `beforeSubmitPrompt`, `preCompact`, `stop`, `afterAgentResponse/afterAgentThought`; Tab-specific `beforeTabFileRead/afterTabFileEdit`; app-level `workspaceOpen`.
- Fields: `command`, `type: command|prompt` (prompt = LLM-evaluated policy), `matcher` (regex), `timeout`, **`failClosed`** (default fail-open), `loop_limit` (stop/subagentStop auto-followups, default 5; `followup_message` enables loop-style iterate-until-goal).
- Verdicts: JSON `{permission: allow|deny|ask, agent_message, user_message}` or exit code 2 = deny.
- **Cloud Agents run project hooks only** (no `~/.cursor/hooks.json`); command hooks work, prompt hooks do not; IDE-only events (sessionStart/End, beforeSubmitPrompt, Tab hooks, workspaceOpen, before/afterMCPExecution, stop, afterAgentResponse/Thought) do not fire in cloud.

### Skills (VERIFIED) — https://cursor.com/docs/skills
- `SKILL.md` + YAML frontmatter (`name` must match folder, `description`, optional `paths` globs, `disable-model-invocation`, `metadata`); recursive discovery from `.agents/skills/`, `.cursor/skills/`, `~/.agents/skills/`, `~/.cursor/skills/`; **nested monorepo skills auto-scoped to their subtree**; compat loading from `.claude/skills/` and `.codex/skills/`; scripts subdir; GitHub import; slash `/skill-name`.
- **Built-in skills:** `/automate`, `/babysit`, `/canvas`, `/create-hook`, `/create-rule`, `/create-skill`, `/create-subagent`, `/cursor-blame`, `/loop`, `/migrate-to-skills` (converts dynamic rules + slash commands -> skills; slash commands become `disable-model-invocation: true`).
- **Auto skill learning:** no background auto-curation; learning is agent-assisted authoring (`/create-skill`) + migration + a cookbook stop-hook pattern that updates skills from session outcomes. Score o.

### Subagents (VERIFIED) — https://cursor.com/docs/subagents
- Own context window; **foreground** (blocking) or **background** (returns immediately); built-ins: **Explore** (fast model, parallel searches), **Bash**, **Browser** (MCP-driven). Custom: `.cursor/agents/*.md` (+ `.claude/agents/`, `.codex/agents/` compat; project > user) with `name`, `description`, `model: inherit|<id>` incl. **model params in brackets** (`claude-opus-5[effort=high,context=300k]`, `composer-2.5[fast=false]`), `readonly: true`, `is_background: true`.
- **Nesting:** since 2.5, subagents can spawn child subagents — **one extra level only** ("a subagent launched by another subagent can't launch further ones").
- **Isolation:** default shares parent checkout; on request, each subagent gets its own **git worktree or its own cloud VM + branch** ("swarm"); **cloud subagents** via `/in-cloud` (own VM+branch) and **`/autopilot`** (agent takes over a PR until mergeable). Background subagent output at `~/.cursor/subagents/`; resumable by agent ID with full context.
- Cloud harness adds a dedicated **computer-use subagent type** (see section 1).

### Plugins spec (VERIFIED) — https://github.com/cursor/plugins , https://cursor.com/docs/reference/plugins , https://cursor.com/docs/plugins
- **Two formats:** **Agent Plugins** (open standard, `plugin.json` at root; agent-plugins.org; spec: https://github.com/agentplugins/agent-plugins-spec) and **Cursor Plugins** (`.cursor-plugin/plugin.json`; adds rules, agents, commands, hooks, **variables** with dashboard-set secrets). Component default discovery: `skills/`, `rules/`, `agents/`, `commands/`, `hooks/hooks.json`, `mcp.json`, root `SKILL.md` single-skill. **Marketplace manifest** `.cursor-plugin/marketplace.json` (<=500 plugins); published JSON Schema at schemas/plugin.schema.json; official marketplace at https://cursor.com/marketplace ; local dev via `~/.cursor/plugins/local/`.

### MCP (VERIFIED)
- Client transports: **stdio** (local process), **SSE**, **Streamable HTTP**; config `.cursor/mcp.json` / `~/.cursor/mcp.json` (project wins on name clash); variable interpolation `${env:}`, `${workspaceFolder}`; **static OAuth** (`auth: {CLIENT_ID, CLIENT_SECRET, scopes}`); Team MCP via dashboard -> "Add to Team Marketplace"; **enterprise allowlists:** command-pattern entries (stdio), URL-pattern entries (HTTP/SSE), per-tool allowlists. Source: https://cursor.com/docs/mcp
- **Cloud split (key architectural fact):** **HTTP servers are proxied through Cursor's backend — "server configurations are never present in the cloud agent's VM"; refresh tokens/headers/credentials never reach the VM.** **stdio servers run inside the cloud VM** (agent can see their config/env). SSE and `mcp-remote` NOT supported in cloud. MCP configs encrypted at rest; `env`/`headers`/`CLIENT_SECRET` redacted, unreadable after save. Source: https://cursor.com/docs/cloud-agent/capabilities
- SDK nuance: cloud stdio rejects `cwd` (throws ConfigurationError); sensitive HTTP headers redacted server-side but forwarded; local SDK agents do not load ambient config unless `settingSources` opted in. Source: https://github.com/cursor/plugins/blob/main/cursor-sdk/skills/cursor-sdk/references/mcp.md
- Security history: MCPoison (section 2b).

### CLI + Agents SDK
- **CLI:** closed binary; headless + GitHub Actions docs (https://cursor.com/docs/cli/headless.md); shell mode; ACP support (https://cursor.com/docs/cli/acp.md); slash commands, `/config`, `/resume`, `/rewind`; transcripts on disk; worker mode (`agent worker start`, `agent worker controller`). **CLI implementation language: UNVERIFIED** (no official statement found).
- **SDK (CORRECTION to task brief):** package is **`@cursor/sdk`** — official docs: "npm install @cursor/sdk... The package name starts with `@`. The bare `cursor/sdk` doesn't exist on npm." The task brief's `@anysphere/cursor-sdk` was **not found in official docs** — treat as UNVERIFIED/likely wrong. Source: https://cursor.com/docs/sdk/typescript
- SDK surface (VERIFIED): `Agent.create({local|cloud})`, `Agent.prompt()` one-shot; `agent.send()` -> `Run` with `stream()/wait()/cancel()/steer()/conversation()`; normalized `SDKMessage` events; `TokenUsage` incl. `cacheReadTokens/cacheWriteTokens`; `Agent.getUsage()` -> **billed tokens + dollar cost**; `agentId` local `agent-<uuid>` vs cloud **`bc-<uuid>`** (auto-detected on resume); **local persistence via SQLite (`SqliteLocalAgentStore`) or JSONL store under a per-workspace state root** — agents survive process restarts; per-platform `@cursor/sdk-<os>-<arch>` native binaries (sandboxing + ripgrep); requires Node 22.13+; Bun flat-build support; `systemPrompt` replacement (local only, gated); SDK hooks; custom tools; plan/agent modes; per-run sticky model override; per-run + per-agent encrypted `cloud.envVars`; artifacts list/download (cloud only).
- **Cursor Router:** `auto-smart` model with `optimize_for: cost|balanced|intelligence`; billed at routed model's list price; enterprise allowlists shape routing pool; teams can disable. Sources: https://cursor.com/docs/sdk/typescript , https://cursor.com/docs/cursor-router.md , https://cursor.com/blog/how-cursor-router-works
- Model catalog observed (Sept 2026): `composer-2.5` (Composer 2 retired; `composer-2`/`composer-2-fast` reroute to 2.5 at auth), claude-opus-5, claude-sonnet-5, claude-fable-5-1, gpt-5.6-sol/terra/luna, grok-4-6/4-5, gemini-3.1-pro, gemini-3.8-flash, muse-spark-1-3; Auto-review classifier on claude-4.5-haiku / gpt-5.4-mini.

## 5. Origin Code hosting

Sources: changelog https://cursor.com/changelog/origin-code-hosting , docs https://cursor.com/docs/origin.md , press (TechCrunch 2026-08-18, VentureBeat 2026-08-17, The New Stack 2026-08-18, InfoQ 2026-08-25, SiliconANGLE).

- **CORRECTION to task brief:** Origin shipped **2026-08-17** (early beta, all paid plans; enterprise admins can **opt out**), not September 2026. It was Cursor's **first product shipped as a fully owned SpaceX subsidiary** (acquisition close 2026-08-14).
- **VERIFIED:** Git-based hosting inside a new **Codebase tab**: repos, PRs (diffs, comments, checks, merge), code browsing, **GitHub sync**. For Origin-native repos **Origin is the source of truth — GitHub is not in the path**; for synced repos **GitHub stays source of truth** and Origin mirrors in real time (PR conversations sync both directions within seconds). Clone/push via HTTPS or **Origin CLI** (https://cursor.com/docs/origin/cli.md). Built by the **Graphite team** (acquired Dec 2025; Graphite co-founder Tomas Reimers leads; stacked-PR heritage). Launch connectors: Vercel, Depot, Buildkite. **No public projects, no built-in CI at beta.** Origin REST API published (openapi.yaml at https://cursor.com/docs/api/origin/openapi.yaml ; Grants API). Pricing/security architecture **unpublished** at beta (flagged by VentureBeat). Launch day coincided with a 6h42m global GitHub outage (SAML/OIDC/SCIM down).

## 6. Task-brief corrections & unverified items (audit trail)

1. **SDK package name:** official docs say `@cursor/sdk`; `@anysphere/cursor-sdk` UNVERIFIED and contradicted by docs wording.
2. **Origin launch date:** 2026-08-17 (not Sept 2026).
3. **Blog date:** cursor.com blog page displays "Jun 2, 2026"; several syndications date the post May 21-25, 2026. Cite the page.
4. Cloud-agent lessons blog says migration took reliability "past two 9s"; Cursor's Temporal talk says homegrown ~90% -> >99% activity success. Consistent ("1 nine -> 2+ nines").
5. CLI binary language/implementation: UNVERIFIED (closed binary; no public statement).
6. "50M actions/day across 7M workflows" — VERIFIED in both blog and talk; first-party numbers, corroborated by independent coverage.

## 7. FULL MATRIX ROW — Cursor (Iter 12)

| Dimension | Score | Evidence (with source) |
|---|---|---|
| Language | o-partial | IDE = closed-source VS Code fork (TS/Node/Electron lineage, internals UNVERIFIED); SDK = TypeScript/Node 22.13+ with per-platform native binaries (https://cursor.com/docs/sdk/typescript); CLI = closed binary, language UNVERIFIED |
| Steady RSS | absent | No published memory figures anywhere (IDE/CLI/worker). UNVERIFIED/absent |
| OS sandbox | full | macOS Seatbelt, Linux Landlock+seccomp (+AppArmor pkg for CLI), UID-remap userns, SSRF-guarded default-deny network policy, protected paths (https://cursor.com/docs/agent/security/run-modes , /docs/reference/sandbox); cloud = Firecracker microVMs, separate AWS account (/docs/cloud-agent/security); engineering blog /blog/agent-sandboxing |
| Permission tiers | full | 3 run modes (Auto-review classifier / Allowlist / Run Everything) + permissions.json allow/block instructions + team dashboard override + hooks deny/ask + always-on Browser/File-Deletion/External-File protections + enterprise MCP allowlists (run-modes, hooks, mcp docs). Cloud agents intentionally bypass approval tiers (VM isolation) |
| Prefix-cache discipline | partial | SDK exposes cacheReadTokens/cacheWriteTokens per run; Temporal activities resume from last successful state without replaying token sequences (content-addressable S3+Redis, Temporal talk); NO documented explicit prompt-cache-preserving conversation-assembly policy |
| AST repomap | partial | Codebase semantic index + Explore subagent with parallel fast-model searches (/docs/subagents, /docs/agent/tools/search.md); no published AST/repomap (tree-sitter/ctags-equivalent) mechanism in current docs — UNVERIFIED beyond search tooling |
| Windowed file view | partial | Agent reads files through Read-style tools with `beforeReadFile` hooks and `.cursorignore` visibility control (/docs/hooks, /docs/reference/ignore-file.md); no documented line-window/streaming file-view protocol |
| PTY scraping | partial | CLI has shell mode + persistent transcripts to disk + terminal tool with before/afterShellExecution observation; PTY-scraping internals undocumented (UNVERIFIED mechanism) |
| Tiered eviction | partial | Compaction exists with `preCompact` observation hook; CLI checkpointing writes only new transcript entries (incremental persistence, changelog); no documented multi-tier (summarize/drop) eviction ladder |
| Subagents | full | Built-ins Explore/Bash/Browser; custom .cursor/agents/*.md (model, readonly, is_background); 1-level nesting; foreground/background; worktree/VM isolation swarms; cloud subagents (/in-cloud, /autopilot); cloud computer-use subagent (VNC+Chrome shared env); resumable by agent ID (/docs/subagents, /blog/cloud-agent-lessons) |
| Multi-model spec launch | partial | Parallel subagents each with own model (incl. per-subagent model params); parallel cloud agents; no documented single "spec -> N heterogeneous model workers" launch primitive |
| Model profiles | full | Per-agent/per-run/per-subagent model selection with bracket params ([effort=high,context=300k,fast=false]), preset variants, Cursor Router auto-smart optimize_for cost/balanced/intelligence, per-run sticky overrides, enterprise model access control (/docs/subagents, /docs/sdk/typescript) |
| Skills | full | SKILL.md standard, 4+ discovery roots, nested monorepo scoping, paths globs, disable-model-invocation, .claude/.codex compat, GitHub import, marketplace/plugins distribution, 10 built-in skills (/docs/skills) |
| Auto skill learning | partial | /create-skill authoring + /migrate-to-skills conversion + cookbook stop-hook skill-update pattern; no autonomous post-hoc skill distillation documented |
| MCP client | full | stdio + SSE + Streamable HTTP, OAuth (dynamic + static creds), Team MCP, enterprise config allowlists, plugins mcp.json, cloud HTTP-proxied with credential redaction (/docs/mcp, /docs/cloud-agent/capabilities) |
| MCP server mode | absent | No documentation of Cursor exposing its own agent tools as an MCP server (checked docs sitemap). Absent/UNVERIFIED |
| WASM | absent | No WebAssembly extension/execution surface documented |
| Sessions | full | Local: SQLite/JSONL agent stores, resume-by-ID across restarts and workspaces, --continue, shared transcripts; Cloud: indefinite conversation history, resume from any client, signal-with-start follow-ups days/months later; Agents Window transcript search (/docs/sdk/typescript, /docs/cli/changelog, /docs/cloud-agent/security-network) |
| Encryption at rest | full | AES-256 per-agent keys, KMS-managed, CMEK/BYOK for enterprise; secrets KMS-encrypted + redacted; MCP configs encrypted; TLS in transit; encrypted VM-disk snapshots (/docs/cloud-agent/security, /docs/cloud-agent/security-network) |
| Checkpoints/rewind | full | IDE local non-git checkpoints auto-created per significant change + restore; CLI /rewind turn-by-turn timeline with files+conversation restore, /undo//restore, subagent checkpoint-resume; VM-level snapshot checkpoint/restore/fork pipelines for cloud (/docs/agent/overview, /docs/cli/changelog, /blog/cloud-agent-lessons) |
| Cost HUD/budgets | partial | Per-run TokenUsage + Agent.getUsage() dollar cost; usage dashboard, spend limits/alerts, user spend-limit admin API, pooled usage; no documented real-time in-session cost HUD (/docs/sdk/typescript, /help/account-and-billing/spend-limits.md, admin-api) |
| Frontends | full | IDE, CLI, Web (cursor.com/agents), mobile apps, Slack, MS Teams, GitHub (PR comments), Agents Window, Automations (schedules/events), SDK embeddings; one cloud agent streams to multiple surfaces (/docs/cloud-agent/capabilities, /docs/integrations/) |
| Multi-client | full | Same cloud agent controllable from desktop/web/mobile/Slack/GitHub/API concurrently; resume from anywhere via bc- ID; cloud subagents delegated from local sessions; self-hosted workers keep IDE/web/mobile UX (/docs/subagents, /docs/sdk/python) |
| Telemetry | partial | Privacy Mode (zero-data-retention routing), enterprise OTel export (/docs/enterprise/opentelemetry-export.md), Analytics APIs (DAU, model usage, agent edits, MCP/skills adoption), audit logs; first-party client telemetry internals undocumented |
| Evals | partial | cursor.com/evals public page + Grok model pages; Bugbot as review-eval loop; CI workflow-history replay testing (internal correctness eval); no user-facing agent-harness benchmark suite documented |

**Row tally (rough): 12 full / 9 partial / 3 absent** — strongest on sandbox/permission tiers, sessions/durability, encryption, subagents, frontends; weakest on published internals (RSS, MCP-server mode, WASM) and undocumented internals (cache discipline, eviction ladder, PTY).

(Score symbols: full = bullet-full, partial = half, absent = circle, per harness convention.)

## 8. Primary source list

1. https://cursor.com/blog/cloud-agent-lessons (Temporal migration, decoupling, computer-use subagent, append-only stream)
2. https://temporal.io/resources/on-demand/building-agentic-consumer-products-with-temporal-at-cursor (ECS->k8s, signal-with-start, CAS S3+Redis, CI replay)
3. https://cursor.com/docs/cloud-agent/security (+ /security-network, /identity) (Firecracker, AWS account split, 90-day TTL, AES-256/CMEK, OIDC socket)
4. https://cursor.com/docs/cloud-agent/self-hosted (+ /pool, /my-machines, /choose-runtime) (worker/pool, outbound-only)
5. https://cursor.com/docs/agent/security/run-modes + https://cursor.com/docs/reference/sandbox (desktop sandbox + permission tiers)
6. https://cursor.com/docs/rules , https://cursor.com/help/customization/rules (rules precedence, nested AGENTS.md COMBINED)
7. https://cursor.com/docs/hooks (hooks.json schema, cloud limitations)
8. https://cursor.com/docs/skills (SKILL.md, built-ins, migration)
9. https://cursor.com/docs/subagents (subagent spec, nesting, /in-cloud, /autopilot)
10. https://github.com/cursor/plugins + https://cursor.com/docs/reference/plugins (plugin spec)
11. https://cursor.com/docs/mcp + https://cursor.com/docs/cloud-agent/capabilities (MCP transports, cloud HTTP-proxy)
12. https://research.checkpoint.com/2025/cursor-vulnerability-mcpoison/ (CVE-2025-54136)
13. https://cursor.com/changelog/origin-code-hosting + https://cursor.com/docs/origin.md + press (TechCrunch/VentureBeat/TheNewStack/InfoQ/SiliconANGLE)
14. https://cursor.com/docs/sdk/typescript (+ /sdk/python, /docs/cli/changelog, /docs/cli/headless.md)
15. https://cursor.com/llms.txt (docs sitemap — paths for all cited .md pages)
