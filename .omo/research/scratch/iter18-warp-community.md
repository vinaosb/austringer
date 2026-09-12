# Iteration 18 — Warp (warp.dev) — Agent C: Benchmarks + Economics + Community
**Date:** 2026-09-11 · **Target:** Warp (NYC; founded 2020 by Zach Lloyd; "Agentic Development Environment" born from terminal) · **Compiler:** Agent C, Iter 18
**Method:** web search (exa/brave), HN Algolia API, GitHub API, warp.dev blog/docs/events, third-party press. Flags: **VERIFY** = corroborated by 2+ independent sources or primary doc; **UNVERIFIED** = single-source/self-reported/not found.

---

## 1. BENCHMARKS

### 1.1 Terminal-Bench #1 at 52% (June 2025) — **VERIFY**
- Vendor-run, **full methodology disclosed**: https://www.warp.dev/blog/terminal-bench (Jun 25, 2025).
  - Ran the official Terminal-Bench harness, exposing Warp via `AbstractInstalledAgent`; each task fed as opening user query in a fresh Agent Mode task.
  - **Hill-climbing admitted**: "Our agent passed around 65% of the test suite across all runs"; 52% = state-of-the-art average on v0.1.1, ~9 pts ahead of next submission.
  - Model mix: Claude Sonnet 4 primary + Claude Opus 4 as planning model; ~2% of requests fell back to Gemini 2.5 Pro / GPT-4.1. Permissions "Always Allow"; mocked user exempt from AI request limits.
  - Shipped-from-bench features: pty control of long-running commands (vim/REPL), auto task lists.
- Independent confirmation of result AND hill-climbing framing: Terminal-Bench maintainers own news post — https://www.tbench.ai/news/warp-sota ("Warp debuts their terminal agent at #1... resolving 52%... Read about how Warp hill-climbed Terminal-bench"). Benchmark is Stanford + Laude Institute, open source.
- Leaderboard turnover fast: Tessl/Hackernoon recap (Sep 2025) still lists Warp leading at 52% vs Terminus/gpt-4.1 ~30%.

### 1.2 Late-2025 Terminal-Bench arc (the "Nov 2025" claim) — **VERIFY**
- **Nov 11-12, 2025**: Warp submission at **50.1%** on Terminal-Bench 2.0; Zach Lloyd LinkedIn: "Warp is back on top of Terminal Bench" — https://www.linkedin.com/posts/zachlloyd_warp-is-back-on-top-of-terminal-bench-one-activity-7394175943608569856-izW3 ("Quality = Model + Harness + App").
- **Nov 18, 2025**: Gemini 3 Pro support blog: 50.1% had been "#2 on the board"; with Gemini 3 Pro rose to **59.1%** — https://www.warp.dev/blog/introducing-gemini-3-support.
- **Nov 19, 2025**: Agents 3.0 post: "Warp is #1 on Terminal-Bench 2.0" (59.1%) — https://www.warp.dev/blog/agents-3-full-terminal-use-plan-code-review-integration. (Task brief's "52% Nov 2025" is imprecise; Nov claims were 50.1% then 59.1%.)
- **Dec 11, 2025**: GPT-5.2 support; **61.14%** on TB 2.0, self-described "#2 overall", "new high watermark" — https://www.warp.dev/blog/gpt-5-2-support-terminal-bench-improvement. Leaderboard entries: 2025-11-11 = 50.1%, 2025-11-20 = 59.1%, **2025-12-12 = 61.2% (Warp's last)** — https://www.tbench.ai/leaderboard/terminal-bench/2.0.

### 1.3 SWE-bench Verified: 71% to 75.8% (#3, Sept 2025) — **VERIFY** (with caveats)
- **71.0%** (Jun 23, 2025), blog: https://www.warp.dev/blog/swe-bench-verified.
  - **Custom vendor harness** (disclosed in official submission): desktop Warp driven inside Docker via their UI integration-testing framework; agent gets no hints / PASS_TO_PASS / FAIL_TO_PASS; pass@1; submission checklist attested. PR: https://github.com/SWE-bench/experiments/pull/278 — **maintainer independently reproduced**: "was able to reproduce the 71% number... Super cool this was just using 1 attempt" (merged by john-b-yang).
