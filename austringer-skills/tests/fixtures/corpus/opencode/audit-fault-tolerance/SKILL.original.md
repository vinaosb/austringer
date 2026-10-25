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
- Circuit breaker patterns — verify circuit breaker state machines, fallback logic, recovery testing
- Error handling review — verify unhandled exception coverage, error propagation chains, catch block quality
- Timeout configuration — verify reasonable timeouts at all service boundaries (HTTP, DB, SMTP, external APIs)
- Failover mechanism audit — verify failover triggers, state synchronization, split-brain prevention
- Graceful degradation check — verify system degrades gracefully rather than hard-failing
- Bulkhead isolation — verify resource pools prevent cascade failures, bounded concurrency
- Resource cleanup — verify `finally` blocks, `using` statements, `defer` calls for cleanup
- CancellationToken propagation — verify all async operations accept and respect cancellation tokens
- HttpClient lifecycle — verify IHttpClientFactory usage, proper disposal, connection pool management
- Outbox pattern verification — verify event publishing uses outbox for at-least-once delivery
- Fire-and-forget safety — verify background operations have error handling and logging
- **.NET/C#**: Polly resilience pipeline validation, IHttpClientFactory usage audit, CancellationToken propagation check, outbox pattern verification, EF Core connection resiliency
- **Python**: tenacity/backoff retry decorator validation, asyncio timeout and semaphore patterns, context manager resource cleanup
- **JavaScript/TypeScript**: AbortController timeout patterns, Promise.allSettled vs Promise.all, p-limit/p-queue bounded concurrency, React error boundaries
- **Go**: context.WithTimeout on all external calls, error wrapping conventions, goroutine lifecycle and leak detection, defer cleanup
- **Java**: Resilience4j configuration audit, try-with-resources compliance, ExecutorService shutdown patterns, Future.get timeout
- **Rust**: Result vs panic strategy, unwrap/expect safety audit, Drop implementation correctness, concurrent access patterns

### DO NOT USE FOR

- Security vulnerabilities (use `audit-security`)
- Test quality and coverage (use `audit-test-coverage`)
- Code style issues (use `audit-code-quality`)
- Architecture layer violations (use `audit-architecture`)
- Observability gaps (use `audit-observability`)

---

## Section 3: Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

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
| Polly config | `*Policy*`, `*Resilience*`, `*Pipeline*` | Retry/circuit breaker/timeout configuration |
| HttpClient | `*Http*`, `*Client*`, `*External*`, `*Provider*` | External service call patterns |
| Message handlers | `*Handler*`, `*Consumer*`, `*Subscriber*` | Message processing resilience |
| Error handling | `*Exception*`, `*Error*`, `*Fault*` | Error types and handling patterns |
| Health checks | `*Health*`, `*Readiness*`, `*Liveness*` | Health monitoring |
| Background jobs | `*Job*`, `*Worker*`, `*Background*`, `*Hosted*` | Background task resilience |
| Database context | `*DbContext*`, `*Repository*`, `*Context*` | Database operation resilience |
| Email/notification | `*Email*`, `*Smtp*`, `*Notification*` | SMTP/notification retry patterns |
| Config | `appsettings*.json`, `*.env`, `*.yaml` | Timeout and retry configuration values |

### 3.3 Resilience Library Detection

| Signal | Pattern | Detected Library |
|--------|---------|------------------|
| Polly | `*.csproj` containing `Polly`, `Microsoft.Extensions.Http.Resilience` | .NET resilience library |
| Tenacity | `pyproject.toml`/`requirements.txt` with `tenacity`, `backoff` | Python retry library |
| opossum | `package.json` with `opossum`, `cockatieljs` | JS circuit breaker library |
| gobreaker | `go.mod` with `sony/gobreaker` | Go circuit breaker |
| Resilience4j | `pom.xml`/`build.gradle` with `resilience4j-*` | Java resilience library |

### 3.4 Infrastructure Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml` | Container health checks |
| Kubernetes | `*.yaml` with `livenessProbe`/`readinessProbe` | Pod health probes |
| OTel | `otel-collector-config.yml` | Observability for resilience |

### 3.5 Discovery Procedure

