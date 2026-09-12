# FINAL PHASE — Agent C: Harness×Model Interaction, Per-Model Prompt Variants, Drift/Determinism

**Phase:** FINAL (LLM-behavior elicitation) · **Date:** 2026-09-12 · **Agent:** C
**Legend:** ✅ VERIFIED (primary source read/confirmed) · ⚠️ SECONDARY (reputable intermediary) · ❌ UNVERIFIED (searched, not found / provenance unclear)

---

## 1. Harness×Model Interaction (the joint-evaluation evidence base)

### 1.1 Stop Comparing LLM Agents Without Disclosing the Harness — ✅ arXiv 2605.23950
https://arxiv.org/abs/2605.23950 (v1, 7 May 2026; Zhang, Wang, Ge, Xu, Hamm, Reddy — Tulane/Rutgers/VT)

Position paper; **Binding Constraint Thesis**: on long-horizon tasks with comparable frontier models, harness-induced variance (HV) is comparable to or larger than model-induced variance (MV). All seeded numbers confirmed in the full text:

- **Controlled factorial (verified):** 3 models (GPT-5.4, Kimi K2.6, GLM-5.1) × 3 harnesses (H1 Minimal → H2 Improved → H3 Full) on a 100-task SWE-bench Verified subset. **Aggregate HV̄/M̄V = 7.80×** (HV 18.48 pp² vs MV 2.37 pp²). Harness shift moves GLM-5.1 by 13.0pp; model swap within fixed harness moves scores only 3.0–5.0pp. **6 of 9 model-pair/harness-pair comparisons flipped ranking.**
- **SEAL vs Claude Code (verified):** Claude Opus 4.5 on SWE-bench Pro: **45.9% (SEAL) → 55.4% (Claude Code)** = +9.5pp harness-only. Under the standardized SEAL scaffold the six leading frontier models span only 4.9pp (41.0–45.9%) — the within-model harness range (9.5pp) is ~2× the within-harness model range.
- **WarpGrep flip (verified):** adding one search subagent (WarpGrep) to otherwise identical infrastructure adds +2.1–2.2pp across models — "comparable to a routine model upgrade" — and **flips the SWE-bench Pro ordering of MiniMax 2.5 vs Claude Opus 4.6**.
- **TB2 (verified):** GPT-5.4 (high) on Terminal-Bench 2: **69.7% → 77.0%** (+7.3pp) from AHE automated harness evolution alone. Also in their Table 1: TerminalBench 2.0 52.8→66.5 (+13.7pp, prompt+middleware+verification); Grok 4 SWE-agent→xAI scaffold +14–16pp on SWE-bench Verified; Vercel 15 tools→2 tools 80%→100%.
- **Corroborating observational data (cited in-paper):** HAL cross-scaffold gaps of 34pp (Claude Sonnet 4.5: 68%→34%; GPT-5 Medium: 46%→12%) and ~48pp (o4-mini) on SWE-bench Verified Mini; third-party monitoring (Epoch) reports 11–15pp scaffold-only variation, "scaffold choice has the single biggest impact."
- **Theory:** agents are closed-loop systems; the harness is the controller 𝒞_H, the model an open-loop stochastic policy π_θ. Three controller-owned reliability quantities: **stability** (Lyapunov-like non-decreasing goal progress), **context drift** (KL divergence of context embeddings from task-relevant p₀), **control lag** (steps between anomaly detection and corrective signal reaching the policy). None improves with a model upgrade.
- **Proposed evaluation framework:** (i) **Harness Card** — structured disclosure across the 7-layer **ETCSOVG** taxonomy (Execution, Tool, Context, Scheduling, Observability, Verification, Governance); (ii) **variance-decomposition protocol** — minimum valid design is a 2×2 model×harness grid with task order/env/stopping held constant, reporting HV per model, MV per harness, HV̄/M̄V, ranking-reversal count, and partial η²; (iii) two valid regimes: **locked-harness** or **factorial**. Anything else = "incomplete and potentially misleading."