- **75.8%** (Sep 1, 2025) with GPT-5 as primary, same production single-agent system; +2% from auto task lists; long-context summarization gains — https://www.warp.dev/blog/swe-bench-verified-update. Official experiments PR #321 "2025-09-01 Warp SWE-bench submission" (merged) — https://github.com/SWE-bench/experiments/pull/321. **#3 ranking** corroborated by Warp itself retroactively ("SWE-bench (#3)" in WarpBench post) and by Sept 2025 press. Independent reproduction of the 75.8% run specifically: **UNVERIFIED** (merger in official repo implies evaluation; no explicit reproducer comment captured this pass).
- 2026 context: Verified leaderboard moved far past 75.8% (GPT-5.5/Codex ~88.7%, Claude Opus 4.5 ~79.2% per aggregators/press) — Warp made **no new submission**.

### 1.4 2026 refresh: ABSENCE CONFIRMED — **VERIFY**
- Scout finding holds. Warp's **last benchmark submission of any kind: Dec 12, 2025** (TB 2.0, 61.2%). No 2026 Terminal-Bench or SWE-bench entries/claims found (leaderboards, blog index, news searches).
- Relative decay documented by third parties: "on the current Terminal-Bench 2.1 leaderboard the top entries are Claude Code, Codex, Terminus 2, and Cursor CLI, with **Warp outside the leading group**" — https://www.developersdigest.tech/blog/warp-2-agentic-terminal. 2026 press compares TB 2.0 scores of Codex (82.7%) vs Claude Code (69.4%) without mentioning Warp — https://hackernoon.com/stop-guessing-ai-coding-tool-to-use-heres-the-honest-breakdown-that-took-me-weeks-to-figure-out.
- Strategic pivot: Warp now argues public benchmarks are poor proxies / possibly contaminated and redirects to **private replay benchmarks** (see 1.6). Critical take: "contamination claim is Lloyd's assessment, and Warp has not published an independent analysis covering the public benchmarks it references" — https://runtimewire.com/article/warp-factory-benchmarks-test-coding-agents-historical-tasks.

### 1.5 Methodology disclosure assessment — **VERIFY**
- Warp is an **above-average discloser** for a vendor: both SWE-bench runs went through the official SWE-bench/experiments repo (logs, trajectories, predictions attached); the TB post details models, planning step, fallback chain, permission settings, and admits selection across runs (65% pass across all runs vs 52% reported average).
- All headline scores are **vendor-run** (self-selected configs, model mixes, retry/fallback chains); independence only via maintainer reproduction (SWE-bench 71% explicitly; TB via leaderboard acceptance).
- Caveat pattern: blog scores are "best configuration of the week," usually tied to a model-launch marketing post (Gemini 3, GPT-5.2) — benchmark as launch vehicle.

### 1.6 "2M+ agents/day, 100M LOC/week" provenance — **VERIFY (as self-reported claims only)**
- Origin: "One month as the Agentic Development Environment" (Jul 29, 2025): "almost 2 million agents per day, growing 200% month-over-month"; "nearly 100 million lines of code per week, growing 55% WoW"; "97%+ acceptance rate of agent-suggested diffs"; 2M+ synced codebases — https://www.warp.dev/blog/agentic-development-environment-two-million-agents. Repeated in Lightspeed plan post (Jul 31, 2025) as "using the #1 coding agent" — https://www.warp.dev/blog/introducing-warp-lightspeed.
- Escalation: First Round interview (Oct 8, 2025): "~3M agents per day, 250M lines of code weekly," revenue "up 19x this year," "$1M ARR every 5-6 days" vs "first $1M took 300+ days" — https://www.firstround.com/ai/warp.
- **No methodology, no audit, no third-party corroboration** of any of these metrics; LOC/week is a vanity metric; "agent run" undefined. April 2026 newsroom adds "nearly one million developers... over half of the Fortune 500" (self-reported) — https://www.warp.dev/newsroom/2026/4/28/warp-open-sources-its-agentic-development-environment.

