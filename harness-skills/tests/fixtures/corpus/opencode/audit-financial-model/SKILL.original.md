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

- Pricing logic verification — validate pricing formulas against market data, check unit economics
- Tax calculation validation — verify tax rates, brackets, deductions applied correctly
- Margin computation audit — gross/net margins, contribution margins, break-even analysis
- Financial scenario consistency — optimistic/base/pessimistic scenarios use coherent assumptions
- Derived metric formula verification — every calculated metric traces to source data correctly
- Revenue projection validation — growth rates realistic, CAC/LTV ratios sane, churn assumptions
- Unit economics audit — per-customer metrics, per-seat costs, utilization rates
- Cross-currency computation verification — ensure currency conversion before arithmetic operations
- Monetary precision consistency — verify decimal precision across all financial calculations
- Portfolio allocation validation — currency grouping, holding reconciliation, balance tie-breaking
- ISO 4217 currency validation — ensure currency codes are valid, not just length-checked
- .NET FinancialPrecision constants usage — verify consistent precision in all monetary math
- Python financial calculation accuracy — decimal.Decimal usage, floating-point avoidance
- JavaScript monetary computation — BigInt or decimal.js for currency, never native float

### DO NOT USE FOR

- Tax compliance and fiscal document generation (use `audit-tax-fiscal`)
- AI token cost estimation (use `audit-ai-cost`)
- Schema validation of metrics.json structure (use `audit-schema-metrics`)
- Customer acquisition metrics like CAC/LTV tracking (use `audit-acquisition-metrics`)
- Security vulnerabilities in financial endpoints (use `audit-security`)
- General code quality in financial modules (use `audit-code-quality`)

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

### 2.2 Financial Domain Detection

| Signal | Pattern | Domain |
|--------|---------|--------|
| Currency types | `Currency`, `Money`, ` MonetaryAmount`, `Decimal` in financial context | Currency handling |
| Pricing models | `Price`, `Rate`, `Tariff`, `Fee`, `Commission` | Pricing logic |
| Portfolio types | `Holding`, `Position`, `Allocation`, `Balance`, `Snapshot` | Portfolio management |
| Tax structures | `TaxRate`, `TaxBracket`, `Filing`, `Jurisdiction` | Tax computation |
| Invoice/billing | `Invoice`, `Billing`, `AmountOwed`, `AmountPaid`, `LineItem` | Billing |

### 2.3 Discovery Procedure

1. **Scan root directory** for all language signal files.
2. **Identify financial domain** by searching for currency, pricing, portfolio, tax, billing patterns.
3. **Identify frameworks** by config file analysis (EF Core, Django ORM, etc.).
4. **Identify test framework** by test directory/config presence.
5. **Select the matching checklist** from Section 3 based on detected technologies.
6. **If multiple languages detected** (monorepo), run all applicable checklists.

---

## Section 3: Technology-Specific Checklists

### 3.1 .NET/C# Financial Checklist

- [ ] **Cross-currency normalization**: Verify all amount arithmetic operations (add, subtract,
  compare) are preceded by currency conversion to a common base currency. No direct subtraction
  of amounts in different currencies.
- [ ] **Portfolio allocation currency grouping**: Holdings summed by type MUST group by currency
  first. Summing across currencies without grouping produces meaningless totals.
- [ ] **Monetary precision consistency**: All monetary calculations use `decimal` type with
  consistent precision. Verify `FinancialPrecision` constants (or equivalent) are used instead
  of magic numbers like `2` or `4` for decimal places.
- [ ] **Cross-field invariants**: `AmountPaid` vs `AmountOwed` — derived balance (`AmountOwed -
  AmountPaid`) must be computed, not stored independently. Storing both without a computed
  balance field leads to divergence.
- [ ] **Holding reconciliation after transactions**: After buy/sell transactions, verify holding
  quantities reconcile (total quantity = sum of individual holdings + pending transactions).
- [ ] **Latest-balance query tie-breaking**: When querying the latest balance snapshot, use
  deterministic tie-breaking on `SnapshotDate` and `CreatedAt`. Without this, concurrent
  snapshots may return inconsistent results.
- [ ] **ISO 4217 currency validation**: Currency codes validated against ISO 4217 list, not just
  by string length check. A 3-character string is necessary but not sufficient.
- [ ] **Decimal vs double**: No `double` or `float` used for monetary values. All financial fields
  are `decimal`.
- [ ] **Rounding mode consistency**: Midpoint rounding mode specified explicitly
  (`MidpointRounding.AwayFromZero` or `ToEven`). No implicit rounding.
- [ ] **Exchange rate freshness**: Verify exchange rates have timestamps and staleness checks.
  Stale rates produce inaccurate conversions.

### 3.2 Python Financial Checklist

- [ ] **Decimal usage**: All monetary calculations use `decimal.Decimal`, not `float`. Import
  `from decimal import Decimal, ROUND_HALF_UP`.
