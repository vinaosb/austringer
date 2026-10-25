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

> **Note**: May return 0 findings if no AI/LLM integration found — valid.

---

## USE FOR / DO NOT USE FOR

### USE FOR

- LLM API call counting — identify AI/LLM calls, count per module
- Token usage estimation — input/output tokens per call, module, run
- Cache strategy evaluation — repeated prompts, embedding caching
- Cost optimization — batch calls, smaller models, prompt compression
- Token budget analysis — cost/op, monthly projected, sensitivity
- Rate limiting impact — throughput vs cost efficiency
- Model selection — flag expensive models where cheaper suffices
- Prompt length optimization — oversized prompts, redundant instructions
- Batch processing — serial calls that could be parallelized
- Streaming vs non-streaming cost — streaming for long responses
- Embedding cost audit — dimensions vs model selection
- Retry/backoff cost — verify retries don't amplify costs
- .NET Semantic Kernel / HttpClient LLM — token counting, caching
- Python OpenAI/Anthropic SDK — token tracking, async batching
- JS/TS AI SDK — streaming, caching patterns

### DO NOT USE FOR

- Financial models (use `audit-financial-model`)
- Infra/DevOps costs (use `audit-cicd`)
- API key security (use `audit-security`)
- AI SDK vulnerabilities (use `audit-dependencies`)
- LLM observability (use `audit-observability`)

---

## Target Discovery

Auto-detect. No hardcoded names.

### Language Detection

| Signal | File Pattern | Lang |
|--------|-------------|------|
| C# | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JS | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TS | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go | `*.go`, `go.mod` | Go |
| Java | `*.java`, `pom.xml`, `build.gradle` | Java |
| Rust | `*.rs`, `Cargo.toml` | Rust |

### AI/LLM Detection

| Signal | Pattern | Provider |
|--------|---------|----------|
| OpenAI SDK | `openai`, `OpenAIClient`, `ChatCompletion` | OpenAI |
| Anthropic SDK | `anthropic`, `ClaudeClient`, `Messages` | Anthropic |
| Semantic Kernel | `Microsoft.SemanticKernel`, `Kernel`, `IPromptTemplate` | Microsoft |
| LangChain | `langchain`, `ChatOpenAI`, `LLMChain` | LangChain |
| LlamaIndex | `llama_index`, `VectorStoreIndex` | LlamaIndex |
| Azure OpenAI | `AzureOpenAIClient`, `OpenAIAsyncClient` | Azure OpenAI |
| Ollama | `ollama`, `OllamaClient` | Ollama (local) |
| HuggingFace | `transformers`, `HuggingFaceEndpoint` | HuggingFace |
| AI SDK (JS) | `ai`, `generateText`, `streamText` | Vercel AI SDK |
| Google AI | `google-generativeai`, `genAI`, `GenerativeModel` | Google Gemini |

### Discovery

1. Scan root for lang signals
2. Identify AI/LLM via SDK imports + client instantiations
3. No AI found → 0-findings summary, STOP
4. Identify frameworks
5. Select checklist per detected tech
6. Multiple providers → audit each independently

---

## Checklists

### .NET/C# AI Cost

- [ ] **Token counting**: Track per request via `Usage.PromptTokens` + `Usage.CompletionTokens`, not estimates
- [ ] **API call counting**: Count/op. Flag >1 call where batching possible
- [ ] **Caching**: Identical prompts cached (in-memory/distributed). No re-call within TTL
- [ ] **Model selection**: Flag `gpt-4`/`gpt-4o` where `gpt-4o-mini` suffices (classification, extraction, Q&A)
- [ ] **Prompt length**: System + user prompts measured. Flag >2000 tokens
- [ ] **Streaming**: Used for long responses. Non-streaming only for short
- [ ] **Retry cost**: max-retry + backoff. No infinite loops amplifying costs
- [ ] **Semantic Kernel**: Prompt templates cached. Plugin functions efficient

### Python AI Cost

- [ ] **Token tracking**: `response.usage.total_tokens` captured + logged per call
- [ ] **Async batching**: `asyncio.gather()` for concurrent calls, not sequential
- [ ] **LangChain cost**: `Callbacks` configured for token counting per chain/agent
- [ ] **Embedding dimensions**: Flag 3072-dim where 768-dim suffices
- [ ] **Template caching**: Compiled once, reused. No re-build per request
- [ ] **Rate limits**: Respect limits. No 429 cascade amplifying costs

### JS/TS AI Cost

- [ ] **Vercel AI SDK**: `generateText`/`streamText` used correctly. Streaming preferred for user-facing
- [ ] **Token counting**: Middleware/callback capturing per request
- [ ] **Client caching**: SWR/React Query for repeated queries
- [ ] **Prompt size**: User input validated + truncated. No unbounded growth
- [ ] **Edge runtime**: LLM calls from edge where possible for latency reduction

### Cross-Language AI Cost

