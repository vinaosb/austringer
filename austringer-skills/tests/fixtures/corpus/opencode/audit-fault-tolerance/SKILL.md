---
name: audit-fault-tolerance
description: >-
  Fault tolerance and resilience auditing: retry logic validation, circuit breaker patterns,
  error handling review, timeout configuration, failover mechanism audit, graceful degradation,
  and resource cleanup verification across all major programming languages and frameworks.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java"]
version: "2.0.0"
---

# Fault Tolerance & Resilience Audit Squad

## USE FOR

- Retry logic validation — verify retry strategies (exponential backoff, max retries, jitter), idempotency keys
- Circuit breaker patterns — verify state machines, fallback logic, recovery testing
- Error handling review — verify unhandled exception coverage, error propagation chains, catch block quality
- Timeout configuration — verify reasonable timeouts at all service boundaries (HTTP, DB, SMTP, external APIs)
- Failover mechanism audit — verify triggers, state sync, split-brain prevention
- Graceful degradation check — system degrades gracefully not hard-failing
- Bulkhead isolation — verify resource pools prevent cascade failures, bounded concurrency
- Resource cleanup — verify `finally`, `using`, `defer` for cleanup
- CancellationToken propagation — all async ops accept and respect tokens
- HttpClient lifecycle — verify IHttpClientFactory usage, proper disposal, connection pool mgmt
- Outbox pattern verification — event publishing uses outbox for at-least-once delivery
- Fire-and-forget safety — background ops have error handling and logging
- **.NET/C#**: Polly resilience pipeline, IHttpClientFactory audit, CancellationToken check, outbox pattern, EF Core connection resiliency
- **Python**: tenacity/backoff retry, asyncio timeout/semaphore, context manager cleanup
- **JavaScript/TypeScript**: AbortController timeout, Promise.allSettled vs all, p-limit/p-queue, React error boundaries
- **Go**: context.WithTimeout, error wrapping, goroutine lifecycle/leak detection, defer cleanup
- **Java**: Resilience4j audit, try-with-resources, ExecutorService shutdown, Future.get timeout
- **Rust**: Result vs panic, unwrap/expect safety, Drop correctness, concurrent access patterns

### DO NOT USE FOR

- Security vulnerabilities (use `audit-security`)
- Test quality and coverage (use `audit-test-coverage`)
- Code style issues (use `audit-code-quality`)
- Architecture layer violations (use `audit-architecture`)
- Observability gaps (use `audit-observability`)

---

## Section 3: Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |

### 3.2 Resilience-Relevant File Detection

| Signal | File Pattern | Resilience Concern |
|--------|-------------|---------------------|
| Polly config | `*Policy*`, `*Resilience*`, `*Pipeline*` | Retry/circuit breaker/timeout config |
| HttpClient | `*Http*`, `*Client*`, `*External*`, `*Provider*` | External service call patterns |
| Message handlers | `*Handler*`, `*Consumer*`, `*Subscriber*` | Message processing resilience |
| Error handling | `*Exception*`, `*Error*`, `*Fault*` | Error types and handling |
| Health checks | `*Health*`, `*Readiness*`, `*Liveness*` | Health monitoring |
| Background jobs | `*Job*`, `*Worker*`, `*Background*`, `*Hosted*` | Background task resilience |
| Database context | `*DbContext*`, `*Repository*`, `*Context*` | DB operation resilience |
| Email/notification | `*Email*`, `*Smtp*`, `*Notification*` | SMTP/notification retry |
| Config | `appsettings*.json`, `*.env`, `*.yaml` | Timeout/retry config values |

### 3.3 Resilience Library Detection