### 1.7 Custom-model benchmark replaying team runs (event mention) — FOUND — **VERIFY**
- **Warp Event: "Reducing your team's cost-per-PR with custom benchmarks" — Thu Sep 17, 2026, 2-3pm EDT**: "build a benchmark by replaying your team's past agent runs using Factory Benchmarks... score runs... tune model selections / custom model routers" — https://www.warp.dev/events/agent-benchmarks-from-team-coding-tasks.
- Product: **Factory Benchmarks** launched Sep 3, 2026 — https://www.warp.dev/blog/warp-factory-benchmarks (replay from stored traces; vary model/harness/scorers; LLM-as-judge; results feed custom model routers).
- Internal case study **WarpBench** (Zach Lloyd, Sep 2, 2026): 30 replayed internal tasks (S/M/L/XL; Go/React + Rust); models Opus 5, GPT-5 Sol, Gemini 3.7, Grok 4.6, GLM 5.3 Flash; run took 3h46m and **cost $2,130.57**; result: switched default from "auto (genius)" (routed to Opus 5) to Grok 4.6 (high), **cost/PR ~$80 to ~$30 (-63%)** with "no measured or anecdotal drop in quality"; later GPT-5.6 Sol default, projected +25% (variance-flagged) — https://www.warp.dev/factories/benchmarks/warpbench.
- Cross-harness benching (Warp vs Claude Code vs Codex as harness variable) is advertised but **"not yet fully supported"** (RuntimeWire; Warp blog says "(soon) across different harnesses like Warp, Claude Code and Codex"; WarpBench ran Warp Agent only).
- Related event: "Crafting Software Factories" — Zach Lloyd + Peter Steinberger (OpenAI) + Paige Bailey (Google DeepMind), moderated by Andrew Reed (Sequoia) — https://www.warp.dev/events.

---

## 2. ECONOMICS

### 2.1 Current plans and credits — **VERIFY**
Sources: https://www.warp.dev/pricing - https://docs.warp.dev/support-and-community/plans-and-billing/credits/ - https://docs.warp.dev/support-and-community/plans-and-billing/add-on-credits/ - third-party audit https://omidsaffari.com/blog/warp-pricing (Aug 20, 2026).
| Plan | Price | Included | Notes |
|---|---|---|---|
| Free | $0 | **no bundled AI credits** | BYOK now allowed; add-on packs ~20% higher per-credit; limited cloud agents |
| Build | $20/mo ($216/yr, -10%) | **1,500 credits = "$20 of agent usage at API rates"** (par) | BYOK; 20 cloud agents; 40 indexed repos |
| Max | $200/mo ($2,160/yr) | **18,000 credits (12x Build)**; Warp values pool at **$240** | 4 vCPU/8GiB cloud runs; ~17% effective discount for prepay |
| Business | $50/user/mo | 1,500 credits/seat | +$30/user over Build buys SSO, ZDR, admin; 25-seat cap; card-only self-serve |
| Enterprise | custom | pooled | BYOLLM, self-hosted workers, invoicing |
- Add-on (Reload) credits: 400/$10 (base $0.025) - 1,000/$20 (-20%) - 3,000/$50 (-35%) - 6,500/$100 (-40%); roll over, valid 12 months; auto-reload with team spend cap. Plan credits do NOT roll over; add-ons do.

### 2.2 Pricing history — **VERIFY**
- **Oct 30, 2025 repricing** ("Introducing Build"): killed Pro/Turbo/Lightspeed; single $20 Build; BYOK added to Build/Business; overage replaced by Reload credits "about 50% cheaper"; existing plans force-migrated at renewal (Turbo/Lightspeed auto-reload capped at $50/$225) — https://www.warp.dev/blog/warp-new-pricing-flexibility-byok. HN reaction lukewarm: 39 pts / 85 comments — https://news.ycombinator.com/item?id=45772558.
- Later (2026): BYOK extended to **Free**; custom inference endpoint to Business+ — https://docs.warp.dev/support-and-community/plans-and-billing/pricing-faqs/ ("Previously BYOK required a Build, Business, or Enterprise subscription"). Free users can also run inference via **SuperGrok / X Premium** subscriptions.

### 2.3 Warp Factories (Early Access, Aug 2026) — **VERIFY**
- PAYG tier: "Pay-as-you-go usage, **20% markup** [over API rates], no subscription"; paid plans: factory usage inside plan credits, additional usage **at API rates** (markup effectively waived); Enterprise: self-hosted workers + BYOLLM "at no Warp credit cost"; selected EA teams get up to $10,000 usage credit — https://www.warp.dev/pricing - https://www.eweek.com/news/warp-ai-software-factory/ - https://omidsaffari.com/blog/warp-pricing.
- Up-sell surface: "cost per PR" insights, automation rate, config-as-code, triggers from GitHub/Slack/webhooks/schedules.

