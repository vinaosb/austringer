---
name: audit-ai-cost
description: >-
  AI token cost estimation auditing: LLM API call counting, token usage estimation,
  cache strategy evaluation, prompt optimization, model selection review,
  and cost optimization recommendations across codebases integrating AI/LLM services.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# AI Token Cost Estimation Audit

> **Note**: This squad may return 0 findings if no AI/LLM integration is found in the codebase.
> This is a valid result — not all projects use AI services.

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- LLM API call counting — identify all AI/LLM API calls in codebase, count per module
- Token usage estimation — estimate input/output tokens per call, per module, per run
- Cache strategy evaluation — assess caching effectiveness for repeated prompts, embedding caching
- Cost optimization recommendations — batch API calls, use smaller models, prompt compression
- Token budget analysis — total cost per operation, monthly projected cost, cost sensitivity
- Rate limiting impact — API rate limits affecting throughput and cost efficiency
- Model selection optimization — flag uses of expensive models where cheaper alternatives suffice
- Prompt length optimization — identify oversized prompts, unnecessary context, redundant instructions
- Batch processing opportunities — identify serial API calls that could be parallelized or batched
- Streaming vs non-streaming cost analysis — verify streaming used where appropriate to reduce timeout costs
- Embedding cost audit — verify embedding dimensions and model selection for vector operations
- Retry/backoff cost analysis — verify retry strategies don't amplify token costs
- .NET Semantic Kernel / HttpClient LLM integration — verify token counting and caching
- Python OpenAI/Anthropic SDK usage — verify token tracking and async batching
- JavaScript/TypeScript AI SDK usage — verify streaming and caching patterns

### DO NOT USE FOR

- Financial model calculations (use `audit-financial-model`)
- Infrastructure/DevOps costs (use `audit-cicd`)
- Security of API key storage (use `audit-security`)
- Dependency vulnerabilities in AI SDKs (use `audit-dependencies`)
- Observability of LLM calls (use `audit-observability`)

---

## Section 2: Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

### 2.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go source files | `*.go`, `go.mod` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle` | Java |
| Rust source files | `*.rs`, `Cargo.toml` | Rust |

### 2.2 AI/LLM Integration Detection

| Signal | Pattern | Provider/Library |
|--------|---------|------------------|
| OpenAI SDK | `openai`, `OpenAIClient`, `ChatCompletion` | OpenAI |
| Anthropic SDK | `anthropic`, `ClaudeClient`, `Messages` | Anthropic |
| Semantic Kernel | `Microsoft.SemanticKernel`, `Kernel`, `IPromptTemplate` | Microsoft |
| LangChain | `langchain`, `ChatOpenAI`, `LLMChain` | LangChain |
| LlamaIndex | `llama_index`, `VectorStoreIndex` | LlamaIndex |
| Azure OpenAI | `AzureOpenAIClient`, `OpenAIAsyncClient` | Azure OpenAI |
| Ollama | `ollama`, `OllamaClient` | Ollama (local) |
| HuggingFace | `transformers`, `HuggingFaceEndpoint` | HuggingFace |
| AI SDK (JS) | `ai`, `generateText`, `streamText` from `ai` package | Vercel AI SDK |
| Google AI | `google-generativeai`, `genAI`, `GenerativeModel` | Google Gemini |

### 2.3 Discovery Procedure

1. **Scan root directory** for all language signal files.
2. **Identify AI/LLM integrations** by searching for SDK imports and client instantiations.
3. **If no AI/LLM integration found**, emit 0-findings summary and STOP.
4. **Identify frameworks** by config file analysis.
5. **Select the matching checklist** from Section 3 based on detected technologies.
6. **If multiple AI providers detected**, audit each independently.

---

## Section 3: Technology-Specific Checklists

### 3.1 .NET/C# AI Cost Checklist

- [ ] **Token counting accuracy**: Verify token counts are tracked per request. Use
  `Usage.PromptTokens` and `Usage.CompletionTokens` from API responses, not estimates.
- [ ] **API call counting**: Count total API calls per operation. Flag operations with
  >1 call where batching could reduce count.
- [ ] **Caching strategy**: Repeated identical prompts cached (in-memory or distributed).
  No re-calling LLM for same input within cache TTL.
- [ ] **Model selection**: Flag `gpt-4` / `gpt-4o` usage where `gpt-4o-mini` or `gpt-3.5-turbo`
  would suffice (classification, extraction, simple Q&A).
- [ ] **Prompt length optimization**: System prompts + user prompts measured. Flag prompts
  exceeding 2000 tokens where truncation is feasible.
- [ ] **Streaming usage**: Streaming API used for long responses to avoid timeout costs
  and improve UX. Non-streaming only for short responses.