| Signal | Pattern | Detected Library |
|--------|---------|------------------|
| Polly | `*.csproj` containing `Polly`, `Microsoft.Extensions.Http.Resilience` | .NET |
| Tenacity | `pyproject.toml`/`requirements.txt` with `tenacity`, `backoff` | Python |
| opossum | `package.json` with `opossum`, `cockatieljs` | JS |
| gobreaker | `go.mod` with `sony/gobreaker` | Go |
| Resilience4j | `pom.xml`/`build.gradle` with `resilience4j-*` | Java |

### 3.4 Infrastructure Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml` | Container health checks |
| Kubernetes | `*.yaml` with `livenessProbe`/`readinessProbe` | Pod health probes |
| OTel | `otel-collector-config.yml` | Observability for resilience |

### 3.5 Discovery Procedure

1. **Scan root directory** for resilience-related file patterns.
2. **Identify primary language** by source file count.
3. **Detect resilience libraries**: Polly (.NET), resilience4j (Java), tenacity (Python), etc.
4. **Identify external service integration**: HTTP clients, DB connections, SMTP, message queues.
5. **Map error handling patterns**: try/catch, Result types, error middleware.
6. **Detect timeout configuration**: Search for `Timeout`, `CancellationToken`, `deadline`, `context.WithTimeout`.
7. **Select matching checklist** from Section 4 based on technologies.
8. **If multiple languages** (monorepo), run all applicable checklists.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Fault Tolerance Checklist

- [ ] **Polly retry/circuit breaker**: Retry with exponential backoff + jitter. Circuit breaker has appropriate threshold, sampling duration, recovery time. Policies composed into pipeline.
- [ ] **HttpClient timeout and disposal**: IHttpClientFactory used (not `new HttpClient()`). Timeout set. HttpResponseMessage disposed (`using`/`await using`).
- [ ] **CancellationToken propagation**: All async methods accept token. Passed through all downstream calls. No swallowed tokens.
- [ ] **Missing circuit breaker on external services**: Flag `HttpClient.SendAsync` without resilience pipeline.
- [ ] **Unbounded Task.WhenAll without bulkhead**: Search for `Task.WhenAll` on unknown-size collections. Use `SemaphoreSlim` or `Channel<T>`.
- [ ] **Fire-and-forget without error handling**: Search for `_ = Task.Run(`, `_ = SomeAsync()` without error handling. All background ops must log failures.
- [ ] **Catch-all without logging**: `catch (Exception ex)` blocks that don't log or rethrow. Never silently swallow.
- [ ] **Missing finally for resource cleanup**: Streams, connections, IDisposable cleaned in finally/using.
- [ ] **Missing outbox pattern**: Event publish after DB ops must use outbox for at-least-once delivery. Direct publish after SaveChanges can lose events.
- [ ] **SMTP sends without timeout/retry**: Email sending must have timeout and retry policy.
- [ ] **Provider waterfall without health tracking**: When trying providers sequentially, track each failure. Circuit per-provider.
- [ ] **Advisory lock without bounded wait**: DB advisory locks need timeout. Flag `pg_advisory_lock` (unbounded) vs `pg_try_advisory_lock` with retry.
- [ ] **Missing HttpClient timeout**: Default 100s. Override for fast-failing endpoints. Long-polling gets explicit longer timeout.
- [ ] **Task.Delay without cancellation**: Use `Task.Delay(timeout, cancellationToken)`.
- [ ] **EF Core connection resiliency**: Verify `EnableRetryOnFailure` configured.

### 4.2 Python Fault Tolerance Checklist

- [ ] **Retry decorator**: External calls use retry with exponential backoff. No bare `while True: try/except` without backoff.
- [ ] **Timeout on HTTP calls**: `requests.get/post` must use `timeout`. Use `httpx` with timeout for async.
- [ ] **Connection pool management**: DB connections use bounded connection pooling.
- [ ] **Error propagation in async**: `asyncio.gather` uses `return_exceptions=False` intentionally. Know gather vs TaskGroup.
- [ ] **Resource cleanup with context managers**: Files, connections, HTTP sessions use `with`/`async with`.
- [ ] **Missing finally blocks**: Cleanup runs even on exceptions. Try/finally or context managers.
- [ ] **Bare except with pass**: Search for `except:` + `pass`. Log, handle, or rethrow.
- [ ] **Unbounded concurrent operations**: Flag unrestricted `asyncio.gather(*tasks)`. Use `asyncio.Semaphore`.
- [ ] **Missing circuit breaker**: Flag external API calls without circuit breaker or fallback.
- [ ] **SIGTERM/SIGINT handling**: Long-running processes handle shutdown gracefully.

