---
name: audit-tax-fiscal
description: >-
  Tax and fiscal compliance auditing: tax calculation verification, fiscal document compliance,
  cross-border tax rule validation, TIN/GIIN format verification, FATCA/CRS reporting accuracy,
  and multi-jurisdiction tax handling across financial codebases.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Tax & Fiscal Compliance Audit

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- Tax calculation verification — rates match jurisdiction requirements
- Fiscal compliance of billing logic — billing handles tax per locale
- Cross-border tax rules — withholding, VAT/GST for international ops
- Sales tax accuracy — calculations, exemptions, thresholds
- Invoice fiscal compliance — mandatory fields, fiscal document numbering
- Tax benefit validation — incentive claims match eligibility
- Transfer pricing review — multi-entity pricing issues
- TIN format validation — Taxpayer ID validation per country
- GIIN validation — Global Intermediary ID patterns
- FATCA/CRS XML generation — regulatory reporting correctness
- Extension deadline handling — overdue logic uses effective dates including extensions
- Multi-jurisdiction tax handling — correct jurisdiction selection and rate
- .NET tax computation — ExtensionDeadline ?? FilingDeadline pattern
- Python tax calculation — decimal precision in tax computations
- JavaScript tax module — rate lookup and application logic

### DO NOT USE FOR

- General financial model accuracy (use `audit-financial-model`)
- Financial regulatory (securities, RBAC, AML/KYC) (use `audit-regulatory-financial`)
- Data protection compliance (use `audit-regulatory-dataprotection`)
- Security vulns in tax endpoints (use `audit-security`)
- General code quality in tax modules (use `audit-code-quality`)

---

## Section 2: Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repo names.

### 2.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go source files | `*.go`, `go.mod` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle` | Java |
| Rust source files | `*.rs`, `Cargo.toml` | Rust |

### 2.2 Tax/Fiscal Domain Detection

| Signal | Pattern | Domain |
|--------|---------|--------|
| Tax types | `TaxRate`, `TaxBracket`, `Filing`, `TaxJurisdiction`, `TaxReturn` | Tax computation |
| Fiscal documents | `Invoice`, `FiscalDocument`, `NFCe`, `NFSe`, `CTe` | Fiscal docs |
| Identifiers | `TIN`, `GIIN`, `CNPJ`, `CPF`, `EIN`, `VATNumber` | Tax ID validation |
| Reporting | `FATCA`, `CRS`, `XML`, `XSD`, `Schema` | Regulatory reporting |
| Deadlines | `FilingDeadline`, `ExtensionDeadline`, `DueDate`, `Overdue` | Deadline mgmt |
| Jurisdictions | `Jurisdiction`, `Country`, `State`, `Municipality` | Multi-jurisdiction |

### 2.3 Discovery Procedure

1. Scan root dir for language signal files.
2. Identify tax/fiscal domain by searching tax, fiscal, filing, jurisdiction patterns.
3. Identify frameworks by config analysis.
4. Identify test framework by test dir/config presence.
5. Select matching checklist from Section 3 based on detected tech.
6. If multiple languages (monorepo), run all applicable checklists.

---

## Section 3: Technology-Specific Checklists

### 3.1 .NET/C# Tax/Fiscal Checklist

- [ ] **Extension deadline handling**: Overdue checks MUST use `ExtensionDeadline ?? FilingDeadline`. Using only `FilingDeadline` ignores extensions, produces false overdue.
- [ ] **Multi-jurisdiction tax**: Verify jurisdiction selection handles overlapping (federal + state + municipal). No single-rate assumptions.
- [ ] **Fiscal document generation**: Verify mandatory fields: taxpayer ID, tax amount, rate, itemization, document number.
- [ ] **TIN format validation**: `IsValidTinFormat` returns `false` for unsupported countries, NOT `true`. Returning `true` lets invalid TINs pass.
- [ ] **GIIN validation**: GIIN format (16 chars: XXXXXX.XXXXX.XX.XXX). Verify regex matches FATCA spec.
- [ ] **FATCA/CRS XML generation**: XML validated against published XSD schemas. No ad-hoc XML without validation.
- [ ] **XSD schema validation**: Regulatory XML validated against official XSD. Schemas versioned and current.
- [ ] **Tax rate lookup accuracy**: Rates from config/database, not hardcoded. Effective dates checked against filing period.
- [ ] **Decimal precision**: All tax calc uses `decimal` with appropriate rounding for jurisdiction.
- [ ] **Tax exemption handling**: Exempt entities/types handled explicitly, not by applying zero rate.