- [ ] **Retry cost control**: Retry policies include max-retry and backoff. No infinite
  retry loops that amplify token costs on repeated failures.
- [ ] **Semantic Kernel usage**: Verify prompt templates are cached, not recompiled per call.
  Plugin functions efficiently orchestrated.

### 3.2 Python AI Cost Checklist

- [ ] **OpenAI SDK token tracking**: `response.usage.total_tokens` captured and logged for
  every call. No silent token consumption.
- [ ] **Async batching**: Multiple independent LLM calls executed concurrently with
  `asyncio.gather()` instead of sequentially.
- [ ] **LangChain cost tracking**: `Callbacks` configured for token counting. Cost tracked
  per chain/agent execution.
- [ ] **Embedding efficiency**: Embedding model dimensions appropriate for use case. Flag
  3072-dim embeddings where 768-dim would suffice.
- [ ] **Prompt template caching**: Prompt templates compiled once, reused. No re-building
  prompt templates per request.
- [ ] **Rate limit handling**: API calls respect rate limits. No 429 errors causing retries
  with exponential backoff that amplify costs.

### 3.3 JavaScript/TypeScript AI Cost Checklist

- [ ] **Vercel AI SDK usage**: `generateText` / `streamText` used correctly. Streaming
  preferred for user-facing responses.
- [ ] **Token counting**: Middleware or callback capturing token usage per request.
- [ ] **Client-side caching**: Repeated queries cached client-side (SWR, React Query) to
  avoid redundant API calls.
- [ ] **Prompt size validation**: User input validated and truncated before sending to LLM.
  No unbounded prompt growth from user input.
- [ ] **Edge runtime optimization**: LLM calls from edge runtime where possible to reduce
  latency and associated timeout costs.

### 3.4 Cross-Language AI Cost Checklist

- [ ] **Batch processing opportunities**: Identify serial API calls that could be batched.
  OpenAI batch API for non-real-time processing.
- [ ] **Cost per operation tracking**: Each operation type has estimated token cost. No
  operations with unknown token cost in production.
- [ ] **Model tier strategy**: Clear mapping of task complexity → model tier. Simple tasks
  (classification, extraction) use cheaper models.
- [ ] **Context window optimization**: Long contexts only when necessary. Summarization or
  retrieval used to reduce context size instead of full document insertion.
- [ ] **Output length limits**: `max_tokens` set appropriately for each call. No unbounded
  output generation.
- [ ] **Cost alerting**: Token usage monitored with alerts at budget thresholds. No silent
  cost overruns.

---

## Section 4: Anti-Patterns

### 4.1 .NET/C# AI Cost Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | No token tracking | high | API responses consumed without capturing usage data | Cannot measure or optimize costs | Log `response.Usage.PromptTokens` + `CompletionTokens` per call |
| 2 | Unbounded token generation | critical | LLM calls without `max_tokens` limit | Single call can consume entire context window budget | Set `max_tokens` per call based on expected output length |
| 3 | GPT-4 for simple tasks | medium | Using `gpt-4` / `gpt-4o` for classification, extraction, formatting | 10-30x cost premium over smaller models | Use `gpt-4o-mini` or `gpt-3.5-turbo` for simple tasks |
| 4 | No caching for repeated prompts | high | Identical prompts re-sent to LLM on every request | Redundant API costs, increased latency | Cache responses with deterministic prompts |
| 5 | Retries without cost awareness | high | Exponential backoff retrying with full prompt on failure | Failed calls still consume input tokens, retries multiply cost | Track cost of retries, consider smaller retry prompts |

### 4.2 Python AI Cost Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Synchronous API calls in loop | high | `for item in items: openai.ChatCompletion.create(...)` | Slow and expensive — no parallelism | Use `asyncio.gather()` with `AsyncOpenAI` client |
| 2 | Oversized embeddings | medium | 3072-dim embeddings for simple similarity search | Higher storage cost, slower search, unnecessary precision | Use smaller model (e.g., `text-embedding-3-small` at 1536 or 768) |
| 3 | LangChain without callbacks | medium | No token/cost callbacks on chains and agents | Silent cost accumulation | Configure callbacks for token tracking |
| 4 | Full document in context | high | Entire documents passed as context instead of retrieval | Token cost proportional to document size, often exceeds budget | Use RAG — retrieve relevant chunks, not full documents |

### 4.3 JavaScript/TypeScript AI Cost Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | No client-side caching | medium | Every user action triggers new LLM API call | Redundant costs for repeated queries | Cache responses with SWR/React Query with appropriate TTL |
| 2 | Unvalidated prompt size | high | User input directly concatenated into prompts without truncation | Single malicious/large input can blow token budget | Validate and truncate user input before LLM call |
| 3 | Non-streaming for long responses | medium | `generateText()` for responses expected >500 tokens | Timeout risk, poor UX, wasted tokens on timeout | Use `streamText()` for responses >500 tokens |

