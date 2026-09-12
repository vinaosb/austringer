# Iteration 18 — Agent B: Warp Git + Changelog Archaeology + Privacy
**Date:** 2026-09-11 · **Agent:** B (archaeology slice) · **Target:** warpdotdev/warp (public 2026-04-28) + warpdotdev/docs + docs.warp.dev
**Method:** cloned warpdotdev/warp (depth 3000) + warpdotdev/docs (depth 300); mined changelog MDX sources in docs repo (canonical source of docs.warp.dev/changelog/2024|2025|2026/); web verification for blogs/press.

---

## 1. GIT ARCHAEOLOGY (warpdotdev/warp, ~4.5 months public)

### 1.1 Initial dump — squashed, no preserved history
- **Root commit:** `0dbd3d567a2090874c08a6cc91e091aebd933847` — "Initial public release of Warp." — author **vorporeal** (Vorporeal, Warp eng), 2026-04-28T13:40:51Z, **+1,431,549 lines across 300 files**. All content added in one commit: CODE_OF_CONDUCT.md, CONTRIBUTING.md (168 lines), Cargo.lock (16,782 lines), LICENSE-AGPL (663 lines), LICENSE-MIT.
  https://github.com/warpdotdev/warp/commit/0dbd3d567a2090874c08a6cc91e091aebd933847
- **History NOT preserved.** No 2021–2026 internal history; the entire client arrives pre-squashed. However, the first ~2 weeks of commits layered on top carry *pre-announcement* dates (see 1.3), i.e., the repo opened with a replayed tail of internal history on top of a squashed root (hypothesis — consistent with observed week counts, not directly proven).

### 1.2 Warp-internal split — the sync pipeline (SMOKING GUN)
- `.github/workflows/repo-sync.yml` wires **public `warpdotdev/warp` ↔ private `warpdotdev/warp-internal`** via `warpdotdev/repo-sync` reusable workflows (`sync`, `restack`, `approve`), triggered on every push/PR to master. Client development actually happens in the private monorepo and is continuously mirrored out; the `repo-sync/private-to-public/*` branches (11edee2, 8216cdd, ea93f25, f5e5d06) are the transport.
  File: https://github.com/warpdotdev/warp/blob/master/.github/workflows/repo-sync.yml
- **What stayed closed (FAQ.md, explicit):** "The **server**, the **Warp Drive backend**, and **Oz** (our agent orchestration layer) are not in this repository and remain proprietary today." Plus: "Not today. Warp's built-in agent harness runs **server-side** and isn't open in this repo today" (why you can't plug Codex/Claude subs into Warp's built-in agent); hosted authentication also closed. "Will the server or Oz ever be open-sourced? We haven't committed to a date."
  https://github.com/warpdotdev/warp/blob/master/FAQ.md
- **Internal ticket systems leak everywhere** in branch/commit names: APP-*, REMOTE-*, REV-*, WAR-*, QUALITY-*, CODE-*, CSAT-*, GROW-*, DEVX-*, DOCFAC-* (e.g., commit `fbbfc41` "Fix grep tool ParseIntError… (QUALITY-1792)"; docs `bea5c04` "(GROW-6090)"). Jira-style IDs survive the public mirror.

