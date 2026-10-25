---
name: aspnetcore-health
description: "ASP.NET Core health check patterns, startup validation, readiness/liveness probes, OpenTelemetry integration, structured logging with Serilog, Grafana/Loki/Tempo/Mimir stack, Docker healthcheck integration, CI/CD health gates, dependency health monitoring. USE FOR: health check endpoints, startup validation, production readiness, observability setup, structured logging, OTel integration, container health checks, deployment verification. DO NOT USE FOR: frontend-only projects, non-.NET backends, pure infrastructure monitoring."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET 8+", "ASP.NET Core", "OpenTelemetry"]
---

# ASP.NET Core Health Checks & Observability

Production-ready patterns for health endpoints, structured logging, and observability in ASP.NET Core.

---

## 1. Health Check Fundamentals

Register health checks in `Program.cs` and map as endpoints. Framework ships built-in checks for common deps.

**Registration:**

```csharp
// Program.cs
builder.Services.AddHealthChecks()
    .AddNpgSql(
        connectionString: builder.Configuration.GetConnectionString("Postgres")!,
        name: "postgresql",
        tags: ["db", "ready"])
    .AddRedis(
        redisConnectionString: builder.Configuration.GetConnectionString("Redis")!,
        name: "redis",
        tags: ["cache", "ready"])
    .AddUrlGroup(
        uri: new Uri("https://api.external.com/health"),
        name: "external-api",
        tags: ["external", "ready"])
    .AddCheck<CustomLogicHealthCheck>("custom-logic", tags: ["ready"]);

app.MapHealthChecks("/health");
```

> **Additive registration:** `AddHealthChecks()` is additive — calling from multiple module registrations merges all into single health check service. Each module registers independently.

**The IHealthCheck interface:**

Return `Healthy()`, `.Degraded()`, or `.Unhealthy()` with optional data payloads.

```csharp
public class CustomLogicHealthCheck : IHealthCheck
{
    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        return Task.FromResult(HealthCheckResult.Healthy("All good"));
    }
}
```

**Built-in checks:**

| Check | Package | What it verifies |
|-------|---------|-----------------|
| `AddNpgSql` | `AspNetCore.HealthChecks.NpgSql` | PostgreSQL connectivity |
| `AddRedis` | `AspNetCore.HealthChecks.Redis` | Redis/Dragonfly ping |
| `AddSqlServer` | `AspNetCore.HealthChecks.SqlServer` | SQL Server connectivity |
| `AddUrlGroup` | `AspNetCore.HealthChecks.Uris` | HTTP endpoint reachability |
| `AddAzureKeyVault` | `AspNetCore.HealthChecks.AzureKeyVault.Secrets` | Key Vault secret access |
| `AddRabbitMQ` | `AspNetCore.HealthChecks.RabbitMQ` | RabbitMQ connection |
| `AddIdentityServer` | `AspNetCore.HealthChecks.UI.Client` | Identity Server token endpoint |

> **NuGet for `IHealthCheck`:** `Microsoft.Extensions.Diagnostics.HealthChecks.Abstractions` needs explicit NuGet ref in module projects. NOT available transitively.

---

## 2. Liveness vs Readiness vs Startup

Three probe types. Map to separate endpoints for independent polling.

```csharp
app.MapHealthChecks("/health/live", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("live"),
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
});

> **AllowAnonymous for probe endpoints:** Health endpoints for Docker/K8s probes need `.AllowAnonymous()` — otherwise orchestrators/monitoring tools can't reach them:
> ```csharp
> app.MapHealthChecks("/health/ready", new HealthCheckOptions { ... }).AllowAnonymous();
> ```

app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready"),
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
});

app.MapHealthChecks("/health/started", new HealthCheckOptions
{
    Predicate = _ => false // No dep checks, just process alive
});
```

**Probe meanings:**
- **Liveness** (`/health/live`): Process alive, not deadlocked. No dep checks. Failure = orchestrator restarts container.
- **Readiness** (`/health/ready`): All deps reachable. Failure = pod removed from LB, not restarted.
- **Startup** (`/health/started`): One-time init completion check. K8s delays traffic until init finishes.

**Kubernetes mapping:**

```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /health/ready
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 5

startupProbe:
  httpGet:
    path: /health/started
    port: 8080
  failureThreshold: 30
  periodSeconds: 2
```