- [ ] **Batch processing**: Serial calls that could be batched (OpenAI batch API for non-real-time)
- [ ] **Cost/op tracking**: Each op has estimated token cost. No unknown costs in prod
- [ ] **Model tier strategy**: Task complexity → model tier. Simple → cheap models
- [ ] **Context window**: Only when necessary. Summarization/retrieval to reduce size
- [ ] **Output limits**: `max_tokens` set per call. No unbounded output
- [ ] **Cost alerting**: Token usage monitored with budget alerts. No silent overruns

---

## Anti-Patterns

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | No token tracking | high | Usage data not captured | Can't measure/optimize | Log `Usage.PromptTokens` + `CompletionTokens` |
| 2 | Unbounded generation | critical | No `max_tokens` | Single call uses entire context budget | Set `max_tokens` per call |
| 3 | GPT-4 for simple tasks | medium | `gpt-4`/`gpt-4o` for classification/extraction | 10-30x cost premium | Use `gpt-4o-mini` |
| 4 | No caching | high | Identical prompts re-sent | Redundant costs, latency | Cache deterministic prompts |
| 5 | Retries without cost awareness | high | Full prompt retry on failure | Failed calls consume input tokens | Track retry cost, smaller retry prompts |

### Python Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | Sync loop calls | high | `for item: openai.ChatCompletion.create(...)` | Slow, expensive, no parallelism | `asyncio.gather()` + `AsyncOpenAI` |
| 2 | Oversized embeddings | medium | 3072-dim for simple search | Higher storage, slower, unnecessary | `text-embedding-3-small` at 1536/768 |
| 3 | LangChain no callbacks | medium | No token/cost callbacks | Silent accumulation | Configure callbacks |
| 4 | Full document context | high | Entire docs as context instead of retrieval | Token cost = doc size, exceeds budget | RAG — retrieve chunks |

### JS/TS Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | No client caching | medium | Every action → new LLM call | Redundant costs | SWR/React Query with TTL |
| 2 | Unvalidated prompt size | high | User input directly concatenated | Malicious/large input blows budget | Validate + truncate input |
| 3 | Non-streaming for long responses | medium | `generateText()` for >500 tokens | Timeout risk, wasted tokens | `streamText()` |

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | No cost monitoring | high | Tokens untracked, no alerts | Silent overruns, budget surprises | Track per call, aggregate, alert at thresholds |
| 2 | Single model for all tasks | medium | One model for everything | Overpay simple, underperform complex | Map complexity → tier |
| 3 | No prompt versioning | medium | Changes untracked | Cost regression undetected | Version prompts, track token usage |

---

## Code Examples (Bad/Good)

### Example 1: Token Counting + Caching (.NET/C#)

```csharp
// BAD: No tracking, no caching
public async Task<string> SummarizeAsync(string text)
{
    var response = await _client.GetChatCompletionsAsync(
        new ChatCompletionOptions { Messages = [new("system", "Summarize"), new("user", text)] });
    return response.Content;
}

// GOOD: Tracking + caching
public class LlmService
{
    private readonly IMemoryCache _cache;
    private readonly ILogger<LlmService> _logger;
    private const string Model = "gpt-4o-mini";

    public async Task<string> SummarizeAsync(string text)
    {
        var cacheKey = $"summary:{Hash(text)}";
        if (_cache.TryGetValue(cacheKey, out string cached))
            return cached;

        var response = await _client.GetChatCompletionsAsync(
            new ChatCompletionOptions
            {
                Model = Model, MaxTokens = 500,
                Messages = [new("system", "Summarize concisely"), new("user", text)]
            });

        _logger.LogInformation("LLM call: model={Model}, prompt={Prompt}, total={Total}",
            Model, response.Usage.PromptTokens, response.Usage.TotalTokens);

        _cache.Set(cacheKey, response.Content, TimeSpan.FromHours(1));
        return response.Content;
    }
}
```

### Example 2: Async Batching (Python)

```python
# BAD: Sequential
def process_items(items: list[str]) -> list[str]:
    results = []
    for item in items:
        response = openai.chat.completions.create(
            model="gpt-4o-mini",
            messages=[{"role": "user", "content": f"Classify: {item}"}]
        )
        results.append(response.choices[0].message.content)
    return results

# GOOD: Async parallel
import asyncio
from openai import AsyncOpenAI
client = AsyncOpenAI()

async def classify_single(item: str, semaphore: asyncio.Semaphore) -> tuple[str, int]:
    async with semaphore:
        response = await client.chat.completions.create(
            model="gpt-4o-mini", max_tokens=50,
            messages=[{"role": "user", "content": f"Classify in one word: {item}"}]
        )
        return response.choices[0].message.content, response.usage.total_tokens

async def process_items(items: list[str]) -> list[str]:
    semaphore = asyncio.Semaphore(10)
    results = await asyncio.gather(*[classify_single(item, semaphore) for item in items])
    return [r for r, _ in results]
```

### Example 3: Model Selection (.NET/C#)

