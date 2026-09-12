# FINAL Phase — Agent B: Tool-Calling Calibration + Delegation-vs-DIY Tendencies

**Scope:** LLM-behavior elicitation for harness design. Seeded facts verified against primary sources (Sep 2026). Status labels: **VERIFY** = confirmed against source; **UNVERIFIED** = seed claim not confirmed verbatim or inferred.
**Date:** 2026-09-12

---

## (a) Evidence Table

### Tendency 1 — Tool-call calibration (over/under-calling)

| # | Finding | Detail | Source | Status |
|---|---------|--------|--------|--------|
| 1.1 | Models over-call indiscriminately | Tool-augmented agents call tools even when they can answer directly; each unnecessary call wastes fees/latency. When2Tool benchmark: 18 environments (15 single-hop, 3 multi-hop) across 3 necessity categories (computational scale, knowledge boundaries, execution reliability). | [arXiv:2605.09252](https://arxiv.org/abs/2605.09252) "LLM Agents Already Know When to Call Tools — Even Without Reasoning" (Sun et al., May 2026) | VERIFY |
| 1.2 | Necessity is decodable but unacted-upon | Tool necessity is linearly decodable from pre-generation hidden states with **AUROC 0.89–0.96 across six models** — substantially exceeding the models' own verbalized reasoning. "Models already know when tools are needed, but fail to act on this knowledge during generation." | same | VERIFY |
| 1.3 | Prompt-only control is coarse | Prompt-only baselines "suppress necessary calls alongside unnecessary ones"; Reason-then-Act incurs disproportionate accuracy cost on hard tasks. Verbalized control ≠ latent knowledge. | same | VERIFY |
| 1.4 | Probe&Prefill | Lightweight linear probe reads hidden-state signal and prefills a steering sentence: **−48% tool calls at only −1.7% accuracy**. Best baseline at comparable accuracy cuts only 6% of calls; similar reduction otherwise costs 5× higher accuracy loss. | same | VERIFY |
| 1.5 | Intrinsic over-calling bias (mechanistic) | 6 models / 3 families on When2Call: high call-accuracy, low no-call accuracy → overall accuracy only 55–70%. Intrinsic Bias Hypothesis (IBH): the call/no-call mapping carries an **activation-independent call offset**; decision-neutral only when no_call activation outweighs call activation. | [arXiv:2605.18882](https://arxiv.org/abs/2605.18882) "To Call or Not to Call" (Shi et al., May 2026) | VERIFY |
| 1.6 | Causal fix via SAE steering | Adaptive Margin-Calibrated Steering (AMCS): closed-form counter-bias shift along SAE decoder directions; cancelling the offset mitigates over-calling with negligible call-accuracy drop. Recasts over-calling as a mechanistic, causally correctable object. | same | VERIFY |
| 1.7 | Seed term "calling effort" | Exact phrase not found in abstract; the concept corresponds to the "call offset" / call-rate margin. | same | UNVERIFIED (concept verified) |
| 1.8 | Knowledge epistemic illusion | Tool overuse is pervasive; models **misjudge their internal knowledge boundaries** and can't perceive actual knowledge availability → prefer external tools over internal knowledge. | [arXiv:2604.19749](https://arxiv.org/abs/2604.19749) "The Tool-Overuse Illusion" (Zeng et al., Mar 2026) | VERIFY |
| 1.9 | DPO boundary alignment | Knowledge-aware epistemic boundary alignment via DPO: **−82.8% tool usage while improving accuracy**. | same | VERIFY |
| 1.10 | Outcome-only rewards cause overuse | Causal link shown by visualizing tool-augmented training: rewarding only final correctness (regardless of tool efficiency) encourages overuse. Balanced rewards cut unnecessary calls **−66.7% (7B) / −60.7% (32B)** without accuracy loss. | same | VERIFY |
| 1.11 | Call rate is one linear direction | A single training-free direction in the residual stream steers call rate **monotonically from ~0% to >90%** with strength α; calls stay well-formed. Bidirectional: dialing up induces calls that land *precisely* on questions the model cannot answer alone; generalizes to unseen tools; transfers across dense/MoE/multimodal. Live-tool sweep traces a cost/accuracy Pareto frontier; open-domain QA **0.29 → 0.56**. | [arXiv:2608.25198](https://arxiv.org/abs/2608.25198) "Tunable Tool-Call Rates in LLM Agents via Representation Steering" (Chen et al., Aug 2026) | VERIFY |
| 1.12 | Parallel-vs-sequential is per-model | Claude 4+ call multiple tools in parallel **by default**; **Claude Fable 5.1 issues fewer parallel calls**, "most noticeably in long agent loops where the next reads are only implied"; Claude 3.7 Sonnet needed a `batch_tool` meta-tool workaround. `<use_parallel_tool_calls>` prompt blocks strengthen parallelism. | [Anthropic parallel-tool-use docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use); [3.7 cookbook](https://platform.claude.com/cookbook/tool-use-parallel-tools) | VERIFY |
| 1.13 | Parallelism is harness-teachable & fragile | Formatting tool_results as one user message per result "teaches" Claude to avoid parallel calls; batching all tool_results in one message restores it. `disable_parallel_tool_use: true` in `tool_choice` = at most one call (auto) / exactly one (any/tool). | [docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use); [troubleshooting](https://platform.claude.com/docs/en/agents-and-tools/tool-use/troubleshooting-tool-use) | VERIFY |
| 1.14 | Trigger boundary is prompt-steerable | "Use the tools to investigate before responding" measurably increases tool use; "Use your judgment" keeps it conservative; `tool_choice` gives hard guarantees. Prompt nudges are real but soft — mirrors 1.3. | [tool-use overview](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview) | VERIFY |

### Tendency 2 — Delegation vs. DIY (subagent spawning)

| # | Finding | Detail | Source | Status |
|---|---------|--------|--------|--------|
| 2.1 | Over-spawning failure mode | Early Anthropic agents "spawned 50 subagents for simple queries, scouring the web endlessly for nonexistent sources, and distracting each other with excessive updates." | [Anthropic engineering blog, Jun 2025](https://www.anthropic.com/engineering/built-multi-agent-research-system) | VERIFY |
| 2.2 | Agents cannot self-judge effort | "Agents struggle to judge appropriate effort for different tasks" → effort-scaling rules embedded in prompts: **simple fact-finding = 1 agent, 3–10 tool calls; comparisons = 2–4 subagents, 10–15 calls each; complex = >10 subagents** with divided responsibilities. Prevented overinvestment in simple queries. | same | VERIFY |
| 2.3 | Token economics dominate | Multi-agent ≈ **15×** chat tokens; single agent ≈ 4×. Token usage alone explains **80%** of BrowseComp performance variance (3 factors = 95%: tokens, # tool calls, model choice). | same | VERIFY |
| 2.4 | Payoff when warranted | Multi-agent (Opus 4 lead + Sonnet 4 subagents) beat single-agent Opus 4 by **90.2%** on internal research eval. Parallel tools + 3–5 parallel subagents cut research time **up to 90%** for complex queries. | same | VERIFY |
| 2.5 | Orchestration counterweights (Opus 4.6-era) | Effort param (low/medium/high/max; `low` documented best-for "subagents"); adaptive thinking; orchestration mode is **opt-in** with standing-consent tool description; hard caps **`MAX_CONCURRENT` + `MAX_TOTAL_SUBTASKS`** (sample: 4 / 200); granularity guidance ("a focused review of a few hundred lines rarely needs more than about ten subtasks"); "reserve the mode for work that justifies the cost." | [Opus 4.6 announcement](https://www.anthropic.com/news/claude-opus-4-6); [mid-conversation effort example](https://platform.claude.com/docs/en/build-with-claude/mid-conversation-effort-example); [effort docs](https://platform.claude.com/docs/en/build-with-claude/effort) | VERIFY |
| 2.6 | Opus 4.6 over-diligence | "Opus 4.6 often thinks more deeply... can add cost and latency on simpler ones. If you're finding that the model is overthinking... dial effort down from high to medium." Vendor-acknowledged over-effort tendency with a dial as counterweight. | [announcement](https://www.anthropic.com/news/claude-opus-4-6) | VERIFY |
| 2.7 | "Over-spawning counterweights for Opus 4.6" as a named thing | No doc uses this exact phrase; the counterweights above (effort dials, opt-in orchestration, caps, granularity guidance) are the concrete mechanism. | same | UNVERIFIED (framing) / mechanism VERIFY |
| 2.8 | Codex: explicit-only spawning | "Codex only spawns subagents when you explicitly ask it to." `spawn_agent` tool text: "Only use spawn_agent if and only if the user explicitly asks for sub-agents, delegation, or parallel agent work. Requests for depth, thoroughness, research, investigation, or detailed codebase analysis do not count as permission to spawn." Defaults: `agents.max_threads = 6`, `agents.max_depth = 1` (no recursive delegation). | [Codex subagents docs](https://developers.openai.com/codex/subagents); [issue #23496](https://github.com/openai/codex/issues/23496) | VERIFY |
| 2.9 | Explicit-only policy is brittle in practice | Repo/AGENTS.md standing authorization conflicts with session spawn policy → **silent degradation**: agent skips mandated subagents and does the work itself, or duplicates delegated research in the parent thread (#18319). Users request `explicit` / `workflow` / `auto` delegation modes (#18513). | [#16996](https://github.com/openai/codex/issues/16996), [#18319](https://github.com/openai/codex/issues/18319), [#18513](https://github.com/openai/codex/issues/18513) | VERIFY |
| 2.10 | Delegation proactivity is a product tier | ChatGPT Work: "At most intelligence levels, ask for delegation explicitly. With Ultra, ChatGPT can proactively delegate work when parallel agents would materially improve speed or quality." Proactive delegation is gated, not model-judged. | [ChatGPT subagents docs](https://learn.chatgpt.com/docs/agent-configuration/subagents) | VERIFY |
| 2.11 | Grok: agent count IS the effort knob | `grok-4.20-multi-agent`: `agent_count` = 4 or 16; via OpenAI-compatible API, `reasoning.effort` low/medium → **4 agents**, high/xhigh → **16 agents**. "More agents means deeper, more thorough research at the cost of higher token usage and latency." For this model, effort "controls how many agents collaborate rather than reasoning depth." Grok CLI also ships `--no-subagents`. | [xAI multi-agent docs](https://docs.x.ai/developers/model-capabilities/text/multi-agent); [xAI reasoning docs](https://docs.x.ai/developers/model-capabilities/text/reasoning); [CLI reference](https://docs.x.ai/build/cli/reference) | VERIFY |

---

## Synthesis (cross-cutting)

1. **Knowledge–action gap is the core calibration fact.** Latent necessity signal is strong (AUROC 0.89–0.96) yet generation ignores it; a persistent call offset + epistemic illusion makes over-calling systematic and per-model, not noise. Prompting alone is provably coarse (suppresses necessary calls; 1.3 vs 1.4).
2. **Call propensity is a controllable scalar.** Both mechanistically (steering α: 0→90% monotonic) and at the API surface (prompt-strength tiers, `tool_choice`, `disable_parallel_tool_use`). Harnesses can and should expose this as a dial.
3. **Delegation eagerness is a profile property, not a universal.** Claude-family defaults eager (50-subagent failures → needs throttles); Codex-family defaults explicit-only (→ needs authorization plumbing; suffers silent degradation); xAI collapses the whole question into an `agent_count` billing knob. Same latent behavior, opposite vendor defaults.
4. **Token budget, not call count, is the dominant outcome variable** (80% of variance; 15× cost multiplier) — and the dominant failure is mis-sized effort, which models cannot judge for themselves (2.2).

---

## (b) Harness Design Recommendations

**R1. Two-tier call-budget governors.** Soft budget injected into context ("N of M calls used; wrap up or justify continuing") + hard budget that denies calls past the cap with a structured error. Defaults keyed to task class: ~3–10 calls (fact-finding) / 10–15 (comparisons) / larger with watchdog (complex), per the Anthropic tiers (2.2). *Evidence tie:* prompt-only suppression is coarse (1.3); agents can't self-judge effort (2.2).

**R2. Task classifier → profile at dispatch.** Classify each task (fact-find / comparison / open research / coding) and set per-class defaults for: tool budget, subagent cap, effort level, parallelism hints. *Evidence tie:* Anthropic's embedded scaling rules (2.2); Grok's 4-vs-16 tiers (2.11).

**R3. Delegation-throttle prompts per profile — verbatim numbers, not vibes.** Embed explicit scaling rules in orchestrator prompts ("Simple fact-finding: 1 agent, 3–10 tool calls") plus counterweights ("Work solo on conversational turns or trivial mechanical edits" — Anthropic's own Workflow-tool text). Default throttle **ON** for eager profiles (Claude-family), **OFF but plumbable** for explicit-only profiles (Codex-family). *Evidence tie:* 2.1, 2.2, 2.7.

**R4. Split subagent caps into two knobs.** `max_concurrent` (fan-out width) and `max_total_subtasks` (queued per workflow), exactly as in Anthropic's orchestration example (4 / 200) — plus `max_depth = 1` default (Codex) to kill recursive fan-out. Splitting lets the model plan a backlog without launching it all at once. *Evidence tie:* 2.5, 2.8.

**R5. Token-budget governor as the primary economic control.** Since tokens explain 80% of outcome variance and multi-agent costs ~15×, budget tokens per run and per subagent, surface burn-rate in the status line, and auto-degrade (collapse fan-out → serialize → solo) when projected spend exceeds cap. Call counts alone are insufficient. *Evidence tie:* 2.3.

**R6. Tool-propensity calibration layer.** For self-hosted models: implement Probe&Prefill gating (probe hidden state → prefill steering sentence, −48% calls @ −1.7% acc) or representation steering (`tool_propensity: α` knob, monotonic 0→90%, induces calls exactly where the model would hallucinate) as a first-class config. For API models: approximate with prompt-strength tiers + `tool_choice` (auto/none/any) + `disable_parallel_tool_use`. *Evidence tie:* 1.4, 1.11, 1.14.

**R7. Spawn-authorization classes (the Codex lesson).** Distinguish **standing authorization** (read-only probes, validation, exploration — satisfiable by AGENTS.md/repo policy) from **fresh approval** (edits, commits, deploys, external side effects). Expose an explicit capability/authority signal to the model; **never silently degrade to parent-DIY** — if spawn is blocked, say so and ask once. *Evidence tie:* 2.9's silent-degradation failures (#16996, #18319, #23496).

**R8. Parent-duplication guard.** When work is delegated, structurally discourage the lead from redoing it in its own thread (instruction + overlap detection between lead tool calls and subagent task descriptions; flag >X% overlap). *Evidence tie:* 2.9 (#18319: "duplicates delegated research in parent thread").

**R9. Tool-efficiency in eval/reward rubrics.** Outcome-only grading trains over-callers (1.10). LLM-as-judge rubrics should score "right tools, reasonable number of times" (Anthropic's rubric did exactly this) and count unnecessary calls as loss — applies to any harness-side eval or fine-tune loop. *Evidence tie:* 1.10, 2.4.

**R10. Parallelism normalization utilities.** (a) Always batch tool_results in ONE message — scattered formatting silently teaches models to serialize (1.13); (b) ship a `batch_tool` meta-tool for under-parallelizing models (3.7 Sonnet pattern; Fable 5.1 under-parallelizes in long loops, 1.12); (c) honor `disable_parallel_tool_use` for ordering-sensitive/mutating tools; (d) document per-model parallel defaults in the profile registry.

**R11. Unified config surface.** Expose all of the above as per-profile, per-task-class config, e.g.:

```toml
[tools.governor]
soft_budget = 30          # R1 soft
hard_budget = 50          # R1 hard
tool_propensity = 0.0     # R6 (steering alpha or prompt-tier alias)

[delegation]
mode = "explicit"         # "explicit" | "workflow" | "auto"  (Codex #18513 taxonomy)
max_concurrent = 4        # R4
max_total_subtasks = 200  # R4
max_depth = 1             # R4
token_budget_per_subagent = 150000  # R5
standing_auth = ["read-only", "validation", "exploration"]  # R7
fresh_auth = ["edit", "commit", "deploy"]                   # R7
```

**Anti-patterns (do NOT):**
- Rely on prompt-only suppression as the sole governor — it kills necessary calls along with unnecessary ones (1.3).
- Impose one global call cap across task classes — correct budgets differ by ~10× between simple and complex work (2.2).
- Let outcome-only evals grade agent quality — it causally induces overuse (1.10).
- Allow silent fallback from "spawn blocked" to "parent does it itself" — the single most damaging real-world Codex failure mode (2.9).

---

### Key URLs
- https://arxiv.org/abs/2605.09252 (When2Tool / Probe&Prefill)
- https://arxiv.org/abs/2605.18882 (intrinsic over-calling bias / AMCS)
- https://arxiv.org/abs/2604.19749 (Tool-Overuse Illusion / DPO −82.8%)
- https://arxiv.org/abs/2608.25198 (representation steering, 0→90% call rate)
- https://www.anthropic.com/engineering/built-multi-agent-research-system
- https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use
- https://platform.claude.com/docs/en/build-with-claude/mid-conversation-effort-example
- https://platform.claude.com/docs/en/build-with-claude/effort
- https://www.anthropic.com/news/claude-opus-4-6
- https://developers.openai.com/codex/subagents
- https://github.com/openai/codex/issues/16996 · /18319 · /18513 · /23496
- https://docs.x.ai/developers/model-capabilities/text/multi-agent
- https://docs.x.ai/developers/model-capabilities/text/reasoning