---

## 3. Custom Health Checks

Write app-specific checks. Keep fast and idempotent.

```csharp
public class PaymentGatewayHealthCheck : IHealthCheck
{
    private readonly IPaymentGatewayClient _client;
    private readonly ILogger<PaymentGatewayHealthCheck> _logger;

    public PaymentGatewayHealthCheck(IPaymentGatewayClient client, ILogger<PaymentGatewayHealthCheck> logger)
    {
        _client = client; _logger = logger;
    }

    public async Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context, CancellationToken cancellationToken = default)
    {
        try
        {
            using var cts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            cts.CancelAfter(TimeSpan.FromSeconds(5));
            var isHealthy = await _client.PingAsync(cts.Token);
            return isHealthy
                ? HealthCheckResult.Healthy("Payment gateway reachable")
                : HealthCheckResult.Degraded("Payment gateway responding but slow");
        }
        catch (OperationCanceledException)
        {
            return HealthCheckResult.Unhealthy("Payment gateway timeout");
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Payment gateway health check failed");
            return HealthCheckResult.Unhealthy("Payment gateway error", ex);
        }
    }
}
```

> **CA1848 — use `LoggerMessage.Define`:** With TreatWarningsAsErrors, CA1848 fires on logging calls in hot paths. Use source generator:
> ```csharp
> internal static partial class Log
> {
>     [LoggerMessage(Level = LogLevel.Warning, Message = "Health check {CheckName} failed")]
>     public static partial void HealthCheckFailed(this ILogger logger, string checkName);
> }
> ```

> **CA1031 suppression justified:** Health checks must never throw. Suppress CA1031 with comment: `// CA1031: Health checks must never throw — degraded results are expected`.

**Register with timeout:**

```csharp
builder.Services.AddHealthChecks()
    .AddCheck<PaymentGatewayHealthCheck>(
        "payment-gateway",
        failureStatus: HealthStatus.Degraded,
        tags: ["external", "ready"],
        timeout: TimeSpan.FromSeconds(5));
```

`failureStatus: HealthStatus.Degraded` means payment gateway failure won't kill readiness probe — returns 200 with degraded payload instead of 503.

---

## 4. Health Check UI

Dashboard + client reporting to central monitoring endpoint.

```csharp
// Install: AspNetCore.HealthChecks.UI, AspNetCore.HealthChecks.UI.InMemory.Storage
// Install: AspNetCore.HealthChecks.UI.Client (for self-reporting)

builder.Services.AddHealthChecksUI().AddInMemoryStorage();

builder.Services.AddHealthChecks()
    .AddCheck<PaymentGatewayHealthCheck>("payment-gateway")
    .Services.AddSingleton(sp => new HealthCheckUISettings
    {
        HealthCheckDatabaseConnectionString = "InMemory",
    });

app.MapHealthChecksUI();
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = _ => true,
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
});
```

**Custom JSON response writer:**

```csharp
public static class CustomHealthResponseWriter
{
    public static async Task WriteJsonResponse(HttpContext context, HealthReport report)
    {
        context.Response.ContentType = "application/json";
        var response = new
        {
            status = report.Status.ToString(),
            totalDurationMs = report.TotalDuration.TotalMilliseconds,
            checks = report.Entries.Select(entry => new
            {
                name = entry.Key,
                status = entry.Value.Status.ToString(),
                description = entry.Value.Description,
                durationMs = entry.Value.Duration.TotalMilliseconds,
                exception = entry.Value.Exception?.Message,
                data = entry.Value.Data
            })
        };
        await context.Response.WriteAsJsonAsync(response);
    }
}

app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    ResponseWriter = CustomHealthResponseWriter.WriteJsonResponse
});
```

---

## 5. Startup Validation

Fail fast when required config or services missing. Runs once during boot, crashes container if wrong.

