---
name: audit-regulatory-contractual
description: >-
  Contractual and legal document compliance auditing: NDA template validation,
  investor agreement structure, terms of service completeness, disclaimer
  coverage, IP clause verification, and legal language consistency. Validates
  that legal documents are complete, contain no placeholder text, and enforce
  contractual obligations in code where applicable.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Audit Squad: Regulatory Contractual & Legal Documents

## Section 2: USE FOR / DO NOT USE FOR

### USE FOR

- NDA template validation — mutual/unilateral completeness, jurisdiction, duration, termination
- Investor agreement compliance — term sheets, SAFE notes, equity structures, shareholder agreements
- ToS and privacy policy review — completeness, legal entity, governing law, dispute resolution
- Disclaimer and risk disclosure — financial data disclaimers, investment risk warnings
- Legal language consistency — defined terms, entity references, cross-references
- IP assignment and non-compete — work product ownership, scope and duration
- NDA system enforcement — access tier enforcement in code, VDR manifest integrity
- Placeholder text detection — no placeholder emails, names, legal text in prod docs
- Financial data export disclaimer — risk disclaimers on all exports
- Contract lifecycle management — expiry tracking, renewal notifications, amendment audit trail
- Jurisdiction and governing law consistency — same jurisdiction across related agreements
- VDR manifest validation — document inventory completeness, access log integrity

### DO NOT USE FOR

- Access control implementation (use `audit-regulatory-financial`)
- Privacy/data protection regulation (use `audit-regulatory-dataprotection`)
- Code security vulnerabilities (use `audit-security`)

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
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` | FastAPI |
| Express | `package.json` listing `express` | Express.js |

### 3.3 Legal/Contractual Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| NDA system | `*Nda*`, `*AccessTier*`, `*Vdr*`, `*DataRoom*` | NDA enforcement |
| Legal templates | `*Legal*`, `*Contract*`, `*Agreement*`, `*Terms*` | Legal documents |
| Disclaimers | `*Disclaimer*`, `*Disclosure*`, `*RiskWarning*` | Risk disclosure |
| IP/Equity | `*IpAssignment*`, `*Equity*`, `*Shareholder*` | Investment agreements |
| Document mgmt | `*Document*`, `*Template*`, `*Clause*` | Document lifecycle |

### 3.4 Discovery Procedure

1. Scan root for language/framework signals.
2. Identify NDA, legal template, disclaimer, document management patterns.
3. Check access tier enforcement tied to NDA status.
4. Verify disclaimer coverage on financial pages and exports.
5. Select matching checklist from Section 4.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **NDA access tier enforcement**: Code enforces PRE_NDA vs POST_NDA. Access tier checks on API endpoints, not UI-only. NDA status tracked per user with doc reference.
- [ ] **Legal template validation**: Version-controlled templates, not hardcoded strings. Variables populated with real data (no placeholder emails/names). All required clauses present.
- [ ] **Disclaimer coverage**: Financial data pages display disclaimers. Export endpoints include disclaimer text. Content reviewed by legal (version/date tracked).
- [ ] **ToS completeness**: Governing law, dispute resolution, liability limitation, termination, data handling, cookie policy. Version tracked with acceptance log.
- [ ] **IP assignment**: IP assignment clauses documented. System tracks acceptance per user. Work product ownership clear.
- [ ] **Contract lifecycle**: Expiry tracking for NDAs/agreements. Renewal notifications. Amendment audit trail (who/what/when). No expired NDA granting access.
- [ ] **VDR manifest integrity**: Document inventory matches actual documents. Immutable access log. Classification matches access tier.

### 4.2 Python Checklist

- [ ] **NDA middleware**: Django/FastAPI middleware enforces access tier by NDA status. Tier downgrade on expiry. Audit log for tier changes.
- [ ] **Template rendering**: Jinja/Django templates with legal clause validation. No placeholder content in prod. Template version tracking.
- [ ] **Disclaimer API**: Serves current versioned disclaimer. Acceptance logged with timestamp + user.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **NDA gate UI**: Financial pages check NDA before rendering. Graceful handling on expired NDA (redirect to renewal). Access tier displayed.
- [ ] **Export disclaimer**: Client exports include disclaimer text. PDF/CSV with disclaimer header/footer. Matches current legal version.
- [ ] **Document viewer**: Secure viewer with access tier enforcement. No download without appropriate tier. Watermarking for confidential docs.

---

## Section 5: Anti-Patterns

### 5.1 Contractual Compliance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Placeholder legal text in prod | critical | `dpo@example.com`, `[Company Name]`, `TBD` in live docs | Invalid contracts, regulatory non-compliance, unenforceability | Template vars with real data, CI check for placeholders |
| 2 | Missing disclaimers on exports | high | Financial exports without risk disclaimer | Regulatory violation, liability | Disclaimer footer on all financial exports |
| 3 | NDA tier check in UI only | high | Access control only hides content in frontend | Direct API bypass | Server-side enforcement on every endpoint |
| 4 | Expired NDA still grants access | critical | Expiration doesn't revoke tier | Unauthorized access post-expiry | Automated tier downgrade on expiry with notification |
| 5 | No contract version tracking | medium | No version/last-updated date | Can't determine which terms apply | Version field on all legal docs with acceptance log |
| 6 | Inconsistent jurisdiction | medium | Related agreements reference different governing laws | Legal ambiguity | Consistent jurisdiction across all related docs |
| 7 | VDR manifest out of sync | medium | Document inventory doesn't match actual | Missing docs undetected | Automated reconciliation with alerting |
| 8 | No amendment audit trail | high | Amendments not tracked | Can't verify which version applies | Immutable amendment log |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded legal text | medium | Legal clauses hardcoded in source | Changes require code deploy, no legal review | Externalized templates with version control |
| 2 | Missing acceptance logging | high | Users access content without ToS/NDA acceptance | Can't prove consent | Acceptance log with timestamp, IP, doc version |
| 3 | Generic error on denial | low | "Access denied" instead of NDA-specific message | Confused users | Specific message: "NDA required — contact [DPO]" |

---

## Section 6: Code Examples (Bad/Good Pairs)

### Example 1: NDA Access Tier Enforcement (.NET)

```csharp
// ❌ BAD: UI-only access control — content hidden but API accessible
[HttpGet("api/investor/materials")]
public async Task<IActionResult> GetInvestorMaterials()
{
    var materials = await _materialService.GetAllAsync();
    return Ok(materials); // No NDA check — anyone authenticated can access
}