### 2.4 Three-bucket credit metering (unit-economics core) — **VERIFY**
- Credits metered as **AI/inference + compute (cloud sandbox) + platform** (run lifecycle, integrations, dashboard, APIs, observability; billed **by agent-hour**). All buckets draw one pool (monthly, then add-ons).
- **Platform credits apply to EVERY cloud agent run regardless of which agent (Warp Agent, Claude Code, Codex), inference source (Warp-managed/BYOK/BYOLLM), or compute (Warp-hosted/self-hosted)**. Local runs on Free/Build/Max: no platform credits; local runs on Business/Enterprise with customer-supplied inference: platform credits apply — https://docs.warp.dev/support-and-community/plans-and-billing/platform-credits/.
- Third-party agent CLIs run directly in the terminal (claude, codex, etc. outside Oz): **no Warp credits at all** (same docs).

### 2.5 Unit-economics commentary (analysis)
- Included credits priced at **API-rate par** (Build) or a **~17% prepay discount** (Max) = Warp's inference line is roughly pass-through; margin comes from (a) 20% Factories PAYG markup, (b) platform/compute credits on cloud/Oz runs, (c) Business seat premium ($30/user for SSO/ZDR = high margin), (d) Free-plan add-on surcharge (~20%), (e) breakage (plan credits expire monthly; only add-ons roll over).
- **Central tension**: metered API-rate credits lose to flat subscriptions (Claude Max $100-200, ChatGPT plans) for heavy users — the dominant r/warpdotdev complaint (sec 3.4). Warp's answers: BYOK everywhere (retention valve, zero inference revenue), Factories markup (value-based pricing on orchestration, not tokens), platform-credit billing for ANY harness in the cloud (monetize the control plane, not the model).
- WarpBench's own $80-to-$30/PR result shows the squeeze: at API rates + 20%, cheaper models shrink absolute revenue per PR — hence the pivot to selling **orchestration/measurement** (Factory Benchmarks, Oz, platform hours) rather than tokens.
- Enterprise BYOLLM + self-hosted workers = ZDR/compliance play, monetized via platform agent-hours and contract.

### 2.6 Funding — **VERIFY**
- Total raised **$73M** (CB Insights) / **$73.6M** (PitchBook): Seed $6M (GV, 2020) - Series A $17M (led by Dylan Field, 2022) - **Series B $50M led by Sequoia, Jun 2023** (Andrew Reed board) — https://www.warp.dev/blog/warp-drive-series-b - https://www.cbinsights.com/company/warp-dev/financials - https://pitchbook.com/profiles/company/455184-10 - https://en.wikipedia.org/wiki/Warp_(terminal). Angels: Sam Altman, Marc Benioff, Jeff Weiner, Elad Gil, Tobias Lutke, Neo, BoxGroup.
- **No 2026 round found** — PitchBook 2026 profile still lists Series B as latest deal; no press/filing hits. OpenAI's sponsorship of the OSS repo (inference compute for agent workflows) reads as a non-dilutive substitute for a growth round. Absence-of-evidence caveat noted.