1. **Scan root directory** for resilience-related file patterns above.
2. **Identify primary language** by source file count.
3. **Detect resilience libraries**: Polly (.NET), resilience4j (Java), tenacity (Python), etc.
4. **Identify external service integration points**: HTTP clients, database connections, SMTP, message queues.
5. **Map error handling patterns**: try/catch blocks, Result types, error middleware.
6. **Detect timeout configuration**: Search for `Timeout`, `CancellationToken`, `deadline`, `context.WithTimeout`.
7. **Select the matching checklist** from Section 4 based on detected technologies.
8. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Fault Tolerance Checklist

- [ ] **Polly retry/circuit breaker configuration**: Verify retry policies use exponential backoff with
  jitter. Verify circuit breaker has appropriate failure threshold, sampling duration, and recovery time.
  Policies should be composed into a resilience pipeline.
- [ ] **HttpClient timeout and disposal**: Verify `IHttpClientFactory` is used (not `new HttpClient()`).
  Verify `Timeout` is set appropriately. Verify `HttpResponseMessage` is disposed (use `using` or
  `await using`).
- [ ] **CancellationToken propagation**: All async methods should accept `CancellationToken` parameter.
  Verify token is passed through all downstream calls (database, HTTP, file I/O). No swallowed tokens.
- [ ] **Missing circuit breaker on external service calls**: Every outbound HTTP call to external services
  should have circuit breaker protection. Flag any `HttpClient.SendAsync` without resilience pipeline.
- [ ] **Unbounded Task.WhenAll without bulkhead**: Search for `Task.WhenAll` on collections of unknown size.
  Use `SemaphoreSlim` or `Channel<T>` to bound concurrency and prevent resource exhaustion.
- [ ] **Fire-and-forget without error handling**: Search for `_ = Task.Run(`, `_ = SomeAsync()` without
  `.ContinueWith` error handling or wrapping in try/catch. All background operations must log failures.
- [ ] **Catch-all without logging**: Search for `catch (Exception ex)` blocks that don't log or rethrow.
  Every catch block should either handle the error, log it, or rethrow — never silently swallow.
- [ ] **Missing finally for resource cleanup**: Verify streams, connections, and IDisposable resources
  are cleaned up in `finally` blocks or `using` statements, even on exception paths.
- [ ] **Missing outbox pattern for event publishing**: Event publishing after database operations should
  use the outbox pattern to ensure at-least-once delivery. Direct publish after SaveChanges can lose events.
- [ ] **SMTP sends without timeout/retry**: Email sending should have timeout configuration and retry
  policy. Flag any `SmtpClient.Send` or `SendAsync` without timeout and error handling.
- [ ] **Provider waterfall without health tracking**: When trying multiple providers in sequence (e.g.,
  exchange rates, notification providers), each failure should be tracked. Circuit should open per-provider.
- [ ] **Advisory lock without bounded wait**: Database advisory locks should have timeout. Flag
  `pg_advisory_lock` (unbounded) vs `pg_try_advisory_lock` with retry and timeout.
- [ ] **Missing HttpClient timeout**: Default `HttpClient.Timeout` is 100s. Verify overridden for
  fast-failing endpoints. Long-polling endpoints should have explicit longer timeout.
- [ ] **Task.Delay without cancellation**: Search for `Task.Delay(` without `CancellationToken` parameter.
  Use `Task.Delay(timeout, cancellationToken)` for cancellable waits.
- [ ] **Database connection resiliency**: Verify EF Core connection resiliency configured for transient
  failures (`EnableRetryOnFailure`).

### 4.2 Python Fault Tolerance Checklist

- [ ] **Retry decorator usage**: Verify external service calls use retry with exponential backoff
  (`tenacity`, `backoff`). No bare `while True: try/except` retry loops without backoff or max attempts.
- [ ] **Timeout on HTTP calls**: Verify `requests.get/post` uses `timeout` parameter. No infinite waits.
  Use `httpx` with timeout config for async code.
- [ ] **Connection pool management**: Verify database connections use connection pooling with bounds.
  No unbounded connection creation in loops.
- [ ] **Error propagation in async pipelines**: Verify `asyncio.gather` uses `return_exceptions=False`
  intentionally. Know the difference between `gather` and `TaskGroup` for error handling.
- [ ] **Resource cleanup with context managers**: Verify file handles, database connections, and HTTP
  sessions use `with` statements or `async with` for guaranteed cleanup.
- [ ] **Missing finally blocks**: Verify cleanup code runs even when exceptions occur. Use `try/finally`
  or context managers for resource cleanup.