### 4.3 JavaScript/TypeScript Fault Tolerance Checklist

- [ ] **Fetch timeout**: Native fetch lacks timeout. Verify AbortController + setTimeout or undici timeout option.
- [ ] **Promise.all without error handling**: Promise.all rejects on first failure. Use allSettled for partial results.
- [ ] **Missing retry on transient errors**: HTTP retry on 429/502/503/504 with backoff. Libraries: axios-retry, got.
- [ ] **Unbounded Promise.all on arrays**: `Promise.all(largeArray.map(...))` unbounded. Use p-limit, p-queue.
- [ ] **Unhandled promise rejections**: All async ops have `.catch()` or try/catch. Register `process.on('unhandledRejection')`.
- [ ] **Missing error boundaries**: React async components wrapped in ErrorBoundary.
- [ ] **Database connection cleanup**: Connections closed on shutdown. Register `beforeExit`/SIGTERM handlers.
- [ ] **Missing timeout on long operations**: Flag while loops without timeout/max iteration guard.
- [ ] **Circuit breaker**: Verify external API calls have circuit breaker or rate limiter. Libraries: opossum, cockatieljs.

### 4.4 Go Fault Tolerance Checklist

- [ ] **Context with timeout**: All external calls use `context.WithTimeout` or `context.WithDeadline`. No `context.Background()` without timeout.
- [ ] **Error wrapping and propagation**: Errors wrapped with `fmt.Errorf("operation: %w", err)`. No discarded errors.
- [ ] **Goroutine lifecycle**: Every goroutine has clear exit condition. Use `context.Done()` or `sync.WaitGroup`. No leaks.
- [ ] **Connection pool config**: `http.Transport` has `MaxIdleConns`, `MaxIdleConnsPerHost`, `IdleConnTimeout`.
- [ ] **Retry with backoff**: Exponential backoff + jitter. No fixed-interval retries.
- [ ] **Resource cleanup with defer**: `defer Close()` for all resources. Check for missing `defer resp.Body.Close()`.
- [ ] **Bounded concurrency**: `semaphore.Weighted` or worker pools. No unbounded `go func()` in loops.
- [ ] **Circuit breaker**: External service calls use circuit breaker.

### 4.5 Java Fault Tolerance Checklist

- [ ] **Resilience4j**: Retry, circuit breaker, rate limiter, bulkhead configured for external calls.
- [ ] **Try-with-resources**: All AutoCloseable use try-with-resources.
- [ ] **ExecutorService shutdown**: Graceful shutdown with `shutdown()` + `awaitTermination()` + `shutdownNow()` fallback.
- [ ] **Future timeout**: `Future.get(timeout, TimeUnit)` always, never bare Future.get().
- [ ] **Unbounded thread pools**: Flag `Executors.newCachedThreadPool()`. Use `newFixedThreadPool(n)` or `ThreadPoolExecutor` with bounds.
- [ ] **Spring @Retryable**: Retry config includes backoff and max attempts. No retry on non-transient exceptions.
- [ ] **Circuit breaker**: All external service calls have circuit breaker with appropriate threshold and wait duration.

---

## Section 5: Anti-Patterns

