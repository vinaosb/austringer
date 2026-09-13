---
name: audit-regulatory-sector
description: >-
  Sector-specific regulatory compliance auditing: aviation (ANAC/ICAO), health
  (ANVISA), data center regulations, energy sector (ANEEL), environmental
  licensing, and industry-specific data handling requirements. Validates that
  regulatory references are accurate, sector-specific validations exist, and
  compliance patterns match the applicable regulatory framework.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Audit Squad: Regulatory Sector & Environmental

## Section 2: USE FOR / DO NOT USE FOR

### USE FOR

- Aviation regulation compliance — ANAC RBAC, ICAO standards for airline ops
- Health sector regulation — ANVISA RDC, health ministry decrees for healthcare systems
- Data center env regulations — PUE energy efficiency, cooling permits, noise/emission limits
- Licensing and permitting verification — construction, zoning, operational permits
- Energy sector compliance — ANEEL (Brazil), EU energy directives, tariff structures
- Environmental licensing — IBAMA, state agencies, impact assessments
- Regulatory reference accuracy — verify business docs cite correct regulation versions
- Sector-specific data handling — health data (HIPAA/LGPD health), aviation (ANAC),
  energy (ANEEL resolutions)
- Industry-specific validation — ICAO codes, ANVISA registration numbers, ANEEL tariff codes
- Compliance documentation — regulatory mapping, certification status, audit readiness

### DO NOT USE FOR

- Financial regulation compliance (use `audit-regulatory-financial`)
- Data protection/privacy (use `audit-regulatory-dataprotection`)
- Industry standards certification (use `audit-regulatory-standards`)
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

### 3.3 Sector Detection

| Signal | File Pattern | Detected Sector |
|--------|-------------|-----------------|
| Aviation models | `*Flight*`, `*Aircraft*`, `*Icao*`, `*Anac*`, `*Rbac*` | Aviation |
| Health models | `*Patient*`, `*Anvisa*`, `*Health*`, `*Medical*`, `*Clinical*` | Healthcare |
| Energy models | `*Tariff*`, `*Aneel*`, `*Meter*`, `*Grid*`, `*Energy*` | Energy |
| Data center | `*DataCenter*`, `*Pue*`, `*Cooling*`, `*Rack*` | Data Center |
| Environmental | `*License*`, `*Permit*`, `*Emission*`, `*Ibama*` | Environmental |
| Regulatory refs | `*Regulation*`, `*Compliance*`, `*Resolution*` | Cross-sector |

### 3.4 Discovery

1. Scan root dir for lang/framework signals.
2. Identify sector-specific domains and regulatory references.
3. Check validation patterns matching sector reqs (ICAO, ANVISA IDs, etc.).
4. Verify regulatory reference data completeness.
5. Select checklist from Section 4.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **Sector-specific validation**: ICAO codes validated (4-letter alpha). ANVISA registration numbers
  with check digit algorithm. ANEEL tariff codes match resolution structure. No free-form strings for regulated codes.
- [ ] **Regulatory reference data**: Version numbers current and correct. Resolution/decree numbers match
  official publication. References link to authoritative sources.
- [ ] **Sector-specific data handling**: Health data encrypted with additional protections (LGPD Art. 11).
  Aviation operational data with retention requirements. Energy metering data audit-immutable.
- [ ] **Validation completeness**: All sector entity types have validation rules. No missing validations
  for regulated fields. FluentValidation or equivalent covers all sector reqs.
- [ ] **Regulatory enum correctness**: Sector enums (aircraft types, medical device classes,
  energy tariff categories) match current regulatory definitions. No outdated categories.

### 4.2 Python Checklist

- [ ] **Sector-specific validators**: Custom validators for regulated codes (ICAO, ANVISA, ANEEL).
  Tested against known valid/invalid examples.
- [ ] **Regulatory reference models**: DB models store regulation references with version.
  No stale references in seed data or fixtures.
- [ ] **Data handling compliance**: Health data with enhanced encryption. Sector-specific retention
  policies enforced. Audit logging for regulated data access.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **Sector-specific form validation**: Frontend validation matches backend sector reqs.
  ICAO, ANVISA, ANEEL code formats validated in forms before submission.
