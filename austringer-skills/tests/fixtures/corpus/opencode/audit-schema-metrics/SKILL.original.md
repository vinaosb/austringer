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

- JSON schema compliance — verify metrics data, configuration files conform to expected schemas
- Metric registry integrity — all metric IDs registered, no orphan references, no duplicate IDs
- Cross-reference validation — placeholders in templates/docs resolved by metric registry entries
- Placeholder resolution verification — every placeholder in content has a corresponding registry entry with non-null value
- Data type consistency — numeric fields contain numbers, percentage fields 0-100, currency fields formatted correctly
- Completeness check — required metric categories present (financial, operational, market, technical)
- Formula validation — derived metrics have correct formula references and base metrics exist
- Configuration schema compliance — verify YAML/JSON config files match expected structure
- Alert metric name consistency — verify metric names in alerts match exported metric names from code
- Enum/validation constraint completeness — verify all enumerated values are covered in code

### DO NOT USE FOR

- Financial model accuracy (use `audit-financial-model`)
- Business logic validation (use `audit-financial-model`)
- Code quality assessment (use `audit-code-quality`)
- Security vulnerabilities (use `audit-security`)
- Observability instrumentation (use `audit-observability`)

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
7. **Scan for metric/registry JSON files** — `metrics.json`, `metric_registry.json`, `*_metrics.json`
8. **Scan for alert configuration files** — Grafana alerting rules, Prometheus alert rules
9. **Select the matching checklist** from Section 4 based on detected technologies.
10. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 Cross-Language Schema & Metrics Checklist

- [ ] **JSON schema validation**: Verify metrics data files conform to declared JSON schema (if schema exists)
- [ ] **Metric registry integrity**: All metric IDs are unique, no duplicates, no orphans
- [ ] **Cross-reference validation**: Every placeholder/token in content documents has a matching registry entry
- [ ] **Placeholder resolution**: Every registry entry has a non-null value (no empty strings, no null)
- [ ] **Data type consistency**: Numeric fields are numbers (not strings), percentages 0-100, currencies formatted
- [ ] **Completeness check**: Required metric categories present (financial, operational, market, technical)
- [ ] **Formula validation**: Derived metrics reference only existing base metrics, formula syntax valid
- [ ] **Configuration schema compliance**: YAML/JSON configs match expected structure and types
- [ ] **Alert metric name consistency**: Metric names in alert rules match exported names from application code
- [ ] **Enum completeness**: All enumerated values referenced in code have corresponding definitions

### 4.2 .NET/C# Specific Checklist

- [ ] **OTel metric naming**: Metric names in code match names referenced in Grafana/Prometheus alerts
- [ ] **Health check metric alignment**: Health check result metrics have corresponding alert rules
- [ ] **EF model schema alignment**: Entity configurations match database migration schema
- [ ] **Configuration binding**: `IOptions<T>` bound config matches appsettings.json structure
- [ ] **Enum value coverage**: All enum values used in code have corresponding database/column definitions

### 4.3 Python Specific Checklist

- [ ] **Pydantic model schema**: Pydantic models match expected data schemas for API input/output
- [ ] **Database column types**: SQLAlchemy/Alembic column types match Pydantic model types
- [ ] **Metric export naming**: Exported Prometheus/OTLP metric names match alert queries

### 4.4 JavaScript/TypeScript Specific Checklist

- [ ] **TypeScript interface alignment**: Interfaces match API response schemas
- [ ] **JSON schema validation**: Zod/Joi schemas match actual data structures
- [ ] **Environment variable schema**: All required env vars have corresponding type definitions

### 4.5 Go Specific Checklist

- [ ] **Struct tag alignment**: JSON/DB struct tags match expected schema
- [ ] **Configuration validation**: Viper/envconfig structs match actual configuration files
- [ ] **Metric naming convention**: Prometheus metric names follow naming convention

### 4.6 Java Specific Checklist

- [ ] **JPA entity schema**: Entity annotations match database schema
- [ ] **DTO schema alignment**: DTOs match API contract schemas
- [ ] **Configuration properties**: `@ConfigurationProperties` match application.yml structure

### 4.7 Rust Specific Checklist