### 5.1 Cross-Language Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | No timeout on external calls | critical | No timeout config on HTTP/DB calls | Thread/connection exhaustion, cascade failures | Set explicit timeout on every external call |
| 2 | Catch-all without logging | high | Empty catch blocks with no logging | Errors silently swallowed, impossible to debug | Catch specific exceptions, log with context, handle/rethrow |
| 3 | Unbounded concurrent operations | high | Task.WhenAll/Promise.all on dynamic collections | Resource exhaustion, OOM, cascade failures | Bound concurrency with semaphore/thread pool/queue |
| 4 | Fire-and-forget without error handling | high | Background tasks without error handling | Unhandled exceptions crash or silently fail | Wrap in try/catch with logging or error callback |
| 5 | Missing finally/cleanup | high | Resources not cleaned up on exception paths | Connection leaks, file handle exhaustion | Use finally, using, defer, context managers |
| 6 | Retry without backoff | medium | Fixed-interval retries on transient errors | Retry storm, makes problem worse | Exponential backoff with jitter |
| 7 | No circuit breaker on external services | high | External calls without circuit breaker | Cascading failures, thread pool exhaustion | Circuit breaker with fallback and recovery |

### 5.2 .NET/C# Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | HttpClient created with new | high | `new HttpClient()` per request | Socket exhaustion (SOCK_TIMEWAIT) | Use IHttpClientFactory with named/typed clients |
| 2 | No timeout on admin/transit API | high | Vault/OpenBao calls without timeout | Indefinite hang blocks thread | Set HttpClient.Timeout + CancellationToken |
| 3 | HttpResponseMessage not disposed | medium | No `using` on HttpResponseMessage | Connection leak under load | `using var response = await client.SendAsync(...)` |
| 4 | Catch-all blocks without logging | high | `catch (Exception ex)` with empty body | Silent failures, lost context | Log with context before rethrowing |
| 5 | CancellationToken not propagated | medium | Async methods accept token but don't pass down | Cancellation doesn't reach I/O | Pass token through all async calls |
| 6 | Missing EnableRetryOnFailure | medium | EF Core without connection resiliency | Transient failures cause immediate crash | `options.EnableRetryOnFailure(maxRetryCount: 3)` |
| 7 | Advisory lock without timeout | high | `pg_advisory_lock` (blocks forever) | Unbounded wait, thread starvation | Use `pg_try_advisory_lock` with retry + timeout |

### 5.3 Python Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | requests without timeout | high | `requests.get(url)` without timeout | Indefinite hang, thread exhaustion | `requests.get(url, timeout=30)` |
| 2 | Bare except with pass | high | `except Exception: pass` | All errors silently swallowed | Catch specific exceptions; log or rethrow |
| 3 | Unbounded asyncio.gather | high | `asyncio.gather(*[call_api(i) for i in range(10000)])` | 10K concurrent connections exhaust | Use asyncio.Semaphore to bound concurrency |
| 4 | Missing connection pooling | medium | New connection per request | Connection overhead, resource waste | Use requests.Session() or httpx.AsyncClient |
| 5 | No retry on transient HTTP | medium | Single attempt on transient-error-prone calls | Unnecessary failures on 429/502/503/504 | Use tenacity or backoff with retry policy |

### 5.4 JavaScript/TypeScript Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Fetch without AbortController timeout | high | `fetch(url)` with no timeout | Request hangs indefinitely | AbortController with setTimeout |
| 2 | Promise.all on large arrays | high | `Promise.all(items.map(fetchItem))` unbounded | Memory/CPU exhaustion | Use p-limit or p-queue for bounded concurrency |
| 3 | Unhandled promise rejections | high | Missing .catch() or try/catch on async | Process crash, unhandled rejection | Always handle errors in async code |
| 4 | No retry on transient HTTP | medium | Single attempt on API calls | Unnecessary failures on 429/5xx | Use axios-retry or got with retry options |
| 5 | Missing error boundaries in React | medium | Async components without error boundary | Unhandled errors crash UI tree | Wrap async components in ErrorBoundary |

---

## Section 6: Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Polly Policy Pipeline — Retry + Circuit Breaker + Timeout

