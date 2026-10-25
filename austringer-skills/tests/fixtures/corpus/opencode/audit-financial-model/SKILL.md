---
name: audit-financial-model
description: >-
  Financial model accuracy auditing: pricing logic verification, margin computation audit,
  cross-currency normalization, monetary precision consistency, revenue projection validation,
  and derived metric formula verification across financial codebases.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Financial Model Accuracy Audit

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- Pricing logic verification — validate formulas against market data, check unit economics
- Tax calculation validation — verify tax rates, brackets, deductions applied correctly
- Margin computation audit — gross/net margins, contribution margins, break-even analysis
- Financial scenario consistency — optimistic/base/pessimistic use coherent assumptions
- Derived metric formula verification — every calculated metric traces to source data
- Revenue projection validation — growth rates realistic, CAC/LTV ratios sane, churn assumptions
- Unit economics audit — per-customer metrics, per-seat costs, utilization rates
- Cross-currency computation verification — convert before arithmetic
- Monetary precision consistency — verify decimal precision across all financial calculations
- Portfolio allocation validation — currency grouping, holding reconciliation, balance tie-breaking
- ISO 4217 currency validation — currency codes valid, not just length-checked
- .NET FinancialPrecision constants usage — consistent precision in monetary math
- Python financial calculation accuracy — decimal.Decimal usage, no float
- JavaScript monetary computation — BigInt or decimal.js for currency, never native float

### DO NOT USE FOR

- Tax compliance and fiscal document generation (use `audit-tax-fiscal`)
- AI token cost estimation (use `audit-ai-cost`)
- Schema validation of metrics.json structure (use `audit-schema-metrics`)
- Customer acquisition metrics like CAC/LTV (use `audit-acquisition-metrics`)
- Security vulnerabilities in financial endpoints (use `audit-security`)
- General code quality in financial modules (use `audit-code-quality`)

---

## Section 2: Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

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

### 2.2 Financial Domain Detection

| Signal | Pattern | Domain |
|--------|---------|--------|
| Currency types | `Currency`, `Money`, `MonetaryAmount`, `Decimal` in financial context | Currency handling |
| Pricing models | `Price`, `Rate`, `Tariff`, `Fee`, `Commission` | Pricing logic |
| Portfolio types | `Holding`, `Position`, `Allocation`, `Balance`, `Snapshot` | Portfolio management |
| Tax structures | `TaxRate`, `TaxBracket`, `Filing`, `Jurisdiction` | Tax computation |
| Invoice/billing | `Invoice`, `Billing`, `AmountOwed`, `AmountPaid`, `LineItem` | Billing |

### 2.3 Discovery Procedure

1. **Scan root directory** for language signal files.
2. **Identify financial domain** — search for currency, pricing, portfolio, tax, billing patterns.
3. **Identify frameworks** by config file analysis.
4. **Identify test framework** by test directory/config presence.
5. **Select matching checklist** from Section 3 based on technologies.
6. **If multiple languages** (monorepo), run all applicable checklists.

---

## Section 3: Technology-Specific Checklists

### 3.1 .NET/C# Financial Checklist

- [ ] **Cross-currency normalization**: Amount arithmetic (add, subtract, compare) preceded by currency conversion to common base. No direct subtraction of different currencies.
- [ ] **Portfolio allocation currency grouping**: Holdings summed by type MUST group by currency first. Summing across currencies without grouping produces meaningless totals.
- [ ] **Monetary precision consistency**: All monetary calculations use `decimal` with consistent precision. `FinancialPrecision` constants used instead of magic numbers.
- [ ] **Cross-field invariants**: `AmountPaid` vs `AmountOwed` — derived balance (`AmountOwed - AmountPaid`) computed, not stored independently.
- [ ] **Holding reconciliation after transactions**: After buy/sell, holding quantities reconcile (total qty = sum of individual + pending).
- [ ] **Latest-balance query tie-breaking**: Deterministic tie-breaking on `SnapshotDate` + `CreatedAt`. No non-deterministic results.
- [ ] **ISO 4217 currency validation**: Validate against full ISO 4217 list, not just length check.
- [ ] **Decimal vs double**: No `double` or `float` for monetary values. All financial fields `decimal`.
- [ ] **Rounding mode consistency**: Midpoint rounding specified explicitly (`AwayFromZero` or `ToEven`). No implicit rounding.
- [ ] **Exchange rate freshness**: Rates have timestamps and staleness checks. Stale rates produce inaccurate conversions.

