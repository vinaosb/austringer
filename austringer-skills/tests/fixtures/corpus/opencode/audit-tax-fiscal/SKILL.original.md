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

- Tax calculation verification — validate tax rates applied match jurisdiction requirements
- Fiscal compliance of billing logic — verify billing code handles tax correctly per locale
- Cross-border tax rules — verify international operations account for withholding, VAT/GST
- Sales tax accuracy — verify sales tax calculations, exemptions, thresholds
- Invoice fiscal compliance — mandatory invoice fields, fiscal document numbering
- Tax benefit validation — verify tax incentive claims match eligibility criteria
- Transfer pricing review — flag potential transfer pricing issues in multi-entity setups
- TIN format validation — verify Taxpayer Identification Number validation is correct per country
- GIIN validation — verify Global Intermediary Identification Number patterns
- FATCA/CRS XML generation — verify regulatory reporting XML output correctness
- Extension deadline handling — verify overdue logic uses effective dates (including extensions)
- Multi-jurisdiction tax handling — verify correct tax jurisdiction selection and rate application
- .NET tax computation accuracy — verify ExtensionDeadline ?? FilingDeadline pattern usage
- Python tax calculation audit — verify decimal precision in tax computations
- JavaScript tax module review — verify tax rate lookup and application logic

### DO NOT USE FOR

- General financial model accuracy and pricing logic (use `audit-financial-model`)
- Financial regulatory compliance like securities, RBAC, AML/KYC (use `audit-regulatory-financial`)
- Data protection compliance (use `audit-regulatory-dataprotection`)
- Security vulnerabilities in tax endpoints (use `audit-security`)
- General code quality in tax modules (use `audit-code-quality`)

---

## Section 2: Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

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
| Fiscal documents | `Invoice`, `FiscalDocument`, `NFCe`, `NFSe`, `CTe` | Fiscal documents |
| Identifiers | `TIN`, `GIIN`, `CNPJ`, `CPF`, `EIN`, `VATNumber` | Tax ID validation |
| Reporting | `FATCA`, `CRS`, `XML`, `XSD`, `Schema` | Regulatory reporting |
| Deadlines | `FilingDeadline`, `ExtensionDeadline`, `DueDate`, `Overdue` | Deadline management |
| Jurisdictions | `Jurisdiction`, `Country`, `State`, `Municipality` | Multi-jurisdiction |

### 2.3 Discovery Procedure

1. **Scan root directory** for all language signal files.
2. **Identify tax/fiscal domain** by searching for tax, fiscal, filing, jurisdiction patterns.
3. **Identify frameworks** by config file analysis.
4. **Identify test framework** by test directory/config presence.
5. **Select the matching checklist** from Section 3 based on detected technologies.
6. **If multiple languages detected** (monorepo), run all applicable checklists.

---

## Section 3: Technology-Specific Checklists

### 3.1 .NET/C# Tax/Fiscal Checklist

- [ ] **Extension deadline handling**: Overdue checks MUST use `ExtensionDeadline ?? FilingDeadline`.
  Using only `FilingDeadline` ignores valid extensions, producing false overdue flags.
- [ ] **Multi-jurisdiction tax handling**: Verify tax jurisdiction selection logic handles
  multiple overlapping jurisdictions (federal + state + municipal). No single-rate assumptions.
- [ ] **Fiscal document generation**: Invoice/fiscal document mandatory fields verified:
  taxpayer ID, tax amount, tax rate, itemization, fiscal document number.
- [ ] **TIN format validation**: `IsValidTinFormat` returns `false` for unsupported countries,
  NOT `true`. Returning `true` for unknown countries allows invalid TINs to pass.
- [ ] **GIIN validation patterns**: GIIN follows specific format (16 characters: XXXXXX.XXXXX.XX.XXX).
  Verify regex or parsing matches FATCA specification.
- [ ] **FATCA/CRS XML generation**: XML output validated against published XSD schemas.
  No ad-hoc XML construction without schema validation.
- [ ] **XSD schema validation**: Regulatory XML validated against official XSD. Schema files
  versioned and up-to-date with regulatory authority specifications.