### 2.7 Open-source strategy economics — **VERIFY**
- Apr 28, 2026: client open-sourced at github.com/warpdotdev/Warp — **AGPLv3** (UI crates warpui_core / warpui **MIT**); **OpenAI is founding sponsor**, GPT models (incl. GPT-5.5) power the Oz-run agentic contribution workflow; public GitHub issues become "source of truth"; roadmap public — https://www.warp.dev/blog/warp-is-now-open-source - https://www.warp.dev/newsroom/2026/4/28/warp-open-sources-its-agentic-development-environment - https://thenewstack.io/warp-open-source-client/ - https://itsfoss.com/news/warp-goes-open-source/.
- Stated economics, verbatim: "we are competing with other highly funded, closed-source competitors, and we do **not** have the resources to compete on price or massively subsidize usage — we need to build our business by offering the best possible product to the most excited community." Bottleneck framing: "the biggest bottleneck to development is no longer writing code — it's all the human-in-the-loop activities around the code." Fulfillment of the Apr 2022 Show HN promise — https://news.ycombinator.com/item?id=30921231.
- Results (Lloyd, ~2 months in; Jul 1, 2026): ~63k stars, 5,100+ forks, "hundreds" of contributions/week; community shipping long-tail fixes (i18n, Windows, embedded systems); "from 30 engineers... to hundreds of people who can contribute"; "We sort of want to be Switzerland" (re: sponsors); human review retained at every stage — https://www.forkable.io/p/why-warp-finally-opened-up-its-code.
- Why-open-source reading: (1) headcount-leveraged R&D without a new round; (2) trust/telemetry rehab (sec 3.3); (3) distribution vs closed rivals (Cursor/Copilot/Claude Code); (4) Oz stays proprietary — open client funnels to paid orchestration (FOSS Force flags fork risk if Oz coupling deepens).
- Criticism: Kilo Code argued OSS credentials are undercut by routing the recommended contribution path through proprietary Oz + a single AI provider (OpenAI) — via https://www.forkable.io/p/why-warp-finally-opened-up-its-code. Foundation-donation calls predicted — https://fossforce.com/2026/05/after-years-of-teasing-warp-finally-goes-open-source/.

---

## 3. COMMUNITY

### 3.1 GitHub (github.com/warpdotdev/Warp) — **VERIFY** (live API, Sep 11-12, 2026)
- **64,962 stars** (matches the "65k stars" figure) - 5,528 forks - 5,232 open issues - AGPL-3.0 (repo-level) - pushed hours before compile. Repo existed since 2021 (issues/feedback) but client source landed Apr 28, 2026.
- Issues-as-roadmap in practice: ACP work tracked publicly — issue **#9233** (ACP support: "bring your own harness like Codex... using that harness's subscription with Warp's UI"), discussion **#9619** (OpenRouter support, "on our list... very soon") — https://github.com/warpdotdev/warp/issues/9233 - https://github.com/warpdotdev/warp/discussions/9619.

### 3.2 Hacker News (Algolia data, Sep 2026)
| Thread | Link | Pts/Cmts | Signal |
|---|---|---|---|
| "Warp is now open-source" (Apr 28, 2026) | https://news.ycombinator.com/item?id=47936264 | **373 / 118** | Large but not dominant; debate re Oz coupling + OpenAI sponsorship. Caveat: direct page fetch rendered a stale "2 points, minutes ago" view; Algolia counts authoritative (created 2026-04-28, last activity 2026-09-09). |
| "The Warp Agent CLI" (Aug 4, 2026) | https://news.ycombinator.com/item?id=49171766 | 111 / 63 | Standalone CLI beyond the Warp terminal (Ghostty/iTerm/VS Code) — harness-neutrality play. |
| "Warp sends a terminal session to LLM without user consent" (Aug 19, 2025, Ask HN) | https://news.ycombinator.com/item?id=44953470 | 94 / 31 | **Telemetry/trust**: Active AI prompt-suggestions shipping session output to LLMs without explicit opt-in; "breach of trust" top comment pattern. |
| "Warp.dev Terminal – Overpriced, Buggy, and AI-Sabotaged My Code" (Jul 27, 2025, Ask HN) | https://news.ycombinator.com/item?id=44704043 | 57 / 42 | **Credit burn + overage billing anger** (pre-repricing). |
| "Warp Code: prompt to production" (Sep 3, 2025) | https://news.ycombinator.com/item?id=45116978 | 52 / 60 | Mixed; AI-pivot skepticism. |
| "Warp Terminal changes pricing model" (Oct 31, 2025) | https://news.ycombinator.com/item?id=45772558 | 39 / 85 | Plan-deprecation confusion; credit non-rollover gripes. |
| "no more login required" (Nov 2024) / "Telemetry is now optional" (Dec 2022) | https://news.ycombinator.com/item?id=42247583 / https://news.ycombinator.com/item?id=33910992 | 73/107 - 56/69 | Trust issues recur; both concessions came only after sustained pressure. |
- Agents 3.0 (Nov 2025): no dedicated high-traction HN thread found — **UNVERIFIED** as a distinct major thread; coverage rode on benchmark posts.

