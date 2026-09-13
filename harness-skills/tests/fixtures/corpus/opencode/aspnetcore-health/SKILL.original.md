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

Production-ready patterns for health endpoints, structured logging, and observability in ASP.NET Core applications.

---

## 1. Health Check Fundamentals

Register health checks in `Program.cs` and map them as endpoints. The framework ships built-in checks for common dependencies.

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

> **Additive registration:** `AddHealthChecks()` is additive — calling it from multiple module/infrastructure service registrations (e.g., `AuthServiceRegistration`, `ObservabilityServiceRegistration`, `ReportingModuleRegistration`) merges all registrations into a single health check service at runtime. Each module can independently register its own checks without coordination.

**The IHealthCheck interface:**

Every health check implements this contract. Return `HealthCheckResult.Healthy()`, `.Degraded()`, or `.Unhealthy()` with optional data payloads.

```csharp
public class CustomLogicHealthCheck : IHealthCheck
{
    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        // Your logic here
        return Task.FromResult(HealthCheckResult.Healthy("All good"));
    }
}
```

**Built-in checks worth knowing:**

| Check | Package | What it verifies |
|-------|---------|-----------------|
| `AddNpgSql` | `AspNetCore.HealthChecks.NpgSql` | PostgreSQL connectivity |
| `AddRedis` | `AspNetCore.HealthChecks.Redis` | Redis/Dragonfly ping |
| `AddSqlServer` | `AspNetCore.HealthChecks.SqlServer` | SQL Server connectivity |
| `AddUrlGroup` | `AspNetCore.HealthChecks.Uris` | HTTP endpoint reachability |
| `AddAzureKeyVault` | `AspNetCore.HealthChecks.AzureKeyVault.Secrets` | Key Vault secret access |
| `AddRabbitMQ` | `AspNetCore.HealthChecks.RabbitMQ` | RabbitMQ connection |
| `AddIdentityServer` | `AspNetCore.HealthChecks.UI.Client` | Identity Server token endpoint |

> **NuGet for `IHealthCheck`:** `Microsoft.Extensions.Diagnostics.HealthChecks.Abstractions` requires an explicit NuGet reference in module/application projects that define custom health checks. It is NOT available transitively from the framework reference or from `Microsoft.Extensions.Diagnostics.HealthChecks`. Without it, `IHealthCheck` won't resolve in module projects.

---

## 2. Liveness vs Readiness vs Startup

Three distinct probe types serve different purposes. Map them to separate endpoints so orchestrators can poll them independently.

```csharp
app.MapHealthChecks("/health/live", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("live"),
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
});

> **AllowAnonymous for probe endpoints:** Health endpoints mapped for Docker/Kubernetes probes need `.AllowAnonymous()` — otherwise orchestrators and infrastructure monitoring tools (Grafana, nginx upstream checks, Kubernetes liveness probes) can't reach them without authentication tokens:
> ```csharp
> app.MapHealthChecks("/health/ready", new HealthCheckOptions { ... })
>     .AllowAnonymous();
> ```

app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready"),
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
});

app.MapHealthChecks("/health/started", new HealthCheckOptions
{
    Predicate = _ => false // No dependency checks, just process alive
});
```

**What goes in each probe:**

- **Liveness** (`/health/live`): The process is alive and not deadlocked. No dependency checks. If this fails, the orchestrator restarts the container.
- **Readiness** (`/health/ready`): All dependencies (database, cache, external APIs) are reachable. The orchestrator removes the pod from the load balancer when this fails, but does not restart it.
- **Startup** (`/health/started`): One-time check that the application finished initializing. Kubernetes uses this to delay sending traffic until initialization completes.

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

Write your own checks for application-specific health logic. Keep them fast and idempotent.

```csharp
public class PaymentGatewayHealthCheck : IHealthCheck
{
    private readonly IPaymentGatewayClient _client;
    private readonly ILogger<PaymentGatewayHealthCheck> _logger;

    public PaymentGatewayHealthCheck(
        IPaymentGatewayClient client,
        ILogger<PaymentGatewayHealthCheck> logger)
    {
        _client = client;
        _logger = logger;
    }

    public async Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
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

> **CA1848 — use `LoggerMessage.Define` for health check logging:** When `TreatWarningsAsErrors` is enabled, CA1848 fires on all logging calls in hot paths. Health check logging must use the `LoggerMessage.Define` source generator pattern:
> ```csharp
> internal static partial class Log
> {
>     [LoggerMessage(Level = LogLevel.Warning, Message = "Health check {CheckName} failed")]
>     public static partial void HealthCheckFailed(this ILogger logger, string checkName);
> }
> ```

> **CA1031 suppression is justified:** Health checks must never throw — a degraded health check should not crash the readiness probe pipeline. Suppressing CA1031 (catch general exceptions) in health check methods is justified with a comment: `// CA1031: Health checks must never throw — degraded results are expected`. Register suppression at the method level.