### 1.3 Branch structure & cadence
- Default branch `master`; no git tags on master — releases are branch-based: `stable_release/v0.2026.04.29…` / `preview_release/v0.2026.04.29…` weekly pairs (04.29, 05.06, 05.13, 05.20, 05.27, 06.03 — same SHA promoted to both, e.g. `2249469e`). 53 GitHub Releases, all published by `github-actions[bot]`; latest stable seen v0.2026.06.08, docs release automation ("weekly release updates (2026-09-03)" commit `8c88bf1`/`91835fc` in docs repo).
- **Huge live branch farm** (hundreds of refs on origin): `oz-agent/*`, `oz/*` (e.g. `oz/queued-prompt-question-mark` + 10 variant branches — visible agent retry/fork behavior), `zach/warp-cli-*` (~80 branches — the Warp CLI saga), `worker/*`, `safia/mirror-issues`, `warp-factory-test/*`, `petra/faq-openai-and-training` (FAQ edit about OpenAI/training in flight), `saga/pricing-transparency-*` (Aug 2026 pricing work).
- **Cadence (2,368 commits on master, 2026-04→09, fetch-depth-limited):** avg ~125/wk; peak **W19 (May 4–10): 240**; launch week W17 (Apr 19–25, *pre*-announcement): 165 → evidence the repo was pushed/populated days before the Apr 28 blog; recent weeks ~43–83. Latest observed HEAD commit: `4143c09` "Bump command signatures for du --time (#15950)" — PR numbering passed **#15,950** in ~4.5 months (~115 PRs/day incl. issues-side traffic).

### 1.4 Community contributions — yes, but bot-dominated
- Contributor count: **120** (repo metadata). Author mix in recent 200 master commits: **`warp-agent-staging[bot]` 132 (66%)**, dependabot 13, Warp staff (~30), externals ~15. Matches OpenAI's claim that "agents now co-create around 90% of the company's pull requests" (openai.com/index/warp, 2026-05-27).
- **Merged external PRs (verified SHAs):**
  - `787c8697` Xavientois — #15533 Credits/Dollars usage toggle
  - `1c6708dd` Xavientois — #15148 **per-category dollar pricing breakdown wired into GUI usage surfaces** (substantial feature by an external!)
  - `ad28e590` bbbiggest — #15566 Windows RDP device-loss renderer recovery
  - `e36efd06` Niyas Hameed — #14618 theme background animation
  - `40e39717` Ben Holmes — #15496 last-tab shortcut fix
  - `720871fe`, `a1cc3a3d`, `def3fd0e` — Xavientois pricing-transparency series
- `.github/workflows/label_external_contributors.yml` exists; changelog credits externals ("thanks @leozeli!" in 2026.04.29 notes).
- **Agent attribution:** 2026.04.29 release added "AI setting to control whether Oz adds an **attribution co-author line** to commit messages and PRs" — the bot-signature knob.

---

## 2. CHANGELOG ARCHAEOLOGY 2024–2026 (docs.warp.dev/changelog/{2024,2025,2026}/, mined from docs-repo MDX)

