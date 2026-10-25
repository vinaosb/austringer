---
name: audit-observability
description: >-
  Observability and telemetry auditing: OpenTelemetry instrumentation verification,
  distributed tracing validation, logging completeness, span coverage, health check
  alignment, and alert configuration correctness across all technology stacks.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Observability & Telemetry Audit

## USE FOR

- OpenTelemetry instrumentation verification — check tracing setup, span creation, metric export
- Distributed tracing validation — verify trace context propagation across service boundaries
- Logging completeness — verify structured logging covers all critical paths, log levels appropriate
- Span coverage — identify uninstrumented code paths, missing span attributes, uninstrumented middleware
- Health check alignment — verify health probes match observability signals and alert configurations
- Dashboard/alert verification — verify monitoring dashboards capture key metrics with correct query names
- Log aggregation review — verify logs flow to correct aggregation system (Loki, ELK, CloudWatch)
- OTel SDK setup completeness — verify ASP.NET Core + HttpClient + EF Core/Npgsql + Redis instrumentation
- Serilog configuration correctness — verify sinks, enrichers, log levels, and output templates
- Alert metric name correctness — verify alert queries match exported metric names
- Custom business metric alerting — verify domain-specific metrics are exported and alerted on
- Degraded dependency handling in readiness probes — verify readiness probes distinguish healthy vs degraded
- Message bus middleware instrumentation — verify spans are created for message handlers

### DO NOT USE FOR

- Test coverage analysis (use `audit-test-coverage`)
- Performance profiling and benchmarking (this is audit, not profiling)
- Infrastructure provisioning (use `audit-security`)
- Log content privacy review (use `audit-privacy-data`)
- Fault tolerance patterns like retry/circuit-breaker (use `audit-fault-tolerance`)

---

## Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (treat as JavaScript superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` containing `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` or `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` containing `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` containing `actix` | Actix Web |
| Gin | `go.mod` containing `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` containing `xunit`/`nunit`/`MSTest` | .NET test frameworks |
| pytest | `pytest.ini`, `pyproject.toml` with `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` files | Go testing |
| JUnit | `*Test.java` with `@Test` annotations | JUnit |
| Cargo test | `Cargo.toml` with `[dev-dependencies]` | Rust test |

### 3.4 CI/CD Detection

| Signal | File Pattern | Platform |
|--------|-------------|----------|
| GitLab CI | `.gitlab-ci.yml` | GitLab |
| GitHub Actions | `.github/workflows/*.yml` | GitHub |
| Jenkins | `Jenkinsfile` | Jenkins |
| CircleCI | `.circleci/config.yml` | CircleCI |
| Azure Pipelines | `azure-pipelines.yml`, `.azure-pipelines.yml` | Azure DevOps |