// ✅ GOOD: Server-side NDA tier enforcement
[HttpGet("api/investor/materials")]
[Authorize(Policy = "PostNdaAccess")]
public async Task<IActionResult> GetInvestorMaterials()
{
    var userId = User.GetUserId();
    var ndaStatus = await _ndaService.GetNdaStatusAsync(userId);

    if (ndaStatus.Tier != AccessTier.PostNda || ndaStatus.ExpiresAt < DateTime.UtcNow)
    {
        return Forbid("NDA required or expired. Contact legal@example.com to renew.");
    }

    var materials = await _materialService.GetByAccessTierAsync(ndaStatus.Tier);
    return Ok(materials);
}
```

### Example 2: Disclaimer on Financial Export (Python)

```python
# ❌ BAD: Export without disclaimer
from fastapi import APIRouter
from fastapi.responses import StreamingResponse

router = APIRouter()

@router.get("/api/financial/export")
async def export_financial_data():
    data = await financial_service.get_all()
    csv_content = data.to_csv()
    return StreamingResponse(
        iter([csv_content]),
        media_type="text/csv",
        headers={"Content-Disposition": "attachment; filename=financial_data.csv"},
    )

# ✅ GOOD: Export with disclaimer footer
from datetime import datetime

CURRENT_DISCLAIMER = (
    "\n\n--- DISCLAIMER ---\n"
    f"This data is provided for informational purposes only and does not constitute "
    f"investment advice. Past performance is not indicative of future results. "
    f"Generated: {datetime.utcnow().isoformat()}Z | Disclaimer v2.1\n"
)

@router.get("/api/financial/export")
async def export_financial_data(user_id: str = Depends(get_current_user_id)):
    data = await financial_service.get_user_data(user_id)
    csv_content = data.to_csv() + CURRENT_DISCLAIMER
    return StreamingResponse(
        iter([csv_content]),
        media_type="text/csv",
        headers={"Content-Disposition": "attachment; filename=financial_data.csv"},
    )
