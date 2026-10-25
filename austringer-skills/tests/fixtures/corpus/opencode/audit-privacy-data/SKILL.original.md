---
name: audit-privacy-data
description: >-
  Privacy and data protection auditing: PII scanning in code and content, data flow
  audit across module boundaries, anonymization verification, consent mechanism review,
  field-level encryption validation, and retention policy enforcement.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Privacy & Data Protection Audit

## USE FOR

- PII scanning in code and content — detect personal identifiers in source code, configs, and documentation
- Data flow audit — trace how personal data flows through the system, identify storage and transit locations
- Anonymization verification — verify data anonymization/pseudonymization is correctly implemented
- Consent mechanism review — check consent collection, storage, withdrawal implementation
- Field-level encryption audit — verify sensitive fields are encrypted at rest, key rotation exists
- Data retention policy enforcement — verify retention policies are implemented with automated deletion
- EF Core query logging for PII exposure — verify query logs don't leak sensitive parameter values
- Search column encryption — verify encrypted fields used in search don't leak partial PII
- Data classification review — verify sensitive data is classified and handled appropriately
- Cookie/tracking compliance — verify cookie consent, tracking transparency
- Data subject rights implementation — verify export (portability), deletion (erasure), and access request handling
- Cross-module data flow tracing — verify PII doesn't leak across module boundaries without consent

### DO NOT USE FOR

- Security vulnerabilities (use `audit-security`)
- License compliance (use `audit-license`)
- LGPD/GDPR legal compliance assessment (use `audit-regulatory-dataprotection`)
- Container or infrastructure security (use `audit-security`)
- Application code quality (use `audit-code-quality`)
- Observability telemetry configuration (use `audit-observability`)

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

### 4.1 .NET/C# Privacy Checklist

- [ ] **PII field-level encryption**: Verify NationalId, SSN, tax IDs are encrypted before storage using pgcrypto or Transit encryption
- [ ] **Data flow tracing across module boundaries**: PII doesn't cross module boundaries without proper consent/context
- [ ] **EF Core query logging**: Sensitive parameter values are not logged in plain text (disable `EnableSensitiveDataLogging` in production)
- [ ] **Consent mechanism completeness**: Consent collected before PII processing, stored with timestamp and scope
- [ ] **Retention policy enforcement**: Automated deletion/anonymization jobs exist and are scheduled
- [ ] **DSR export security**: Data subject request exports are encrypted during transit and at rest
- [ ] **Search column encryption**: Encrypted fields used in search queries don't leak partial plaintext via deterministic encryption
- [ ] **Chat/message encryption**: No static keys from config — use per-tenant or per-session key derivation
- [ ] **Anonymization verification**: Anonymization functions truly remove identifying information, not just mask
- [ ] **PII in exception messages**: Exceptions don't include PII in messages that get logged
- [ ] **Configuration secrets**: No PII in `appsettings.json`, connection strings, or environment variable values
- [ ] **Multi-tenant data isolation**: Tenant data is properly isolated — no cross-tenant PII leakage
- [ ] **Audit trail PII**: Audit logs containing PII have appropriate access controls and retention

### 4.2 Python Privacy Checklist

- [ ] **PII in log output**: No `print()` or `logging` statements that include personal identifiers
- [ ] **Database field encryption**: Sensitive fields encrypted via `cryptography` or `pgcrypto`
- [ ] **Consent API endpoints**: Consent endpoints properly validate scope and store decisions
- [ ] **Data export encryption**: Exports encrypted with AES-256 or equivalent before download
- [ ] **Retention cron jobs**: Periodic cleanup tasks scheduled for data expiration
- [ ] **Input sanitization**: User inputs sanitized before storage, no raw PII in error responses
- [ ] **ORM query logging**: Django/SQLAlchemy query logging disabled for sensitive models in production

### 4.3 JavaScript/TypeScript Privacy Checklist

- [ ] **Client-side PII exposure**: No PII in URL parameters, localStorage, or client-side cookies
- [ ] **API response filtering**: API responses don't include more PII than the consumer needs (field filtering)
- [ ] **Consent cookie management**: Cookie consent properly implemented with opt-in before tracking
- [ ] **Log redaction**: Client-side logging (Sentry, etc.) configured to redact PII
- [ ] **Data export download**: Exports served over HTTPS with appropriate headers (no-cache, no-store)
- [ ] **Browser storage**: Session data cleared on logout, no persistent PII in IndexedDB

### 4.4 Go Privacy Checklist

- [ ] **PII in error returns**: Error messages don't include personal identifiers
- [ ] **Database encryption**: Sensitive fields encrypted at application or database level
- [ ] **Log sanitization**: Structured logging excludes PII fields from output
- [ ] **Consent middleware**: Request-scoped consent verification before PII processing