### 3.2 Python Financial Checklist

- [ ] **Decimal usage**: All monetary calculations use `decimal.Decimal`, not `float`. Import `from decimal import Decimal, ROUND_HALF_UP`.
- [ ] **Precision context**: `decimal.getcontext().prec` set appropriately (typically 28+).
- [ ] **Currency conversion before arithmetic**: Convert first, then compute. No `price_usd + price_eur`.
- [ ] **Floating-point avoidance**: No `float` in financial computation paths.
- [ ] **Pandas financial columns**: If DataFrames, financial columns typed as `Decimal` or `str`, not `float64`.
- [ ] **Rounding consistency**: Consistent strategy (`ROUND_HALF_UP`). No implicit Python float rounding.

### 3.3 JavaScript/TypeScript Financial Checklist

- [ ] **Decimal library usage**: No native `number` for currency. Use `decimal.js`, `big.js`, `Dinero.js`, or `currency.js`.
- [ ] **Currency conversion gate**: Convert before compute.
- [ ] **JSON serialization precision**: `JSON.stringify` loses precision for large numbers. Use string representation for monetary values.
- [ ] **Integer cents pattern**: If integers, all amounts in smallest unit (cents, satoshis). No floating-point cents.
- [ ] **Rounding consistency**: Consistent rounding mode across all operations.

### 3.4 Cross-Language Financial Checklist

- [ ] **Formula traceability**: Every derived metric traces to source data. No magic numbers.
- [ ] **Scenario consistency**: Optimistic/base/pessimistic assumptions coherent internally. Growth, discount, churn rates consistent.
- [ ] **Break-even validation**: Break-even verified against cost structure and unit economics.
- [ ] **Margin computation**: Gross = (Revenue - COGS) / Revenue. Net includes all operating expenses. Contribution = Revenue - Variable Costs.
- [ ] **Time period alignment**: All comparisons use same time periods. Monthly vs annual normalized.
- [ ] **Edge case handling**: Zero-division guards in ratios. Null/missing values handled in aggregates.

---

## Section 4: Anti-Patterns

### 4.1 .NET/C# Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Cross-currency subtraction | critical | Subtracting amounts in different currencies without conversion | Meaningless result (USD 100 - EUR 80) | Convert to base currency before arithmetic |
| 2 | Summing holdings without currency grouping | critical | Summing by type across currencies | Mixed USD/EUR/etc. total — meaningless | Group by currency, sum within each |
| 3 | Storing AmountPaid and AmountOwed independently | high | Both stored without derived balance | Divergence over time | Compute balance as derived value |
| 4 | Using `double` for money | critical | `double`/`float` for monetary fields | Floating-point rounding errors compound | Use `decimal` everywhere |
| 5 | Latest-balance without tie-breaking | high | Snapshot query by date only | Non-deterministic on concurrent snapshots | Add CreatedAt as tie-breaker |
| 6 | ISO 4217 validation by length only | medium | `currencyCode.Length == 3` as sole validation | 3-letter non-currency passes | Validate against full ISO 4217 list |
| 7 | Magic precision numbers | medium | Hard-coded `2` or `4` for decimal places | Inconsistent rounding | Use named constants |
| 8 | Missing midpoint rounding mode | medium | `Math.Round(value, 2)` without explicit mode | Default banker's rounding may not match business rules | Always specify MidpointRounding |

### 4.2 Python Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Float for money | critical | `float` for monetary calculations | Cannot represent 0.01 exactly | `from decimal import Decimal` |
| 2 | Pandas float64 for currency | high | df columns float64 for money | Precision loss in aggregation | Use Decimal or string |
| 3 | Mixed-currency Series operations | critical | `df['amount'].sum()` across currencies | Meaningless total | Group by currency first |

### 4.3 JavaScript/TypeScript Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Native number for currency | critical | `let price = 19.99` | IEEE 754 double loses accuracy | Use decimal.js, big.js, or integer cents |
| 2 | JSON number for money | high | `{ "amount": 123456.78 }` in JSON | Precision loss on serialization | String: `{ "amount": "123456.78" }` |