```csharp
// BAD: No resilience on external HTTP call
public class ExchangeRateService
{
    private readonly HttpClient _httpClient;

    public async Task<decimal> GetRateAsync(string from, string to)
    {
        var response = await _httpClient.GetAsync($"/api/rates/{from}/{to}");
        response.EnsureSuccessStatusCode();
        return await response.Content.ReadFromJsonAsync<decimal>();
    }
}
```

```csharp
// GOOD: Polly resilience pipeline with retry, circuit breaker, and timeout
using Polly;
using Polly.CircuitBreaker;
using Polly.Timeout;

var retry = Policy
    .Handle<HttpRequestException>()
    .Or<TimeoutRejectedException>()
    .WaitAndRetryAsync(3, attempt =>
        TimeSpan.FromSeconds(Math.Pow(2, attempt)) + TimeSpan.FromMicroseconds(Random.Shared.Next(0, 1000)));

var circuitBreaker = Policy
    .Handle<HttpRequestException>()
    .CircuitBreakerAsync(
        handledEventsAllowedBeforeBreaking: 5,
        durationOfBreak: TimeSpan.FromSeconds(30));

var timeout = Policy.TimeoutAsync<HttpResponseMessage>(TimeSpan.FromSeconds(10));

var pipeline = Policy.WrapAsync(retry, circuitBreaker, timeout);

public async Task<decimal> GetRateAsync(string from, string to, CancellationToken ct)
{
    var response = await pipeline.ExecuteAsync(async token =>
        await _httpClient.GetAsync($"/api/rates/{from}/{to}", token), ct);

    response.EnsureSuccessStatusCode();
    return await response.Content.ReadFromJsonAsync<decimal>(ct);
}
```

#### Example 2: IHttpClientFactory with Resilience

```csharp
// BAD: HttpClient created with new — socket exhaustion
public class PaymentService
{
    public async Task<PaymentResult> ChargeAsync(ChargeRequest request)
    {
        using var client = new HttpClient(); // Socket leak!
        var response = await client.PostAsJsonAsync("https://payments.example.com/charge", request);
        return await response.Content.ReadFromJsonAsync<PaymentResult>();
    }
}
```

```csharp
// GOOD: IHttpClientFactory with named client and resilience
builder.Services.AddHttpClient("Payments")
    .ConfigureHttpClient(client =>
    {
        client.BaseAddress = new Uri("https://payments.example.com");
        client.Timeout = TimeSpan.FromSeconds(30);
    })
    .AddResilienceHandler("payments-pipeline", builder =>
    {
        builder.AddRetry(new HttpRetryStrategyOptions
        {
            MaxRetryAttempts = 3,
            BackoffType = DelayBackoffType.Exponential,
            UseJitter = true
        });
        builder.AddCircuitBreaker(new HttpCircuitBreakerStrategyOptions
        {
            SamplingDuration = TimeSpan.FromSeconds(10),
            FailureRatio = 0.5,
            BreakDuration = TimeSpan.FromSeconds(30)
        });
        builder.AddTimeout(TimeSpan.FromSeconds(10));
    });

public class PaymentService
{
    private readonly HttpClient _client;

    public PaymentService(IHttpClientFactory httpClientFactory)
    {
        _client = httpClientFactory.CreateClient("Payments");
    }

    public async Task<PaymentResult> ChargeAsync(ChargeRequest request, CancellationToken ct)
    {
        var response = await _client.PostAsJsonAsync("/charge", request, ct);
        response.EnsureSuccessStatusCode();
        return await response.Content.ReadFromJsonAsync<PaymentResult>(ct);
    }
}
```

#### Example 3: CancellationToken Propagation