### 4.4 Cross-Language AI Cost Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | No cost monitoring | high | Token usage not tracked, no alerts on budget thresholds | Silent cost overruns, budget surprises | Track tokens per call, aggregate per operation, alert on thresholds |
| 2 | Single model for all tasks | medium | One model used for everything from summarization to code generation | Overpaying for simple tasks, underperforming on complex ones | Map task complexity → model tier |
| 3 | No prompt versioning | medium | Prompts changed without tracking impact on token usage | Cost regression without detection | Version prompts, track token usage per version |

---

## Section 5: Code Examples (Bad/Good Pairs)

### Example 1: Token Counting and Caching (.NET/C#)

```csharp
// BAD: No token tracking, no caching — every call is a new API request
public async Task<string> SummarizeAsync(string text)
{
    var response = await _client.GetChatCompletionsAsync(
        new ChatCompletionOptions { Messages = [new("system", "Summarize"), new("user", text)] });
    return response.Content;
    // No usage tracking, no cache, cost unknown
}

// GOOD: Token tracking with caching
public class LlmService
{
    private readonly IMemoryCache _cache;
    private readonly ILogger<LlmService> _logger;
    private const string Model = "gpt-4o-mini"; // Right-sized model

    public async Task<string> SummarizeAsync(string text)
    {
        var cacheKey = $"summary:{Hash(text)}";
        if (_cache.TryGetValue(cacheKey, out string cached))
            return cached;

        var response = await _client.GetChatCompletionsAsync(
            new ChatCompletionOptions
            {
                Model = Model,
                MaxTokens = 500, // Bounded output
                Messages = [new("system", "Summarize concisely"), new("user", text)]
            });

        _logger.LogInformation(
            "LLM call: model={Model}, prompt={Prompt}, completion={Completion}, total={Total}",
            Model, response.Usage.PromptTokens, response.Usage.CompletionTokens,
            response.Usage.TotalTokens);

        var result = response.Content;
        _cache.Set(cacheKey, result, TimeSpan.FromHours(1));
        return result;
    }
}
```

### Example 2: Async Batching for Parallel LLM Calls (Python)

```python
# BAD: Sequential API calls — slow and wasteful
import openai

def process_items(items: list[str]) -> list[str]:
    results = []
    for item in items:
        response = openai.chat.completions.create(
            model="gpt-4o-mini",
            messages=[{"role": "user", "content": f"Classify: {item}"}]
        )
        results.append(response.choices[0].message.content)
    return results  # N sequential API calls

# GOOD: Async parallel calls with token tracking
import asyncio
from openai import AsyncOpenAI
import logging

logger = logging.getLogger(__name__)
client = AsyncOpenAI()

async def classify_single(item: str, semaphore: asyncio.Semaphore) -> tuple[str, int]:
    async with semaphore:
        response = await client.chat.completions.create(
            model="gpt-4o-mini",       # Right-sized model
            max_tokens=50,             # Bounded output for classification
            messages=[{"role": "user", "content": f"Classify in one word: {item}"}]
        )
        tokens = response.usage.total_tokens
        logger.info("Classification tokens: %d", tokens)
        return response.choices[0].message.content, tokens

async def process_items(items: list[str]) -> list[str]:
    semaphore = asyncio.Semaphore(10)  # Limit concurrency
    results = await asyncio.gather(*[classify_single(item, semaphore) for item in items])
    total_tokens = sum(t for _, t in results)
    logger.info("Total tokens for %d items: %d", len(items), total_tokens)
    return [r for r, _ in results]
```

### Example 3: Model Selection Strategy (.NET/C#)

```csharp
// BAD: GPT-4 for everything regardless of task complexity
public async Task<string> CallLlmAsync(string prompt)
{
    var response = await _client.GetChatCompletionsAsync(
        new ChatCompletionOptions { Model = "gpt-4o", Messages = [new("user", prompt)] });
    return response.Content; // Expensive model for all tasks
}

// GOOD: Task-aware model selection
public static class ModelSelector
{
    public const string Complex = "gpt-4o";           // Reasoning, code gen, analysis
    public const string Standard = "gpt-4o-mini";     // Classification, extraction, formatting
    public const string Embedding = "text-embedding-3-small";

    public static string ForTask(TaskComplexity complexity) => complexity switch
    {
        TaskComplexity.High => Complex,
        TaskComplexity.Low => Standard,
        _ => Standard
    };
}

public async Task<string> ClassifyAsync(string input)
{
    // Classification is Low complexity — use cheaper model
    var response = await _client.GetChatCompletionsAsync(
        new ChatCompletionOptions
        {
            Model = ModelSelector.ForTask(TaskComplexity.Low),
            MaxTokens = 50
        });
    return response.Content;
}
```

