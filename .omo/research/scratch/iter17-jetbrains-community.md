# Iteration 17 — Agent B: JetBrains Junie/AI — Economics + Community + Archaeology
Researched: 2026-09-12 (all URLs visited live this session unless flagged UNVERIFIED)

---

## 1. AI CREDITS ECONOMICS

### 1.1 Tier structure (VERIFIED — official pricing + docs)
Source: https://www.jetbrains.com/ai-ides/buy/ and https://www.jetbrains.com/help/ai-assistant/licensing-and-subscriptions.html (docs "Last modified: 22 July 2026")

| Tier | Price (individual) | AI Credits / 30 days | Top-ups |
|---|---|---|---|
| AI Free | $0 | **3** | No |
| AI Pro | $10/mo — $100/yr | **10** | Yes |
| AI Ultimate | $30/mo — $300/yr | **35** | Yes |
| AI Trial | Free | 10 per trial period | Yes |
| AI Enterprise (org-only) | $60/user | "on par or higher" than Ultimate — exact number undisclosed ("still rethinking the AI quota model") | Not yet |

- **Organizations (per-user) pay double / get double**: AI Pro $20/mo -> 20 credits; AI Ultimate $60/mo -> 70 credits. (Docs tab "Organizations (per-user)". Explains a July 2026 complaint mentioning "included 20 credits" on AI Pro.)
- **JetBrains Central (org pool)**: All Products Pack contributes 20 credits/mo per license; dotUltimate 20; AI Free 3. Pool accumulates monthly; top-ups expire 12 months after entering pool.
- **Unit economics: 1 AI Credit = $1 USD** (charged in local currency). "The number of AI Credits included in your plan corresponds to its subscription price, with a small bonus for AI Ultimate."
  - Annual AI Ultimate = 420 credits for $300 -> ~40% bonus (official FAQ). Monthly Ultimate ~ 15% bonus.
- **Top-ups**: any amount, valid 12 months, consumed only after monthly quota exhausts; NOT available in Mainland China.
- **Free tier availability**: IDEs >=2025.1 only; not Android Studio, not free Community editions (PyCharm CE / IntelliJ CE), not via License Server; serviceable territories only.

### 1.2 Credit burn per task type (VERIFIED — official "Usage per AI Credit")
Source: https://www.jetbrains.com/help/ai-assistant/licensing-and-subscriptions.html#usage-per-ai-credit
One AI Credit approximately equals:
- ~10 AI Chat code-generation requests
- ~40 editor inline code-generation requests
- ~25 "explain this code" requests
- ~280 documentation requests
- ~140 commit-message generations

JetBrains support confirms: "a single credit cannot be mapped to a fixed number of tokens or calls"; quota computed dynamically per request from tokens in/out, model choice, context size (files, MCP data, chat history), and number of internal agent calls. Junie and Claude-based agents "consume noticeably more than a single AI Chat message." (Support thread, June 2026 replies — see 1.4.)

### 1.3 Real-world burn rates reported by users (VERIFIED quotes)
- **Ultimate (35 credits)**: OP Rdspromo, Nov 18 2025: 28/35 -> 10/35 in "a few hours of normal use spread over a couple of days." (Thread 31142845556370)
- **Jun 21 2026, Mjassani (AI Ultimate)**: "asked Claude Agent to perform a task that I could have done myself in about 3 minutes... It consumed 2 credits, i.e. roughly $2 of my monthly quota for a trivial task." -> ~35 trivial tasks/month burns the plan.
- **Jun 26 2026, Shayne O'Neill**: "I purchased 15 credits, and it used them in two questions... Nearly $11 AUD per question. No code generation used, and fairly minimal code inspection."
- **Jul 14 2026, Dmb Subscription**: "My monthly quota was consumed within 30 minutes... USD 10 burned in minutes" (AI Pro). Canceled.
- **Jul 20 2026, Ferdinando Villa (AI Pro)**: Codex-medium writing 18 lines consumed his last 2 credits; same task via ChatGPT app "used only a tiny fraction of my weekly allotment."
- **Free tier (3 credits)**: Reddit r/Jetbrains 1owu3zr: "3 credits is gone in a couple minutes with any model worth a damn"; a JetBrains support reply concedes the 3-credit quota exhausting "after just a few days of occasional use" is "expected."
- **Model-dependence**: staff repeatedly name **Gemini 3 Flash as most cost-efficient** (Mar + Jun 2026 replies); "GPT-5 is often cheaper than Claude 4 while giving comparable results"; "Claude is completely unusable except for short questions" (Reddit 1u38d76). Mar 2026 commenter: even Gemini 3 Flash "still burns credits very quickly."
- **Agentic multiplier**: agentic workflows (Junie / Claude agent) make "several internal model calls" per task, so trivial tasks cost multiples of a chat message. Sep 1 2026 commenter (Simone): AI Chat consumes "at least 2x" Junie for same tasks (UNVERIFIED impression); also alleges **silent model switching after IDE updates** changing consumption.