- [ ] **Bare except with pass**: Search for `except:` or `except Exception:` followed by `pass`.
  Every exception handler should log, handle, or rethrow.
- [ ] **Unbounded concurrent operations**: Search for unbounded `asyncio.gather(*tasks)` on dynamic lists.
  Use `asyncio.Semaphore` to bound concurrency.
- [ ] **Missing circuit breaker for external APIs**: Flag any service that calls external APIs without
  circuit breaker or fallback mechanism.
- [ ] **SIGTERM/SIGINT handling**: Long-running processes should handle shutdown signals gracefully.
  Use `signal.signal` or `asyncio.create_task` with cancellation.

### 4.3 JavaScript/TypeScript Fault Tolerance Checklist

- [ ] **Fetch timeout**: Native `fetch` doesn't have timeout. Verify `AbortController` with `setTimeout`
  or use `node-fetch`/`undici` with timeout option.
- [ ] **Promise.all without error handling**: `Promise.all` rejects on first failure. Verify error handling
  or use `Promise.allSettled` when partial results are acceptable.
- [ ] **Missing retry on transient errors**: Verify HTTP calls retry on 429, 502, 503, 504 status codes
  with exponential backoff. Libraries: `axios-retry`, `got` retry options.
- [ ] **Unbounded Promise.all on arrays**: `Promise.all(largeArray.map(...))` creates unbounded concurrency.
  Use `p-limit`, `p-queue`, or manual semaphore.
- [ ] **Unhandled promise rejections**: Verify all async operations have `.catch()` or try/catch.
  Register `process.on('unhandledRejection')` handler.
- [ ] **Missing error boundaries**: React components that make async calls should be wrapped in
  `ErrorBoundary`. Verify error boundary exists at appropriate levels.
- [ ] **Database connection cleanup**: Verify database connections/clients are closed on shutdown.
  Register `beforeExit` or `SIGTERM` handlers.
- [ ] **Missing timeout on long operations**: Flag any `while` loop or recursive function without
  timeout or max iteration guard.
- [ ] **Circuit breaker for external APIs**: Verify external API calls have circuit breaker or
  rate limiter. Libraries: `opossum`, `cockatieljs`.

### 4.4 Go Fault Tolerance Checklist

- [ ] **Context with timeout on all external calls**: Every external HTTP/RPC/database call should use
  `context.WithTimeout` or `context.WithDeadline`. No background `context.Background()` without timeout.
- [ ] **Error wrapping and propagation**: Errors should be wrapped with `fmt.Errorf("operation: %w", err)`.
  No silently discarded errors (`_ = riskyOperation()`).
- [ ] **Goroutine lifecycle management**: Every goroutine should have a clear exit condition. Use
  `context.Done()` channel or `sync.WaitGroup` for coordination. No goroutine leaks.
- [ ] **Connection pool configuration**: Verify `http.Transport` has `MaxIdleConns`, `MaxIdleConnsPerHost`,
  and `IdleConnTimeout` configured. Default values may not be appropriate.
- [ ] **Retry with backoff**: Verify retry logic uses exponential backoff with jitter. No fixed-interval
  retries on transient errors.
- [ ] **Resource cleanup with defer**: Verify `defer Close()` for all resources (HTTP bodies, files,
  database connections). Check for missing `defer resp.Body.Close()`.
- [ ] **Bounded concurrency**: Use `semaphore.Weighted` or worker pools for concurrent operations.
  No unbounded `go func()` spawns in loops.
- [ ] **Circuit breaker**: Verify external service calls use circuit breaker (`sony/gobreaker` or similar).

### 4.5 Java Fault Tolerance Checklist

- [ ] **Resilience4j configuration**: Verify retry, circuit breaker, rate limiter, and bulkhead
  configurations are present for external service calls.
- [ ] **Try-with-resources**: All `AutoCloseable` types (streams, connections, statements) must use
  try-with-resources. No unclosed resources.
- [ ] **ExecutorService shutdown**: Verify `ExecutorService` instances are shut down gracefully with
  `shutdown()` + `awaitTermination()` + `shutdownNow()` fallback.
- [ ] **Future timeout**: `Future.get()` without timeout blocks indefinitely. Always use
  `future.get(timeout, TimeUnit)`.
- [ ] **Unbounded thread pools**: Flag `Executors.newCachedThreadPool()` (unbounded). Use
  `Executors.newFixedThreadPool(n)` or `ThreadPoolExecutor` with bounds.
