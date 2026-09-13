---
name: audit-acquisition-metrics
description: >-
  Acquisition metrics auditing: CAC/LTV data validation, analytics code review,
  marketing metric consistency verification, funnel data integrity checks,
  conversion rate validation, and cohort analysis accuracy across data-driven codebases.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Acquisition Metrics Audit

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- CAC (Customer Acquisition Cost) data validation — verify CAC calculations match marketing spend data
- LTV (Lifetime Value) analysis — verify LTV assumptions, churn rates, revenue per customer
- Analytics code review — verify tracking code captures all required events
- Marketing metric consistency — cross-check funnel metrics between business plan and analytics
- Funnel data integrity — awareness → consideration → conversion → retention data chain
- Cohort analysis validation — verify cohort definitions and retention calculations
- Conversion rate validation — verify conversion rate formulas and time period alignment
- Revenue attribution accuracy — verify attribution models and channel credit assignment
- A/B test statistical validity — verify sample sizes, confidence intervals, significance tests
- .NET analytics event tracking — verify event emission completeness and payload accuracy
- Python analytics pipeline review — verify data transformation correctness in ETL/ELT
- JavaScript/TypeScript tracking code — verify client-side event tracking implementation
- Churn rate computation audit — verify churn definition consistency and time window alignment

### DO NOT USE FOR

- Financial model calculations and pricing logic (use `audit-financial-model`)
- Tax compliance and fiscal document verification (use `audit-tax-fiscal`)
- Competitive intelligence and market positioning (use `audit-competitive-intel`)
- Data pipeline infrastructure reliability (use `audit-fault-tolerance`)
- PII handling in analytics data (use `audit-privacy-data`)

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

### 2.2 Analytics/Metrics Domain Detection

| Signal | Pattern | Domain |
|--------|---------|--------|
| Analytics SDKs | `analytics`, `segment`, `mixpanel`, `amplitude`, `ga`, `gtag` | Event tracking |
| Metric types | `CAC`, `LTV`, `ARPU`, `MRR`, `ARR`, `Churn`, `Conversion` | Business metrics |
| Funnel patterns | `Funnel`, `Stage`, `Conversion`, `Dropoff`, `Retention` | Funnel analysis |
| Cohort patterns | `Cohort`, `Retention`, `ChurnRate`, `Lifetime` | Cohort analysis |
| Attribution | `Attribution`, `Channel`, `Campaign`, `Source`, `Medium` | Attribution |
| A/B testing | `Experiment`, `Variant`, `Control`, `Treatment`, `Significance` | A/B testing |

### 2.3 Discovery Procedure

1. **Scan root directory** for all language signal files.
2. **Identify analytics/metrics domain** by searching for tracking, metric, funnel patterns.
3. **Identify analytics SDKs** by dependency/package analysis.
4. **Identify frameworks** by config file analysis.
5. **Select the matching checklist** from Section 3 based on detected technologies.
6. **If multiple languages detected** (monorepo), run all applicable checklists.

---

## Section 3: Technology-Specific Checklists

### 3.1 .NET/C# Analytics Checklist

- [ ] **Analytics event completeness**: All key user actions tracked with analytics events.
  Page views, button clicks, form submissions, purchases, signups all emit events.
- [ ] **Event payload accuracy**: Event properties include all required fields (userId,
  timestamp, event name, properties). No missing required fields.
- [ ] **CAC calculation verification**: CAC = Total Marketing Spend / New Customers Acquired.
  Verify marketing spend includes all channels (paid, organic effort, sales team cost).
- [ ] **LTV computation audit**: LTV = ARPU × Gross Margin × (1 / Churn Rate). Verify
  each component sourced from correct data and time-aligned.
- [ ] **Funnel data integrity**: Each funnel stage maps to a trackable event. No gaps in
  the awareness → consideration → conversion → retention chain.
- [ ] **Conversion rate validation**: Conversion rate = Conversions / Total Visitors. Verify
  denominator and numerator use same time period and same population.
- [ ] **Cohort definition consistency**: Cohort definitions (signup date, first purchase,
  activation event) used consistently across all reports.
- [ ] **DateTime handling in time-series**: All time-based metrics use UTC or consistent
  timezone. No mixed timezone comparisons.