### 4.5 Java Privacy Checklist

- [ ] **Entity field encryption**: JPA entities encrypt sensitive fields via `@Convert` or database functions
- [ ] **PII in toString/hashCode**: Sensitive fields excluded from `toString()` and `hashCode()`
- [ ] **Logback pattern**: No PII fields in log patterns
- [ ] **Spring Security**: PII endpoints require appropriate authentication and authorization

### 4.6 Rust Privacy Checklist

- [ ] **PII in Debug/Display**: Sensitive structs implement custom `Debug` that redacts PII
- [ ] **Error types**: Domain error types don't embed PII in error messages
- [ ] **Database encryption**: Sensitive fields encrypted before serialization

---

## Anti-Patterns

### 5.1 Privacy-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | NationalId stored in plaintext | critical | Government IDs stored without encryption in database | Data breach exposes highly sensitive PII; legal liability | Encrypt at rest using pgcrypto or Transit encryption with per-tenant keys |
| 2 | DSR export data unencrypted | critical | Data subject request exports served as unencrypted files | PII exposed during download, violates GDPR/LGPD | Encrypt export file with AES-256, deliver via secure channel |
| 3 | Search columns leaking partial PII | high | Deterministic encryption on search columns reveals patterns | Partial PII recovery via frequency analysis | Use blinded tokens or dedicated search index with access controls |
| 4 | Chat encryption using static config key | critical | Encryption key for chat messages loaded from config file | Key compromise exposes all historical messages | Derive keys per-tenant or per-session using KMS/Vault |
| 5 | `EnableSensitiveDataLogging` in production | high | EF Core configured to log parameter values | Query logs contain PII (emails, IDs, names) | Disable in production: `options.EnableSensitiveDataLogging(false)` |
| 6 | PII in exception messages | high | Exception messages include user data: `throw new Exception($"User {email} not found")` | Exceptions logged with PII, visible in log aggregation | Use structured errors with IDs, not PII: `throw new UserNotFoundException(userId)` |
| 7 | Missing consent before data processing | high | PII processed before consent is verified | Violates GDPR/LGPD consent requirements | Verify consent scope before any PII processing operation |
| 8 | Cross-tenant PII leakage | critical | Shared database with improper tenant isolation | One tenant's PII visible to another | Enforce tenant filtering at query level; test cross-tenant access |
| 9 | PII in URL parameters | medium | `/api/users?email=john@example.com` | PII in server logs, browser history, referrer headers | Use opaque identifiers: `/api/users/{id}` |
| 10 | No retention policy enforcement | medium | Data retained indefinitely with no automated cleanup | Legal liability, increased breach impact | Implement scheduled retention jobs with configurable periods |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded PII in source code | critical | Email addresses, phone numbers, names in code/config | PII committed to version control, accessible to all | Use test fixtures with synthetic data, PII only in secure storage |
| 2 | Secrets and PII mixed | critical | Encryption keys stored alongside personal data | Single breach exposes both keys and data | Separate key management from data storage; use dedicated key service |
| 3 | Unencrypted PII in transit | high | API responses containing PII over HTTP | Man-in-the-middle can intercept personal data | Enforce HTTPS, use TLS for all PII-bearing endpoints |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: NationalId Encryption at Rest

```csharp
// ❌ BAD: NationalId stored in plaintext — data breach exposes sensitive PII
public class Person
{
    public string NationalId { get; set; }  // Stored as plain text
}

// ✅ GOOD: Encrypted via pgcrypto with per-tenant key
public class Person
{
    public string NationalIdEncrypted { get; set; }  // pgcrypto encrypt()
}

// In EF configuration:
// modelBuilder.Entity<Person>()
//     .Property(p => p.NationalIdEncrypted)
//     .HasComputedColumnSql(
//         "encrypt(NationalId::bytea, current_setting('app.tenant_key')::bytea, 'aes-cbc')");
```

#### Example 2: EF Core Sensitive Data Logging

```csharp
// ❌ BAD: Sensitive data logging enabled in production — PII in query logs
builder.Services.AddDbContext<AppDbContext>(options =>
    options.UseNpgsql(connectionString)
           .EnableSensitiveDataLogging()  // Logs parameter values
           .EnableDetailedErrors());

// ✅ GOOD: Conditional — only in development
builder.Services.AddDbContext<AppDbContext>(options =>
{
    options.UseNpgsql(connectionString);
    if (builder.Environment.IsDevelopment())
    {
        options.EnableSensitiveDataLogging()
               .EnableDetailedErrors();
    }
});
```