```csharp
public class StartupHealthCheck : IHealthCheck
{
    private readonly IConfiguration _configuration;

    public StartupHealthCheck(IConfiguration configuration) => _configuration = configuration;

    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context, CancellationToken cancellationToken = default)
    {
        var errors = new List<string>();
        if (string.IsNullOrWhiteSpace(_configuration.GetConnectionString("Postgres")))
            errors.Add("PostgreSQL connection string is missing");
        if (string.IsNullOrWhiteSpace(_configuration["Keycloak:Authority"]))
            errors.Add("Keycloak authority is missing");
        if (string.IsNullOrWhiteSpace(_configuration["Encryption:Key"]))
            errors.Add("Encryption key is not configured");

        return errors.Count == 0
            ? Task.FromResult(HealthCheckResult.Healthy("Startup validation passed"))
            : Task.FromResult(HealthCheckResult.Unhealthy("Startup validation failed", null,
                new Dictionary<string, object> { ["errors"] = errors }));
    }
}
```

**Register and run at startup:**

```csharp
builder.Services.AddHealthChecks()
    .AddCheck<StartupHealthCheck>("startup", tags: ["startup"]);

app.MapHealthChecks("/health/started", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("startup"),
    ResultStatusCodes =
    {
        [HealthStatus.Healthy] = StatusCodes.Status200OK,
        [HealthStatus.Degraded] = StatusCodes.Status503ServiceUnavailable,
        [HealthStatus.Unhealthy] = StatusCodes.Status503ServiceUnavailable,
    }
});
```

**Programmatic startup check (crash immediately):**

```csharp
var healthCheckService = app.Services.GetRequiredService<HealthCheckService>();
var startupReport = await healthCheckService.CheckHealthAsync(
    predicate: check => check.Tags.Contains("startup"));

if (startupReport.Status != HealthStatus.Healthy)
{
    var errors = startupReport.Entries
        .Where(e => e.Value.Status == HealthStatus.Unhealthy)
        .Select(e => $"{e.Key}: {e.Value.Description ?? e.Value.Exception?.Message}");
    throw new InvalidOperationException($"Startup health check failed: {string.Join("; ", errors)}");
}
```

---

## 6. Structured Logging with Serilog

Every log event carries typed properties, not just formatted strings.

**Setup:**

```csharp
Log.Logger = new LoggerConfiguration()
    .Enrich.FromLogContext()
    .Enrich.WithMachineName()
    .Enrich.WithProcessId()
    .Enrich.WithThreadId()
    .Enrich.WithProperty("Application", "MyApp")
    .WriteTo.Console(outputTemplate: "[{Timestamp:HH:mm:ss} {Level:u3}] {Message:lj} {Properties:j}{NewLine}{Exception}")
    .WriteTo.File(path: "/var/log/app/log-.txt", rollingInterval: RollingInterval.Day, retainedFileCountLimit: 30,
        outputTemplate: "{Timestamp:yyyy-MM-dd HH:mm:ss.fff zzz} [{Level:u3}] {SourceContext} {Message:lj} {Properties:j}{NewLine}{Exception}")
    .WriteTo.Seq(serverUrl: "http://localhost:5341")
    .CreateLogger();

builder.Host.UseSerilog();
```

> **Serilog config in `appsettings.json`:** Serilog reads from its own top-level `Serilog` section, NOT nested under `Logging`:
> ```json
> { "Serilog": { "MinimumLevel": { "Default": "Information" }, "WriteTo": [...] } }
> ```
> Do NOT nest under `"Logging": { "Serilog": ... }` — silently ignored.

**Request logging middleware:**

```csharp
app.UseSerilogRequestLogging(options =>
{
    options.MessageTemplate = "HTTP {RequestMethod} {RequestPath} responded {StatusCode} in {ElapsedMilliseconds:0.000}ms";
    options.EnrichDiagnosticContext = (diagnosticContext, httpContext) =>
    {
        diagnosticContext.Set("RequestHost", httpContext.Request.Host.Value);
        diagnosticContext.Set("UserAgent", httpContext.Request.Headers["User-Agent"].ToString());
        diagnosticContext.Set("UserIdentity", httpContext.User.Identity?.Name);
        diagnosticContext.Set("ClientIP", httpContext.Connection.RemoteIpAddress?.ToString());
    };
    options.GetLevel = (httpContext, elapsed, ex) =>
    {
        if (ex != null) return Serilog.Events.LogEventLevel.Error;
        if (httpContext.Response.StatusCode >= 500) return Serilog.Events.LogEventLevel.Error;
        if (httpContext.Response.StatusCode >= 400) return Serilog.Events.LogEventLevel.Warning;
        return Serilog.Events.LogEventLevel.Information;
    };
});
```

