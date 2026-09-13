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

## USE FOR / DO NOT USE FOR

### USE FOR

- CAC validation — verify CAC matches marketing spend
- LTV analysis — verify LTV assumptions, churn rates, revenue/customer
- Analytics code review — tracking code captures all events
- Marketing metric consistency — cross-check funnel metrics biz plan vs analytics
- Funnel data integrity — awareness → consideration → conversion → retention chain
- Cohort analysis — verify cohort definitions, retention calculations
- Conversion rate validation — verify formulas, time period alignment
- Revenue attribution accuracy — verify models, channel credit assignment
- A/B test statistical validity — sample sizes, confidence intervals, significance
- .NET analytics event tracking — event emission completeness, payload accuracy
- Python analytics pipeline review — ETL/ELT transformation correctness
- JS/TS tracking code — client-side event tracking implementation
- Churn rate computation audit — definition consistency, time window alignment

### DO NOT USE FOR

- Financial model calculations / pricing (use `audit-financial-model`)
- Tax compliance / fiscal docs (use `audit-tax-fiscal`)
- Competitive intelligence / market positioning (use `audit-competitive-intel`)
- Data pipeline reliability (use `audit-fault-tolerance`)
- PII in analytics (use `audit-privacy-data`)

---

## Target Discovery

Auto-detect. No hardcoded names.

### Language Detection

| Signal | File Pattern | Lang |
|--------|-------------|------|
| C# | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JS | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TS | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go | `*.go`, `go.mod` | Go |
| Java | `*.java`, `pom.xml`, `build.gradle` | Java |
| Rust | `*.rs`, `Cargo.toml` | Rust |

### Analytics/Metrics Detection

| Signal | Pattern | Domain |
|--------|---------|--------|
| Analytics SDKs | `analytics`, `segment`, `mixpanel`, `amplitude`, `ga`, `gtag` | Event tracking |
| Metric types | `CAC`, `LTV`, `ARPU`, `MRR`, `ARR`, `Churn`, `Conversion` | Business metrics |
| Funnel | `Funnel`, `Stage`, `Conversion`, `Dropoff`, `Retention` | Funnel analysis |
| Cohort | `Cohort`, `Retention`, `ChurnRate`, `Lifetime` | Cohort analysis |
| Attribution | `Attribution`, `Channel`, `Campaign`, `Source`, `Medium` | Attribution |
| A/B testing | `Experiment`, `Variant`, `Control`, `Treatment`, `Significance` | A/B testing |

### Discovery

1. Scan root for lang signals
2. Identify analytics domain via tracking/metric/funnel patterns
3. Identify analytics SDKs via deps
4. Identify frameworks
5. Select checklist by detected tech
6. If polyglot, run all

---

## Checklists

### .NET/C# Analytics

- [ ] **Event completeness**: All key user actions tracked (page views, clicks, forms, purchases, signups emit events)
- [ ] **Event payload**: All required fields present (userId, timestamp, event name, properties)
- [ ] **CAC**: = Total Marketing Spend / New Customers. Include all channels (paid, organic, sales)
- [ ] **LTV**: = ARPU × Gross Margin × (1 / Churn Rate). Components time-aligned from correct sources
- [ ] **Funnel integrity**: Each stage maps to trackable event. No gaps awareness→consideration→conversion→retention
- [ ] **Conversion rate**: = Conversions / Total Visitors. Same time period + population for both
- [ ] **Cohort definitions**: Consistent across all reports (signup date, first purchase, activation event)
- [ ] **DateTime**: All metrics UTC or consistent TZ. No mixed TZ comparisons

### Python Analytics

- [ ] **ETL correctness**: Transformations preserve accuracy. No silent data loss in joins/filters
- [ ] **Pandas aggregation**: GroupBy correct. No silent NaN propagation
- [ ] **Statistical tests**: Correct tests (chi-square prop, t-test means) with proper assumptions
- [ ] **Churn**: = Lost Customers / Start Period Customers. Time windows aligned
- [ ] **Decimal precision**: Financial metrics use `Decimal`, not `float`

### JS/TS Analytics

- [ ] **Client-side tracking**: SDK init before event emission. No race conditions
- [ ] **Deduplication**: No duplicate events on reload, double-click, SPA navigation
- [ ] **Consent**: No events before user consent (GDPR/LGPD)
- [ ] **Cross-domain tracking**: Identity maintained across subdomains
- [ ] **Error tracking**: JS errors tracked with stack traces + context

