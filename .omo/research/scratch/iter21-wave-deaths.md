# Iteration 21 — Agent B: Wave Deaths (HKUDS/AutoAgent) + Skill-Learning Straggler Tier

Date: 2026-09-12 · Method: GitHub REST API verification + git archaeology (fresh clone of HKUDS/AutoAgent @ `16c12b052ef2330a198063c62a07a7f9723031e3`) + press triangulation (TechCrunch/exa). Every repo meta line below was pulled live on 2026-09-12.

## Verdict table

| Target | Stars (verified) | Last push | Verdict |
|---|---|---|---|
| HKUDS/AutoAgent | 9,783 | 2025-10-16 (~11mo dormant) | AVOID (dependency); dead progenitor of the wave |
| PrimeIntellect-ai/prime-agent | 20,568 | 2026-09-12 (today) | **ADOPT** — reference runtime self-improving harness |
| AMAP-ML/SkillClaw | 2,593 | 2026-08-17 | **ADOPT** — cross-agent collective skill library |
| sentient-agi/EvoSkill | 1,181 | 2026-08-24 | **ADOPT** — offline benchmark-driven skill evolution |
| aiming-lab/SkillRL | 970 | 2026-05-17 (~4mo stale) | WATCH — training-time RL co-evolution, different layer |
| zjunlp/SkillX | 284 | 2026-09-03 (active) | WATCH — strong method, artifact completeness unverified |
| YeQing17-2026/OmniAgent | 2,564 | 2026-07-27 | **AVOID — UNVERIFIED provenance, flag** |
| NousResearch/hermes-agent | 244,645 | 2026-09-12 | Ledger anchor; now a $1.5B-valuation company asset |

---

## 1. AUTOGENT AUTOPSY: HKUDS/AutoAgent