### 3.2 Python Tax/Fiscal Checklist

- [ ] **Decimal for tax**: Use `decimal.Decimal`, not `float`.
- [ ] **Jurisdiction-specific rounding**: Different jurisdictions require different rounding rules.
- [ ] **Tax rate config**: Rates from external config/database, not hardcoded.
- [ ] **Fiscal document numbering**: Sequential with no gaps (regulatory requirement in many jurisdictions).
- [ ] **XML generation safety**: Use XML libraries (lxml, xml.etree), not string concatenation. Escape special chars.

### 3.3 JavaScript/TypeScript Tax/Fiscal Checklist

- [ ] **Decimal library**: No native `number` for tax. Use `decimal.js` or equivalent.
- [ ] **Tax rate API reliability**: Error handling and fallback for rate lookups.
- [ ] **Currency/tax coupling**: Tax in same currency as transaction. No cross-currency tax without conversion.
- [ ] **Fiscal document validation**: Client-side matches server-side rules. No discrepancies.

### 3.4 Cross-Language Tax/Fiscal Checklist

- [ ] **Extension deadline awareness**: All overdue logic checks for extension grants before flagging overdue.
- [ ] **Multi-jurisdiction stacking**: Tax correctly stacked across jurisdictions (e.g., ICMS + PIS/COFINS in Brazil).
- [ ] **Withholding tax treatment**: Withholding deducted at source where required.
- [ ] **Tax treaty benefits**: Double taxation treaty benefits applied where eligible.
- [ ] **Transfer pricing docs**: Intercompany transactions documented at arm's length.
- [ ] **Audit trail**: All tax calculations logged with inputs, rates, outputs for regulatory audit.
- [ ] **Retroactive rate changes**: Handle retroactive changes with effective date logic.

---

## Section 4: Anti-Patterns

### 4.1 .NET/C# Tax/Fiscal Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `IsValidTinFormat` returns true for unknown countries | critical | Default `true` for unsupported country codes | Invalid TINs pass validation — regulatory violation | Return `false` for unsupported with logged warning |
| 2 | Overdue check ignores extension deadlines | high | `if (DateTime.Now > filing.FilingDeadline)` without extension check | False overdue flags, operational disruption | Use `filing.ExtensionDeadline ?? filing.FilingDeadline` |
| 3 | Hardcoded tax rates | high | Rates embedded in source code | Rate changes need deploy; stale rates in prod | Externalize to config/database with effective dates |
| 4 | Single-jurisdiction assumption | high | One tax rate per transaction | Multi-jurisdiction needs tax stacking | Support multiple jurisdictions per transaction |
| 5 | XML string concat for FATCA/CRS | critical | Building regulatory XML with string concat | Malformed XML fails submission | Use XML libraries + XSD validation |
| 6 | Missing XSD validation | high | FATCA/CRS XML not validated against schema | Invalid submissions rejected | Validate against official XSD before submit |

### 4.2 Python Tax/Fiscal Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Float for tax amounts | critical | `float` in tax calculations | Rounding errors compound, regulatory non-compliance | `decimal.Decimal` for all tax values |
| 2 | f-string XML generation | high | `f"<tax>{amount}</tax>"` for regulatory XML | Injection risk, malformed XML | Use `lxml` or `xml.etree.ElementTree` |
| 3 | Missing fiscal document sequence | high | Non-sequential invoice numbers | Regulatory violation in many jurisdictions | Enforce sequential numbering with gap detection |

### 4.3 JavaScript/TypeScript Tax/Fiscal Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Native number for tax | critical | `number` type for tax calculations | IEEE 754 precision loss | Use `decimal.js` or integer cents |
| 2 | Client-only tax validation | high | Tax rules only enforced client-side | Server bypassed — regulatory risk | Server-side validation as source of truth |