```csharp
// BAD: CancellationToken accepted but not propagated
public async Task<List<Order>> GetOrdersAsync(Guid customerId, CancellationToken ct)
{
    await Task.Delay(2000);
    var orders = await _dbContext.Orders
        .Where(o => o.CustomerId == customerId)
        .ToListAsync(); // No token passed
    return orders;
}

// GOOD: Token propagated through all async operations
public async Task<List<Order>> GetOrdersAsync(Guid customerId, CancellationToken ct)
{
    await Task.Delay(2000, ct);
    var orders = await _dbContext.Orders
        .Where(o => o.CustomerId == customerId)
        .AsNoTracking()
        .ToListAsync(ct);
    return orders;
}
```

#### Example 4: Outbox Pattern for Event Publishing

```csharp
// BAD: Publish event directly after SaveChanges — event lost on failure
public async Task Handle(CreateOrderCommand command)
{
    var order = new Order(command.CustomerId, command.Items);
    _dbContext.Orders.Add(order);
    await _dbContext.SaveChangesAsync();

    // If publish fails, event lost — order created but handler never notified
    await _eventBus.PublishAsync(new OrderCreatedEvent(order.Id));
}

// GOOD: Outbox pattern — event stored atomically with entity
public async Task Handle(CreateOrderCommand command, CancellationToken ct)
{
    var order = new Order(command.CustomerId, command.Items);
    _dbContext.Orders.Add(order);

    var outboxMessage = new OutboxMessage(
        eventType: nameof(OrderCreatedEvent),
        payload: JsonSerializer.Serialize(new OrderCreatedEvent(order.Id)),
        createdAt: DateTimeOffset.UtcNow);
    _dbContext.OutboxMessages.Add(outboxMessage);

    await _dbContext.SaveChangesAsync(ct);
    // Background worker processes outbox
}
```

### 6.2 Python Examples

#### Example 1: HTTP Timeout and Retry

```python
# BAD: No timeout, no retry on external API call
import requests
def fetch_exchange_rate(currency: str) -> float:
    response = requests.get(f"https://api.example.com/rates/{currency}")
    response.raise_for_status()
    return response.json()["rate"]

# GOOD: Timeout + retry with exponential backoff
import requests
from tenacity import retry, stop_after_attempt, wait_exponential_jitter, retry_if_exception_type

@retry(
    stop=stop_after_attempt(3),
    wait=wait_exponential_jitter(initial=1, max=10, jitter=2),
    retry=retry_if_exception_type((requests.Timeout, requests.ConnectionError)),
)
def fetch_exchange_rate(currency: str) -> float:
    response = requests.get(
        f"https://api.example.com/rates/{currency}",
        timeout=10,
    )
    response.raise_for_status()
    return response.json()["rate"]
```

#### Example 2: Bounded Async Concurrency

```python
# BAD: Unbounded concurrency — all 10000 requests fire simultaneously
import asyncio
import httpx

async def fetch_all_rates(currencies: list[str]) -> dict:
    async with httpx.AsyncClient() as client:
        tasks = [client.get(f"https://api.example.com/rates/{c}") for c in currencies]
        responses = await asyncio.gather(*tasks)
        return {currencies[i]: responses[i].json()["rate"] for i in range(len(currencies))}

# GOOD: Bounded concurrency with semaphore
import asyncio
import httpx

async def fetch_all_rates(currencies: list[str], max_concurrent: int = 10) -> dict:
    semaphore = asyncio.Semaphore(max_concurrent)
    results = {}

    async def fetch_one(client: httpx.AsyncClient, currency: str) -> None:
        async with semaphore:
            response = await client.get(
                f"https://api.example.com/rates/{currency}",
                timeout=10,
            )
            response.raise_for_status()
            results[currency] = response.json()["rate"]

    async with httpx.AsyncClient() as client:
        tasks = [fetch_one(client, c) for c in currencies]
        await asyncio.gather(*tasks)
    return results
```

#### Example 3: Resource Cleanup with Context Managers

