# Iteration 11 — Agent C: Qwen Code (QwenLM/qwen-code) — Community + Benchmarks + Model-Harness Co-Design

Date: 2026-09-11. All URLs fetched/verified this session unless flagged `UNVERIFIED`.
Repo state: `QwenLM/qwen-code` README (main, fetched 2026-09-11) — https://github.com/QwenLM/qwen-code

---

## 1. COMMUNITY

### 1.1 Hacker News

**Launch reception (Jul 2025).** HN item 44653806 is a top-level *comment* (flakiness, 2025-07-22) on the Qwen3-Coder launch story (item 44653072, "Qwen3-Coder: Agentic coding in the world[...]", **765 points**, submitted 2025-07-22; verified via Algolia item API). The comment: "The qwen-code app seems to be a gemini-cli fork... I hope these OSS CC clones converge at some point" + quotes the official line "we are also open-sourcing a command-line tool for agentic coding: Qwen Code. Forked from Gemini Code".
- https://hn.algolia.com/api/v1/items/44653806 ; https://news.ycombinator.com/item?id=44653072

Fork-reception sub-thread (all 2025-07-22/23, story 44653072):
- Skepticism of fork proliferation: "I hope these OSS CC clones converge" (flakiness).
- **Counter-argument that became the co-design thesis** (chartered_stack, id 44657025): "the point of custom CLIs is that each model is trained to handle tool calls differently... tool call performance is wildly different... Convergence is meaningful only when the models and their performance are commoditized."
- Platform view: "kudos to Gemini CLI team for making it open source... easily tunable to new models like Qwen... Wouldn require people to fork" (mkagenius, 44656612); real-hacker (44656877): "Just one open-source CC, with a configurable base_url/apikey."
- Cynicism about big-co OSS shipping "in poor condition... then the company gets the most credit and not all the contributors" (nicce, 44657031).
- Local-model question "Can you run qwen-code locally?" (gabeyaw, 44658902) — unanswered in-thread.

**Local-model thread (Sep 2025).** Item 45208933 (paool, 2025-09-11, comment on story 45186015): "Can you use your Qwen instance in CLIs like Claude code, codex...?" Answer (evilduck, 45214843): "opencode.ai and QwenLM/qwen-code both allow you to configure any API as the LLM provider. That said, running agentic workloads on local LLMs will be a short and losing battle against context size..." — recurring theme: Qwen Code praised as *the* OSS CLI for BYO-endpoint, doubted for local-model long-horizon work.
- https://hn.algolia.com/api/v1/items/45208933

**2026-era HN threads (verified via Algolia search_by_date, created_at_i > 2026-01-01):**
- **"Tell HN: Qwen Free Tier Is Discontinued"** (dhruv_ahuja, 2026-04-16, 9 pts, 4 comments, id 47789014): 401 "token expired" errors; CLI now prints: "Qwen OAuth free tier was discontinued on 2026-04-15. Run /auth to switch to Coding Plan, OpenRouter, Fireworks AI, or another provider." "It is official: Qwen free tier is dead. It was great while it lasted, especially with the latest 3.6 plus model." https://news.ycombinator.com/item?id=47789014
- **"Qwen OAuth Free tier will be discontinued on 2026-04-15"** (angst, 2026-04-15, id 47775172) — links teardown PR https://github.com/QwenLM/qwen-code/pull/3207
- **"Show HN: Qwen3.8-Max - Use Qwen Studio and MCP to Code Locally for Free"** (tohid4n, 2026-08-09, id 49228173, 3 pts): quality datapoint — "slower than Codex and Claude Code, especially in thinking mode... In fast mode, it gets noticeably closer... I would place it closer to the next Opus 4.7 level. These are just my impressions... not a benchmark." https://news.ycombinator.com/item?id=49228173
- "Qwen3.8-Max reproduce a research paper" (dyfang, 2026-08-06, id 49201103, URL = the qwen-code repo) — Qwen using its own harness for a demo (dogfooding).
- Ecosystem tools listing Qwen Code as first-class: AionUi (2026-01-13, id 46601652), Agentic Metric (2026-03-09, id 47308185), Ecotokens (2026-04-02, id 47617015).
- Sentiment datapoint (jwitthuhn, 2026-04-20, comment on "Qwen3.6-Max-Preview" story 47834565): "I have been using qwen-code (the software...) which is a fork of gemini-cli and the tool use with Qwen models at least has been great."