**Correlation IDs with middleware:**

```csharp
public class CorrelationIdMiddleware
{
    private const string HeaderName = "X-Correlation-ID";
    private readonly RequestDelegate _next;
    public CorrelationIdMiddleware(RequestDelegate next) => _next = next;

    public async Task InvokeAsync(HttpContext context)
    {
        var correlationId = context.Request.Headers.TryGetValue(HeaderName, out var header)
            ? header.ToString() : Guid.NewGuid().ToString();
        using (LogContext.PushProperty("CorrelationId", correlationId))
        {
            context.Response.Headers.Append(HeaderName, correlationId);
            await _next(context);
        }
    }
}
```

---

## 7. OpenTelemetry Integration

OTel gives metrics, traces, logs with vendor-neutral SDK. OTLP exporter sends to collector.

```csharp
builder.Services.AddOpenTelemetry()
    .ConfigureResource(resource => resource
        .AddService(serviceName: builder.Configuration["OTel:ServiceName"] ?? "MyApp",
            serviceVersion: typeof(Program).Assembly.GetName().Version?.ToString() ?? "1.0.0")
        .AddAttributes(new Dictionary<string, object>
        {
            ["deployment.environment"] = builder.Environment.EnvironmentName
        }))
    .WithTracing(tracing => tracing
        .AddSource("MyApp.*")
        .AddAspNetCoreInstrumentation(options => { options.Filter = ctx => !ctx.Request.Path.StartsWithSegments("/health"); })
        .AddHttpClientInstrumentation()
        .AddNpgsql()
        .AddRedisInstrumentation()
        .AddOtlpExporter(options => { options.Endpoint = new Uri(builder.Configuration["OTel:OtlpEndpoint"] ?? "http://localhost:4317"); }))
    .WithMetrics(metrics => metrics
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddMeter("MyApp.Metrics")
        .AddOtlpExporter(options => { options.Endpoint = new Uri(builder.Configuration["OTel:OtlpEndpoint"] ?? "http://localhost:4317"); }));
```

**Custom metrics:**

```csharp
public class OrderMetrics
{
    private readonly Counter<long> _ordersCreated;
    private readonly Histogram<double> _orderProcessingDuration;
    private readonly UpDownCounter<int> _activeOrders;

    public OrderMetrics(IMeterFactory meterFactory)
    {
        var meter = meterFactory.Create("MyApp.Metrics");
        _ordersCreated = meter.CreateCounter<long>("app.orders.created", unit: "{order}", description: "Number of orders created");
        _orderProcessingDuration = meter.CreateHistogram<double>("app.orders.processing_duration", unit: "ms", description: "Order processing duration");
        _activeOrders = meter.CreateUpDownCounter<int>("app.orders.active", unit: "{order}", description: "Current orders being processed");
    }

    public void OrderCreated(string paymentMethod) => _ordersCreated.Add(1, new("payment_method", paymentMethod));
    public OrderProcessingScope MeasureProcessing() => new(_orderProcessingDuration);
    public void IncrementActiveOrders() => _activeOrders.Add(1);
    public void DecrementActiveOrders() => _activeOrders.Add(-1);
}

public readonly struct OrderProcessingScope : IDisposable
{
    private readonly Histogram<double> _histogram;
    private readonly long _startTimestamp;
    public OrderProcessingScope(Histogram<double> histogram) { _histogram = histogram; _startTimestamp = Stopwatch.GetTimestamp(); }
    public void Dispose() { var elapsed = Stopwatch.GetElapsedTime(_startTimestamp); _histogram.Record(elapsed.TotalMilliseconds); }
}
```

**Custom activity (span):**

```csharp
using System.Diagnostics;

public class OrderService
{
    private static readonly ActivitySource ActivitySource = new("MyApp.Orders");

    public async Task ProcessOrderAsync(Order order)
    {
        using var activity = ActivitySource.StartActivity("ProcessOrder");
        activity?.SetTag("order.id", order.Id.ToString());
        activity?.SetTag("order.total", order.Total);
        await _paymentGateway.ChargeAsync(order);
    }
}
```

---

## 8. Observability Stack (Grafana)

Local stack: Grafana, Loki, Tempo, Mimir — dashboards for logs, traces, metrics without paid SaaS.

