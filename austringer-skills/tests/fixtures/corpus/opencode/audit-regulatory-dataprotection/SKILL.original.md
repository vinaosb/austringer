---
name: audit-regulatory-dataprotection
description: >-
  Data protection regulatory compliance auditing: LGPD (Lei 13.709), GDPR,
  consent mechanism validation, data subject rights implementation, breach
  notification infrastructure, DPO obligations, and cross-border data transfer
  controls. Validates both code-level enforcement and legal compliance posture.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Audit Squad: Regulatory Data Protection (LGPD/GDPR)

## Section 2: USE FOR / DO NOT USE FOR

### USE FOR

- LGPD article-by-article compliance audit — verify compliance with Lei 13.709/2018 (Arts. 7, 18, 33, 41, 48)
- GDPR adequacy verification — Articles 6, 17, 33, 35 for EU-relevant projects
- Consent mechanism validation — legal basis registration, consent collection/storage/withdrawal APIs
- Data subject rights (DSR) implementation — access, deletion, portability, correction endpoints
- Breach notification protocol verification — Art. 48 notification timeline, content requirements, infrastructure
- DPO (Encarregado) obligation audit — Art. 41 requirements, contact availability, process documentation
- Cross-border data transfer compliance — Art. 33 adequacy decisions, standard contractual clauses
- DPIA (Data Protection Impact Assessment) review — Art. 38 high-risk processing identification
- Data retention policy enforcement — per-entity retention schedules, automated purge mechanisms
- Encryption-at-rest verification — field-level encryption, key rotation, DEK/KEK management
- Consent API validation in .NET — legal basis enum completeness, data categories, collection method tracking
- DSAR endpoint completeness audit — right to access, erasure, portability, rectification, objection
- PII field audit across database schemas — sensitive data classification, minimization compliance
- Breach notification infrastructure — alerting pipelines, contact point configuration, SLA enforcement
- Consent revocation propagation — downstream service notification, data processing cessation
- Privacy policy completeness review — no placeholder emails, current legal entity, DPO contact

### DO NOT USE FOR

- PII scanning in code content (use `audit-privacy-data`)
- License compliance scanning (use `audit-license`)
- Security vulnerability scanning (use `audit-security`)
- General code quality review (use `audit-code-quality`)

---

## Section 3: Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` | FastAPI |
| Express | `package.json` listing `express` | Express.js |

### 3.3 Data Protection Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Consent models | `*Consent*`, `*LegalBasis*`, `*DataSubject*` | Consent management |
| Encryption config | `*Encrypt*`, `*Vault*`, `*KeyRotation*`, `*DekCache*` | Encryption at rest |
| Retention policies | `*Retention*`, `*DataLifecycle*`, `*Purge*` | Data lifecycle |
| DSAR endpoints | `*DataSubjectRequest*`, `*DSAR*`, `*Portability*` | Data subject rights |
| Breach notification | `*Breach*`, `*Notification*`, `*DPO*` | Breach handling |
| Privacy policy | `*PrivacyPolicy*`, `*TermsOfService*` | Legal documents |

### 3.4 Discovery Procedure

1. Scan root directory for language/framework signals.
2. Identify consent, encryption, retention, and DSAR patterns.
3. Check for multi-tenancy configuration (Finbuckle, tenant resolvers).
4. Verify breach notification infrastructure (alerting, email, contact points).
5. Select matching checklist from Section 4 based on detected technologies.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **Consent API validation**: Legal basis enum covers all LGPD Art. 7 bases (consent, legal obligation,
  contract execution, legitimate interest, credit protection, health protection, regulatory compliance).
  Data categories mapped to legal bases. Collection method tracked per consent record.
- [ ] **DSAR endpoint completeness**: Endpoints exist for access (Art. 18 I), correction (Art. 18 II),
  anonymization (Art. 18 III), portability (Art. 18 V), deletion (Art. 18 VI), consent revocation (Art. 18 IX).
  Each endpoint returns within regulatory timeframes.
- [ ] **Encryption at rest**: Sensitive fields encrypted before DB storage. DEK cached in-process memory only
  (never in distributed cache). Key rotation mechanism exists with zero-downtime re-encryption.