- [ ] **Precision context**: `decimal.getcontext().prec` set appropriately for financial math
  (typically 28+).
- [ ] **Currency conversion before arithmetic**: Same cross-currency rule — convert first, then
  compute. No `price_usd + price_eur`.
- [ ] **Floating-point avoidance**: No `float` anywhere in financial computation paths. `Decimal`
  for all monetary values.
- [ ] **Pandas financial columns**: If using DataFrames, financial columns typed as
  `Decimal` or `str`, not `float64`.
- [ ] **Rounding consistency**: Consistent rounding strategy (`ROUND_HALF_UP` for most
  financial applications). No implicit Python float rounding.

### 3.3 JavaScript/TypeScript Financial Checklist

- [ ] **Decimal library usage**: No native `number` for currency. Use `decimal.js`, `big.js`,
  `Dinero.js`, or `currency.js`.
- [ ] **Currency conversion gate**: Same as other languages — convert before compute.
- [ ] **JSON serialization precision**: `JSON.stringify` loses precision for large numbers.
  Use string representation for monetary values in JSON.
- [ ] **Integer cents pattern**: If using integer representation, all amounts in smallest
  currency unit (cents, satoshis). No floating-point cents.
- [ ] **Rounding consistency**: Consistent rounding mode across all financial operations.

### 3.4 Cross-Language Financial Checklist

- [ ] **Formula traceability**: Every derived financial metric traces to source data inputs.
  No "magic numbers" in formulas.
- [ ] **Scenario consistency**: Optimistic/base/pessimistic scenarios use coherent assumptions.
  Growth rates, discount rates, and churn assumptions are internally consistent.
- [ ] **Break-even validation**: Break-even point calculation verified against cost structure
  and unit economics.
- [ ] **Margin computation**: Gross margin = (Revenue - COGS) / Revenue. Net margin includes
  all operating expenses. Contribution margin = Revenue - Variable Costs.
- [ ] **Time period alignment**: All financial comparisons use the same time periods.
  Monthly vs annual comparisons normalized.
- [ ] **Edge case handling**: Zero-division guards in all ratio calculations. Null/missing
  values handled in aggregation functions.

---

## Section 4: Anti-Patterns

### 4.1 .NET/C# Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Cross-currency subtraction | critical | Subtracting monetary amounts in different currencies without conversion | Produces meaningless results (e.g., USD 100 - EUR 80) | Convert both to base currency before arithmetic |
| 2 | Summing holdings without currency grouping | critical | Summing portfolio holdings by type across currencies | Total is a mix of USD, EUR, etc. — meaningless | Group by currency, then sum within each currency |
| 3 | Storing AmountPaid and AmountOwed independently | high | Both stored as independent fields without derived balance | Divergence over time — paid can exceed owed silently | Compute balance as derived value: `AmountOwed - AmountPaid` |
| 4 | Using `double` for money | critical | `double` / `float` for monetary fields | Floating-point rounding errors compound in financial math | Use `decimal` everywhere for money |
| 5 | Latest-balance without tie-breaking | high | Querying latest snapshot by date only without secondary sort | Concurrent snapshots on same date return non-deterministic results | Add `CreatedAt` as tie-breaker with deterministic ordering |
| 6 | ISO 4217 validation by length only | medium | `currencyCode.Length == 3` as sole validation | 3-letter non-currency strings pass validation | Validate against full ISO 4217 code list |
| 7 | Magic precision numbers | medium | Hard-coded `2` or `4` for decimal places | Inconsistent rounding across calculations | Use named constants (`FinancialPrecision.Default`) |
| 8 | Missing midpoint rounding mode | medium | `Math.Round(value, 2)` without explicit mode | Default banker's rounding may not match business rules | Always specify `MidpointRounding.AwayFromZero` or appropriate mode |

### 4.2 Python Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Float for money | critical | Using `float` for monetary calculations | Binary floating-point cannot represent 0.01 exactly | `from decimal import Decimal` |
| 2 | Pandas float64 for currency | high | DataFrame columns with `float64` dtype for money | Precision loss in aggregation operations | Use `Decimal` dtype or store as string, convert for computation |
| 3 | Mixed-currency Series operations | critical | `df['amount'].sum()` across multiple currencies | Returns meaningless total | Group by currency first: `df.groupby('currency')['amount'].sum()` |

### 4.3 JavaScript/TypeScript Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Native number for currency | critical | `let price = 19.99` | IEEE 754 double precision loses accuracy | Use `decimal.js`, `big.js`, or integer cents |
| 2 | JSON number for money | high | `{ "amount": 123456.78 }` in JSON | Precision loss on serialization/deserialization | String representation: `{ "amount": "123456.78" }` |