**docker-compose.observability.yml:**

```yaml
services:
  tempo:
    image: grafana/tempo:latest
    command: ["-config.file=/etc/tempo.yaml"]
    volumes:
      - ./tempo.yaml:/etc/tempo.yaml
    ports:
      - "4317:4317"   # OTLP gRPC
      - "4318:4318"   # OTLP HTTP

  loki:
    image: grafana/loki:latest
    ports:
      - "3100:3100"

  mimir:
    image: grafana/mimir:latest
    command: ["-config.file=/etc/mimir.yaml"]
    volumes:
      - ./mimir.yaml:/etc/mimir.yaml
    ports:
      - "9009:9009"

  grafana:
    image: grafana/grafana:latest
    environment:
      GF_AUTH_ANONYMOUS_ENABLED: "true"
      GF_AUTH_ANONYMOUS_ORG_ROLE: "Admin"
    ports:
      - "3000:3000"
    depends_on:
      - tempo
      - loki
      - mimir
```

**OTel Collector config (logs→Loki, traces→Tempo, metrics→Mimir):**

```yaml
# otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc: { endpoint: 0.0.0.0:4317 }
      http: { endpoint: 0.0.0.0:4318 }

processors:
  batch:
    timeout: 5s
    send_batch_size: 1024

exporters:
  otlphttp: { endpoint: http://tempo:4318 }
  loki: { endpoint: http://loki:3100/loki/api/v1/push }
  prometheusremotewrite: { endpoint: http://mimir:9009/api/v1/push }

service:
  pipelines:
    traces:  { receivers: [otlp], processors: [batch], exporters: [otlphttp] }
    logs:    { receivers: [otlp], processors: [batch], exporters: [loki] }
    metrics: { receivers: [otlp], processors: [batch], exporters: [prometheusremotewrite] }
```

**Serilog OTLP sink:**

```csharp
builder.Services.AddSerilog(configure =>
{
    configure.Enrich.FromLogContext()
        .WriteTo.OpenTelemetry(options =>
        {
            options.Endpoint = "http://otel-collector:4317";
            options.Protocol = OtlpProtocol.Grpc;
            options.ResourceAttributes = new Dictionary<string, object> { ["service.name"] = "MyApp" };
        });
});
```

---

## 9. Docker Integration

Docker can poll health endpoints for container state.

**Dockerfile:**

```dockerfile
FROM mcr.microsoft.com/dotnet/aspnet:8.0 AS base
WORKDIR /app
EXPOSE 8080

RUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health/live || exit 1
```

**Without curl (wget):**

```dockerfile
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q --spider http://localhost:8080/health/live || exit 1
```

**docker-compose.yml with health-dependent startup:**

```yaml
services:
  app:
    build: .
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health/ready"]
      interval: 15s
      timeout: 5s
      start_period: 20s
      retries: 3

  worker:
    build: .
    command: ["dotnet", "MyApp.Worker.dll"]
    depends_on:
      app:
        condition: service_healthy

  nginx:
    image: nginx:alpine
    depends_on:
      app:
        condition: service_healthy
```

---

## 10. CI/CD Health Gates

Every deploy pipeline should verify app is healthy before marking stage passed. Catches misconfigs, missing secrets, broken migrations.

**Bash script for post-deploy verification:**

```bash
#!/usr/bin/env bash
# health-check-deploy.sh
set -euo pipefail

HEALTH_URL="${1:?Usage: health-check-deploy.sh <url>}"
MAX_WAIT_SECONDS="${2:-120}"
INTERVAL_SECONDS=5
ATTEMPTS=$((MAX_WAIT_SECONDS / INTERVAL_SECONDS))

echo "Waiting for healthy response from ${HEALTH_URL} (max ${MAX_WAIT_SECONDS}s)..."

for i in $(seq 1 $ATTEMPTS); do
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${HEALTH_URL}" || true)
    if [ "$HTTP_CODE" = "200" ]; then
        BODY=$(curl -s "${HEALTH_URL}")
        STATUS=$(echo "$BODY" | jq -r '.status // "unknown"')
        if [ "$STATUS" = "Healthy" ]; then
            echo "Health check passed after $((i * INTERVAL_SECONDS))s"
            exit 0
        else
            echo "Attempt ${i}/${ATTEMPTS}: Status is '${STATUS}' (expected 'Healthy')"
        fi
    else
        echo "Attempt ${i}/${ATTEMPTS}: HTTP ${HTTP_CODE}"
    fi
    sleep $INTERVAL_SECONDS
done

echo "Health check failed after ${MAX_WAIT_SECONDS}s"
exit 1
```