```python
# BAD: File not closed on exception
def process_transactions(filepath: str) -> list:
    f = open(filepath)
    data = json.load(f)
    results = []
    for tx in data["transactions"]:
        results.append(validate_transaction(tx))
    f.close()
    return results

# GOOD: Context manager ensures cleanup
def process_transactions(filepath: str) -> list[dict[str, Any]]:
    with open(filepath) as f:
        data = json.load(f)
    results = []
    for tx in data["transactions"]:
        results.append(validate_transaction(tx))
    return results
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: Fetch with Timeout via AbortController

```javascript
// BAD: Fetch without timeout — hangs indefinitely
async function fetchUserData(userId) {
    const response = await fetch(`/api/users/${userId}`);
    return response.json();
}

// GOOD: Fetch with AbortController timeout
async function fetchUserData(userId, timeoutMs = 10000) {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
    try {
        const response = await fetch(`/api/users/${userId}`, {
            signal: controller.signal,
        });
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        return await response.json();
    } finally {
        clearTimeout(timeoutId);
    }
}
```

#### Example 2: Bounded Promise Concurrency

```javascript
// BAD: Unbounded Promise.all — 10000 simultaneous requests
async function fetchAllRates(currencies) {
    const results = await Promise.all(
        currencies.map(c => fetch(`/api/rates/${c}`).then(r => r.json()))
    );
    return Object.fromEntries(currencies.map((c, i) => [c, results[i].rate]));
}

// GOOD: Bounded concurrency with p-limit
import pLimit from 'p-limit';

async function fetchAllRates(currencies, concurrency = 10) {
    const limit = pLimit(concurrency);
    const entries = await Promise.all(
        currencies.map(c =>
            limit(async () => {
                const response = await fetch(`/api/rates/${c}`);
                if (!response.ok) throw new Error(`Failed for ${c}: ${response.status}`);
                const data = await response.json();
                return [c, data.rate];
            })
        )
    );
    return Object.fromEntries(entries);
}
```

#### Example 3: Error Boundary in React

```typescript
// BAD: Async component without error boundary
function UserProfile({ userId }: { userId: string }) {
    const [user, setUser] = useState(null);
    const [error, setError] = useState(null);
    useEffect(() => {
        fetchUser(userId).then(setUser).catch(setError);
    }, [userId]);
    if (error) return <div>Error: {error.message}</div>;
    if (!user) return <Loading />;
    return <UserDetails user={user} />;
}

// GOOD: Error boundary wraps component tree
class ErrorBoundary extends React.Component<
    { children: React.ReactNode; fallback?: React.ReactNode },
    { hasError: boolean; error: Error | null }
> {
    state = { hasError: false, error: null };
    static getDerivedStateFromError(error: Error) {
        return { hasError: true, error };
    }
    componentDidCatch(error: Error, info: React.ErrorInfo) {
        logger.error("ErrorBoundary caught", { error, info });
    }
    render() {
        if (this.state.hasError) {
            return this.props.fallback ?? (
                <ErrorFallback error={this.state.error!}
                    onRetry={() => this.setState({ hasError: false, error: null })} />
            );
        }
        return this.props.children;
    }
}

<ErrorBoundary>
    <UserProfile userId={userId} />