### 3.2 Python Analytics Checklist

- [ ] **ETL transformation correctness**: Verify data transformations in analytics pipelines
  preserve metric accuracy. No silent data loss in joins or filters.
- [ ] **Pandas aggregation accuracy**: GroupBy operations produce correct results. No
  silent NaN propagation in metric calculations.
- [ ] **Statistical test validity**: A/B test significance calculations use correct tests
  (chi-square for proportions, t-test for means) with proper assumptions.
- [ ] **Churn rate computation**: Churn = Lost Customers / Start of Period Customers.
  Verify time window alignment (monthly churn uses monthly data).
- [ ] **Decimal precision for metrics**: Financial metrics (CAC, LTV, ARPU) use `Decimal`,
  not `float`, for monetary values.

### 3.3 JavaScript/TypeScript Analytics Checklist

- [ ] **Client-side tracking implementation**: Analytics SDK initialized before event
  emission. No race conditions with page load.
- [ ] **Event deduplication**: Prevent duplicate event emission on page reload,
  double-click, or SPA navigation.
- [ ] **Consent compliance**: Analytics events not fired before user consent obtained
  (GDPR/LGPD requirement).
- [ ] **Cross-domain tracking**: User identity maintained across subdomains and
  cross-domain navigation.
- [ ] **Error tracking coverage**: JavaScript errors tracked as analytics events
  with stack traces and context.

### 3.4 Cross-Language Analytics Checklist

- [ ] **Metric definition consistency**: Same metric name means same calculation across
  all systems (backend, frontend, data warehouse, BI tool).
- [ ] **Data freshness validation**: Analytics data pipelines have freshness SLAs. Stale
  data flagged or excluded from reports.
- [ ] **Funnel stage alignment**: Business-defined funnel stages match tracked events.
  No orphan stages without corresponding events.
- [ ] **Attribution model consistency**: Single attribution model used consistently.
  Last-touch, first-touch, and multi-touch not mixed in same report.
- [ ] **Sample size sufficiency**: A/B tests run with adequate sample sizes before
  conclusions drawn. Minimum detectable effect defined.

---

## Section 4: Anti-Patterns

### 4.1 .NET/C# Analytics Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Missing event tracking for key actions | high | Analytics events not emitted for signup, purchase, cancellation | Cannot measure conversion, retention, or revenue impact | Track all key user actions with structured event payloads |
| 2 | Hardcoded metric formulas | medium | CAC/LTV formulas in multiple places with slight variations | Inconsistent metrics across reports, merge conflicts | Centralized metric calculation service |
| 3 | Mixed timezone time-series | high | DateTime comparisons across UTC and local time | Inflated/deflated daily metrics, wrong cohort assignment | Use UTC consistently, convert for display only |
| 4 | Funnel data gaps | high | Funnel stages defined but not all tracked with events | Broken conversion analysis, inaccurate dropoff rates | Map every business funnel stage to a tracked event |

### 4.2 Python Analytics Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Silent NaN in aggregations | high | `df.groupby().sum()` silently ignores NaN | Metric totals appear correct but are incomplete | `df.fillna(0)` or explicit NaN handling with logging |
| 2 | Float for financial metrics | medium | CAC/LTV stored as `float64` | Precision loss in aggregation | `Decimal` type for monetary metrics |
| 3 | Incorrect statistical test | high | Using t-test for conversion rates (proportions) | Invalid p-values, wrong conclusions | Chi-square or Fisher's exact test for proportions |

### 4.3 JavaScript/TypeScript Analytics Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Duplicate events on SPA navigation | high | Analytics events fired multiple times per user action | Inflated metrics, wrong conversion rates | Deduplicate events with IDempotency keys |
| 2 | Analytics before consent | critical | Tracking events fired before user consent | GDPR/LGPD violation — legal risk | Gate analytics initialization on consent state |
| 3 | Missing error tracking | medium | JavaScript errors not tracked as analytics events | Cannot measure impact of bugs on conversion | Track errors with `window.onerror` or framework error boundary |