**Register with timeout:**

```csharp
builder.Services.AddHealthChecks()
    .AddCheck<PaymentGatewayHealthCheck>(
        "payment-gateway",
        failureStatus: HealthStatus.Degraded,
        tags: ["external", "ready"],
        timeout: TimeSpan.FromSeconds(5));
```

The `failureStatus: HealthStatus.Degraded` setting means a payment gateway failure won't take the whole readiness probe down. It returns HTTP 200 with a degraded payload instead of HTTP 503.

---

## 4. Health Check UI

The Health Checks UI package gives you a dashboard and a client that reports to a central monitoring endpoint.

```csharp
// Install: AspNetCore.HealthChecks.UI, AspNetCore.HealthChecks.UI.InMemory.Storage
// Install: AspNetCore.HealthChecks.UI.Client (for self-reporting)

builder.Services.AddHealthChecksUI()
    .AddInMemoryStorage();

// Self-report to the UI (important for Docker/K8s where the UI polls each instance)
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

**Custom JSON response writer for programmatic consumption:**

```csharp
public static class CustomHealthResponseWriter
{
    public static async Task WriteJsonResponse(
        HttpContext context,
        HealthReport report)
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

// Usage
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    ResponseWriter = CustomHealthResponseWriter.WriteJsonResponse
});
```

---

## 5. Startup Validation

Fail fast when required configuration or services are missing. This is separate from readiness checks. Startup validation runs once during boot and crashes the container if something is wrong.

```csharp
public class StartupHealthCheck : IHealthCheck
{
    private readonly IConfiguration _configuration;

    public StartupHealthCheck(IConfiguration configuration)
    {
        _configuration = configuration;
    }

    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
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
            : Task.FromResult(HealthCheckResult.Unhealthy(
                "Startup validation failed",
                null,
                new Dictionary<string, object> { ["errors"] = errors }));
    }
}
```

**Register and run at startup:**

```csharp
// Add with the "startup" tag only
builder.Services.AddHealthChecks()
    .AddCheck<StartupHealthCheck>("startup", tags: ["startup"]);

// Map to a dedicated endpoint
app.MapHealthChecks("/health/started", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("startup"),
    // Don't cache startup results
    ResultStatusCodes =
    {
        [HealthStatus.Healthy] = StatusCodes.Status200OK,
        [HealthStatus.Degraded] = StatusCodes.Status503ServiceUnavailable,
        [HealthStatus.Unhealthy] = StatusCodes.Status503ServiceUnavailable,
    }
});
```

**Programmatic startup check (crash immediately, don't wait for probes):**

```csharp
var healthCheckService = app.Services.GetRequiredService<HealthCheckService>();
var startupReport = await healthCheckService.CheckHealthAsync(
    predicate: check => check.Tags.Contains("startup"));

if (startupReport.Status != HealthStatus.Healthy)
{
    var errors = startupReport.Entries
        .Where(e => e.Value.Status == HealthStatus.Unhealthy)
        .Select(e => $"{e.Key}: {e.Value.Description ?? e.Value.Exception?.Message}");

    throw new InvalidOperationException(
        $"Startup health check failed: {string.Join("; ", errors)}");
}
```

---

## 6. Structured Logging with Serilog

Structured logging means every log event carries typed properties, not just formatted strings. This makes logs searchable and aggregatable.

**Setup:**

```csharp
// Program.cs - top of file, before building the host
Log.Logger = new LoggerConfiguration()
    .Enrich.FromLogContext()
    .Enrich.WithMachineName()
    .Enrich.WithProcessId()
    .Enrich.WithThreadId()
    .Enrich.WithProperty("Application", "MyApp")
    .WriteTo.Console(
        outputTemplate: "[{Timestamp:HH:mm:ss} {Level:u3}] {Message:lj} {Properties:j}{NewLine}{Exception}")
    .WriteTo.File(
        path: "/var/log/app/log-.txt",
        rollingInterval: RollingInterval.Day,
        retainedFileCountLimit: 30,
        outputTemplate: "{Timestamp:yyyy-MM-dd HH:mm:ss.fff zzz} [{Level:u3}] {SourceContext} {Message:lj} {Properties:j}{NewLine}{Exception}")
    .WriteTo.Seq(serverUrl: "http://localhost:5341")
    .CreateLogger();

