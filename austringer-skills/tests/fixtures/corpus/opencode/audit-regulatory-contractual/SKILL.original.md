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

- NDA template validation — mutual/unilateral completeness, jurisdiction, duration, termination clauses
- Investor agreement compliance — term sheets, SAFE notes, equity structures, shareholder agreements
- Terms of service and privacy policy review — completeness, legal entity, governing law, dispute resolution
- Disclaimer and risk disclosure verification — financial data disclaimers, investment risk warnings
- Legal language consistency across documents — defined terms, entity references, cross-references
- IP assignment and non-compete clause validity — work product ownership, non-compete scope and duration
- NDA system enforcement verification — access tier enforcement in code, VDR manifest integrity
- Placeholder text detection — no placeholder emails, names, or legal text in production documents
- Financial data export disclaimer coverage — risk disclaimers on all financial data exports
- Contract lifecycle management — expiry tracking, renewal notifications, amendment audit trail
- Jurisdiction and governing law consistency — same jurisdiction across related agreements
- VDR (Virtual Data Room) manifest validation — document inventory completeness, access log integrity

### DO NOT USE FOR

- Access control implementation in code (use `audit-regulatory-financial`)
- Privacy/data protection regulation (use `audit-regulatory-dataprotection`)
- Code security vulnerabilities (use `audit-security`)

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

### 3.3 Legal/Contractual Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| NDA system | `*Nda*`, `*AccessTier*`, `*Vdr*`, `*DataRoom*` | NDA enforcement |
| Legal templates | `*Legal*`, `*Contract*`, `*Agreement*`, `*Terms*` | Legal documents |
| Disclaimers | `*Disclaimer*`, `*Disclosure*`, `*RiskWarning*` | Risk disclosure |
| IP/Equity | `*IpAssignment*`, `*Equity*`, `*Shareholder*` | Investment agreements |
| Document management | `*Document*`, `*Template*`, `*Clause*` | Document lifecycle |

### 3.4 Discovery Procedure

1. Scan root directory for language/framework signals.
2. Identify NDA, legal template, disclaimer, and document management patterns.
3. Check for access tier enforcement tied to NDA status.
4. Verify disclaimer/disclaimer coverage on financial pages and exports.
5. Select matching checklist from Section 4 based on detected technologies.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **NDA access tier enforcement**: Code enforces PRE_NDA vs POST_NDA content separation. Access
  tier checks on API endpoints, not just UI hiding. NDA status tracked per user with document
  reference (signed NDA document ID linked to access tier).
- [ ] **Legal document template validation**: Templates use version-controlled content, not hardcoded
  strings. Template variables populated with real data (no placeholder emails, names, or entities).
  Template rendering produces valid legal documents with all required clauses.
- [ ] **Disclaimer coverage**: All financial data pages display appropriate disclaimers. Export endpoints
  include disclaimer text in exported files. Disclaimer content reviewed by legal (version/date tracked).
- [ ] **Terms of service completeness**: ToS includes governing law, dispute resolution, limitation of
  liability, termination clause, data handling, cookie policy. Version tracked with acceptance log.
- [ ] **IP assignment verification**: Employee/contractor IP assignment clauses documented. System tracks
  IP assignment acceptance per user. Work product ownership clear in legal documents.
- [ ] **Contract lifecycle**: Expiry tracking for NDAs and agreements. Renewal notification triggers
  (email/dashboard). Amendment audit trail (who changed what, when). No expired NDA granting access.
- [ ] **VDR manifest integrity**: Document inventory matches actual documents. Access log immutable.
  Document classification (confidential, restricted) matches access tier.

### 4.2 Python Checklist

- [ ] **NDA middleware**: Django/FastAPI middleware enforces access tier based on NDA status.
  Tier downgrade when NDA expires. Audit log for tier changes.
- [ ] **Template rendering**: Jinja/Django templates with legal clause validation. No placeholder
  content in production renders. Template version tracking.
- [ ] **Disclaimer API**: Disclaimer endpoint serves current, versioned disclaimer content.
  Disclaimer acceptance logged with timestamp and user.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **NDA gate UI**: Financial content pages check NDA status before rendering. Graceful handling
  when NDA expired (redirect to renewal, not just hide content). Access tier displayed to user.
- [ ] **Export disclaimer**: Client-side export functions include disclaimer text. PDF/CSV exports
  contain disclaimer footer or header. Disclaimer matches current legal version.
- [ ] **Document viewer**: Secure document viewer with access tier enforcement. No download without
  appropriate NDA tier. Watermarking for confidential documents.