### 3.3 r/warpdotdev sentiment (2025-2026) — **VERIFY** (multi-thread patterns)
- "What have you done to Warp Terminal?" (50 up / 39 c): "the market has caught up, and there is almost no reason at all to use Warp over something like Claude Code" — https://www.reddit.com/r/warpdotdev/comments/1r404b6/
- "Loved warp, forced to find alternatives" (25 / 14): "Now I burn my warp credits in 3 days, that's why I switched"; $50/mo stack (llm CLI + kilocode + Claude Code) replaces Warp Turbo — https://www.reddit.com/r/warpdotdev/comments/1ow7foe/
- "i think warp is the ONE" (32 / 22): split — "best tab management, best cli" vs "credits burn quickly"; "Claude code (max subscription) has become my go to" — https://www.reddit.com/r/warpdotdev/comments/1t6f1o1/
- "Those who have switched, what are your thoughts?": "genius mode... hoovers up my credits"; BYOK "burned through $100 each of OpenAI and Anthropic usage in a week"; math vs Claude $200 tier (Warp 18,000 credits) — https://www.reddit.com/r/warpdotdev/comments/1qvk0wm/
- "Warp vs. Claude Code": heavy users route around limits via OpenCode/OpenRouter when "18k credits are low"; staff reply points to discussion #9619 — https://www.reddit.com/r/warpdotdev/comments/1t4htu1/
- "We just launched universal agent support in Warp — run Claude Code, Codex, OpenCode... AMA": launch of rival-harness hosting with Warp's review UI/tabs/notifications; pushback: "went from amazing terminal tool to just another agentic AI client"; WSL/Windows nits (~1s tab switch) — https://www.reddit.com/r/warpdotdev/comments/1slf86f/
- Positive counter-signal: "Warp is actually surprisingly kind of good to use now" (32 / 22): "genuinely superior terminal suited for the current era"; community advice: "acknowledge nearly everyone is going to be using either Codex or Claude Code for the next 1-5+ years... design the terminal to be ideal for such cases" — https://www.reddit.com/r/warpdotdev/comments/1snko27/
- Credit granularity anecdote: "One opus call for a generic edit can be like 25 creds"; "Opus eats credits like crazy when you have a big context" — https://www.reddit.com/r/warpdotdev/comments/1nidjut/
- Slack: no official Warp community Slack found — primary channels are **Discord** (warp.dev/discord) + GitHub — **UNVERIFIED** as listed in brief.

### 3.4 Praise/complaint pattern summary (2026)
- **Praise**: terminal UX (tabs/blocks/diff-review panel), multi-model freedom, universal-agent hosting, OSS goodwill, fast community-driven fixes post-AGPL (Windows/i18n).
- **Complaints**: (1) **credit burn / cost vs Claude Max subscriptions** — dominant theme before and after the Oct 2025 repricing; (2) **agent quality vs Claude Code/Codex** — sharper after Warp stopped publishing benchmarks while rival scores rose (sec 1.4); (3) **Windows/WSL parity** — recurring perf/bug reports; partially community-fixed post-OSS; (4) **telemetry/consent trust** — Active AI session-to-LLM incident (Aug 2025) on top of login-required (until Nov 2024) and opt-out telemetry (2022) history; (5) identity anxiety — "just another agentic AI client."

---

## 4. POSITIONING

### 4.1 "Software factory" framing — **VERIFY**
- Doctrine: Jun 18, 2026 internal memo — engineers as "factory engineers" whose job is "to build the system that builds the product" (via RuntimeWire). Productized as **Warp Factories** (EA, Aug 2026): cloud agent fleets triggered from GitHub/Slack/webhooks/schedules, config-as-code, insights (cost per PR, automation rate), Factory Benchmarks + custom model routers — https://www.warp.dev/pricing - https://www.warp.dev/blog/warp-factory-benchmarks.
- Legitimacy/endorsement: Sequoia-moderated "Crafting Software Factories" livestream with OpenAI (Peter Steinberger) + Google DeepMind (Paige Bailey) — https://www.warp.dev/events. Sequoia's Training Data podcast: "terminal as AI's workbench."