### Cross-Language Analytics

- [ ] **Metric definition consistency**: Same name = same calc across all systems
- [ ] **Data freshness**: Pipelines have freshness SLAs. Stale data flagged
- [ ] **Funnel stage alignment**: Biz-defined stages match tracked events. No orphan stages
- [ ] **Attribution model**: Single model used consistently. Not mixing last/first/multi-touch
- [ ] **Sample size**: A/B tests adequate before conclusions. MDE defined

---

## Anti-Patterns

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | Missing events for key actions | high | No events for signup, purchase, cancellation | Can't measure conversion/retention/revenue | Structured events for all key actions |
| 2 | Hardcoded metric formulas | medium | CAC/LTV in multiple places, slight variations | Inconsistent metrics, merge conflicts | Centralized metric service |
| 3 | Mixed TZ time-series | high | DateTime across UTC vs local | Wrong daily metrics, cohort assignment | UTC consistently, convert for display |
| 4 | Funnel data gaps | high | Stages defined but not all tracked | Broken conversion analysis | Map every stage to event |

### Python Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | Silent NaN in aggregations | high | `df.groupby().sum()` ignores NaN | Metrics appear correct but incomplete | `df.fillna(0)` or explicit NaN handling |
| 2 | Float for financial metrics | medium | CAC/LTV as `float64` | Precision loss | `Decimal` for monetary |
| 3 | Wrong statistical test | high | t-test for conversion rates (proportions) | Invalid p-values | Chi-square / Fisher's exact |

### JS/TS Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | Duplicate events on SPA nav | high | Multiple fires per action | Inflated metrics, wrong rates | Idempotency keys |
| 2 | Analytics before consent | critical | Events before consent | GDPR/LGPD violation — legal risk | Gate on consent state |
| 3 | Missing error tracking | medium | JS errors not tracked | Can't measure bug impact | Track via `window.onerror` or error boundary |

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why | Fix |
|---|-------------|-----|------|-----|-----|
| 1 | Metric definition drift | high | Same name, diff calc across systems | Decisions on inconsistent data | Single source of truth for definitions |
| 2 | Incomplete funnel tracking | high | Stages not backed by events | Analytics gaps | Map every stage to event |
| 3 | Attribution model mixing | medium | Last-touch in one, first-touch in another | Incomparable channel performance | Single model per report |
| 4 | Underpowered A/B tests | medium | Conclusions before adequate sample | False +/-, wrong decisions | Define MDE, sample size before start |

---

## Code Examples (Bad/Good Pairs)

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

### Example 2: LTV Calculation (.NET/C#)

```csharp
// BAD: Inconsistent time periods, mixed sources
public decimal CalculateLTV(decimal monthlyRevenue, decimal annualChurn)
{
    return monthlyRevenue * 12 / annualChurn;
}

// GOOD: Aligned periods, explicit components
public decimal CalculateLTV(MetricPeriod period)
{
    var arpu = period.AverageRevenuePerUser;
    var grossMargin = period.GrossMargin;
    var churnRate = period.MonthlyChurnRate;
    if (churnRate == 0) return decimal.MaxValue;
    return arpu * grossMargin * (1m / churnRate);
}
```

### Example 3: Funnel Integrity (Python)

```python
# BAD: Stages not backed by events
FUNNEL_STAGES = ["awareness", "consideration", "conversion", "retention"]

# GOOD: Every stage mapped to event
FUNNEL_EVENT_MAP = {
    "awareness": "page_view",
    "consideration": "product_view",
    "conversion": "purchase_completed",
    "retention": "repeat_purchase",
}

def validate_funnel_integrity(tracked_events: set[str]) -> list[str]:
    missing = []
    for stage, event in FUNNEL_EVENT_MAP.items():
        if event not in tracked_events:
            missing.append(f"Funnel stage '{stage}' has no tracked event '{event}'")
    return missing
```

### Example 4: Consent-Gated Analytics (JS/TS)

```javascript
// BAD: No consent check
import analytics from "analytics-sdk";
analytics.init({ key: "prod-key-123" });
analytics.track("page_view", { page: "/home" });

// GOOD: Gate on consent
import analytics from "analytics-sdk";
let analyticsInitialized = false;
function onConsentGranted() {
    if (!analyticsInitialized) {
        analytics.init({ key: getAnalyticsKey() });
        analyticsInitialized = true;
    }
}
function trackEvent(name, properties) {
    if (!analyticsInitialized) return;
    analytics.track(name, properties);
}
```