```csharp
// BAD: GPT-4 for everything
public async Task<string> CallLlmAsync(string prompt)
{
    var response = await _client.GetChatCompletionsAsync(
        new ChatCompletionOptions { Model = "gpt-4o", Messages = [new("user", prompt)] });
    return response.Content;
}

// GOOD: Task-aware selection
public static class ModelSelector
{
    public const string Complex = "gpt-4o";
    public const string Standard = "gpt-4o-mini";
    public const string Embedding = "text-embedding-3-small";
    public static string ForTask(TaskComplexity c) => c switch
    {
        TaskComplexity.High => Complex,
        _ => Standard
    };
}
```

### Example 4: Prompt Size Validation (JS/TS)

```typescript
// BAD: Unbounded prompt
async function analyzeDocument(userInput: string) {
  const response = await generateText({
    model: openai("gpt-4o"),
    prompt: `Analyze this: ${userInput}`,
  });
  return response.text;
}

// GOOD: Validate + truncate
const MAX_INPUT = 10_000;
async function analyzeDocument(userInput: string) {
  if (userInput.length > MAX_INPUT) userInput = userInput.slice(0, MAX_INPUT) + "\n[...truncated]";
  const response = await streamText({
    model: openai("gpt-4o-mini"), maxTokens: 1000,
    prompt: `Analyze this concisely: ${userInput}`,
  });
  return response.text;
}
```

---

## Output Format

JSON, one finding per object.

### Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "LLM call without max_tokens — unbounded generation possible",
  "recommendation": "Set max_tokens. For classification: 50, summarization: 500, generation: 2000",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "performance|architecture|maintainability|testing|observability"
}
```

### Fields

- **severity**: One of `critical|high|medium|low`. See §7.
- **file**: Relative, forward slashes.
- **line**: 1-based.
- **endLine**: Optional.
- **description**: Factual, specific. No hedging.
- **recommendation**: Actionable. Include cost impact.
- **category**: AI cost: `performance` (cost), `architecture` (caching/model), `observability` (tracking)

### Summary

```json
{
  "summary": {
    "squad": "audit-ai-cost",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Python"],
    "totalFindings": 6,
    "bySeverity": { "critical": 1, "high": 2, "medium": 3, "low": 0 },
    "byCategory": { "performance": 3, "architecture": 2, "observability": 1 },
    "filesScanned": 15, "filesWithFindings": 5,
    "aiProviders": ["OpenAI", "Azure OpenAI"],
    "estimatedMonthlyTokens": 5000000,
    "estimatedMonthlyCost": "$45.00"
  }
}
```

---

## Severity

### Base

| Level | Def | Example |
|-------|-----|---------|
| **critical** | Unbounded consumption, cost runaway | Loops without limits, no max_tokens, no rate limiting |
| **high** | Significant waste, no observability | No token tracking, GPT-4 simple tasks, no caching |
| **medium** | Suboptimal efficiency | Oversized prompts, missing batch, oversized embeddings |
| **low** | Best practice | Missing versioning, doc gaps |

### Escalation

| Trigger | + | Applies |
|---------|---|---------|
| In loop/iteration | +1 | All |
| Real-time user-facing path | +1 | All |
| No cost monitoring at all | +1 | All |
| API keys in source code | +1 | All |

### De-escalation

| Trigger | - | Applies |
|---------|---|---------|
| Test/demo only | -1 (min low) | All |
| Mitigating control exists | -1 (min low) | All |
| Dev-only tooling | -1 (min low) | All |

---

## Integration

### Dependency Graph

```
audit-security ────────────────────────┐
audit-dependencies ────────────────────┤
audit-observability ───────────────────┤
audit-ai-cost ◄────────────────────────┘  (THIS)
audit-financial-model ─────────────────┘ (overlap)
audit-consolidated ◄─────────────────── (consumes)
audit-consistency-coherence            (consumes)
```

### Upstream

- **audit-security**: API key storage, endpoint security
- **audit-dependencies**: AI SDK versions, vulnerabilities
- **audit-observability**: LLM tracing, metrics

### Downstream

- **audit-consolidated**: Aggregates
- **audit-consistency-coherence**: Cross-domain alignment
- **audit-financial-model**: May reference cost projections

### Overlaps

| Squad | Overlap With | Boundary |
|-------|-------------|----------|
| audit-ai-cost | audit-financial-model | AI-cost: token/LLM; Model: financial calc |
| audit-ai-cost | audit-observability | AI-cost: optimization; Observability: tracing/metrics |
| audit-ai-cost | audit-security | AI-cost: efficiency; Security: key exposure, injection |

### Execution

1. **audit-security** — API keys
2. **audit-dependencies** — SDK versions
3. **audit-observability** — LLM tracing
4. **audit-ai-cost** — THIS
5. **audit-consolidated**

### Output Integration

Sequential squads SHOULD:
1. Read previous findings
2. Deduplicate
3. Cross-reference (escalate multi-squad)
4. Output per §6