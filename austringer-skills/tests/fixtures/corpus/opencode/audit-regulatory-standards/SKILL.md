---
name: audit-regulatory-standards
description: >-
  Industry standards and certifications compliance auditing: ISO 27001 control
  mapping to code, SOC 2 trust criteria validation, PCI-DSS payment flow audit,
  business continuity planning, backup encryption verification, vendor risk
  assessment, and RPO/RTO objective documentation. Validates that infrastructure
  and code-level controls satisfy industry standard requirements.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Audit Squad: Regulatory Industry Standards & Certifications

## Section 2: USE FOR / DO NOT USE FOR

### USE FOR

- ISO 27001 info security controls audit — map controls to code (encryption, access control, audit logging)
- SOC 2 trust criteria verification — security, availability, processing integrity, confidentiality, privacy
- PCI-DSS compliance audit — payment flow validation, cardholder data handling, network segmentation
- BCP/DR review — RPO/RTO objectives documented and tested
- Backup encryption verification — encrypted at rest, offsite/geo-redundant, tested restore
- Vendor risk assessment — third-party risk register, DPA review, SLA monitoring
- Change management — code review enforced, deployment approval, rollback procedures
- Infrastructure hardening — Keycloak/OpenBao production readiness, TLS, security headers
- Security scanning pipeline — Trivy/Snyk/Dependabot config, no blanket suppressions
- Incident response plan — roles, escalation paths, communication templates
- Config management — infra-as-code, environment parity, secrets management
- Access control certification — least privilege, MFA, privileged access management

### DO NOT USE FOR

- Sector-specific regulation (use `audit-regulatory-sector`)
- Code-level security vulns (use `audit-security`)
- Data protection regulation (use `audit-regulatory-dataprotection`)
- Dependency/license scanning (use `audit-dependencies` and `audit-license`)

---