- [ ] **Tax rate lookup accuracy**: Tax rates sourced from configuration/database, not hardcoded.
  Rate effective dates checked against filing period.
- [ ] **Decimal precision for tax amounts**: All tax calculations use `decimal` type with
  appropriate rounding for the jurisdiction.
- [ ] **Tax exemption handling**: Exempt entities and exempt transaction types handled
  explicitly, not by applying zero rate.

### 3.2 Python Tax/Fiscal Checklist

- [ ] **Decimal for tax amounts**: All tax calculations use `decimal.Decimal`, not `float`.
- [ ] **Jurisdiction-specific rounding**: Different jurisdictions require different rounding
  rules (e.g., Brazil rounds differently than EU).
- [ ] **Tax rate configuration**: Rates loaded from external config/database, not hardcoded
  in source files.
- [ ] **Fiscal document numbering**: Sequential numbering with no gaps (regulatory requirement
  in many jurisdictions).
- [ ] **XML generation safety**: Use XML libraries (lxml, xml.etree), not string concatenation.
  Escape special characters in taxpayer data.

### 3.3 JavaScript/TypeScript Tax/Fiscal Checklist

- [ ] **Decimal library for tax**: No native `number` for tax calculations. Use `decimal.js`
  or equivalent.
- [ ] **Tax rate API reliability**: Tax rate lookups have error handling and fallback mechanisms.
- [ ] **Currency/tax coupling**: Tax amount in same currency as transaction. No cross-currency
  tax without conversion.
- [ ] **Fiscal document validation**: Client-side validation matches server-side rules.
  No discrepancies between frontend and backend validation.

### 3.4 Cross-Language Tax/Fiscal Checklist

- [ ] **Extension deadline awareness**: All overdue logic checks for extension grants before
  flagging as overdue.
- [ ] **Multi-jurisdiction stacking**: Tax amounts correctly stacked across jurisdictions
  (e.g., ICMS + PIS/COFINS in Brazil, VAT + withholding in EU).
- [ ] **Withholding tax treatment**: Withholding tax deducted at source where required by
  cross-border rules.
- [ ] **Tax treaty benefits**: Double taxation treaty benefits applied where eligible.
- [ ] **Transfer pricing documentation**: Intercompany transactions documented at arm's length.
- [ ] **Audit trail**: All tax calculations logged with inputs, rates, and outputs for
  regulatory audit trail.
- [ ] **Retroactive rate changes**: Handle retroactive tax rate changes with effective date
  logic, not just current rate.

---

## Section 4: Anti-Patterns

### 4.1 .NET/C# Tax/Fiscal Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `IsValidTinFormat` returns true for unknown countries | critical | Default return `true` for unsupported country codes | Allows invalid TINs to pass validation — regulatory violation | Return `false` for unsupported countries with logged warning |
| 2 | Overdue check ignores extension deadlines | high | `if (DateTime.Now > filing.FilingDeadline)` without extension check | Extensions are legally valid — false overdue flags cause operational disruption | Use `filing.ExtensionDeadline ?? filing.FilingDeadline` |
| 3 | Hardcoded tax rates | high | Tax rates embedded in source code | Rate changes require code deployment; stale rates in production | Externalize to configuration/database with effective dates |
| 4 | Single-jurisdiction assumption | high | One tax rate per transaction | Multi-jurisdiction transactions require tax stacking | Support multiple tax jurisdictions per transaction |
| 5 | XML string concatenation for FATCA/CRS | critical | Building regulatory XML with string concatenation | Malformed XML fails regulatory submission, data corruption | Use XML libraries + XSD schema validation |
| 6 | Missing XSD validation | high | FATCA/CRS XML not validated against official schema | Invalid submissions rejected by regulatory authority | Validate against official XSD before submission |

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
| 2 | Client-only tax validation | high | Tax rules only enforced client-side | Server can be bypassed — regulatory risk | Server-side validation as source of truth |