### 4.4 Cross-Language Analytics Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Metric definition drift | high | Same metric name, different calculation across systems | Decisions based on inconsistent data | Single source of truth for metric definitions |
| 2 | Funnel without complete tracking | high | Business defines funnel stages not backed by events | Analytics shows gaps, conversion analysis impossible | Map every stage to a trackable event |
| 3 | Attribution model mixing | medium | Last-touch in one report, first-touch in another | Cannot compare channel performance consistently | Document and enforce single attribution model per report |
| 4 | Underpowered A/B tests | medium | Conclusions drawn before adequate sample size reached | False positives/negatives, wrong decisions | Define MDE and required sample size before starting |

---

## Section 5: Code Examples (Bad/Good Pairs)

### Example 1: Analytics Event Tracking (.NET/C#)

```csharp
// BAD: Missing analytics events for key user actions
public async Task<Result> CompletePurchaseAsync(Order order)
{
    await _orderRepository.SaveAsync(order);
    await _paymentService.ChargeAsync(order.Total);
    // No analytics event emitted — purchase not tracked
    return Result.Success();
}

// GOOD: Track key action with structured event
public async Task<Result> CompletePurchaseAsync(Order order)
{
    await _orderRepository.SaveAsync(order);
    await _paymentService.ChargeAsync(order.Total);

    _analytics.Track(new AnalyticsEvent
    {
        Name = "purchase_completed",
        UserId = order.UserId,
        Properties = new Dictionary<string, object>
        {
            ["order_id"] = order.Id,
            ["revenue"] = order.Total,
            ["currency"] = order.Currency,
            ["items_count"] = order.Items.Count
        }
    });

    return Result.Success();
}
```

### Example 2: LTV Calculation with Proper Components (.NET/C#)

```csharp
// BAD: LTV with inconsistent time periods and data sources
public decimal CalculateLTV(decimal monthlyRevenue, decimal annualChurn)
{
    return monthlyRevenue * 12 / annualChurn; // Monthly revenue / annual churn = ???
}

// GOOD: Aligned time periods, explicit components
public decimal CalculateLTV(MetricPeriod period)
{
    var arpu = period.AverageRevenuePerUser;        // Same period
    var grossMargin = period.GrossMargin;            // Same period
    var churnRate = period.MonthlyChurnRate;          // Monthly, same period
    if (churnRate == 0) return decimal.MaxValue;     // Guard zero division

    return arpu * grossMargin * (1m / churnRate);
}
```

### Example 3: Funnel Data Integrity Check (Python)

```python
# BAD: Funnel stages not backed by tracked events
FUNNEL_STAGES = ["awareness", "consideration", "conversion", "retention"]
# No mapping to actual events — gap between business definition and implementation

# GOOD: Every stage mapped to a trackable event
FUNNEL_EVENT_MAP = {
    "awareness": "page_view",
    "consideration": "product_view",
    "conversion": "purchase_completed",
    "retention": "repeat_purchase",
}

def validate_funnel_integrity(tracked_events: set[str]) -> list[str]:
    """Verify all funnel stages have corresponding tracked events."""
    missing = []
    for stage, event in FUNNEL_EVENT_MAP.items():
        if event not in tracked_events:
            missing.append(f"Funnel stage '{stage}' has no tracked event '{event}'")
    return missing
```

### Example 4: Consent-Gated Analytics (JavaScript/TypeScript)

```javascript
// BAD: Analytics initialized without consent check
import analytics from "analytics-sdk";
analytics.init({ key: "prod-key-123" });       // Fires immediately — GDPR violation
analytics.track("page_view", { page: "/home" }); // Tracked before consent

// GOOD: Gate analytics on user consent
import analytics from "analytics-sdk";

let analyticsInitialized = false;

function onConsentGranted() {
    if (!analyticsInitialized) {
        analytics.init({ key: getAnalyticsKey() });
        analyticsInitialized = true;
    }
}

function trackEvent(name, properties) {
    if (!analyticsInitialized) return; // No tracking before consent
    analytics.track(name, properties);
}
```

### Example 5: A/B Test Statistical Validity (Python)