### 3.5 Infrastructure Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml`, `docker-compose*.yml` | Containerization |
| Kubernetes | `*.yaml` with `apiVersion` + `kind` | Orchestration |
| Terraform | `*.tf` | Infrastructure as Code |
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` in dependencies | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root directory** for all file patterns above.
2. **Identify primary language** by file count (most source files wins).
3. **Identify frameworks** by config file analysis.
4. **Identify test framework** by test directory/config presence.
5. **Identify CI/CD** by workflow file presence.
6. **Identify infrastructure** by Docker/ORM/observability configs.
7. **Select the matching checklist** from Section 4 based on detected technologies.
8. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 .NET/C# Observability Checklist

- [ ] **OTel SDK setup completeness**: Verify ASP.NET Core instrumentation (`AddAspNetCoreInstrumentation`)
- [ ] **HttpClient instrumentation**: `AddHttpClientInstrumentation()` registered for outbound HTTP calls
- [ ] **EF Core/Npgsql instrumentation**: `AddNpgsqlInstrumentation()` or EF Core instrumentation — database spans must be visible
- [ ] **Redis/Dragonfly instrumentation**: StackExchange.Redis instrumentation registered if caching is used
- [ ] **Serilog configuration**: Sinks configured (Loki, Console), enrichers (ThreadId, TraceId, SpanId), log level override
- [ ] **Health check alignment with alerts**: Each registered health check has a corresponding alert rule
- [ ] **Span coverage for message bus**: Wolverine middleware creates spans for command/message handlers
- [ ] **Alert metric name correctness**: Alert queries reference exact exported metric names (no typos, no stale names)
- [ ] **Custom business metric export**: Domain-specific counters/histograms exported via OTel Meter
- [ ] **Readiness probe vs health check**: Readiness probe reflects actual dependency health, not just `probe_success`
- [ ] **Degraded state handling**: Health checks returning `Degraded` are surfaced in readiness probes, not masked
- [ ] **Structured logging enrichment**: All log entries include TraceId, SpanId, TenantId where applicable
- [ ] **Exporter configuration**: OTLP exporter endpoint configured, sampling strategy appropriate
- [ ] **Wolverine middleware wiring**: Span helpers defined AND wired into middleware pipeline (not just defined)
- [ ] **Metric naming convention**: OTel metrics follow naming convention (dot-separated, lowercase)
- [ ] **Health check tags**: Each health check uses distinct tags for liveness vs readiness grouping

### 4.2 Python Observability Checklist

- [ ] **OTel SDK setup**: `opentelemetry-sdk`, `opentelemetry-instrumentation-*` packages installed and configured
- [ ] **Auto-instrumentation**: Flask/Django/FastAPI auto-instrumentation enabled or manual spans cover all endpoints
- [ ] **Structured logging**: `structlog` or `logging-json` configured with trace context injection
- [ ] **Health check endpoints**: `/health` and `/ready` endpoints return actual dependency status
- [ ] **Metric export**: Prometheus or OTLP exporter configured with appropriate endpoints
- [ ] **Span attributes**: Custom spans include meaningful attributes (operation, tenant, module)
- [ ] **Logging completeness**: All exception paths produce structured log entries, not just `print()`
- [ ] **Trace context propagation**: HTTP clients propagate W3C trace context headers

### 4.3 JavaScript/TypeScript Observability Checklist

- [ ] **OTel SDK setup**: `@opentelemetry/sdk-node` or equivalent configured
- [ ] **HTTP instrumentation**: Express/Fastify/NestJS instrumentation for inbound requests
- [ ] **HTTP client instrumentation**: `@opentelemetry/instrumentation-http` for outbound calls
- [ ] **Structured logging**: Pino, Winston, or Bunyan with JSON output and trace correlation
- [ ] **Health endpoints**: `/health`, `/ready` with actual dependency checks, not just 200 OK
- [ ] **Custom metrics**: Business metrics exported via OTel Meter or Prometheus client
- [ ] **Error tracking**: Sentry or equivalent configured with trace correlation

### 4.4 Go Observability Checklist

- [ ] **OTel SDK**: `go.opentelemetry.io/otel` configured with correct exporter
- [ ] **HTTP middleware**: Tracing middleware on all HTTP handlers
- [ ] **Database instrumentation**: `otelsql` or equivalent for database query spans
- [ ] **Structured logging**: `slog` or `zap` with JSON output and trace correlation
- [ ] **Health checks**: Health probe endpoints with dependency status
- [ ] **Metric registration**: Prometheus or OTLP metrics registered and exported

### 4.5 Java Observability Checklist

- [ ] **OTel agent**: Javaagent or manual instrumentation configured
- [ ] **Spring Boot Actuator**: Health and metrics endpoints configured
- [ ] **Micrometer**: Metrics exported to Prometheus or OTLP
- [ ] **Logging**: SLF4J with MDC trace context propagation
- [ ] **Distributed tracing**: Brave or OTel instrumentation for HTTP, database, messaging

### 4.6 Rust Observability Checklist

- [ ] **Tracing crate**: `tracing` with `tracing-subscriber` configured for structured output
- [ ] **OTel integration**: `opentelemetry` crate with appropriate exporter
- [ ] **HTTP instrumentation**: `tower-http` tracing layer on HTTP handlers
- [ ] **Health endpoints**: Health probe endpoints returning dependency status
- [ ] **Metric export**: `metrics` crate with Prometheus or OTLP exporter

---

## Anti-Patterns

### 5.1 Observability-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Missing EF Core/Npgsql instrumentation | high | No DB instrumentation registered | Database spans invisible in traces, cannot identify slow queries | Register `AddNpgsqlInstrumentation()` or EF Core instrumentation in OTel setup |
| 2 | Alert queries using wrong metric names | high | Grafana alert references `http_requests_total` but app exports `http_server_request_duration_seconds_count` | Alerts never fire or fire on wrong data | Verify metric names in alert match exactly what the application exports |
| 3 | Readiness probe only checks `probe_success` | high | `/ready` returns OK even when dependencies return `Degraded` | Traffic routed to unhealthy instances | Readiness probe must aggregate all health check results and fail on degraded/unhealthy |
| 4 | Wolverine span helpers defined but never wired | medium | Span creation methods exist in code but are not called from middleware | Message handlers run without trace spans | Wire span helpers into Wolverine middleware pipeline |
| 5 | Serilog without trace enrichment | medium | Logs lack TraceId/SpanId, breaking log-trace correlation | Cannot correlate logs with distributed traces | Add `Enrich.WithTraceId()` and `Enrich.WithSpanId()` to Serilog config |
| 6 | Health checks without alert rules | medium | Health checks registered but no alerting rules configured | Dependency failures go unnoticed until user reports | Create alert rules for each critical health check |
| 7 | OTel sampling too aggressive | medium | `TraceIdRatioBased` sampler set to <1% in development | Most traces dropped, gaps in observability | Use parent-based sampling; 100% in dev, lower in production |
| 8 | Missing `AsNoTracking` span attribution | low | Read queries not attributed to specific operation | Cannot distinguish read vs write query performance | Add span attributes for query type and operation name |
| 9 | Structured logging with string interpolation | low | `logger.LogInformation($"User {userId} logged in")` | Structured logging lost, cannot filter/query by userId | Use `logger.LogInformation("User {UserId} logged in", userId)` |
| 10 | Log level misconfiguration | medium | Information logs filtered out in production | Important operational data lost | Review log level overrides, ensure critical paths log at Information minimum |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets in observability config | critical | Database credentials, API keys in OTel exporter config | Credentials exposed in telemetry infrastructure | Use environment variables or secret manager for all credentials |
| 2 | Health check returning 200 for all states | high | `/health` always returns HTTP 200 regardless of dependency status | Load balancer routes traffic to broken instances | Map health statuses to HTTP codes: Healthy→200, Degraded→503, Unhealthy→503 |
| 3 | Missing context propagation headers | high | Outbound HTTP calls missing W3C trace headers | Distributed trace broken at service boundary | Ensure all HTTP clients inject traceparent/tracestate headers |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: OTel Instrumentation Setup — Missing DB Spans

```csharp
// ❌ BAD: No database instrumentation — DB spans invisible in traces
builder.Services.AddOpenTelemetry()
    .WithTracing(tracing => tracing
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddOtlpExporter(opts => opts.Endpoint = new Uri(otelEndpoint)));