## Section 3: Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repo names.

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
| ASP.NET Core | `Program.cs` + `*.csproj` with `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` with `Microsoft.EntityFrameworkCore` | EF Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` | FastAPI |
| Express | `package.json` listing `express` | Express.js |

### 3.3 Standards Compliance Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Security scanning | `.trivyignore`, `.snyk`, `dependabot.yml` | Vuln scanning |
| Backup scripts | `*backup*`, `*restore*`, `scripts/` | DR |
| TLS/SSL config | `*nginx*`, `*tls*`, `*ssl*`, `*.pem` | Transport security |
| Keycloak/OpenBao | `*keycloak*`, `*openbao*`, `*vault*` | Identity/secrets |
| Security headers | `*SecurityHeaders*`, `*Csp*` | HTTP security |
| Observability | `*otel*`, `*grafana*`, `*prometheus*` | Monitoring/alerting |
| CI/CD pipelines | `.gitlab-ci.yml`, `.github/workflows/` | Change management |

### 3.4 Discovery Procedure

1. Scan root dir for language/framework signals.
2. Identify security scanning, backup, TLS, identity management configs.
3. Check CI/CD for security gates, approval workflows.
4. Verify observability stack completeness (metrics, logs, traces, alerting).
5. Select matching checklist from Section 4 based on detected tech.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **ISO 27001 control mapping**: Encryption at rest (AES-256 + key rotation), access control (policy-based auth + claims), audit logging (immutable audit entries), config management (env vars, no secrets in code).
- [ ] **SOC 2 trust criteria**: Security (auth + encryption + logging), availability (health checks + alerting), processing integrity (input validation + error handling), confidentiality (encryption + access tiers), privacy (consent + DSAR + retention).
- [ ] **PCI-DSS payment flow**: No cardholder data in logs. TLS 1.2+. Network segmentation. No PAN in plaintext.
- [ ] **Backup encryption**: Scripts encrypt before storage. Keys separate from backup storage. Offsite/geo-redundant config exists. Restore procedure documented and tested.
- [ ] **Infrastructure hardening**: Keycloak NOT in dev mode in prod (realm export, HTTPS required, brute-force protection). OpenBao NOT in dev mode (storage backend, TLS, audit logging).
- [ ] **Security scanning pipeline**: Trivy/Snyk with no blanket suppressions. `.trivyignore` entries have individual CVE justifications (not year-range). CI blocks on HIGH/CRITICAL.
- [ ] **RPO/RTO documentation**: RPO and RTO documented per component. Backup frequency meets RPO. Failover meets RTO.
- [ ] **Change management**: Code review enforced in CI. Deployment requires approval. Rollback documented. Emergency change process defined.

### 4.2 Python Checklist

- [ ] **Encryption**: `cryptography` lib for sensitive data. HSM or secret manager for key mgmt. No keys in env vars without encryption wrapper.
- [ ] **Audit logging**: Structured logging with correlation IDs. Logs shipped to centralized system. Retention meets compliance.
- [ ] **Security scanning**: `bandit` in CI. `safety` or `pip-audit` for deps. No blanket ignore rules.
- [ ] **Backup and recovery**: DB backup automation. Restore tested on schedule. Backup encryption verified.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **Security headers**: CSP, HSTS, X-Frame-Options. No inline scripts without nonce. CORS restricted.
- [ ] **Dependency scanning**: `npm audit` / `yarn audit` in CI with no high/critical exceptions. Lockfile committed. Dependabot/Renovate.
- [ ] **Transport security**: TLS 1.2+. Certificate rotation automated. HSTS preloaded.
- [ ] **Change management**: PR reviews required. CI gates pass before merge. Deployment audit trail.

---

## Section 5: Anti-Patterns

### 5.1 Industry Standards Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | OpenBao/Keycloak in dev mode | critical | Prod using dev/unsealed mode | No audit, no TLS, default tokens, no persistence | Prod config: TLS, audit logging, sealed storage backend |
| 2 | No backup scheduling | critical | Backups manual/non-existent | Data loss in disaster, RPO/RTO unachievable | Automated backup + encryption + offsite + tested restore |
| 3 | Blanket Trivy suppression by year | high | `.trivyignore` suppresses all CVEs for year range | Real vulns hidden, false compliance | Individual CVE triage with justification per finding |
| 4 | Security scans allow_failure | medium | CI has `allow_failure: true` on security stage | Security regressions merge undetected | Security scans MUST block pipeline |
| 5 | No RPO/RTO documentation | high | Recovery objectives not documented | No measurable DR target, audit failure | Document RPO/RTO per component, backup aligned |
| 6 | Missing vendor risk assessment | medium | Third-party services without risk eval | Supply chain risk, SOC 2 audit failure | Vendor risk register with DPA, SLA, review schedule |
| 7 | Unencrypted backups | high | Backups stored without encryption | Data breach from backup theft | Encrypt backups before storage, separate keys |
| 8 | No restore testing | high | Backups exist but never restored | Backup corruption discovered during disaster | Scheduled restore tests with documented results |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets | critical | Credentials in configs or source | ISO 27001 A.9, credential leak | Secret manager or env vars |
| 2 | No audit logging | high | Critical ops not logged | SOC 2 compliance failure, no forensic trail | Structured logging for all critical operations |
| 3 | Missing health checks | medium | No readiness/liveness probes | SOC 2 availability criteria unmet | Health checks for all external deps |

---

## Section 6: Code Examples (Bad/Good Pairs)

### Example 1: Trivy Ignore Configuration

```yaml
# ❌ BAD: Blanket suppression by year range — hides real CVEs
# .trivyignore
CVE-2025-0001
CVE-2025-*
# Suppressing all 2025 CVEs

# ✅ GOOD: Individual CVE triage with justification
# .trivyignore
CVE-2025-12345  # False positive: library not used in affected code path, re-evaluate Q3 2026
CVE-2025-67890  # Accepted risk: dev-only dependency, not in production image
```

### Example 2: Backup Encryption Verification (.NET)

```csharp
// ❌ BAD: Backup script with no encryption
public class BackupService
{
    public async Task CreateBackupAsync()
    {
        var data = await _dbContext.ExportAllAsync();
        await File.WriteAllTextAsync($"/backups/dump_{DateTime.UtcNow:yyyyMMdd}.json", data);
    }
}

// ✅ GOOD: Encrypted backup with key separation
public class BackupService
{
    private readonly IEncryptionService _encryption;
    private readonly IOffsiteStorage _offsiteStorage;

    public async Task CreateBackupAsync()
    {
        var data = await _dbContext.ExportAllAsync();
        var encrypted = await _encryption.EncryptAsync(data);
        var fileName = $"backup_{DateTime.UtcNow:yyyyMMdd_HHmmss}.enc";

        await _offsiteStorage.StoreAsync(fileName, encrypted);

        _logger.LogInformation("Backup created: {FileName}, Size: {Size} bytes, Encrypted: true",
            fileName, encrypted.Length);
    }
}
```

### Example 3: Security Scan Gate in CI Pipeline

```yaml
# ❌ BAD: Security scan allowed to fail
security-scan:
  stage: security
  script:
    - trivy fs .
  allow_failure: true  # Pipeline continues even with critical CVEs

