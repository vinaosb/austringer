# FINAL Phase (LLM-behavior elicitation) — Agent A: Provider control surfaces + per-model behavioral defaults

**Date**: 2026-09-12 · **Method**: primary docs fetched from platform.claude.com, developers.openai.com, ai.google.dev, docs.x.ai (this session). Every claim tagged [VERIFIED] (read from primary source today) or [UNVERIFIED] (seeded/secondary, not confirmed in primary docs this session).

---

## 1. Per-family control matrix

### 1.1 Anthropic (Messages API)

| Knob | Wire path | Values / range | Default | Disable? | Notes & traps |
|---|---|---|---|---|---|
| Thinking mode | `thinking.type` | `adaptive` \| `enabled` \| `disabled` | adaptive **on** for Opus 5, Sonnet 5, Fable 5/5.1, Mythos 5/5.1, Mythos Preview; **off-until-set** (`adaptive`) for Opus 4.6/4.7/4.8, Sonnet 4.6 | `disabled` OK on Sonnet 5 + Opus 5 (Opus 5 only at effort ≤ `high`; at `xhigh`/`max` → **400**). Fable 5/5.1, Mythos 5/5.1: `disabled` → **400 always** | [VERIFIED] [thinking](https://platform.claude.com/docs/en/build-with-claude/thinking) |
| Legacy budget | `thinking.type:"enabled"` + `budget_tokens` | min **1,024**; must be `< max_tokens` (exception: interleaved, budget may exceed) | n/a | n/a | Deprecated on 4.6 (requests still succeed). **4.7+ (Opus 4.7/4.8, Opus 5, Sonnet 5, Fable 5/5.1, Mythos 5/5.1): `type:"enabled"` → 400.** Budget is a *target*, not cap; `max_tokens` is the hard ceiling. No `max_tokens:0` cache pre-warm combo. [VERIFIED] [extended-thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking) |
| Effort | `output_config.effort` | `low`, `medium`, `high`, `xhigh`, `max` | **`high`** (== omitting the field) | n/a (soft signal, not budget) | `max` on: Fable 5.1/5, Mythos 5.1/5, Mythos Preview, Opus 5/4.8/4.7/4.6, Sonnet 5/4.6 (+Opus 4.5). `xhigh` on: Fable 5.1/5, Mythos 5.1/5, Opus 5/4.8/4.7, Sonnet 5 (NOT Sonnet 4.6). Scales **all** tokens incl. tool calls. Seed "4.6+, low..max" is right per-model support is fragmented — check per model. [VERIFIED] [effort](https://platform.claude.com/docs/en/build-with-claude/effort) |
| Mid-conversation effort | `role:"system"` msg with empty `content` + `output_config.effort` | same 5 levels | — | — | Beta header `mid-conversation-output-config-2026-07-01`; **preserves prompt cache**; only Fable 5.1, Mythos 5.1, Opus 5 (others → 400). Top-level effort change between requests **invalidates cache** (rendered into prompt). [VERIFIED] [effort §beta](https://platform.claude.com/docs/en/build-with-claude/effort#change-effort-mid-conversation-beta) |
| Thinking display | `thinking.display` | `summarized` \| `omitted` \| `updates`(beta hdr `thinking-display-updates-2026-08-18`) | `summarized` on ≤4.6 models; **`omitted`** on Opus 5, Sonnet 5, 4.7, 4.8, Fable 5/5.1, Mythos 5/5.1 | invalid with `type:"disabled"` | `omitted` = faster TTFT, same cost; signature round-trips unchanged. `updates` surfaces between-tool-call progress text. [VERIFIED] [thinking](https://platform.claude.com/docs/en/build-with-claude/thinking#controlling-thinking-display) |
| Forced tool use | `tool_choice` | `auto`, `none`, `any`, `tool` | `auto` | — | Manual `enabled` thinking: only `auto`/`none` (forcing → error). Adaptive: `any`/`tool` OK **except Fable 5.1 + Mythos 5.1 → 400** (use `strict:true` + instruction / mid-convo system msg instead). [VERIFIED] [thinking](https://platform.claude.com/docs/en/build-with-claude/thinking#thinking-with-tool-use), [Fable 5.1 migration](https://platform.claude.com/docs/en/models/fable-5-1/migration-guide) |
| Interleaved thinking (manual mode) | beta hdr `interleaved-thinking-2025-05-14` | — | — | — | Per-model split: 4.5 & earlier = header required; Sonnet 4.6 = header works but deprecated; **Opus 4.6 manual mode = no interleaving at all** (adaptive only); Haiku 4.5 = none (header ignored). Adaptive models interleave automatically. [VERIFIED] [extended-thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking#interleaved-thinking-in-manual-mode) |
| Prefill | assistant last-turn content | — | — | — | **400 on 4.6+ & Mythos Preview.** Use system instructions / structured outputs. [VERIFIED] [best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices) |
| Thinking-block binding | round-trip thinking blocks | — | — | — | Fable 5.1: blocks valid only for producing model + unchanged prefix; edits → 400 (permanent for that body) or `thinking-binding-controls-2026-08-01` + `prefix_mismatch_behavior:"drop_block"`; model switches drop unread blocks (`input_transformations` / `model_binding_mismatch`). Mythos 5.1 skips the check. [VERIFIED] [Fable 5.1 migration](https://platform.claude.com/docs/en/models/fable-5-1/migration-guide) |
| Cache interaction | — | — | — | — | **Changing `budget_tokens` invalidates cache breakpoints** (value is rendered into prompt; doc shows cache_creation 1370 → read 1370 → re-create 1370 on a 4k→8k change). Same for thinking-mode switch and top-level effort change. Pick one value, hold for conversation. [VERIFIED] [extended-thinking §caching](https://platform.claude.com/docs/en/build-with-claude/extended-thinking#prompt-caching-in-manual-mode) |

### 1.2 OpenAI (Responses API)

| Knob | Wire path | Values | Default | Disable? | Notes & traps |
|---|---|---|---|---|---|
| Reasoning effort | `reasoning.effort` | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` — **model-dependent subset** | **`medium` for gpt-5.5**; GPT-5.6 defaults `medium` in both modes | `none` disables reasoning on models that support it | **gpt-5.5: `none`, `low`, `medium`, `high`, `xhigh` only** (no `max`/`minimal` per model page). **GPT-6 Astra: `none` → HTTP 400.** `xhigh` = deep research/long agentic runs; `max` = hardest tasks. Models reason adaptively *within* an effort. [VERIFIED] [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning), [gpt-5.5 model page](https://developers.openai.com/api/docs/models/gpt-5.5) |
| Reasoning mode | `reasoning.mode` | `standard` \| `pro` | `standard` | — | GPT-5.6 family only; orthogonal to effort; pro = more model work, billed at standard rates. [VERIFIED] [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning) |
| Output length | `text.verbosity` | `low`, `medium`, `high` | **`medium`** | — | Linear output scaling; GPT-5-era cookbook measured ≈ **560 / 849 / 1288 avg output tokens** low/med/high [UNVERIFIED for current models — from [GPT-5 cookbook](https://developers.openai.com/cookbook/examples/gpt-5/gpt-5_new_params_and_tools)]. On GPT-5.5, `low` is "proportionally more concise" than GPT-5.4 `low`. Effort ≠ length: control length via verbosity/prompt, depth via effort. [VERIFIED] [GPT-5.5 guidance](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-5.5) |
| Reasoning context | `reasoning.context` | `auto` \| `current_turn` \| `all_turns` | `current_turn` (pre-5.6), `all_turns` (GPT-5.6) | — | Same-family reuse only (5.6-sol/terra/luna interop; not across 5.5↔5.6). Stateless: `encrypted_content` auto-included on reasoning items. [VERIFIED] [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning) |
| Mid-conversation effort | `configuration_update` input item | effort values | — | — | **GPT-6 Astra only** (standard, single-agent); preserves prompt prefix/cache; holds until next update. [VERIFIED] [reasoning guide §](https://developers.openai.com/api/docs/guides/reasoning#change-reasoning-mid-conversation) |
| Sampling | `temperature`, `top_p`, `top_logprobs` | — | — | — | **Remove for GPT-6 Astra** (unsupported). [VERIFIED] [Astra guide](https://developers.openai.com/api/docs/guides/latest-model) |
| Spend ceiling | `max_output_tokens` | — | — | — | Hard ceiling incl. reasoning; `status:"incomplete"` + `reason:"max_output_tokens"` can fire **before any visible text**. Reserve ≥25k. [VERIFIED] [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning) |

### 1.3 Google (Gemini API / Interactions)

| Knob | Wire path | Values | Default | Disable? | Notes & traps |
|---|---|---|---|---|---|
| Thinking depth | `generation_config.thinking_level` | `minimal`, `low`, `medium`, `high` | **`high` (dynamic)** | `minimal` ≈ "no thinking for most queries" but **"does not guarantee thinking is off"**; **3.1 Pro does NOT support `minimal`** (floor = `low`) → thinking effectively cannot be disabled on Pro. 3.1 Flash-Lite default = `minimal` | Levels are **relative allowances, not token guarantees**. [VERIFIED] [Gemini 3 guide](https://ai.google.dev/gemini-api/docs/gemini-3) |
| Legacy budget | `thinking_budget` | int | — | — | **`thinking_level` + `thinking_budget` in one request → 400 error.** [VERIFIED] [Gemini 3 guide](https://ai.google.dev/gemini-api/docs/gemini-3) |
| Temperature | `temperature` | — | **1.0** | — | "Strongly recommend" keeping 1.0; <1.0 → looping / degraded reasoning, esp. math. [VERIFIED] [Gemini 3 guide](https://ai.google.dev/gemini-api/docs/gemini-3) |
| Thought signatures | encrypted `thought` fields round-tripped | — | — | — | Stateful mode (`previous_interaction_id`): server-managed. **Stateless: you must include thought blocks + signatures in subsequent requests to validate authenticity.** Seed "required-else-400" for multi-turn function calling: requirement VERIFIED on this page; the exact 400 wording is on the thought-signatures/function-calling docs, not re-read this session [UNVERIFIED-precise]. [Gemini 3 guide](https://ai.google.dev/gemini-api/docs/gemini-3), [thought signatures](https://ai.google.dev/gemini-api/docs/thinking) |

### 1.4 xAI (Grok API)

| Knob | Wire path | Values | Default | Disable? | Notes & traps |
|---|---|---|---|---|---|
| Reasoning effort | `reasoning_effort` (Chat) / `reasoning.effort` (Responses) | `low`, `medium`, `high`, `xhigh` | **`high`** | **Cannot be disabled** (no off value) | `grok-4.6`: low/med/high/xhigh; `grok-4.5`: low/med/high — `xhigh` on unsupported models **silently treated as `high`** (no error). `presencePenalty`, `frequencyPenalty`, `stop` → **error** with reasoning models. Encrypted reasoning via `include:["reasoning.encrypted_content"]`; summarized reasoning streamed for grok-4.6. [VERIFIED] [xAI reasoning](https://docs.x.ai/developers/model-capabilities/text/reasoning) |
| Multi-agent fleet size | `reasoning.effort` (= `agent_count` in xAI SDK) | `low`/`medium` → **4 agents**; `high`/`xhigh` → **16 agents** | — | — | `grok-4.20-multi-agent` only (beta): effort = **agent count, not depth**. Traps: **no Chat Completions**, **no `max_tokens`**, no client-side/custom tools (built-ins + remote MCP only); only leader output exposed, sub-agent state encrypted unless `use_encrypted_content`; all agents' tokens + server tool calls billed. [VERIFIED] [xAI multi-agent](https://docs.x.ai/developers/model-capabilities/text/multi-agent) |

---

## 2. Per-model behavioral defaults table

| Model | Thinking default | Effort default / recommended | Behavioral default that matters to a harness | Source |
|---|---|---|---|---|
| **Claude Opus 4.6** | off until `adaptive` set | `high`; effort composes w/ legacy budget | **More upfront exploration than prior models**, esp. at high effort — gathers extensive context / multi-thread research unprompted. Over-trigger risk: models responsive to system prompt; "If in doubt, use X" language now *overtriggers* → soften to "Use X when it helps". Counterweight prompt: "choose an approach and commit to it... avoid revisiting decisions unless contradicted". May over-think (inflated thinking tokens) → lower effort or constrain via prompt. [VERIFIED] | [best practices §overthinking](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices) |
| — Opus 4.6 "subagent predilection" | | | Seeded claim of documented strong subagent predilection: adjacent guidance confirmed (over-exploration, tool over-triggering, "room to think and act across subagents" at xhigh/max on 4.7/4.8) but the *exact* Opus 4.6 subagent-predilection wording was not located in fetched pages [UNVERIFIED]. Note Opus 5 has an official "subagent control" prompting section; GPT-6 Astra has the *opposite* (under-delegates). | see effort doc + Astra guide |
| **Claude Opus 4.7** | off until `adaptive` | default `high`; **start `xhigh` for coding/agentic**; `max` only with eval headroom | Respects effort levels *more strictly* than 4.6 (esp. low/medium: scopes to what was asked; shallow reasoning on hard problems → raise effort, don't prompt around; `max` can **overthink** on structured-output tasks). At xhigh/max set `max_tokens` ~64k. | [effort page](https://platform.claude.com/docs/en/build-with-claude/effort) |
| **Claude Opus 5** | **on** (adaptive), display `omitted` | default `high`; sweep low/medium as cost lever | Effort does **not** reliably shorten visible responses → prompt for length. Thinking disable OK ≤ `high` effort, 400 at xhigh/max; with thinking off may emit **tool calls as plain text / internal XML tags**. May under-delegate → prompt for subagent use. | [thinking](https://platform.claude.com/docs/en/build-with-claude/thinking), [effort](https://platform.claude.com/docs/en/build-with-claude/effort) |
| **Claude Fable 5.1** (Mythos 5.1 same, minus prefix check) | **always on**, no config accepted (`disabled`→400, `enabled`→400) | all 5 levels, default `high`; gains biggest at `xhigh`/`max` | **Fewer parallel tool calls in long agent loops** (one call/turn when next reads are only implied) → append one-sentence batching instruction per round (turn-scoped system msg, beta `clear_at:"next_user_message"`). Fewer progress messages → `display:"updates"` + prompt. **Fewer search/retrieval calls at `low` effort** (answers from memory) → raise effort or instruct when to search. Prefill → 400. Forced tool_choice → 400. Reasoning-elicitation in answer text → refusal `stop_details.category:"reasoning_extraction"`. Per-message effort preserves cache. | [Fable 5.1 migration](https://platform.claude.com/docs/en/models/fable-5-1/migration-guide), [best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices) |
| **Claude Sonnet 4.6 / Sonnet 5** | 4.6: off until `adaptive`; 5: on | default `high`; docs advise Sonnet 4.6 **set explicitly** (medium = recommended balance) to avoid latency surprises | Sonnet 5: `disabled` OK; medium ≈ Sonnet 4.6 at high. | [effort](https://platform.claude.com/docs/en/build-with-claude/effort) |
| **GPT-5.5** | reasoning built-in | **`medium` default**; supported: none/low/med/high/xhigh | **Concise, direct, task-oriented by default**; more efficient reasoning (fewer tokens at same effort); literal instruction-following; `text.verbosity=low` → shorter than 5.4 low. **Warning: conflicting instructions, weak stopping criteria, or open-ended tool access + higher effort → overthinking, unnecessary searching, quality regressions.** Preamble prompting improves TTFT; keep `phase` on replayed assistant items. | [GPT-5.5 guidance](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-5.5), [model page](https://developers.openai.com/api/docs/models/gpt-5.5) |
| **GPT-6 Astra** | reasoning built-in | no `none` (400); migrate `none`/`minimal` → `low` | **Asks more clarifying questions / stops early** → needs explicit bias-to-action prompts. **Delegates to subagents LESS than desired** → prompt for delegation. Over-thorough testing on small tasks. Detailed, formatted, list/table-heavy output → prompt for prose. Sensitive to skill-file instructions (audit AGENTS.md); conflicting guidance can pause work. Remove temperature/top_p/logprobs. | [Astra guide](https://developers.openai.com/api/docs/guides/latest-model) |
| **Gemini 3.1 Pro / 3 Flash / 3.1 Flash-Lite** (all preview) | dynamic, `high` default | `thinking_level` low..high (Pro: no `minimal`) | 3.1 Pro floor = `low` (can't turn thinking off). Dynamic: model self-allocates below the level ceiling. Temperature 1.0 mandatory-ish. Long time-to-first-token at `high` (deep reasoning before visible output). | [Gemini 3 guide](https://ai.google.dev/gemini-api/docs/gemini-3) |
| **grok-4.6 / grok-4.5** | always on, **cannot disable**, default `high` | low..xhigh (xhigh: 4.6+) | Default-high = expensive out of the box → harness should SET explicitly even for cheap calls. | [xAI reasoning](https://docs.x.ai/developers/model-capabilities/text/reasoning) |
| **grok-4.20-multi-agent** | multi-agent orchestration | effort = **4 or 16 agents** | 4 = focused queries; 16 = deep research, "significantly more tokens". No max_tokens cap possible. | [xAI multi-agent](https://docs.x.ai/developers/model-capabilities/text/multi-agent) |

---

## 3. Elicitation guidance for a harness: SET vs PROMPT

### Principle
SET the deterministic knobs (API params) for depth/cost/latency; PROMPT for the *behavioral pathologies* each model family documents. Never try to fix with prompting what a parameter controls, and vice versa.

### Anthropic
- **SET**: `output_config.effort` explicitly per workload class (never rely on `high` default); `thinking.type:"adaptive"` on 4.6/4.7/4.8 family; `thinking.display:"summarized"|"updates"` if UI renders reasoning, else `omitted` for TTFT; `max_tokens` as the only hard ceiling (64k+ at xhigh/max on Opus 4.7/4.8/5). **Hold effort + budget constant for the life of a cached conversation**; use per-message effort beta (Fable 5.1/Mythos 5.1/Opus 5) instead of top-level flips.
- **PROMPT** (per-model counterweights): Opus 4.6 → commit-to-approach instruction + soften "MUST use tool X" language; Fable 5.1 → per-round tool-call batching instruction + explicit progress-update requests + "search when unsure" at low effort; Opus 5 → explicit length instructions (effort won't do it) + mitigation prompt if running thinking-disabled; all → "Thinking adds latency..." steering prompt only when adaptive triggers too often on big system prompts.
- **Never**: send `budget_tokens` to 4.7+; prefill on 4.6+; force tool_choice on Fable 5.1/Mythos 5.1 (use `strict:true` + instruction); edit history/`system`/`tools` mid-conversation on Fable 5.1 (append-only, or `drop_block`).

### OpenAI
- **SET**: `reasoning.effort` (medium start for 5.5; `low` before `none` when tools matter; Astra has no `none`); `text.verbosity` independently for length; `configuration_update` items (Astra) for mid-run effort changes instead of re-sending request-level effort (keeps cache).
- **PROMPT**: outcome-first + explicit **stopping conditions / retrieval budgets** (this is the documented antidote to the overthinking warning); bias-to-action + delegation prompts for Astra; preamble requests for TTFT; personality blocks for customer-facing 5.5 (default is terse).
- **Never**: raise effort on tasks with conflicting instructions or open-ended tool access without evals; send temperature/top_p to Astra; drop `phase` on replayed assistant items.

### Google
- **SET**: `thinking_level` only (low for latency, high default for quality); temperature stays 1.0. Never send `thinking_budget` alongside it (400). On Pro, accept the `low` floor — you cannot fully disable thinking.
- **PROMPT**: relatively little — thinking behavior is knob-driven, not promptable; the main harness duty is **plumbing**: pass thought signatures back verbatim in stateless multi-turn function-calling loops or requests fail validation.

### xAI
- **SET**: `reasoning_effort` **explicitly on every call** — default `high` is the most expensive default of the four families; map your harness's effort scale to low/medium for agent-loop chatter. For `grok-4.20-multi-agent`, understand effort = fleet size (4 vs 16): it is a cost multiplier, not a depth dial; there is no `max_tokens` brake.
- **PROMPT**: nothing for disabling reasoning (impossible); strip `presence_penalty`/`frequency_penalty`/`stop` from requests to reasoning models (they error).

### Cross-family invariants for the harness
1. One effort scale, four meanings: Anthropic=whole-response token posture (cache-coupled), OpenAI=thinking depth (cache-decoupled, mid-conversation mutable), Google=thinking ceiling (advisory), xAI=thinking depth or *agent count*.
2. "Off" exists only on: OpenAI `none` (non-Astra), Gemini `minimal` (non-Pro, non-guaranteed), Anthropic `disabled` (Sonnet 5 / Opus 5 ≤ high). xAI: never.
3. Every family has a 400-trap list (Anthropic: budget-on-4.7+, prefill, forced-choice-on-Fable, disabled-on-Fable, xhigh/max-disabled-on-Opus5, budget+level on Gemini, none-on-Astra, penalties-on-Grok). A harness should feature-detect these per model, not per provider.

---

## 4. Source index (all fetched 2026-09-12)
- https://platform.claude.com/docs/en/build-with-claude/thinking
- https://platform.claude.com/docs/en/build-with-claude/extended-thinking
- https://platform.claude.com/docs/en/build-with-claude/effort
- https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices
- https://platform.claude.com/docs/en/models/fable-5-1/migration-guide
- https://developers.openai.com/api/docs/guides/reasoning
- https://developers.openai.com/api/docs/guides/latest-model (GPT-6 Astra + ?model=gpt-5.5)
- https://developers.openai.com/api/docs/models/gpt-5.5
- https://ai.google.dev/gemini-api/docs/gemini-3 (via markdown mirror)
- https://docs.x.ai/developers/model-capabilities/text/reasoning
- https://docs.x.ai/developers/model-capabilities/text/multi-agent

### Residual UNVERIFIED items
1. Opus 4.6 "strong subagent predilection" exact documentation (adjacent evidence only).
2. text.verbosity 560/849/1288 numbers (GPT-5-era cookbook; not re-measured for 5.5/5.6/Astra).
3. Gemini thought-signature missing → exact HTTP 400 wording (requirement confirmed; error string not re-read).