- [ ] **Regulatory reference display**: Citations displayed with version and date.
  Links to authoritative sources where applicable.
- [ ] **Data export compliance**: Sector-specific exports include required regulatory metadata.
  No exports missing mandatory fields.

---

## Section 5: Anti-Patterns

### 5.1 Sector Compliance Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Missing sector-specific validations | high | Regulated fields validated as generic strings | Incorrect data enters system, regulatory violation | Sector-specific validators with format/check-digit rules |
| 2 | Stale regulatory references | medium | Business docs cite outdated regulation versions | Compliance with wrong rules, audit finding | Version-controlled reference data with update process |
| 3 | Incomplete regulatory reference data | high | Reference tables missing regulated categories | Cannot classify/process data correctly | Complete seed data covering all current categories |
| 4 | Free-form strings for regulated codes | high | ICAO codes, ANVISA IDs as unvalidated strings | Data quality issues, invalid codes | Strongly-typed value objects with format validation |
| 5 | Missing sector-specific retention | medium | Health/aviation data with generic retention policy | Over/under retention, regulatory non-compliance | Per-sector retention aligned with specific regs |
| 6 | No regulatory audit trail | high | Changes to regulated data not audited | Cannot prove compliance during inspection | Immutable audit entries for all regulated data mutations |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded regulatory constants | medium | Regulation numbers hardcoded across codebase | Updating requires finding all occurrences | Centralized reference with version control |
| 2 | Missing validation tests | medium | Sector validators have no test coverage | Regressions unnoticed | Unit tests per validator with valid/invalid examples |
| 3 | Generic error messages | low | "Invalid input" instead of specific regulation reference | Users cannot correct, compliance unclear | Error messages cite regulation + expected format |

---

## Section 6: Code Examples (Bad/Good Pairs)

### Example 1: Sector-Specific Validation (.NET)

```csharp
// ❌ BAD: Generic string validation for regulated ICAO code
public class FlightDto
{
    [Required]
    [StringLength(4)]
    public string AirportCode { get; set; } = string.Empty; // No format validation
}

// ✅ GOOD: Value object with sector-specific validation
public sealed record IcaoCode
{
    public string Value { get; }
    public IcaoCode(string value)
    {
        if (string.IsNullOrWhiteSpace(value) || value.Length != 4 ||
            !value.All(char.IsLetter))
            throw new ArgumentException(
                "ICAO code must be exactly 4 letters (ICAO Annex 14)", nameof(value));

        Value = value.ToUpperInvariant();
    }
}

public class FlightDto
{
    public IcaoCode DepartureAirport { get; init; }
    public IcaoCode ArrivalAirport { get; init; }
}
```

### Example 2: Regulatory Reference Data (Python)

```python
# ❌ BAD: Stale regulation reference hardcoded
REGULATION_VERSION = "RDC 16/2013"  # Outdated — current is RDC 430/2020

def validate_medical_device(data: dict) -> bool:
    return data.get("risk_class") in ["I", "II", "III", "IV"]

# ✅ GOOD: Version-controlled reference with current data
from datetime import date
from dataclasses import dataclass

@dataclass(frozen=True)
class RegulationReference:
    code: str
    version: str
    publication_date: date
    url: str

CURRENT_ANVISA_RDC = RegulationReference(
    code="RDC 430/2020",
    version="2020",
    publication_date=date(2020, 10, 15),
    url="https://www.in.gov.br/en/web/dou/-/resolucao-rdc-n-430-de-15-de-outubro-de-2020-282094747"
)

def validate_medical_device(data: dict, regulation: RegulationReference = CURRENT_ANVISA_RDC) -> bool:
    valid_classes = {"I", "II", "III", "IV"}
    return data.get("risk_class") in valid_classes
```

### Example 3: Sector-Specific Retention Policy (.NET)

