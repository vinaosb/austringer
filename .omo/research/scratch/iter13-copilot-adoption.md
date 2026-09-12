# Iteration 13 — Agent C: GitHub Copilot (Adoption + Benchmarks + Economics + Community + First-Party Models)

Compiled: 2026-09-11. Target: GitHub Copilot (Microsoft/GitHub). Method: web verification against Microsoft IR transcripts, github.blog/changelog, GitHub Docs, and third-party sources. Every claim tagged VERIFIED (primary or ≥2 independent secondary), PARTIAL, or UNVERIFIED.

---

## 1. ADOPTION & ECONOMICS

### 1.1 Adoption timeline (all VERIFIED)

| Date | Metric | Source |
|---|---|---|
| Jul 2025 | ~20M cumulative Copilot users (Nadella, FY25 Q4 earnings call; up from ~15M in Apr 2025) | https://axis-intelligence.com/github-copilot-statistics/ ; https://thebriefscript.com/github-copilot-revenue-2026/ |
| 2026-01-28 (FY26 Q2) | **4.7M paid Copilot subscribers, +75% YoY**; **Copilot Pro+ individual subs +77% QoQ** ("fastest-growing price tier") | Microsoft IR: https://www.microsoft.com/en-us/investor/events/fy-2026/earnings-fy-2026-q2 ; Motley Fool transcript: https://www.fool.com/earnings/call-transcripts/2026/01/28/microsoft-msft-q2-2026-earnings-call-transcript/ ; https://office365itpros.com/2026/01/30/microsoft-fy26-q2-results/ |
| 2026-01-28 | Microsoft switched reporting from "users" to "paid subscribers" — deliberate shift to monetized base | https://axis-intelligence.com/github-copilot-statistics/ |
| 2026-02-25 | Copilot CLI GA for all Copilot subscribers (public preview Sep 2025) | https://github.blog/changelog/2026-02-25-github-copilot-cli-is-now-generally-available/ ; https://www.infoq.com/news/2026/04/github-copilot-cli-ga/ |
| 2026-04-20 | Individual-plan signup pause + tightened limits (see 1.4) | https://github.blog/news-insights/company-news/changes-to-github-copilot-individual-plans/ |
| 2026-04-29 (FY26 Q3) | **~140K organizations use Copilot (Enterprise), enterprise subs "nearly tripled YoY"**; majority of users leverage multiple models; **Copilot CLI usage "nearly doubling month over month"** (≈2× MoM — task claim VERIFIED as "nearly doubled MoM") | Microsoft IR: https://www.microsoft.com/en-us/investor/events/fy-2026/earnings-fy-2026-q3 ; AOL transcript: https://www.aol.com/finance/microsoft-msft-q3-2026-earnings-231859600.html |
| 2026-06-01 | Usage-based billing (AI Credits) live for ALL users | https://github.blog/news-insights/company-news/github-copilot-is-moving-to-usage-based-billing/ ; https://github.com/orgs/community/discussions/192948 |
| 2026-06-17 | Individual signups reopen gradually | https://github.blog/changelog/2026-06-17-copilot-individual-plan-sign-ups-are-reopening/ (linked from https://github.com/orgs/community/discussions/197066) |
| 2026-07-29 (FY26 Q4) | **50M Copilot users**; **Copilot revenue accelerated >60% QoQ** after June usage-based shift; GitHub 225M users; >90% of Fortune 500 choose GitHub; gross margin pressured by Copilot usage, "improved through the quarter" post-June | Globe & Mail transcript: https://www.theglobeandmail.com/investing/markets/stocks/MSFT/pressreleases/3729593/microsoft-msft-q4-2026-earnings-call-transcript/ ; https://office365itpros.com/ (Jul 30, 2026 post); https://www.digitalapplied.com/blog/microsoft-fy26-q4-earnings-copilot-arr |

