# Iteration 14 — Google Antigravity: Trust & Archaeology (Agent B retry)

**Date compiled:** 2026-09-11 · **Slice:** security (record-only) + quota/economics arc + changelog sweep
**Mode:** breadth-over-depth, ~15 min time-box. Status flags: [CONFIRMED] = seen in primary/secondary source this session; [UNVERIFIED] = per orchestrator brief, not independently confirmed.

---

## 1. SECURITY (record-only)

### 1.1 Mindgard "Forced Descent" — persistent code-execution backdoor (Nov 19 2025)

- **Mechanism:** A malicious "trusted workspace" plants rules that make the agent copy an attacker-controlled file to the **global config dir** `~/.gemini/antigravity/mcp_config.json`. That file executes on **every app launch**, even with no project open, even with Terminal execution policy = Off, even after full uninstall/reinstall (config dir is not removed by uninstaller). Only manual deletion of `mcp_config.json` remediates. [CONFIRMED]
- **Disclosure timeline:** Reported to Google VRP **Nov 19, 2025**, issue **#462139778**; closed **~48h later** as **"Won't Fix (Intended Behavior)"**; later reopened after public pressure; Google initially called it "not severe." [CONFIRMED]
- **Sources:**
  - Mindgard disclosure page (Nov 19 2025): https://mindgard.ai/disclosures/google-antigravity-ide---persistent-code-execution
  - Mindgard blog, Aaron Portnoy (updated May 7 2026, still states "left unaddressed by Google"): https://mindgard.ai/blog/google-antigravity-persistent-code-execution-vulnerability
  - CSO Online coverage (Nov 27 2025): https://www.csoonline.com/article/4097698/security-researchers-caution-app-developers-about-risks-in-using-google-antigravity.html

**PATCH STATUS: CONFLICTED — both claims recorded, do not resolve here.**
- **Claim A (CSA: fixed):** CSA research note (Apr 22 2026): "Both vulnerabilities were responsibly disclosed and patched by February 28, 2026… the Mindgard Forced Descent finding was addressed in the same release cycle."
  - URLs: https://labs.cloudsecurityalliance.org/research/csa-research-note-agentic-ide-prompt-injection-sandbox-escap/ and PDF https://labs.cloudsecurityalliance.org/wp-content/uploads/2026/04/CSA_research_note_agentic-ide-prompt-injection-sandbox-escape_20260422-csa-styled-1.pdf