- [ ] **Spring @Retryable**: Verify retry configuration includes backoff and max attempts. No retry
  on non-transient exceptions.
- [ ] **Hystrix/Resilience4j circuit breaker**: Verify circuit breakers configured for all external
  service calls with appropriate failure rate threshold and wait duration.

---

## Section 5: Anti-Patterns

### 5.1 Cross-Language Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | No timeout on external calls | critical | HTTP/database calls without timeout configuration | Thread/connection exhaustion, cascade failures | Set explicit timeout on every external call |
| 2 | Catch-all without logging | high | Empty catch blocks or catch-all with no logging | Errors silently swallowed, impossible to debug | Catch specific exceptions, log with context, rethrow or handle |
| 3 | Unbounded concurrent operations | high | Task.WhenAll/Promise.all on dynamic collections without limit | Resource exhaustion, OOM, cascade failures | Bound concurrency with semaphore, thread pool, or queue |
| 4 | Fire-and-forget without error handling | high | Background tasks launched without error handling | Unhandled exceptions crash or silently fail | Wrap in try/catch with logging, or use fire-and-forget with error callback |
| 5 | Missing finally/cleanup | high | Resources not cleaned up on exception paths | Connection leaks, file handle exhaustion, memory leaks | Use finally blocks, using statements, defer, context managers |
| 6 | Retry without backoff | medium | Fixed-interval retries on transient errors | Retry storm, makes problem worse | Exponential backoff with jitter |
| 7 | No circuit breaker on external services | high | External service calls without circuit breaker | Cascading failures, thread pool exhaustion | Circuit breaker with fallback and recovery |

### 5.2 .NET/C# Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | HttpClient created with new | high | `new HttpClient()` per request or in using block | Socket exhaustion (SOCK_TIMEWAIT), connection pool thrashing | Use `IHttpClientFactory` with named/typed clients |
| 2 | No timeout on admin/transit API calls | high | Vault/OpenBao API calls without timeout | Indefinite hang blocks calling thread, cascade failure | Set `HttpClient.Timeout` and pass `CancellationToken` |
| 3 | HttpResponseMessage not disposed | medium | Not using `using` on HttpResponseMessage | Connection leak, socket exhaustion under load | `using var response = await client.SendAsync(...)` |
| 4 | Catch-all blocks without logging | high | `catch (Exception ex)` with empty body or only `throw` | Silent failures, lost error context | Log with context before rethrowing: `logger.LogError(ex, "Operation failed")` |
| 5 | CancellationToken not propagated | medium | Async methods accept token but don't pass it downstream | Cancellation doesn't reach actual I/O operation | Pass token through all async calls: `await db.SaveChangesAsync(token)` |
| 6 | Missing EnableRetryOnFailure | medium | EF Core without connection resiliency configuration | Transient network glitches cause immediate failure | `options.EnableRetryOnFailure(maxRetryCount: 3)` |
| 7 | Advisory lock without timeout | high | `pg_advisory_lock` (blocks forever) instead of try variant | Unbounded wait causes thread starvation | Use `pg_try_advisory_lock` with retry loop and timeout |

### 5.3 Python Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | requests without timeout | high | `requests.get(url)` without timeout parameter | Indefinite hang, thread exhaustion | `requests.get(url, timeout=30)` |
| 2 | Bare except with pass | high | `except Exception: pass` or `except: pass` | All errors silently swallowed | Catch specific exceptions; log or rethrow |
| 3 | Unbounded asyncio.gather | high | `asyncio.gather(*[call_api(i) for i in range(10000)])` | 10000 concurrent connections exhaust resources | Use `asyncio.Semaphore` to bound concurrency |
| 4 | Missing connection pooling | medium | New connection per request instead of session reuse | Connection overhead, resource waste | Use `requests.Session()` or `httpx.AsyncClient` |
| 5 | No retry on transient HTTP errors | medium | Single attempt on HTTP calls that can fail transiently | Unnecessary failures on 429/502/503/504 | Use `tenacity` or `backoff` library with retry policy |