### 1.2 The Scaffold Effect — ✅ paper verified (arXiv 2607.22585); ❌ Zenodo DOI unconfirmed
https://arxiv.org/abs/2607.22585 (Vats & Golev, Sentient Labs, 8 Jun 2026; also on OpenReview: https://openreview.net/forum?id=nw4d83V687)

- **40× token cost at equal pass rate (verified):** 2 models (Qwen 3.6 Plus, MiniMax M2.5) × 3 harnesses (Goose, OpenCode, OpenHands-SDK) × 50 Terminal-Bench Pro tasks = 300 trials. Pass rates 38–50%, pairwise harness deltas only **0–8pp** (most within bootstrap noise), but **tokens/solved-task: Goose ~28–37K vs OpenCode ~1.15–1.55M = 40.8–41.9×** (bootstrap-robust). Systematically replicates: the ordering Goose ≪ OpenHands ≪ OpenCode is identical for both models.
- **Failure fingerprints are harness properties (verified):** Goose → REASON-dominated (stops cleanly, zero VERIFY); OpenHands-SDK → VERIFY+MAX_TURNS (accepts plausible-but-wrong); OpenCode → TIME/HANG idle-spinning (zero VERIFY, positive verdicts trustworthy). **Each fingerprint replicates across both models** → "scaffold property rather than a model property." Each implies a different oversight discipline for a human reviewer.
- **Mechanism:** no-action (idle) turns: 2.0–2.16/task for OpenCode vs 0.2–0.3 for Goose = 10× — each idle turn re-ships the full accumulated context ("per-task wait/oversight tax"). Turn counts differ only ~1.2×, so the cost gap is per-turn context growth + tool serialization.
- **In-paper precedent:** Terminal-Bench 2.0 paper reports Opus 4.5 at 52.1% vs 57.8% under two harnesses consuming **256.9M vs 3.9M input tokens (65×)** for a 5.7pp gain.
- **Recommendation:** the unit of evaluation is the **harness–model pair**; report tokens/solved, no-action turns, and failure-category vector alongside pass rate.
- ❌ **UNVERIFIED:** the seeded Zenodo DOI `10.5281/zenodo.19819492` — record page JS-blocked; could not confirm it resolves to this paper. Cite arXiv/OpenReview instead until confirmed.

### 1.3 Harness Engineering for Predictable Agentic Systems — ✅ arXiv 2608.26197
https://arxiv.org/abs/2608.26197 (25 Aug 2026)

- Deterministic execution layer (finite-state control, forced tool selection, output validation, bounded retry, structured planning) on Qwen-2.5-7B-Instruct + Gemma-3-27B, finance+legal tasks.
- **First-pass harness = mixed result (verified):** significantly improves reproducibility in 1 of 4 model-task cells, **degrades it in 2**, no effect in 1 → constraints are not model-agnostic.
- **Diagnosis → fix:** once tool/state/output sequences are consistent, the unconstrained free-text *planning step* is the dominant residual variance source. Adding **Structured Planning** (schema-validated plan before any tool call) → 3 of 4 cells hit Reproducibility Rate/Determinism Index = 1.000 at N=100; success → 100% in 3 of 4.
- **Cost is model-dependent (verified):** token cost falls in every cell, but latency splits by model — one model gets faster under constraint, the other "markedly slower." Thesis: harness engineering's benefit "must be measured, not assumed."

### 1.4 Claw-SWE-Bench — ✅ arXiv 2606.12344 (seed's "adapter-only" claim verified)
https://arxiv.org/abs/2606.12344 · code: https://github.com/opensquilla/claw-swe-bench