```csharp
// ❌ BAD: Generic retention for all data types
public class RetentionConfig
{
    public int RetentionDays { get; set; } = 2555; // 7 years for everything
}

// ✅ GOOD: Sector-aware retention policies
public class SectorRetentionConfig
{
    private readonly Dictionary<string, SectorRetentionPolicy> _policies = new()
    {
        ["FlightRecord"] = new(365, "ANAC RBAC 121.809 — 12 months"),
        ["MedicalRecord"] = new(20 * 365, "LGPD Art. 11 + CFM Resolution — 20 years"),
        ["EnergyMetering"] = new(5 * 365, "ANEEL Res. 882 — 5 years"),
        ["EnvironmentalLicense"] = new(int.MaxValue, "Permanent — IBAMA requirement"),
    };

    public SectorRetentionPolicy GetPolicy(string entityType) =>
        _policies.GetValueOrDefault(entityType)
        ?? throw new InvalidOperationException(
            $"No sector retention policy defined for {entityType}. " +
            "All regulated entities must have explicit retention.");
}

public record SectorRetentionPolicy(int RetentionDays, string RegulatoryBasis);
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
  "description": "What was found and why it matters. Include the sector regulation reference.",
  "recommendation": "Specific fix with code example and regulatory reference.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|architecture|maintainability",
  "sector": "Aviation|Healthcare|Energy|DataCenter|Environmental|CrossSector"
}
```

### 7.2 Summary Report

```json
{
  "summary": {
    "squad": "audit-regulatory-sector",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 7,
    "bySeverity": { "critical": 0, "high": 3, "medium": 3, "low": 1 },
    "byCategory": { "compliance": 5, "architecture": 1, "maintainability": 1 },
    "sectorCoverage": { "Aviation": 2, "Healthcare": 3, "Energy": 2 },
    "filesScanned": 25,
    "filesWithFindings": 7
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Direct regulatory violation with penalty risk | Health data without required encryption, no audit trail for regulated data |
| **high** | Significant compliance gap | Missing sector-specific validations, stale regulation references, incomplete data |
| **medium** | Degraded compliance posture | Generic retention for regulated data, missing validation tests, hardcoded constants |
| **low** | Best practice deviation | Generic error messages, incomplete regulation display, minor formatting |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects safety-critical system (aviation/health) | +1 level | Aviation, Healthcare |
| Finding involves regulated data handling | +1 level | All sectors |
| Finding affects regulatory reporting | +1 level | All sectors |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in test/mock code | -1 level (min low) | All technologies |
| Finding has existing mitigating control | -1 level (min low) | All technologies |
| Finding in deprecated code | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Adjustments

#### .NET/C#
- Missing value object for regulated code → high (data integrity)
- Stale regulation enum values → medium (compliance accuracy)
- No FluentValidation for sector entity → medium (validation gap)

#### Python
- No sector-specific validator → medium (data quality)
- Stale regulation in seed data → medium (compliance accuracy)
- Generic error handling for regulated ops → low (operational)

#### JavaScript/TypeScript
- Missing form validation for regulated fields → medium (data quality)
- No regulation reference display → low (UX/compliance)

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-regulatory-dataprotection ─────┐
audit-regulatory-financial ──────────┤
audit-regulatory-standards ──────────┤
audit-regulatory-sector ◄────────────┤  (this squad)
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### 9.2 Upstream

- **audit-regulatory-dataprotection**: Data protection — adds sector-specific handling
- **audit-regulatory-standards**: Industry standards — adds sector-specific layers
- **audit-architecture**: Data flow — validates sector-specific reqs

### 9.3 Downstream

- **audit-consolidated**: Aggregates findings
- **audit-consistency-coherence**: Cross-domain validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-regulatory-sector | audit-regulatory-standards | Standards covers ISO/SOC2/PCI-DSS; this squad covers sector-specific regs (ANAC, ANVISA, ANEEL) |
| audit-regulatory-sector | audit-regulatory-dataprotection | Dataprotection covers LGPD/GDPR; this squad covers sector-specific data handling |

### 9.5 Recommended Execution Order

1. **audit-regulatory-dataprotection** — General data protection baseline
2. **audit-regulatory-standards** — Industry standard compliance
3. **audit-regulatory-sector** — Sector-specific compliance (this squad)
4. **audit-consolidated** — Aggregate all findings

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip findings already reported by dataprotection or standards squads.
3. Cross-reference: escalate sector-specific findings confirmed by other squads.
4. Output in standardized Section 7 format.