// ✅ GOOD: All instrumentation sources registered — full trace visibility
builder.Services.AddOpenTelemetry()
    .WithTracing(tracing => tracing
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddNpgsqlInstrumentation()           // PostgreSQL spans
        .AddSource("Wolverine")                // Message bus spans
        .AddOtlpExporter(opts => opts.Endpoint = new Uri(otelEndpoint)));
```

#### Example 2: Health Check Readiness Probe — Masking Degraded State

```csharp
// ❌ BAD: Readiness only checks probe_success, ignores degraded dependencies
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready"),
    ResponseWriter = async (context, report) =>
    {
        // Returns 200 even when status is Degraded
        context.Response.StatusCode = 200;
        await context.Response.WriteAsync("OK");
    }
});

// ✅ GOOD: Readiness fails on degraded dependencies
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready"),
    ResultStatusCodes =
    {
        [HealthStatus.Healthy] = StatusCodes.Status200OK,
        [HealthStatus.Degraded] = StatusCodes.Status503ServiceUnavailable,
        [HealthStatus.Unhealthy] = StatusCodes.Status503ServiceUnavailable
    }
});
```

### 6.2 Python Examples

#### Example 1: Structured Logging Without Trace Context

```python
# ❌ BAD: Logs without trace context — cannot correlate with distributed traces
import logging

logger = logging.getLogger(__name__)
logger.info("Processing order %s for customer %s", order_id, customer_id)

# ✅ GOOD: Structured logging with trace context injection
import structlog

logger = structlog.get_logger()
logger.info(
    "processing_order",
    order_id=order_id,
    customer_id=customer_id,
    trace_id=trace.get_current_span().get_span_context().trace_id,
    span_id=trace.get_current_span().get_span_context().span_id,
)
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: Alert Metric Name Mismatch