- [ ] **Data retention policy enforcement**: Per-entity retention schedules (not blanket 7-year for all).
  Automated purge/scheduled jobs for expired data. Retention policies documented per data category.
- [ ] **PII field audit**: All PII fields identified and classified. Sensitive fields use encrypted columns.
  No PII in log output (structured logging with redaction). No PII in error messages.
- [ ] **Breach notification infrastructure**: Alert pipeline triggers within 72 hours (LGPD Art. 48).
  DPO contact point configured. Notification template includes required fields (nature of data,
  affected subjects, measures taken, risks). Contact points not placeholder emails.
- [ ] **Consent revocation propagation**: Revoking consent stops all downstream processing.
  No consent bypassing in non-production environments (mock provider must not return "authorized"
  without actual consent check). Audit trail records revocation timestamp.
- [ ] **Data portability export completeness**: Export includes all data categories. Export format is
  machine-readable (JSON/CSV). Export does not store as unencrypted plaintext on disk.
  Export respects data minimization — only data belonging to the requesting subject.
- [ ] **Privacy policy completeness**: No placeholder emails (e.g., `dpo@example.com`). Legal entity
  name current. DPO contact information present. Last updated date visible. Covers all processing
  activities. Version tracking for policy changes.
- [ ] **Multi-tenant data isolation**: Tenant-scoped DSAR queries. No cross-tenant data leakage in
  export endpoints. Tenant isolation verified at database query level.

### 4.2 Python Checklist

- [ ] **Consent middleware**: Django/FastAPI middleware validates consent before processing PII.
  Consent stored with timestamps and legal basis. Audit trail immutable.
- [ ] **DSAR handler completeness**: All Art. 18 rights implemented as API endpoints or management
  commands. Response time within regulatory requirements.
- [ ] **Encryption**: `cryptography` library for field-level encryption. Keys from environment/HSM,
  never hardcoded. Key rotation without service restart.
- [ ] **Data retention**: Celery/ARQ scheduled tasks for data purge. Per-model retention configuration.
  Soft-delete before hard-delete with grace period.
- [ ] **Logging redaction**: No PII in log output. Structured logging with field redaction. Sensitive
  fields masked in exception handlers.
- [ ] **Environment separation**: `.env.example` documents all data protection config variables.
  Production consent checks never bypassed via environment flags.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **Consent banner/mechanism**: Frontend consent capture sends legal basis and data categories.
  Consent state persisted server-side (not cookie-only). Granular consent per processing purpose.
- [ ] **Data subject request UI**: Forms for access, deletion, portability requests. Confirmation
  email sent. Status tracking visible to data subject.
- [ ] **Client-side data minimization**: Only necessary PII collected in forms. No sensitive data in
  URL parameters. No PII in client-side logs or analytics events.
- [ ] **Third-party data sharing**: Cookie consent for analytics. Data Processing Agreements (DPA)
  documented for all third-party processors. No unauthorized data transmission.

---

## Section 5: Anti-Patterns

### 5.1 Data Protection Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Consent bypass in non-prod | critical | Mock provider returning "authorized" without consent check | Data processed without legal basis, violates Art. 7 | Mock should simulate consent workflow; log bypass clearly |
| 2 | Blanket retention policy | high | Single 7-year retention for all entities regardless of category | Over-retention violates minimization principle (Art. 8) | Per-entity retention schedules with automated enforcement |
| 3 | DSR export as plaintext | high | Data subject export stored as unencrypted file on disk | PII exposure, unauthorized access risk | Encrypt export, deliver via secure channel, auto-delete after download |
| 4 | Placeholder DPO contact | critical | `dpo@example.com` or `privacy@domain.com` in privacy policy | Regulatory contact unreachable, violates Art. 41 | Real DPO contact verified and reachable |
| 5 | Consent in cookies only | high | Consent state stored only in browser cookies, not server-side | Cannot verify consent for DSAR, no audit trail | Server-side consent record with timestamp, legal basis, collection method |
| 6 | No consent revocation propagation | high | Revoking consent does not stop downstream processing | Processing continues without legal basis | Event-driven revocation cascade to all processors |
| 7 | PII in log output | high | Sensitive data logged in plaintext (names, emails, CPF) | PII leak via log aggregation, violates confidentiality | Structured logging with field redaction, PII masked |
| 8 | Missing breach notification SLA | high | No 72-hour alert pipeline for breach notification | Violates LGPD Art. 48 timeline, regulatory penalty | Automated breach detection with alerting pipeline and DPO contact |
| 9 | Unencrypted PII at rest | critical | Sensitive fields stored as plaintext in database | Data breach exposes PII directly | Field-level encryption with key rotation |
| 10 | Cross-tenant data in DSAR | high | Data export includes records from other tenants | Cross-tenant data leak, privacy violation | Tenant-scoped queries with isolation verification |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets | critical | API keys, encryption keys in source code | Credential leak via version control | Environment variables or secret manager |
| 2 | Catching and swallowing exceptions | high | Empty catch blocks in consent/DSAR handlers | Silent data processing failures | Catch specific exceptions, log with context, handle explicitly |
| 3 | Magic strings for legal basis | medium | `"consent"` string literals instead of enum | Typos cause incorrect legal basis | Enum with all LGPD Art. 7 legal bases |