builder.Host.UseSerilog();
```

> **Serilog config in `appsettings.json`:** Serilog reads configuration from its own top-level `Serilog` section, NOT nested under `Logging`:
> ```json
> {
>   "Serilog": {
>     "MinimumLevel": { "Default": "Information" },
>     "WriteTo": [
>       { "Name": "Console" },
>       { "Name": "File", "Args": { "path": "/var/log/app/log-.txt", "rollingInterval": "Day" } }
>     ]
>   }
> }
> ```
> Do NOT nest Serilog config under `"Logging": { "Serilog": ... }` — it will be silently ignored.

**Request logging middleware (log every HTTP request with structured fields):**

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
            ? header.ToString()
            : Guid.NewGuid().ToString();

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

OpenTelemetry gives you metrics, traces, and logs with a vendor-neutral SDK. The OTLP exporter sends everything to a collector.

```csharp
// Program.cs
builder.Services.AddOpenTelemetry()
    .ConfigureResource(resource => resource
        .AddService(
            serviceName: builder.Configuration["OTel:ServiceName"] ?? "MyApp",
            serviceVersion: typeof(Program).Assembly.GetName().Version?.ToString() ?? "1.0.0")
        .AddAttributes(new Dictionary<string, object>
        {
            ["deployment.environment"] = builder.Environment.EnvironmentName
        }))
    .WithTracing(tracing => tracing
        .AddSource("MyApp.*")
        .AddAspNetCoreInstrumentation(options =>
        {
            options.Filter = ctx =>
                !ctx.Request.Path.StartsWithSegments("/health");
        })
        .AddHttpClientInstrumentation()
        .AddNpgsql()
        .AddRedisInstrumentation()
        .AddOtlpExporter(options =>
        {
            options.Endpoint = new Uri(
                builder.Configuration["OTel:OtlpEndpoint"] ?? "http://localhost:4317");
        }))
    .WithMetrics(metrics => metrics
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddMeter("MyApp.Metrics")
        .AddOtlpExporter(options =>
        {
            options.Endpoint = new Uri(
                builder.Configuration["OTel:OtlpEndpoint"] ?? "http://localhost:4317");
        }));
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

        _ordersCreated = meter.CreateCounter<long>(
            name: "app.orders.created",
            unit: "{order}",
            description: "Number of orders created");

        _orderProcessingDuration = meter.CreateHistogram<double>(
            name: "app.orders.processing_duration",
            unit: "ms",
            description: "Order processing duration in milliseconds");

        _activeOrders = meter.CreateUpDownCounter<int>(
            name: "app.orders.active",
            unit: "{order}",
            description: "Current number of orders being processed");
    }

    public void OrderCreated(string paymentMethod) =>
        _ordersCreated.Add(1, new("payment_method", paymentMethod));

    public OrderProcessingScope MeasureProcessing() =>
        new(_orderProcessingDuration);

    public void IncrementActiveOrders() => _activeOrders.Add(1);
    public void DecrementActiveOrders() => _activeOrders.Add(-1);
}

public readonly struct OrderProcessingScope : IDisposable
{
    private readonly Histogram<double> _histogram;
    private readonly long _startTimestamp;

    public OrderProcessingScope(Histogram<double> histogram)
    {
        _histogram = histogram;
        _startTimestamp = Stopwatch.GetTimestamp();
    }

    public void Dispose()
    {
        var elapsed = Stopwatch.GetElapsedTime(_startTimestamp);
        _histogram.Record(elapsed.TotalMilliseconds);
    }
}
```

**Custom activity (span) for business logic:**

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

        // The activity auto-propagates to downstream HTTP calls
        await _paymentGateway.ChargeAsync(order);
    }
}
```

---

## 8. Observability Stack (Grafana)

A local observability stack with Grafana, Loki, Tempo, and Mimir gives you dashboards for logs, traces, and metrics without a paid SaaS tool.

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

**OTel Collector config (forward logs to Loki, traces to Tempo, metrics to Mimir):**

```yaml
# otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  batch:
    timeout: 5s
    send_batch_size: 1024

exporters:
  otlphttp:
    endpoint: http://tempo:4318
  loki:
    endpoint: http://loki:3100/loki/api/v1/push
  prometheusremotewrite:
    endpoint: http://mimir:9009/api/v1/push

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [batch]
      exporters: [otlphttp]
    logs:
      receivers: [otlp]
      processors: [batch]
      exporters: [loki]
    metrics:
      receivers: [otlp]
      processors: [batch]
      exporters: [prometheusremotewrite]
```