# ✅ GOOD: Security scan blocks pipeline
security-scan:
  stage: security
  script:
    - trivy fs --exit-code 1 --severity HIGH,CRITICAL .
  allow_failure: false  # Pipeline blocks on HIGH/CRITICAL findings
  rules:
    - if: $CI_PIPELINE_SOURCE == "schedule"
      when: never  # Exclude scheduled runs from blocking
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
  "description": "What was found and why it matters. Reference the standard clause (ISO 27001 A.x, SOC 2 CC.x, PCI-DSS x.x).",
  "recommendation": "Specific fix with standard clause reference and code example if applicable.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|architecture|observability",
  "standard": "ISO27001|SOC2|PCI-DSS|General"
}
```

### 7.2 Summary Report

```json
{
  "summary": {
    "squad": "audit-regulatory-standards",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 8,
    "bySeverity": { "critical": 1, "high": 3, "medium": 3, "low": 1 },
    "byCategory": { "compliance": 5, "security": 2, "architecture": 1 },
    "standardCoverage": { "ISO27001": 4, "SOC2": 3, "PCI-DSS": 1 },
    "filesScanned": 30,
    "filesWithFindings": 8
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Standards Example |
|-------|-----------|-------------------|
| **critical** | Immediate compliance failure, data loss, or security breach | OpenBao/Keycloak in dev mode, no backup, unencrypted backups |
| **high** | Significant compliance gap with audit failure risk | Blanket CVE suppression, no RPO/RTO, no restore testing |
| **medium** | Degraded compliance posture, incomplete docs | Missing vendor risk assessment, security scans allow_failure |
| **low** | Best practice deviation, minor doc gap | Missing CSP on one page, incomplete change log |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects data integrity or availability | +1 level | All tech |
| Finding involves encryption/key management | +1 level | All tech |
| Finding in production infra config | +1 level | Infrastructure |
| Finding affects compliance certification | +1 level | All tech |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in dev/test env only | -1 level (min low) | All tech |
| Finding has existing mitigating control | -1 level (min low) | All tech |
| Finding in deprecated code scheduled for removal | -1 level (min low) | All tech |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- OpenBao in dev mode → critical (no audit, no seal)
- Security stage `allow_failure: true` → medium (regressions merge silently)
- No health checks → medium (SOC 2 availability)

#### Python
- `bandit` not configured → medium (security scanning gap)
- Unencrypted backups → high (data exposure risk)
- No log correlation IDs → low (forensic difficulty)

#### JavaScript/TypeScript
- Missing CSP header → medium (XSS risk)
- `npm audit` with exceptions → medium (known vulns)
- No HSTS → high (transport security gap)

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-security ──────────────────────┐
audit-dependencies ──────────────────┤
audit-observability ─────────────────┤
audit-cicd ──────────────────────────┤
audit-regulatory-standards ◄─────────┤  (this squad)
audit-consolidated ◄─────────────────┘  (consumes all squad outputs)
audit-consistency-coherence            (consumes all squad outputs)
```

### 9.2 Upstream Dependencies

- **audit-security**: Code-level security findings — this squad validates controls satisfy ISO 27001, SOC 2, PCI-DSS
- **audit-dependencies**: CVE/license data — validates dep scanning meets compliance
- **audit-observability**: Telemetry findings — validates monitoring/alerting meets SOC 2 availability
- **audit-cicd**: Pipeline findings — validates change mgmt meets compliance

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Cross-domain alignment validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-regulatory-standards | audit-security | Security scans for code vulns; this squad validates standard compliance |
| audit-regulatory-standards | audit-observability | Observability validates telemetry; this squad validates SOC 2 availability |
| audit-regulatory-standards | audit-regulatory-sector | This squad covers industry standards; sector covers industry-specific regulations |

### 9.5 Recommended Execution Order

1. **audit-security** — Code-level security assessment
2. **audit-dependencies** — Third-party risk baseline
3. **audit-observability** — Telemetry validation
4. **audit-cicd** — Pipeline security
5. **audit-regulatory-standards** — Standard compliance (this squad)
6. **audit-consolidated** — Aggregate all findings

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip findings already reported by audit-security or audit-dependencies.
3. Cross-reference: escalate findings confirmed by multiple squads (security + standards).
4. Output in standardized Section 7 format for aggregation.