### 4.4 Cross-Language Tax/Fiscal Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Ignoring extension deadlines | high | Only checking base filing deadline | Extensions legally binding — false overdue | `effectiveDeadline = extension ?? base` |
| 2 | No tax audit trail | medium | Tax calc not logged with inputs | Can't reconstruct for regulatory audit | Log: inputs, rates, jurisdiction, outputs, timestamp |
| 3 | Missing retroactive rate handling | medium | Always using current tax rate | Rate changes applied incorrectly | Apply rate effective at transaction date, not current |
| 4 | Withholding tax omission | high | Cross-border payments without withholding | Regulatory violation, penalties | Apply withholding based on payee jurisdiction and treaty |

---

## Section 5: Code Examples (Bad/Good Pairs)

### Example 1: Effective Due Date with Extension Deadline (.NET/C#)

```csharp
// BAD: Only checks FilingDeadline, ignores extensions
public bool IsOverdue(Filing filing)
{
    return DateTime.UtcNow > filing.FilingDeadline;
}

// GOOD: Uses extension deadline if granted, otherwise filing deadline
public bool IsOverdue(Filing filing)
{
    var effectiveDeadline = filing.ExtensionDeadline ?? filing.FilingDeadline;
    return DateTime.UtcNow > effectiveDeadline;
}
```

### Example 2: TIN Format Validation (.NET/C#)

```csharp
// BAD: Returns true for unknown countries — any TIN passes
public static bool IsValidTinFormat(string tin, string countryCode)
{
    if (countryCode == "US") return Regex.IsMatch(tin, @"^\d{3}-\d{2}-\d{4}$");
    if (countryCode == "BR") return Regex.IsMatch(tin, @"^\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}$");
    return true; // DANGER: Unknown countries pass any TIN
}

// GOOD: Returns false for unsupported countries with logging
public static bool IsValidTinFormat(string tin, string countryCode)
{
    if (countryCode == "US") return Regex.IsMatch(tin, @"^\d{3}-\d{2}-\d{4}$");
    if (countryCode == "BR") return Regex.IsMatch(tin, @"^\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}$");
    if (countryCode == "DE") return Regex.IsMatch(tin, @"^\d{11}$");

    Log.Warning("TIN validation not implemented for country {Country}", countryCode);
    return false; // Reject unsupported countries — fail-safe
}
```

### Example 3: FATCA/CRS XML with Schema Validation (.NET/C#)

```csharp
// BAD: String concatenation for regulatory XML
public string GenerateFatcaReport(FilingData data)
{
    return $"<FATCA><Report>{data.TaxId}</Report></FATCA>";
}

// GOOD: Use XML library + XSD validation
public XDocument GenerateFatcaReport(FilingData data)
{
    var doc = new XDocument(
        new XElement("FATCA",
            new XElement("Report",
                new XElement("TaxId", data.TaxId))));

    var schema = XmlSchema.Read(File.OpenRead("fatca-v2.0.xsd"), null);
    doc.Validate(new[] { schema }, (sender, args) =>
    {
        throw new InvalidOperationException($"FATCA XML validation failed: {args.Message}");
    });

    return doc;
}
```

### Example 4: Multi-Jurisdiction Tax Stacking (.NET/C#)

```csharp
// BAD: Single tax rate assumption
public decimal CalculateTotalTax(decimal amount, decimal taxRate)
{
    return amount * taxRate;
}

// GOOD: Stack taxes across jurisdictions
public decimal CalculateTotalTax(decimal amount, IEnumerable<TaxJurisdiction> jurisdictions)
{
    return jurisdictions
        .Where(j => j.AppliesTo(amount))
        .Sum(j => Math.Round(amount * j.Rate, FinancialPrecision.Default, MidpointRounding.AwayFromZero));
}
```

### Example 5: Decimal Tax Calculation in Python

```python
# BAD: float for tax — precision loss
def calculate_tax(amount: float, rate: float) -> float:
    return round(amount * rate, 2)

# GOOD: Decimal for all tax calculations
from decimal import Decimal, ROUND_HALF_UP

def calculate_tax(amount: Decimal, rate: Decimal) -> Decimal:
    tax = amount * rate
    return tax.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
```

### Example 6: JavaScript Tax Safety

