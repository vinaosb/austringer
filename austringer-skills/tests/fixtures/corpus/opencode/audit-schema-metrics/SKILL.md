---
name: audit-schema-metrics
description: >-
  Schema and metrics validation auditing: JSON schema compliance, metric registry
  integrity, cross-reference validation, placeholder resolution, configuration
  schema compliance, and alert metric name consistency.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Schema & Metrics Validation Audit

## USE FOR

- JSON schema compliance — metrics data, config files conform to expected schemas
- Metric registry integrity — all metric IDs registered, no orphan references, no duplicate IDs
- Cross-reference validation — placeholders in templates/docs resolved by metric registry
- Placeholder resolution — every placeholder has registry entry with non-null value
- Data type consistency — numeric fields are numbers, percentage fields 0-100, currency formatted
- Completeness check — required metric categories present (financial, operational, market, technical)
- Formula validation — derived metrics have correct formula references, base metrics exist
- Configuration schema compliance — YAML/JSON configs match expected structure
- Alert metric name consistency — metric names in alerts match exported names from code
- Enum/validation constraint completeness — all enumerated values covered in code

### DO NOT USE FOR

- Financial model accuracy (use `audit-financial-model`)
- Business logic validation (use `audit-financial-model`)
- Code quality assessment (use `audit-code-quality`)
- Security vulnerabilities (use `audit-security`)
- Observability instrumentation (use `audit-observability`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repo names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (JS superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` with `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` with `Microsoft.EntityFrameworkCore` | EF Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` with `actix` | Actix Web |
| Gin | `go.mod` with `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` with `xunit`/`nunit`/`MSTest` | .NET test frameworks |
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
| Terraform | `*.tf` | IaC |
| DB migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` in deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. Scan root dir for all file patterns above.
2. Identify primary language by file count.
3. Identify frameworks by config analysis.
4. Identify test framework by test dir/config presence.
5. Identify CI/CD by workflow file presence.
6. Identify infrastructure by Docker/ORM/observability configs.
7. Scan for metric/registry JSON files — `metrics.json`, `metric_registry.json`, `*_metrics.json`
8. Scan for alert config files — Grafana/Prometheus alert rules.
9. Select matching checklist from Section 4 based on detected tech.
10. If multiple languages (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 Cross-Language Schema & Metrics Checklist

- [ ] **JSON schema validation**: Verify metrics data conforms to declared JSON schema
- [ ] **Metric registry integrity**: All metric IDs unique, no duplicates, no orphans
- [ ] **Cross-reference validation**: Every placeholder/token in content has matching registry entry
- [ ] **Placeholder resolution**: Every registry entry has non-null value (no empty strings, no null)
- [ ] **Data type consistency**: Numeric fields are numbers, percentages 0-100, currencies formatted
- [ ] **Completeness check**: Required categories present (financial, operational, market, technical)
- [ ] **Formula validation**: Derived metrics reference only existing base metrics, valid syntax
- [ ] **Config schema compliance**: YAML/JSON configs match expected structure and types
- [ ] **Alert metric name consistency**: Names in alerts match exported names from code
- [ ] **Enum completeness**: All enumerated values referenced in code have corresponding definitions

### 4.2 .NET/C# Specific Checklist

- [ ] **OTel metric naming**: Code metric names match Grafana/Prometheus alert names
- [ ] **Health check alignment**: Health check result metrics have corresponding alert rules
- [ ] **EF model schema alignment**: Entity configs match DB migration schema
- [ ] **Config binding**: `IOptions<T>` bound config matches appsettings.json structure
- [ ] **Enum value coverage**: All enum values in code have corresponding DB/column definitions

### 4.3 Python Specific Checklist

- [ ] **Pydantic model schema**: Pydantic models match expected API input/output schemas
- [ ] **DB column types**: SQLAlchemy/Alembic column types match Pydantic model types
- [ ] **Metric export naming**: Exported Prometheus/OTLP metric names match alert queries

### 4.4 JavaScript/TypeScript Specific Checklist

- [ ] **TypeScript interface alignment**: Interfaces match API response schemas
- [ ] **JSON schema validation**: Zod/Joi schemas match actual data structures
- [ ] **Env var schema**: All required env vars have corresponding type definitions

### 4.5 Go Specific Checklist

- [ ] **Struct tag alignment**: JSON/DB struct tags match expected schema
- [ ] **Config validation**: Viper/envconfig structs match actual config files
- [ ] **Metric naming convention**: Prometheus metrics follow naming convention

### 4.6 Java Specific Checklist

- [ ] **JPA entity schema**: Entity annotations match DB schema
- [ ] **DTO schema alignment**: DTOs match API contract schemas
- [ ] **Config properties**: `@ConfigurationProperties` match application.yml structure

### 4.7 Rust Specific Checklist

- [ ] **Serde schema**: Serialize/deserialize derive macros match expected data formats
- [ ] **Config struct alignment**: Config structs match TOML/YAML config files
- [ ] **Enum completeness**: All enum variants handled in match expressions

---

## Anti-Patterns

### 5.1 Schema & Metrics-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Broken cross-reference | critical | Placeholder in content has no registry entry | Content renders raw placeholder | Add missing entry to registry |
| 2 | Alert names don't match code | high | Alert refs `http_requests_total` but code exports `http_server_request_duration_seconds_count` | Alerts never fire or fire on wrong data | Align alert names with exact exported names |
| 3 | Duplicate metric IDs | high | Two registry entries share same ID | Ambiguous refs; last-write-wins | Deduplicate IDs, merge or create distinct |
| 4 | Orphan metric refs | medium | Registry entry refs base metric that doesn't exist | Derived calculations fail | Add missing base metric or fix broken reference |
| 5 | Null/empty registry values | medium | Entry exists but null/empty value | Content renders blank | Provide valid non-null values |
| 6 | Data type mismatch | medium | Numeric stored as string | Calculations fail, chart rendering issues | Store correct JSON types |
| 7 | Missing required metric category | high | Registry missing whole category | Incomplete BI, reporting gaps | Add metrics for all required categories |
| 8 | Schema file out of date | medium | Schema doesn't match actual data | Validation passes but data semantically wrong | Update schema or data to match |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded config values | high | Config hardcoded in code instead of external files | Can't update without deploy; schema drift | Externalize all config to YAML/JSON/env |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Alert Metric Name Mismatch

```csharp
// ❌ BAD: Code exports metric with different name than alert references
// In code:
builder.Services.AddOpenTelemetry()
    .WithMetrics(metrics => metrics
        .AddMeter("FamilyOffice.Web")
        .AddOtlpExporter());

// Meter code exports: familyoffice_http_requests_count
// Alert references: familyoffice_http_requests_total
// Result: Alert never fires

// ✅ GOOD: Consistent metric naming across code and alerts
// In code — use naming convention matching Prometheus/OTel standards:
_counter.Add(1, new("http_route", route), new("http_status_code", statusCode.ToString()));
// Exports as: familyoffice_http_requests_total (OTel convention)
// Alert references exact same name:
// expr: rate(familyoffice_http_requests_total[5m])
```

#### Example 2: Configuration Schema Binding

```csharp
// ❌ BAD: appsettings.json uses snake_case, C# class uses PascalCase — binding fails silently
// appsettings.json:
// { "database_host": "localhost", "database_port": 5432 }
// C# class:
// public class DbConfig { public string DatabaseHost { get; set; } }

// ✅ GOOD: Consistent naming with explicit mapping or JsonPropertyName
public class DbConfig
{
    [JsonPropertyName("database_host")]
    public string DatabaseHost { get; set; } = null!;
    [JsonPropertyName("database_port")]
    public int DatabasePort { get; set; }
}
```

### 6.2 Python Examples

#### Example 1: Metric Registry Cross-Reference Validation

```python
# ❌ BAD: Content references metric not in registry
metrics = {
    "avn_revenue_growth": {"value": 12.5, "unit": "percent"},
    "avn_customer_count": {"value": 1500, "unit": "count"},
}
content = "Our revenue grew by {{avn_revenue_growth}}% with {{avn_churn_rate}} churn"
# avn_churn_rate is NOT in the registry → broken placeholder

# ✅ GOOD: All placeholders validated against registry
def validate_placeholders(content: str, registry: dict) -> list[str]:
    placeholders = re.findall(r"\{\{(\w+)\}\}", content)
    missing = [p for p in set(placeholders) if p not in registry]
    if missing:
        raise SchemaValidationError(f"Missing registry entries: {missing}")
    return placeholders
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: Zod Schema Alignment

```typescript
// ❌ BAD: Zod schema doesn't match actual API response
const UserSchema = z.object({
  name: z.string(),
  email: z.string().email(),
});
// API returns: { firstName: string, email: string }
// firstName → name mismatch causes validation to fail

// ✅ GOOD: Schema aligned with actual data contract
const UserSchema = z.object({
  firstName: z.string(),
  email: z.string().email(),
});
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
  "description": "What was found and why it matters. Include schema/registry issue.",
  "recommendation": "Specific fix to ensure data integrity. Include corrected value if applicable.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "architecture|maintainability|compliance|observability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative path from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Line where finding ends (multi-line issues).
- **description**: Factual, specific. Include schema violation or registry issue. No hedging.
- **recommendation**: Actionable. Include corrected value or schema fix.
- **technology**: Primary tech. Use `CrossLanguage` for multi-file patterns.
- **category**: Most specific category.

### 7.3 Summary Report

```json
{
  "summary": {
    "squad": "audit-schema-metrics",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": ["JSON", "YAML", ".NET/C#"],
    "totalFindings": 14,
    "bySeverity": { "critical": 1, "high": 3, "medium": 7, "low": 3 },
    "byCategory": { "architecture": 6, "maintainability": 4, "compliance": 2, "observability": 2 },
    "filesScanned": 30,
    "filesWithFindings": 10
  }
}
```

---

## Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Broken cross-reference (placeholder with no registry entry), missing required category | `{{METRIC_ID}}` renders raw; entire category absent |
| **high** | Schema violation in core metrics, duplicate IDs, alert names don't match code | Two entries same ID; alert refs non-existent metric name |
| **medium** | Missing optional fields, inconsistent types, orphan refs, schema out of date | String for numeric; base metric missing for derived |
| **low** | Documentation gaps in metric descriptions, minor schema improvements |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding in auth/authorization code | +1 level | All tech |
| Finding in encryption/key management | +1 level | All tech |
| Finding affects multiple tenants/users | +1 level | Multi-tenant systems |
| Finding in public-facing endpoint | +1 level | Web apps |
| Finding involves financial/billing data | +1 level | Financial systems |
| Finding has known CVE | +1 level | Dep vulns |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in non-production/test code only | -1 level (min low) | All tech |
| Finding has existing mitigating control | -1 level (min low) | All tech |
| Finding in deprecated code | -1 level (min low) | All tech |
| Finding in docs/comments | -1 level (min low) | All tech |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Alert metric name mismatch → high (monitoring blind spot)
- Config binding type mismatch → medium (runtime failure)
- Missing health check metric → medium (monitoring gap)

#### Python
- Pydantic schema mismatch with API → high (validation failure)
- Missing Zod/Joi schema for input → medium (no validation)
- JSON schema file out of date → medium (false validation)

#### JavaScript/TypeScript
- TS interface doesn't match API → medium (type mismatch)
- Missing z.infer<> type alignment → low (type drift)
- Env var without type definition → medium (runtime error)

#### Go
- Struct tag mismatch with DB schema → high (data corruption)
- Missing config validation → medium (silent misconfiguration)

#### Java
- JPA entity column type mismatch → high (schema migration failure)
- Missing @ConfigurationProperties binding → medium (config not loaded)

#### Rust
- Serde derive mismatch → medium (deserialization failure)
- Missing enum variant handling → medium (panic on unexpected value)

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

- **audit-observability**: Provides exported metric names to cross-reference against alert configs
- **audit-architecture**: Provides schema/structure analysis to validate against data definitions
- **audit-financial-model**: Provides financial metric definitions for registry validation

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Cross-domain alignment validation
- **audit-observability**: Uses schema findings to verify alert metric name correctness

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-schema-metrics | audit-observability | Schema: data/registry integrity; observability: runtime metric export |
| audit-schema-metrics | audit-financial-model | Schema: registry completeness; financial: formula correctness |
| audit-schema-metrics | audit-architecture | Schema: data schema compliance; architecture: code structure |

### 9.5 Recommended Execution Order

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Code-level vuln scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Impl quality
5. **audit-test-coverage** — Test assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-schema-metrics** — Schema & metrics validation
13. **audit-consolidated** — Aggregate all findings
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip already reported findings.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in standardized Section 7 format for aggregation.