---

## Section 6: Code Examples (Bad/Good Pairs)

### Example 1: Consent Bypass in Non-Production (.NET)

```csharp
// ❌ BAD: Mock provider bypasses consent entirely
public class MockConsentProvider : IConsentProvider
{
    public Task<ConsentResult> CheckConsentAsync(string subjectId, string purpose) =>
        Task.FromResult(new ConsentResult { IsAuthorized = true }); // Always authorized!
}

// ✅ GOOD: Mock simulates real consent workflow
public class MockConsentProvider : IConsentProvider
{
    private readonly Dictionary<(string, string), bool> _consents = new();

    public Task<ConsentResult> CheckConsentAsync(string subjectId, string purpose)
    {
        var hasConsent = _consents.TryGetValue((subjectId, purpose), out var authorized) && authorized;
        return Task.FromResult(new ConsentResult
        {
            IsAuthorized = hasConsent,
            LegalBasis = hasConsent ? LegalBasis.Consent : LegalBasis.None,
            CheckedAt = DateTimeOffset.UtcNow
        });
    }
}
```

### Example 2: Per-Entity Data Retention Policy (.NET)

```csharp
// ❌ BAD: Blanket 7-year retention for all entities
public class DataRetentionService
{
    public TimeSpan GetRetentionPeriod(string entityType) => TimeSpan.FromDays(7 * 365);
}

// ✅ GOOD: Per-entity retention with regulatory mapping
public class DataRetentionService
{
    private readonly Dictionary<string, RetentionPolicy> _policies = new()
    {
        ["FinancialTransaction"] = new(TimeSpan.FromDays(10 * 365), "Art. 174 CTN — tax retention"),
        ["ConsentRecord"] = new(TimeSpan.FromDays(5 * 365), "LGPD Art. 8 — consent proof retention"),
        ["MarketingInteraction"] = new(TimeSpan.FromDays(2 * 365), "Legitimate interest — 24 months"),
        ["SecurityLog"] = new(TimeSpan.FromDays(180), "Operational necessity — 6 months"),
    };

    public RetentionPolicy GetRetentionPolicy(string entityType) =>
        _policies.GetValueOrDefault(entityType)
        ?? throw new InvalidOperationException($"No retention policy defined for {entityType}");
}

public record RetentionPolicy(TimeSpan Duration, string LegalBasis);
```

### Example 3: PII Redaction in Python Logging

```python
# ❌ BAD: PII logged in plaintext
import logging

logger = logging.getLogger(__name__)

def process_data_subject_request(subject_id: str, request_type: str):
    logger.info(f"Processing {request_type} request for user {subject_id}")
    # subject_id is PII — logged in plaintext

# ✅ GOOD: Structured logging with PII redaction
import logging
import hashlib

logger = logging.getLogger(__name__)

def _redact(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()[:8]

def process_data_subject_request(subject_id: str, request_type: str):
    logger.info(
        "Processing DSAR request",
        extra={
            "request_type": request_type,
            "subject_hash": _redact(subject_id),
            "timestamp": datetime.utcnow().isoformat(),
        },
    )
```