**90% of Fortune 100**: PARTIAL. Repeated by many secondary aggregators attributed to GitHub/Microsoft (https://axis-intelligence.com/github-copilot-statistics/ ; https://sqmagazine.co.uk/copilot-statistics/). Nearest primary statement found is the FY26 Q4 call's "over 90% of the Fortune 500 choose GitHub" (stronger claim, different denominator). Treat "90% of Fortune 100" as a durable vendor marketing claim, not an earnings-disclosed metric.

Also notable FY26 Q2 color: Siemens "going all in on GitHub" after rollout to 30,000+ developers; Agent HQ positioned as "the organizing layer for all coding agents like Anthropic, OpenAI, Google, Cognition, and xAI"; Copilot SDK ships the CLI runtime (same transcript, fool.com link above).

### 1.2 ARR — analyst estimates only (Microsoft has NEVER disclosed)

- **Microsoft has never disclosed Copilot revenue/ARR** in any earnings release or call: https://www.perspectives.plus/p/microsoft-ai-numbers-good-bad-ugly ("GitHub Copilot ARR has never been disclosed in an earnings [call]").
- **$451M–$848M ARR**: https://thebriefscript.com/github-copilot-revenue-2026/ (June 2026; 4.7M subs × blended ARPU $8–$15/19 depending on assumed enterprise discount depth).
- **$900M–$1.1B ARR**: https://axis-intelligence.com/github-copilot-statistics/ (Q2 FY26 estimate); https://aibusiness.vc/b2b/github-copilot-2b-arr ("Analysts who build revenue estimates from that seat count and the tier mix land at $0.9–1.1B").
- A viral **"$2B ARR" claim did not survive analyst scrutiny** per https://aibusiness.vc/b2b/github-copilot-2b-arr.
- Task's $451M–$1.1B range = union of the two credible estimate families → **VERIFIED as analyst-estimate range, UNVERIFIED as fact**. FY26 Q4's "+60% QoQ revenue acceleration" (Globe & Mail transcript) is the only directionally official revenue signal.

### 1.3 Pricing & AI Credits (2026) — all VERIFIED

Plan prices (https://github.com/features/copilot/plans ; https://docs.github.com/en/copilot/get-started/plans ; https://www.nocode.mba/articles/github-copilot-pricing):
- **Free $0** (2,000 completions/mo; auto-model only; 200-credit-class allowance) / **Pro $10** / **Pro+ $39** / **Max $100** (individual) / **Business $19/user** / **Enterprise $39/user**.

AI Credits mechanics (https://docs.github.com/copilot/concepts/billing/usage-based-billing-for-individuals — fetched in full):
- **1 AI credit = $0.01 USD**, token-metered (input + output + cached tokens, model-specific rates).
- Included monthly allowance = **base credits** (match plan price: $1 = 100 credits) + **flex allotment** ("designed to adapt as the economics of AI evolve"):

| Plan | Price | Base | Flex | Total |
|---|---|---|---|---|
| Pro | $10 | 1,000 | 500 | 1,500 |
| Pro+ | $39 | 3,900 | 3,100 | 7,000 |
| Max | $100 | 10,000 | 10,000 | 20,000 |

- Credits **do not carry over**; reset 00:00 UTC on the 1st of each month. Overage via prepaid budget; additional usage may be capped. **10% discount on model costs when using auto model selection.** Code completions/next-edit suggestions remain unlimited and unbilled on paid plans. Billed features: Chat, CLI, cloud agent, Spaces, Spark, third-party agents. Business/Enterprise follow the same base=$price×100 pattern (community: "$19 = 1,900 credits" — https://www.reddit.com/r/devops/comments/1sx8elv/).
- Changelog framing: "Copilot Pro+: $39/month, including $39 in monthly AI Credits" (https://github.blog/news-insights/company-news/github-copilot-is-moving-to-usage-based-billing/).
- Mid-cycle upgrade pro-rata logic + FAQ: https://github.com/orgs/community/discussions/197089

### 1.4 The April 2026 individual-plan crisis (economics inflection) — VERIFIED

- **2026-04-20**: New signups **paused** for Copilot Pro, Pro+, Student; usage limits tightened; Opus models removed from Pro (Opus 4.7 restricted to Pro+, Opus 4.5/4.6 removed from Pro+ over time); refunds via cancel-through-May-20. https://github.blog/changelog/2026-04-20-changes-to-github-copilot-plans-for-individuals/ ; https://github.blog/news-insights/company-news/changes-to-github-copilot-individual-plans/ ; community megathread https://github.com/orgs/community/discussions/192963
- Context: free trials had been suspended the prior week for abuse (https://github.com/orgs/community/discussions/192573); Apr 22 changelog paused Business self-serve on Free/Team orgs (per https://letsdatascience.com/news/github-pauses-new-copilot-individual-plan-signups-2af29285).
- Stated cause (Joe Binder, VP Copilot product): "Long-running, parallelized sessions now regularly consume far more resources than the original plan structure was built to support." Analysis framing: agentic usage broke flat-rate economics (https://thenextweb.com/news/github-copilot-signup-pause-agentic-ai-usage-limits).
- Signups reopened gradually from **2026-06-17** (https://github.blog/changelog/2026-06-17-copilot-individual-plan-sign-ups-are-reopening/).
- FY26 Q3 call (Apr 29) pre-announced the pivot: "we announced our move to a usage-based pricing model... align pricing to actual usage and cost" (AOL transcript above).

---

## 2. BENCHMARKS

### 2.1 Vendor posture — nuance vs. task premise

Task premise "vendor publishes none for coding agent" needs a 2026 correction: GitHub historically published no official leaderboard entries (Copilot appears on **neither** https://www.swebench.com/ nor https://www.tbench.ai/), **but** on 2026-06-25 GitHub published a harness study — https://github.blog/ai-and-ml/github-copilot/evaluating-performance-and-efficiency-of-the-github-copilot-agentic-harness-across-models-and-tasks/ — claiming Copilot's harness is "on par" with Claude Code/Codex CLI at equal model with **lower token consumption in most configurations** (benchmarks: SWE-bench Verified, SWE-bench Pro, SkillsBench, TerminalBench 2.0, internal Win-Hill; 4 models; 5 runs each; best run reported as pass@1; normalized reasoning effort=medium). Caveats: no absolute per-benchmark score table in leaderboard form; "best scored run" of five inflates results; third-party writeup notes Copilot CLI used 7%/4% MORE tokens than Codex CLI on SWE-bench Verified with GPT-5.4/5.5 (https://alphasignal.ai/news/github-copilot-matches-claude-code-while-burning-fewer-tokens). So: no leaderboard entries = still true; no vendor benchmark publication = no longer true as of June 2026.

### 2.2 SWE-bench Verified: conflicting third-party claims (46.3% vs 56%)

- **46.3%**: tech-insider.org's "independent benchmarks (updated March 2026)" table — "GitHub Copilot (Agent Mode) 46.3% vs Cursor 51.7%": https://tech-insider.org/github-copilot-vs-cursor-2026/
- **56%**: same site's "July 2026 update" ("GitHub Copilot solved 56% of 500 tasks"); also https://timewell.jp/en/columns/ai-coding-tools-complete-benchmark-2026 ("GitHub Copilot Agent 56% — per independent evaluation") and https://tech-insider.org/claude-code-vs-github-copilot-2026/ (which separately says "Codegen clocked it at just 56%... back in April 2025").
- Adjacent outlier: "Copilot Workspace ~44%" (https://agentbrisk.com/blog/ai-coding-agent-benchmarks-2026/); "Claude Code 80.8% vs Copilot ~60–70%" (https://docs.bswen.com/blog/2026-03-20-claude-code-vs-github-copilot-benchmarks/).
- **RELIABILITY FLAG (high)**: Neither 46.3% nor 56% appears on the official SWE-bench Verified leaderboard (https://www.swebench.com/verified.html — no Copilot entry). All figures trace to SEO aggregator sites with no published harness/model snapshot/date/methodology; the two numbers are not reconcilable (unknown model mix, unknown agent mode vs coding agent, unknown date). A 9.7-point swing between "March 2026" and "July 2026" from the *same outlet* is unexplained. **Verdict: both UNVERIFIED; usable only as folklore-range, not as evidence.**

### 2.3 Session-Bench (Aug 2026) — Copilot CLI 10/18 — VERIFIED, with reliability flags

- Leaderboard v0.3, corrected 2026-08-12 (probe 2026-08-04): **Copilot CLI 1.0.77 = 10/18 gates**, format "nested-envelope JSONL (event-sourced)", rank 8 of 10. Pi 18/19, OpenClaw 17/18, Claude Code 2.1.220 = 12/18, Codex 0.146.0 = 12/18, Cursor 7/18. https://jazzyalex.github.io/agent-sessions/bench/ ; https://github.com/jazzyalex/session-bench
- Measures **session-file preservation** (inspectability/portability), NOT coding ability — SWE-bench complements it (per the project's own framing).
- **Reliability flags**: (a) went through **three correction rounds** — Copilot was 11→12/19 after an O3 doc correction, then cut to **10/18** when T2 (format-version declaration) was tightened; (b) "**Copilot's documented-schema pass is disputed (the weakest pass on the board)**" — CLI storage docs and SDK event docs match the measured artifact but "no single official page ties the two surfaces together"; (c) single identical probe prompt, CLI surface only, original fixtures not public ("not fully possible for others to repeat the tests exactly" — https://shoftech.com/question/session-bench-reveals-...).

### 2.4 Terminal-Bench — no public Copilot entries; vendor internal only

- Public TB 2.0/2.1 leaderboards contain **no GitHub Copilot CLI rows** (Codex CLI, Terminus-2, Claude Code, Gemini CLI, Droid, etc. only): https://www.tbench.ai/ ; survey https://lagindicator.com/dev-tools/cli-benchmarks/ ("No official benchmark isolates the CLI from the model"; same-model rows show ~5–10 pt harness spread).
- GitHub's own June 25 harness post ran **TerminalBench 2.0 internally** (89 tasks, 5 runs, best run reported; Copilot "never below a competitor on completion or to the right on cost" — vendor-claims, not independently checkable): github.blog link in 2.1.
- Vendor internal TB claim reused for MAI-Code-1.1-Flash: "+22% improvement on Terminal-Bench 2.1 in GitHub Copilot CLI" (https://github.com/microsoft/MAI-Code).
- **Verdict: Copilot's Terminal-Bench performance is UNVERIFIED by any public third party.**

### 2.5 Aider leaderboards — model-centric, but one Copilot-CLI agent entry

- Aider's polyglot leaderboard (225 Exercism exercises) ranks **models via the Aider harness**; no Copilot-product entry: https://aider.chat/docs/leaderboards/
- Exception (June 2026): issue #5321 proposes adding **"Squad" (multi-agent orchestration running inside GitHub Copilot CLI, `copilot --yolo -p`, Claude Opus 4.8)** at **98.7% pass rate** (148/148 completed languages at 100% attempt-2); referenced PR #5332 "Add Squad (Copilot CLI + Claude Opus 4.8) to polyglot leaderboard: 98.7% pass rate": https://github.com/aider-ai/aider/issues/5321 — flag: agentic methodology differs from single-call Aider submissions (maintainer asked which category it belongs in); treat as a contested/not-merged-by-default data point.

### 2.6 Other third-party signals

- **ts-bench** (SWE-Lancer/Exercism TS, tiny N): Copilot CLI + claude-sonnet-4.6 **4/5**; Copilot CLI + claude-opus-4.6 **2/5** (same sonnet model: Claude Code 3/5-row comparisons vary): https://laiso.github.io/ts-bench/
- **Production-scale characterization (arXiv 2608.00101, 2026-07-30)**: sampled Copilot coding-agent traces, one week of June 2026 — **3.2M users, 13M sessions, 761M LLM calls, 95T tokens**; 87% of calls agent-initiated; ~1:1 LLM:tool ratio; tool-failure retry loops turn 9% of turns into ~4× compute; model switches hit 6.4% of sessions (mostly reactive to errors/rate-limits) and destroy KV cache (8% hit rate after switch): https://arxiv.org/html/2608.00101 — best available independent economics/reliability evidence.
- **Reliability incident**: GitHub availability report Aug 2026 — "GitHub Copilot (the Kimi K3 model); Customer impact: 63.3% of Kimi K3 requests failed during this impact window": https://github.blog/news-insights/company-news/github-availability-report-august-2026/ — third-party-model instability lands on Copilot's reliability.

---

## 3. COMMUNITY SENTIMENT (2026)

### 3.1 Billing backlash (dominant complaint post-June 1) — VERIFIED pattern
- r/technology: "GitHub just switched Copilot to metered billing, and developers are watching months of credits vanish in a single day" (https://www.reddit.com/r/technology/comments/1twmt4e/); "Angry devs vow to flee GitHub Copilot as metered billing takes hold" (https://www.reddit.com/r/technology/comments/1tur88b/).
- r/github: "new credit-based pricing is highway robbery... 10-22x cost increase disguised as 'flexibility'" (https://www.reddit.com/r/github/comments/1ttcpw0/).
- r/GithubCopilot: "AI Credit billing is speedrunning a trust crisis — no per-request breakdown. No clear token [accounting]" (https://www.reddit.com/r/GithubCopilot/comments/1txw7h2/); "billing/AI credits are unacceptable" (https://www.reddit.com/r/GithubCopilot/comments/1tx9jy0/); counterpoint thread crediting visibility of true inference cost (https://www.reddit.com/r/GithubCopilot/comments/1uf4fef/); "silent downgrade" / paying customers deactivated (https://www.reddit.com/r/github/comments/1u84m1z/).
- Press: Visual Studio Magazine "Copilot Billing Shock" (https://visualstudiomagazine.com/articles/2026/06/04/copilot-billing-shock-hits-developers.aspx); Business Insider — GitHub's own internal estimator showed some users paying "hundreds more" (https://www.businessinsider.com/github-copilot-token-uage-pricing-change-reaction-2026-6).

### 3.2 Model-routing opacity — VERIFIED complaint cluster
- "No receipts" complaint (per-request model/token breakdown) — threads in 3.1; the docs only expose aggregate dashboards (https://docs.github.com/copilot/concepts/billing/usage-based-billing-for-individuals).
- Auto model selection "balanc[es] task intent and model health" — i.e., silently reroutes on rate limits (vendor post 2.1; arXiv 2608.00101 found 6.4% of sessions switched models, mostly reactive; switches also cost users via cache-cold re-billing).
- Compounding: Kimi K3 63.3% failure incident (2.6) — users cannot pre-tell which model serves them.

### 3.3 Agent quality vs Claude Code — mixed-to-negative on agentic tasks
- "Claude Code beats GitHub Copilot on SWE-bench (80.8% vs ~60-70%), but benchmarks don't code" (https://docs.bswen.com/blog/2026-03-20-claude-code-vs-github-copilot-benchmarks/).
- Agent HQ guide: Claude "strong at multi-file refactoring, architectural reasoning"; Copilot's edge is integration, not raw agent quality (https://www.heyuan110.com/posts/ai/2026-03-13-github-agent-hq-vscode-guide/).
- Sandboxing gap: copilot-cli issue #892 — "Coming from Claude Code, this is a pretty big gap... features like sandboxing are only available in VS Code" (https://github.com/github/copilot-cli/issues/892); community built navikt/cplt (Seatbelt wrapper) to compensate — HN: https://news.ycombinator.com/item?id=47709871.
- HN grumble: Copilot CLI autopilot overrode git config to set "GitHub Copilot" as commit author; permission system needed to stop it (https://news.ycombinator.com/item?id=47571455).
- Praise side: CLI GA thread — "Copilot CLI PM here: I'm super proud of our team's velocity... industry-leading agentic harness" + substantial engagement (https://news.ycombinator.com/item?id=47155959); Reddit users note MAI-Code's ~60% token savings under usage billing (https://www.reddit.com/r/GithubCopilot/comments/1twbh9v/).

### 3.4 Enterprise controls — genuine praise — VERIFIED
- CLI GA ships org policies (admin model gating), network-access management, proxy, OAuth device flow + `GITHUB_ASKPASS` for CI, `preToolUse` policy hooks (https://github.blog/changelog/2026-02-25-github-copilot-cli-is-now-generally-available/).
- Org-level CLI usage metrics (DAU + token consumption per terminal session) (https://www.infoq.com/news/2026/04/github-copilot-cli-ga/).
- Agent HQ governance: access controls, audit logs, usage-metrics dashboard, policy enforcement, Code Quality scanning of agent output (https://www.heyuan110.com/posts/ai/2026-03-13-github-agent-hq-vscode-guide/ ; https://aigeneers.eu/article/github-agent-hq-and-the-shift-to-autonomous-ai-engineers).
- Anecdote on internal adoption: "GitHub's legal team has the highest percentage of [CLI] adoption at 93%" (LinkedIn post quoting an internal/earnings-adjacent claim — https://www.linkedin.com/posts/davehgreen_the-hype-about-github-copilot-cli-is-real-activity-7455407740694200320-IQQE — UNVERIFIED anecdote).

### 3.5 HN / Agent HQ / sandboxes / CLI GA timeline
- Agent HQ: announced GitHub Universe 2025 (Oct 2025); **public preview Feb 2026**; Claude+Codex gated to Pro+/Enterprise (friction complaint: "prices out many individual developers"); cloud agents run in ephemeral GitHub-Actions sandboxes billed via AI Credits + Actions minutes (https://www.heyuan110.com/posts/ai/2026-03-13-github-agent-hq-vscode-guide/ ; https://aigeneers.eu/article/github-agent-hq-and-the-shift-to-autonomous-ai-engineers ; earnings mention in FY26 Q2 transcript).
- Cloud + local sandboxes for Copilot public preview 2026-06-02: https://github.blog/changelog/2026-06-02-cloud-and-local-sandboxes-for-github-copilot-now-in-public-preview/ (via issue #892); local `/sandbox enable` experimental; early breakage reports (copilot-cli issue #3653, via #892).
- No dedicated HN Agent HQ thread retrieved in this pass — UNVERIFIED beyond secondary coverage; HN CLI GA + sandbox threads confirmed (3.3).

---

## 4. FIRST-PARTY MODEL ARC

### 4.1 MAI-Code-1-Flash — VERIFIED
- **Introduced 2026-06-02 at Build** by Microsoft AI (Suleyman org): "trained from the ground up on clean, traceable and enterprise-grade data, without distillation from third-party models"; trained directly against the GitHub Copilot production harness. https://microsoft.ai/news/introducingmai-code-1-flash/
- Specs: sparse MoE, **137B total / ~5B active params, 256K context**; GitHub-listed price **$0.75 in / $0.075 cached / $4.50 out per 1M tokens**. https://decodethefuture.org/en/microsoft-mai-code-1-flash-copilot/
- Vendor benchmarks (harness-run, not independent): beats Claude Haiku 4.5 incl. **51.2% vs 35.2% SWE-Bench Pro**; "up to 60% fewer tokens" on SWE-bench Verified. (microsoft.ai link above; third-party read: "roughly 51% SWE-Bench Pro... same tier as GPT-5.3, clearly below frontier" — https://decodethefuture.org/en/microsoft-mai-code-1-flash-copilot/)
- **GA for Copilot Business/Enterprise 2026-06-26** (individual rollout started Jun 2): https://github.blog/changelog/2026-06-26-mai-code-1-flash-for-copilot-business-and-copilot-enterprise/
- **Hosting (exact task phrase VERIFIED)**: "MAI-Code-1-Flash and MAI-Code-1.1-Flash are first-party Microsoft models hosted on Azure in GitHub's tenant... served on Microsoft Azure AI Foundry within GitHub's tenant." https://docs.github.com/en/copilot/reference/ai-models/model-hosting
- **Deprecated 2026-09-10** — ~14 weeks after introduction: https://github.blog/changelog/2026-08-11-upcoming-deprecation-of-mai-code-1-flash/

### 4.2 MAI-Code-1.1-Flash — VERIFIED
- **2026-08-11**, rolling out in Copilot: adds **native vision**; improvements in coding quality, instruction following, tool use, performance. https://github.blog/changelog/2026-08-11-mai-code-1-1-flash-available-in-github-copilot/
- Vendor deltas vs 1.0 (https://github.com/microsoft/MAI-Code): **~quarter the cost**; image inputs; **+22% on Terminal-Bench 2.1 in Copilot CLI**; +15% on .NET tasks; **4% more code survives through commit**; 25% faster output streaming; 25% fewer tokens.
- Model card (https://microsoft.ai/pdf/MAI-Code-1.1-Flash-Model-Card.PDF): trained with GitHub Copilot production harness; pretraining cutoff Dec 2025; evaluated in same VS Code-based production harness; **Copilot CLI support "planned for a later rollout"** at card time.
- Tier availability: Free/Student via auto-select only; Pro/Pro+/Max manual; **Business/Enterprise admin policy OFF by default** (ops burden flagged by migration guides: https://flowith.io/blog/mai-code-1-flash-deprecation-migration/ ; https://byteiota.com/mai-code-1-1-flash-is-in-github-copilot-act-before-september-10/).
- Pricing caveat: per-token rate for 1.1 not yet published at launch per Neomanex ("hold budget forecasts until Microsoft publishes a per-token rate") — https://neomanex.com/news/mai-code-1-1-flash-launch.

### 4.3 Raptor mini retirement + Sept 1 deprecation wave — VERIFIED
- **2026-09-01**: Raptor Mini deprecated; suggested replacement **MAI-Code-1.1-Flash** — i.e., the first-party model is now the designated successor to the legacy small model (Raptor mini = GPT-4.1-mini-based small tier). Same wave killed Gemini 3.1 Pro, Claude Opus 4.5/4.6, Claude Sonnet 4.5/4.6 (Sonnet 4.6 retained for individual annual plans). https://github.blog/changelog/2026-08-31-selected-github-copilot-models-deprecated/ ; https://byteiota.com/github-copilot-model-deprecations-sept-2026/
- Migration pain: hardcoded model IDs in Actions/extensions broke on Sep 1; MAI-Code-1-Flash followed Sep 10; Enterprise admins must enable 1.1 policy or teams lose MAI access entirely (byteiota + https://cloudninjas.ca/ai/copilot-model-deprecation-forces-migration-to-mai-code-1-1-flash-for-engineers-and-admins/).

### 4.4 Reception & quality reports
- Positive: framed as strategic decoupling from OpenAI + margin play ("lowers the cost of every Copilot completion, removes a single point of dependence"); "good enough... cheap, not frontier — and Microsoft is not pretending otherwise" (https://decodethefuture.org/en/microsoft-mai-code-1-flash-copilot/ ; https://blog.getbind.co/mai-code-1-flash-vs-gpt-5-4-mini-microsofts-first-coding-model-goes-head-to-head/ — "built entirely without OpenAI's involvement. That distinction matters strategically").
- Skeptical: "Benchmarks are Microsoft's own; we haven't independently tested the model" (Neomanex); positioning "a routing decision, not a frontier decision" (decodethefuture); byteiota on 1.1: "The reliability story is still being written."
- Community cost angle: r/GithubCopilot thread welcoming MAI-Code's ~60% token reduction under usage billing (https://www.reddit.com/r/GithubCopilot/comments/1twbh9v/).
- Known-issues channel exists: microsoft/MAI-Code GitHub repo for feedback/issues (https://github.com/microsoft/MAI-Code); an opencode-integration complaint notes MAI-Code is "maybe the ONLY model in Copilot that can be considered first-party" (https://github.com/anomalyco/opencode/issues/34222).
- Saturation signal: dev.to repost farms comparing "MAI-Code vs Claude Code" with fabricated-looking numbers exist — treat any dev.to benchmark tables as UNVERIFIED noise (example: https://dev.to/leojulieta/why-microsoft-mai-code-beats-claude-code-copilot-on-azure-4apf).

---

## 5. VERIFICATION LEDGER (task-claim → status)

| Task claim | Status |
|---|---|
| 20M cumulative users Jul 2025 | VERIFIED (secondary citing FY25 Q4 call) |
| 4.7M paid subs FY26 Q2, 2026-01-28, +75% YoY | VERIFIED (Microsoft IR + 2 transcripts) |
| Pro+ +77% QoQ | VERIFIED (same call) |
| ~140K orgs FY26 Q3 | VERIFIED ("nearly 140,000 organizations... in Enterprise") |
| 90% Fortune 100 | PARTIAL — vendor marketing; primary statement is ">90% of Fortune 500" (FY26 Q4) |
| CLI usage ~2× MoM | VERIFIED ("nearly doubling month over month", FY26 Q3 call) |
| ARR $451M–$1.1B | VERIFIED as analyst-estimate range ($451–848M + $0.9–1.1B families); UNVERIFIED as fact; MSFT never disclosed |
| Pricing Free/$10/$39/$100/$19/$39 | VERIFIED |
| AI Credits $0.01 token-metered; base+flex allotments | VERIFIED (docs table) |
| Apr 2026 signup pause | VERIFIED (Apr 20; reopened Jun 17) |
| Vendor publishes no coding-agent benchmarks | OUTDATED — no leaderboard entries, but June 2026 harness study exists |
| SWE-bench 46.3% vs 56% | UNVERIFIED — conflicting SEO-aggregator claims, absent from official leaderboard |
| Session-Bench Aug 2026 Copilot CLI 10/18 | VERIFIED (v0.3, 2026-08-12; disputed-schema caveat, 3 correction rounds) |
| Terminal-Bench entries | NONE public — UNVERIFIED; vendor-internal only |
| Aider leaderboards | No official Copilot entry; contested Squad-via-CLI 98.7% submission (issue #5321/PR #5332) |
| "First-party Microsoft models hosted on Azure in GitHub's tenant" | VERIFIED verbatim (GitHub Docs model-hosting) |
| MAI-Code-1-Flash → 1.1-Flash arc | VERIFIED (Jun 2 intro → Jun 26 BE GA → Aug 11 1.1 → Sep 10 1.0 deprecation) |
| Raptor mini retirement | VERIFIED (Sep 1, 2026; replaced by MAI-Code-1.1-Flash) |
