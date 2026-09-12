# Iteration 12 — Agent C: Cursor (Models + Benchmarks + Community)

**Date of research:** 2026-09-11 · **Target harness:** Cursor (Anysphere, Inc. — since 2026-08-14 a wholly owned SpaceX subsidiary inside the SpaceXAI division)
**Legend:** ✅ VERIFIED (primary source fetched) · ✅S VERIFIED (secondary/corroborated) · ⚠️ UNVERIFIED / flagged

---

## 0. Headline context (changes the framing of everything below)

- **SpaceX acquired Anysphere for $60B, all-stock.** Announced 2026-06-16, **closed 2026-08-14** (Cursor post: "Cursor is now a part of SpaceX", https://cursor.com/blog/joining-spacex). Anysphere shareholders received ~391M SpaceX Class A shares (SEC Form 8-K per valueaddvc.com; press-reported price: CNBC 6/16, The Verge 8/14). Cursor now operates inside **SpaceXAI** (division formed when SpaceX absorbed xAI on 2026-02-02; xAI valued $250B). ✅S
- **OpenAI is winding down its model contract with Cursor.** OpenAI post 2026-08-28: proposed **shutoff 2026-11-12** (max contractual notice after change-of-control), citing inability to trust SpaceX/Musk ToS compliance (https://openai.com/index/our-decision-on-cursor-following-its-acquisition-by-spacex/). WIRED: Cursor was a top-5 OpenAI customer; >$1B/yr projected API revenue. Cursor CEO Michael Truell (now a SpaceX exec) claims OpenAI models are "~5% of Cursor user traffic." Anthropic (Tom Brown) publicly committed to keep Claude in Cursor; WIRED notes Anthropic has $45B of SpaceX datacenter deals (⚠️ that motive is WIRED's framing). ✅
- **April 2026 partnership preceded the deal:** "Cursor partners with SpaceX on model training" (2026-04-21, https://cursor.com/blog/spacex-model-training) — Colossus compute access; reports of an April option: ~$10B partnership vs $60B buyout. ✅
- **Financials (secondary):** $2.3B Series D co-led by Accel/Coatue at **$29.3B** (2025-11-13); ~$1B ARR Nov 2025 → **$4B annualized by Jun 2026** (valueaddvc, freemalta); gcn.com cites $2B ARR + "more than a million paying developers." Market share ~26% (May 2026), down from 41% peak (freemalta). ⚠️ All secondary — treat as directional.
- Acquisition (Firetiger joins Cursor, 2026-08-13, https://cursor.com/blog/firetiger) and Gartner MQ Leader for Enterprise AI Coding Agents (2026-05-22, https://cursor.com/blog/cursor-leads-gartner-mq-2026) also on blog. ✅

---

## 1. MODEL LINE

### 1.1 Composer 2 — arXiv 2603.24477 ("Composer 2 Technical Report") ✅
- **Base:** Kimi K2.5 (Moonshot AI), **1.04T param / 32B active MoE**, chosen via internal evals (codebase perplexity, coding knowledge, state tracking); Kimi K2.5 ref: arXiv 2602.02276. Sources: https://arxiv.org/abs/2603.24477 (v1 2026-03-25, v2 2026-03-26), https://cursor.com/blog/composer-2-technical-report (2026-03-27), launch post https://cursor.com/blog/composer-2 (2026-03-19).
- **Two-phase training:** continued pretraining (code-heavy mix; 32k → 256k long-context → short SFT) then large-scale agentic RL. Key finding: **lower pretraining loss predicts higher downstream RL reward** (validated on Qwen3-Coder-30B-A3B scaling ladder).
- **MXFP8 on NVIDIA B300s, AdamW** — confirmed verbatim. Custom low-precision MoE kernels for Blackwell.
- **MoE router replay:** inference engine returns selected expert indices per token/layer; trainer overrides routing to match; Cursor's extension filters replayed experts below a plausibility threshold derived from the router's own top-k (reduces p99 numerics mismatch). RL inference via **Fireworks AI** partnership (also Colfax thanked).
- **Async multi-region RL pipeline** ("fully asynchronous RL pipeline spanning multiple regions") + **Anyrun** internal compute platform (hundreds of thousands of sandboxed coding environments); weight sync / fault tolerance / environment fidelity covered. ✅
- **MTP speculative decoding:** confirmed — additional **Multi-Token Prediction layers trained for speculative decoding** to serve faster in production (§3.1, cites Gloeckle et al. 2024, DeepSeek-AI 2024b). ✅
- **Self-summarization (compaction-in-the-loop):** confirmed — RL rollouts chain **multiple generations linked by model-written summaries** (technique introduced in Composer 1.5; refs https://cursor.com/blog/self-summarization) to enable long-horizon training (§4.1). ✅
- **Correction to task brief:** sharded **Muon + dual-mesh HSDP** details are NOT in the arXiv report — they are in the **Composer 2.5 blog** (below). The Oct 2025 Composer-1 post (https://cursor.com/blog/composer) already used "MXFP8 MoE kernels + expert parallelism + hybrid sharded data parallelism."

### 1.2 Composer 2.5 — released 2026-05-18 ✅
- Source: https://cursor.com/blog/composer-2-5 (2026-05-18). Same **Kimi K2.5 open-source checkpoint** as Composer 2. Docs: https://cursor.com/docs/models/cursor-composer-2-5.
- **Targeted RL with textual feedback:** localized on-policy distillation — insert a hint at the offending turn → teacher distribution; KL loss pulls student there; retains global RL objective. Refs arXiv 2601.19897, 2601.20802, 2601.18734.
- **Synthetic data:** 25x more synthetic tasks than Composer 2 (e.g., "feature deletion" tasks); reward-hacking war stories (reverse-engineering a Python type-checking cache; decompiling Java bytecode to resurrect a deleted API).
- **Sharded Muon + dual-mesh HSDP:** Newton-Schulz at natural granularity (per attention head / per stacked-MoE expert); async all-to-all batching of same-shaped tensors; **optimizer step 0.2s on the 1T model**; separate HSDP layouts for expert vs non-expert weights; CP=2 + EP=8 on 8 GPUs.
- **Pricing:** $0.50/M in, $2.50/M out (cache read $0.20); **Fast variant same intelligence: $3.00/M in, $15.00/M out** — Fast is the default. Double-usage first-week promo; a 1/10-rate 10x promo ran 2026-05-19 (forum-confirmed).
- **1T-parameter from-scratch model — STATUS:** announced as **in training** with SpaceXAI: "a significantly larger model from scratch, using 10x more total compute. With Colossus 2's million H100-equivalents…" (Composer 2.5 blog + /composer page). No name, no date, no benchmark as of 2026-09-11. ⚠️ Note: the "1T" in the Muon section = the K2.5-based (1.04T) model being continued-pretrained, NOT the from-scratch model (which is "significantly larger" than that).
- Composer lineage: Composer 1 (2025-10-29) → Composer 1.5 (2026-02-09, "scaled RL by over 20x") → Composer 2 (2026-03) → Composer 2.5 (2026-05).

### 1.3 Real-time RL (Composer, production online RL) ✅
- https://cursor.com/blog/real-time-rl-for-composer (2026-03-26, Wanqi Zhu). Real inference tokens as training data; user responses (accept / reject / frustrated follow-up) as reward. **Ships an improved checkpoint behind Auto as often as every ~5 hours**; keeps data (almost-)on-policy; each cycle gated on eval suites incl. CursorBench before deploy. Explicit reward-hacking incidents acknowledged and fixed.
- Reported gains (secondary recap of the blog, headsupai.io): +2.28% edit persistence, −10.3% latency. ⚠️S exact figures re-verify against blog text.
- First used for **Tab**, then applied to Composer (blog states this explicitly).

### 1.4 Cursor Tab online RL ✅ (all four numbers exact)
- https://cursor.com/blog/tab-rl (2025-09-12, Shomil Jain). **>400M requests/day**; checkpoint rollout + data collection = **1.5–2h** on-policy loop; reward shaping **accept +0.75 / reject −0.25 / no-suggestion 0** ⇒ only suggest when P(accept) ≥ 25%. Result: **21% fewer suggestions, 28% higher accept rate**. Legacy stack docs/case studies: zenml.io, rlinf.readthedocs.io.

### 1.5 Grok first-party line ("jointly trained with SpaceXAI") ✅
- Docs (https://cursor.com/docs/models-and-pricing) list **Cursor Grok 4.6** and **Cursor Grok 4.5** in the Cursor Models pool, each noted **"Jointly trained by Cursor and SpaceXAI"**: Grok 4.6 $2 in / $0.5 cache / $6 out (Fast: $4/$1/$12); Grok 4.5 $2/$0.5/$6 (Fast: $4/$1/$18). ✅
- Grok 4.6 cited as "an early joint model" in acquisition-close coverage; nav shows Models → Grok + **Evals** (https://cursor.com/evals), plus **Grok Bot** (https://x.ai/bot) and a "Grok Bot Terms" legal page. Free plan is locked to Grok 4.6 Medium or Composer 2.5 Fast (forum-confirmed staff answers). ✅

### 1.6 Cursor Router (Auto) ✅
- Three Auto modes confirmed in docs + staff forum replies: **Auto Cost / Auto Balance (default) / Auto Intelligence**.
- **Auto Cost:** flat list price **$1.25 in / $1.25 cache-write / $0.25 cache-read / $6 out**, hidden by default; **exempt from Cursor Token Rate**. Auto Balance/Intelligence bill at the routed model's API rates (+ Token Rate when third-party).
- Auto Cost + first-party models remain usable after hitting included-usage caps (after an Aug 2026 fallback bug was fixed server-side ~Aug 10).

---

## 2. PRICING / USAGE ✅

Source: https://cursor.com/docs/models-and-pricing, https://cursor.com/docs/account/teams/pricing, https://cursor.com/help/account-and-billing/bugbot-usage-based-billing, Pricing Policy (updated 2026-08-21, https://cursor.com/terms/pricing).

- **Two usage pools** (reset monthly): **Cursor Models** (Grok 4.6, Grok 4.5, Composer 2.5 — "significantly more included usage") vs **Other Models** (third-party at API list price).
- **Plans:** Start ₹649/mo (India-only; Cursor Models pool only, no Fast/effort control); Pro $20 / Pro Plus $60 / Ultra $200 (Other Models included ≈ $20/$70/$400). Teams: Standard $40, Premium $120/user/mo (5x usage).
- **Cursor Token Rate: $0.25/M tokens** on Teams & Enterprise for **third-party** model requests — applies to input/output/cached tokens, included + on-demand + **BYOK**, and to Auto Balance/Intelligence routing to third-party. Exemptions: all first-party models (Composer, Grok) and Auto Cost. Justified as covering "custom model execution (Tab, Apply), infrastructure and processing."
- **Bugbot:** switched from **$40/seat/mo** to **usage-based** (announced 2026-05-11, https://cursor.com/blog/may-2026-bugbot-changes; existing customers migrate at first renewal after 2026-06-08). **Average run $1.00–$1.50** by PR size/complexity — task brief confirmed exactly. Teams: bills from on-demand; Individuals: included usage then on-demand. Effort levels (Default/High/Custom) only on usage-based plans. Pricing Policy counts "Bugbot Usage Fees" toward Precommitted Usage; "Bugbot dynamically selects the optimal model for each task."

---

## 3. BENCHMARKS

### 3.1 CursorBench (internal) ✅
- Methodology post: https://cursor.com/blog/cursorbench (2026-03-11, Naman Jain). Hybrid online–offline evals. Tasks mined via **"Cursor Blame"** (commits traced to originating agent request); short, underspecified real-developer phrasing; **agentic graders**; refresh every few months; current version **CursorBench-3** (task scope ~2x v1 in LOC/files; larger than SWE-bench Verified/Pro/Multilingual; includes monorepos, production-log investigation). Claimed: better frontier model separation + better correlation with online metrics than public benchmarks. Also used as regression gate in the 5h real-time-RL loop.
- Composer-2-report scores (our harness; self-reported where both exist) ✅ (arXiv 2603.24477 Table 1):

| Model | CursorBench | SWE-bench Multi. | Terminal-Bench |
|---|---|---|---|
| **Composer 2** | **61.3** | **73.7** | **61.7** |
| Composer 1.5 | 44.2 | 65.9 | 47.9 |
| Composer 1 | 38.0 | 56.9 | 40.0 |
| Opus 4.6 High | 58.2 | 75.8/77.8 | 58.0/65.4 |
| GPT-5.4 | 63.9 | 76.8/– | 66.5†/75.1 |
| GPT-5.3 Codex | 59.1 | 74.8/– | 64.8†/77.3 |
| GLM-5 | 42.7 | 66.9/73.3 | 59.6/56.2 |
| Kimi K2.5 (base) | 36.0 | 65.1/73.0 | 47.3/50.8 |

† = Claude Code / official-leaderboard scores substituted for Anthropic; OpenAI safety filters refused 5 GPT-5.4 + 3 GPT-5.3-Codex tasks (scored 0). Terminal-Bench claims in blogs: Composer 2 = 61.7 (self-harness), framed vs Claude Code leaderboard numbers. Composer 2.5 benchmarks ship only as blog images (table screenshot; no text table) — exact 2.5 numbers ⚠️ UNVERIFIED from text.
- Composer 2.5 adds **effort curves** (effort-calibration framing) — behavioral axes ("communication style, effort calibration") claimed to be invisible to public benchmarks.

### 3.2 Bugbot as an agentic review system ✅
- Architecture: original pipeline ran **8 parallel passes with randomized diff ordering → majority voting → validator models** for false positives; **fall 2025** switch to a **fully agentic loop** (reasons over diff, calls tools, decides depth; prompting flipped from restraining to aggressive investigation). Source: https://cursor.com/blog/building-bugbot (2026-01-15, Jon Kaplan).
- **Resolution-rate metric** (AI judge classifies at merge time whether flagged bugs were fixed; spot-checked with PR authors). 40 major experiments: **52% → 70%+** resolution; bugs/run 0.4 → 0.7; resolved bugs/PR 0.2 → 0.5. **>2M PRs/month** (Rippling, Discord, Samsara, Airtable, Sierra AI; also run on Cursor's own code pre-merge).
- **Autofix:** beta Jan 2026 → **GA 2026-02-25** (https://cursor.com/blog/bugbot-autofix, 2026-02-26): spawns independent cloud agents in VMs to propose fixes on the PR; **>35% of Autofix changes merged**; resolution 52% → **76%**.
- **Learned rules** (https://cursor.com/blog/bugbot-learning, 2026-04-08): self-improvement from live accept/act signals; 110k+ repos opted in, 44k+ rules generated. Published comparison: **Bugbot 78.13%** (n=50,310 PRs) vs Greptile 63.49% / CodeRabbit 48.96% / Copilot 46.69% / Codex 45.07% / Gemini Code Assist 30.93%. (Self-published methodology — weigh accordingly.)
- Effort levels (2026-05-11 changelog): Default 0.7 bugs/run @ >79% resolved; High 0.95 bugs/run. June 2026: ~3x faster (~90s vs ~5min), ~22% cheaper/run, 0.56 → 0.62 bugs/run (https://cursor.com/changelog/bugbot-updates-june-2026).
- **BugBench:** confirmed as the **internal Bugbot benchmark** used with the resolution-rate metric across the 40 experiments (named in the building-bugbot post via forum mirror https://forum.cursor.com/t/building-a-better-bugbot/149040/1). No public dataset. ✅S

### 3.3 Multi-agent kernel optimization ✅
- Repo: https://github.com/anysphere/kernel-optimization-results (created 2026-04-14; HEAD 87ea087). README: multi-agent system autonomously optimized **235 CUDA kernels for Blackwell B200**, **38% geomean speedup** over baselines. Structure: L1 (94 single-op), L2 (82 fused), Quant (33 FP8/NVFP4), FlashInfer-Bench (26), per-workload CSVs (baseline/SOL/selected latency). Writeup: https://cursor.com/blog/multi-agent-kernels. ⚠️ Self-published baselines; no independent replication found.

---

## 4. COMMUNITY

### 4.1 Hacker News
- Series D era: "Cursor: Past, Present, and Future" — 61 pts / 60 comments, 2025-11-13, https://news.ycombinator.com/item?id=45915087 (same day as the $2.3B Series D). ✅
- Acquisition: "SpaceX Purchases Cursor, a Claude Code and OpenAI Codex Competitor" — 2026-06-16, https://news.ycombinator.com/item?id=48557095. ✅
- OpenAI cutoff cluster (2026-08-29 → 09-01): https://news.ycombinator.com/item?id=49486444 ("OpenAI Dumps Cursor"), 49487134, 49493319 ("Will it be Anthropic or X AI after Cursor getting no access to OpenAI?"), 49511084, 49516583. Small point counts on most — discussion migrated to forums/X. ✅

### 4.2 Cursor Forum (forum.cursor.com) — dominant complaint patterns 2026 ✅
1. **Forced-Composer routing:** legacy request-based plans route subagents through Composer 2/2.5 Fast "by design" (staff: "It's not a bug, it's a current limitation of how the plan works"); users who disabled Explore subagent models still saw Composer spawned (bug acknowledged). https://forum.cursor.com/t/cursor-is-losing-the-things-that-made-it-worth-paying-for/162929
2. **Usage-cap fallback bug (Jul–Aug 2026):** at 100% included usage, "Switch to auto" set invalid model value; Auto Balance / Composer 2.5 Fast wrongly blocked while Auto Cost + plain Composer 2.5 worked; fixed server-side ~2026-08-10. Threads 166442, 166657, 166489.
3. **Free-plan locks + quota mismatch:** locked to Composer 2.5 Fast (later Grok 4.6 Medium), "hit your usage limit" while dashboard showed remaining quota — enforcement/dashboard desync acknowledged, no ETA. Thread 165268.
4. **Burn-rate anxiety:** "$200 plan exhausted by 10% in 48h" (thread 160544): Opus 4.6 high-thinking reasoning tokens + Composer cache-read volume + Skills context bloat (user measured 92K tokens of skills in a 200K window). Staff response: dashboard "looks scarier than spend" because cache reads are discounted; suggested disabling Fast.
5. **Billing-transparency meta-thread:** >400 comments / ~900 downvotes era nostalgia from the Jun 2025 pricing switch; gcn.com (2026-09-06) reports rivals capturing switchers; Apology/refunds for Jun 16–Jul 4, 2025 overcharges recalled. ✅S
6. **Quality skepticism:** "Codex review vs Bugbot review" (thread 169245, 2026-08-23): power user documents Codex review finding real P1s after Bugbot reported clean passes (concurrency/state-machine classes); staff reply clarifies PR reviews are incremental by default. Good counterweight to Bugbot marketing metrics. ✅
- **Praise patterns:** Bugbot resolution-rate transparency, Composer 2.5 price ($0.50/$2.50), Tab fewer-but-better suggestions, 5h real-time-RL velocity; dev-tool bloggers (RLinf, ZenML) treat Cursor's online-RL stack as the reference implementation.

### 4.3 Reddit / third-party
- r/cursor sentiment mirrors the forum: billing/pool confusion + Composer-forced-default resentment dominate; Claude Code leads satisfaction polls (46% vs market-leader 9% per gcn.com-cited surveys). ⚠️S (secondary aggregations; no single authoritative r/cursor thread pinned here).

### 4.4 Enterprise adoption claims — FLAGGED
- **Cursor's own enterprise page (fetched 2026-09-11): "Trusted by 64% of Fortune 500 companies," 50,000+ enterprises, 100M+ lines of enterprise code/period.** ✅ https://cursor.com/enterprise
- Task brief's "**70% of F1000**" → ⚠️ **UNVERIFIED / likely conflation.** No primary source found; closest verified numbers are 64% F500 (Cursor page) and "more than half the Fortune 500" (adwaitx recap of Bugbot post). Use 64% F500, cite cursor.com/enterprise.
- "**1M+ paying**" → ⚠️S secondary only (gcn.com 2026-09-06: "more than a million paying developers"; consistent with Cursor's 2025-era "1M+ customers" claims). Not on any fetched primary page — attribute if used.
- Gartner: Leader in 2026 Magic Quadrant for Enterprise AI Coding Agents (https://cursor.com/blog/cursor-leads-gartner-mq-2026, 2026-05-22). ✅

---

## 5. Corrections vs. iteration brief
1. **Muon/HSDP** → belongs to the **Composer 2.5 blog (2026-05-18)**, not arXiv 2603.24477 (which has MXFP8/B300, router replay, async multi-region RL, MTP, self-summarization — all confirmed).
2. **"Grok 4.5/4.6 jointly trained with SpaceXAI"** → verbatim-confirmed in Cursor's pricing tables; both are first-party "Cursor Models."
3. **1T from-scratch Colossus 2 model** → announced/in-training since 2026-05-18, no name/date/benchmarks; it is "significantly larger" than the 1.04T K2.5 base — do not call it "the 1T model."
4. **Bugbot** → 8-pass majority-vote is the *pre-fall-2025* architecture; current system is the agentic loop + learned rules; 76–78% resolution, 2M+ PRs/mo confirmed.
5. **"70% F1000"** → replace with **64% of Fortune 500** (primary). **"~$1–1.50/run Bugbot since May 2026"** → confirmed exactly (2026-05-11).
6. **New since brief:** OpenAI wind-down (shutoff proposed 2026-11-12) materially changes third-party model availability in the harness comparison — OAI-model slices for other agents should note Cursor loses GPT-5.4/5.3-Codex access after Nov 12 unless renegotiated.