### 1.4 The canonical complaint thread + JetBrains responses (VERIFIED)
Thread: "Serious Concerns Regarding JetBrains AI Pricing & Credit Consumption with Junie and AI Assistant"
https://intellij-support.jetbrains.com/hc/en-us/community/posts/31142845556370-Serious-Concerns-Regarding-JetBrains-AI-Pricing-Credit-Consumption-with-Junie-and-AI-Assistant
- Created Nov 18, 2025 by "Rdspromo" (12 votes, 25+ comments, active through Sep 1, 2026). OP canceled license O331N8V4TJ; cites GitHub Copilot unlimited $39/mo as alternative; "People are canceling. Developers are leaving."
- **JetBrains replies (verbatim anchors)**:
  - Nov 22, 2025 (Leonardo Sancho): "In hindsight, we should have communicated these changes better" + FAQ link + "ways to reduce AI quota consumption."
  - Mar 2 / Apr 21 / Jun 4 / Jun 26 / Jul 15+22+24, 2026: repeated "open a ticket with logs" boilerplate (SUPPORT-A-76 chat dump, SUPPORT-A-75 logs). OP (Jul 25, 2026): "You have been investigating this since November and nothing has been done!!!"
  - Jun 26, 2026 (Sajal Mandal), fullest official explanation: since the **August 2025 update** "AI credits reflect the real per-token cost of each request"; credits "directly tied to their own API costs"; suggests Gemini Flash for routine work, GPT-5 over Claude, new chats per topic, trimming thresholds; Full Line completion runs on-device and is free.
  - JetBrains links its own transparency feature requests: **LLM-16701** (per-request cost tracking) and **LLM-19959** (detailed quota usage report) — i.e., as of mid-2026 the promised usage dashboard still did not exist. https://youtrack.jetbrains.com/issue/LLM-19959
  - KB: SUPPORT-A-1981 (control Junie quota), SUPPORT-A-2448 (high quota causes), SUPPORT-A-2539 (why cloud quota consumed faster than before), SUPPORT-A-2446 ("Why Plans Changed on August 25, 2025").

### 1.5 The August 25, 2025 quota reset — origin event (VERIFIED)
- Announcement: "A Simpler, More Transparent Model for AI Quotas" — https://blog.jetbrains.com/ai/2025/08/a-simpler-more-transparent-model-for-ai-quotas/ (rollout "Starting August 25, 2025"; launch postponed once per https://x.com/jetbrains/status/1955907817497395490 — UNVERIFIED original date).
- Damage-control FAQ (Sep 16, 2025): https://blog.jetbrains.com/ai/2025/09/faq-new-ai-quota/ — full text captured. Key admissions:
  - "**We hear you. And you are right – the new quotas are a reduction from what you experienced.** When we launched, we had little usage data for agentic experiences and no top-up feature existed, so we set quotas much higher than the real cost of AI usage... That felt great, but it loses money at scale... **We set expectations incorrectly. That is on us.**"
  - "There is no scam here. We are aligning usage to real, public provider prices per token... some companies still burn VC money to attract users (also that is changing). We are a real business; we cannot play that game."
  - "about 80% of users (plan and usage dependent) will not hit the new limits" — "the most active AI users are the most affected."
  - Junie framed as premium: "Junie... perform[s] like agents in more expensive plans (up to $200/month), but we aim to keep the cost of usage reasonable."
  - Refunds: "special alternatives for annual AI Pro and AI Ultimate users"; none for All Products Pack / dotUltimate (AI Pro bundled free).
- Community reaction repost: r/Jetbrains "We promised to be more transparent about JetBrains AI..." https://www.reddit.com/r/Jetbrains/comments/1mp890e/ (Aug 2025).

### 1.6 The cost dial: "plan on strong model, implement on cheap" (VERIFIED — JetBrains framing)
- GA post (Jun 17, 2026): "Junie supports any model, without lock-in – and that is how you control cost... **Cost efficiency stops being a property of the tool and becomes a dial you hold.**" https://blog.jetbrains.com/junie/2026/06/junie-coding-agent-out-of-beta/
- Same post, Plan mode: "**Plan on a strong model; implement on a cheap one.** The agent does not wander, so your bill stays low."
- Junie Local post (Aug 24, 2026): "Cost efficiency has been a dial you hold in Junie for a while... **Junie Local turns that dial down to zero.**" https://blog.jetbrains.com/junie/2026/08/junie-local-launch/
- Bypass routes (VERIFIED): BYOK (Anthropic/OpenAI/Google); local runtimes LiteLLM / LM Studio / Ollama; Junie Local (free, on-device); custom local models "not affected by your quota"; local Full Line completion uses no cloud credits.

