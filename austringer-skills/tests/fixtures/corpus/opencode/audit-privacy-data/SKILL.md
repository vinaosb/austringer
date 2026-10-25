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

- PII scanning — detect personal identifiers in code, configs, docs
- Data flow audit — trace PII through system, identify storage/transit
- Anonymization verification — verify anonymization/pseudonymization
- Consent mechanism review — check collection, storage, withdrawal
- Field-level encryption audit — verify sensitive fields encrypted, key rotation
- Data retention policy enforcement — verify automated deletion
- EF Core query logging for PII — verify query logs don't leak sensitive params
- Search column encryption — verify encrypted fields don't leak partial PII
- Data classification review — verify sensitive data classified/handled
- Cookie/tracking compliance — verify cookie consent, tracking transparency
- Data subject rights — verify export, deletion, access request handling
- Cross-module data flow tracing — verify PII doesn't leak across boundaries

### DO NOT USE FOR

- Security vulnerabilities (use `audit-security`)
- License compliance (use `audit-license`)
- LGPD/GDPR legal compliance (use `audit-regulatory-dataprotection`)
- Container/infrastructure security (use `audit-security`)
- Code quality (use `audit-code-quality`)
- Observability telemetry (use `audit-observability`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repo names.

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
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` or `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` containing `actix` | Actix Web |
| Gin | `go.mod` containing `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` with `xunit`/`nunit`/`MSTest` | .NET test |
| pytest | `pytest.ini`, `pyproject.toml` `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` | Go testing |
| JUnit | `*Test.java` with `@Test` | JUnit |
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
| OTel | `otel-collector-config.yml`, `opentelemetry` deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root** for file patterns above.
2. **Identify primary language** by file count.
3. **Identify frameworks** by config analysis.
4. **Identify test framework** by test dir/config.
5. **Identify CI/CD** by workflow files.
6. **Identify infrastructure** by Docker/ORM/OTel configs.
7. **Select checklist** from Section 4 based on detected tech.
8. **Multiple languages** (monorepo/polyglot) → run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 .NET/C# Privacy Checklist

- [ ] **PII field-level encryption**: NationalId, SSN, tax IDs encrypted via pgcrypto/Transit
- [ ] **Data flow tracing**: PII doesn't cross module boundaries without consent
- [ ] **EF Core query logging**: Sensitive params not logged plaintext (disable `EnableSensitiveDataLogging` in prod)
- [ ] **Consent mechanism**: Consent collected before PII processing, stored with timestamp + scope
- [ ] **Retention policy**: Automated deletion/anonymization jobs scheduled
- [ ] **DSR export security**: Encrypted during transit and at rest
- [ ] **Search column encryption**: Encrypted search fields don't leak via deterministic encryption
- [ ] **Chat/message encryption**: No static config keys — per-tenant/session key derivation
- [ ] **Anonymization verification**: Anonymization truly removes identifiers, not just mask
- [ ] **PII in exceptions**: Exceptions don't include PII in logged messages
- [ ] **Config secrets**: No PII in `appsettings.json`, connection strings, env vars
- [ ] **Multi-tenant isolation**: No cross-tenant PII leakage
- [ ] **Audit trail PII**: PII-bearing audit logs have access controls + retention

### 4.2 Python Privacy Checklist

- [ ] **PII in log output**: No `print()`/`logging` with personal identifiers
- [ ] **DB field encryption**: Sensitive fields encrypted via `cryptography`/`pgcrypto`
- [ ] **Consent API endpoints**: Validate scope, store decisions
- [ ] **Data export encryption**: AES-256 before download
- [ ] **Retention cron jobs**: Periodic cleanup for data expiration
- [ ] **Input sanitization**: No raw PII in error responses
- [ ] **ORM query logging**: Django/SQLAlchemy logging disabled for sensitive models in prod

### 4.3 JavaScript/TypeScript Privacy Checklist

- [ ] **Client-side PII exposure**: No PII in URL params, localStorage, cookies
- [ ] **API response filtering**: Return only needed PII (field filtering)
- [ ] **Consent cookie mgmt**: Opt-in before tracking
- [ ] **Log redaction**: Sentry/etc. configured to redact PII
- [ ] **Data export download**: HTTPS, no-cache/no-store headers
- [ ] **Browser storage**: Session data cleared on logout, no persistent PII in IndexedDB

### 4.4 Go Privacy Checklist

- [ ] **PII in error returns**: Error messages don't include personal identifiers
- [ ] **DB encryption**: Sensitive fields encrypted at app/DB level
- [ ] **Log sanitization**: Structured logging excludes PII
- [ ] **Consent middleware**: Request-scoped consent before PII processing

### 4.5 Java Privacy Checklist

- [ ] **Entity field encryption**: JPA `@Convert` or DB functions for sensitive fields
- [ ] **PII in toString/hashCode**: Sensitive fields excluded
- [ ] **Logback pattern**: No PII fields in log patterns
- [ ] **Spring Security**: PII endpoints require auth

### 4.6 Rust Privacy Checklist

- [ ] **PII in Debug/Display**: Custom `Debug` redacts PII
- [ ] **Error types**: Domain errors don't embed PII
- [ ] **DB encryption**: Sensitive fields encrypted before serialization

---

## Anti-Patterns

### 5.1 Privacy-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | NationalId plaintext | critical | Gov IDs stored unencrypted | Breach exposes highly sensitive PII; legal liability | Encrypt via pgcrypto/Transit with per-tenant keys |
| 2 | DSR export unencrypted | critical | Exports served as unencrypted files | PII exposed during download, GDPR/LGPD violation | AES-256 encrypt, deliver via secure channel |
| 3 | Search column PII leak | high | Deterministic encryption reveals patterns | Partial PII recovery via frequency analysis | Blinded tokens or dedicated search index |
| 4 | Chat static config key | critical | Encryption key from config file | Key compromise exposes all messages | Per-tenant/session keys via KMS/Vault |
| 5 | SensitiveDataLogging in prod | high | EF Core logs param values | Query logs contain PII (emails, IDs) | Disable in prod: `EnableSensitiveDataLogging(false)` |
| 6 | PII in exceptions | high | `throw new Exception($"User {email} not found")` | Exceptions logged with PII | `throw new UserNotFoundException(userId)` |
| 7 | Missing consent before processing | high | PII processed before consent verified | GDPR/LGPD violation | Verify consent scope before PII ops |
| 8 | Cross-tenant PII leak | critical | Shared DB without tenant isolation | One tenant sees another's PII | Enforce tenant filter at query level |
| 9 | PII in URL params | medium | `/api/users?email=john@example.com` | PII in server logs, browser history, referrer | Use opaque IDs: `/api/users/{id}` |
| 10 | No retention policy | medium | Data retained indefinitely | Legal liability, increased breach impact | Scheduled retention jobs with configurable periods |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded PII in source | critical | Emails, phones, names in code/config | PII committed to VCS, accessible to all | Test fixtures with synthetic data |
| 2 | Secrets + PII mixed | critical | Encryption keys stored alongside PII | Single breach exposes keys + data | Separate key mgmt from data storage |
| 3 | Unencrypted PII in transit | high | PII over HTTP | MITM intercepts personal data | Enforce HTTPS, TLS for all PII endpoints |

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

- **severity**: One of `critical`, `high`, `medium`, `low`. See Section 8 for escalation.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Line where finding ends (multi-line).
- **description**: Factual, specific. Include PII type + exposure path. No hedging.
- **recommendation**: Actionable. Include code snippet when possible.
- **technology**: Primary tech. Use `CrossLanguage` for multi-language patterns.
- **category**: Most specific category.

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
| **critical** | PII plaintext, no encryption at rest, cross-tenant leakage | NationalId plaintext, static encryption key, DSR export unencrypted |
| **high** | PII in logs, missing consent, no retention, search column leakage | SensitiveDataLogging, PII in exceptions, missing consent |
| **medium** | Incomplete anonymization, unclear classification, PII in URLs | Partial anonymization, missing field classification, query params with PII |
| **low** | Best practice improvements, doc gaps | Missing privacy docs, minor consent UI |

### 8.2 Severity Escalation Triggers

Apply modifiers to base severity. Multiple stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Gov ID (SSN, NationalId, CPF) | +1 level | All |
| Affects multiple tenants | +1 level | Multi-tenant |
| Public endpoint or export | +1 level | Web apps |
| Health or financial data | +1 level | Domain-specific |
| Regulatory requirement (GDPR/LGPD) | +1 level | All |
| Auth/authorization code | +1 level | All |
| Data encryption/key mgmt | +1 level | All |
| Public-facing endpoint | +1 level | Web apps |
| Financial/billing data | +1 level | Financial systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Test/seed data only | -1 level (min low) | All |
| Existing mitigating control | -1 level (min low) | All |
| Deprecated module | -1 level (min low) | All |
| Documentation/comments | -1 level (min low) | All |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- `EnableSensitiveDataLogging` in prod → high
- NationalId without encryption → critical
- Static encryption key in `appsettings.json` → critical

#### Python
- `print(email)` in prod → medium
- `pickle` serialization of PII → high
- Missing consent check → high

#### JavaScript/TypeScript
- PII in URL params → medium
- PII in localStorage without encryption → high
- Missing cookie consent → medium

#### Go
- PII in error return values → medium
- Missing middleware consent check → high

#### Java
- PII in `toString()` → medium
- Missing `@PreAuthorize` on PII endpoints → high

#### Rust
- PII in `Debug` → low
- Missing encryption before DB write → critical

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

- **audit-security**: May identify PII-handling endpoints needing encryption review
- **audit-architecture**: May identify data flow paths across modules
- **audit-observability**: May reveal PII in logs via telemetry config

### 9.3 Downstream Consumers

- **audit-regulatory-dataprotection**: Uses privacy findings for LGPD/GDPR compliance
- **audit-consolidated**: Aggregates all findings
- **audit-consistency-coherence**: Cross-domain alignment validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-privacy-data | audit-security | Privacy: PII handling + encryption; Security: vulnerabilities + access control |
| audit-privacy-data | audit-regulatory-dataprotection | Privacy: code-level PII patterns; Regulatory: legal compliance assessment |
| audit-privacy-data | audit-observability | Privacy: PII in data flows; Observability: PII in logs/traces |

### 9.5 Recommended Execution Order

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII + data protection (this squad)
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-consolidated** — Aggregate all
13. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip already-reported findings.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in Section 7 format.