### 4.4 Cross-Language Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Implicit exchange rate | high | Assuming 1:1 conversion between currencies | Hidden error in all downstream calculations | Explicit rate with timestamp and source |
| 2 | Scenario assumption drift | medium | Base 5% growth, optimistic 15% without justification | Incoherent projections | Document assumptions, cross-reference |
| 3 | Unguarded division in ratios | high | margin = profit / revenue without zero check | Runtime crash or NaN | Guard: `if revenue == 0: return 0` |
| 4 | Stale exchange rates | high | Cached rates without staleness check | Inaccurate conversions | Timestamp + max-age validation |

---

## Section 5: Code Examples (Bad/Good Pairs)

### Example 1: Cross-Currency Normalization (.NET/C#)

```csharp
// BAD: Subtracting amounts in different currencies
public decimal CalculateGain(Holding purchase, Holding sale)
{
    return sale.Amount - purchase.Amount; // USD - EUR = ???
}

// GOOD: Convert to base currency before computation
public decimal CalculateGain(Holding purchase, Holding sale, IExchangeRateService rates)
{
    var purchaseBase = rates.ConvertToBase(purchase.Amount, purchase.Currency);
    var saleBase = rates.ConvertToBase(sale.Amount, sale.Currency);
    return Math.Round(saleBase - purchaseBase, FinancialPrecision.Default, MidpointRounding.AwayFromZero);
}
```

### Example 2: FinancialPrecision Constants and Rounding (.NET/C#)

```csharp
// BAD: Magic numbers, default rounding mode
public decimal ComputeMargin(decimal revenue, decimal cost)
{
    return Math.Round(revenue - cost, 2);
}

// GOOD: Named constants, explicit rounding mode
public static class FinancialPrecision
{
    public const int Default = 2;
    public const int Extended = 4;
    public const int ExchangeRate = 6;
}

public decimal ComputeMargin(decimal revenue, decimal cost)
{
    var margin = revenue - cost;
    return Math.Round(margin, FinancialPrecision.Default, MidpointRounding.AwayFromZero);
}
```

### Example 3: Holding Reconciliation with Currency Grouping (.NET/C#)

```csharp
// BAD: Summing without currency grouping
public decimal TotalPortfolioValue(IEnumerable<Holding> holdings)
{
    return holdings.Sum(h => h.Quantity * h.Price); // Mixes USD, EUR, BRL...
}

// GOOD: Group by currency, convert to base, then sum
public decimal TotalPortfolioValue(IEnumerable<Holding> holdings, IExchangeRateService rates)
{
    return holdings
        .GroupBy(h => h.Currency)
        .Sum(g => g.Sum(h => h.Quantity * h.Price)
                   .ConvertToBase(rates, g.Key));
}
```

### Example 4: Latest Balance with Tie-Breaking (.NET/C#)

```csharp
// BAD: Non-deterministic — concurrent snapshots differ
public async Task<BalanceSnapshot?> GetLatestBalance(Guid accountId)
{
    return await _dbContext.BalanceSnapshots
        .Where(s => s.AccountId == accountId)
        .OrderByDescending(s => s.SnapshotDate)
        .FirstOrDefaultAsync();
}

// GOOD: Deterministic tie-breaking with CreatedAt
public async Task<BalanceSnapshot?> GetLatestBalance(Guid accountId)
{
    return await _dbContext.BalanceSnapshots
        .Where(s => s.AccountId == accountId)
        .OrderByDescending(s => s.SnapshotDate)
        .ThenByDescending(s => s.CreatedAt)
        .FirstOrDefaultAsync();
}
```

### Example 5: Decimal Usage in Python

```python
# BAD: float arithmetic for money
def calculate_total(items: list[dict]) -> float:
    return sum(i["price"] * i["quantity"] for i in items)

# GOOD: Decimal for all monetary calculations
from decimal import Decimal, ROUND_HALF_UP

def calculate_total(items: list[dict]) -> Decimal:
    total = sum(Decimal(str(i["price"])) * Decimal(str(i["quantity"])) for i in items)
    return total.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
```

### Example 6: JavaScript Currency Safety

