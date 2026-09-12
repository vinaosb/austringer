# Model Profiles: LLM Behavior Elicitation for Coding Harnesses

FINAL PHASE deliverable - queue item 24 of .omo/research/harness-research.md.
Compiled 2026-09-11. Every claim is sourced from primary vendor documentation
or from a verified section of harness-research.md (cited as sec N).

## 1. Behavior axes

Eight axes on which frontier coding models differ enough to require
per-model harness treatment.

### A1 Premature wrap-up ("context anxiety")
As the context window fills, the model starts concluding work early - it
believes it is near its limit and wraps up. Anthropic measured this
"strongly" in Claude Sonnet 4.5, where compaction alone was not
sufficient and full context resets became essential harness components.
Opus 4.5 largely removed the behavior on its own; harnesses built for
4.5 dropped resets for it. (Anthropic engineering: "Harness design for
long-running application development" 2026-03-24; "Effective harnesses
for long-running agents" 2025-11-26)

### A2 Self-evaluation leniency
Asked to grade its own output, an agent "confidently praises the work",
even when quality is obviously mediocre - worst on subjective tasks
(design), present even on verifiable ones. A separately prompted
evaluator tuned to be skeptical is far more tractable than making a
generator self-critical. (Anthropic harness-design 2026-03-24)

### A3 Lazy coding / edit elision
The GPT-4 Turbo family elided large code sections with "# ... original
code here ..." comments in search/replace formats. Aider's unified-diff
format cut laziness 3x (61% benchmark score). (aider.chat
/docs/unified-diffs.html, /docs/more/edit-formats.html)

### A4 Reasoning-depth miscalibration
GPT-5.1 "can sometimes err on the side of being excessively concise at
the cost of answer completeness" and needs persistence prompting; the
Codex-tuned models auto-calibrate effort (adaptive reasoning) and punish
manual over-tuning; GPT-5.x exposes effort none/low/medium/high/xhigh
with medium recommended as the interactive default. (OpenAI GPT-5.1 and
Codex prompting guides)

### A5 Verbosity / preamble indiscipline
GPT-5 takes a verbosity API parameter; GPT-5.2 needs explicit clamps (XML
verbosity specs) or it drifts long; the Codex models are the opposite -
they IGNORE the verbosity parameter and it hurts behavior, and their
preambles are contract-tuned (1-2 sentence updates, cadence every 1-3
steps, hard floor every 6 steps / 10 tool calls). (OpenAI GPT-5.2 and
Codex prompting guides; Codex CLI docs)

### A6 Subagent-spawn overdrive
Without effort-scaling rules, agentic models over-delegate - spawning
dozens of subagents where a few suffice. Grok Build ships explicit
effort-scaling rules for exactly this (observed 50-subagent spawn
storms). (harness-research.md sec 15)

### A7 Edit-format competence
Model families differ in which edit formats they can reliably emit:
Gemini models routinely break fencing conventions (need diff-fenced with
the path inside the fence); GPT-4 Turbo needs udiff; gpt-3.5-class could
not use diff at all (whole only); DeepSeek is the strongest architect-
mode editor (improves every architect model except Sonnet). (aider
edit-formats.html, benchmarks-1106, architect post 2024-09-26)

### A8 Instruction drift / premature completion
Over long tasks models one-shot instead of iterating, drift from scope
(GPT-5.2 is explicitly "more sensitive to scope drift"), declare victory
early (a later session "looks around, sees progress, and declares the
job done"), and mark features passing without end-to-end verification.
(OpenAI cookbook; Anthropic effective-harnesses failure-mode table)

## 2. Elicitation techniques, mapped to axes

T1 Context resets + structured handoff (A1): for anxiety-prone models,
clear context entirely between sessions and pass a handoff artifact;
compaction alone leaves anxiety intact. Drop resets for Opus 4.5+.
T2 JSON checklists + one-feature-at-a-time (A8): an initializer writes a
feature list as JSON (models overwrite Markdown checklists more readily
than JSON) with every feature marked failing; each session fixes one
feature and may only flip a passes flag; removal is forbidden by prompt
contract ("unacceptable to remove or edit tests").
T3 Generator/evaluator separation (A2): split producer and judge into
two agents with separately tuned prompts; negotiate a sprint contract
(defining "done") before code is written; keep the evaluator only where
the task sits beyond the model's solo capability boundary - it is
overhead otherwise.
T4 Per-model edit format (A3, A7): whole / diff / diff-fenced / udiff /
editor-diff / editor-whole selected per model family (see profiles).
T5 Reasoning budget controls (A4): OpenAI reasoning_effort ladder
(none-xhigh; plan-mode effort may exceed session effort); Gemini 2.5
thinking_budget (128-32768 or -1 dynamic; 0 invalid for Pro) vs Gemini 3
thinking_level - the parameter generations are mutually exclusive, so
harnesses need a name-based vendor adapter; in-session escalation
keywords (Claude Code "ultrathink") or slash commands (/think) beat
config-file edits.
T6 Verbosity controls (A5): API verbosity parameter for GPT-5, XML
clamps for 5.2, NOTHING for Codex models (they ignore it harmfully),
Claude Code output styles for persona-level shaping.
T7 Tool-design compensation (A7): apply_patch-style tools for precise
edits; hashline anchors lift weak editors from 6.7% to 68.3% edit
success (Grok Build, sec 15); keep ripgrep available in the environment.
T8 Session rituals + structured artifacts (A8): fixed session start (pwd,
read progress notes, read feature list, git log, run init.sh, basic
smoke test); end every session with a commit + progress update so the
next window starts from a clean, documented state.
T9 Persistence prompting (A4): explicitly instruct persistence and
completeness for 5.1-class concision.
T10 Effort-scaling rules (A6): hard caps plus explicit escalation
criteria for subagent fan-out.

## 3. Per-model quick profiles

Claude Sonnet 4.5 - A1 strong (resets essential, compaction
insufficient); otherwise the reference harness citizen.
Claude Opus 4.5 - A1 largely gone (drop resets); needs fewer steps than
Sonnet 4.5 on agent benchmarks (Junie testing).
Claude Opus 4.6 - better long-context retrieval, longer task stamina,
catches own mistakes; strip scaffold that 4.5 needed (Anthropic dropped
sprints and moved QA to end-of-run).
GPT-5 - verbosity parameter + reasoning_effort; temperature ignored at
high effort; validate tool parameters per route.
GPT-5.1 - none reasoning mode for low latency; errs concise; persistence
prompting helps.
GPT-5.2 / 5.2-Codex - scope-drift sensitive; XML verbosity clamps;
xhigh effort tier exists.
GPT-5-Codex / 5.3-Codex - adaptive reasoning default (do not micro-
manage effort); DO NOT pass verbosity; short developer messages (~40%
shorter than the GPT-5 reference); preamble cadence pre-tuned; first-
class compaction; escalate approvals promptly or it stalls.
GPT-5.4 - unified frontier line; effort none-xhigh; native computer
use; same tool patterns as Codex.
Gemini 2.5 - thinking_budget 128-32768 or -1; Pro cannot disable
thinking (sending 0 errors - Cline bug 7735); edit format diff-fenced.
Gemini 3+ - thinking_level replaces budget; parameters mutually
exclusive across generations; harness needs a name-based adapter.
GPT-4 Turbo class - udiff mandatory (laziness). GPT-3.5 class - whole
format only.
DeepSeek - reasoner variants need explicit reasoning-token handling
(sec 16); strongest architect-mode editor.
Grok - hashline anchoring rescues weak editors; effort-scaling rules
for subagent storms (sec 15).

## 4. Verified harness adaptations (cross-reference)

aider - per-model edit formats incl. diff-fenced (Gemini), udiff (GPT-4
Turbo), editor-* prompts for architect mode; --edit-format override.
Grok Build (sec 15) - hashline anchors (6.7% to 68.3%), effort-scaling
rules, tool-behavior version registry per model.
Factory Droid (sec 16) - complexity-tier subagent routing; reasoning-
token handling for o-series / deepseek-reasoner.
Anthropic reference harnesses - initializer + coding agent, JSON
feature lists, context resets (Sonnet 4.5 era) dropped for Opus 4.5+,
generator/evaluator split, sprint contracts.
Codex CLI - model_verbosity / model_reasoning_effort /
tool_output_token_limit / plan_mode_reasoning_effort / auto-compact
threshold (~85-90% of context window).
Gemini CLI - lacks in-session thinking control (issue 25122, parity gap
vs ultrathink and /think).
Cline - parameter adapters must respect per-model constraints (bug
7735: thinkingBudget 0 sent to Gemini 2.5 Pro).

## 5. Launcher balancing for our harness

Profile schema (per model id):
  edit_format:      whole | diff | diff-fenced | udiff | hashline
  thinking_param:   none | effort(none-xhigh) | budget(128-32768,-1) |
                    level(low-high) | adaptive
  verbosity_param:  api | xml-clamp | forbidden | output-style
  preamble:         default | cadence(1-3) | contract
  context_strategy: resets+handoff | compaction | hybrid
  drift_guards:     json-checklist | sprint-contract | none
  anchors:          hashline | search-replace
Selection rules:
  R1 Unknown-model default: diff + verify-after-apply + compaction.
  R2 Vendor adapter keyed on model-name generation (Gemini 2.5 vs 3+;
     GPT-5 vs Codex-tuned), never on user guesswork.
  R3 Evaluator agent spun up only when task complexity exceeds the
     model's verified solo boundary (T3).
  R4 Thinking escalation via in-session keyword or slash command, not
     restart.
  R5 Respect parameter legality: never 0 to Gemini 2.5 Pro; no
     verbosity to Codex models; no temperature at high effort.
  R6 Effort caps on subagent fan-out with explicit escalation criteria.

## 6. Sources
- anthropic.com/engineering/effective-harnesses-for-long-running-agents
  (2025-11-26)
- anthropic.com/engineering/harness-design-long-running-apps (2026-03-24)
- aider.chat: /docs/more/edit-formats.html, /docs/unified-diffs.html,
  /2024/09/26/architect.html, /docs/benchmarks-1106.html
- developers.openai.com/cookbook: Codex prompting guide, GPT-5.1 guide
- ai.google.dev/gemini-api/docs/thinking; gemini-cli issues 25122 and
  15533; cline issue 7735
- harness-research.md sec 15 (Grok Build), sec 16 (Factory Droid)