**GitLab CI usage:**

```yaml
deploy:
  stage: deploy
  script:
    - docker compose up -d
  environment: staging

health-check:
  stage: verify
  needs: [deploy]
  script:
    - bash scripts/health-check-deploy.sh "https://staging.example.com/health/ready" 120
  retry:
    max: 2
    when: script_failure

rollback:
  stage: rollback
  needs: [health-check]
  when: on_failure
  script:
    - docker compose down
    - docker compose -f docker-compose.yml -f docker/docker-compose.staging.yml up -d
    - bash scripts/health-check-deploy.sh "https://staging.example.com/health/ready" 120
```

**PowerShell version:**

```powershell
param(
    [Parameter(Mandatory)][string]$Url,
    [int]$TimeoutSeconds = 120,
    [int]$IntervalSeconds = 5
)

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
while ($stopwatch.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    try {
        $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 5
        if ($response.StatusCode -eq 200) {
            $body = $response.Content | ConvertFrom-Json
            if ($body.status -eq "Healthy") {
                Write-Host "Health check passed after $($stopwatch.Elapsed.TotalSeconds)s"
                exit 0
            }
        }
    } catch {
        Write-Host "Attempt failed: $($_.Exception.Message)"
    }
    Start-Sleep -Seconds $IntervalSeconds
}
Write-Error "Health check failed after ${TimeoutSeconds}s"
exit 1
```

---

## 11. Anti-Patterns

Repeated mistakes in production .NET services.

**Don't skip health checks in CI/CD.** Deploy without health gate means broken releases reach users. Every env needs post-deploy health verification.

**Don't check external services in liveness probes.** Third-party API down → liveness fails → orchestrator restarts container. Restart won't fix external API → restart loop. External checks belong in readiness only, use `failureStatus: HealthStatus.Degraded`.

**Don't return HTTP 200 when dependencies unhealthy.** Some teams map everything to 200 and check JSON body. K8s treats 200 as healthy, 503 as unhealthy. Return correct status code.

**Don't log health check requests at Information level.** Health probes fire every few seconds per container. Logging each at Info floods logs, drives up storage costs. Filter out or log at Debug.

```csharp
// Option A: Filter in Serilog
app.UseSerilogRequestLogging(options =>
{
    options.GetLevel = (ctx, _, ex) =>
        ctx.Request.Path.StartsWithSegments("/health")
            ? Serilog.Events.LogEventLevel.Debug
            : /* your normal logic */;
});

// Option B: Skip middleware for health endpoints
app.UseWhen(
    ctx => !ctx.Request.Path.StartsWithSegments("/health"),
    branch => branch.UseSerilogRequestLogging());
```

**Don't expose detailed health data on public endpoints.** Readiness returns dep names, connection strings in errors, internal topology. Map detailed response to internal endpoint, return simple status to public.

```csharp
// Public: simple status only
app.MapHealthChecks("/health", new HealthCheckOptions
{
    ResponseWriter = async (ctx, report) =>
    {
        ctx.Response.StatusCode = report.Status == HealthStatus.Healthy ? 200 : 503;
        await ctx.Response.WriteAsync(report.Status.ToString());
    }
});

// Internal: full details (restricted by network or auth)
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
}).RequireAuthorization("InternalOnly");
```

**Don't put expensive queries in health checks.** `SELECT COUNT(*) FROM orders` on every check hammers DB at scale. Use lightweight `SELECT 1` or `pg_is_in_recovery()`.

## Related Skills

- **postgresql-efcore** — EF Core patterns including IDbContextFactory used by health checks
- **dotnet-clean-arch** — Project structure where health checks are registered in Program.cs
- **docker-deployment** — Docker HEALTHCHECK and compose service_healthy conditions
- **signalr-realtime** — Health monitoring for SignalR connections and backplane
- **quartz-scheduling** — Background job health and scheduler status monitoring