```

### Example 3: Placeholder Text Detection (.NET)

```csharp
// ❌ BAD: Template with placeholder content
public class LegalDocumentService
{
    public string GenerateNda(string counterpartyName)
    {
        return $@"
            NON-DISCLOSURE AGREEMENT
            Between: [COMPANY LEGAL NAME]  // Placeholder!
            And: {counterpartyName}
            Contact: legal@example.com     // Placeholder email!
            Jurisdiction: [STATE/COUNTRY]  // Placeholder!
        ";
    }
}

// ✅ GOOD: Template with validated real data
public class LegalDocumentService
{
    private readonly ILegalEntityRepository _legalEntity;
    private readonly IDisclaimerRepository _disclaimers;

    public async Task<string> GenerateNdaAsync(string counterpartyName, string jurisdiction)
    {
        var entity = await _legalEntity.GetCurrentAsync();
        var disclaimer = await _disclaimers.GetCurrentAsync("NDA_DISCLAIMER");

        return $@"
            NON-DISCLOSURE AGREEMENT (v{disclaimer.Version})
            Between: {entity.LegalName}
            And: {counterpartyName}
            Contact: {entity.LegalContactEmail}
            Jurisdiction: {jurisdiction}
            Generated: {DateTime.UtcNow:yyyy-MM-dd}
        ";
    }
}
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
  "description": "What was found and why it matters. Reference the contractual requirement.",
  "recommendation": "Specific fix with code example and legal template reference.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|architecture|maintainability",
  "contractType": "NDA|ToS|Investor|Disclaimer|IP|General"
}
```

### 7.2 Summary Report

```json
{
  "summary": {
    "squad": "audit-regulatory-contractual",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 9,
    "bySeverity": { "critical": 2, "high": 3, "medium": 3, "low": 1 },
    "byCategory": { "compliance": 7, "architecture": 1, "maintainability": 1 },
    "contractTypeCoverage": { "NDA": 4, "ToS": 2, "Disclaimer": 2, "IP": 1 },
    "filesScanned": 20,
    "filesWithFindings": 8
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Contractual Example |
|-------|-----------|---------------------|
| **critical** | Invalid/unenforceable legal doc, unauthorized access | Placeholder text in prod, expired NDA granting access |
| **high** | Significant compliance gap, legal liability | Missing disclaimers on exports, UI-only NDA check, no amendment trail |
| **medium** | Degraded compliance, incomplete docs | No version tracking, inconsistent jurisdiction, VDR out of sync |
| **low** | Best practice deviation | Generic access denied, hardcoded legal text |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Investor-facing documents | +1 level | All |
| Affects contractual enforceability | +1 level | All |
| Involves confidential data access | +1 level | All |
| Multiple jurisdictions | +1 level | Multi-jurisdiction |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Draft/template under development | -1 level (min low) | All |
| Existing mitigating control | -1 level (min low) | All |
| Deprecated code | -1 level (min low) | All |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Placeholder text in legal template → critical
- NDA tier check in UI only → high
- No disclaimer on financial export → high

#### Python
- Missing disclaimer in export → high
- No NDA middleware → high
- Hardcoded legal text → medium

#### JavaScript/TypeScript
- Missing NDA gate on financial pages → high
- No disclaimer in client export → medium
- Generic access denied → low

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-regulatory-financial ──────────┐
audit-regulatory-dataprotection ─────┤
audit-regulatory-contractual ◄───────┤  (this squad)
audit-consolidated ◄─────────────────┘  (consumes all squad outputs)
audit-consistency-coherence            (consumes all squad outputs)
```

### 9.2 Upstream Dependencies

- **audit-regulatory-financial**: Access control/RBAC findings — validates NDA-tier enforcement in contractual context
- **audit-regulatory-dataprotection**: Privacy/consent findings — validates contractual docs align with data protection

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all findings
- **audit-consistency-coherence**: Cross-domain alignment validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-regulatory-contractual | audit-regulatory-financial | Financial: RBAC/access control; This: NDA enforcement + contractual docs |
| audit-regulatory-contractual | audit-regulatory-dataprotection | Dataprotection: LGPD/GDPR; This: contractual/ToS alignment with privacy |

### 9.5 Recommended Execution Order

1. **audit-regulatory-financial** — Access control foundations
2. **audit-regulatory-dataprotection** — Data protection alignment
3. **audit-regulatory-contractual** — Contractual enforcement (this squad)
4. **audit-consolidated** — Aggregate all

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip findings already reported by financial or dataprotection.
3. Cross-reference: escalate contractual findings confirmed by access control issues.
4. Output in Section 7 format.