### 2.1 Agent Mode naming evolution + "Warp AI" brand retirement
| Date | Event |
|---|---|
| 2023-04 | "Warp AI" brand launched (OpenAI-powered chat in terminal) — Wikipedia; 2023 changelog has **12** "Warp AI" mentions |
| 2024-02 | "**New Agent Mode in Warp AI**: use plain English on the command line for multi-step workflows" — Agent Mode debuts as a sub-brand of Warp AI (2024.mdx L497); same month "free preview for Warp AI and Warp Drive for teams has ended" (monetization, L564) |
| 2024 | "Warp AI" fades: only 4 mentions all year; features rebranded "Agent Mode" |
| 2025-2026 | **Zero** "Warp AI" mentions — brand fully retired; naming now: Agent Mode (local), Warp Agent (cloud), Warp Agent CLI (TUI) |
| 2025-06-20 | **Warp 2.0 = "The Agentic Development Environment"** launch (agents panel, Universal Input, codebase context "without storing any code on Warp's servers", 70% SWE-bench claim, pay-as-you-go overages) |
| 2026-08-13 | "Renamed the **Oz agent UI to Warp Agent** across the app and CLI" (#11022) — Oz de-branded *inside the client* |

### 2.2 Oz → Automation Platform rename (deadline Oct 6, 2026)
- The rename is **docs/platform-first, absent from the client changelog**: `platform/index.mdx` L15: "**Oz is now the [Automation Platform]. Only the name changed.** … The `oz` CLI and the web app **keep the Oz name until October 6, 2026**."
  https://docs.warp.dev/platform/ (source: docs repo `src/content/docs/platform/index.mdx`)
- Docs commits: `3434130` (#632 "Automations rename"), `bea5c04` (#680 "CLI/API sunset callouts", GROW-6090), `5ddf5a5` (#681 "**correct Oz CLI/API/web app holdout date to 10/6/2026**"). Residual Oz naming still being scrubbed: 2026.09.09 renamed "Oz Cloud API Keys" settings page → "API keys" (warp PR #15424 = commit `7a7cd7a`, seen at recent HEAD).
- Also binary-level rename: 2026 release "The Warp **TUI binary renamed from `warp-tui` to `warp`**" (#13904).

### 2.3 Agents 3.0 arc (Full Terminal Use rollout)
- **2025-11-18/19 (v0.2025.11.18.12.24 + .11.19.08.12):** branded **Agents 3.0** — blog: https://www.warp.dev/blog/agents-3-full-terminal-use-plan-code-review-integration (2025-11-19). Features: **Full Terminal Use** ("agent can use the terminal as you would: REPLs, debuggers, full-screen apps like top — only product on the market"), `/plan` spec-driven dev, Interactive Code Review, Slack+Linear integrations, web search; claimed **#1 Terminal-Bench 2.0**.
- Retrospective: warp.dev/blog/2025-in-review (2025-12-30) confirms Agents 3.0 framing.

### 2.4 Credit-model pricing introduction
- **2025-06-20 (Warp 2.0):** usage-based "pay-as-you-go overages" debut with Pro/Turbo plans — start of the credit economy.
- **2026-02-04 (v0.2026.02.04.08.20):** new paid plan **Max** = "12x more monthly credits than Build."
- **2025-12-17:** "credit transparency footer" specifying which models were used; **2026-08** (PR #15148, #15148-adjacent #15137, external author Xavientois): per-category **dollar-cost breakdown alongside credits**.
- Third-party snapshot (datastudios.org, 2026-09-04): Free 75 credits/mo, Build $18/1,500, Max $180/18,000, Business/Enterprise custom (SOC 2, SSO, ZDR). Warp does not publish per-action credit costs.

### 2.5 MCP feature timeline
2025-05-28 MCP server support for Agent Mode → 2025-09-01 Streamable HTTP transport → 2025-11-19 team-shareable MCP configs + out-of-box servers (GitHub, Linear) → 2025-12-10 custom HTTP headers, 12-17 `warp mcp list` + MCP in integrations → 2026-04-27 project-scoped MCP servers spawn from repo root → 2026 Factory MCP (docs.warp.dev/factories/factory-mcp/).

### 2.6 Reversals / renames / deprecations (selection)
- 2025-09-10: "combine 'refine' and 'cancel' into a single 'reject'" → 2025-09-24: "change the 'Reject' label to '**Refine**'" — two-way label churn within 2 weeks.
- 2025-08-20: default input 'Classic' → '**Universal**'.
- 2025-10-29: community links migrated **Discord → Slack**.
- 2026-02-04: `edit` slash command renamed → `open`.
- 2024: Settings "Subshells" tab → "Warpify".
- **2026-04-27 (v0.2026.04.27, release notes bundled into 2026.04.29 week): "Warp is now open source."** + TOML settings file + `/feedback` → public GitHub issue.
- Docs repo does terminology sweeps with sunset callouts ahead of the 10/6/2026 Oz holdout.

---

## 3. PRIVACY POSTURE

- **Docs:** Privacy & data control (docs.warp.dev/support-and-community/privacy-and-security/privacy/), **Network Log**, Secret Redaction; post-open-sourcing the privacy page now lists "Read and audit Warp's client source code at warpdotdev/warp (AGPL v3)" as a first-class transparency control.
- **Telemetry stack:** Sentry (crash) + Rudderstack (analytics); docs publish an "**exhaustive telemetry table**" of event names; claimed "high-level metrics… do not include any user generated content." Opt-out via Settings > Privacy ("Help improve Warp", "Send crash reports"); requests carry `X-Warp-Telemetry-Enabled`, and "even if this is missing… our server assumes it's disabled."
- **The keystroke question, post-open-sourcing:** AI interactions/console inputs ARE collected for AI features but "unconditionally" pass through **Secret Redaction**; and — the sharpest edge — "**Telemetry must be enabled to use AI features on the Free plan**, while paid plans can opt out at any time." Business/Enterprise are covered by a **Zero Data Retention (ZDR) agreement: "No AI interaction or console data is ever collected."** So the free tier still pays for AI with analytics; the open-sourced client lets auditors verify what's *sent*, not opt everyone out.
- **Native Network Log feature:** Command Palette → "Show Warp Network Log" → `tail -f warp_network.log`; pre-request/post-response hooks in Warp's HTTP client. **Post-open-source hardening (2026.04.27):** in-app read-only pane backed by a **50-item in-memory buffer** replaced the on-disk log file. Documented gap: **Sentry crash-report traffic is NOT captured** in the network log (SDK encapsulates networking) — crash reporting must be disabled via Settings instead.
- **Enterprise controls:** trust center (docs.warp.dev/enterprise/security-and-compliance/trust-center/), security overview, SSO, BYOLLM (Bedrock/Gemini Enterprise), team-managed keys, secret redaction with custom regex (2025-09-17), data-management/delete-account flow (24h deletion jobs). Third-party summaries confirm SOC 2 + SSO + ZDR on Business/Enterprise tiers.
- **Training pledge (FAQ.md):** "Will my issues, comments, or code be used to train models? **No.** Warp does not use contributions to this repository, or the discussion around them, for model training." (A `petra/faq-openai-and-training` branch exists in the public repo — FAQ language around OpenAI/training being actively edited.)

## 4. OPEN-SOURCE GOVERNANCE

- **Dual license mechanics:** `warpui_core` + `warpui` crates → **MIT** (LICENSE-MIT, 7 lines); everything else → **AGPL-3.0** (LICENSE-AGPL). Stated rationale (FAQ.md): AGPL "closes the network-use loophole" so a closed fork/hosted derivative can't be shipped back to users; MIT on the UI framework "where we want maximum reuse." Company use as a terminal doesn't trigger AGPL; forks allowed, proprietary relaunches not.
- **CLA — DISCREPANCY:** FAQ.md has a section "**Why is there a CLA?** The CLA grants Warp the rights it needs to redistribute contributions…" — but **no CLA file, signing flow, or CLA bot exists anywhere in the repo** (searched root, `.github/` workflows — only OWNERS-style `check_approvals.yml`, `label_external_contributors.yml`, `repo-sync.yml`). UNVERIFIED: how/where the CLA is enforced (possibly an external service); the FAQ text may be aspirational or the mechanism lives outside the repo.
- **Contribution policy (CONTRIBUTING.md):** issues-first; readiness labels (`ready-to-spec`, `ready-to-implement`, `needs-mocks`, `warp:reserved-internal` — Oz auto-rejects contributor PRs on reserved issues); feature work requires in-repo **spec PRs** (`specs/GH<n>/product.md` + `tech.md`; examples GH408, GH1063, GH1066); **Oz auto-reviews every PR, then a human SME is auto-requested**; `/warp-agent-review` re-review command (3× max); "collaborator" role (merged-PR track record) gets `@oz` dispatch + **complimentary Oz credits**; anyone may use any coding agent — but "you should not be using agents like OpenClaw to engage in conversation with our team."
- **OWNERS gating:** `STAKEHOLDERS` (CODEOWNERS format) maps paths → SMEs (default `@warpdotdev/oss-maintainers`); `check_approvals.yml` enforces owner approval per changed path.
- **OpenAI sponsorship:** primary sources say **"founding sponsor of the repository"** (warp.dev blog + newsroom, 2026-04-28) with agentic workflows powered by GPT-5.5 (openai.com/index/warp, 2026-05-27: "founding sponsor"; Zach Lloyd to Forkable: "We sort of want to be Switzerland… but having this support for the agents, because it's expensive, is really valuable, and they've been a great partner"). **UNVERIFIED — "financial support from OpenAI":** Wikipedia's Warp (terminal) article states "With financial support from OpenAI" citing ONLY Steven Melendez, Fast Company, 2026-04-28. No primary source discloses cash amount or contract terms; "sponsor" could be in-kind model/compute support. Also note Wikipedia's framing that the "Oz orchestration platform" was open-sourced **contradicts Warp's own FAQ (Oz stays proprietary)** — treat the Wikipedia paragraph as low-reliability, single-sourced. Criticism on record: Kilo Code argued agent-gated OSS + single-provider sponsorship undermines open-source credentials (forkable.io, 2026-07-01).

## 5. FIVE JUICIEST FINDINGS

1. **The open repo is a mirror of a private one:** `.github/workflows/repo-sync.yml` hardcodes `private_repo: warpdotdev/warp-internal` — all client work still happens privately and syncs out on every push; internal Jira-style ticket IDs (APP-5579, QUALITY-1792, GROW-6090) ride along in public commit messages. https://github.com/warpdotdev/warp/blob/master/.github/workflows/repo-sync.yml
2. **Agents write most of it, and say so:** `warp-agent-staging[bot]` authored 132/200 (66%) of recent master commits; an in-app setting (2026.04.29) toggles whether Oz adds an **attribution co-author line** to commits/PRs; OpenAI (2026-05-27) claims ~90% of Warp's PRs are agent co-created — external humans do land real features (Xavientois' #15148 per-category pricing breakdown, bbbiggest' RDP renderer fix #15566).
3. **The Oz→Automation Platform rename has a hard kill date:** docs state the `oz` CLI and web app "keep the Oz name **until October 6, 2026**" (docs commit `5ddf5a5`, PR #681; `platform/index.mdx` L15), while the client already renamed the "Oz agent UI" to "Warp Agent" (2026.08.13, PR #11022) — a live, dated deprecation you can watch happen.
4. **Privacy asymmetry baked into pricing:** Free-plan users cannot use AI features without telemetry (docs privacy page caution box); paid can opt out; Business/Enterprise get ZDR with "no AI interaction or console data ever collected." The 2026.04.27 release quietly replaced the on-disk `warp_network.log` with an in-app 50-item buffer, and the documented Network Log gap is that Sentry crash traffic isn't captured at all.
5. **The OpenAI money story is softer than the headlines:** Wikipedia's "financial support from OpenAI" traces to a single Fast Company piece; every primary source says "founding sponsor," Warp's CEO frames it as expensive-agent-compute support ("we want to be Switzerland… but it's really valuable"), and Wikipedia also wrongly claims Oz was open-sourced when Warp's FAQ says Oz/server/Drive backend "remain proprietary today." **UNVERIFIED.**

## 6. UNVERIFIED / LOW-CONFIDENCE REGISTER
- "Financial support from OpenAI" (cash) — single source (Fast Company via Wikipedia); primary sources say sponsor. UNVERIFIED.
- CLA existence/enforcement mechanics — FAQ mentions a CLA; none found in repo. UNVERIFIED.
- W17 (Apr 19–25) 165-commit cluster — inferred replay of internal history pre-announcement; plausible, not proven.
- Total master commit count (2,368) is fetch-depth-bounded; true count slightly higher.
- Warp's SWE-bench 70% / Terminal-Bench #1 / "nearly 1M developers" / "ARR grew 35x" — company claims, uncorroborated.
- Free-tier credit numbers (75/1,500/18,000, $18/$180) — third-party (datastudios.org, 2026-09-04); not from docs.warp.dev directly.

**Key anchors:** initial `0dbd3d5` (2026-04-28) · "Warp is now open source" changelog entry 2026-04.27 · Agents 3.0 blog 2025-11-19 · Warp 2.0 changelog 2025-06-20 · Max plan 2026-02-04 · Oz holdout 2026-10-06.
