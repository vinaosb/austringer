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

- OpenTelemetry instrumentation verification — tracing setup, span creation, metric export
- Distributed tracing validation — trace context propagation across service boundaries
- Logging completeness — structured logging covers critical paths, log levels appropriate
- Span coverage — uninstrumented code paths, missing span attributes, uninstrumented middleware
- Health check alignment — health probes match observability signals and alert configs
- Dashboard/alert verification — dashboards capture key metrics with correct queries
- Log aggregation review — logs flow to correct system (Loki, ELK, CloudWatch)
- OTel SDK setup — ASP.NET Core + HttpClient + EF Core/Npgsql + Redis instrumentation
- Serilog configuration — sinks, enrichers, log levels, output templates
- Alert metric name correctness — alert queries match exported metric names
- Custom business metric alerting — domain-specific metrics exported and alerted
- Degraded dependency handling in readiness — readiness distinguishes healthy vs degraded
- Message bus middleware instrumentation — spans for message handlers

### DO NOT USE FOR

- Test coverage analysis (use `audit-test-coverage`)
- Performance profiling (this is audit, not profiling)
- Infrastructure provisioning (use `audit-security`)
- Log content privacy review (use `audit-privacy-data`)
- Fault tolerance patterns (use `audit-fault-tolerance`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (JS superset) |
| Go | `*.go`, `go.mod`, `go.sum` | Go |
| Java | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | Pattern | Framework |
|--------|---------|-----------|
| ASP.NET Core | `Program.cs` + `*.csproj` with `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor |
| EF Core | `*.csproj` with `Microsoft.EntityFrameworkCore` | EF Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` / `pyproject.toml` with `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` with `actix` | Actix Web |
| Gin | `go.mod` with `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Framework |
|--------|---------|-----------|
| xUnit/NUnit/MSTest | `*.csproj` with `xunit`/`nunit`/`MSTest` | .NET |
| pytest | `pytest.ini`, `pyproject.toml` `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` | Go |
| JUnit | `*Test.java` with `@Test` | JUnit |
| Cargo test | `Cargo.toml` `[dev-dependencies]` | Rust |

### 3.4 CI/CD Detection

| Signal | Pattern | Platform |
|--------|---------|----------|
| GitLab CI | `.gitlab-ci.yml` | GitLab |
| GitHub Actions | `.github/workflows/*.yml` | GitHub |
| Jenkins | `Jenkinsfile` | Jenkins |
| CircleCI | `.circleci/config.yml` | CircleCI |
| Azure Pipelines | `azure-pipelines.yml`, `.azure-pipelines.yml` | Azure DevOps |

### 3.5 Infrastructure Detection

| Signal | Pattern | Component |
|--------|---------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml`, `docker-compose*.yml` | Containerization |
| Kubernetes | `*.yaml` with `apiVersion` + `kind` | Orchestration |
| Terraform | `*.tf` | IaC |
| DB migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema mgmt |
| OTel | `otel-collector-config.yml`, `opentelemetry` in deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root** for all patterns above.
2. **Identify primary language** by file count.
3. **Identify frameworks** by config analysis.
4. **Identify test framework** by dir/config.
5. **Identify CI/CD** by workflow files.
6. **Identify infrastructure** by Docker/ORM/observability configs.
7. **Select matching checklist** from Section 4.
8. **If multiple languages**, run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 .NET/C# Observability Checklist

- [ ] **OTel SDK setup**: ASP.NET Core instrumentation (`AddAspNetCoreInstrumentation`)
- [ ] **HttpClient instrumentation**: `AddHttpClientInstrumentation()` for outbound HTTP
- [ ] **EF Core/Npgsql instrumentation**: `AddNpgsqlInstrumentation()` — DB spans visible
- [ ] **Redis/Dragonfly instrumentation**: StackExchange.Redis registered if caching used
- [ ] **Serilog config**: Sinks (Loki, Console), enrichers (ThreadId, TraceId, SpanId), log level override
- [ ] **Health check alignment with alerts**: Each health check has corresponding alert rule
- [ ] **Span coverage for message bus**: Wolverine middleware creates spans for command/message handlers
- [ ] **Alert metric name correctness**: Queries reference exact exported names (no typos/stale)
- [ ] **Custom business metric export**: Domain-specific counters/histograms via OTel Meter
- [ ] **Readiness probe vs health check**: Readiness reflects actual dependency health
- [ ] **Degraded state handling**: `Degraded` surfaced in readiness, not masked
- [ ] **Structured logging enrichment**: All entries include TraceId, SpanId, TenantId
- [ ] **Exporter configuration**: OTLP endpoint configured, sampling strategy appropriate
- [ ] **Wolverine middleware wiring**: Span helpers defined AND wired into pipeline
- [ ] **Metric naming convention**: Dot-separated, lowercase
- [ ] **Health check tags**: Distinct tags for liveness vs readiness grouping

### 4.2 Python Observability Checklist

- [ ] **OTel SDK**: `opentelemetry-sdk`, `opentelemetry-instrumentation-*` installed and configured
- [ ] **Auto-instrumentation**: Flask/Django/FastAPI auto-instrumentation or manual spans cover all endpoints
- [ ] **Structured logging**: `structlog` or `logging-json` with trace context injection
- [ ] **Health endpoints**: `/health` and `/ready` return actual dependency status
- [ ] **Metric export**: Prometheus or OTLP exporter configured
- [ ] **Span attributes**: Custom spans include meaningful attributes (operation, tenant, module)
- [ ] **Logging completeness**: All exception paths produce structured logs, not `print()`
- [ ] **Trace context propagation**: HTTP clients propagate W3C trace context headers

### 4.3 JavaScript/TypeScript Observability Checklist

- [ ] **OTel SDK**: `@opentelemetry/sdk-node` or equivalent configured
- [ ] **HTTP instrumentation**: Express/Fastify/NestJS instrumentation for inbound
- [ ] **HTTP client instrumentation**: `@opentelemetry/instrumentation-http` for outbound
- [ ] **Structured logging**: Pino, Winston, Bunyan with JSON + trace correlation
- [ ] **Health endpoints**: `/health`, `/ready` with actual dependency checks
- [ ] **Custom metrics**: Business metrics via OTel Meter or Prometheus client
- [ ] **Error tracking**: Sentry or equivalent with trace correlation

### 4.4 Go Observability Checklist

- [ ] **OTel SDK**: `go.opentelemetry.io/otel` with correct exporter
- [ ] **HTTP middleware**: Tracing middleware on all HTTP handlers
- [ ] **Database instrumentation**: `otelsql` or equivalent for query spans
- [ ] **Structured logging**: `slog` or `zap` with JSON + trace correlation
- [ ] **Health checks**: Probe endpoints with dependency status
- [ ] **Metric registration**: Prometheus or OTLP metrics registered and exported

### 4.5 Java Observability Checklist

- [ ] **OTel agent**: Javaagent or manual instrumentation configured
- [ ] **Spring Boot Actuator**: Health and metrics endpoints
- [ ] **Micrometer**: Metrics exported to Prometheus or OTLP
- [ ] **Logging**: SLF4J with MDC trace context propagation
- [ ] **Distributed tracing**: Brave or OTel for HTTP, DB, messaging

### 4.6 Rust Observability Checklist

- [ ] **Tracing crate**: `tracing` with `tracing-subscriber` for structured output
- [ ] **OTel integration**: `opentelemetry` crate with exporter
- [ ] **HTTP instrumentation**: `tower-http` tracing layer on handlers
- [ ] **Health endpoints**: Probe endpoints with dependency status
- [ ] **Metric export**: `metrics` crate with Prometheus or OTLP exporter

---

## Anti-Patterns

### 5.1 Observability-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Missing EF Core/Npgsql instrumentation | high | No DB instrumentation | DB spans invisible, can't ID slow queries | Register AddNpgsqlInstrumentation() |
| 2 | Alert queries using wrong metric names | high | Grafana alert references wrong name | Alerts never fire or fire on wrong data | Verify metric names match exported |
| 3 | Readiness probe only checks probe_success | high | /ready OK even when dependencies Degraded | Traffic routed to unhealthy instances | Aggregate all health checks, fail on degraded |
| 4 | Wolverine span helpers defined but never wired | medium | Span methods exist but not in middleware | Message handlers run without traces | Wire span helpers into pipeline |
| 5 | Serilog without trace enrichment | medium | Logs lack TraceId/SpanId | Cannot correlate logs with traces | Add Enrich.WithTraceId()/WithSpanId() |
| 6 | Health checks without alert rules | medium | Registered checks but no alerting | Failures unnoticed until user reports | Create alert rules for each critical check |
| 7 | OTel sampling too aggressive | medium | <1% TraceIdRatioBased in dev | Most traces dropped | Parent-based; 100% dev, lower in prod |
| 8 | Missing AsNoTracking span attribution | low | Read queries not attributed to operation | Cannot distinguish read vs write perf | Add span attributes for query type |
| 9 | Structured logging with string interpolation | low | `logger.LogInformation($"User {userId}")` | Structured logging lost, can't filter | Use `logger.LogInformation("User {UserId}", userId)` |
| 10 | Log level misconfiguration | medium | Info logs filtered out in prod | Important data lost | Review overrides, ensure critical at Info min |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets in observability config | critical | DB creds, API keys in OTel exporter config | Exposed in telemetry infra | Use env vars or secret manager |
| 2 | Health check returning 200 for all states | high | /health always 200 regardless of status | Load balancer routes to broken instances | Map: Healthy→200, Degraded→503, Unhealthy→503 |
| 3 | Missing context propagation headers | high | Outbound HTTP missing W3C trace headers | Distributed trace broken | Ensure traceparent/tracestate injected |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: OTel Instrumentation Setup — Missing DB Spans

```csharp
// BAD: No database instrumentation — DB spans invisible
builder.Services.AddOpenTelemetry()
    .WithTracing(tracing => tracing
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddOtlpExporter(opts => opts.Endpoint = new Uri(otelEndpoint)));

// GOOD: All instrumentation sources registered
builder.Services.AddOpenTelemetry()
    .WithTracing(tracing => tracing
        .AddAspNetCoreInstrumentation()
        .AddHttpClientInstrumentation()
        .AddNpgsqlInstrumentation()
        .AddSource("Wolverine")
        .AddOtlpExporter(opts => opts.Endpoint = new Uri(otelEndpoint)));
```

#### Example 2: Health Check Readiness Probe — Masking Degraded

```csharp
// BAD: Readiness only checks probe_success, ignores degraded
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready"),
    ResponseWriter = async (context, report) =>
    {
        context.Response.StatusCode = 200;
        await context.Response.WriteAsync("OK");
    }
});

// GOOD: Readiness fails on degraded dependencies
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
# BAD: Logs without trace context — cannot correlate
import logging
logger = logging.getLogger(__name__)
logger.info("Processing order %s for customer %s", order_id, customer_id)

# GOOD: Structured logging with trace context injection
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
# BAD: Alert references wrong metric name — will never fire
alert: HighErrorRate
expr: rate(http_requests_errors_total[5m]) / rate(http_requests_total[5m]) > 0.05

# GOOD: Alert references exact exported metric name
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
  "description": "Observability gap or misconfiguration found.",
  "recommendation": "Specific fix with config/code example.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "observability|security|performance|maintainability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based where finding starts.
- **endLine**: Optional. End of multi-line issue.
- **description**: Factual, specific. Include misconfigured pattern. No hedging.
- **recommendation**: Actionable. Include config/code snippet.
- **technology**: Primary technology. `CrossLanguage` for multi-file.
- **category**: Most specific.

### 7.3 Summary Report

Emit summary after all findings:

```json
{
  "summary": {
    "squad": "audit-observability",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "OTel", "Grafana"],
    "totalFindings": 12,
    "bySeverity": { "critical": 0, "high": 3, "medium": 7, "low": 2 },
    "byCategory": { "observability": 10, "performance": 1, "security": 1 },
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
| **critical** | Complete observability blind spot in production | No tracing, no health checks on critical deps |
| **high** | Significant gap in trace/log/alert coverage | Missing DB instrumentation, wrong alert names, readiness masking degraded |
| **medium** | Degraded observability, missing correlation | Missing Serilog enrichment, unwired middleware spans, aggressive sampling |
| **low** | Best practice deviation, minor improvement | Missing span attributes, string interpolation in structured logs |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Affects production incident response | +1 level | All |
| Health check for critical dep (DB, secrets) | +1 level | All |
| Alerts will never fire on critical condition | +1 level | All |
| Affects multiple services in distributed system | +1 level | Microservices |
| Auth/authorization code | +1 level | All |
| Data encryption/key management | +1 level | All |
| Affects multiple tenants/users | +1 level | Multi-tenant |
| Public-facing endpoint | +1 level | Web apps |
| Financial/billing data | +1 level | Financial |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Non-production/test code only | -1 level (min low) | All |
| Existing mitigating control documented | -1 level (min low) | All |
| Deprecated code scheduled for removal | -1 level (min low) | All |
| Documentation/comments | -1 level (min low) | All |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Missing `AddNpgsqlInstrumentation` → high
- `EnableSensitiveDataLogging` in prod OTel → high (PII in traces)
- Wolverine `AddSource` missing → medium

#### Python
- Missing OTel auto-instrumentation → medium
- `print()` in prod code paths → low
- Missing trace context in logs → medium

#### JavaScript/TypeScript
- Missing `@opentelemetry/instrumentation-http` → medium
- Unhandled promise rejection without span → high
- Missing health endpoint dep check → medium

#### Go
- Missing context propagation in HTTP handlers → high
- Unclosed span in defer → low
- Missing `slog` JSON handler → medium

#### Java
- Missing Micrometer Prometheus export → medium
- Broken SLF4J MDC propagation → medium

#### Rust
- Missing `tracing` span in error paths → medium
- `println!` in production → low

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
audit-consolidated ◄────────────────┘
audit-consistency-coherence
```

### 9.2 Upstream Dependencies

- **audit-security**: Which endpoints/services need tracing coverage.
- **audit-architecture**: Service boundaries needing trace context propagation.
- **audit-cicd**: If observability tools deployed in pipeline.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all findings.
- **audit-consistency-coherence**: Cross-domain alignment validation.

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-observability | audit-fault-tolerance | Observability: telemetry instrumentation; Fault-tolerance: retry/circuit-breaker |
| audit-observability | audit-cicd | Observability: runtime telemetry; CI/CD: pipeline config |
| audit-observability | audit-privacy-data | Observability: telemetry completeness; Privacy: PII in logs/traces |

### 9.5 Recommended Execution Order

For full audit:
1. **audit-dependencies** — Third-party risk
2. **audit-security** — Vuln scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation (this squad)
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. Domain-specific squads
12. **audit-consolidated** — Aggregate all
13. **audit-consistency-coherence** — Cross-domain check

### 9.6 Output Integration

When squads run sequentially:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip already reported.
3. Cross-reference: escalate confirmed by multiple.
4. Output in standardized Section 7 format.