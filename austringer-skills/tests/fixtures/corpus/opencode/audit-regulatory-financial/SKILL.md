---
name: audit-regulatory-financial
description: >-
  Financial regulatory compliance auditing: securities compliance (CVM/CSSF),
  RBAC/ABAC policy validation, NDA access tier enforcement, AML/KYC reference
  validation, transaction approval workflows, and audit trail immutability.
  Validates that access control matches regulatory requirements for financial
  data and investor materials.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Audit Squad: Regulatory Financial & Access Control

## Section 2: USE FOR / DO NOT USE FOR

### USE FOR

- Securities/investor material compliance — CVM (Brazil), CSSF (Luxembourg) regulatory reqs
- RBAC/ABAC policy validation — role definitions match business access reqs
- NDA access tier audit — PRE_NDA vs POST_NDA content separation in code
- Claims-based auth audit — .NET policy-based auth with minimal required claims
- Financial page RBAC — module-specific auth policies, not blanket `[Authorize]`
- Transaction approval workflows — multi-step approval, segregation of duties
- KYC document verification — identity validation, AML screening integration
- Audit trail immutability — financial action logging, tamper-proof storage
- Role hierarchy verification — inheritance correctness, no privilege escalation
- Access tier enforcement — data classification aligned with access levels
- Export endpoint authorization — module-specific RBAC on data exports
- Financial disclosure requirements — regulatory disclosure completeness
- Admin action audit persistence — durable audit entries for all admin ops

### DO NOT USE FOR

- Tax calculation compliance (use `audit-tax-fiscal`)
- Financial model accuracy (use `audit-financial-model`)
- Data protection/privacy (use `audit-regulatory-dataprotection`)
- Code security vulnerabilities (use `audit-security`)

---

## Section 3: Target Discovery

Auto-detect project structure. No hardcoded repo names.

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

### 3.3 Financial/Access Control Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Authorization policies | `*Policy*`, `*Authorize*`, `*RBAC*`, `*ABAC*` | Access control |
| NDA access tiers | `*Nda*`, `*AccessTier*`, `*Vdr*` | NDA system |
| Financial models | `*Financial*`, `*Transaction*`, `*Portfolio*`, `*Securities*` | Financial domain |
| KYC/AML | `*Kyc*`, `*Aml*`, `*Screening*`, `*Identity*` | Compliance checks |
| Audit trail | `*Audit*`, `*AuditLog*`, `*AuditEntry*` | Audit logging |
| Export endpoints | `*Export*`, `*Report*`, `*Download*` | Data export |

### 3.4 Discovery

1. Scan root dir for lang/framework signals.
2. Identify auth, NDA, financial, KYC/AML patterns.
3. Check role/policy definitions and access tiers.
4. Verify audit trail (persistence, immutability).
5. Select checklist from Section 4.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **Claims-based auth audit**: All financial endpoints use policy-based auth
  (`[Authorize(Policy = "...")]`), not bare `[Authorize]`. Policies with specific claim reqs.
  No hardcoded role strings — use `AuthorizationOptions` policy registration.
- [ ] **Role hierarchy verification**: Role inheritance correct — no child with more permissions
  than parent. No circular inheritance. Admin role explicit, not implicit.
- [ ] **Access tier enforcement**: Data classification (public, internal, confidential, restricted)
  maps to access tiers. Checks at query level, not just UI. No data leakage between tiers.
- [ ] **Financial page RBAC**: Each module has its own auth policy. Export endpoints
  have module-specific RBAC (not blanket `RequireAuthorization`). View/edit/delete separate policies.
- [ ] **Transaction approval workflows**: Multi-step approval above threshold.
  Segregation of duties — creator ≠ approver. State machine prevents invalid transitions.
- [ ] **KYC document verification**: Document type validation (identity, proof of address, etc.).
  Document expiry tracking. AML screening integration (not just flag).
- [ ] **Audit trail immutability**: Financial action logs persisted to durable storage (DB, not
  just log files). Logs cannot be edited or deleted by any role. Timestamps UTC and immutable.
  Admin actions logged AND persisted as durable audit entries.
- [ ] **Enum usage for financial status**: `ReportingAccountStatus` and similar use enum types,
  not free-form strings. No magic strings. Values documented with regulatory meaning.
- [ ] **Export endpoint security**: All exports require specific permissions (not just
  "authenticated"). Logged with user, timestamp, data scope. Rate-limited to prevent bulk extraction.

### 4.2 Python Checklist

- [ ] **Django/FastAPI permission classes**: Financial views use custom permission classes
  with role/claim checks. Not `@login_required` only — must check specific permissions.