**Serilog sink that writes to OTLP (so logs flow through the collector):**

```csharp
builder.Services.AddSerilog(configure =>
{
    configure
        .Enrich.FromLogContext()
        .WriteTo.OpenTelemetry(options =>
        {
            options.Endpoint = "http://otel-collector:4317";
            options.Protocol = OtlpProtocol.Grpc;
            options.ResourceAttributes = new Dictionary<string, object>
            {
                ["service.name"] = "MyApp"
            };
        });
});
```

---

## 9. Docker Integration

Docker and Docker Compose can poll your health endpoints to determine container state.

**Dockerfile:**

```dockerfile
FROM mcr.microsoft.com/dotnet/aspnet:8.0 AS base
WORKDIR /app
EXPOSE 8080

# Health check using curl (install it first)
RUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health/live || exit 1
```

**Without curl (use wget instead):**

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

The `service_healthy` condition means nginx and the worker won't start until the app passes its readiness probe. The `start_period` gives the app time to initialize before Docker starts counting failures.

---

## 10. CI/CD Health Gates

Every deployment pipeline should verify the app is healthy before marking the stage as passed. This catches misconfigurations, missing secrets, and broken migrations immediately.

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

**PowerShell version for Windows agents:**

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
    }
    catch {
        Write-Host "Attempt failed: $($_.Exception.Message)"
    }

    Start-Sleep -Seconds $IntervalSeconds
}

Write-Error "Health check failed after ${TimeoutSeconds}s"
exit 1
```

---

## 11. Anti-Patterns

These are mistakes that show up repeatedly in production .NET services.

**Don't skip health checks in CI/CD.** A deploy without a health gate means broken releases reach users. Every environment, even staging, needs a post-deploy health verification step.

**Don't check external services in liveness probes.** If a third-party API goes down, your liveness probe fails, and the orchestrator restarts your container. The restart won't fix the external API, so you get a restart loop. External checks belong in readiness only, and even then consider using `failureStatus: HealthStatus.Degraded` so a single external dependency doesn't take the whole app offline.

**Don't return HTTP 200 when dependencies are unhealthy.** Some teams map everything to 200 and check the JSON body for status. This breaks orchestrator expectations. Kubernetes treats 200 as healthy and 503 as unhealthy. Return the correct status code.

**Don't log health check requests at Information level.** Health probes fire every few seconds per container. Logging each one at Info floods your logs with noise and drives up storage costs. Either filter them out entirely or log at Debug.

```csharp
// Option A: Filter in Serilog
app.UseSerilogRequestLogging(options =>
{
    options.GetLevel = (ctx, _, ex) =>
        ctx.Request.Path.StartsWithSegments("/health")
            ? Serilog.Events.LogEventLevel.Debug
            : /* your normal logic */;
});

// Option B: Skip the middleware entirely for health endpoints
app.UseWhen(
    ctx => !ctx.Request.Path.StartsWithSegments("/health"),
    branch => branch.UseSerilogRequestLogging());
```

**Don't expose detailed health data on public endpoints.** The readiness endpoint returns dependency names, connection strings in error messages, and internal topology. Map the detailed response to an internal endpoint and return a simple status to the public one.

```csharp
// Public: simple status only
app.MapHealthChecks("/health", new HealthCheckOptions
{
    ResponseWriter = async (ctx, report) =>
    {
        ctx.Response.StatusCode = report.Status == HealthStatus.Healthy
            ? 200
            : 503;
        await ctx.Response.WriteAsync(report.Status.ToString());
    }
});

// Internal: full details (restricted by network or auth)
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
}).RequireAuthorization("InternalOnly");
```

**Don't put expensive queries in health checks.** A readiness probe that runs a `SELECT COUNT(*) FROM orders` on every check will hammer your database at scale. Use lightweight queries like `SELECT 1` or `pg_is_in_recovery()` that verify connectivity without load.

## Related Skills

- **postgresql-efcore** — EF Core patterns including IDbContextFactory used by health checks
- **dotnet-clean-arch** — Project structure where health checks are registered in Program.cs
- **docker-deployment** — Docker HEALTHCHECK instruction and compose service_healthy conditions
- **signalr-realtime** — Health monitoring for SignalR connections and backplane
- **quartz-scheduling** — Background job health and scheduler status monitoring