### 5.4 JavaScript/TypeScript Fault Tolerance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Fetch without AbortController timeout | high | `fetch(url)` with no timeout mechanism | Request hangs indefinitely | Use `AbortController` with `setTimeout` for timeout |
| 2 | Promise.all on large arrays | high | `Promise.all(items.map(fetchItem))` with unbounded array | Memory/CPU exhaustion, connection limit | Use `p-limit` or `p-queue` for bounded concurrency |
| 3 | Unhandled promise rejections | high | Missing `.catch()` on promises or try/catch on async | Process crash, unhandled rejection warning | Always handle errors in async code |
| 4 | No retry on transient HTTP errors | medium | Single attempt on API calls | Unnecessary failures on 429/5xx | Use `axios-retry` or `got` with retry options |
| 5 | Missing error boundaries in React | medium | Async components without error boundary | Unhandled errors crash entire UI tree | Wrap async components in ErrorBoundary |

---

## Section 6: Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Polly Policy Pipeline — Retry + Circuit Breaker + Timeout

```csharp
// ❌ BAD: No resilience on external HTTP call
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
// ✅ GOOD: Polly resilience pipeline with retry, circuit breaker, and timeout
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
// ❌ BAD: HttpClient created with new — socket exhaustion
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
// ✅ GOOD: IHttpClientFactory with named client and resilience
// Registration in Program.cs:
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

// Usage:
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
        return await response.Content.ReadFromJsonAsync<PaymentResult>(ct)
            ?? throw new InvalidOperationException("Failed to deserialize payment result");
    }
}
```

#### Example 3: CancellationToken Propagation

```csharp
// ❌ BAD: CancellationToken accepted but not propagated
public async Task<List<Order>> GetOrdersAsync(Guid customerId, CancellationToken ct)
{
    // Token accepted but never passed downstream
    await Task.Delay(2000); // Blocking delay without cancellation
    var orders = await _dbContext.Orders
        .Where(o => o.CustomerId == customerId)
        .ToListAsync(); // No token passed to EF Core
    return orders;
}
```

```csharp
// ✅ GOOD: Token propagated through all async operations
public async Task<List<Order>> GetOrdersAsync(Guid customerId, CancellationToken ct)
{
    await Task.Delay(2000, ct); // Cancellable delay
    var orders = await _dbContext.Orders
        .Where(o => o.CustomerId == customerId)
        .AsNoTracking()
        .ToListAsync(ct); // Token passed to EF Core
    return orders;
}
```

#### Example 4: Outbox Pattern for Event Publishing

```csharp
// ❌ BAD: Publish event directly after SaveChanges — event lost on failure
public async Task Handle(CreateOrderCommand command)
{
    var order = new Order(command.CustomerId, command.Items);
    _dbContext.Orders.Add(order);
    await _dbContext.SaveChangesAsync();

    // If this fails, event is lost — order created but handler never notified
    await _eventBus.PublishAsync(new OrderCreatedEvent(order.Id));
}
```

```csharp
// ✅ GOOD: Outbox pattern — event stored atomically with entity
public async Task Handle(CreateOrderCommand command, CancellationToken ct)
{
    var order = new Order(command.CustomerId, command.Items);
    _dbContext.Orders.Add(order);

    // Store event in same transaction — atomic guarantee
    var outboxMessage = new OutboxMessage(
        eventType: nameof(OrderCreatedEvent),
        payload: JsonSerializer.Serialize(new OrderCreatedEvent(order.Id)),
        createdAt: DateTimeOffset.UtcNow);
    _dbContext.OutboxMessages.Add(outboxMessage);

    await _dbContext.SaveChangesAsync(ct);
    // Background worker processes outbox and publishes events
}
```

### 6.2 Python Examples

#### Example 1: HTTP Timeout and Retry

```python
# ❌ BAD: No timeout, no retry on external API call
import requests

def fetch_exchange_rate(currency: str) -> float:
    response = requests.get(f"https://api.example.com/rates/{currency}")
    response.raise_for_status()
    return response.json()["rate"]
```

```python
# ✅ GOOD: Timeout + retry with exponential backoff
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
        timeout=10,  # Explicit timeout
    )
    response.raise_for_status()
    return response.json()["rate"]
```

#### Example 2: Bounded Async Concurrency

```python
# ❌ BAD: Unbounded concurrency — all 10000 requests fire simultaneously
import asyncio
import httpx

async def fetch_all_rates(currencies: list[str]) -> dict:
    async with httpx.AsyncClient() as client:
        tasks = [client.get(f"https://api.example.com/rates/{c}") for c in currencies]
        responses = await asyncio.gather(*tasks)  # Unbounded!
        return {currencies[i]: responses[i].json()["rate"] for i in range(len(currencies))}
```

