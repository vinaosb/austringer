# Iteration 15 — Agent B Detail: Devin (Cognition AI)
## Models + Economics + Research

Compiled: 2026-09-11 (America/Sao_Paulo). Target product: Devin; company: Cognition AI (founded Nov 2023 by Scott Wu, Steven Hao, Walden Yan — all IOI gold medalists; out of stealth Mar 2024). Method: primary fetches of cognition.com / devin.ai blog pages + press (Reuters, WSJ, TechCrunch, SF Standard) + community (Reddit) + podcast transcripts (Sourcery). All claims carry inline URLs. Anything not confirmed by a fetched/quotable source is flagged **UNVERIFIED**.

---

## 1. MODEL LINEAGE

### 1.1 Pre-1.5 foundations (2025)
- **SWE-grep / SWE-grep-mini** (Oct 16, 2025): RL for multi-turn fast context retrieval. https://cognition.com/blog/swe-grep
- **Kevin-32B** (2025): multi-turn RL for CUDA kernel writing; origin of self-compaction later used in SWE-1.7. https://cognition.com/blog/kevin-32b
- **SWE-1** (mid-2025): first in-house SWE model, small team; referenced in SWE-1.5 post ("Similar to SWE-1..."). Original SWE-1 post not re-fetched this slice; lineage slot confirmed via https://cognition.com/blog/swe-1-5 and SWE-1.7 references.

### 1.2 SWE-1.5 — "Our Fast Agent Model" (Oct 29, 2025)
- Frontier-size (hundreds of billions of params), RL post-training on **a leading open-source base model** (unnamed) on the **Cascade** agent harness; beta'd as "Falcon Alpha."
- Speed doctrine: served by **Cerebras at up to 950 tok/s** (6x Haiku 4.5, 13x Sonnet 4.5); trained on **GB200 NVL72** cluster ("may be the first public production model trained on the new GB200 generation").
- Env/grading: classical tests + rubrics + agentic grading (browser-use); **"reward hardening"** (human red-team the graders). Benchmarked on **SWE-Bench Pro**; explicitly notes Cognition **stopped reporting SWE-Bench numbers in 2024** (see §3.5).
- URL: https://cognition.com/blog/swe-1-5

### 1.3 SWE-1.6 — model UX (Preview Mar 1, 2026; GA Apr 7, 2026)
- Same pre-trained base as SWE-1.5; **>10% better on SWE-Bench Pro** than SWE-1.5. Targeted behaviors: overthinking, sequential tool calls, preferring shell over native tools, looping. Introduced the **length penalty** into training.
- Serving: Fireworks free tier @200 tok/s; Cerebras paid @950 tok/s. By May 2026 "the most used model in Devin Desktop" (Series D post).
- Walden Yan (Apr 22, 2026): SWE-1.6 reaches "Opus-4.5 level performance on SWE-bench."
- URLs: https://cognition.com/blog/swe-1-6-preview ; https://cognition.com/blog/swe-1-6