### 4.4 Cross-Language Financial Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Implicit exchange rate | high | Assuming 1:1 conversion between currencies | Hidden financial error in all downstream calculations | Explicit exchange rate with timestamp and source |
| 2 | Scenario assumption drift | medium | Base case uses 5% growth, optimistic uses 15% without justification | Internally inconsistent financial projections | Document assumptions, cross-reference between scenarios |
| 3 | Unguarded division in ratios | high | Computing margin = profit / revenue without zero check | Runtime crash or infinity/NaN propagation | Guard: `if revenue == 0: return 0` or handle explicitly |
| 4 | Stale exchange rates | high | Using cached exchange rates without staleness check | Inaccurate conversions, regulatory risk | Timestamp + max-age validation on exchange rates |

---

## Section 5: Code Examples (Bad/Good Pairs)

### Example 1: Cross-Currency Normalization (.NET/C#)

```csharp
// BAD: Subtracting amounts in different currencies — meaningless result
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
// BAD: Magic numbers for precision, default rounding mode
public decimal ComputeMargin(decimal revenue, decimal cost)
{
    return Math.Round(revenue - cost, 2); // What rounding mode? What precision?
}

// GOOD: Named constants, explicit rounding mode
public static class FinancialPrecision
{
    public const int Default = 2;
    public const int Extended = 4;  // For intermediate calculations
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
// BAD: Summing all holdings without currency grouping
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
// BAD: Non-deterministic — concurrent snapshots may return different results
public async Task<BalanceSnapshot?> GetLatestBalance(Guid accountId)
{
    return await _dbContext.BalanceSnapshots
        .Where(s => s.AccountId == accountId)
        .OrderByDescending(s => s.SnapshotDate)
        .FirstOrDefaultAsync(); // Which one if multiple on same date?
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
    return sum(i["price"] * i["quantity"] for i in items)  # float imprecision

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
  "description": "Cross-currency subtraction without conversion: USD amount subtracted from EUR amount in CalculateGain()",
  "recommendation": "Convert both amounts to base currency using IExchangeRateService before arithmetic",
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
- **category**: Primary category — for financial findings, use `compliance` for regulatory issues,
  `architecture` for design flaws, `maintainability` for precision issues.

### 6.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-financial-model",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 12,
    "bySeverity": {
      "critical": 2,
      "high": 4,
      "medium": 5,
      "low": 1
    },
    "byCategory": {
      "compliance": 4,
      "architecture": 3,
      "maintainability": 3,
      "performance": 2
    },
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
| **critical** | Materially misstated financial position, data loss | Cross-currency arithmetic, float for money, wrong formula in production billing |
| **high** | Significant correctness risk, broken formula chain | Missing currency grouping, stale rates, non-deterministic balance queries |
| **medium** | Degraded accuracy, maintainability risk | Magic precision numbers, missing rounding mode, inconsistent scenarios |
| **low** | Best practice deviation, documentation gaps | Missing comments on formulas, unused precision constants |

### 7.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in billing/payment code | +1 level | All technologies |
| Finding affects portfolio valuation | +1 level | All technologies |
| Finding involves multi-currency operations | +1 level | All technologies |
| Finding is in investor-facing reporting | +1 level | All technologies |
| Finding affects tax calculations | +1 level | All technologies |

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
audit-code-quality ────────────────────┤
audit-financial-model ◄────────────────┘  (THIS SQUAD)
audit-tax-fiscal ◄────────────────────── (overlaps with this squad)
audit-acquisition-metrics ──────────────┘ (overlaps with this squad)
audit-consolidated ◄─────────────────── (consumes this squad's output)
audit-consistency-coherence            (consumes this squad's output)
```

### 8.2 Upstream Dependencies

- **audit-security**: May reference findings for exposed financial data or calculation endpoints
- **audit-architecture**: May reference findings for financial module coupling and layer violations
- **audit-dependencies**: May reference findings for vulnerable financial libraries

### 8.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings
- **audit-tax-fiscal**: May reference financial-model findings for tax basis calculations

### 8.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-financial-model | audit-tax-fiscal | Financial-model focuses on pricing/logic/precision; tax-fiscal focuses on tax compliance and fiscal documents |
| audit-financial-model | audit-acquisition-metrics | Financial-model focuses on formula accuracy; acquisition-metrics focuses on CAC/LTV tracking |
| audit-financial-model | audit-ai-cost | Financial-model focuses on financial calculations; ai-cost focuses on token/LLM costs |

### 8.5 Recommended Execution Order

1. **audit-dependencies** — Financial library versions
2. **audit-security** — Financial endpoint security
3. **audit-architecture** — Financial module structure
4. **audit-financial-model** — THIS SQUAD: formula accuracy and precision
5. **audit-tax-fiscal** — Tax compliance (uses financial-model output)
6. **audit-consolidated** — Aggregate all findings

### 8.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 6 format for aggregation.