### 1.7 Cost-per-task benchmarks (cross-check)
- AltAI review: Junie at **$1.14/problem, "roughly 4x cheaper than the next-best competitor at $4.91"** — https://altaitools.com/jetbrains-junie-review/ (UNVERIFIED third-party aggregation).
- tech-insider.org citing the public SWE-Rebench leaderboard: "Junie Agent" = **61.8% (±0.54) resolved, 73.9% pass@5, $0.81/problem, 1,684,501 tokens/problem, 85.9% cache-hit rate** — https://tech-insider.org/jetbrains-junie-vs-github-copilot-vs-cursor-2026/ (UNVERIFIED vs live https://swe-rebench.com — VERIFY before citing).
- Implication: at $0.81-$1.14/task, a full Ultimate quota (~35 credits = $35) buys ~30-43 agentic tasks — matching user reports of quotas lasting days-to-weeks.

---

## 2. BENCHMARK CLAIMS

### 2.1 The #1 SWE-Rebench claim (VERIFIED)
- GA post, Jun 17, 2026 (Dmitry Savelev): "On the latest run of SWE-Rebench – an independent agent benchmark – **Junie placed as the number-one coding agent**," with pull-quote:
  > "SWE-Rebench draws fresh tasks each cycle to keep the evaluation honest, so results move from run to run. In this cycle Junie came out as the top model-harness, with **61.6% resolved and a 72.7% pass@5** — placing it ahead of the other agents and competitive with raw frontier models"
  > — **Alexander Golubev, Research Lead at Nebius**
  URL: https://blog.jetbrains.com/junie/2026/06/junie-coding-agent-out-of-beta/
- Blog RSS archive confirms identical text in feed: https://blog.jetbrains.com/junie/tag/news/feed/

### 2.2 Methodology of SWE-Rebench (VERIFIED — primary sources)
- About page: https://swe-rebench.com/about (full text captured)
- Nebius announcement: https://nebius.com/blog/posts/swe-rebench-dataset (dataset open-sourced, >21,000 tasks)
- Paper: "SWE-rebench: An Automated Pipeline for Task Collection..." https://arxiv.org/pdf/2505.20411 ; V2 (language-agnostic): https://openreview.net/pdf?id=UCAda9kS57
- Methodology facts:
  1. **Continuously mines fresh GitHub issues -> PR pairs** from active repos to defeat training-set contamination (SWE-bench static since late 2023).
  2. **Decontamination by date**: issue/PR creation dates tracked against model release dates; potentially contaminated evals explicitly flagged on the leaderboard.
  3. **Centralized evaluation**: Nebius runs everything with one fixed minimal ReAct scaffolding, identical prompts, default hyperparameters, 128K context (text-based and tool-based modes; full system prompts published).
  4. **Stochasticity control**: each model/harness run **5 times**; reports **SEM and pass@5** — discourages best-of-N cherry-picking.
  5. Contact: alex_golubev@nebius.com (matches the quoted "Alexander Golubev, Research Lead at Nebius").
  6. Disclosed funding: **API credits from Google, OpenAI, Anthropic, MiniMax, StepFun**; infra by TractoAI.

### 2.3 Independence + durability assessment (analysis)
- **Independence: genuine but with caveats.** Nebius is a separate company; the eval is centralized, decontaminated, statistically careful — stronger than self-reported SWE-bench Verified numbers. Caveats: (a) JetBrains chose which "model-harness" to submit and ran it in its own agent loop; the blog does NOT disclose which underlying model(s) powered the #1 run (UNVERIFIED — likely Gemini/Claude mix per default-model history); (b) Nebius publicly solicits "proprietary model" vendors to discuss "providing credits or access for evaluation" (about page) — a vendor-funding channel; (c) "top model-harness" != best model: it is model+Junie scaffolding, exactly what JetBrains sells.
- **Durability: explicitly non-durable.** JetBrains' own pull-quote concedes "results move from run to run." Quoted 61.6%/72.7% vs leaderboard 61.8%/73.9%/$0.81 already shows cycle drift. Independent read (mcp.directory): "Treat 61.6%/72.7% as a strong snapshot, not a permanent ranking — check the live swe-rebench.com leaderboard." https://mcp.directory/blog/junie-jetbrains-coding-agent-2026
- **Retro-baseline**: "At launch in January 2025, Junie's 53.6% score was below the top 10 of SWE-bench Verified" — https://altaitools.com/jetbrains-junie-review/ (UNVERIFIED; plausible; VERIFY against archived Junie pages).