- **19.1% → 73.4% Pass@1, same GLM 5.1 backbone (verified):** bare adapter (model writes unified diff in final response) has **69.1% apply-failure rate**; full adapter (model edits via tools, runner exports git patch) drops apply failures to **<1.5%**. Pure harness/adapter delta = 54.3pp.
- Joint axes: model choice moves Pass@1 by 29.4pp; **harness choice by 27.4pp** under fixed models (Qwen 3.6-flash: 38.6%→66.0% across five claws; 12.5pp spread on GLM 5.1). Cost-aware Pareto framing: similar accuracy, order-of-magnitude different cost (GPT-5.5 $1399 vs DeepSeek-V4 Flash $8.2 at 70.3%).
- ⚠️ Note: the seeded "9–17pp same-model swings (Deep Feed synthesis)" — ❌ source unverified. Verified analogues: 11–15pp (Epoch monitoring, cited in 2605.23950), 12.5–27.4pp claw spreads (Claw-SWE-Bench), +2.1pp (WarpGrep), ±7–9.5pp (SEAL/TB2). Treat "9–17pp" as the low end of a 2–48pp literature range.

### 1.5 Corroborating practitioner sources
- ⚠️ **Stencil, "The Harness Problem" (12 Feb 2026):** https://stencil.so/blog/the-harness-problem — OpenAI's `apply_patch` given to non-OpenAI models: patch-failure 50.7% (Grok 4), 46.2% (GLM-4.7); hashline edit format lifts Grok Code Fast 1 from 6.7%→68.3% (format was masking model ability); Claude Code sub-agent JSONL leakage waste. "A different edit format improves Anthropic's own models by 5–14 points while cutting output tokens ~20%."
- ⚠️ **Signal/Youpan, "Better models, worse tools" (Jul 2026):** https://signal.youpan.com/the-stack/15-better-models-worse-tools-custom-edit-schemas — Armin Ronacher found Opus 4.8 / Sonnet 5 emit *more* malformed calls to Pi's custom `edits[]` tool than older Claudes: RL training on Claude Code's native search/replace shape creates a schema prior that leaks invented fields into third-party tools. Advice: measure tool-call success per model version; keep custom schemas close to native shapes; consider schema-per-model routing; never assume a newer model is a drop-in upgrade.
- ⚠️ **AST Edits benchmark (Apr 2026):** https://geometricagi.github.io/2026/04/02/ast-edits.html — edit-format results are **model-dependent** (unified diff: +48pp on o4-mini, −21pp on Haiku from hashline-diff), itself live evidence of the model×harness interaction term. AST-targeted edits: zero format failures, 100% on 3 of 4 models.
- Context: ✅ **Harness Engineering: Anatomy of 11 Coding Agents** — https://arxiv.org/abs/2609.00006 (no runtime imports an agentic framework; behavioral policy migrating "from prompt prose to configuration").

---

## 2. Production Per-Model Prompt/Tool-Format Variants

### 2.1 Cursor — ✅ per-model harness customization (secondary but consistent)
Sources: Cursor engineering content summarized at https://rajrajhans.com/bookmarks/325/ ("Continually improving our agent harness"); corroborated by https://nicolasbustamante.com/blog/model-harness-fit

- **Edit format is routed per model family (verified via both sources):** OpenAI models get **patch-based** file-editing tools ("that's what they were trained on"); Anthropic models get **string-replace**. Prompting style also differs: "OpenAI is literal and precise, Claude is more intuitive." The seeded phrasing "extra reasoning tokens + more mistakes when mismatched" is a ⚠️ paraphrase of the mismatch costs (retry loops, patch failures) documented in Stencil/Model-Harness-Fit; exact quote not located.
- **New-model onboarding:** "weeks tuning the harness to that model's quirks"; one model exhibited **"context anxiety"** (refusing work as context filled) fixed by prompt adjustment.
- **Mid-conversation model takeover = OOD (verified):** switching models mid-task makes the conversation history **out-of-distribution** for the incoming model (different tool shapes) plus cache misses; mitigated by conversation summarization; recommendation is **stay with one model per task**.
- Metrics: "Keep Rate" (fraction of agent code surviving over time) + LLM-judged satisfaction; tool-call errors classified (InvalidArguments vs ProviderError vs UnexpectedEnvironment) with anomaly detection.

### 2.2 GitHub Copilot CLI — ✅ explicit per-family tool routing (primary-adjacent)
Source: https://nicolasbustamante.com/blog/model-harness-fit (reads Copilot SDK changelog + source; SDK is MIT: https://github.com/github/copilot-sdk)

