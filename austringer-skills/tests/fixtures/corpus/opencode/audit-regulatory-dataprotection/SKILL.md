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

- LGPD compliance audit — Lei 13.709/2018 (Arts. 7, 18, 33, 41, 48)
- GDPR adequacy verification — Articles 6, 17, 33, 35 for EU projects
- Consent mechanism validation — legal basis registration, collection/storage/withdrawal APIs
- DSR implementation — access, deletion, portability, correction endpoints
- Breach notification protocol — Art. 48 timeline, content requirements, infra
- DPO (Encarregado) obligation audit — Art. 41 requirements, contact availability, process docs
- Cross-border data transfer — Art. 33 adequacy decisions, standard contractual clauses
- DPIA review — Art. 38 high-risk processing identification
- Data retention policy enforcement — per-entity retention schedules, automated purge
- Encryption-at-rest verification — field-level encryption, key rotation, DEK/KEK
- Consent API validation (.NET) — legal basis enum, data categories, collection method
- DSAR endpoint completeness — access, erasure, portability, rectification, objection
- PII field audit across DB schemas — classification, minimization
- Breach notification infra — alerting pipelines, contact points, SLA
- Consent revocation propagation — downstream notification, data cessation
- Privacy policy review — no placeholder emails, current legal entity, DPO contact

### DO NOT USE FOR

- PII scanning in code (use `audit-privacy-data`)
- License scanning (use `audit-license`)
- Security scanning (use `audit-security`)
- Code quality review (use `audit-code-quality`)

---

## Section 3: Target Discovery