### Example 5: A/B Test Validity (Python)

```python
# BAD: No significance test, no sample size
def analyze_ab_test(control_conversions, treatment_conversions):
    if treatment_conversions > control_conversions:
        return "Treatment wins!"

# GOOD: Proper test with sample size validation
from scipy import stats
def analyze_ab_test(control_n, control_conversions, treatment_n, treatment_conversions,
                    min_sample=1000, alpha=0.05):
    if control_n < min_sample or treatment_n < min_sample:
        return {"result": "insufficient_sample", "message": f"Need {min_sample} per group"}
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

## Output Format

JSON objects, one per finding.

### Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "Analytics event not tracked for purchase — conversion rate unmeasurable",
  "recommendation": "Add analytics.track('purchase_completed', { orderId, revenue, currency }) after payment",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|architecture|performance|maintainability|testing|observability"
}
```

### Field Rules

- **severity**: One of `critical|high|medium|low`. See §7 for escalation.
- **file**: Relative path, forward slashes, no absolute.
- **line**: 1-based start.
- **endLine**: Optional multi-line end.
- **description**: Factual, specific. No hedging.
- **recommendation**: Actionable. Code snippet when possible.
- **category**: Analytics: `observability` (gaps), `architecture` (definitions), `compliance` (consent)

### Summary Report

```json
{
  "summary": {
    "squad": "audit-acquisition-metrics",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 8,
    "bySeverity": { "critical": 1, "high": 3, "medium": 3, "low": 1 },
    "byCategory": { "observability": 4, "architecture": 2, "compliance": 2 },
    "filesScanned": 30,
    "filesWithFindings": 7
  }
}
```

---

## Severity

### Base Severity

| Level | Def | Example |
|-------|-----|---------|
| **critical** | Legal/regulatory violation, total data loss | Analytics before consent (GDPR), all events dropped |
| **high** | Materially wrong metrics, broken tracking | Missing purchase events, inconsistent definitions, wrong stats |
| **medium** | Degraded quality, minor gaps | Missing properties, float for financial, underpowered tests |
| **low** | Best practice | Missing docs, verbose event names |

### Escalation

| Trigger | + | Applies |
|---------|---|---------|
| Affects revenue reporting | +1 | All |
| Investor-facing metrics | +1 | All |
| Affects consent/privacy | +1 | All |
| A/B test decisions | +1 | All |
| Billing-related events | +1 | All |

### De-escalation

| Trigger | - | Applies |
|---------|---|---------|
| Test/demo code only | -1 (min low) | All |
| Mitigating control exists | -1 (min low) | All |
| Deprecated code | -1 (min low) | All |

---

## Integration

### Dependency Graph

```
audit-security ────────────────────────┐
audit-architecture ────────────────────┤
audit-code-quality ────────────────────┤
audit-privacy-data ────────────────────┤
audit-financial-model ─────────────────┤
audit-acquisition-metrics ◄────────────┘  (THIS)
audit-competitive-intel ──────────────┘ (overlap)
audit-consolidated ◄─────────────────── (consumes this)
audit-consistency-coherence            (consumes this)
```

### Upstream

- **audit-privacy-data**: Analytics PII compliance
- **audit-financial-model**: CAC/LTV basis
- **audit-architecture**: Module structure

### Downstream

- **audit-consolidated**: Aggregates
- **audit-consistency-coherence**: Cross-domain alignment
- **audit-competitive-intel**: May reference metrics

### Overlaps

| Squad | Overlap With | Boundary |
|-------|-------------|----------|
| audit-acquisition-metrics | audit-financial-model | Metrics: CAC/LTV tracking; Model: pricing formulas |
| audit-acquisition-metrics | audit-competitive-intel | Metrics: own-product; Intel: market positioning |
| audit-acquisition-metrics | audit-privacy-data | Metrics: completeness; Privacy: PII handling |

### Execution Order

1. **audit-privacy-data** — PII in analytics
2. **audit-financial-model** — CAC/LTV basis
3. **audit-acquisition-metrics** — THIS
4. **audit-competitive-intel**
5. **audit-consolidated**

### Output Integration

Sequential squads SHOULD:
1. Read previous findings
2. Deduplicate
3. Cross-reference (escalate multi-squad confirmed)
4. Output per §6 format