---

## Section 7: Output Format

All audit findings MUST be emitted as structured JSON objects.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the specific LGPD/GDPR article reference.",
  "recommendation": "Specific fix with code example. Reference the regulatory article and correct pattern.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|architecture|observability",
  "regulation": "LGPD|GDPR|Both",
  "article": "Art. 7 I|Art. 18|Art. 48|Art. 41|GDPR Art. 6|GDPR Art. 17"
}
```

### 7.2 Summary Report

```json
{
  "summary": {
    "squad": "audit-regulatory-dataprotection",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 12,
    "bySeverity": { "critical": 2, "high": 4, "medium": 5, "low": 1 },
    "byCategory": { "compliance": 8, "security": 3, "architecture": 1 },
    "regulationCoverage": { "LGPD": 10, "GDPR": 5, "Both": 3 },
    "filesScanned": 45,
    "filesWithFindings": 15
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Data Protection Example |
|-------|-----------|------------------------|
| **critical** | Immediate regulatory violation, data breach risk, or consent bypass | Consent always returning authorized, unencrypted PII at rest, placeholder DPO contact |
| **high** | Significant compliance gap with clear regulatory penalty | Missing DSAR endpoint, no breach notification pipeline, consent not propagated |
| **medium** | Degraded compliance posture, incomplete implementation | Missing retention policy for entity type, incomplete privacy policy, partial consent tracking |
| **low** | Best practice deviation, documentation gap | Missing consent timestamp, verbose error messages, minor policy wording issues |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects data subject rights | +1 level | All technologies |
| Finding involves PII exposure | +1 level | All technologies |
| Finding affects multiple tenants | +1 level | Multi-tenant systems |
| Finding is in public-facing consent/DSAR endpoint | +1 level | Web applications |
| Finding involves financial/health PII (special category) | +1 level | Sensitive data systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test/mock code with clear documentation | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Mock consent provider always returning authorized → critical (consent bypass)
- Blanket retention without per-entity policy → high (over-retention)
- DSAR export stored as unencrypted plaintext → high (PII exposure)
- Missing `[Authorize]` on DSAR endpoints → high (unauthorized access)

#### Python
- Missing consent middleware → high (processing without legal basis)
- PII in log output → high (data leak)
- No encryption for PII fields → critical (data breach risk)

#### JavaScript/TypeScript
- Consent stored only in cookies → medium (no server audit trail)
- Missing data minimization in forms → low (over-collection)
- Third-party data sharing without DPA → high (unauthorized processing)

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-privacy-data ──────────────────┐
audit-security ──────────────────────┤
audit-architecture ──────────────────┤
audit-observability ─────────────────┤
audit-regulatory-dataprotection ◄────┤  (this squad)
audit-consolidated ◄─────────────────┘  (consumes all squad outputs)
audit-consistency-coherence            (consumes all squad outputs)
```

### 9.2 Upstream Dependencies

- **audit-privacy-data**: Provides PII scanning results — this squad validates whether regulatory controls
  exist for the PII that audit-privacy-data identifies
- **audit-security**: Provides encryption and auth security findings — this squad validates regulatory
  compliance of those controls
- **audit-architecture**: Provides data flow analysis — this squad validates whether flows comply with
  data protection regulations

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-regulatory-dataprotection | audit-privacy-data | Privacy-data scans for PII in code; this squad validates regulatory controls and compliance |
| audit-regulatory-dataprotection | audit-security | Security scans for vulnerabilities; this squad validates regulatory compliance of data protection |
| audit-regulatory-dataprotection | audit-regulatory-financial | This squad covers data protection; financial covers securities/access control regulations |

### 9.5 Recommended Execution Order

1. **audit-privacy-data** — Identify PII and data flows
2. **audit-security** — Verify security controls
3. **audit-regulatory-dataprotection** — Validate regulatory compliance (this squad)
4. **audit-consolidated** — Aggregate all findings

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by audit-privacy-data.
3. Cross-reference: escalate findings confirmed by both privacy-data and this squad.
4. Output in the standardized Section 7 format for aggregation.