### 4.2 Cross-harness neutrality ("be Switzerland") — **VERIFY**
- Warp ships and hosts rival harnesses: native **Claude Code / Codex / Gemini CLI** support plus "universal agent support" (Claude Code, Codex, OpenCode inside Warp's review UI/tabs/notifications) — https://www.warp.dev/agents/claude-code + r/warpdotdev AMA (sec 3.3). Docs bill platform credits for cloud runs "regardless of which agent runs the task (Warp Agent, Claude Code, or Codex)."
- **ACP (Zed's Agent Client Protocol) explicitly on roadmap**: issue #9233 + staff comments — "exploring ACP support with Warp's agent UI... unlock bringing your own harness like Codex, and using that harness's subscription with Warp's UI." That is the Zed-ACP neutral-host strategy: monetize shell/UI/orchestration while model subscriptions stay with the labs. Factory Benchmarks plans cross-harness scoring (Warp vs Claude Code vs Codex) "(soon)".
- Neutrality boundary: direct CLI use of claude/codex inside the terminal consumes zero Warp credits — "we do not tax what we do not orchestrate."

### 4.3 vs Copilot / Cursor / Devin (analysis)
- **Copilot**: enterprise distribution + GitHub gravity. Warp counters with terminal-native agent fleets (Factories), AGPL openness, multi-model/multi-harness neutrality, self-reported "half the Fortune 500" usage. Explicitly refuses a price-subsidy war.
- **Cursor**: IDE incumbent for agentic coding. Warp's wedge: terminal + cloud orchestration (Oz pitched as "Kubernetes for agents") + evaluation infrastructure (Factory Benchmarks) rather than editor share.
- **Devin**: autonomous-agent SaaS. Warp's factories are analogous but grounded in the existing developer desktop + repo workflow, with per-PR cost telemetry as the enterprise hook.
- **Claude Code / Codex CLI (the real 2026 threat)**: Warp's response is two-sided — host them (neutrality) while competing on harness quality (Warp Agent + Oz). Risks: benchmark retreat since Dec 2025 removed its strongest quality proof at exactly the moment rivals pulled ahead (TB 2.0 leaders at 69-83% vs Warp's 61.2%); Reddit shows defection to Claude Max subscriptions on cost.
- OpenAI sponsorship cuts both ways: funds the OSS motion, but taints neutrality — "Switzerland" with one sponsor (Kilo Code critique).

---

## 5. VERIFICATION ROLL-UP
**Verified**: TB 52% June 2025 + methodology (vendor-run, maintainer-acknowledged incl. hill-climbing); TB late-2025 arc (50.1% "back on top" Nov 12; 59.1% #1 via Agents 3.0 Nov 19; 61.14% "#2" Dec 11; last leaderboard entry 2025-12-12); SWE-bench 71% (independently reproduced by maintainers, PR #278) then 75.8% #3 Sept 2025 (PR #321 merged; explicit reproducer comment not captured); **no 2026 benchmark refresh — absence confirmed** (TB 2.1: Warp outside leading group per third parties); 2M agents/day + 100M LOC/week = self-reported Jul 2025 blog metrics, no methodology; custom-model replay = Factory Benchmarks (Sep 3, 2026) + WarpBench case study (Sep 2) + Sep 17, 2026 event; pricing table (Build 1500/$20 par, Max 18000/$200 vs $240 value), 20% Factories PAYG markup, BYOK now on Free, custom endpoint Business+, BYOLLM Enterprise; 3-bucket credits incl. platform credits on all cloud runs (agent-hour billing); $73M funding, Series B $50M Sequoia Jun 2023, no 2026 round; OSS Apr 28, 2026 AGPL/MIT + OpenAI founding sponsor; repo 64,962 stars / 5,528 forks / 5,232 open issues; HN thread stats (47936264 = 373/118; 49171766 = 111/63; telemetry 44953470 = 94/31; pricing 45772558 = 39/85); Reddit credit-burn/defection-to-Claude-Max pattern; ACP roadmap issue #9233.

**Unverified / caveats**: independent reproduction of the 75.8% SWE-bench run; ALL usage/revenue metrics (agents/day, LOC/week, 97% acceptance, 19x revenue, 1M devs, 63k-stars-at-July-1); WarpBench $80-to-$30 cost/PR (self-reported; "still needs outside proof" per RuntimeWire); public-benchmark contamination claim (no published analysis); dedicated "Agents 3.0" HN thread; Slack community existence; stale-render anomaly on direct fetch of HN 47936264 (Algolia counts used).