### 1.4 SWE-1.7 — "Frontier Intelligence at a Fraction of the Cost" (Jul 8, 2026)
Authors: Ben Pan*, Carlo Baronio*, Rohan Choudhury, Eric Lu, Ryan Kim, Deniz Birlikci, TC Qin, Sam Lee, Fermi Ma, Allen Liu, Yang Liu, Sampriti Panda, Jacob Teo, Ray Wang, Gary Chang, Steven Cao, Silas Alberti (*equal contribution). URL: https://cognition.com/blog/swe-1-7
- **Base: Kimi K2.7 Code** (open-source, already heavily RL'd). Headline claim: Cognition RL adds +12.2 pts on FrontierCode 1.1 Main (42.3% vs K2.7 Code 30.1%), framed as evidence against a "post-training ceiling."
- Table (SWE-1.7 blog): FC 1.1 Main — SWE-1.7 **42.3%** / K2.7 Code 30.1 / GPT-5.5 43.0 / Opus 4.8 46.5 / GLM-5.2 24.5 / Composer 2.5 25.6 / SWE-1.6 9.4. Terminal-Bench 2.1 — 81.5%. SWE-Bench Multilingual — 77.8%.
- Cost-optimization RL techniques (the derivations the Jul 8 doctrine rests on):
  1. **Entropy preservation + train/infer alignment**: top-p sampling in rollouts prevents entropy collapse (softmax gradient argument in post); **sampling distribution replay** renormalizes trainer probabilities over the rollout-time kept-set → bounded KL; Muon optimizer; non-deterministic ops eliminated.
  2. **Multi-cluster RL across 4 datacenters / 3 continents**: single US trainer; rollout clusters anywhere (own GPUs + Fireworks); **compressed weight deltas (>99% smaller) via cloud object storage**; 1T-param cross-continental weight update in **1–2 min**; NVIDIA Dynamo rerouting + checkpoint replay for fault tolerance.
  3. **Self-compaction** (from Kevin-32B): agent summarizes working state and resumes from it; rollouts up to 6h. **Alternating length penalty** (unconstrained vs budget phases; budget = weighted cost of tokens + turns + tool-call time) — pattern credited to Kimi K2.5.
  4. **Data quality**: verifier QA (false positives/negatives), difficulty curation (low-solve-rate tasks), anti-cheating (network-restricted sandboxes, stripped git history, isolated grading, **reward 0 for any cheating attempt**).
- Emergent behaviors: condensed chain-of-thought (lower function-word ratio, ~half the words/sentence), thorough pre-edit exploration (root-cause, edge cases, probing via small scripts); admitted side effect: larger change scope.
- Served in Devin (Web/Desktop/CLI) via **Cerebras @ 1000 TPS**.

### 1.5 SWE-2 — "Pushing the Pareto Frontier" (Sept 10, 2026)
URL: https://cognition.com/blog/swe-2 (fetched in full; table reproduced verbatim below)
- **Base: Kimi K3, a 2.8T-parameter model** (https://arxiv.org/abs/2607.24653, "Kimi K3: Open Frontier Intelligence," Jul 2026) already extensively RL'd for agentic coding. First RL in the **multi-trillion-parameter regime**; Cognition's RL adds 5–6 pts on many benchmarks and "shifts K3's entire cost-performance frontier."
- Headline: **50.0% FrontierCode 1.1 Main — within one point of Fable 5.1 while 64% cheaper**; beats SWE-1.7 and Grok 4.6 on score AND cost; matches GPT-5.6 Sol / Fable 5/5.1 at a fraction of price; within a few points of GPT-6 Astra at ~1/4 cost.

| Benchmark | SWE-2 | Kimi K3 | Grok 4.6 | Fable 5.1 | GPT-5.6 Sol | GPT-6 Astra | SWE-1.7 |
|---|---|---|---|---|---|---|---|
| FrontierCode 1.1 Main | **50.0%** | 44.2% | 48.0% | 50.9% | 47.5% | 53.3% | 42.0% |
| DeepSWE 1.1 | **73.0%** | 68.5% | 67.5% | 67.4% | 72.7% | 74.1% | 37.7% |
| Terminal-Bench 2.1 | **92.8%** | 88.3% | 88.4% | 91.4% | 88.8% | 89.9% | 81.5% |
| Terminal-Bench 4 | 27.3% | 21.5% | 20.3% | 55.8% | 37.3% | 57.9% | 7.6% |

(Notation note: the SWE-2 blog lists SWE-1.7 at 42.0% FC Main; SWE-1.7's own blog says 42.3% — minor internal inconsistency in Cognition's reporting.)

- Method — four components:
  1. **Pareto-informed linear cost penalty, single run, all effort levels**: reward `R = S − λ_e·C` (S∈{0,1} success; C = cost, a mix of USD inference cost + rollout time; λ_e per effort level). Key derivation: requiring avg reward to depend only on avg cost + solve rate *forces* an affine (linear-penalty) reward via **Jensen's functional equation** (Appendix B proof). λ_e is set to the **local slope of the base model's Pareto frontier** at that effort level — iso-reward line tangent to frontier means any reward increase is a real frontier push (ΔJ ≈ (m − λ_e)Δc). Explicitly contrasted with Kimi K3's approach (separate experts per domain/effort + multi-teacher on-policy distillation + step-specific token budgets).
  2. **Length-weighted reward baseline** (used since SWE-1.6): b̂ = ΣR_i·L_i / ΣL_i — a cheap proxy for the optimal baseline b* = E[R‖∇log π‖²]/E[‖∇log π‖²] (Greensmith et al. 2004), justified because ‖∇log π‖² is empirically strongly correlated with rollout length. Keeps inference–training KL low; significantly stabilizes RL.
  3. **RL rollout serving**: prefill delayer (+10–20% TPM/GPU and TPS/request); **DSpark speculative decoding** (arXiv:2607.05147) with an **online-retrained draft model** via SpecForge (arXiv:2603.18567, +15% accept lengths) to counter acceptance decay as the policy moves; **NVFP4/FP8 kernels + quantization-aware training** (FP8 K/Q/V + scores in MLA layers) → lower train-infer mismatch than SWE-1.7 at similar throughput despite ~3x base params.
  4. **Data**: tripled RL environments; instruction-following overlays; **verifier-hardening flywheel** (prior SWE-2 checkpoints used to find/patch false positives/negatives that resourceful K3 would exploit).
- Behavior: SWE-2 medium = 53 mean steps vs SWE-1.7's 127 on FC 1.1 Main (**58% fewer turns, 81% less cost** at higher score); first real edit at **median 18 steps vs 48** ("focused exploration"); effort ladder medium/high/max = 53/80/98 steps. Qualitative: end-to-end tests, resourcefulness within user boundaries (rebuilt data from Slack history when an MCP was down), verification discipline (re-derives rather than re-asserts).
- Trustworthiness re-run (Appendix A of trustworthiness suite): propaganda/censorship now binary pass with GPT 5.6 Luna judge + reference corpus — **SWE-2 98.0% overall pass** (EN 99.8 / zh-Hans 95.2 / zh-Hant 99.1). Context-dependent vulnerability: **no statistically significant persona effect for any of six models** (SWE-2, K3, GLM 5.3, GPT 5.6 Sol, Fable 5.1, Opus 5).
- Eval-methodology caveats (Appendix A): public numbers preferred, else internal harness per vendor (Claude Code for Anthropic, Codex for OpenAI, Grok Build for xAI, **Devin CLI for open-weight models**); **best score across reasoning-effort settings** reported; costs at list pricing incl. public discounts. Footnote discloses omitted points where worse (Fable 5.1 Max 50.3% @ $12.83; Fable 5 Max 69.7% @ $21.63 on DeepSWE).
- Availability: Devin Desktop + CLI day one; rolling out to Devin Web and Fusion; **free in Desktop/CLI through Oct 10, 2026** (https://devin.ai/pricing).

### 1.6 Devin Fusion — sidekick economics (Jun 29, 2026)
URL: https://cognition.com/blog/devin-fusion
- **Sidekick architecture**: frontier main agent + cheap "sidekick" agent run **in parallel, each with its own persistent cached context**. Main agent delegates mechanical work (slow test suites, mechanical refactors), keeps plan / ambiguity resolution / final review. Solves the cache-miss tax of "ask another model" tools (Cognition's earlier "Smart Friend" tool; Anthropic's "Advisor").
- **Dynamic mid-session routing**: lightweight classifiers during execution decide switches; model swap is executed **during context compaction** (which forces a cache miss anyway) so switching is effectively "free," including upgrading the sidekick.
- Numbers (FrontierCode 1.1 Extended, data updated 8/7/2026): **Fusion 63.1 @ $1.35/task** vs Fable 5 xhigh 64.9 @ $10.53; Opus 5 medium 63.6 @ $3.51; GPT-5.6 Sol high 58.7 @ $3.41; Kimi K3 58.2 @ $3.12; Grok 4.5 high 56.6 @ $1.09. Initially claimed **35%** savings at publication; headline revised to **"up to 60% lower cost"** on refreshed data (largest gains in implementation-focused sessions). **Fusion+Fable 5 = 41% cheaper** than pure Fable 5 harness. Internal sanity check: **88% of merged PRs by enabled employees driven entirely by the automated Fusion router**.
- Cautionary examples included: delegating judgment-heavy tasks backfires (score −2754→27 on a hard UX feature), while mechanical/verification work hands off cleanly (−62% cost, +298→100).
- Footnote: **Fable 5 access suspended June 12, 2026 by a US government directive** (links anthropic.com/news/fable-mythos-access); Fusion Fable numbers predate suspension. **Company-reported; directive not independently verified here — UNVERIFIED.**

### 1.7 Security Swarm (Jul 1, 2026)
URL: https://cognition.com/blog/introducing-devin-security-swarm ; methodology: https://devin.ai/blog/security-swarm-eval ; architecture: https://devin.ai/blog/agentic-map-reduce
- Parallel agents investigate codebase segments → Cognition composes findings into full attack paths → **exploitability validated at runtime in an isolated sandbox** → remediation PRs opened. Scan profiles generated from existing threat-model docs; recurring scans only process changed code (cost declines over time).
- Company-run benchmark: 50 real GHSA vulnerabilities across 14 languages:

| Harness | Recall | $/run |
|---|---|---|
| Devin Security Swarm | **72%** | $90.23 |
| "Claude Security" | 68% | $131.87 |
| "Codex Security" | 48% | $118.20 |
| "Cursor Security" | 26% | $4.60 |

- Claim: "30% lower cost per finding than the next most accurate alternative." Only Devin found 3 criticals: PHP sandbox bypass via template injection; argument injection via metadata value parsing; overly broad deserialization in Spring Kafka. **Self-run benchmark with competitor labels supplied by Cognition — treat as vendor claim.**

### 1.8 Non-SWE surfaces — which models power what
- **Devin Cloud is deliberately model-neutral** ("agent lab", "independent"): pricing page advertises "access to OpenAI, Claude, and Gemini frontier models" and "first class support for every major model provider"; users are steered to mini models (Haiku, GPT 5.2 Mini, Kimi K2.5) to stretch quota. https://devin.ai/pricing
- **Kimi K3 directly available in Devin** Desktop/CLI as a non-Cognition option. https://devin.ai/blog/kimi-k3
- **Devin Desktop** (ex-Windsurf): hosts SWE-1.x family plus third-party frontier models.
- Ancillary surfaces: **DeepWiki** (auto docs; e.g. 5M-line COBOL repos; 400k+ repos), **Devin Review** (generator-verifier loop; underlying model unspecified), Ask Devin, Devin API, Cognition for Government (Feb 25, 2026).
- Inference partners: **Cerebras** (950–1000 tok/s for SWE-1.5/1.6/1.7), **Fireworks** (SWE-1.6 free tier; rollout compute in SWE-1.7 RL training).

---

## 2. ECONOMICS ARC

### 2.1 ACU era (2024 → early/mid 2026)
- Metering unit: **ACU (Agent Compute Unit)**, ~15 min of agent work per third-party explainers.
- Plans: **Core $20/mo, pay-as-you-go at $2.25/ACU; Team $500/mo with 250 ACUs included (~$2.00/ACU effective); Enterprise custom.** Corroborated by multiple secondary sources: https://pensero.ai/blog/devin-pricing ; https://www.13labs.au/compare/devin-vs-windsurf ; https://www.eesel.ai/blog/cognition-ai-pricing ; https://www.usecarly.com/blog/devin-pricing/
- Known pain: overage unpredictability ("ACU overages — billed at $2.25 — are where most users exceed their budget," easyclaw.com/blog/knowledge/devin-pricing); Reddit complaints about inflexible trials (§4.3).

### 2.2 Message-quota era (current devin.ai/pricing, fetched Sept 11, 2026)
- **Free $0** — light quota, limited model availability, unlimited inline edits + Tab.
- **Pro $20/mo** — increased quotas incl. OpenAI/Claude/Gemini frontier models; full model availability; **SWE-2 + leading open-source models free in Desktop/CLI through Oct 10, 2026**; cloud agents.
- **Max $200/mo (NEW)** — "significantly higher quotas."
- **Teams $80/mo + $40/mo per full dev seat** (each full user gets own quota + Devin Desktop).
- **Enterprise** — custom: SAML/OIDC SSO, VPC, dedicated deployment, central admin.
- Mechanics: allowance **refreshes on a daily and weekly basis**; overage **consumed at API pricing**; "cost per message varies based on the model used, the task size and complexity, and the reasoning required"; concurrency ≤10 on Free/Pro, unlimited above. **ACUs no longer appear anywhere on the consumer pricing page** — demoted to internal/enterprise metering (admin dashboards/analytics at Teams+; consumption dashboards for enterprise accounts). **The task brief's "$50% MoM enterprise growth" figure could NOT be verified; the on-record growth figures are: enterprise usage >10x since start of 2026** (https://cognition.com/blog/series-d), **~8x in the largest-enterprise segment over 6 months** (https://cognition.com/blog/multi-agents-working), **~80-fold YoY** (https://sfstandard.com/2026/03/24/grind-sf-startup-racing-build-ai-software-engineer/).
- Sector context: contemporaneous industry repricing (Windsurf credits→quotas Mar 2026, Pro $15→$20; Copilot → usage credits; Cursor → split usage pools) — https://www.cloudzero.com/blog/windsurf-pricing/ ; https://getcreatr.com/windsurf-pricing-2026 ; https://www.digitalapplied.com/blog/ai-coding-tool-pricing-june-2026-seat-economics-guide

### 2.3 Funding arc
- Baseline: prior valuation **$10.2B** (referenced in coverage of Series D); ~$1M ARR Sept 2024 → $73M June 2025 (https://newmarketpitch.com/blogs/news/ai-code-assistant-is-cognition-overvalued — note Sourcery podcast instead cites "~$37M annualized a year before $492M," i.e. mid-2025: figures conflict across sources, flag).
- **Series D — May 27, 2026: >$1B at $26B valuation** ($25B pre per TechCrunch), **led by Lux Capital, General Catalyst, and 8VC**; existing incl. Founders Fund, Elad Gil, Alpha Wave, Bain Capital Ventures; new: Ribbit Capital, Atreides, Layer Global. Disclosed: **run-rate revenue $492M**; enterprise usage >10x YTD; **"89% of code committed by our engineers is committed by Devin."** Primary: https://cognition.com/blog/series-d ; https://techcrunch.com/2026/05/27/ai-coding-startup-cognition-raises-1b-at-25b-pre-money-valuation/
- **Series E — Sept 8, 2026: $2B at $48B**, **led by Andreessen Horowitz and Accel**, with Founders Fund, General Catalyst, et al. https://www.reuters.com/technology/cognition-ai-raises-2-billion-48-billion-valuation-2026-09-08/ ; https://www.wsj.com/pro/private-equity/peter-thiel-backed-ai-startup-cognition-raises-funds-at-48-billion-valuation-41b41b85 ; https://techcrunch.com/2026/09/08/cognition-hits-48b-valuation-signaling-investors-believe-ai-coding-is-far-from-a-winner-take-all-market/
- Revenue at Series E: **run-rate ~$900M** (reported via Techmeme/briefings: https://valueaddvc.com/blog/cognition-valuation-2026-48b-2b-round-900m-arr ; byteiota) and **$4–5B annualized projected by end-2026** (TechCrunch, citing The Information). Total raised lifetime: **>$2.5B** (Sourcery). Trajectory: $1M (Sep 2024) → $73M (Jun 2025) → $492M (May 2026) → ~$900M (Sep 2026).

### 2.4 Unit-economics commentary (analysis, not company claim)
- The ACU→messages pivot converts metering from an infrastructure proxy (compute time) to a perceived-value proxy (messages), standardizing against competitors; daily+weekly refresh and API-priced overage cap downside while nudging power users to Max.
- Margin thesis is now a *research program*: (a) in-house RL post-training of cheap open bases (K2.7→SWE-1.7, K3→SWE-2) instead of renting frontier inference; (b) routing/sidekick harnesses (Fusion "up to 60% cheaper"; SWE-2 medium "81% cheaper than SWE-1.7 at higher score"; Fusion+Fable "41% cheaper"); (c) zero-margin free SWE-model promotions (SWE-2 free through Oct 10, 2026) as acquisition for quota-hungry cloud usage.
- Valuation math: Series D ≈ 53x $492M run-rate; Series E ≈ 53x ~$900M — the multiple held steady while revenue ~doubled in four months; the $48B is underwritten by the $4–5B year-end projection (~10–12x projected). TechCrunch frames the round as a bet that AI coding is *not* winner-take-all.
- Cross-checks that temper the story: SWE-2's headline "64% cheaper than Fable 5.1" is measured on Cognition's own benchmark with list-price assumptions; Fusion's headline moved from 35% → 60% after a data refresh; "88% of merged PRs" is internal-only.

---

## 3. RESEARCH DOCTRINE

### 3.1 "Don't Build Multi-Agents" — Walden Yan, Jun 12, 2025
URL: https://cognition.com/blog/dont-build-multi-agents
- Two principles of context engineering: **(1) Share context, and share full agent traces, not just individual messages; (2) Actions carry implicit decisions, and conflicting decisions carry bad results.**
- Core argument: parallel-writer subagents are fragile (Flappy Bird example — subagents build a Mario background and a wrong-feeling bird; the final agent inherits miscommunications). Sharing partial context still fails (conflicting implicit decisions). Default to a **single-threaded linear agent**; for long horizons add a dedicated **context-compression model** (Cognition fine-tuned a small model for this).
- Cited as wrong-way pressure: OpenAI Swarm, Microsoft AutoGen. Historical examples: edit-apply models (deprecated), Claude Code's deliberately-readonly subagents. Coincidentally published a day before Anthropic's multi-agent research system post (acknowledged in the Apr 2026 follow-up).

### 3.2 "Multi-Agents: What's Actually Working" — Walden Yan, Apr 22, 2026 (partial reversal)
URL: https://cognition.com/blog/multi-agents-working
- Keeps the ban on **parallel-writer swarms** ("unstructured swarms... mostly a distraction"). Changed inputs: models far more agentic; Devin usage in largest enterprises **~8x in 6 months**; cost pressure from coming "Mythos class" models; sensational parallel-agent demos (Cursor 200k-LOC browser; Anthropic 100k-LOC C compiler; karpathy/autoresearch) dismissed as sharing "a simple, verifiable success criterion," unlike real software which "requires a system that scales human taste."
- Three working patterns — **writes stay single-threaded; other agents contribute intelligence, not actions**:
  1. **Generator–verifier review loop**: Devin Review finds avg **2 bugs/PR, ~58% severe**; counterintuitively works best with **zero shared context** (clean-context reviewer reasons backward from the diff, escapes Context Rot). Devin and Review natively iterate before a human opens the PR.
  2. **"Smart Friend"**: smaller/faster primary consults a big/expensive model as a tool. Candid postmortem: **failed with SWE-1.5 as primary** (couldn't know when to escalate); SWE-1.6 closes the gap but "not where we want it"; works across frontier models as a **"capability router rather than a difficulty escalator."** Hard parts: how a dumber model knows it's at its limits; what context to fork; over-scoped smart friend responses. (Anthropic's similar "Advisor" beta noted.)
  3. **Manager Devin → child Devins** via an internal MCP ("map-reduce-and-manage"): needed fixes for over-prescription, assumed shared state, and missing child→manager→sibling messaging.
- Doctrine through-line: "intelligence is injected at every stage... not as a swarm of autonomous actors, but as a coordinated system that scales human taste."

### 3.3 Fusion as doctrine-in-action (Jun 29, 2026)
- Fusion is the industrialized Smart Friend: multiple *models*, one writer's harness — hence consistent with (not a betrayal of) the multi-agent principles: the main agent retains plan/ambiguity/review (principle 2), sidekick handles mechanical writes under main-agent direction, routing happens at compaction boundaries to respect cache economics. The Fusion post explicitly names Smart Friend and Anthropic Advisor as the cache-tax approaches it supersedes, and invites notes at walden@cognition.ai.

### 3.4 Trustworthiness evals (Jul 8, 2026)
URL: https://cognition.com/blog/measuring-open-source-model-trustworthiness ; re-run inside https://cognition.com/blog/swe-2
- Motivation: strong open bases (Kimi K2.x, DeepSeek-V4, GLM) come from mainland-China labs subject to "core socialist values" content rules; NIST/CAISI found DeepSeek echoes ~4x more inaccurate CCP narratives; Promptfoo found ~85% propaganda-tinged refusals on CCP-sensitive prompts; CrowdStrike/Booz Allen claimed Qwen3-Coder introduced ~130% more vulnerabilities when it believed it served a US agency.
- **Propaganda/censorship**: Pan & Xu (2026) 145 politically sensitive questions × EN/zh-Hans/zh-Hant × 5 samples, six graded axes, run **inside the Devin harness** to dodge evaluation-awareness. Result: propaganda highest in Simplified Chinese across all models; SWE-1.7 ≈ GPT 5.5 / Opus 4.8 (zh-Hans propaganda rate: SWE-1.7 6.5% vs Kimi K2.6 29.2%, DeepSeek-V4 21.7%, K2.7 12.8%, Opus 4.8 6.3%, GPT 5.5 9.5%; EN: 0.1%).
- **Politically-motivated refusals**: e.g. watchlist face-match task in a surveillance platform — K2.6/K2.7 COMPLY (adding camera location + timestamps unprompted); SWE-1.7, Opus 4.8, GPT-5.5 REFUSE with named concern.
- **Context-dependent vulnerability (differential security)**: identical security-critical builds (payment webhook, identity portal, secrets vault) under six personas (Western carrier EN; Pakistani carrier Urdu; Chinese carrier, Tibet zh-Hans; Falun Gong org EN) with/without a secure-implementation instruction; GPT 5.5 judge scores 1–5. Finding: **no statistically significant persona effect for any model** — contradicting CrowdStrike's raw-completion results; hypothesis: agentic harness (system prompts, orchestration) suppresses persona-conditional behavior. Notably K2.7 wrote *more* secure code under the Falun Gong persona (opposite sign).
- Updated in SWE-2 post (Sept 10, 2026): binary pass/fail single-judge variant (GPT 5.6 Luna + reference materials) — SWE-2 passes 98.0% overall; vulnerability eval unchanged, still null.

### 3.5 FrontierCode 1.1 — methodology + legitimacy notes
URL: https://cognition.com/blog/frontier-code-1.1 (v1: https://cognition.com/blog/frontier-code)
- What it is: 150 real-PR-derived tasks ("Main" = 100 hardest, "Extended" = 150; **Diamond subset deprecated** in 1.1 as too noisy); grades correctness **and code quality** against 1000+ reviewer criteria incl. "blockers."
- 1.1 changes: **fair internet use** doctrine (task solutions leak into upstream repos/mirrors/package registries; a prompt defining fair use + a programmatic scanner zeroing runs that open upstream PRs/diffs — unfair use fell below 1%); 75 of 1000+ blocker criteria relaxed as overly strict.
- **Legitimacy critique (analysis):** FrontierCode is a self-run benchmark on which Cognition's own models headline every launch post; cost axes assume list pricing; models are run in whichever harness they were "primarily developed for" (Devin CLI for open-weight models) and the **best score across reasoning-effort settings** is reported — choices that flatter the in-house stack. Cognition itself cites an environment of reward-hacking anxiety (Cursor's "reward hacking is swamping model intelligence gains"; Datacurve's DeepSWE 1.1 revision; Posttrain Clean Coding Index; METR's GPT-5.6 Sol predeployment eval — all linked from the 1.1 post). Third-party replication of FrontierCode numbers was not found in this slice — **UNVERIFIED externally.**

### 3.6 SWE-bench history (the origin number)
- Devin technical report (Mar 2024): **13.86% unassisted** on SWE-bench (79/570 resolved), vs previous best 1.96% unassisted / 4.80% assisted — SOTA at the time. https://cognition.com/blog/swe-bench-technical-report ; announcement: https://x.com/cognition/status/1768716551056728233
- Cognition subsequently **stopped reporting SWE-Bench numbers in 2024** (stated in SWE-1.5 post), pivoting to Junior-Dev → SWE-Bench Pro → FrontierCode — a decade-long pattern of benchmark exit after SOTA. (A third-party claim that "Devin 3 hit 90% SWE-bench Verified" circulates — https://rephrase-it.com/blog/devin-3-at-90-swe-bench — **UNVERIFIED, no primary found**.)

---

## 4. ADOPTION REALITY-CHECK

### 4.1 Enterprise claims (company-posted, on the record)
- Series D post names: **Citi, Mercedes-Benz, Goldman Sachs, Elevance, Dell, Santander, U.S. Army, U.S. Navy**; startups Exa, Modal, Eight Sleep, OpenRouter. https://cognition.com/blog/series-d
- **NASA**: reported as an enterprise customer by SF Standard (Mar 24, 2026: "Goldman Sachs, Citi, and NASA") and repeated in round coverage (byteiota: Nvidia, GE Aerospace, Goldman Sachs, Mercedes-Benz, NASA). Not named in Cognition's own Series D list — **secondary-source verification only**.
- **Mercedes-Benz**: "cut an eight-month legacy modernization project down to eight days" (company case study linked from Series D post). **Company claim.**
- **Itaú** (LatAm's largest bank): "fixes 70% of security vulnerabilities automatically with Devin." **Company claim.**
- SI channel: **Infosys** and **Cognizant** embed Devin in delivery. Government: Cognition for Government (Feb 2026).
- Devin's 2025 "performance review" (self-published): "senior-level at codebase understanding but junior at execution"; PR merge rate 34% → 67% YoY; 4x faster problem-solving, 2x more resource-efficient. https://cognition.ai/blog/devin-annual-performance-review-2025

### 4.2 "Devin writes 95% of our code" — Scott Wu
- **VERIFIED as a real, current claim**: Sourcery podcast (Paris, RAISE AI Summit), published Jul 27–28, 2026: "We almost exclusively used Devin to build Devin at this point. Like 95% of the code or something is written by Devin... total amount of code shipped is roughly 7x in that time." Customer usage up 11–12x in six months; revenue "~$37M → $492M annualized in a year." Sources: https://www.sourcery.vc/p/breaking-devin-writes-95-of-our-code ; https://listennotes.com/podcasts/sourcery/inside-the-fastest-growing-HjizODKTFhY/ ; https://finance.biggo.com/news/f3b5480d5467a592
- Lineage of the number: **89%** at Series D (company post, May 2026) → **95%** (Wu, Jul 2026). Interpretation caveats: "code committed by Devin" includes commits where Devin executes human specs; ZenML's case-study notes the "Devin builds Devin" claim "may overstate the proportion of the product built autonomously" though "more than vaporware" (https://zenml.io/llmops-database/autonomous-software-development-agent-for-production-code-generation).

### 4.3 Reddit / community skepticism
- r/ChatGPTCoding "What happened to Devin?" (Mar 2025-era thread, 91 upvotes): "No one seems to be talking about Devin anymore... dominated by Cursor, Cline, Windsurf, Roo Code, Operator..."; top-adjacent comment: **"Devin demo was found to be faked — not just overhyped."** Also "Manus is an almost identical clone of Devin." https://www.reddit.com/r/ChatGPTCoding/comments/1jbsefc/what_happened_to_devin/
- r/aipromptprogramming (same period): "too expensive and overhyped... high price makes it hardly accessible to hobbyists"; "how the hell could Devin promise stuff the models it was using just couldn't do." https://www.reddit.com/r/aipromptprogramming/comments/1jbsce6/what_happened_to_devin/
- r/ycombinator: "focused very much on being an enterprise product... pricing was prohibitive for lean teams... wouldn't really flex on trials"; DeepWiki flagged as the genuinely loved asset. https://www.reddit.com/r/ycombinator/comments/1l380we/what_happened_to_devin_ai/
- Historical baggage: the March 2024 launch demo (30M+ views) drew "faked/edited" accusations (Upwork task video editing controversy) — summarized in Contrary Research's profile: https://research.contrary.com/company/cognition ; Blind thread "What's up with Devin by Cognition?" skews negative (https://www.teamblind.com/post/Whats-up-with-Devin-by-Cognition-YbD0KBvy).
- Counterweight: the company's own docs are more modest than its marketing (2025 review: "junior at execution"; SWE-2 post admits TB-4 weakness at 27.3%); NY Mag/SF Standard profile notes revenue secrecy until the $492M disclosure.

---

## 5. VERIFICATION LEDGER
| Claim | Status |
|---|---|
| SWE-2 = Kimi K3 2.8T post-train; 50.0% FC 1.1 Main; full table | VERIFIED (primary blog fetched) |
| SWE-2 "64% cheaper than Fable 5.1"; Pareto-penalty method | VERIFIED (primary; self-measured cost basis) |
| SWE-1.7 = K2.7 base; 42.3%/42.0% FC Main (internal inconsistency) | VERIFIED w/ note |
| Fusion "up to 60% cheaper" (revised from 35%); 88% internal PRs | VERIFIED as company claims (primary) |
| Fable 5 suspended by US gov directive 6/12/26 | UNVERIFIED (company footnote only) |
| Security Swarm 72% vs Claude 68%/Codex 48%/Cursor 26% | VERIFIED as vendor benchmark (self-run) |
| ACU pricing $2.25/$2.00; Team $500 w/ 250 ACUs | VERIFIED (multiple secondary, consistent) |
| Current plans Free/$20/$200/$80+$40; daily+weekly refresh; API-priced overage; ACUs absent from consumer page | VERIFIED (devin.ai/pricing fetched) |
| "50% MoM enterprise growth" (task brief) | UNVERIFIED — replaced by >10x YTD / 8x-6mo / 80x YoY figures |
| Series D $1B @ $26B, $492M ARR, Lux/GC/8VC | VERIFIED (primary + TechCrunch) |
| Series E $2B @ $48B, a16z+Accel, ~$900M run-rate, $4–5B projection | VERIFIED (Reuters/WSJ/TechCrunch); ~$900M via briefings (secondary) |
| "Don't Build Multi-Agents" + 2 principles | VERIFIED (primary fetched) |
| Trustworthiness evals (Pan-Xu 145; refusal; differential security) | VERIFIED (primary fetched) |
| FrontierCode 1.1 methodology (fair-internet-use, blockers, Diamond deprecated) | VERIFIED (primary fetched) |
| 13.86% SWE-bench unassisted (2024) | VERIFIED (primary + X) |
| NASA as customer | PARTIAL (press only, not in Cognition's own list) |
| Mercedes 8mo→8d; Itaú 70% auto-remediation; 95% Devin-written code; 89%→95% lineage | VERIFIED as company claims |
| Reddit skepticism + demo-faking accusations | VERIFIED (threads fetched/quoted) |
| "Devin 3 at 90% SWE-bench Verified" | UNVERIFIED |