- [ ] **Role-based query filtering**: DB queries filtered by user's access tier.
  No `Model.objects.all()` on financial data without access tier filtering.
- [ ] **Audit log model**: Immutable audit log model, no update/delete endpoints.
  Admin actions written to DB, not just logged.
- [ ] **KYC integration**: Document verification calls external KYC service (not just file upload).
  Status tracked with timestamp and result.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **Route guards**: Financial pages have route guards checking specific permissions.
  No client-only access control — all checks replicated server-side.
- [ ] **API auth headers**: All financial API calls include auth context.
  No cached tokens beyond expiry. Token refresh with re-auth for sensitive ops.
- [ ] **Audit logging**: Financial actions logged with user context, action type, resource.
  Logs sent to server-side audit service, not browser console.

---

## Section 5: Anti-Patterns

### 5.1 Financial Regulatory Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Blanket RequireAuthorization on exports | high | `[Authorize]` without module-specific policy on exports | Any auth user exports any financial data | `[Authorize(Policy = "FinancialExport")]` |
| 2 | Admin actions logged not persisted | high | Admin actions to logger, not durable DB | Audit trail lost on log rotation, no regulatory evidence | Dual write: logger + immutable DB audit table |
| 3 | Free-form string for financial status | medium | `ReportingAccountStatus` as `string` not `enum` | Typos cause incorrect state, no compile-time safety | Enum with all valid states, documented regulatory meaning |
| 4 | Client-only access control | critical | Page visibility only in frontend | Bypass via direct API call, privilege escalation | Server-side auth on every endpoint |
| 5 | No segregation of duties | high | Creator can also approve transactions | Fraud risk, CVM/CSSF violation | Creator ≠ approver enforcement |
| 6 | Hardcoded role names in code | medium | `"Admin"`, `"Manager"` string literals in auth checks | Fragile, not auditable, inconsistent | Policy-based auth with named policies in DI |
| 7 | Missing KYC screening | high | KYC is file upload without screening | No AML/identity verification | Integration with KYC provider, screening result tracking |
| 8 | Audit log with update/delete API | high | Log entries modifiable or deletable | Tamper evidence destroyed, regulatory non-compliance | Immutable append-only audit log, no mutation endpoints |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets | critical | API keys, connection strings in source code | Credential exposure in version control | Env vars or secret manager |
| 2 | Catching and swallowing exceptions | high | Empty catch blocks in financial handlers | Silent transaction failures, audit gaps | Catch specific, log context, handle explicitly |
| 3 | Magic strings for roles/status | medium | String literals for role names and financial status | Typos, no compile-time safety | Enums or constants with documentation |

---

## Section 6: Code Examples (Bad/Good Pairs)

### Example 1: Module-Specific Authorization (.NET)

```csharp
// ❌ BAD: Blanket authorization — any authenticated user can export
[Authorize]
[HttpGet("api/financial/export")]
public async Task<IActionResult> ExportFinancialData()
{
    var data = await _financialService.GetAllAsync();
    return File(data.ToCsv(), "text/csv");
}

// ✅ GOOD: Module-specific policy with export permission
[Authorize(Policy = "FinancialExport")]
[HttpGet("api/financial/export")]
public async Task<IActionResult> ExportFinancialData()
{
    var userTenant = User.GetTenantId();
    var data = await _financialService.GetByTenantAsync(userTenant);
    await _auditService.LogAsync(new AuditEntry
    {
        Action = "FinancialExport",
        UserId = User.GetUserId(),
        Timestamp = DateTime.UtcNow,
        ResourceScope = userTenant
    });
    return File(data.ToCsv(), "text/csv");
}
```

### Example 2: Durable Audit Trail (.NET)

```csharp
// ❌ BAD: Admin action logged but not persisted
public class AdminService
{
    private readonly ILogger<AdminService> _logger;

    public async Task DeleteUserAsync(Guid userId)
    {
        _logger.LogWarning("Admin deleting user {UserId}", userId);
        await _userRepository.DeleteAsync(userId);
    }
}

// ✅ GOOD: Dual write — logger + immutable audit table
public class AdminService
{
    private readonly ILogger<AdminService> _logger;
    private readonly IAuditRepository _auditRepository;

    public async Task DeleteUserAsync(Guid userId, Guid performedBy)
    {
        _logger.LogWarning("Admin {AdminId} deleting user {UserId}", performedBy, userId);

        await _auditRepository.AppendAsync(new AuditEntry
        {
            ActionType = AuditActionType.UserDeletion,
            PerformedBy = performedBy,
            TargetId = userId,
            Timestamp = DateTime.UtcNow,
            Metadata = JsonSerializer.Serialize(new { reason = "admin_action" })
        });

        await _userRepository.DeleteAsync(userId);
    }
}
```