```javascript
// BAD: Native number for currency
function computeTotal(items) {
    return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
    // 0.1 + 0.2 = 0.30000000000000004
}

// GOOD: Decimal.js for monetary computation
import Decimal from "decimal.js";

function computeTotal(items) {
    return items.reduce(
        (sum, item) => sum.plus(new Decimal(item.price).times(item.quantity)),
        new Decimal(0)
    ).toDecimalPlaces(2, Decimal.ROUND_HALF_UP);
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
  "description": "Cross-currency subtraction without conversion in CalculateGain()",
  "recommendation": "Convert both amounts to base currency using IExchangeRateService",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|architecture|performance|maintainability|testing|observability"
}
```

### 6.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 7.
- **file**: Relative path from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Line where finding ends.
- **description**: Factual, specific. Include problematic code fragment. No hedging.
- **recommendation**: Actionable. Include code snippet when possible.
- **technology**: Primary technology.
- **category**: `compliance` for regulatory, `architecture` for design, `maintainability` for precision.

### 6.3 Summary Report

Emit summary after all findings:

```json
{
  "summary": {
    "squad": "audit-financial-model",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 12,
    "bySeverity": { "critical": 2, "high": 4, "medium": 5, "low": 1 },
    "byCategory": { "compliance": 4, "architecture": 3, "maintainability": 3, "performance": 2 },
    "filesScanned": 38,
    "filesWithFindings": 9
  }
}
```

---

## Section 7: Severity Guidelines

### 7.1 Base Severity Definitions

| Level | Definition | Financial Context Example |
|-------|-----------|--------------------------|
| **critical** | Materially misstated financial position, data loss | Cross-currency arithmetic, float for money, wrong billing formula |
| **high** | Significant correctness risk, broken formula chain | Missing currency grouping, stale rates, non-deterministic balance queries |
| **medium** | Degraded accuracy, maintainability risk | Magic precision numbers, missing rounding mode, inconsistent scenarios |
| **low** | Best practice deviation, documentation gaps | Missing comments on formulas, unused precision constants |

### 7.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding in billing/payment code | +1 level | All |
| Finding affects portfolio valuation | +1 level | All |
| Finding involves multi-currency ops | +1 level | All |
| Finding in investor-facing reporting | +1 level | All |
| Finding affects tax calculations | +1 level | All |

### 7.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in test/demo code only | -1 level (min low) | All |
| Existing mitigating control | -1 level (min low) | All |
| Deprecated code | -1 level (min low) | All |

---

## Section 8: Integration Notes

### 8.1 Squad Dependency Graph

```
audit-dependencies ────────────────────┐
audit-security ────────────────────────┤
audit-architecture ────────────────────┤
audit-code-quality ────────────────────┤
audit-financial-model ◄────────────────┘  (THIS SQUAD)
audit-tax-fiscal ◄────────────────────── (overlaps)
audit-acquisition-metrics ──────────────┘ (overlaps)
audit-consolidated ◄─────────────────── (consumes this)
audit-consistency-coherence            (consumes this)
```

### 8.2 Upstream Dependencies

- **audit-security**: May reference exposed financial data/endpoints.
- **audit-architecture**: May reference financial module coupling/violations.
- **audit-dependencies**: May reference vulnerable financial libraries.

### 8.3 Downstream Consumers

- **audit-consolidated**: Aggregates all findings.
- **audit-consistency-coherence**: Validates cross-domain alignment.
- **audit-tax-fiscal**: May reference financial-model findings for tax basis.

### 8.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-financial-model | audit-tax-fiscal | Pricing/logic/precision vs tax compliance/fiscal docs |
| audit-financial-model | audit-acquisition-metrics | Formula accuracy vs CAC/LTV tracking |
| audit-financial-model | audit-ai-cost | Financial calculations vs token/LLM costs |

### 8.5 Recommended Execution Order

1. **audit-dependencies** — Financial library versions
2. **audit-security** — Financial endpoint security
3. **audit-architecture** — Financial module structure
4. **audit-financial-model** — THIS SQUAD
5. **audit-tax-fiscal** — Tax compliance (uses financial-model output)
6. **audit-consolidated** — Aggregate all findings

### 8.6 Output Integration

When squads run sequentially:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip findings already reported.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in standardized Section 6 format.