### 6.2 Python Examples

#### Example 1: PII in Exception Messages

```python
# ❌ BAD: Exception message contains PII — logged and visible to operators
def get_customer(email: str):
    customer = db.query(Customer).filter_by(email=email).first()
    if not customer:
        raise ValueError(f"Customer not found: {email}")

# ✅ GOOD: Use opaque ID — PII never reaches logs
def get_customer(email: str):
    customer = db.query(Customer).filter_by(email=email).first()
    if not customer:
        raise CustomerNotFoundError()  # No PII in message
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: PII in URL Parameters

```typescript
// ❌ BAD: PII exposed in URL — visible in logs, history, referrer headers
async function lookupUser(email: string) {
    const response = await fetch(`/api/users?email=${encodeURIComponent(email)}`);
    return response.json();
}

// ✅ GOOD: Use opaque identifier — PII stays out of URLs
async function lookupUser(userId: string) {
    const response = await fetch(`/api/users/${encodeURIComponent(userId)}`);
    return response.json();
}
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
  "description": "What was found and why it matters. Include the PII handling pattern or data flow issue.",
  "recommendation": "Specific fix with code example. Reference privacy best practice.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|privacy|architecture"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the PII type and exposure path. No hedging ("might be a problem").
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns that span multiple files in different languages.
- **category**: Primary category. Use the most specific category.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-privacy-data",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "PostgreSQL"],
    "totalFindings": 18,
    "bySeverity": {
      "critical": 2,
      "high": 5,
      "medium": 8,
      "low": 3
    },
    "byCategory": {
      "privacy": 10,
      "security": 5,
      "compliance": 3
    },
    "filesScanned": 55,
    "filesWithFindings": 15
  }
}
```

---

## Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | PII exposed in plaintext, no encryption at rest, or cross-tenant leakage | NationalId in plaintext, static encryption key, DSR export unencrypted |
| **high** | PII in logs, missing consent, no retention policy, search column leakage | Sensitive data logging enabled, PII in exception messages, missing consent |
| **medium** | Incomplete anonymization, unclear data classification, PII in URLs | Partial anonymization, missing field classification, query parameters with PII |
| **low** | Best practice improvements, documentation gaps | Missing privacy docs, minor consent UI issues |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding involves government ID (SSN, NationalId, CPF) | +1 level | All technologies |
| Finding affects multiple tenants | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint or export | +1 level | Web applications |
| Finding involves health or financial data | +1 level | Domain-specific |
| Finding has regulatory requirement (GDPR/LGPD) | +1 level | All technologies |
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test/seed data only | -1 level (min low) | All technologies |
| Finding has existing mitigating control | -1 level (min low) | All technologies |
| Finding is in deprecated module | -1 level (min low) | All technologies |
| Finding is in documentation/comments | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- `EnableSensitiveDataLogging` in production → high (PII in query logs)
- NationalId in EF entity without encryption → critical (regulatory)
- Static encryption key in `appsettings.json` → critical (key compromise)

#### Python
- `print(email)` in production code → medium (PII in stdout)
- `pickle` serialization of PII data → high (insecure serialization)
- Missing consent check before data access → high (regulatory)

#### JavaScript/TypeScript
- PII in URL parameters → medium (exposure in logs/history)
- PII in localStorage without encryption → high (client-side exposure)
- Missing cookie consent banner → medium (regulatory)

#### Go
- PII in error return values → medium (log exposure)
- Missing middleware consent check → high (unauthorized processing)

#### Java
- PII in `toString()` → medium (log exposure)
- Missing `@PreAuthorize` on PII endpoints → high (auth bypass)

#### Rust
- PII in `Debug` implementation → low (debug output exposure)
- Missing encryption before DB write → critical (plaintext storage)

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

- **audit-security**: May inform which endpoints handle PII and need encryption review
- **audit-architecture**: May identify data flow paths across module boundaries
- **audit-observability**: May reveal if logs contain PII through telemetry configuration

### 9.3 Downstream Consumers

- **audit-regulatory-dataprotection**: Uses privacy findings to assess LGPD/GDPR compliance
- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-privacy-data | audit-security | Privacy focuses on PII handling and encryption; security focuses on vulnerabilities and access control |
| audit-privacy-data | audit-regulatory-dataprotection | Privacy focuses on code-level PII patterns; regulatory focuses on legal compliance assessment |
| audit-privacy-data | audit-observability | Privacy focuses on PII in data flows; observability focuses on PII in logs/traces |

### 9.5 Recommended Execution Order

For a full audit, run squads in this sequence:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII and data protection review (this squad)
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