### 1.1 Verified vitals
- https://github.com/HKUDS/AutoAgent — 9,783★, 1,362 forks, MIT, **not archived**. Created 2025-02-06. Last push 2025-10-16 → ~11 months dormant as of 2026-09-12. (gh api repos/HKUDS/AutoAgent)
- Identity: launched as **MetaChain** (arXiv 2502.05957, released 2025-02-10), renamed **AutoAgent v0.2.0** on 2025-02-17. Clone history confirms the lineage: `6418259 2025-02-13 Merge branch 'main' of https://github.com/HKUDS/MetaChain`.
- README framing ([permalink](https://github.com/HKUDS/AutoAgent/blob/16c12b052ef2330a198063c62a07a7f9723031e3/README.md#L30-L47)): "a **Fully-Automated** and highly **Self-Developing** framework that enables users to create and deploy LLM agents through **Natural Language Alone**", with bullets for "Intelligent Resource Orchestration … through iterative self-improvement" and "Self-Play Agent Customization", plus a GAIA-benchmark leaderboard badge ("matches Deep Research using Claude 3.5 rather than OpenAI's o3").

### 1.2 Git archaeology (frozen repo, depth-50 clone)
- **Last functional code change: 2025-02-28** — `106c869 fix gemini error` is the last commit touching `*.py`/`*.sh` (verified via `git log -- "*.py" "*.sh"`).
- All 20 commits after 2025-02-28 (Mar → Oct 2025) are README edits, WeChat/Feishu QR-code uploads, and "Add files via upload" (slides, Communication.md). Final commit `16c12b0 2025-10-16 Update Communication.md`.
- Implication: **functional development life ≈ 3 weeks** (Feb 6–28, 2025), then ~8 months of community-management husk, then total silence. Codebase: 99 Python files; meta-agent layer under `autoagent/agents/meta_agent/` + tool-CRUD in `autoagent/tools/meta/`.

### 1.3 What its "self-improvement" actually did
Evidence ([edit_tools.py L67-68](https://github.com/HKUDS/AutoAgent/blob/16c12b052ef2330a198063c62a07a7f9723031e3/autoagent/tools/meta/edit_tools.py#L67-L68)):

```python
@register_tool("create_tool")
def create_tool(tool_name: str, tool_code: str, context_variables):
```

The meta-agent (`tool_editor.py`, `agent_former.py`, `workflow_former.py`, `agent_creator.py`) writes Python tool source as a string via `create_tool`, generates agent profiles from a natural-language requirement, and assembles workflows. **Classification: developer-time self-construction, not runtime learning.** There is no persistent memory, no trajectory→skill distillation, no post-task refinement, no skill deduplication or verification — nothing survives a session as learned state. Versus the 2026 tier (Prime Agent's `/refine`, SkillClaw's evolve server, EvoSkill's benchmark loop), AutoAgent's "self-improving" framing was **authoring automation oversold as self-improvement**. The wave's founding marketing claim and its substance must be kept distinct in our ledger.

### 1.4 Why it died
- **No explicit death**: no archive, no deprecation notice, no successor statement found in repo or README (UNVERIFIED that any statement exists anywhere).
- **HKUDS academic-lab release-farm model (INFERENCE)**: the org ships a new repo per paper/theme and reallocates attention. Verified org leaderboard 2026-09-12 (top 12 by stars, with last push): CLI-Anything 49.3k★ (08-21), **nanobot 48.0k★ (pushed 2026-09-11)**, LightRAG 39.6k★ (09-12), DeepTutor 39.3k★ (09-10), Vibe-Trading 33.3k★, RAG-Anything 23.3k★, AI-Trader 22.3k★, DeepCode 16.5k★, **OpenHarness 15.7k★ (06-04)**, ViMax 12.3k★, **AutoAgent 9.8k★ (2025-10-16 — oldest push in the tier)**, ClawWork 8.5k★. AutoAgent is the only dormant repo in the org's top tier.
- **LightRAG gravity**: the lab's durable, maintained line is RAG/knowledge (LightRAG, RAG-Anything, MiniRAG, DeepTutor) — multi-year maintenance exists there; agent-line effort moved to nanobot (created 2026-02-01).
- **Is nanobot the successor? INFERENCE — UNVERIFIED.** No explicit successor statement found. Circumstantial: nanobot occupies the same personal-agent lane; its May-2026 roadmap ([discussion #431](https://github.com/HKUDS/nanobot/discussions/431)) reframes as "open-source personal agent companion" with skills-as-context, memory, plugin SDK — absorbing AutoAgent's ambitions at production quality; HKUDS also fielded OpenHarness for the harness lane. If an explicit AutoAgent→nanobot handoff statement exists, we did not find it.
- **Death signature**: hype spike (9.8k★ in 3 weeks, trendshift badge) → 3 weeks of code → 8 months of README maintenance → abandonment without deprecation. Org attention reallocation, not community rejection.

---

## 2. STRAGGLER TIER (skill-learning), API-verified 2026-09-12

### 2.1 AMAP-ML/SkillClaw — 2,593★
Verified: created 2026-04-10; pushed 2026-08-17 (~4wk); MIT; 259 forks; 39 open issues; org = Alibaba Amap ML (also MetaClaw, LongHorizon-Harness); paper [arXiv:2604.08377](https://arxiv.org/abs/2604.08377). HF claim: "#2 Paper of the Day" on HF Daily Papers 2026-04-11 — the task brief said "HF Paper-of-the-Day"; it was **#2** that day (nuance noted). https://github.com/AMAP-ML/SkillClaw
Mechanism: a local API proxy intercepts `/v1/chat/completions` + `/v1/messages` for any agent (Hermes, Codex, Claude Code, OpenClaw, QwenPaw, IronClaw, PicoClaw, ZeroClaw, NanoClaw, NemoClaw), captures session artifacts; an optional Evolve Server (workflow engine Summarize→Aggregate→Execute, or OpenClaw-agent engine) distills/dedups/evolves `SKILL.md` skills in shared storage (local/OSS/S3), with versioned validation workers and a dashboard.
**Verdict**: The unique asset is **collective, cross-agent, cross-user skill pooling** with publish/validation gates — nothing else in the tier does team-level evolution, and the proxy-level integration makes it harness-agnostic. Moderate cadence. ADOPT as an integrable layer for our stack.

### 2.2 PrimeIntellect-ai/prime-agent — 20,568★
Verified: repo created 2026-05-08; launched 2026-08-05 ([blog](https://www.primeintellect.ai/blog/prime-agent), [arXiv:2608.23552](https://arxiv.org/abs/2608.23552) — Princeton/MIT/Prime Intellect); MIT; **pushed 2026-09-12 (today)**; 2,248 forks. Continual Harness concept paper: [arXiv:2605.09998](https://arxiv.org/abs/2605.09998). https://github.com/PrimeIntellect-ai/prime-agent
Mechanism: one tool — a persistent IPython kernel (RLM: context as a variable; sub-agents as function calls returning at admission, async via `agent_message.send`). **Continual Harness** formalizes harness state H = (ρ, G, K, M) — prompt notes, sub-agent specs, skills, memory — as typed CRUD state persisted to disk across sessions. `/refine` applies the smallest evidence-backed edit (records trigger + outcome, versioned, rollback by ID, base system prompt immutable; planning async at turn boundaries). Benchmarks: ARC-AGI-3 RHAE Best@1 30%→95.5% (Opus 5; human expert baseline 95.4%); 85.5-hour autonomous nanoGPT run; matches/exceeds Pi, Claude Code, Codex and outperforms **Hermes Agent**, OpenCode, Kimi-Code on other harness benchmarks (their paper's claim).
**Verdict**: **The most significant artifact in this tier and the reference implementation of runtime harness self-improvement.** Credible known org (verifiers/PRIME-RL/INTELLECT lineage). Caveats: only 1 month since open-source launch; RLM paradigm favors models comfortable computing in-REPL; paper admits no model is yet trained around it ("model-harness co-learning" is the upside). ADOPT as primary study + integration candidate; it benchmarks directly against hermes-agent.

### 2.3 sentient-agi/EvoSkill — 1,181★
Verified: created 2026-03-04; pushed 2026-08-24; Apache-2.0; Sentient Labs; paper [arXiv:2603.02766](https://arxiv.org/abs/2603.02766). https://github.com/sentient-agi/EvoSkill
Mechanism: offline benchmark-driven evolution loop: base agent runs tasks → proposer analyzes failures → generator writes skill/prompt mutations → evaluator scores on held-out data → frontier keeps top-N programs **as `program/*` git branches with `frontier/*` tags** (auditable, rollback-friendly). Explicitly extends GEPA from single-prompt optimization to whole-agent evolution. Supports Claude Code, OpenCode, OpenHands, Goose, Codex CLI, Harbor; skills in agentskills.io format transfer cross-agent and cross-model (arXiv:2604.01687). README honestly marks continuous evolution and benchmark-free evolution as open problems (citing NousResearch/hermes-agent-self-evolution).
**Verdict**: The best **offline lab-quality optimizer** — produces auditable skills against your own benchmarks, orthogonal to runtime loops. ADOPT for the offline optimization lane; the git-branch frontier pattern is directly stealable for our harness.

### 2.4 aiming-lab/SkillRL — 970★
Verified: created 2026-02-08; pushed 2026-05-17 (**~4mo stale**); MIT; 0 open issues; paper [arXiv:2602.08234](https://arxiv.org/abs/2602.08234). https://github.com/aiming-lab/SkillRL
Mechanism: **training-time** skill↔policy co-evolution: a SkillBank JSON (general skills / task-specific skills / common mistakes) is injected into GRPO/PPO rollouts (verl-agent); `enable_dynamic_update` mutates the bank from validation failures during RL; 7B models on ALFWorld/WebShop/Search; SFT data + checkpoints on HF. aiming-lab is the research cluster SkillClaw credits for MetaClaw. Fun fact from their News: they lost several hundred stars in an accidental unstar incident (2026-03-02) — explains star-count churn around the task brief's number.
**Verdict**: Interesting science at a **different layer** (model post-training, not harness), and currently dormant. WATCH; revisit only if we run RL post-training.

### 2.5 zjunlp/SkillX — 284★
Verified: created 2026-01-09; pushed 2026-09-03 (**active**); MIT; 27 forks; Zhejiang University NLP lab w/ Ant Group acknowledgement; paper [arXiv:2604.04804](https://arxiv.org/abs/2604.04804). https://github.com/zjunlp/SkillX
Mechanism: auto-constructs a **three-tier skill knowledge base** from trajectories — Planning / Functional / Atomic skills — via extract → merge/filter/update → exploratory expansion; plug-and-play strong-to-weak transfer; ~10-pt gains on AppWorld, BFCL-v3, τ2-Bench; beats A-Mem/AWM/ExpeL baselines.
Caveat: the README still carries a "**Planned Release**" section for the codebase — how much is actually usable despite recent commits is UNVERIFIED; adoption is tiny (284★/27 forks).
**Verdict**: Strong method, weak artifact so far. WATCH — but steal the taxonomy: the planning/functional/atomic layering is a good schema for our own skill store.

### 2.6 YeQing17-2026/OmniAgent — 2,564★ ⚠️ FLAGGED
Verified: GitHub API license = **NOASSERTION** (README badge claims GPL-3.0 — mismatch); created 2026-04-16; pushed 2026-07-27; 385 forks; 39 open issues. https://github.com/YeQing17-2026/OmniAgent
Provenance red flags (verified via users API): **the account was created 2026-04-16T13:55Z and the repo 9 minutes later (14:04Z) the same day**; the account has exactly 1 public repo (this one), 301 followers, no bio, no org. 2,564★ in under 5 months on a single-repo day-zero account. Self-described "inspired by OpenClaw"; comparative marketing table vs OpenClaw and Hermes; grandiose unverifiable claims ("unbypassable (industry-first)" security, "BrainModel Self-Evolution" via online GRPO+PRM, "90% token savings"); no paper, no org backing, docs "on the way".
**Verdict**: AVOID. UNVERIFIED provenance; star-velocity pattern consistent with inflation; the NOASSERTION license state is itself a hygiene problem (any OpenClaw-derivation obligations are obscured). Do not adopt; do not count it as evidence of the tier's health.

---

## 3. NOUS RESEARCH company state
- **Funding**: TechCrunch 2026-07-13 — finalizing **≥$75M at $1.5B valuation**, led by Robot Ventures with significant USV participation; not closed at report time; all parties declined comment. Prior total ~$70M, including the Paradigm-led $50M Series A (Apr 2025, ~$1B valuation). (Note: several secondary posts state the Series A was "less than three months prior" — Beckmann and RuntimeWire corroborate April 2025, ~14 months before the talks. The round remains UNVERIFIED-closed as of this writing.)
- **Product/asset**: NousResearch/hermes-agent — verified 2026-09-12: **244,645★ / 50,720 forks**, MIT, pushed today (press cited ~214k★ in July; it has kept growing). Ships built-in skills (web search, coding, image understanding) and "automatically learn[s] from people's usage and build[s] more skills without manual intervention" (TechCrunch). Monetization: hosted tiers $20–$200/mo + "Hermes Agent Enterprise" (forward-deployed-engineer job listing is the tell). Hermes Desktop (macOS/Win/Linux, MIT) shipped June 2026, demoed in Nvidia's GTC keynote.
- **Open-weights strategy**: Hermes 4.3 (36B on Seed-OSS-36B-Base, 512K context) post-trained on **Psyche**, Nous's decentralized training network (24 nodes across the open internet, Solana-coordinated consensus, DisTrO/DeMo compression; claimed ~144k tok/s ≈ centralized parity — company-reported). Thesis: open-core agent + open weights + decentralized-training narrative; monetize hosting/enterprise, give away the core.
- **Teknium arc**: co-founder (with Quesnelle, Malhotra, Mitra; founded 2023) — from community open-weights finetuner (Hermes model line) to agent-company founder; hermes-agent's star count is the valuation's core collateral.
- **Risk notes for our ledger**: no public revenue/conversion data; **CVE-2026-9350** (Hermes Agent ≤2026.4.16, Batch Runner missing authorization, public exploit available) — production-readiness caveat for any integration; Prime Agent's paper already reports beating Hermes Agent on several harness benchmarks.

---

## 4. ADOPT / AVOID / WATCH for the ledger
- **ADOPT**: **prime-agent** (reference runtime self-improving harness; Continual Harness CRUD + `/refine` evidence-backed versioned edits + immutable base prompt = the pattern to copy), **EvoSkill** (offline skill-evolution loop; git-branch `program/*` frontier is directly stealable), **SkillClaw** (cross-agent shared skill library; proxy-interception pattern integrates with harnesses we already track).
- **WATCH**: **SkillX** (planning/functional/atomic taxonomy; verify code completeness), **SkillRL** (training-time co-evolution; revisit if/when we do RL), **HKUDS nanobot + HKUDS OpenHarness** (15.7k★ harness repo pushed 2026-06-04 — new lead, not in task brief; successor-gravity watch), **NousResearch/hermes-agent-self-evolution** (Nous's benchmark-free evolution experiments, referenced by EvoSkill — bridge between the Nous and research tiers).
- **AVOID**: **HKUDS/AutoAgent** as a dependency (dormant ~11mo; code frozen 2025-02-28; authoring-time codegen, not self-improvement), **YeQing17-2026/OmniAgent** (unverified provenance, license mismatch, star-pattern suspicion).

## Sources
- https://github.com/HKUDS/AutoAgent (+ clone SHA 16c12b052ef2330a198063c62a07a7f9723031e3, git log)
- https://github.com/HKUDS/nanobot/discussions/431 · https://github.com/HKUDS/nanobot · org API orgs/HKUDS/repos
- https://github.com/AMAP-ML/SkillClaw · https://arxiv.org/abs/2604.08377 · https://huggingface.co/papers/2604.08377
- https://github.com/PrimeIntellect-ai/prime-agent · https://www.primeintellect.ai/blog/prime-agent · https://arxiv.org/abs/2608.23552 · https://arxiv.org/abs/2605.09998
- https://github.com/sentient-agi/EvoSkill · https://arxiv.org/abs/2603.02766
- https://github.com/aiming-lab/SkillRL · https://arxiv.org/abs/2602.08234
- https://github.com/zjunlp/SkillX · https://arxiv.org/abs/2604.04804
- https://github.com/YeQing17-2026/OmniAgent (+ users/YeQing17-2026 API)
- https://techcrunch.com/2026/07/13/hermes-agent-maker-nous-research-in-talks-for-new-funding-at-1-5b-valuation/ · https://beckmann.ai/en/ai-economy/2026-07/nous-research-hermes-valuation · https://runtimewire.com/article/nous-research-75-million-round-1-5-billion-valuation
- https://github.com/NousResearch/hermes-agent (API, 2026-09-12)