### 2.4 "From Leaderboards to Model Profiles" — methodology counter-post (VERIFIED, full text captured)
URL: https://blog.jetbrains.com/junie/2026/08/from-leaderboards-to-profiles/ — published 2026-08-31; authors Dmitry Savelev (Junie), Felix Plantenberg (ML intern), Marco Damonte (ML Scientist).
- Core claim: resolve rate hides a "benchmark meaning gap"; two models with identical resolve rates are not interchangeable.
- Pipeline: 4 perspectives — **Outcome / Efficiency (tokens, calls, cost, seconds) / Patch quality (containment, complexity) / Process quality (explore->implement->validate trajectory)**; deterministic metrics + LLM judges over issue+patch+trajectory+bounded repo context; failure-stage taxonomy (localization / implementation / validation).
- Data: Claude Opus 4.7 vs Gemini 3.5 Flash across 4 datasets, 523 tasks: Opus 51.1% vs Gemini 48.6%; identical outcomes on 430 tasks (214 both-solve, 216 both-fail); only 93 tasks separated them.
- Divergence findings:
  - Opus: stronger diagnosis; "In **123 runs** Opus performed no executable validation, including **68 runs that still resolved the task**"; skipped validation in 23.6% of runs; moderate/severe hallucination 24.7%.
  - Gemini 3.5 Flash: stronger validation discipline; but **37.3% of runs moderate/severe hallucination**; 15.3% significant/severe patch redundancy (vs Opus 6.5%); showcased run: first production edit at step 88 vs Opus finishing in 53 steps.
  - Wider set (522 shared tasks): **GPT-5.5 highest resolve 51.5%**, only model that always ran an executable check; **Qwen 3.6 27B FP8 resolved 38.9% at 3% of GPT-5.5's cost per run**; Opus led every patch-quality metric with shortest trajectories; GPT-5.5 and Opus within a cent per run.
  - Private-benchmark anecdote: Opus 4.7 = 184 steps, $2.79/run vs Gemini Flash = 271 steps, $1.24/run for identical solve counts.
- Limitations (their own): profiles are "specific [to the] Junie scaffold used in this evaluation"; LLM judges are "diagnostic signals rather than ground truth"; golden-patch bias acknowledged.
- Significance: 8 weeks after headlining a leaderboard rank, JetBrains publishes an argument that resolve rate is the wrong decision variable — simultaneously justifying model routing ("choose the right model for the job"), i.e., the dial-based pricing.