- **Claim B (axonbuild: no fix):** No fix documented as of **Aug 16, 2026** per axonbuild tracker. [UNVERIFIED — exact axonbuild page did not surface in this session's searches; URL per orchestrator brief (axonbuild domain).]
- **Corroborating B:** Mindgard's own blog, last updated **May 7, 2026**, still says the vuln "has been left unaddressed by Google" — 2+ months after CSA's claimed Feb 28 fix. Tension between A and Mindgard's own text is unresolved.

### 1.2 Pillar Security — prompt injection → RCE + sandbox escape via `fd -X`

- **Mechanism:** `find_by_name` is a **native tool** whose `Pattern` param is interpolated into the `fd` CLI without sanitizing leading `-`. Injecting `-Xsh` turns file search into **batch execution** (`fd -X sh` executes matched files). Because native tools fire **before** Secure/Strict Mode evaluation, the highest security setting never inspects the call — structural "pre-evaluation trust" gap, not a config error. Chain completes via permitted file-write (stage script → trigger via search). [CONFIRMED]
- **Timeline:** Reported **Jan 6/7 2026** (CyberScoop: Jan 6; CSA: submitted Jan 7, acknowledged same day, accepted Jan 24) → **patched Feb 28, 2026** → **bounty awarded Mar 26, 2026** → **public disclosure Apr 20, 2026**. **No CVE assigned.** [CONFIRMED]
- **Sources:**
  - CyberScoop (Apr 20 2026): https://cyberscoop.com/google-antigravity-pillar-security-agent-sandbox-escape-remote-code-execution/
  - CSO Online (Apr 21 2026): https://www.csoonline.com/article/4161382/prompt-injection-turned-googles-antigravity-file-search-into-rce.html
  - Dark Reading (Apr 21 2026): https://www.darkreading.com/vulnerabilities-threats/google-fixes-critical-rce-flaw-ai-based-antigravity-tool
  - SecurityWeek (Apr 22 2026): https://www.securityweek.com/google-antigravity-in-crosshairs-of-security-researchers-cybercriminals/
  - Pillar Security (vendor): https://www.pillar.security/blog/the-week-of-sandbox-escapes (per-brief primary post on pillarsecurity.io/pillar.security fd -X write-up: [UNVERIFIED exact URL this session]; thehackernews.com coverage also cited in brief [UNVERIFIED exact URL this session])
- **Follow-on (July 2026, same vendor):** "Time bomb in .vscode" — agent writes `tasks.json` with `runOn: folderOpen` (not on Antigravity's write blocklist) → host executes on next reopen, outside Secure Mode; Google downgraded it ("difficult to exploit", social-engineering category) while calling the report "of exceptional quality." https://www.pillar.security/blog/a-time-bomb-in-vscode-bypassing-antigravitys-secure-mode [CONFIRMED]

### 1.3 Google AI VRP posture (pattern)

- Inconsistent triage: Mindgard's report closed in 48h as "intended behavior," reopened only after publicity; Known Issues page edited "in stealth" (Portnoy via CSO). Pillar's later report got same-day acknowledgment, ~7-week fix, bounty — but July 2026 sandbox-bypass findings were **downgraded** as social-engineering-dependent. No public advisories/CVEs for either flagship bug. [CONFIRMED from sources above]

---

## 2. QUOTA / ECONOMICS ARC

| Phase | Date | What happened | Key source |
|---|---|---|---|
| Launch generosity | Nov 18 2025 | Free preview, no disclosed pricing; vague quota language ("high, generous," "meaningful"); refresh "every five hours" | https://antigravity.google/blog/introducing-google-antigravity ; https://tpsreport.news/news/google-antigravity-quota-cuts-pricing-restructure-march-2026 |
| Tightening | Jan 2026 | Quiet tightening begins; weekly lockouts appear; "bait and switch" criticism starts; one user later measured pre/post-Jan limits | Forum 137702 trajectory (below); TPS Report |
| Credits + cuts | Mar 11 2026 | "AI Plan Upgrade": AI Credits at **$25 / 2,500 credits** (token conversion undisclosed); measured cuts: one Pro user **300M → <9M weekly input tokens (~97%)**; brief cites ~92% figure [UNVERIFIED exact %]; "5-hour refresh" widely reported as actually weekly; **multi-day / ~168h lockouts** for paying users incl. Ultra; Ultra users capped mid-March with no notice | TPS Report (above); https://help.apiyi.com/en/google-antigravity-quota-cut-policy-changes-developer-impact-guide-en.html ; https://support.google.com/googleone/thread/416663138/google-ai-credits-a-new-way-to-manage-your-ai-usage?hl=en |
| Restructure | May 19 2026 | New **Ultra 5× $99.99** ($100) and **Ultra 20× $199.99** (down from $249.99); **single Gemini pool** (Flash+Pro limits merged, drawn down per API pricing, e.g. 8× tokens if Flash is 8× cheaper); **AI credits removed from base plans → overage-only** (setting: AI Credit Overages = Always/Never); $100 bonus credits for Ultra, offer exp. May 25 2026; Org plan via Gemini Enterprise Agent Platform (consumption) | https://antigravity.google/blog/changes-to-antigravity-plans ; https://antigravity.google/docs/plans/ ; https://aialleyway.com/antigravity-pricing/ |
| 429 bugs | ~Sep 4–10 2026 | Wave of **HTTP 429 RESOURCE_EXHAUSTED despite full quota**, all models, Pro & Ultra, persisting across reinstall/accounts. Community root-cause for a large subset: **bundled localhost TLS cert expired Sep 4 2026 20:55 GMT** → editor↔language-server auth fails → requests go out unauthenticated → 429; CLI shipped fresh cert, Ubuntu .deb lagged; workaround `NODE_TLS_REJECT_UNAUTHORIZED=0`; separate quota-desync backend cases also reported | Threads below |

**Sept 2026 forum threads (discuss.ai.google.dev):**
- 180907 — "Antigravity 1.23.2 - 429 Error since last night" (Sep 5 2026): https://discuss.ai.google.dev/t/antigravity-1-23-2-429-error-since-last-night/180907
- 181274 — "Agent Execution Termination Error – HTTP 429" (Sep 7 2026; multiple Pro/Ultra users, quotas full): https://discuss.ai.google.dev/t/antigravity-agent-execution-termination-error-http-429-resource-exhausted/181274
- 179354 — TLS-cert root-cause analysis (Antigravity 2.9.1): https://discuss.ai.google.dev/t/persistent-http-429-resource-exhausted-error-despite-available-quota-antigravity-2-9-1/179354
- 182074 — 429 despite quota "1% used," 3+ days: https://discuss.ai.google.dev/t/429-resource-exhausted-on-antigravity-1-23-2-despite-quota-showing-1-used-need-to-stay-on-1-x-full-ide/182074
- 182129 — still failing after refresh: https://discuss.ai.google.dev/t/antigravity-agent-still-failing-with-http-429-resource-exhausted/182129

**Hidden caps (forum 137702, Apr 2 2026, Ultra subscriber $249.99/mo):** https://discuss.ai.google.dev/t/ultra-technical-analysis-the-16-384-output-token-cap-1-024-thinking-limit-are-silently-breaking-agent-workflows-data-comparisons-and-a-request-for-transparency/137702
- **16,384 output tokens/turn hard cap, all plans** (25% of Gemini 3.1 Pro's 65,536; 12.8% of Claude Opus 4.6's 128K); not configurable, not documented.
- **`max_thinking_length` = 1,024 for Claude models** (Anthropic API supports 128K; 0.8% utilization) — on the $250/mo tier.
- **Gemini silent truncation:** `finish_reason: max_tokens` not surfaced to the agent → agent continues on broken output (correctness bug, vs Claude which errors explicitly).
- Supporting data: https://github.com/keisksw/antigravity-output-analysis [CONFIRMED repo exists per forum post; repo contents not independently reviewed this session]
- Related context threads: quota-thread with 645 replies; #136174, #136343, #136188, #136764 (all Mar–May 2026 Ultra complaints).

---

## 3. CHANGELOG SWEEP — reversals & breakage (antigravity.google/blog + /changelog)

1. **Launch breakage (Nov 18 2025):** IDE + Agent Manager launched; Mindgard found Forced Descent **within 24h**; by day ~11 multiple independent researchers (Wunderwuzzi: 5 holes incl. exfil/RCE; Swanda: indirect prompt injection) had public findings. [CONFIRMED]
2. **Agent Manager removed from IDE (announced May 19 2026):** "In an upcoming release, we will remove the Agent Manager from the Antigravity IDE, turning the IDE into a purely agent-powered IDE" — reversal of the Nov 2025 headline surface. Source: https://antigravity.google/blog/introducing-google-antigravity-2 [CONFIRMED]
3. **IDE auto-converted to 2.0 app (May 19 2026):** Existing IDE installs "will automatically update to Antigravity 2.0" (prompted to keep IDE) — 2.0 is a standalone desktop app with **no IDE**; AGY 2.0 launch-day patch 0.1 fixed duplicate-project imports, CJK title migration, Google One credits not applied. Sources: blog above; https://antigravity.google/changelog [CONFIRMED]
4. **Gemini CLI retired for consumer tiers (Jun 18 2026):** Announced May 19 2026 — Gemini CLI + Gemini Code Assist IDE extensions/GitHub stop serving Google AI Pro/Ultra/free on Jun 18 2026; replaced by Antigravity CLI (Go, same harness as 2.0); enterprise/API-key access retained. Sources: https://developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli/ ; https://github.com/google-gemini/gemini-cli/discussions/27274 [CONFIRMED]
5. **Recurring 429-class bugs in changelog itself:** v2.11.0 (Aug 26 2026) "Fixed cold-start token endpoint routing to prevent HTTP 429 resource exhaustion errors on launch" — a month before the Sep TLS-cert 429 wave; v2.9.1→2.13.0 cadence shows heavy stabilization churn (auth races, quota-panel fixes, hook/subagent hangs). Source: https://antigravity.google/changelog [CONFIRMED]
6. **Blog cadence sanity check:** model drops every few weeks (Gemini 3 Flash Dec 17 2025 → 3.1 Pro Feb 19 → 3.5/3.6/3.7/3.8 Flash May 19–Sep 1 2026); platform expansion May 19 2026 (CLI, SDK, Enterprise) → Aug 2026 (Custom Agents, IDE Extensions, Remote Control, Enterprise GA). Source: https://antigravity.google/blog [CONFIRMED]

---

## 4. FIVE FINDINGS (dates + URLs)

1. **Forced Descent patch status is genuinely conflicted** (Nov 19 2025 report → Won't Fix → CSA says fixed in Feb 28 2026 cycle vs axonbuild no-fix-documented as of Aug 16 2026; Mindgard's May 7 2026 blog still says unaddressed). CSA: https://labs.cloudsecurityalliance.org/research/csa-research-note-agentic-ide-prompt-injection-sandbox-escap/ · axonbuild: [UNVERIFIED, per brief] · Mindgard: https://mindgard.ai/blog/google-antigravity-persistent-code-execution-vulnerability
2. **Pillar Security chain (Jan 7 → Feb 28 → Mar 26 → Apr 20 2026, no CVE)** proved Antigravity's native-tool "pre-evaluation trust" defeats its own strongest mode; Google's VRP downgrades later sandbox escapes as social-engineering. https://www.pillar.security/blog/the-week-of-sandbox-escapes · https://cyberscoop.com/google-antigravity-pillar-security-agent-sandbox-escape-remote-code-execution/
3. **Quota whiplash arc:** generous Nov 2025 → quiet Jan 2026 tightening → Mar 11 2026 credits ($25/2,500) + ~92–97% measured cuts + ~168h lockouts → May 19 2026 restructure ($99.99/$199.99 Ultra tiers, single Gemini pool, credits→overage) → Sep 2026 429-despite-full-quota wave (TLS-cert expiry Sep 4 2026 + backend desync). https://antigravity.google/blog/changes-to-antigravity-plans · https://tpsreport.news/news/google-antigravity-quota-cuts-pricing-restructure-march-2026 · https://discuss.ai.google.dev/t/persistent-http-429-resource-exhausted-error-despite-available-quota-antigravity-2-9-1/179354
4. **Undocumented capability caps at the top tier** (Apr 2 2026): 16,384 output tokens/turn on all plans; 1,024 Claude thinking tokens; Gemini truncation silent. Zero official docs. https://discuss.ai.google.dev/t/ultra-technical-analysis-the-16-384-output-token-cap-1-024-thinking-limit-are-silently-breaking-agent-workflows-data-comparisons-and-a-request-for-transparency/137702
5. **Product-surface reversals within 6 months:** Agent Manager (launch headline, Nov 2025) scheduled for removal from the IDE; IDE auto-updates into the IDE-less 2.0 app (May 19 2026); Gemini CLI shut off for consumer tiers Jun 18 2026; changelog shows repeated auth/quota 429 fixes (v2.11.0 Aug 26 2026). https://antigravity.google/blog/introducing-google-antigravity-2 · https://developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli/ · https://antigravity.google/changelog

**Trust-arc one-liner:** launch on trust, triage security reports as "intended behavior," quietly reprice quotas twice, hard-cap the flagship tier without documentation, then break auth with an expired localhost cert — while shipping a CLI retirement that forces the remaining consumer base onto the shaken platform.