- **Three-leg strategy (verified from changelog):** (1) `apply_patch` tool is included **only** when the active model is Codex-family ("Codex specific patch toolchain", v0.0.366); (2) Anthropic models get the Edit/Write tool shape they were trained on; (3) per-model routing as a product stance for a multi-model harness.
- Transport-level routing per family is standard practice (e.g., Claude IDs → Anthropic Messages; GPT/o-series → Responses; Gemini → Chat Completions; per-model effort/reasoning levels preserved in requests — see https://docs.openclaw.ai/providers/github-copilot).

### 2.3 Claude Code per-model prompt variants — ❌ UNVERIFIED (flagged)
- ✅ The repo **exists**: https://github.com/asgeirtj/system_prompts_leaks (asgeirtj; ~51–64k stars by Jul 2026; CC0-1.0 metadata; contains `Anthropic/claude-code/` prompts+skills, plus Cursor, Copilot, Codex, Grok prompts). Community analyses note some leaked prompts flagged as fake — "stars do not equal authenticity" (https://www.shareuhack.com/en/posts/ai-system-prompt-leak-claude-gpt-analysis-2026).
- ❌ **NOT found in repo greps:** `lean_prompt`, `over-comment`, `Opus 4.6 specific` — zero hits. The only `MODEL LAUNCH` hits are inside a *skill doc* (claude-api migration guidance about codebases tagging sync points), not a system-prompt counterweight mechanism. So the seeded claims — **lean_prompt/full system-prompt flag, @[MODEL LAUNCH] counterweight paragraphs (over-commenting / false-claims / thoroughness / assertiveness), per-model effort defaults — are UNVERIFIED.** Alternative verification path if needed: the `claw-code` Rust port (https://github.com/ultraworkers/claw-code) which Model-Harness-Fit says tracks Claude Code closely; Claude Code does inject observable `<system-reminder>` blocks per turn.
- ✅ Related verified nuance: labs co-train model ↔ native harness tool shape (Anthropic: search/replace; OpenAI: `apply_patch`), so "per-model tool/prompt variants" is not just folklore — it is RL-visible in model behavior (Signal/Youpan, above).

---

## 3. Drift & Determinism

### 3.1 Baseline behavioral drift — ✅ arXiv 2601.19934
https://arxiv.org/abs/2601.19934 (Nicholson, 12 Jan 2026)

- **Temp-0 nondeterminism (verified, exact):** exact repeats at T=0.0 → unique-output fraction **0.240 (gpt-4o-mini)** vs **0.093 (llama3.1-8b)**; Jaccard 0.893 vs 0.966. Matches seed "~25% vs ~10%".
- Perturbed inputs at T=0: 0.572 vs 0.274. Reuse mode reduces (0.200/0.100). T=0.7: ≈0.99–1.00 unique, Jaccard <0.52. 20–30 runs/prompt, 5 prompt categories.
- **Attribution nuance (important):** "larger models = more variable" is supported via cited prior work (120B model identical in only ~12.5% of runs; 7–8B fully consistent) — but in this paper's own comparison the API-served model is the more variable one, and Appendix A attributes much of that to the **serving stack** (dynamic batching, queuing, traffic shaping): "API-induced drift should be regarded as a systemic property of the inference pipeline rather than a user-controlled parameter."
- **Variance budgets:** treat drift like an error budget (±5% in finance-calibrated settings); measurement must be separated from control; single-run reporting obscures worst-case behavior.

### 3.2 Variability layers; temp-0 ≠ deterministic trajectory — ✅ arXiv 2606.08998
https://arxiv.org/abs/2606.08998 ("Sampling, State, and the Stochasticity of AI Agents", v3 Jul 2026). ⚠️ Seed's title "Token Not Taken" does not appear in any version (v1: "…Variability of AI Agent Outputs") — flag as nickname/UNVERIFIED title; content matches seed exactly.

- Separates **one intrinsic** source (token sampling w/ PRNG) from **extrinsic** classes: serving infrastructure (batch composition, numerics, MoE load, routing — "a seed usually cannot remove this in managed services"), environment (live data, tools, clock), and context/orchestration.
- One flipped near-tie token under greedy decoding can redirect a tool call → different branch → different outcome ("amplification from token-level sampling to agent-level behavior").
- **"Temperature zero should be read narrowly"** — it disables intentional sampling only; it does not guarantee an identical agent trajectory.
- Prescribes deterministic/seeded runs as **diagnostic controls** (pre-production regression, post-incident replay with logged context/observations/decoding metadata) — not as a production determinism guarantee.

### 3.3 Behavioral reproducibility of tool-calling agents — ✅ arXiv 2605.28840
https://arxiv.org/abs/2605.28840 · code: https://github.com/Abelo9996/agent-consistency

- 6 models × 19 tasks × 10 runs = 1,140 traces. **"Structural consistency, parametric variance":** Tool-Sequence Similarity **TSS = 0.87** vs Argument Consistency **AC = 0.69** (d=0.75, p<10⁻¹³) — matches seed.
- **60% of first divergence occurs in the first two pipeline steps** (mean divergence point 2.2) → monitor only the first 1–2 tool calls vs a reference trace.
- **Consistency predicts success:** high-TSS cells 90.2% correct vs 61.2% low-TSS (d=0.81); AC is benign (r=0.12, n.s.) → **TSS is a correctness-free reliability proxy**; a structurally deviant run is ~3× more likely to fail.
- **Task-spec ambiguity beats model choice** as a consistency lever (ambiguity cuts AC 28%, model selection η² n.s.) → cheapest fix is better task specs, not better models. NL outputs match <5% even when tool sequences are identical → **assert on structured behavior, not text**.

### 3.4 The consistency gap + episodic memory fix — ✅ arXiv 2609.08832
https://arxiv.org/abs/2609.08832 (8 Sep 2026)

- AppWorld, ReAct/GPT-4.1: **Pass⁵ = 53% vs Mean@5 = 77%** → 24.4pp "consistency gap"; ~32% of tasks mix passes and failures across runs — exactly where Mean@k misleads.
- Fix: black-box **Consistency Analyzer** (resample each inference point; flat near-tied token distributions = flip-prone even at temp 0) + **Guideline Generator** → episodic-memory guidelines injected into future similar tasks. **Pass⁵ +16.0pp same-task (53→69%), +13pp similar-task**, Mean@5 never degraded (+3.6pp); aggregate trajectory consistency predicts pass/fail at AUROC ≈ 0.70 (usable without ground truth).
- Bonus context: ✅ Agent Drift, arXiv 2601.04170 (multi-agent behavioral degradation over long interactions; Agent Stability Index; episodic memory resets + adaptive routing as mitigations).

---

## (a) The Joint (Model×Harness) Evaluation Doctrine — synthesized

1. **The unit of evaluation is the harness–model pair** (Scaffold Effect; Binding Constraint Thesis). A benchmark score B(M,H) is a joint draw: Var(B) = MV + HV + interaction, and on long-horizon tasks HV ≥ MV with non-negligible interaction (7.80× measured; 6/9 ranking reversals).
2. **No cross-model claim without harness disclosure.** Adopt the ETCSOVG Harness Card (7 layers) for every internal eval run; treat undisclosed-scaffold leaderboard deltas < ~10pp as unattributable noise — they are routinely harness-sized (+2.1pp WarpGrep flips rankings; model-upgrade-sized gains are 2–4pp).
3. **Two valid regimes only:** locked-harness (all models on one pinned harness) or factorial (report HV, MV, reversals, η²). Our regression harness = locked-harness; our harness experiments = mini-factorial (≥2 models × ≥2 harness variants).
4. **Report cost+behavior axes with pass rate:** tokens/solved-task (40–65× harness spread), no-action/idle turns (10×), failure-category vector (fingerprints are harness properties — Goose=REASON / OpenHands=VERIFY / OpenCode=TIME patterns replicate across models), latency.
5. **Harness changes are model-sensitive interventions** (2608.26197): a constraint that helps one model degrades another; measure per-model cost (latency split, reproducibility deltas) before shipping; structured/schema-validated planning is the highest-yield determinism lever once tool/state paths are pinned.
6. **Tool-format compatibility is a per-model property** (apply_patch ≈50% failure off-family; string-replace whitespace-fragile; RL native-tool priors leak into third-party schemas; new model releases can *raise* malformed-call rates) → track tool-call success/retry rate per model version as a first-class metric.
7. **Adapter/patch-contract design can dominate the headline number** (19.1→73.4): never let the model hand-write patch text; edit-via-tools + runner-extracted diffs.

## (b) Per-model prompt/tool-format variant patterns worth adopting

1. **Edit-format routing per model family** (Cursor, Copilot CLI): OpenAI-family → patch/diff-shaped tool; Anthropic-family → search/replace Edit/Write shape. Cheapest correct version: keep our edit tool *shaped like the native tool of whichever family is active*, or route between two schema adapters.
2. **Per-model prompt styling** (Cursor): literal/precise phrasing for OpenAI-family, intuition-tolerant for Anthropic-family; treat a new model onboarding as a multi-week harness-tuning task with per-model quirks (e.g., "context anxiety") logged and patched.
3. **Per-model effort/reasoning defaults** (Copilot/OpenClaw catalog pattern): store per-model supported effort levels in a model catalog and preserve them in requests; ⚠️ Claude-Code-specific effort defaults from the leaks repo remain unverified.
4. **Schema-similarity rule** (Signal/Youpan): if we must use a custom tool schema, make it resemble the family's native one; add per-model "schema dialect" only if measured retry/token cost justifies it; re-run tool-integration tests on **every** model upgrade.
5. **Never switch model mid-session** without a summarization/compaction boundary (OOD history + cache misses); if multi-model is required, switch at task boundaries only (Cursor's explicit recommendation).
6. **Model-launch counterweights** (unverified pattern, directionally supported): if adopting per-model addenda, mirror the claimed structure — small model-specific paragraphs counteracting known biases (over-commenting, false-confidence, over/under-thoroughness, assertiveness) gated on model ID — but gate on our own eval evidence, not on the unverified leak. Verified foundation for this: failure fingerprints are harness/prompt-shape properties, and 2608.26197 shows prompt-level structure (schema-validated planning) is the residual-variance killer.
7. **Determinism scaffolding from 2608.26197:** finite-state task control, forced tool selection, output validation, bounded retry + **structured planning validated against a fixed schema** → Determinism Index 1.000 in 3/4 cells. Adopt for high-stakes eval lanes; measure latency cost per model first.

## (c) Drift implications for our evals + session reproducibility design

1. **Assume temp-0 nondeterminism from day one** (~24% distinct outputs at T=0 for an API mini-model; ~9% local 8B): never draw conclusions from single runs. N≥3 for smoke, N≥5 for release gates; report Mean@k **and** Pass^k (all-run success) — the 24pp Mean@5↔Pass⁵ gap is the number users actually experience.
2. **Log the whole trajectory, not just the call** (2606.08998): session reproducibility = pinned (model version + route + decoding params + PRNG seed where exposed) **plus** full context/observation logs + decoding metadata for replay; accept managed-API serving (batching/numerics) as an irreducible layer and say so in run manifests.
3. **TSS as a cheap, correctness-free reliability signal** (2605.28840): compare each run's first 1–2 tool calls against a reference trace; structural deviation ⇒ ~3× failure odds. Add TSS + divergence-point to our eval telemetry; assert on tool sequences/arguments, never on natural-language output (<5% exact match even when behavior is identical).
4. **Consistency-targeted memory** (2609.08832): resample flip-prone steps offline, distill into guidelines, inject on similar tasks — +16pp Pass⁵ at no Mean@k cost. Natural fit for our session-memory layer: store *consistency scorecards*, not just outcomes.
5. **Variance budgets per eval tier** (2601.19934 + Khatchadourian & Franco): define tolerable drift per output class (strict for numbers/patches; loose for prose); exceed-budget triggers investigation, not silent rerun.
6. **Spec quality is the biggest consistency lever** (2605.28840): ambiguity in task specs hurt consistency more than model choice — invest in unambiguous eval task definitions before buying consistency with stronger models.
7. **Drift attribution protocol:** when a score moves, decompose (i) model/route/version change, (ii) harness/config diff (ETCSOVG card diff), (iii) run-to-run sampling variance (re-run same config), (iv) serving-stack drift (provider-side; detect via temp-0 repeated probes). Only (iii) is "noise"; the rest are regression classes with different owners.
8. **Early-divergence replay:** since 60% of divergence is in steps 1–2, keep a golden-prefix replay suite (identical first two tool calls against pinned context) as the fastest regression tripwire.

---

## Verification ledger

| Seed claim | Status | Source |
|---|---|---|
| 7.8× harness-vs-model variance | ✅ exact (7.80×) | arXiv 2605.23950 §4.2 |
| Opus 4.5 SEAL 45.9% → Claude Code 55.4% | ✅ exact | arXiv 2605.23950 §2.1/§4.1 |
| WarpGrep subagent flips rankings | ✅ exact (MiniMax 2.5 vs Opus 4.6) | arXiv 2605.23950 §2.1 |
| TB2 69.7→77.0 harness-only | ✅ exact (AHE, GPT-5.4 high) | arXiv 2605.23950 Table 1 |
| Scaffold Effect: 40× tokens at equal pass | ✅ exact (40.8–41.9×) | arXiv 2607.22585 (Zenodo DOI ❌ unconfirmed) |
| Failure fingerprints are harness properties | ✅ exact (replicate across 2 models) | arXiv 2607.22585 §4.5 |
| 2608.26197 constraints help one model, hurt another | ✅ exact (1 improve/2 degrade/1 null; latency split) | arXiv 2608.26197 abstract |
| Cursor: patch for OpenAI vs string-replace for Anthropic | ✅ (via ⚠️ secondary) | rajrajhans.com/bookmarks/325/; nicolasbustamante.com/blog/model-harness-fit |
| "extra reasoning tokens + more mistakes" (Cursor quote) | ⚠️ paraphrase only | mismatch costs documented in Stencil blog |
| Copilot CLI per-family tool routing | ✅ (apply_patch Codex-family-only, v0.0.366) | Model-Harness-Fit blog / copilot-sdk changelog |
| Mid-conversation takeover = OOD | ✅ exact | Cursor talk summary + Model-Harness-Fit §Mid-Chat |
| system_prompts_leaks repo | ✅ exists | github.com/asgeirtj/system_prompts_leaks |
| lean_prompt flag / @[MODEL LAUNCH] counterweights / per-model effort defaults (Claude Code) | ❌ UNVERIFIED — greps: lean_prompt, over-comment, "Opus 4.6 specific" = 0 hits; "MODEL LAUNCH" only in a skill doc | same repo |
| Deep Feed synthesis 9–17pp same-model swings | ❌ source unverified (analogues: 11–15pp Epoch, 12.5–27.4pp Claw-SWE) | — |
| Claw-SWE-Bench 19.1→73.4 adapter-only | ✅ exact (GLM 5.1; apply-fail 69.1%→<1.5%) | arXiv 2606.12344 |
| 2601.19934: ~25% vs ~10% temp-0 distinct outputs | ✅ exact (0.240 / 0.093); "larger=more variable" from cited prior work; API-stack attribution nuance | arXiv 2601.19934 §4/§5/App A |
| 2606.08998 "Token Not Taken" | ✅ ID+content verified; ❌ title is actually "Sampling, State, and the Stochasticity of AI Agents" | arXiv 2606.08998 |
| 2605.28840: 0.87 structural vs 0.69 argument; early divergence predicts failure | ✅ exact (TSS 0.87/AC 0.69; 60% divergence in steps 1–2; high-TSS 90.2% vs 61.2%) | arXiv 2605.28840 |
| 2609.08832: 53% vs 77%, +16pt from episodic memory | ✅ exact (Pass⁵ 53% vs Mean@5 77%, 24.4pp gap; +16.0pp same-task / +13pp similar-task) | arXiv 2609.08832 |