### 2.5 Other eval disclosures (VERIFIED as claims; content UNVERIFIED, single-source)
- **JetBrains private test set**: "real, recent commits in our own projects that no model has been trained on." Used to justify (a) Gemini 3.7 Flash default: "matched the solve rate of our premium Sonnet-5 midtier model, at roughly a third of the cost per task" (https://blog.jetbrains.com/junie/2026/08/junie-gemini-3-7-flash/, Aug 17, 2026); (b) Junie Local: "Qwen3.6-27B scored on par with Sonnet 4.5 (10,000-token reasoning limit). GPT-5 at medium effort scored slightly higher" — with disclosed caveat that the local model ran **reasoning-disabled** vs cloud models with it on (https://blog.jetbrains.com/junie/2026/08/junie-local-launch/). Sets are NOT public.
- Companion deep-dive: "How We Optimized the Qwen 3.6 Model for Our Junie Agent" — https://blog.jetbrains.com/junie/2026/08/qwen-for-junie/ (KV-cache reuse across tasks; speculative decoding ~2x generation; M5 8-bit arithmetic -> ~40% prefill throughput, patch to be PR'd to MLX-VLM).
- Related: "We Gave Agents IDE-Native Search Tools. They Got Faster and Cheaper." — https://blog.jetbrains.com/ai/2026/05/what-happens-when-you-give-agents-ide-native-seach-tools/ (May 4, 2026).

---

## 3. COMMUNITY SENTIMENT 2026

### 3.1 Junie vs Claude Code / Codex debates (VERIFIED Reddit threads)
- "Why would we choose Junie AI over Codex or Claude Code?" — https://www.reddit.com/r/Jetbrains/comments/1owu3zr/
  - Pro: "Junie imo is better that claude/codex. But you do need to actively switch your model based on task so that it does not wope out your credits."
  - Con: Spring backend: "I can add about 6 or 7 [CRUD features] a month with tests before I run out of tokens... way slower than claude code."
  - Free tier: 3 credits ~ "2-3 hours of intensive work for complex task" (optimistic) vs "gone in a couple minutes."
- "which is the best IA agent in jetbrains?" — https://www.reddit.com/r/Jetbrains/comments/1pdo9f8/ (34 comments)
  - "I bought yearly ultimate and that has been a total waste of cash... everything I ever asked it to do, it fucked up, even simple tasks, and it quits early."
  - Counter: "Junie + Opus is awesome if you know how to prompt. Only problem is cost transparency... how many tokens you actually used per request; what cut jetbrains might be taking."
- "Is Junie worth it?" — https://www.reddit.com/r/Jetbrains/comments/1ssjq88/ (11 votes, 48 comments)
  - "Codex happens to be the best in term of price and code quality. Junie has no 5 hours reset and its monthly credits can run out in a week."
  - MCP note: "Previously you had to set the server in Junie. With 2026.1, you set MCP servers in AI Assistant – and there is now a switch to just have the MCP server in Jetbrains available."
- "JetBrains AI (Chat & Junie): My feedback after 2 months" — https://www.reddit.com/r/Jetbrains/comments/1u38d76/
  - "even my 70 per month are not close to enough" (org Ultimate); "Claude is completely unusable except for short questions"; "a half-finished solution"; "credits drain incredibly fast. Much faster than you would experience with Claude Code or Codex."
  - Economics insight: "the $200 sub you get at Anthropic gives you ~$8k worth of inference if we measure by API costs" — competitors subsidize; JetBrains resells near-API rates.
  - Positive: "0.4 credits... gets me a complete code review of a major intranet application" (Gemini Flash reviews are cheap).
- "Junie alternative after credit change?" — https://www.reddit.com/r/Jetbrains/comments/1nv2id4/ — "Just burned 3 credits for having Junie to implement a simple HTTP Client"; replies pivot to Claude Code in JetBrains IDEs / Windsurf plugin.
- "Jetbrains and AI – My hopes for a successful 2026 and beyond" — https://www.reddit.com/r/Jetbrains/comments/1q1lnhq/ (86 votes, 20 comments)
  - "Junie is not very good and uses credits in a very unpredictable manner."
  - Counter-trend: "Junie was not good until last winter... especially after the Gemini 3 Flash came out at the end of the year, things improved significantly, and Junie is now part of my workflow."
- "First time using Junie, and I am surprised" — https://www.reddit.com/r/Jetbrains/comments/1q22z3m/ — quality parity with Claude acknowledged but preference for Claude on credits; "totally buffed [baffled]... just before the change in the monetization, she got way worse."
- "Claude Code vs Junie?" (r/ClaudeAI) — https://www.reddit.com/r/ClaudeAI/comments/1kxo3gv/ (19 votes) — "Junie is more agentic and the workflow is quite nice, but Claude Code is just amazing."
- "Cluade vs Jetbrains AI value" — https://www.reddit.com/r/Jetbrains/comments/1m94y3l/ — split verdicts: "AI Ultimate it is just okay, at least for me" vs "blew through the limit in an afternoon."

**Sentiment synthesis**: IDE-native integration universally praised; **credit opacity + burn is the #1 complaint** across every thread and the support forum (Nov 2025 -> Sep 2026, unbroken); Claude models priced out of reach ("unusable"), Gemini Flash the community-endorsed cheap default; dominant exit paths: Claude Code CLI, ChatGPT/Codex auth, Copilot plugin, Windsurf plugin. Recurring accusation: JetBrains resells tokens at ~API prices while competitors subsidize (staff answer concedes 1 credit = $1 tied to their API costs).

### 3.2 Junie Local reception (Aug 24, 2026 launch)
- Announcement: https://blog.jetbrains.com/junie/2026/08/junie-local-launch/ — free, `/local` one-command install, Qwen3.6-27B 4-bit (~20 GB), **requires Apple M5 + 64 GB RAM**; no quota/tokens/cloud; offline-capable; "Plan mode, live prompting, guidelines, skills, and your /commands behave the same way." Chose Qwen3.6 over 3.8 (3.8 needs reasoning enabled -> ~4x slower); prototypes for DGX Spark + RTX 5090.
- JetBrains pre-concedes: "We know that an M5 Mac with 64 GB of RAM is a big ask... it puts Junie Local out of reach for many people"; "looking at 24 GB cards."
- Community context (r/LocalLLM, r/LocalLLaMA, Aug 2026): dense 27B on Apple Silicon ~10-30 tok/s; 64 GB "super tight" with Qwen 27B-class; users confirm "Apple specifically optimized m5 for prefill/prompt processing" — corroborates JetBrains' prefill-first story AND skepticism on hardware floor:
  - https://www.reddit.com/r/LocalLLM/comments/1w4n0ri/ ; https://www.reddit.com/r/LocalLLaMA/comments/1vpyr1d/ ; https://www.reddit.com/r/LocalLLM/comments/1vqpbd6/
- No large dedicated Junie-Local reception thread surfaced this session (UNVERIFIED reception volume; treat as niche-positive with hardware gatekeeping).

### 3.3 Air preview reception
- What it is: "JetBrains Air — an Agentic Development Environment... delegate coding tasks to multiple AI agents and run them concurrently" — https://air.dev/ ; docs: https://www.jetbrains.com/help/air/quick-start-with-air.html
- Public preview: Mar 2026 — https://blog.jetbrains.com/air/2026/03/air-launches-as-public-preview-a-new-wave-of-dev-tooling-built-on-26-years-of-experience/ ; Windows Jun 2026 — https://blog.jetbrains.com/air/2026/06/jetbrains-air-lands-on-windows/
- Agent-agnostic via ACP: "Air now supports GitHub Copilot, OpenCode, Pi, Cline, and other ACP agents" — https://blog.jetbrains.com/air/
- Reception: r/Jetbrains "Thoughts on JetBrains new agentic IDE, Air" — https://www.reddit.com/r/Jetbrains/comments/1s2hs19/ — "I consider myself somewhat a fan of JetBrains products, but **this IDE is way below my expectations. The UI has been altered without an apparent [reason]**" (direct fetch bot-blocked; snippet via search index — VERIFY full thread in browser).

---

## 4. BLOG / CHANGELOG ARCHAEOLOGY 2025-2026

### 4.1 Timeline (all VERIFIED via blog/RSS this session unless noted)
| Date | Event | URL |
|---|---|---|
| Jan 2025 | **Junie announced** — closed preview / EAP, "Meet Junie, Your Coding Agent" | https://blog.jetbrains.com/junie/2025/01/meet-junie-your-coding-agent-by-jetbrains/ ; InfoQ: https://www.infoq.com/news/2025/01/jetbrains-junie-agent/ |
| Apr 2025 (IDE 2025.1) | **AI Free tier introduced** (unlimited local completion + small cloud quota) | secondary: https://skywork.ai/skypage/en/jetbrains-ai-assistant-review/2032070189540384768 (UNVERIFIED; consistent with docs "AI Free... starting from version 2025.1") |
| Aug 13->25, 2025 | **Quota model overhaul**: announced, postponed once, live Aug 25 — 1 credit = $1, big quota cuts, top-ups introduced | https://blog.jetbrains.com/ai/2025/08/a-simpler-more-transparent-model-for-ai-quotas/ ; https://x.com/jetbrains/status/1955907817497395490 |
| Sep 16, 2025 | FAQ admission post ("That is on us") | https://blog.jetbrains.com/ai/2025/09/faq-new-ai-quota/ |
| ~Oct 2025 | **JetBrains adopts ACP** (Zed's Agent Client Protocol) — "Every JetBrains IDE will support any ACP-compatible agent" | https://www.linkedin.com/posts/zeddotdev_acp-brings-jetbrains-on-board-... (date UNVERIFIED — activity ID ~Oct 2025) |
| Dec 8, 2025 | **Junie merged into AI Chat (Beta)** — explicit plan: "eventually deprecate and fully merge the separate Junie interface into the AI chat" | https://blog.jetbrains.com/ai/2025/12/junie-now-integrated-into-the-ai-chat/ |
| Jan 2026 | **ACP Agent Registry live** (with Zed) for IDEs 2025.3+; AI Assistant plugin ships IDE-aligned version numbers ("2025.3"; ComponentSource release Jan 21, 2026) | https://blog.jetbrains.com/ai/2026/01/acp-agent-registry/ ; https://www.componentsource.com/product/jetbrains-ai-assistant/releases ; YouTrack LLM-22543 references "2025.3.1" |
| Mar 2026 | **Junie CLI beta** ("the LLM-agnostic coding agent"); **Air public preview** | https://blog.jetbrains.com/junie/2026/03/junie-cli-the-llm-agnostic-coding-agent-is-now-in-beta/ ; https://blog.jetbrains.com/air/2026/03/air-launches-as-public-preview-a-new-wave-of-dev-tooling-built-on-26-years-of-experience/ |
| Apr 8, 2026 | **Junie CLI <-> IDE connection (Beta)** — auto-detects running IDE; semantic index, test runners, refactors | https://blog.jetbrains.com/junie/2026/04/junie-cli-inside-your-jb-ide/ |
| Jun 1, 2026 | **Mellum2 open-sourced** ("A Fast Model for AI Workflows") | https://blog.jetbrains.com/ai/2026/06/mellum2-goes-open-source-a-fast-model-for-ai-workflows/ |
| **Jun 17, 2026** | **Junie GA (leaves Beta after ~17 months)** — SWE-Rebench #1 claim; Advanced Plan mode (.junie/plans); agentic debugging (real IDE debugger, incl. decompiled/JAR sources); remote control; PR code review (GH Actions / GitLab on-prem, /review); **IDE integration rebuilt on ACP**; BYOK; local runtimes | https://blog.jetbrains.com/junie/2026/06/junie-coding-agent-out-of-beta/ |
| Aug 17, 2026 | **Default model swap -> Gemini 3.7 Flash at 40% off** (limited time); private-set claim: matches "Sonnet-5 midtier" at ~1/3 cost | https://blog.jetbrains.com/junie/2026/08/junie-gemini-3-7-flash/ |
| Aug 24, 2026 | **Junie Local launch** (free, on-device, Qwen3.6-27B, M5/64GB floor) | https://blog.jetbrains.com/junie/2026/08/junie-local-launch/ |
| Aug 31, 2026 | "From Leaderboards to Model Profiles" methodology post | https://blog.jetbrains.com/junie/2026/08/from-leaderboards-to-profiles/ |
| Aug 2026 | "How to Use AI Agents in IntelliJ IDEA With ACP" (agent-agnostic pivot continues) | https://blog.jetbrains.com/idea/2026/08/how-to-use-ai-agents-in-intellij-idea-with-acp/ |

### 4.2 Reversals / renames / deprecations (the juicy arcs)
1. **Beta->GA arc: 17 months.** Closed preview Jan 2025 -> GA Jun 17, 2026. GA post pre-empts cynicism: "This is not a rename or a repackage."
2. **Quota reversal (Aug-Sep 2025).** Pre-Aug 2025: generous usage, percentage-bar meter, no top-ups. Post: $1-per-credit metering with quotas *reduced* — JetBrains admits old quotas were "set much higher than the real cost... as a temporary measure" and "loses money at scale." Refund/credit accommodations for annual subscribers. Longest-running community wound; thread active Sep 1, 2026.
3. **Junie standalone UI deprecation.** Dec 8, 2025: two-UIs experiment conceded ("industry now converging on clearer UX standards"), Junie absorbed into AI Chat agent selector; transition keeps partially separate settings; "hardcore Junie fans" redirected to Discord. Corroborated by Sep 1, 2026 support comment referring to "Junie (which will be removed soon)" + complaint AI Chat consumes ~2x Junie credits for same tasks (UNVERIFIED impression).
4. **AI Assistant versioning shift.** Historically IDE-locked plugin builds ("JetBrains AI Assistant 2024.1 Updates", Apr 2024); by 2025.3 the plugin/YouTrack/version listings use IDE-aligned version numbers (2025.3, 2025.3.1, 2026.1, 2026.2; help docs "2026.2"). Effect: AI features release-train with quarterly IDEs, not independent plugin cadence. (VERIFIED via ComponentSource + LLM-22543 + docs footer; intermediate 2025-era versioning details UNVERIFIED.)
5. **Model churn & silent defaults.** Default model -> Gemini 3.7 Flash (Aug 17, 2026, replacing 3.6) with 40%-off promo; referenced default/premium models across 2025-26: Claude 3.7 -> 4.x -> Sonnet 4.5 -> "Sonnet-5 midtier"; GPT-4o era -> GPT-5 -> GPT-5.5; Gemini 2.5 -> 3 Flash -> 3.7 Flash; in-house Mellum (OSS as Mellum2, Jun 2026); Qwen 3.6 tuning (Aug 2026). User allegation of silent model switching post-update (support thread Sep 1, 2026). Explicit model-deprecation announcements not found in sampled feeds (UNVERIFIED — likely in YouTrack/help release notes).
6. **MCP evolution timeline.** 2025: Junie per-project MCP server config (`.junie/` settings; Action Allowlist). Dec 2025 post lists "MCP server" among Junie settings being migrated. 2026.1: MCP servers configured centrally in AI Assistant settings + switch exposing the IDE itself as an MCP server (Reddit 1ssjq88 comment — UNVERIFIED vs release notes). Jan 2026: ACP Agent Registry becomes the agent-distribution layer alongside MCP wiring.
7. **ACP timeline.** Zed-origin protocol -> JetBrains adoption (~Oct 2025) -> ACP Agent Registry in IDEs 2025.3+ (Jan 2026) -> Junie CLI<->IDE beta via ACP (Apr 8, 2026) -> GA integration "rebuilt on top of ACP... same protocol Junie CLI uses to talk to your IDE" (Jun 17, 2026) -> Air launched agent-agnostic on ACP (Mar-Jun 2026) -> user-facing how-to (Aug 2026). Strategic meaning: JetBrains IDEs reposition from "our AI" to a client/host for ANY ACP agent (Claude Code included).

### 4.3 AI-blog post-type census (Jun 2026 feed window)
Research-flavored marketing on the rise: AlphaEvolve for IDE algorithms (May 29, 2026), IDE-native search for agents (May 4, 2026), "The IDE Is Already an AI Quality Variable" (Apr 30, 2026), "Our 2026 Direction: AI and Classic Workflows" (Apr 28, 2026), Skill Manager/Skill Repository (Apr 28, 2026), Koog 1.0 (May 27, 2026), Mellum2 OSS (Jun 1, 2026), Codex hackathon coverage (Apr-May 2026). Feed: https://blog.jetbrains.com/ai/feed/

---

## 5. FIVE JUICIEST FINDINGS

1. **JetBrains admitted the Aug 25, 2025 quota reset was a correction of its own loss-leader — then spent a year deflecting the fallout.** The Sep 16, 2025 FAQ verbatim: "the new quotas are a reduction from what you experienced... We set expectations incorrectly. That is on us"; "about 80% of users will not hit the new limits." Yet the canonical thread (31142845556370, Nov 18, 2025 -> Sep 1, 2026) shows the same "open a ticket with logs" boilerplate ten months later, while JetBrains' own transparency feature requests (LLM-16701, LLM-19959) remain open — the usage dashboard promised "in a few weeks" in Sep 2025 still absent mid-2026. https://blog.jetbrains.com/ai/2025/09/faq-new-ai-quota/ + https://intellij-support.jetbrains.com/hc/en-us/community/posts/31142845556370
2. **The "#1 on SWE-Rebench" headline is engineered to be true and disclaimed in the same sentence.** JetBrains quotes Nebius' Alexander Golubev granting "top model-harness, 61.6% resolved, 72.7% pass@5" — and immediately notes "results move from run to run." The public leaderboard already shows a different cycle (61.8% ±0.54, 73.9% pass@5, $0.81/problem, 1.68M tokens/problem, 85.9% cache-hit). Nebius methodology is genuinely strong (fresh mined issues, date-based decontamination flags, fixed ReAct scaffold, 5 runs, SEM) — but JetBrains chose the harness it submitted and never disclosed which model powered the winning run. https://blog.jetbrains.com/junie/2026/06/junie-coding-agent-out-of-beta/ + https://swe-rebench.com/about
3. **Eight weeks after headlining a leaderboard, JetBrains published a post arguing leaderboards are the wrong metric.** "From Leaderboards to Model Profiles" (Aug 31, 2026): equal-resolve-rate models diverge wildly — Opus 4.7 skipped executable validation in 123/523 runs (68 of them "resolved"); Gemini 3.5 Flash hallucinated moderately/severely in 37.3% of runs; GPT-5.5 hit 51.5% resolve with perfect validation discipline; Qwen 3.6 27B FP8 got 38.9% at 3% of GPT-5.5's cost. Read cynically: it retroactively reframes the #1 claim AND doubles as justification for the model-routing "cost dial" that monetizes per-model credit burn. https://blog.jetbrains.com/junie/2026/08/from-leaderboards-to-profiles/
4. **The credit math makes Junie a metered API reseller while every direct competitor subsidizes.** 1 credit = $1; Ultimate annual = 420 credits/$300 (~40% bonus). Documented burn: 2 credits for a 3-minute trivial task; 15 purchased credits gone in 2 questions; full Pro quota in 30 minutes; benchmark-level cost $0.81-$1.14/task => ~30-43 tasks/month on Ultimate. Users counterpose Copilot unlimited at $39/mo and the observation that a $200 Claude Max sub ~= $8k of API-rate inference. Staff answer concedes credits track "their own API costs" — no subsidy, hence the exodus narrative in every 2026 thread. https://www.jetbrains.com/help/ai-assistant/licensing-and-subscriptions.html + threads in section 3.1
5. **The product strategy quietly inverted: from "our agent, Junie" to "any agent in our IDE."** Dec 8, 2025: Junie's standalone UI slated for deprecation into AI Chat. ~Oct 2025: JetBrains adopts Zed's ACP; Jan 2026: ACP Agent Registry ships in-IDE; Jun 2026: Junie itself rebuilt on ACP and GA'd; Mar 2026: Air launches as an agent-agnostic "Agentic Development Environment" hosting Copilot/OpenCode/Pi/Cline. The IDE becomes the tollbooth, not the agent — a tacit community-read admission Junie cannot win on quality alone (86-vote Reddit thread: "Junie is not very good and uses credits in a very unpredictable manner"). Meanwhile Junie Local (free, on-device, zero-credit) is the new loss-leader the old quotas used to be. https://blog.jetbrains.com/ai/2025/12/junie-now-integrated-into-the-ai-chat/ + https://blog.jetbrains.com/ai/2026/01/acp-agent-registry/ + https://blog.jetbrains.com/air/2026/03/air-launches-as-public-preview-a-new-wave-of-dev-tooling-built-on-26-years-of-experience/

---

## VERIFICATION LEDGER
- VERIFIED (primary, fetched this session): licensing docs (2026-07-22 rev), FAQ Sep 2025, GA post (Jun 17, 2026), Junie Local post (RSS full text), Gemini 3.7 Flash post (RSS full text), From-Leaderboards post (full text), swe-rebench.com/about (full text), support thread 31142845556370 (full text incl. 25 comments Nov 2025-Sep 2026), AI-blog feed, Junie news feed, Junie-in-AI-Chat post (RSS full text), ACP registry post (title/date via search), Reddit snippets (via search index).
- UNVERIFIED (single-source or secondary; flag before external citation): $1.14/$4.91 cost-per-problem comparison (AltAI); leaderboard 61.8%/73.9%/$0.81/1.68M-token numbers (tech-insider citing live board); Jan 2025 53.6% SWE-bench Verified figure (AltAI); Air Reddit thread full sentiment (fetch bot-blocked); "AI Free introduced with 2025.1" framing (secondary source); exact ACP-adoption announcement date (~Oct 2025); org-tier "20 credits on AI Pro" universal applicability (docs confirm per-user org pricing; commenter context unknown); silent-model-switch allegation (user report); MCP-settings unification in 2026.1 (Reddit comment, not checked against release notes); Mellum2/Qwen post contents beyond RSS summaries.