### Example 3: Role-Based Query Filtering (Python)

```python
# ❌ BAD: No access tier filtering — returns all financial data
class FinancialViewSet(viewsets.ModelViewSet):
    queryset = FinancialRecord.objects.all()  # No filtering

# ✅ GOOD: Filter by user's access tier
class FinancialViewSet(viewsets.ModelViewSet):
    def get_queryset(self):
        user_tier = self.request.user.access_tier
        if user_tier == AccessTier.ADMIN:
            return FinancialRecord.objects.all()
        return FinancialRecord.objects.filter(
            access_level__lte=user_tier.level
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
  "description": "What was found and why it matters. Include the specific regulatory reference (CVM, CSSF, etc.).",
  "recommendation": "Specific fix with code example. Reference the regulatory requirement.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|architecture|observability",
  "regulation": "CVM|CSSF|AML|KYC|General"
}
```

### 7.2 Summary Report

```json
{
  "summary": {
    "squad": "audit-regulatory-financial",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 10,
    "bySeverity": { "critical": 1, "high": 4, "medium": 4, "low": 1 },
    "byCategory": { "compliance": 6, "security": 3, "architecture": 1 },
    "regulationCoverage": { "CVM": 4, "CSSF": 3, "AML": 2, "KYC": 1 },
    "filesScanned": 35,
    "filesWithFindings": 12
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Immediate regulatory violation, fraud risk, data breach | Client-only access control, no segregation of duties, mutable audit logs |
| **high** | Significant compliance gap, clear penalty | Blanket auth on exports, admin actions not persisted, no KYC screening |
| **medium** | Degraded compliance posture | Free-form strings for status, hardcoded role names, partial RBAC |
| **low** | Best practice deviation | Missing permission description, minor policy naming inconsistency |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding in financial transaction flow | +1 level | All technologies |
| Finding involves investor/securities data | +1 level | Financial systems |
| Finding affects multiple tenants | +1 level | Multi-tenant systems |
| Finding in public-facing financial endpoint | +1 level | Web applications |
| Finding involves admin/privileged ops | +1 level | All technologies |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in test/mock code with documented purpose | -1 level (min low) | All technologies |
| Finding has existing mitigating control | -1 level (min low) | All technologies |
| Finding in deprecated code | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Adjustments

#### .NET/C#
- Blanket `[Authorize]` on financial exports → high (unauthorized data access)
- Admin actions logged not persisted → high (no audit trail)
- `ReportingAccountStatus` as string → medium (type safety)
- Missing `[Authorize]` on admin endpoints → critical (auth bypass)

#### Python
- `objects.all()` without access tier filtering → high (data leak)
- Mutable audit log model → high (tamper evidence destroyed)
- Missing KYC screening → high (regulatory non-compliance)

#### JavaScript/TypeScript
- Client-only route guards → critical (privilege escalation)
- Missing token refresh for sensitive ops → medium (session risk)
- No server-side audit logging → high (no evidence trail)

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-security ──────────────────────┐
audit-architecture ──────────────────┤
audit-regulatory-financial ◄─────────┤  (this squad)
audit-regulatory-dataprotection ─────┤
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### 9.2 Upstream

- **audit-security**: Auth findings — validates financial regulatory compliance
- **audit-architecture**: Data flow — validates access tier + segregation of duties
- **audit-regulatory-dataprotection**: Data protection — adds financial-specific regulatory layers

### 9.3 Downstream

- **audit-consolidated**: Aggregates findings
- **audit-consistency-coherence**: Cross-domain validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-regulatory-financial | audit-security | Security scans vulnerabilities; this squad validates financial regulatory compliance |
| audit-regulatory-financial | audit-tax-fiscal | This covers access control/securities; tax-fiscal covers tax calculation |
| audit-regulatory-financial | audit-financial-model | This covers regulatory access control; financial-model covers pricing/logic |

### 9.5 Recommended Execution Order

1. **audit-security** — Verify auth/authorization controls
2. **audit-architecture** — Analyze data flows and module structure
3. **audit-regulatory-financial** — Validate financial regulatory compliance (this squad)
4. **audit-consolidated** — Aggregate all findings

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip findings already reported by audit-security.
3. Cross-reference: escalate findings confirmed by both security and this squad.
4. Output in standardized Section 7 format.