### 1.2 Reddit
- r/Qwen_AI thread on the 2026-04-13 quota cut (2026-04-13, surfaced via aggregator): "Qwen - Free tier policy update: daily quota adjusted to 100 requests/day (from 1,000). Free tier will be discontinued on 2026-04-15" — direct URL UNVERIFIED (only seen quoted in https://unifuncs.com/s/6ktTIKeJ and CSDN aggregation).
- r/LocalLLaMA-type guidance recurs in blogs: Qwen Code recommended as the CLI to pair with local/Ollama/vLLM endpoints (evilduck HN above; devopstales below).

### 1.3 CN-side signal (verified)
- IT之家/新浪财经 (2025-08-09): "阿里云通义千问 Qwen Code 宣布每日可免费运行2000次" — 2,000/day free, no token cap; mainland users 2,000/day via Qwen OAuth, overseas 1,000/day via OpenRouter. https://finance.sina.com.cn (quoted) + https://news.aibase.com/zh/news/20388
- 知乎 AI早报 2026-04-14 (zhuanlan.zhihu.com, quoted via aggregator): reports 1,000 to 100/day cut and 2026-04-15 shutdown, recommends OpenRouter/Fireworks/Bailian migration. Direct zhihu URL UNVERIFIED (secondhand).
- CSDN AI编程社区 long-form Qwen Code vs Claude Code piece: users report hallucinations / poor instruction-following on real-world repos; "部分模型可能针对基准测试进行了过度优化" (suspicion of benchmark over-optimization). https://aicoding.csdn.net/688ae6baa6db534ba2bd6366.html
- HK press on Coding Plan pricing (AASTOCKS/Yahoo HK, Apr 2026): Lite first month 7.9 CNY; Pro 50 USD/mo, CN Pro 200 CNY/mo, new-user Pro first month 39.9 CNY; "适配QwenCode、ClaudeCode及Cline等AI工具". Quoted via https://unifuncs.com/s/6ktTIKeJ — UNVERIFIED primary.
- WeChat/Junyu personal signal: none surfaced publicly (UNVERIFIED/absent).
### 1.4 Blog comparisons (praise/complaints ledger)
- **getbind.co / Bind AI (2025-08-23)**, "Qwen Code CLI vs Gemini CLI": "Choose Qwen Code CLI if you want a free, generous quota... 2,000 requests/day for individuals"; "parser-level changes targeting Qwen code models"; "tight coupling to Qwen-Coder behavior (fewer lost-in-translation moments)"; speed/stability "model/provider dependent"; editor/MCP story behind Gemini. https://blog.getbind.co/qwen-code-cli-vs-gemini-cli-which-one-is-better/
- **LogRocket (2025-08-26)**, hands-on Qwen3-Coder CLI: "Qwen3-Coder isn outperforming Claude Code yet, but it is close. The biggest draw is that Qwen is far more cost-effective." 256K ctx expandable to 1M via YaRN; 7.5T training tokens. https://blog.logrocket.com/qwen-3-coder-agentic-cli/
- **SpaceSpider (2026-04-06)**, 90-day 3-CLI grid (Claude/Codex/Qwen): Qwen = "the dark horse. Surprisingly strong on mechanical tasks, cheap, and the only one... you can run against a local or self-hosted endpoint without much fuss. Weakest on judgment." "It struggles with refactor-this-module-to-be-cleaner... literal-minded... If you say run-the-tests, it will run npm test even if your repo uses pnpm... Its diff quality is the roughest of the three." Cost: Qwen ~0.1-0.2x Opus. "Qwen benefits enormously from a good QWEN.md... Without one, it does not explore well." https://www.spacespider.app/blog/claude-code-vs-codex-vs-qwen
- **aicoolies (2026-05-31)**: "Claude Code wins this comparison for most production engineering teams... Qwen Code is not a weak candidate; it is a strategic challenger"; notes "Qwen OAuth free access ended on April 15, 2026; use Alibaba ModelStudio, a provider API key, or a self-hosted endpoint." https://aicoolies.com/comparisons/qwen-code-vs-claude-code
- **DevOpsTales (2026-03-18)**: 4-CLI matrix; Qwen "context 32K-128K", git integration "manual", "best value" — NUMBERS/CLAIMS SMELL AI-GENERATED (timing table to 1s precision; context numbers contradict current docs). Treat as low-quality UNVERIFIED. https://devopstales.github.io/ai/ai-coding-cli-comparison/
- **setmyclaw (2026-02-19)**: claims Qwen-Code "runs locally on M1+ Mac, no API keys" — conflates the CLI with local models; low-reliability UNVERIFIED. https://setmyclaw.com/blog/ai-coding-agents-comparison-2026
- **wenshao/codeagents CN comparison doc** (the very doc the README cites for its "feature parity" claim): Qwen Code = TypeScript/Apache-2.0, 6+ providers, Arena multi-agent, yolo_mode, loop detection, 6-language UI; cons: no enterprise managed-settings, plugins community-only, "部分代码仍带上游痕迹" (upstream gemini-cli residue), model access now needs Coding Plan/BYOK. https://github.com/wenshao/codeagents/blob/main/docs/comparison/qwen-vs-claude-code.md

### 1.5 Praise/complaint synthesis
**Praise:** free/BYOK openness (OSS client + any endpoint); quota generosity (2025 era); speed on mechanical tasks; tool-use quality with Qwen models specifically; Windows support now real (official PowerShell installer in README); fast release cadence; i18n incl. CN.
**Complaints:** weak abstract judgment/diff quality vs Claude Code (SpaceSpider, aicoolies, CSDN); model coupling — best behavior only with Qwen models (getbind "tight coupling"; chartered_stack HN); stability/401 token-expiry breakage at OAuth sunset (HN 47789014, issue #3281); Windows historically weak (Qoder release notes even cite "+50% Windows terminal-command accuracy" for its custom model, implying Windows was a known pain point repo-wide); memory/context hunger when driven by small local models (HN 45208933).

---

## 2. BENCHMARKS

### 2.1 README harness table — VERIFIED with full context
**Original (Jul 2025) README** (verified at commit 5d2a678cb267eb874b30026a50122b352e22ff86, pre-2025-08-10):

### Terminal-Bench Performance
| Agent     | Model              | Accuracy |
| --------- | ------------------ | -------- |
| Qwen Code | Qwen3-Coder-480A35 | 37.5%    |
| Qwen Code | Qwen3-Coder-30BA3B | 31.3%    |

Self-reported; harness details (turns, timeouts, Terminus vs Qwen Code) never given in README -> weak provenance. Permalink: https://github.com/QwenLM/qwen-code/blob/5d2a678cb267eb874b30026a50122b352e22ff86/README.md

**Current README (2026-09-11)** has REPLACED it with a "Qwen Code Evaluation" section: SWE-bench_Verified (500 cases), 3 trials/version (1,500 jobs), 7 Qwen Code versions, model **Qwen 3.7 Max**, temp=1.0 top_p=0.95, interleaved/adaptive thinking high effort, 1M ctx, max_iterations=500, 7,200s runtime timeout. Avg scores 76.40-77.80% across versions 0.0.14 to 0.22.0 (pass^3 65.8-69.2%). Self-reported; harness now exhaustively specified — a direct answer to the harness-attribution critique. https://github.com/QwenLM/qwen-code#qwen-code-evaluation

### 2.2 Model-level claims
- **Qwen3-Coder-Next (arXiv 2603.00729, submitted 2026-02-28)** — https://arxiv.org/abs/2603.00729 . 80B-A3B, Qwen3-Next base (hybrid Gated DeltaNet + Gated Attention MoE, 48 layers, 512 experts/10 act/1 shared), 262,144-token native context, agentic mid-training + RL, ~800K synthesized verifiable tasks, 370 languages. Paper tables (verified from HTML): **Table 5, Terminal-Bench 2.0, Qwen3-Coder-Next 80A3 = 34.2 / 36.2 / 30.9 / 25.8** across harness settings ("dashes indicate we could not reliably obtain test results"); Table 4 SWE-bench Multilingual/Pro 80A3 = 62.8/56.2/64.3/42.7/38.7. **TASK CLAIM CHECK: "SWE-bench V ~70.6-73.4 + TB2.0 51.5" is NOT what the paper shows for Coder-Next; those exact numbers (73.4 SWE-bench Verified, 51.5 TB2.0) belong to the Qwen3.6-35B-A3B card. Coder-Next SWE-bench Verified 70.6 claim UNVERIFIED.**
- **Qwen3.6-35B-A3B HF card (2026-04-22; blog "Agentic Coding Power, Now Open to All" 2026-04-14)**: SWE-bench Verified **73.4** (internal bash+file-edit scaffold, 200K ctx), SWE-bench Pro 49.5 ("problematic tasks corrected" — their refined set), **Terminal-Bench 2.0 = 51.5** (Harbor/Terminus-2, 3h timeout, avg of 5 runs), SkillsBench via OpenCode, NL2Repo via Claude Code. https://huggingface.co/Qwen/Qwen3.6-35B-A3B ; https://qwen.ai/blog?id=qwen3.6-35b-a3b
- **Qwen3.8-Max (2026-08-02/03)**: 2.4T MoE, first open-weight Max-class (weights the week after). **Terminal-Bench 2.1 = 86.6 — VERIFIED as a *vendor self-report*, evaluated with the *Claude Code harness* (avg@10, 5h timeout, max_tokens=131,072)**; beats Opus4.8/Fable5 (84.6), behind GPT5.6 Sol (88.8) in their table; SWE-bench Pro 67.7; PaperBench 93.0. https://qwen.ai/blog?id=qwen3.8 ; mirror https://www.alibabacloud.com/blog/603421 ; MarkTechPost recap https://www.marktechpost.com/2026/08/03/alibaba-qwen-releases-qwen3-8-max/ . TheModelGap grades Qwen3.8-Max 86.6 as "self-reported, not plotted vs independent": https://themodelgap.com/benchmarks/terminal-bench-2-1
### 2.3 Independent harness-level evals of Qwen Code itself
- **Official Terminal-Bench 2.0 leaderboard has NO "Qwen Code" agent row.** Closest independent Qwen-model rows (tbench.ai/leaderboard/terminal-bench/2.0): Dakou Agent (iflow) + Qwen3-Coder-480B 27.2% (2025-12-28); OpenHands + 480B 25.4%; **Terminus 2 + 480B 23.9%** (2025-11-01); little-coder + Qwen3.6-35B-A3B 24.6%/23.0% (2026-05-14). All far below the README 37.5% claim and the card 51.5% -> vendor/harness delta of +10-25 pts vs official Terminus-2 numbers. **UNRESOLVED discrepancy** (settings/effort/hacks attribution unclear). https://www.tbench.ai/leaderboard/terminal-bench/2.0
- **"The Scaffold Effect in Coding Agents" (arXiv 2607.22585)**: independent controlled study, Qwen 3.6 Plus + MiniMax M2.5 x (Goose, OpenCode, OpenHands-SDK) on 50-task Terminal-Bench Pro subset: pass-rate harness effect only 0-8 pp (mostly within noise), but **tokens-per-solved-task varies up to 40x** by harness; failure fingerprints (REASON for Goose, VERIFY/MAX_TURNS for OpenHands, TIME/idle-loop for OpenCode) replicate across models. Recommends harness-model pair as the unit of evaluation. Qwen Code itself not among the three harnesses — gap our loop can fill. https://arxiv.org/html/2607.22585v1
- TB2.1 official board (tbench.ai): top = Claude Code+Fable5 83.8%, Terminus2+Fable5 80.4%, Claude Code+Opus4.8 78.9%; official TB (v3-era) board Sep 2026: Codex+GPT-6 Astra 58.2%, Claude Code+Fable 5.1 57.9% — no Qwen harness rows. https://www.tbench.ai/leaderboard/terminal-bench/2.1 ; https://www.tbench.ai/

---

## 3. MODEL-HARNESS CO-DESIGN

- **Custom tool-call parser shipped in serving stacks.** QwenLM/Qwen3-Coder repo: "Qwen3-Coder function calling relies on our new tool parser in both **SGLang** and **vLLM**... We updated both the special tokens and their corresponding token ids." https://github.com/QwenLM/Qwen3-Coder . Coder-Next card: SGLang flag --tool-call-parser qwen3_coder (sglang>=0.5.8); vLLM --enable-auto-tool-choice --tool-call-parser qwen3_coder (vllm>=0.15.0). https://huggingface.co/Qwen/Qwen3-Coder-Next ; SGLang cookbook https://docs.sglang.io/cookbook/autoregressive/Qwen/Qwen3-Coder-Next
- **The qwen3_coder format itself is a harness-motivated design.** Paper sec 4.2.2: "different CLI/IDE scaffolds (e.g., Cline, Qoder, OpenCode...) adopt distinct tool-calling schemas"; JSON "introduces heavy escaping overhead for multi-line code", so they designed an XML-style format qwen3_coder "designed for string-heavy arguments"; Figure 5 shows SWE-bench Verified rising monotonically with the number of distinct tool chat templates used in training (format-diversity -> format-robustness). arXiv 2603.00729.
- **Framework-and-model ship together (explicit claims).** Current README: "The framework and the Qwen models are open-source. **They evolve together.** No vendor lock-in"; "Qwen Code is actively iterating on itself — using its own agent and models to file issues, submit PRs..." Free-tier shutdown post doubles down: "we will focus on **tighter integration between models and code-agent**" (pomelo-nwu, issue #3203, EN+CN). HF Coder-Next card: "adaptability to various scaffold templates enables seamless integration with... Claude Code, Qwen Code, Qoder, Kilo, Trae, Cline."
- **Coder-Next trained ON harness trajectories — including Qwen Code.** Paper sec 3.1.2: multi-turn agentic data generated with SWE-agent, Mini-SWE-agent, OpenHands, Claude-Code, **Qwen-Code**, Terminus; teacher = Qwen3-Coder-480B-A35B-Instruct. **Figure 3 finding: within-scaffold performance scales with mid-training tokens, but cross-scaffold transfer is limited** ("models trained on trajectories from one scaffold do not transfer strongly to others") — the strongest published evidence that harness-specific training data is a moat, and that harness comparisons measure partly-trained-in coupling.
- **Hybrid attention 256K**: Coder-Next = 48-layer hybrid (12 x [3x(Gated DeltaNet->MoE) -> 1x(Gated Attention->MoE)]), 262,144-token native context, 3B active — positioned for cheap local agent serving. arXiv 2603.00729 + HF card.
- **API quirks harnesses must handle (qwen3-coder flash/plus + successors):**
  - Qwen3-Coder models are **non-thinking only**: "does not generate think-blocks... specifying enable_thinking=False is no longer required" (HF card / Qwen3-Coder repo).
  - Qwen Cloud thinking docs: enable_thinking is a per-request toggle (hybrid models); **when thinking+tools are on, reasoning_content must be echoed back with tool results or accuracy degrades**; streaming delivers thinking tokens before tool-call deltas; some open models **require streaming** for thinking; temperature forced to 0.6 in thinking mode; reasoning_effort/preserve_thinking on qwen3.8-flash. https://docs.qwencloud.com/developer-guides/text-generation/thinking ; https://docs.qwencloud.com/developer-guides/getting-started/latest-model
  - **Live co-design failure mode**: SGLang issue #36537 (2026-08-26): Qwen3.8-Flash-Next + thinking (reasoning_effort low/medium/xhigh) + tools + --tool-call-parser qwen3_coder -> deterministic token-ID-0 loop ("tool!!!!..."); works with thinking disabled or parser off (then raw tool_call XML lands in content instead of message.tool_calls — exactly the harness-integration hazard the paper UX-expert section targets). "Thinking-mode agent clients such as OpenCode cannot use this model tools through SGLang." https://github.com/sgl-project/sglang/issues/36537
  - Sampling contract: Coder-Next recommended temperature=1.0, top_p=0.95, top_k=40; README harness eval uses adaptive thinking + 1M ctx + 500 iterations — score depends on harness knobs as much as model.
---

## 4. COMMERCIALIZATION ARC (all primary-source verified)

| Date | Event | Source |
|---|---|---|
| 2025-07-23 (launch) | Qwen OAuth free: **2,000 req/day, 60 req/min, no token counting** (CN 2,000; overseas via OpenRouter 1,000) | getbind 2025-08-23 https://blog.getbind.co/qwen-code-cli-vs-gemini-cli-which-one-is-better/ ; aibase https://news.aibase.com/zh/news/20388 |
| 2025 (undated PR) | Cut **2,000 -> 1,000/day** "to allow more users to access Qwen Code... fairer resource distribution" | PR https://github.com/QwenLM/qwen-code/pull/1730 |
| 2026-04-02 | Qwen3.6-Plus live via OAuth (the "incredibly popular" combo) | commit 4aa70d6 news timeline |
| 2026-04-13 | **Issue #3203**: quota 1,000 -> **100/day effective immediately**; free tier sunset 2026-04-15; PR #3206 (100/day copy, 12 files, 7 locales); commit 4aa70d6 README announcement | https://github.com/QwenLM/qwen-code/issues/3203 ; https://github.com/QwenLM/qwen-code/issues/3206 ; https://github.com/QwenLM/qwen-code/commit/4aa70d6c962a7dd3389a54f4664948f6507f5621 |
| 2026-04-15 | OAuth free tier **discontinued**; PR #3207 removes OAuth entry; CLI message directs to Coding Plan / OpenRouter / Fireworks | https://github.com/QwenLM/qwen-code/pull/3207 ; HN 47775172/47789014; issue #3281 |
| post | Migration target: **Alibaba Cloud Coding Plan / ModelStudio** (Lite from 7.9 CNY first mo; Pro 50 USD/mo, CN Pro 200 CNY/mo, Pro first-mo 39.9 CNY per press) | issue #3203 README diff; press quotes via unifuncs (UNVERIFIED primary) |

**Official rationale (pomelo-nwu, issue #3203, EN+CN):** "the qwen-code + qwen3.6-plus combination has been incredibly popular... costs have become too much for **our small team** to sustain... qwen-code was born from the community (forked from gemini-cli)... we will focus on tighter integration between models and code-agent."

**Backlash ledger (issue #3203 comments + HN):** "Reduce-daily-free-quota but killing OAuth... what a BS?"; students/"future paying customers" pleas; training-data argument (free tier = cheap RL data); "does a corporation as massive as Alibaba truly lack the budget... clears the path for your user base to migrate to Google [Gemini CLI 1,000/day free]"; **sharpest complaint: the paid replacement was unbuyable** — 50 USD Coding Plan "constantly out of stock", "Insufficient resources" on click, "Temporary out of stock... Is this some kind of joke?"; Decrypt: "Free Qwen Is Dead" (URL via aggregator, UNVERIFIED direct); auth-then-fail UX bug (issue #3281: OAuth "authenticated successfully" then quota-exceeded on first prompt).

**Iteration-relevant read:** the free tier was the distribution engine for Qwen Code as a harness; killing it (with a broken paid funnel) transferred goodwill to OpenCode/OpenRouter and made "harness + free model quota" bundles (Gemini CLI) the switching default.

---

## 5. QODER vs QWEN CODE — two harness philosophies inside Alibaba (verified)

- **Qoder (global, qoder.com)**: closed commercial "Agentic Platform" — Qoder IDE ("Autonomous Development IDE"), Quest spec-driven delegation, QoderWake "AI employees", **Qoder CLI** ("terminal-native AI coding partner — and an agent engine you can build on"), Cloud Agents. First shipped **2025-08-21**; Qoder 1.0 GA **2026-06-16** ("from AI IDE to Autonomous Development Desktop"; "Agent Harness rebuilt from the ground up — every task is natively a bounded, traceable execution unit"; 60+ releases in 9 months, "building Qoder with Qoder"). https://qoder.com/en ; https://www.alibabacloud.com/blog/introducing-qoder-1-0-from-ai-ide-to-autonomous-development-desktop_603260 ; https://www.alibabacloud.com/blog/introducing-qoder-agentic-coding-platform-for-real-software_603349
- **Qwen-Coder-Qoder custom model (2026-02-01)**: "Customizing a fast-evolving frontier model for real software" — a Qwen-Coder derivative RL-tuned specifically to the Qoder agent framework/tools/scenarios; Qoder CN release notes: task success "surpasses Cursor Composer-1", "**50% higher terminal-command accuracy on Windows**" (their internal benchmarks; self-reported). https://qoder.com/en/blog/qwen-coder-qoder ; https://help.aliyun.com/en/lingma/product-overview/qoder-cn-update-log
- **Lingma -> Qoder CN rebrand 2026-05-20** (v1.0.0/v3.0.0): TONGYI Lingma renamed "Qoder CN"; credits-based billing; Lingma IDE -> Qoder CN IDE/Desktop; VS Code extension deprecated; Quest becomes standalone window; Experts Mode (Planning/Research/Coding/Review/Testing); new models GLM-5.1, Kimi-K2.6 alongside. https://help.aliyun.com/en/lingma/product-overview/introduction-of-lingma ; same release-notes URL. Account split: Alibaba-Cloud-account line vs qoder.com.cn "Full Suite" line; unified upgrade path from 2026-06-20. https://help.aliyun.com/en/lingma/faq ; https://www.alibabacloud.com/help/en/lingma/qoder-cn-qoder-cn
- **Two harness philosophies**: (a) **Qwen Code** = open-source Apache-2.0 terminal harness, multi-protocol (OpenAI/Anthropic/Gemini/Qwen + local), BYOK, "framework and models evolve together" — the community/open-weights flywheel; (b) **Qoder** = closed, spec-driven "agentic engineering" desktop where the model is privately fine-tuned to *one* harness (Qwen-Coder-Qoder) — the harness-trained-model moat taken to its logical end. Cross-linkage: Qwen3.8-Max is evaluated on an in-house "**QwenQoderBench**" (Qwen blog note 11) and the Coder-Next card lists Qoder among target scaffolds; the Coder-Next paper cites Qoder as a distinct tool-calling schema it trains robustness for. Note Qoder CLI competes *directly* with Qwen Code in the terminal — Alibaba ships two terminal harnesses.

---

## 6. Open threads for the loop
1. Vendor-vs-official TB2.0 delta for Qwen pairs (51.5 card vs 24.6 little-coder; 37.5 old README vs 23.9 Terminus-2+480B) — worth a replication slice.
2. "Scaffold Effect" paper lacks Qwen Code as a harness arm — natural extension target.
3. Qwen3.8-Max TB2.1 86.6 is Claude-Code-harness avg@10 self-report; official-board gap unmeasured.
4. Qoder CLI vs Qwen Code head-to-head: no public benchmark found; only Qoder internal claims.
5. Unverified items flagged above: r/Qwen_AI direct URL, Zhihu direct URLs, Decrypt direct URL, Coding Plan press pricing, Coder-Next "SWE-bench V 70.6".