Auto-detect. No hardcoded repo names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JavaScript source | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | EF Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` | FastAPI |
| Express | `package.json` listing `express` | Express.js |

### 3.3 Data Protection Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Consent models | `*Consent*`, `*LegalBasis*`, `*DataSubject*` | Consent mgmt |
| Encryption config | `*Encrypt*`, `*Vault*`, `*KeyRotation*`, `*DekCache*` | Encryption at rest |
| Retention policies | `*Retention*`, `*DataLifecycle*`, `*Purge*` | Data lifecycle |
| DSAR endpoints | `*DataSubjectRequest*`, `*DSAR*`, `*Portability*` | Data subject rights |
| Breach notification | `*Breach*`, `*Notification*`, `*DPO*` | Breach handling |
| Privacy policy | `*PrivacyPolicy*`, `*TermsOfService*` | Legal documents |

### 3.4 Discovery

1. Scan root dir for lang/framework signals.
2. Identify consent, encryption, retention, DSAR patterns.
3. Check multi-tenancy (Finbuckle, tenant resolvers).
4. Verify breach infra (alerting, email, contacts).
5. Select checklist from Section 4.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **Consent API validation**: Legal basis enum covers all LGPD Art. 7 bases (consent, legal obligation,
  contract execution, legitimate interest, credit protection, health protection, regulatory compliance).
  Data categories mapped to legal bases. Collection method tracked per consent record.
- [ ] **DSAR endpoint completeness**: Endpoints for access (Art. 18 I), correction (Art. 18 II),
  anonymization (Art. 18 III), portability (Art. 18 V), deletion (Art. 18 VI), consent revocation (Art. 18 IX).
  Each returns within regulatory timeframes.
- [ ] **Encryption at rest**: Sensitive fields encrypted before DB storage. DEK in-process memory only
  (never distributed cache). Key rotation with zero-downtime re-encryption.
- [ ] **Data retention enforcement**: Per-entity retention (not blanket 7-year). Auto purge for expired data.
  Retention policies documented per data category.
- [ ] **PII field audit**: All PII fields identified and classified. Sensitive fields use encrypted columns.
  No PII in log output (structured logging with redaction). No PII in error messages.
- [ ] **Breach notification infra**: Alert pipeline triggers within 72 hours (LGPD Art. 48).
  DPO contact configured. Notification template includes required fields (nature of data,
  affected subjects, measures taken, risks). No placeholder email contacts.
- [ ] **Consent revocation propagation**: Revocation stops all downstream processing.
  No bypass in non-prod (mock must not return "authorized" without consent check). Audit trail records revocation.
- [ ] **Data portability export completeness**: Export includes all data categories. Machine-readable format
  (JSON/CSV). Not stored unencrypted on disk. Respects data minimization — only requesting subject's data.
- [ ] **Privacy policy completeness**: No placeholder emails (`dpo@example.com`). Legal entity name current.
  DPO contact present. Last updated date visible. Covers all processing activities. Version tracking.
- [ ] **Multi-tenant data isolation**: Tenant-scoped DSAR queries. No cross-tenant leakage in
  export endpoints. Tenant isolation verified at DB query level.

### 4.2 Python Checklist

- [ ] **Consent middleware**: Django/FastAPI middleware validates consent before PII processing.
  Consent stored with timestamps and legal basis. Audit trail immutable.
- [ ] **DSAR handler completeness**: All Art. 18 rights as API endpoints or management commands.
  Response within regulatory requirements.
- [ ] **Encryption**: `cryptography` library for field-level encryption. Keys from env/HSM, never hardcoded.
  Key rotation without service restart.
- [ ] **Data retention**: Celery/ARQ scheduled tasks for data purge. Per-model retention config.
  Soft-delete → hard-delete with grace period.
- [ ] **Logging redaction**: No PII in log output. Structured logging with field redaction. Sensitive
  fields masked in exception handlers.
- [ ] **Environment separation**: `.env.example` documents all data protection config vars.
  Production consent checks never bypassed via env flags.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **Consent banner/mechanism**: Frontend consent capture sends legal basis + data categories.
  Consent persisted server-side (not cookie-only). Granular per processing purpose.
- [ ] **DSR UI**: Forms for access, deletion, portability requests. Confirmation email sent.
  Status tracking visible to data subject.
- [ ] **Client-side data minimization**: Only necessary PII collected. No sensitive data in URL params.
  No PII in client-side logs or analytics events.
- [ ] **Third-party data sharing**: Cookie consent for analytics. DPA documented for all processors.
  No unauthorized data transmission.

---

## Section 5: Anti-Patterns

### 5.1 Data Protection Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Consent bypass in non-prod | critical | Mock returns "authorized" without consent check | No legal basis, violates Art. 7 | Mock simulates consent; log clearly |
| 2 | Blanket retention policy | high | Single 7-year retention for all entities | Over-retention violates Art. 8 | Per-entity retention with automated enforcement |
| 3 | DSR export as plaintext | high | Export stored unencrypted on disk | PII exposure, unauthorized access | Encrypt export, secure channel, auto-delete |
| 4 | Placeholder DPO contact | critical | `dpo@example.com` in privacy policy | Unreachable contact, violates Art. 41 | Real DPO contact verified |
| 5 | Consent in cookies only | high | Consent only in browser, not server-side | No DSAR verification, no audit trail | Server-side consent with timestamp + basis |
| 6 | No revocation propagation | high | Revoking consent doesn't stop downstream processing | Processing continues without legal basis | Event-driven revocation cascade |
| 7 | PII in log output | high | Plaintext names, emails, CPF in logs | PII leak via log aggregation | Structured logging with redaction |
| 8 | Missing breach SLA | high | No 72-hour alert pipeline | Violates LGPD Art. 48 | Automated detection + alerting |
| 9 | Unencrypted PII at rest | critical | Plaintext fields in DB | Data breach exposes PII directly | Field-level encryption + key rotation |
| 10 | Cross-tenant data in DSAR | high | Export includes other tenant records | Cross-tenant leak, privacy violation | Tenant-scoped queries + isolation |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets | critical | API keys, encryption keys in source code | Credential leak via version control | Env vars or secret manager |
| 2 | Catching and swallowing exceptions | high | Empty catch blocks in consent/DSAR handlers | Silent data processing failures | Catch specific exceptions, log context, handle explicitly |
| 3 | Magic strings for legal basis | medium | `"consent"` string literals instead of enum | Typos cause incorrect legal basis | Enum with all LGPD Art. 7 bases |

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

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Direct regulatory violation, data breach, consent bypass | Consent always authorized, unencrypted PII, placeholder DPO |
| **high** | Significant compliance gap, clear penalty | Missing DSAR endpoint, no breach pipeline, consent not propagated |
| **medium** | Degraded compliance, partial implementation | Missing retention policy, incomplete privacy policy, partial consent tracking |
| **low** | Best practice deviation, doc gap | Missing timestamp, verbose errors, minor wording |

### 8.2 Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects data subject rights | +1 level | All technologies |
| Finding involves PII exposure | +1 level | All technologies |
| Finding affects multiple tenants | +1 level | Multi-tenant systems |
| Finding in public-facing consent/DSAR endpoint | +1 level | Web applications |
| Finding involves financial/health PII (special category) | +1 level | Sensitive data systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in test/mock code with clear docs | -1 level (min low) | All technologies |
| Finding has existing mitigating control | -1 level (min low) | All technologies |
| Finding in deprecated code scheduled for removal | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Adjustments

#### .NET/C#
- Mock consent always authorized → critical (consent bypass)
- Blanket retention without per-entity policy → high (over-retention)
- DSAR export unencrypted plaintext → high (PII exposure)
- Missing `[Authorize]` on DSAR endpoints → high (unauthorized access)

#### Python
- Missing consent middleware → high (processing without legal basis)
- PII in log output → high (data leak)
- No encryption for PII fields → critical (data breach risk)

#### JavaScript/TypeScript
- Consent in cookies only → medium (no server audit trail)
- Missing data minimization in forms → low (over-collection)
- Third-party sharing without DPA → high (unauthorized processing)

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-privacy-data ──────────────────┐
audit-security ──────────────────────┤
audit-architecture ──────────────────┤
audit-observability ─────────────────┤
audit-regulatory-dataprotection ◄────┤  (this squad)
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### 9.2 Upstream

- **audit-privacy-data**: PII scans — this squad validates regulatory controls
- **audit-security**: Encryption/auth findings — validates regulatory compliance
- **audit-architecture**: Data flow analysis — validates flow compliance with data protection regs

### 9.3 Downstream

- **audit-consolidated**: Aggregates findings
- **audit-consistency-coherence**: Cross-domain validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-regulatory-dataprotection | audit-privacy-data | Privacy-data scans PII in code; this squad validates regulatory controls |
| audit-regulatory-dataprotection | audit-security | Security scans vulnerabilities; this squad validates regulatory compliance |
| audit-regulatory-dataprotection | audit-regulatory-financial | This covers data protection; financial covers securities/access control |

### 9.5 Recommended Execution Order

1. **audit-privacy-data** — Identify PII and data flows
2. **audit-security** — Verify security controls
3. **audit-regulatory-dataprotection** — Validate regulatory compliance (this squad)
4. **audit-consolidated** — Aggregate all findings

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip findings already reported by audit-privacy-data.
3. Cross-reference: escalate findings confirmed by both privacy-data and this squad.
4. Output in standardized Section 7 format.