```javascript
// BAD: Native number for tax
function calculateTax(amount, rate) {
    return Math.round(amount * rate * 100) / 100;
}

// GOOD: Decimal.js for tax computation
import Decimal from "decimal.js";

function calculateTax(amount, rate) {
    return new Decimal(amount)
        .times(rate)
        .toDecimalPlaces(2, Decimal.ROUND_HALF_UP);
}
```

---

## Section 6: Output Format

All audit findings MUST be emitted as structured JSON objects.

### 6.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "IsValidTinFormat returns true for unsupported country codes, allowing invalid TINs to pass",
  "recommendation": "Return false for unsupported countries and log a warning for implementation tracking",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|architecture|performance|maintainability|testing|observability"
}
```

### 6.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 7.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Line where finding ends.
- **description**: Factual, specific. Include problematic code fragment. No hedging.
- **recommendation**: Actionable. Include code snippet showing fix.
- **technology**: Primary tech of finding.
- **category**: `compliance` for regulatory violations, `architecture` for design flaws, `maintainability` for code issues.

### 6.3 Summary Report

```json
{
  "summary": {
    "squad": "audit-tax-fiscal",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 10,
    "bySeverity": { "critical": 2, "high": 4, "medium": 3, "low": 1 },
    "byCategory": { "compliance": 5, "architecture": 3, "maintainability": 2 },
    "filesScanned": 25,
    "filesWithFindings": 8
  }
}
```

---

## Section 7: Severity Guidelines

### 7.1 Base Severity Definitions

| Level | Definition | Tax/Fiscal Context Example |
|-------|-----------|---------------------------|
| **critical** | Regulatory violation, incorrect tax in prod | TIN validation accepting invalid IDs, wrong tax rate in billing, FATCA XML without schema validation |
| **high** | Significant compliance risk, missing jurisdiction handling | Missing extension deadline, hardcoded tax rates, single-jurisdiction assumption |
| **medium** | Degraded compliance posture, edge case gaps | Missing audit trail, no retroactive rate handling, outdated XSD |
| **low** | Best practice improvement | Doc gaps, logging improvements |

### 7.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects production billing/invoicing | +1 level | All tech |
| Finding involves FATCA/CRS reporting | +1 level | All tech |
| Finding affects cross-border transactions | +1 level | All tech |
| Finding involves TIN/GIIN validation | +1 level | All tech |
| Finding affects multiple jurisdictions | +1 level | All tech |
| Finding in government submission path | +1 level | All tech |

### 7.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in test/demo code only | -1 level (min low) | All tech |
| Finding has existing mitigating control | -1 level (min low) | All tech |
| Finding in deprecated code | -1 level (min low) | All tech |

---

## Section 8: Integration Notes

### 8.1 Squad Dependency Graph

```
audit-dependencies ────────────────────┐
audit-security ────────────────────────┤
audit-architecture ────────────────────┤
audit-financial-model ─────────────────┤
audit-tax-fiscal ◄─────────────────────┘  (THIS SQUAD)
audit-regulatory-financial ────────────┘ (overlaps)
audit-consolidated ◄─────────────────── (consumes output)
audit-consistency-coherence            (consumes output)
```

### 8.2 Upstream Dependencies

- **audit-financial-model**: Tax basis depends on financial model accuracy
- **audit-security**: Tax endpoints must be secured
- **audit-architecture**: Tax module structure and coupling analysis

### 8.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings
- **audit-consistency-coherence**: Cross-domain alignment
- **audit-regulatory-financial**: May reference tax findings for securities overlap

### 8.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-tax-fiscal | audit-financial-model | Tax: compliance and fiscal docs; financial: pricing and precision |
| audit-tax-fiscal | audit-regulatory-financial | Tax: calculation and filing; regulatory: securities, AML/KYC, RBAC |
| audit-tax-fiscal | audit-regulatory-dataprotection | Tax: tax compliance; dataprotection: PII in tax records |

### 8.5 Recommended Execution Order

1. **audit-dependencies** — Tax library versions
2. **audit-security** — Tax endpoint security
3. **audit-financial-model** — Financial model accuracy
4. **audit-tax-fiscal** — THIS SQUAD: tax compliance
5. **audit-regulatory-financial** — Financial regulatory
6. **audit-consolidated** — Aggregate all findings

### 8.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip already reported findings.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in standardized Section 6 format for aggregation.