```python
# ✅ GOOD: Bounded concurrency with semaphore
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
# ❌ BAD: File not closed on exception
def process_transactions(filepath: str) -> list:
    f = open(filepath)  # Not closed if exception occurs below
    data = json.load(f)
    results = []
    for tx in data["transactions"]:
        results.append(validate_transaction(tx))  # May raise
    f.close()  # Never reached if validate raises
    return results
```

```python
# ✅ GOOD: Context manager ensures cleanup
import json
from typing import Any

def process_transactions(filepath: str) -> list[dict[str, Any]]:
    with open(filepath) as f:  # Always closed, even on exception
        data = json.load(f)

    results = []
    for tx in data["transactions"]:
        results.append(validate_transaction(tx))
    return results
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: Fetch with Timeout via AbortController

```javascript
// ❌ BAD: Fetch without timeout — hangs indefinitely
async function fetchUserData(userId) {
    const response = await fetch(`/api/users/${userId}`);
    return response.json();
}
```

```javascript
// ✅ GOOD: Fetch with AbortController timeout
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
// ❌ BAD: Unbounded Promise.all — 10000 simultaneous requests
async function fetchAllRates(currencies) {
    const results = await Promise.all(
        currencies.map(c => fetch(`/api/rates/${c}`).then(r => r.json()))
    );
    return Object.fromEntries(currencies.map((c, i) => [c, results[i].rate]));
}
```

```javascript
// ✅ GOOD: Bounded concurrency with p-limit (or manual semaphore)
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
// ❌ BAD: Async component without error boundary
function UserProfile({ userId }: { userId: string }) {
    const [user, setUser] = useState(null);
    const [error, setError] = useState(null);

    useEffect(() => {
        fetchUser(userId)
            .then(setUser)
            .catch(setError); // Only catches this component, not children
    }, [userId]);

    if (error) return <div>Error: {error.message}</div>;
    if (!user) return <Loading />;
    return <UserDetails user={user} />;
}
```

```typescript
// ✅ GOOD: Error boundary wraps component tree
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
                <ErrorFallback
                    error={this.state.error!}
                    onRetry={() => this.setState({ hasError: false, error: null })}
                />
            );
        }
        return this.props.children;
    }
}

// Usage:
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

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the resilience gap pattern.
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns that span multiple files.
- **category**: Always `fault_tolerance` for this squad.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-fault-tolerance",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 18,
    "bySeverity": {
      "critical": 2,
      "high": 5,
      "medium": 8,
      "low": 3
    },
    "byCategory": {
      "fault_tolerance": 18
    },
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
| **critical** | Can cause total system outage, data loss, or indefinite hang | No timeout on database calls, socket exhaustion from HttpClient misuse, unbounded concurrent operations |
| **high** | Significant resilience gap with clear failure scenario | Missing circuit breaker on external service, catch-all without logging, fire-and-forget without error handling |
| **medium** | Degraded resilience posture or missing best practice | Missing CancellationToken propagation, no retry on transient errors, missing EnableRetryOnFailure |
| **low** | Defense-in-depth improvement | Suboptimal timeout values, retry without jitter, verbose error messages |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in critical path (payment, auth, data persistence) | +1 level | All technologies |
| Finding affects multiple services/endpoints | +1 level | Distributed systems |
| Finding can cause data loss | +1 level | All technologies |
| Finding is in production-only code | +1 level | All technologies |
| Finding involves external service with known instability | +1 level | All technologies |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control (e.g., upstream timeout) | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in prototype/spike directory | -1 level (min low) | All technologies |

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
| **Go** | context.Background without timeout | high | No cancellation mechanism |
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

- **audit-security**: Security findings may inform resilience gaps (e.g., auth failure handling).
- **audit-architecture**: Architectural patterns may reveal resilience design flaws (e.g., missing outbox).
- **audit-observability**: Observability gaps may prevent detecting resilience failures.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-fault-tolerance | audit-observability | Fault-tolerance focuses on resilience patterns; observability focuses on telemetry and monitoring |
| audit-fault-tolerance | audit-security | Fault-tolerance focuses on error handling; security focuses on vulnerability exploitation |
| audit-fault-tolerance | audit-architecture | Fault-tolerance focuses on runtime resilience; architecture focuses on structural design |

### 9.5 Recommended Execution Order

Fault tolerance squad runs at position 6 in full audit sequence:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** ← This squad — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-consolidated** — Aggregate all findings
13. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.
