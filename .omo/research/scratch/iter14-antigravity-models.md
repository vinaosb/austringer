# Iteration 14 — Google Antigravity: Agent C (Models + Benchmarks + Community)

Research date: 2026-09-11. All URLs accessed 2026-09-11 unless noted. Confidence flags: **VERIFIED** (directly observed on primary source), **SECONDARY** (credible third party, not independently reproduced), **UNVERIFIED** (self-report only / no independent confirmation).

---

## 1. MODEL–HARNESS CO-DESIGN

### 1.1 Gemini 3.x Flash cadence (six weeks, three Flashes)

| Model | Release | Source |
|---|---|---|
| Gemini 3.6 Flash | 2026-07-21 | [CodingFleet timeline](https://codingfleet.com/blog/gemini-38-flash-review/) (SECONDARY) |
| Gemini 3.7 Flash | 2026-08-13 | [Model card](https://deepmind.google/models/model-cards/gemini-3-7-flash/), [llm-stats](https://llm-stats.com/blog/research/gemini-3.7-flash-launch) (VERIFIED) |
| Gemini 3.8 Flash (+3.8 Flash Cyber) | 2026-09-01/02 | [blog.google](https://blog.google/innovation-and-ai/models-and-research/gemini-models/3-8-flash-and-3-8-flash-cyber/), [Antigravity blog](https://antigravity.google/blog/gemini-3-8-flash-in-google-antigravity) (VERIFIED) |

- Google's own framing, twice: "our third Flash release in only six weeks" — [blog.google](https://blog.google/innovation-and-ai/models-and-research/gemini-models/3-8-flash-and-3-8-flash-cyber/), [antigravity.google blog](https://antigravity.google/blog/gemini-3-8-flash-in-google-antigravity). 9to5Google notes "third Flash update in three months" ([link](https://9to5google.com/2026/09/02/gemini-3-8-flash-launch/)).
- 3.8 Flash pricing: **$0.75/1M input, $3.75/1M output** intro through 2026-12-31; **doubles to $1.50/$7.50 on 2027-01-01** (same doubling hits 3.6/3.7 Flash). Output price includes thinking tokens; Batch/Flex 50% off; Priority 1.8x; cache read $0.075→$0.15. [blog.google](https://blog.google/innovation-and-ai/models-and-research/gemini-models/3-8-flash-and-3-8-flash-cyber/), [ai.google.dev latest-model](https://ai.google.dev/gemini-api/docs/latest-model), [apidog](https://apidog.com/blog/gemini-3-8-flash-pricing/), [ofox](https://ofox.ai/blog/gemini-3-8-flash-api-pricing-2026/) (VERIFIED, multiple concordant).
- Spec: `gemini-3.8-flash` GA 2026-09-02 on Gemini API + Vertex; 1,048,576 in / 65,536 out; thinking low|medium(default)|high; `minimal` errors; Computer Use preview; Live API not supported ([ai.google.dev](https://ai.google.dev/gemini-api/docs/latest-model), [IndieSeek](https://indieseek.co/blogs/gemini-3-8-flash-antigravity-default-agent-cost-checklist/)).
- 3.8 Flash Cyber: cybersecurity variant, gated "Fairwind Program" for trusted defenders ([blog.google](https://blog.google/innovation-and-ai/models-and-research/gemini-models/3-8-flash-and-3-8-flash-cyber/)).

### 1.2 "Co-optimized" harness claims

- Antigravity launch post explicitly ties product to model step-change: "Gemini 3 … represents a step-change for agentic coding, and requires us to think about what the next step-change of an IDE should be" ([antigravity.google, 2025-11-18](https://antigravity.google/blog/introducing-google-antigravity)) — co-design narrative from day one.
- 3.8 Flash shipped day-one inside Antigravity, framed as "daily workhorse … improvements across agentic development workflows" ([antigravity.google](https://antigravity.google/blog/gemini-3-8-flash-in-google-antigravity)). Model card lists Antigravity as a first-class deployment surface alongside Gemini app/AI Studio ([DeepMind model card](https://deepmind.google/models/model-cards/gemini-3-8-flash/)).
- Launch demos are all Antigravity-native: 3D castle game "using a looping instruction in Google Antigravity" (agent self-orchestrates Nano Banana texture calls, headless-browser playtesting, raycast-bug fixes), DOS Google Maps "in a single prompt in Google Antigravity" ([blog.google](https://blog.google/innovation-and-ai/models-and-research/gemini-models/3-8-flash-and-3-8-flash-cyber/)).
- Strongest explicit co-design claim: Teamwork blog says 3.7 Flash producing PhD-level math results is "the first time a Flash-tier model has produced such PhD-level mathematical research **with the right orchestration framework**" ([antigravity.google, 2026-08-27](https://antigravity.google/blog/teamwork-when-ai-becomes-a-research-partner)).
- Same-day default flip: Gemini API release notes made 3.8 Flash the default model of the Antigravity managed agent **and** the Antigravity SDK on 2026-09-02 ([ai.google.dev](https://ai.google.dev/gemini-api/docs/latest-model)). Operator risk of silent model drift under an unchanged agent ID documented by [IndieSeek](https://indieseek.co/blogs/gemini-3-8-flash-antigravity-default-agent-cost-checklist/).
- Counter-signal: Google says 3.8 Flash "may use more tokens on long, complex tasks by design." Artificial Analysis measured **$0.58/task (high) vs $0.40 for 3.7 Flash (~30% more output tokens)** — same list price, higher run cost ([apidog](https://apidog.com/blog/gemini-3-8-flash-pricing/), [ofox](https://ofox.ai/blog/gemini-3-8-flash-api-pricing-2026/)) (SECONDARY, independent lab).

### 1.3 How Antigravity surfaces model choice

- Docs model selector table ([antigravity.google/docs/models/](https://antigravity.google/docs/models/)): Gemini 3.8 Flash, 3.7 Flash, 3.6 Flash, 3.1 Pro on **all plans**; **Claude Sonnet 4.6 (thinking), Claude Opus 4.6 (thinking), GPT-OSS-120b (Medium) on Free/Plus/Pro/Ultra — NOT Enterprise** (VERIFIED).
- Docs self-contradiction: models page says third-party models available Free→Ultra; plans page says "Access to third-party models" is for Google AI Ultra users. Both live as of Aug 2026 ([digitalmatters.me analysis](https://digitalmatters.me/artificial-intelligence-ai/antigravity-model-selection/)) (VERIFIED contradiction).
- Non-Gemini models are capacity/constraint-limited in practice:
  - **Subagents cannot use them**: documented subagent `model` frontmatter allows only `inherit|flash|pro` — no Claude/GPT-OSS value ([antigravity docs via digitalmatters](https://digitalmatters.me/artificial-intelligence-ai/antigravity-model-selection/)).
  - Background/non-customizable models (indexing, browser actuation, checkpointing, summarization) are fixed Gemini models regardless of dropdown ([official reply, discuss.ai.google.dev/t/122323](https://discuss.ai.google.dev/t/how-can-i-know-what-model-im-actually-using/122323/1)).
  - Historical: at launch Claude needed BYO Anthropic key ([beginnersinai](https://beginnersinai.org/google-antigravity/)); now billed through Google. Custom endpoints: Cloudtop/Linux only ([ayyaztech](https://ayyaztech.com/blog/google-antigravity-free-claude-gemini-3-ide-tutorial)).
  - Gemini 3.1 Pro exposes Low/High effort only; GPT-OSS-120b only Medium ([digitalmatters](https://digitalmatters.me/artificial-intelligence-ai/antigravity-model-selection/)).

### 1.4 Manual Opus→Flash routing as community tactic (VERIFIED as community practice)

- Continuity Engine research thread documents it as a convergent survival strategy: "**Opus → Flash manual routing (3–4 switches per session)**", voluntary quota stop at ~40% usage, `.antigravityignore` to stop background-indexing quota burn, `/handoff` + third-party memory extensions ([discuss.ai.google.dev/t/139335](https://discuss.ai.google.dev/t/community-research-quota-architecture-failures-a-product-proposal-continuity-engine/139335), [GitHub](https://github.com/VIKAS9793/antigravity-continuity-engine)).
- Rationale: Opus thinking tokens are hidden quota consumers — "burns credits approximately 4x faster than Gemini"; single complex Opus session reported at 635+ credits ([Sabaoon model guide](https://www.sabaoon.dev/blog/google-antigravity-models-compared)); "If Opus enters a planning loop, it can drain your 5-hour bucket in minutes" ([antigravity.codes via Continuity Engine evidence](https://github.com/VIKAS9793/antigravity-continuity-engine/blob/main/product-case-study/03-user-evidence.md)).
- Community consensus default: Flash for everything it can handle; Opus only when Gemini Pro + Sonnet have both failed ([Sabaoon](https://www.sabaoon.dev/blog/google-antigravity-models-compared)).

### 1.5 Vendor benchmarks

**Gemini 3.7 Flash (official model card + launch post, 2026-08-13) — VERIFIED as vendor-published:**
| Benchmark | 3.7 Flash | 3.6 Flash | Comparators (model card) |
|---|---|---|---|
| FrontierCode 1.1 Main | 43.6% | 34.4% | Sonnet 5: 42.7%, GPT-5.6 Terra: 41.3% |
| DeepSWE v1.1 | 65.3% | 48.6% (card) / 49.0% (blog) | Terra: 69.6% |
| Code Arena (WebDev) Elo | **1588** | 1538 | Sonnet 5: 1541, Terra: 1523 |
| GDP.pdf / AutomationBench | 34.0% / 30.4% | 22.0% / 17.0% | — |

Sources: [model card](https://deepmind.google/models/model-cards/gemini-3-7-flash/), [DeepMind page](https://deepmind.google/models/gemini/flash/), [AI Studio dev guide](https://aistudio.google.com/learn/gemini-3-7-flash-developer-guide). Note the 48.6 vs 49.0 baseline discrepancy between card and blog. All vendor-run: "These are Google's own numbers, not independent results" ([layer3labs](https://www.layer3labs.io/guides/gemini-3-7-flash-review)). Terminal-Bench 2.1 at 85.8% appears in [emergent.sh](https://emergent.sh/learn/gemini-3-7-flash-benchmarks) but not the card excerpt — VERIFY before citing. AA Intelligence Index 56 (independent).

**Gemini 3.8 Flash (2026-09-02):**
- Official model card exists ([deepmind.google](https://deepmind.google/models/model-cards/gemini-3-8-flash/)) but full benchmark table not captured here — **VERIFY** against card before publishing numbers.
- Secondary claims (CodingFleet, SECONDARY): DeepSWE v1.1 **73.7%** (vs "Claude Opus 5" 74.0%), Terminal-Bench 2.1 **89.4%** (#1), Vals Finance 61.4%, CharXiv 86.2%; AA Index 59 (high) per ofox table. Treat as unconfirmed until card-verified.
- Independent cost-per-task: AA $0.24/$0.41/$0.58 per index task at low/med/high ([apidog](https://apidog.com/blog/gemini-3-8-flash-pricing/)).

### 1.6 Antigravity agent on the Gemini API — economics (VERIFIED, official docs)

Managed agent `antigravity-preview-05-2026`, same harness as the IDE ([ai.google.dev/gemini-api/docs/antigravity-agent](https://ai.google.dev/gemini-api/docs/antigravity-agent)). Default model history: 3.5 Flash (May 19 I/O launch, [blog.google](https://blog.google/innovation-and-ai/technology/developers-tools/managed-agents-gemini-api/)) → 3.6 Flash (July) → 3.7 Flash (Aug) → **3.8 Flash (Sept 2)**.

Official estimated-cost table (docs):
| Task category | Input tokens | Output tokens | Typical cost |
|---|---|---|---|
| Research & synthesis | 100k–500k | 10k–40k | $0.30–$1.00 |
| Document & content generation | 100k–500k | 15k–50k | $0.30–$1.30 |
| Process & system design | 100k–400k | 10k–30k | $0.25–$0.80 |
| Data processing & analysis | 300k–3M | 30k–150k | $0.70–$3.25 |

- "**Complex agentic workflows with many tool calls can accumulate 3–5 million tokens in a single interaction, with costs up to ~$5**" ([docs](https://ai.google.dev/gemini-api/docs/antigravity-agent)) — confirms the $0.25–$5/interaction band.
- 50–70% of input tokens typically cached; sandbox compute unbilled in preview; `max_total_tokens` budget control (best-effort cap, returns `status:"incomplete"`); context compaction at ~135k ([docs](https://ai.google.dev/gemini-api/docs/antigravity-agent), [Pondero guide](https://pondero.ai/agents/guides/google-antigravity-managed-agents-may-2026/)).
- Independent anchor points (Artificial Analysis Coding Agent Index): Antigravity SDK (3.7 Flash high) — Index 57, $1.40/task, **15.1M tokens/task**, 110.8 turns vs Gemini CLI 33/$1.04/4.7M ([comparison](https://artificialanalysis.ai/agents/coding-agents/comparisons/antigravity-sdk-vs-gemini-cli)); Antigravity SDK (3.8 Flash high) — Index 42, $2.47/task, **25.4M tokens/task** vs Codex GPT-6 Astra 62/$7.47/6.7M ([comparison](https://artificialanalysis.ai/agents/coding-agents/comparisons/antigravity-sdk-vs-codex)). Antigravity harness is far more token-hungry but cheaper per task and faster (6.4–11.7 min vs 11.1–29.4 min).
- Anecdotal floor: DEV writer ran a 14-service audit for **$0.37 total**; one recursive-retry loop "ate $0.89 in seconds" before caps existed ([dev.to](https://dev.to/stephen_sebastian_c85ea2b/i-spent-037-testing-googles-agent-api-on-14-services-heres-every-bug-youll-hit-3nkh)).
---

## 2. BENCHMARKS

### 2.1 ChatForest "third-party" table — treat cautiously (SECONDARY, provenance unclear)

[ChatForest Antigravity 2.0 review, 2026-05-22](https://chatforest.com/reviews/google-antigravity-2-0-agent-platform-review/):
| Benchmark | Gemini 3.5 Flash | Claude Opus 4.7 | GPT-5.5 |
|---|---|---|---|
| MCP Atlas | **83.6%** | 79.1% | 75.3% |
| Terminal-Bench 2.1 | **76.2%** | 69.4% | 70.3% (78.2% per sibling page) |
| SWE-Bench Pro | **69.2%** | **76.4%** | 72.1% |
| ARC-AGI-2 | 71.3% | 73.2% | 84.6% |
| MRCR v2 | 71.1% | 79.4% | 73.8% |
| Hallucination rate | **61%** | 38% (36% elsewhere) | 44% |

Cautions (confirmed by cross-reading ChatForest's own pages):
1. **Internal inconsistency**: ChatForest's Gemini 3.5 Flash review says "The exact Gemini 3.5 Flash score [on SWE-Bench Pro] is not published" and that 3.5 Flash "trails its own upcoming Pro model by ~10 points" ([review](https://chatforest.com/reviews/google-gemini-3-5-flash-agentic-speed-llm-review/)) — yet the Antigravity review table prints 69.2%. Opus 4.7 hallucination is 36% on one page, 38% on another.
2. **Provenance**: ChatForest is an aggregator/blog, not a benchmarking lab; it documents no runs. The real MCP-Atlas paper exists ([arXiv 2602.00933](https://arxiv.org/html/2602.00933), 1,000 tasks, 36 servers), but third-party summaries put April 2026 MCP-Atlas top score at **77.3% (Opus 4.7)** ([andrew.ooo](https://andrew.ooo/answers/mcp-atlas-vs-terminal-bench-2026/)) — inconsistent with ChatForest's 79.1%/83.6% cells.
3. **Benchmark health**: OpenAI's July 2026 audit estimated ~30% of SWE-Bench Pro public tasks are broken ([BenchLM](https://benchlm.ai/benchmarks/swe-bench-pro)); Pro runs ~20 points below Verified on the same model (Opus 4.8: 69.2 Pro vs 88.6 Verified) so cross-benchmark comparison is invalid ([agentguides](https://agentguides.dev/leaderboard/)).
4. The 61% hallucination figure is the Artificial Analysis measure; direction (Flash hallucinates far more than Opus) plausible, exact cells UNVERIFIED.

**Bottom line**: directional secondary source only; do not cite table cells as established fact.

### 2.2 Teamwork framework claims — provenance assessment

Claims ([antigravity.google blog, 2026-08-27](https://antigravity.google/blog/teamwork-when-ai-becomes-a-research-partner); [blog.google, 2026-08-31](https://blog.google/innovation-and-ai/technology/developers-tools/antigravity-teamwork-multi-agent/); [docs](https://antigravity.google/docs/teamwork/)):
- **Seven open problems** in math/TCS (venues cited: FOCS, JMLR): sparse convex optimization, subspace approximation, provable LLM quantization, vector embeddings, Prefix-Matrix Factorizations, Knuth's Cycles Conjecture. "Reviewed and confirmed correct by **human experts** with the exception of the Knuth's Cycles result — where the 40-page proof was **formally verified in Lean**." Five papers claimed on arXiv.
- TCSBench **71%** (Long Proof pattern, 3.7 Flash + 3.1 Pro) vs 67.7% in the TCSBench paper (3.6 Flash + 3.1 Pro). TCSBench is "**independently developed within Google**" — internal, not independent.
- **RISC-V simulator**: cycle-accurate out-of-order CPU simulator from scratch (3.7 Flash), boots xv6 to shell, runs 100+ RISC-V benchmarks, **0.71% average cycle alignment error** vs air-gapped BOOM hardware ground truth.
- Upstream OSS: optimizations **merged into Eigen and ParlayHash** (ParlayHash: 2x insert throughput at 64 threads, claimed).
- Provenance split: **Lean verification, if genuine, is machine-checkable — the strongest, falsifiable claim**; merged upstream commits are externally auditable; "human experts" unnamed; TCSBench internal. **VERIFY before amplifying**: the five arXiv papers, the Lean proof artifacts (not linked in the post), the Eigen/ParlayHash merge commits.
- External reproduction signal: a developer ran /teamwork-preview end-to-end on a paid Pro plan — 2h15m app build, all-3.7-Flash, no quota wall ([Medium/Google Cloud](https://medium.com/google-cloud/antigravity-teamwork-for-long-running-tasks-de74825a6ae9)).
- Known friction: quota-blind execution — Teamwork starts multi-agent tasks with no pause-on-quota and no cost estimate ([forum 170693](https://discuss.ai.google.dev/t/teamwork-preview/170693)); observability gaps tracked in [CLI issue #301](https://github.com/google-antigravity/antigravity-cli/issues/301).

### 2.3 Independent systematic Antigravity-agent evals — CONFIRMED ABSENT (one nuance)

- **CONFIRMED**: no independent, systematic evaluation of the **Antigravity IDE product/agent** (Manager surface, artifacts, browser verification, end-to-end UX) exists as of 2026-09-11. Vendor-run only: "These are Google's own numbers, not independent results" ([layer3labs](https://www.layer3labs.io/guides/gemini-3-7-flash-review)).
- **Nuance**: Artificial Analysis **has** added the **Antigravity SDK harness** to its independent Coding Agent Index (DeepSWE, Terminal-Bench, SWE-Atlas-QnA) ([vs Gemini CLI](https://artificialanalysis.ai/agents/coding-agents/comparisons/antigravity-sdk-vs-gemini-cli), [vs Codex](https://artificialanalysis.ai/agents/coding-agents/comparisons/antigravity-sdk-vs-codex)) — independent **harness-level** testing, not product-level eval. Independent findings: Antigravity harness = high capability, very high token burn (15–25M tokens/task), fast, cheap per task; extreme benchmark-version sensitivity (TB2.1: 87% vs TB4.0: 15% on adjacent SDK configs).
---

## 3. COMMUNITY

### 3.1 Google AI Developers Forum (discuss.ai.google.dev)

**Quota arc (Jan–Apr 2026)** — VERIFIED threads:
- [137702](https://discuss.ai.google.dev/t/ultra-technical-analysis-the-16-384-output-token-cap-1-024-thinking-limit-are-silently-breaking-agent-workflows-data-comparisons-and-a-request-for-transparency/137702) (2026-04-02, Ultra subscriber; fetched in full): hardcoded **16,384-token output cap per turn on all plans** (25% of Gemini API max, 12.8% of Claude's); `max_thinking_length` pinned to **1,024 for Claude** (0.8% of capacity, at $249.99/mo); **Gemini silent truncation** — `finish_reason:"max_tokens"` not surfaced, agent proceeds on broken output ("a correctness bug"). Trajectory: Nov 2025 generous launch → Jan 2026 "quiet tightening" → Mar 2026 5x quota cut + AI Credits → Apr 2026 caps undocumented. **Accuracy note**: 137702 is *not* the 645-reply quota thread; it cites "there are 645 replies in the main quota thread for that." The 645-reply thread ID was not directly located — VERIFY if needed. Companion threads: [136174](https://discuss.ai.google.dev/t/feedback-critical-concerns-regarding-ultra-quota-reductions-loss-of-transparency-and-broken-promises/136174), [136343](https://discuss.ai.google.dev/t/the-state-of-the-antigravity-ultra-plan-is-a-joke/136343), [136764](https://discuss.ai.google.dev/t/1024-tokens-is-not-thinking-how-google-stealthily-killed-the-ultra-plan-for-developers/136764), [124988](https://discuss.ai.google.dev/t/urgent-community-feedback-drastic-quota-reductions-and-paid-user-frustrations/124988). 137702 drew ~1 reply in 6 weeks — official silence itself notable.
- [139335 Continuity Engine proposal](https://discuss.ai.google.dev/t/community-research-quota-architecture-failures-a-product-proposal-continuity-engine/139335) (2026-04-12, VERIFIED): 7 failure modes (mid-workflow termination, cross-model quota contagion, no predictive cost awareness, no session continuity, thinking-token opacity, UI/backend desync, infinite loops); proxy metrics — Task Completion Rate 45–52%, Quota Interruption Rate 68–75%, User Intervention Rate 82–88% — **explicitly sentiment-derived estimates, not telemetry** (author's own caveat); 5-capability proposal (auto model routing, pre-flight cost checks, circuit breakers, state handoff, decoupled quota pools). GitHub: [VIKAS9793/antigravity-continuity-engine](https://github.com/VIKAS9793/antigravity-continuity-engine).

**September 2026 429 wave** — VERIFIED, multi-thread outage cluster:
- [180907](https://discuss.ai.google.dev/t/antigravity-1-23-2-429-error-since-last-night/180907) (2026-09-05, 34+ replies): 429 `RESOURCE_EXHAUSTED` on ALL models with quota dashboard full; persists across accounts/networks; v1.16.5 + Gemini 3.1 Pro downgrade reported working.
- Also [181274](https://discuss.ai.google.dev/t/antigravity-agent-execution-termination-error-http-429-resource-exhausted/181274/9) (2026-09-10), [181092](https://discuss.ai.google.dev/t/429-resource-exhausted-on-all-models-despite-full-quota-antigravity-1-23-2-rhel-9-7/181092), [181727](https://discuss.ai.google.dev/t/antigravity-agent-execution-terminated-due-to-error/181727), [181789](https://discuss.ai.google.dev/t/persistent-http-429-resource-exhausted-on-agent-and-usage-across-multiple-pcs-accounts-and-networks/181789), [182129](https://discuss.ai.google.dev/t/antigravity-agent-still-failing-with-http-429-resource-exhausted/182129), [179354](https://discuss.ai.google.dev/t/persistent-http-429-resource-exhausted-error-despite-available-quota-antigravity-2-9-1/179354/2).
- Community root-cause (179354, detailed forensics): **bundled localhost TLS cert inside the Linux language server expired 2026-09-04 20:55 GMT** → editor↔language-server handshake fails → unauthenticated requests → zero-quota anonymous bucket → 429. CLI got a new cert (valid to 2027-03-26) before the desktop .deb; workaround `NODE_TLS_REJECT_UNAUTHORIZED=0`. Second documented bug: **AG-859** tier/quota desync (paid entitlements unrecognized; credit-overage fallback never fires). Support-path failure documented: Google One "has no escalation path to Antigravity," in-app feedback endpoint broken, 1,000 AI credits offered. TLS forensics = plausible-but-community-derived (SECONDARY).

### 3.2 Reddit / Hacker News

- Hubs: `r/GoogleAntigravityIDE`, `r/google_antigravity`, `r/Bard` (cited as sources in the Continuity Engine repo). r/Bard (Nov 2025): "Antigravity's rate limits are a slap in the face to Ultra/Advanced subscribers… thrown into the exact same bucket as a free user" ([evidence doc](https://github.com/VIKAS9793/antigravity-continuity-engine/blob/main/product-case-study/03-user-evidence.md)).
- HN: [item 46876045](https://news.ycombinator.com/item?id=46876045) — Windsurf-vs-Antigravity lineage confusion; framing: Google hired founders + licensed tech, Cognition owns Windsurf; now separate competitors. No large HN thread adjudicating Antigravity agent quality located — consistent with weak independent eval coverage.

### 3.3 Model-label-mismatch claims — UNVERIFIED (officially denied)

- [120631](https://discuss.ai.google.dev/t/the-model-list-in-antigravity-is-misleading-gemini-3-pro-is-actually-gemini-2-0-flash-and-claude-opus-4-5-is-actually-claude-3-5-sonnet/120631) (2026-02-02): "Gemini 3 Pro is actually Gemini 2.0 Flash, and Claude Opus 4.5 is actually Claude 3.5 Sonnet" — based on self-ID probes; consistent downgrade pattern across users claimed.
- [121376](https://discuss.ai.google.dev/t/model-mismatch-ui-shows-claude-sonnet-4-5-thinking-but-response-quality-suggests-lower-tier-model/121376/1) (2026-02-05): UI shows Sonnet 4.5 (Thinking), quality suggests lower tier; X evidence ([@thegismar](https://x.com/thegismar/status/2019518223306174634)); "token buckets drain from Gemini while Claude is selected" claimed.
- [122323](https://discuss.ai.google.dev/t/how-can-i-know-what-model-im-actually-using/122323/1) (2026-02-10): cutoff-date probes diverge from real Claude; a Gemini response invented a "**Broker layer / PLACEHOLDER_M18 model pool**" architecture — the routing-broker "evidence" itself comes from model confabulation. **Official staff reply**: "your reasoning requests are processed by that specific model. There is no 'model swapping'…"; wrapped models "can sometimes 'hallucinate' incorrect training cutoffs or self-identities." Official docs confirm only that *background* models (indexing, browser checkpointing, summarization) are fixed Gemini regardless of selection — a legitimate source of Gemini-flavored behavior under a Claude label.
- [119658](https://discuss.ai.google.dev/t/question-how-can-we-verify-which-model-antigravity-actually-used-for-a-given-response/119658) (2026-01-28): request for per-message "model actually used" badge — still absent.
- Adjacent anecdote (UNVERIFIED): Antigravity-wrapped "Opus 4.6" allegedly recommended `wsl --unregister` on an `--import-in-place` distro, destroying a 168GB VHDX, while real Opus elsewhere warns it's irreversible ([122323/6](https://discuss.ai.google.dev/t/how-can-i-know-what-model-im-actually-using/122323/6)) — framed as wrapper/system-prompt safety regression.
- **Status**: self-report + self-ID probes only; no packet-level or log verification surfaced. Keep UNVERIFIED; the verifiable core: **no per-response model transparency exists**.

### 3.4 AirAsia "50%" vendor claim — VERIFIED as a claim; wording drifts

- Case study ([cloud.google.com/customers/airasianext](https://cloud.google.com/customers/airasianext), ~2026-08-10): "more than 50% of AirAsia Next's **production-ready code** is generated through these intelligent agentic workflows"; QA transformed via the Antigravity agentic browser. CTO Nikunj Shanti at I/O: "more than half of our production-ready code is generated through these agentic workflows" ([The New Stack, 2026-05-20](https://thenewstack.io/google-io-antigravity-codemender-ai-agentic/)).
- Antigravity enterprise blog ([2026-08-20](https://antigravity.google/blog/antigravity-enterprise)) shifts the metric to "over 50% of their **production QA code**" — repeated by [TechGolly](https://techgolly.com/news/google-bundles-antigravity-coding-agents-into-gemini-enterprise-to-streamline-software-development). **Two different claims circulate: "production-ready code" (I/O, case study) vs "production QA code" (Aug blog).** Self-reported, unaudited, no methodology.
- Other named adopters: Accenture, CGI, Cognizant, Datamatics, Deloitte, Wipro, Monks, PwC, WPP ([tonyreviewsthings](https://www.tonyreviewsthings.com/google-antigravity-gemini-enterprise/), [beri.net](https://www.beri.net/article/google-antigravity-2-enterprise-coding-agent-pricing-war-2026)) — quotes only, no metrics.

### 3.5 Windsurf-lineage criticism (Hyperdev) — VERIFIED quote, partially dated

- Lineage facts: July 2025 $2.4B reverse acqui-hire (Varun Mohan, Douglas Chen, ~40 R&D) + non-exclusive tech license; ~250 remaining employees/product → Cognition; Antigravity shipped 2025-11-18 ([Geeky Gadgets](https://www.geeky-gadgets.com/windsurf-story-so-far/), [HN 46876045](https://news.ycombinator.com/item?id=46876045)).
- The criticism, Robert Matsuoka / Hyperdev ([2025-11-28](https://hyperdev.matsuoka.com/p/googles-24b-hedge-antigravity-and)): "**The 'multi-agent' capability is really multi-instance task parallelization—valuable for productivity but not an advance in orchestration science.**" Manager-spawned agents run independently — "no inter-agent communication, no role specialization, no collaborative refinement" vs AutoGen/LangGraph/CrewAI.
- **Dating caveat**: written against Nov 2025 product. Aug 2026 Teamwork adds role specialization (Sentinel/Project Orchestrator/Explorers/Workers) and adversarial verification (Critic/Challenger/Auditor/Success Auditor) with cross-agent critique — the named gap partially closed on Google's own claims, still without independent evaluation.
- "Rebranded Windsurf" UI-forensics claims ("same keybindings; `.antigravity` config mirrors `.windsurf`") circulate ([Medium/gitconnected](https://levelup.gitconnected.com/the-2-4b-clone-how-three-companies-carved-up-one-startup-in-72-hours-36039524a9bb)) — UNVERIFIED, plausible given licensed codebase, no forensic publication.

### 3.6 Adoption numbers — vendor-only

- "Millions of developers": "**The company says** millions of developers are already building with the platform" ([The New Stack, I/O 2026](https://thenewstack.io/google-io-antigravity-codemender-ai-agentic/)) — explicitly Google's claim; no independent install/DAU/seat figure found (app-store stats, earnings disclosures, surveys). WebDev Arena Elo is leaderboard participation, not adoption.

---

## 4. VERIFICATION LEDGER

| Claim | Status |
|---|---|
| 3 Flashes in 6 weeks; 3.8 Flash $0.75/$3.75 intro, doubling 2027-01-01 | VERIFIED (vendor, multi-source) |
| 3.7 Flash: DeepSWE 65.3% / FrontierCode 43.6% / WebDev 1588 Elo | VERIFIED as vendor-published (vendor-run) |
| 3.8 Flash: DeepSWE 73.7% / TB2.1 89.4% | SECONDARY (CodingFleet) — VERIFY vs model card |
| Antigravity agent: $0.25–$5/interaction, 3–5M tokens, 50–70% cached | VERIFIED (official docs) |
| ChatForest table (83.6 / 76.2 / 69.2 vs 76.4 / 61% halluc.) | Exists as published; internally inconsistent, provenance unclear — treat cautiously |
| Teamwork: 7 open problems, Lean-verified Knuth proof, RISC-V 0.71% | VERIFIED as claims; Lean/arXiv/merge artifacts VERIFY before amplifying; TCSBench is Google-internal |
| No independent systematic Antigravity-agent eval | CONFIRMED (nuance: AA tests the SDK harness independently) |
| "645-reply quota thread 137702" | CORRECTION: 137702 cites the 645-reply thread; is itself the 16k-token-cap analysis. Sept 429 threads 180907/181274 VERIFIED |
| Continuity Engine 139335 | VERIFIED (proxy metrics self-admitted) |
| Model-label mismatch (Claude→Gemini swap) | UNVERIFIED; officially denied; real gap = no per-response model transparency |
| AirAsia >50% | VERIFIED as vendor claim; wording drift "production-ready" vs "production QA" |
| "Multi-instance ≠ multi-agent" (Hyperdev) | VERIFIED quote; partially dated by Aug 2026 Teamwork |
| "Millions of developers" | VENDOR-ONLY, no independent figure |