### 4.4 Cross-Language Tax/Fiscal Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Ignoring extension deadlines | high | Only checking base filing deadline | Extensions are legally binding — false overdue flags | `effectiveDeadline = extension ?? base` |
| 2 | No tax audit trail | medium | Tax calculations not logged with inputs | Cannot reconstruct calculations for regulatory audit | Log: inputs, rates, jurisdiction, outputs, timestamp |
| 3 | Missing retroactive rate handling | medium | Always using current tax rate | Rate changes with effective dates applied incorrectly | Apply rate effective at transaction date, not current date |
| 4 | Withholding tax omission | high | Cross-border payments without withholding | Regulatory violation, potential penalties | Apply withholding tax based on payee jurisdiction and treaty |

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
    return $"<FATCA><Report>{data.TaxId}</Report></FATCA>"; // Malformed, no validation
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
    return amount * taxRate; // Only one jurisdiction
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
    return round(amount * rate, 2)  # float imprecision

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
    return Math.round(amount * rate * 100) / 100; // float imprecision
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

All audit findings MUST be emitted as structured JSON objects. One finding per object.

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

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 7 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the problematic code fragment. No hedging.
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding.
- **category**: For tax findings, use `compliance` for regulatory violations,
  `architecture` for design flaws, `maintainability` for code issues.

### 6.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-tax-fiscal",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 10,
    "bySeverity": {
      "critical": 2,
      "high": 4,
      "medium": 3,
      "low": 1
    },
    "byCategory": {
      "compliance": 5,
      "architecture": 3,
      "maintainability": 2
    },
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
| **critical** | Regulatory violation, incorrect tax in production | TIN validation accepting invalid IDs, wrong tax rate in billing, FATCA XML without schema validation |
| **high** | Significant compliance risk, missing jurisdiction handling | Missing extension deadline logic, hardcoded tax rates, single-jurisdiction assumption |
| **medium** | Degraded compliance posture, edge case gaps | Missing audit trail, no retroactive rate handling, outdated XSD schema |
| **low** | Best practice improvement | Documentation gaps, logging improvements |

### 7.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects production billing/invoicing | +1 level | All technologies |
| Finding involves FATCA/CRS reporting | +1 level | All technologies |
| Finding affects cross-border transactions | +1 level | All technologies |
| Finding involves TIN/GIIN validation | +1 level | All technologies |
| Finding affects multiple jurisdictions | +1 level | All technologies |
| Finding in government submission path | +1 level | All technologies |

### 7.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test/demo code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control | -1 level (min low) | All technologies |
| Finding is in deprecated code | -1 level (min low) | All technologies |

---

## Section 8: Integration Notes

### 8.1 Squad Dependency Graph

```
audit-dependencies ────────────────────┐
audit-security ────────────────────────┤
audit-architecture ────────────────────┤
audit-financial-model ─────────────────┤
audit-tax-fiscal ◄─────────────────────┘  (THIS SQUAD)
audit-regulatory-financial ────────────┘ (overlaps with this squad)
audit-consolidated ◄─────────────────── (consumes this squad's output)
audit-consistency-coherence            (consumes this squad's output)
```

### 8.2 Upstream Dependencies

- **audit-financial-model**: Tax basis calculations depend on financial model accuracy
- **audit-security**: Tax endpoints must be secured against unauthorized access
- **audit-architecture**: Tax module structure and coupling analysis

### 8.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings
- **audit-regulatory-financial**: May reference tax findings for securities compliance overlap

### 8.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-tax-fiscal | audit-financial-model | Tax-fiscal focuses on tax compliance and fiscal documents; financial-model focuses on pricing logic and precision |
| audit-tax-fiscal | audit-regulatory-financial | Tax-fiscal focuses on tax calculation and filing; regulatory-financial focuses on securities, AML/KYC, RBAC |
| audit-tax-fiscal | audit-regulatory-dataprotection | Tax-fiscal focuses on tax compliance; dataprotection focuses on PII in tax records |

### 8.5 Recommended Execution Order

1. **audit-dependencies** — Tax library versions
2. **audit-security** — Tax endpoint security
3. **audit-financial-model** — Financial model accuracy (tax basis)
4. **audit-tax-fiscal** — THIS SQUAD: tax compliance verification
5. **audit-regulatory-financial** — Financial regulatory compliance
6. **audit-consolidated** — Aggregate all findings

### 8.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 6 format for aggregation.