- [ ] **Serde schema**: Serialize/deserialize derive macros match expected data formats
- [ ] **Configuration struct alignment**: Config structs match TOML/YAML configuration files
- [ ] **Enum completeness**: All enum variants handled in match expressions

---

## Anti-Patterns

### 5.1 Schema & Metrics-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Broken cross-reference | critical | Placeholder (e.g., `{{METRIC_ID}}`) in content has no registry entry | Content renders with raw placeholder — broken documentation/presentation | Add missing entry to metric registry with valid value |
| 2 | Metric names in alerts don't match code | high | Grafana alert references `http_requests_total` but code exports `http_server_request_duration_seconds_count` | Alerts never fire or fire on wrong data; monitoring blind spots | Align alert metric names with exact exported names from application code |
| 3 | Duplicate metric IDs in registry | high | Two registry entries share the same ID | Ambiguous references; last-write-wins may hide data | Deduplicate IDs, merge values or create distinct IDs |
| 4 | Orphan metric references | medium | Registry entry references base metric that doesn't exist | Derived metric calculations fail or produce incorrect results | Add missing base metric or fix broken reference |
| 5 | Null or empty registry values | medium | Metric registry entry exists but has `null` or empty string value | Content renders with blanks; appears incomplete | Provide valid non-null values for all registry entries |
| 6 | Data type mismatch | medium | Numeric metric stored as string (`"42"` instead of `42`) | Calculations fail or produce wrong results; chart rendering issues | Store values in correct JSON types (number, not string) |
| 7 | Missing required metric category | high | Registry missing entire category (e.g., no financial metrics) | Incomplete business intelligence; reporting gaps | Add metrics for all required categories |
| 8 | Schema file out of date | medium | JSON schema definition doesn't match actual data files | Validation passes but data is semantically incorrect | Update schema to match current data structure, or update data to match schema |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded configuration values | high | Config values hardcoded in code instead of external config files | Cannot update without code deployment; schema drift | Externalize all configuration to YAML/JSON/env files |

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
  "description": "What was found and why it matters. Include the schema/registry issue.",
  "recommendation": "Specific fix to ensure data integrity. Include corrected value if applicable.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "architecture|maintainability|compliance|observability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the schema violation or registry issue. No hedging.
- **recommendation**: Actionable. Include corrected value or schema fix.
- **technology**: Primary technology of the finding. Use `CrossLanguage` for patterns that span multiple files in different languages.
- **category**: Primary category. Use the most specific category.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-schema-metrics",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": ["JSON", "YAML", ".NET/C#"],
    "totalFindings": 14,
    "bySeverity": {
      "critical": 1,
      "high": 3,
      "medium": 7,
      "low": 3
    },
    "byCategory": {
      "architecture": 6,
      "maintainability": 4,
      "compliance": 2,
      "observability": 2
    },
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
| **critical** | Broken cross-reference (placeholder with no registry entry), missing required metric category | `{{METRIC_ID}}` renders raw in production; entire metric category absent |
| **high** | Schema violation in core metrics, duplicate metric IDs, alert names don't match code | Two entries with same ID; alert references non-existent metric name |
| **medium** | Missing optional fields, inconsistent data types, orphan references, schema out of date | String value for numeric field; base metric missing for derived metric |
| **low** | Documentation gaps in metric descriptions, minor schema improvements |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |
| Finding has known CVE associated | +1 level | Dependency vulnerabilities |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in documentation/comments | -1 level (min low) | All technologies |

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
- TypeScript interface doesn't match API → medium (type mismatch)
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

- **audit-observability**: Provides exported metric names to cross-reference against alert configurations
- **audit-architecture**: Provides schema/structure analysis to validate against data definitions
- **audit-financial-model**: Provides financial metric definitions for registry validation

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings
- **audit-observability**: Uses schema findings to verify alert metric name correctness

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-schema-metrics | audit-observability | Schema focuses on data/registry integrity; observability focuses on runtime metric export |
| audit-schema-metrics | audit-financial-model | Schema focuses on registry completeness; financial focuses on formula correctness |
| audit-schema-metrics | audit-architecture | Schema focuses on data schema compliance; architecture focuses on code structure |

### 9.5 Recommended Execution Order

For a full audit, run squads in this sequence:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-schema-metrics** — Schema & metrics validation (this squad)
13. **audit-consolidated** — Aggregate all findings
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.