</ErrorBoundary>
```

---

## Section 7: Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Be specific — include the resilience gap.",
  "recommendation": "Specific fix with brief code example if applicable.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "fault_tolerance"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative path from project root. Forward slashes. Never absolute.
- **line**: 1-based line number where finding starts.
- **endLine**: Optional. Line where finding ends.
- **description**: Factual, specific. Include resilience gap pattern.
- **recommendation**: Actionable. Include code snippet when possible.
- **technology**: Primary technology. Use `CrossLanguage` only for multi-file patterns.
- **category**: Always `fault_tolerance`.

### 7.3 Summary Report

Emit summary after all findings:

```json
{
  "summary": {
    "squad": "audit-fault-tolerance",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 18,
    "bySeverity": { "critical": 2, "high": 5, "medium": 8, "low": 3 },
    "byCategory": { "fault_tolerance": 18 },
    "filesScanned": 60,
    "filesWithFindings": 15
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Fault Tolerance-Specific Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Total system outage, data loss, indefinite hang | No timeout on DB calls, socket exhaustion from HttpClient, unbounded concurrency |
| **high** | Significant resilience gap with clear failure | Missing circuit breaker, catch-all without logging, fire-and-forget without error handling |
| **medium** | Degraded resilience posture | Missing CancellationToken, no retry, missing EnableRetryOnFailure |
| **low** | Defense-in-depth improvement | Suboptimal timeout, retry without jitter, verbose errors |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding in critical path (payment, auth, data persistence) | +1 level | All |
| Finding affects multiple services/endpoints | +1 level | Distributed systems |
| Finding can cause data loss | +1 level | All |
| Finding in production-only code | +1 level | All |
| Finding involves external service with known instability | +1 level | All |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in non-production/test only | -1 level (min low) | All |
| Existing mitigating control | -1 level (min low) | All |
| Deprecated code scheduled for removal | -1 level (min low) | All |
| Prototype/spike directory | -1 level (min low) | All |

### 8.4 Technology-Specific Severity Adjustments

| Technology | Pattern | Base Severity | Notes |
|-----------|---------|---------------|-------|
| **.NET/C#** | HttpClient with `new` | high | Socket exhaustion |
| **.NET/C#** | Missing CancellationToken propagation | medium | Async cancellation broken |
| **.NET/C#** | Catch-all without logging | high | Silent failures |
| **.NET/C#** | Missing EnableRetryOnFailure | medium | Transient DB errors crash |
| **Python** | requests without timeout | high | Indefinite hang |
| **Python** | Bare except with pass | high | Errors swallowed |
| **Python** | Unbounded asyncio.gather | high | Resource exhaustion |
| **JavaScript** | Fetch without AbortController | high | Indefinite hang |
| **JavaScript** | Promise.all on large arrays | high | Resource exhaustion |
| **JavaScript** | Unhandled promise rejections | high | Process crash |
| **Go** | context.Background without timeout | high | No cancellation |
| **Go** | Ignored errors (`_ =`) | high | Silent failures |
| **Go** | Unbounded goroutine spawn | high | Goroutine leak |
| **Java** | Future.get() without timeout | high | Indefinite block |
| **Java** | Unbounded thread pool | high | Resource exhaustion |

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-security ─────────────────────┐
audit-architecture ─────────────────┤
audit-code-quality ─────────────────┤
audit-dependencies ─────────────────┤
audit-test-coverage ────────────────┤
audit-fault-tolerance ──────────────┤
audit-observability ────────────────┤
audit-privacy-data ─────────────────┤
audit-cicd ─────────────────────────┤
audit-license ──────────────────────┤
audit-consolidated ◄────────────────┘
audit-consistency-coherence
```

### 9.2 Upstream Dependencies

- **audit-security**: May inform resilience gaps (auth failure handling).
- **audit-architecture**: May reveal resilience design flaws (missing outbox).
- **audit-observability**: Gaps may prevent detecting resilience failures.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates findings.
- **audit-consistency-coherence**: Validates cross-domain alignment.

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-fault-tolerance | audit-observability | Resilience patterns vs telemetry/monitoring |
| audit-fault-tolerance | audit-security | Error handling vs vulnerability exploitation |
| audit-fault-tolerance | audit-architecture | Runtime resilience vs structural design |

### 9.5 Recommended Execution Order

Runs at position 6 in full audit:

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Code-level vuln scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** ← This squad
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. Domain-specific squads
12. **audit-consolidated** — Aggregate all
13. **audit-consistency-coherence** — Cross-domain check

### 9.6 Output Integration

When squads run sequentially:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip findings already reported.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in standardized Section 7 format.