```python
# BAD: Drawing conclusions from underpowered test
def analyze_ab_test(control_conversions, treatment_conversions):
    if treatment_conversions > control_conversions:
        return "Treatment wins!"  # No significance test, no sample size check

# GOOD: Proper statistical test with sample size validation
from scipy import stats

def analyze_ab_test(control_n, control_conversions, treatment_n, treatment_conversions,
                    min_sample=1000, alpha=0.05):
    if control_n < min_sample or treatment_n < min_sample:
        return {"result": "insufficient_sample", "message": f"Need {min_sample} per group"}

    # Chi-square test for proportions
    contingency = [[control_conversions, control_n - control_conversions],
                    [treatment_conversions, treatment_n - treatment_conversions]]
    chi2, p_value, _, _ = stats.chi2_contingency(contingency)

    return {
        "result": "significant" if p_value < alpha else "not_significant",
        "p_value": p_value,
        "control_rate": control_conversions / control_n,
        "treatment_rate": treatment_conversions / treatment_n,
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
  "description": "Analytics event not tracked for purchase completion — conversion rate cannot be measured",
  "recommendation": "Add analytics.track('purchase_completed', { orderId, revenue, currency }) after payment confirmation",
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
- **category**: For analytics findings, use `observability` for tracking gaps,
  `architecture` for metric definition issues, `compliance` for consent violations.

### 6.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-acquisition-metrics",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 8,
    "bySeverity": {
      "critical": 1,
      "high": 3,
      "medium": 3,
      "low": 1
    },
    "byCategory": {
      "observability": 4,
      "architecture": 2,
      "compliance": 2
    },
    "filesScanned": 30,
    "filesWithFindings": 7
  }
}
```

---

## Section 7: Severity Guidelines

### 7.1 Base Severity Definitions

| Level | Definition | Analytics Context Example |
|-------|-----------|--------------------------|
| **critical** | Legal/regulatory violation, complete data loss | Analytics tracking before consent (GDPR), all events silently dropped |
| **high** | Materially incorrect metrics, broken tracking | Missing purchase/signup events, inconsistent metric definitions, wrong statistical tests |
| **medium** | Degraded analytics quality, minor gaps | Missing event properties, float for financial metrics, underpowered tests |
| **low** | Best practice improvement | Missing event documentation, verbose event names |

### 7.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding affects revenue reporting | +1 level | All technologies |
| Finding involves investor-facing metrics | +1 level | All technologies |
| Finding affects user consent/privacy | +1 level | All technologies |
| Finding impacts A/B test decisions | +1 level | All technologies |
| Finding affects billing-related events | +1 level | All technologies |

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
audit-security ────────────────────────┐
audit-architecture ────────────────────┤
audit-code-quality ────────────────────┤
audit-privacy-data ────────────────────┤
audit-financial-model ─────────────────┤
audit-acquisition-metrics ◄────────────┘  (THIS SQUAD)
audit-competitive-intel ──────────────┘ (overlaps with this squad)
audit-consolidated ◄─────────────────── (consumes this squad's output)
audit-consistency-coherence            (consumes this squad's output)
```

### 8.2 Upstream Dependencies

- **audit-privacy-data**: Analytics tracking must comply with PII handling rules
- **audit-financial-model**: CAC/LTV financial calculations depend on financial model accuracy
- **audit-architecture**: Analytics module structure and integration patterns

### 8.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings
- **audit-competitive-intel**: May reference acquisition metrics for competitive analysis

### 8.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-acquisition-metrics | audit-financial-model | Metrics focuses on CAC/LTV tracking and funnel data; financial-model focuses on pricing formulas and precision |
| audit-acquisition-metrics | audit-competitive-intel | Metrics focuses on own-product analytics; competitive-intel focuses on market positioning |
| audit-acquisition-metrics | audit-privacy-data | Metrics focuses on tracking completeness; privacy-data focuses on PII handling in analytics data |

### 8.5 Recommended Execution Order

1. **audit-privacy-data** — PII handling in analytics data
2. **audit-financial-model** — Financial model accuracy (CAC/LTV basis)
3. **audit-acquisition-metrics** — THIS SQUAD: analytics and metrics audit
4. **audit-competitive-intel** — Competitive positioning
5. **audit-consolidated** — Aggregate all findings

### 8.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 6 format for aggregation.