```yaml
# ❌ BAD: Alert references wrong metric name — will never fire
alert: HighErrorRate
expr: rate(http_requests_errors_total[5m]) / rate(http_requests_total[5m]) > 0.05
# App actually exports: http_server_request_duration_seconds_count{error="true"}

# ✅ GOOD: Alert references exact exported metric name
alert: HighErrorRate
expr: |
  sum(rate(http_server_request_duration_seconds_count{error="true"}[5m]))
  /
  sum(rate(http_server_request_duration_seconds_count[5m]))
  > 0.05
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the observability gap or misconfiguration.",
  "recommendation": "Specific fix with configuration/code example. Reference OTel or logging docs.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "observability|security|performance|maintainability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the misconfigured pattern. No hedging ("might be a problem").
- **recommendation**: Actionable. Include a config/code snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns that span multiple files in different languages.
- **category**: Primary category. Use the most specific category.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-observability",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "OTel", "Grafana"],
    "totalFindings": 12,
    "bySeverity": {
      "critical": 0,
      "high": 3,
      "medium": 7,
      "low": 2
    },
    "byCategory": {
      "observability": 10,
      "performance": 1,
      "security": 1
    },
    "filesScanned": 35,
    "filesWithFindings": 8
  }
}
```

---

## Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Complete observability blind spot in production | No tracing at all, no health checks on critical dependencies |
| **high** | Significant gap in trace/log/alert coverage | Missing DB instrumentation, wrong alert metric names, readiness masking degraded |
| **medium** | Degraded observability, missing correlation | Missing Serilog enrichment, unwired middleware spans, overly aggressive sampling |
| **low** | Best practice deviation, minor improvement | Missing span attributes, string interpolation in structured logs |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects production incident response | +1 level | All technologies |
| Finding is in a health check for a critical dependency (database, secrets) | +1 level | All technologies |
| Finding means alerts will never fire on a critical condition | +1 level | All technologies |
| Finding affects multiple services in a distributed system | +1 level | Microservices |
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in documentation/comments | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Missing `AddNpgsqlInstrumentation` → high (DB spans invisible)
- `EnableSensitiveDataLogging` in production OTel → high (PII in traces)
- Wolverine `AddSource` missing → medium (message bus spans invisible)

#### Python
- Missing OTel auto-instrumentation → medium (manual span gaps)
- `print()` in production code paths → low (unstructured output)
- Missing trace context in logs → medium (correlation broken)

#### JavaScript/TypeScript
- Missing `@opentelemetry/instrumentation-http` → medium (outbound calls untraced)
- Unhandled promise rejection without span → high (trace gap on errors)
- Missing health endpoint dependency check → medium (false healthy status)

#### Go
- Missing context propagation in HTTP handlers → high (trace broken)
- Unclosed span in defer → low (span skew)
- Missing `slog` JSON handler → medium (unstructured logs)

#### Java
- Missing Micrometer Prometheus export → medium (metrics invisible)
- Broken SLF4J MDC propagation → medium (log-trace correlation broken)

#### Rust
- Missing `tracing` span in error paths → medium (error traces incomplete)
- `println!` in production code → low (unstructured output)

---

## Integration Notes

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
audit-refactoring ──────────────────┤
audit-product-roadmap ──────────────┤
audit-acquisition-metrics ──────────┤
audit-ai-cost ──────────────────────┤
audit-regulatory-* ─────────────────┤
audit-lmas-traceability ────────────┤
audit-schema-metrics ───────────────┤
audit-consolidated ◄────────────────┘  (consumes all other squad outputs)
audit-consistency-coherence            (consumes all other squad outputs)
```

### 9.2 Upstream Dependencies

- **audit-security**: May inform which endpoints/services need tracing coverage
- **audit-architecture**: May identify service boundaries that need trace context propagation
- **audit-cicd**: May reveal if observability tools are deployed in the pipeline

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-observability | audit-fault-tolerance | Observability focuses on telemetry instrumentation; fault-tolerance focuses on retry/circuit-breaker patterns |
| audit-observability | audit-cicd | Observability focuses on runtime telemetry; CI/CD focuses on pipeline configuration |
| audit-observability | audit-privacy-data | Observability focuses on telemetry completeness; privacy focuses on PII in logs/traces |

### 9.5 Recommended Execution Order

For a full audit, run squads in this sequence:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation (this squad)
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