### Example 4: Prompt Size Validation (JavaScript/TypeScript)

```typescript
// BAD: Unbounded prompt from user input
async function analyzeDocument(userInput: string) {
  const response = await generateText({
    model: openai("gpt-4o"),
    prompt: `Analyze this: ${userInput}`, // userInput could be 100K chars
  });
  return response.text;
}

// GOOD: Validate and truncate input, use appropriate model
const MAX_INPUT_CHARS = 10_000;
const MAX_OUTPUT_TOKENS = 1000;

async function analyzeDocument(userInput: string) {
  if (userInput.length > MAX_INPUT_CHARS) {
    userInput = userInput.slice(0, MAX_INPUT_CHARS) + "\n[...truncated]";
  }

  const response = await streamText({
    model: openai("gpt-4o-mini"), // Right-sized for analysis
    maxTokens: MAX_OUTPUT_TOKENS,
    prompt: `Analyze this concisely: ${userInput}`,
  });

  return response.text;
}
```

---

## Section 6: Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 6.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "LLM call without max_tokens limit — unbounded token generation possible",
  "recommendation": "Set max_tokens parameter based on expected output length. For classification: 50, summarization: 500, generation: 2000",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "performance|architecture|maintainability|testing|observability"
}
```

### 6.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 7 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the problematic code fragment. No hedging.
- **recommendation**: Actionable. Include estimated cost impact when possible.
- **technology**: Primary technology of the finding.
- **category**: For AI cost findings, use `performance` for cost issues,
  `architecture` for caching/model selection, `observability` for tracking gaps.

### 6.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-ai-cost",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Python"],
    "totalFindings": 6,
    "bySeverity": {
      "critical": 1,
      "high": 2,
      "medium": 3,
      "low": 0
    },
    "byCategory": {
      "performance": 3,
      "architecture": 2,
      "observability": 1
    },
    "filesScanned": 15,
    "filesWithFindings": 5,
    "aiProviders": ["OpenAI", "Azure OpenAI"],
    "estimatedMonthlyTokens": 5000000,
    "estimatedMonthlyCost": "$45.00"
  }
}
```

---

## Section 7: Severity Guidelines

### 7.1 Base Severity Definitions

| Level | Definition | AI Cost Context Example |
|-------|-----------|------------------------|
| **critical** | Unbounded token consumption, cost runaway | LLM calls in loops without limits, no max_tokens, no rate limiting |
| **high** | Significant cost waste, no observability | No token tracking, GPT-4 for simple tasks, no caching for repeated prompts |
| **medium** | Suboptimal cost efficiency | Oversized prompts, missing batch optimization, oversized embeddings |
| **low** | Best practice improvement | Missing prompt versioning, documentation gaps |

### 7.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in a loop/iteration pattern | +1 level | All technologies |
| Finding affects real-time user-facing path | +1 level | All technologies |
| Finding has no cost monitoring at all | +1 level | All technologies |
| Finding involves API keys in source code | +1 level | All technologies |

### 7.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test/demo code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control | -1 level (min low) | All technologies |
| Finding is in development-only tooling | -1 level (min low) | All technologies |

---

## Section 8: Integration Notes

### 8.1 Squad Dependency Graph

```
audit-security ────────────────────────┐
audit-dependencies ────────────────────┤
audit-observability ───────────────────┤
audit-ai-cost ◄────────────────────────┘  (THIS SQUAD)
audit-financial-model ─────────────────┘ (overlaps with this squad)
audit-consolidated ◄─────────────────── (consumes this squad's output)
audit-consistency-coherence            (consumes this squad's output)
```

### 8.2 Upstream Dependencies

- **audit-security**: API key storage and LLM endpoint security
- **audit-dependencies**: AI SDK versions and vulnerabilities
- **audit-observability**: LLM call tracing and metrics instrumentation

### 8.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings
- **audit-financial-model**: May reference AI cost findings for total cost projections

### 8.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-ai-cost | audit-financial-model | AI-cost focuses on token/LLM costs; financial-model focuses on financial calculations |
| audit-ai-cost | audit-observability | AI-cost focuses on cost optimization; observability focuses on tracing/metrics setup |
| audit-ai-cost | audit-security | AI-cost focuses on cost efficiency; security focuses on API key exposure and prompt injection |

### 8.5 Recommended Execution Order

1. **audit-security** — API key storage
2. **audit-dependencies** — AI SDK versions
3. **audit-observability** — LLM call tracing setup
4. **audit-ai-cost** — THIS SQUAD: token cost optimization
5. **audit-consolidated** — Aggregate all findings

### 8.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 6 format for aggregation.