---

## Section 5: Anti-Patterns

### 5.1 Contractual Compliance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Placeholder legal text in production | critical | `dpo@example.com`, `[Company Name]`, `TBD` in live legal documents | Invalid contracts, regulatory non-compliance, legal unenforceability | Template variables with real data, CI check for placeholder patterns |
| 2 | Missing disclaimers on financial exports | high | Exported financial data has no risk disclaimer | Regulatory violation, potential liability | Disclaimer footer/header on all financial exports |
| 3 | NDA tier check in UI only | high | Access control only hides content in frontend | Direct API access bypasses NDA tier restriction | Server-side access tier enforcement on every endpoint |
| 4 | Expired NDA still grants access | critical | NDA expiration does not revoke access tier | Unauthorized access to confidential data post-expiry | Automated tier downgrade on NDA expiry with notification |
| 5 | No contract version tracking | medium | Legal documents have no version or last-updated date | Cannot determine which terms apply to which user | Version field on all legal documents with acceptance log |
| 6 | Inconsistent jurisdiction across agreements | medium | Related agreements reference different governing laws | Legal ambiguity, dispute resolution uncertainty | Consistent jurisdiction across all related legal documents |
| 7 | VDR manifest out of sync | medium | Document inventory does not match actual documents | Missing documents undetected, audit failure | Automated manifest reconciliation with alerting |
| 8 | No amendment audit trail | high | Contract amendments not tracked | Cannot verify which version applies, legal disputes | Immutable amendment log with who/when/what |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded legal text | medium | Legal clauses hardcoded in source files | Changes require code deployment, no legal review cycle | Externalized templates with version control |
| 2 | Missing acceptance logging | high | Users access content without logging ToS/NDA acceptance | Cannot prove consent, legal vulnerability | Acceptance log with timestamp, IP, document version |
| 3 | Generic error on access denial | low | "Access denied" instead of NDA-specific message | Users confused about why access blocked | Specific message: "NDA required — contact [DPO]" |

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
| **critical** | Invalid or unenforceable legal document, unauthorized access | Placeholder text in production, expired NDA still granting access |
| **high** | Significant compliance gap with legal liability | Missing disclaimers on exports, NDA check in UI only, no amendment trail |
| **medium** | Degraded compliance posture, incomplete documentation | No version tracking, inconsistent jurisdiction, VDR manifest out of sync |
| **low** | Best practice deviation, minor issue | Generic access denied message, hardcoded legal text |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding involves investor-facing documents | +1 level | All technologies |
| Finding affects contractual enforceability | +1 level | All technologies |
| Finding involves confidential data access | +1 level | All technologies |
| Finding affects multiple jurisdictions | +1 level | Multi-jurisdiction systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in draft/template under development | -1 level (min low) | All technologies |
| Finding has existing mitigating control | -1 level (min low) | All technologies |
| Finding is in deprecated code | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Placeholder text in legal template → critical (unusable legal document)
- NDA tier check in UI only → high (access bypass)
- No disclaimer on financial export → high (regulatory liability)

#### Python
- Missing disclaimer in export → high (liability risk)
- No NDA middleware enforcement → high (access bypass)
- Hardcoded legal text → medium (inflexible)

#### JavaScript/TypeScript
- Missing NDA gate on financial pages → high (unauthorized viewing)
- No disclaimer in client export → medium (liability)
- Generic access denied message → low (UX)

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

- **audit-regulatory-financial**: Provides access control/RBAC findings — this squad validates
  whether NDA-specific access tiers are correctly enforced in contractual context
- **audit-regulatory-dataprotection**: Provides privacy/consent findings — this squad validates
  whether contractual documents align with data protection requirements

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-regulatory-contractual | audit-regulatory-financial | Financial covers RBAC/access control; this squad covers NDA enforcement and contractual documents |
| audit-regulatory-contractual | audit-regulatory-dataprotection | Dataprotection covers LGPD/GDPR; this squad covers contractual/ToS alignment with privacy requirements |

### 9.5 Recommended Execution Order

1. **audit-regulatory-financial** — Verify access control foundations
2. **audit-regulatory-dataprotection** — Verify data protection alignment
3. **audit-regulatory-contractual** — Validate contractual enforcement (this squad)
4. **audit-consolidated** — Aggregate all findings

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by financial or dataprotection squads.
3. Cross-reference: escalate contractual findings confirmed by access control issues.
4. Output in the standardized Section 7 format for aggregation.
