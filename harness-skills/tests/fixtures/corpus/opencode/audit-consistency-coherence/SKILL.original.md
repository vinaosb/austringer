---
name: audit-consistency-coherence
description: >-
  Cross-domain consistency and coherence audit: strategic coherence (Rumelt criteria),
  technical consistency (DDD ubiquitous language), and AI agent cognitive coherence
  (MADC/HCV/Wald). Consumes outputs from all other squads and validates alignment.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Consistency & Coherence Squad

Transversal overlay squad. Consumes outputs from all other squads and validates
cross-domain alignment across three orthogonal dimensions: strategic coherence,
technical consistency, and AI agent cognitive coherence.

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- Cross-domain coherence audit between business plan documents and code implementation
- Checking terminology consistency between specs, code, and tests (DDD ubiquitous language)
- Prescribing MADC/HCV debate protocols for critical audit findings
- Wald-based stopping criteria for multi-agent audit debates
- Strategic coherence review using Rumelt's criteria (diagnosis, guiding policy, coherent actions)
- Domain invariant validation across architectural layers in any language
- Multi-agent debate consistency verification before final deliverables
- Detecting conflicting terminology across modules in polyglot projects
- Verifying strategy-practice alignment across detected languages and frameworks

### DO NOT USE FOR

- Replacing other specialized squads (this is an overlay, not a substitute) (use the specific squad)
- Code quality or security scanning (use `audit-code-quality`, `audit-security`)
- Financial model accuracy (use `audit-financial-model`)
- PII scanning (use `audit-privacy-data`)
- Unit test execution (use `audit-test-coverage`)
- Dependency auditing (use `audit-dependencies`)

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

### 2.2 Domain Document Detection

| Signal | File Pattern | Document Type |
|--------|-------------|---------------|
| Business plan | `*business-plan*`, `*businessplan*` | Strategic documents |
| Metrics registry | `metrics.json`, `*metrics*.json` | KPI definitions |
| BDD scenarios | `*.feature`, `*.gherkin` | Behavioral specifications |
| API contracts | `*.swagger.json`, `openapi.yaml`, `*.proto` | Interface definitions |
| Domain glossary | `*glossary*`, `*ubiquitous-language*` | Terminology reference |

### 2.3 Discovery Procedure

1. **Scan root directory** for all language and document patterns above.
2. **Identify primary language** by file count (most source files wins).
3. **Identify domain documents** for cross-referencing with code.
4. **Collect available squad outputs** from the configured audit output directory.
5. **Build terminology matrix**: business terms → code terms → test terms → DB terms.
6. **Run all three dimensions** based on detected materials.

---

## Section 3: Dimension 1 — Business Plan Coherence (Rumelt Criteria)

Reference: Richard Rumelt, *Good Strategy Bad Strategy* (2011). Strategy kernel = diagnosis + guiding policy + coherent actions.

### 3.1 Inconsistência Estrutural

Detect mutually exclusive objectives and policies:
- "Premium positioning" combined with "aggressive cost cutting"
- "Rapid expansion" combined with "conservative capital allocation"
- Value proposition contradicts target persona definition

**Audit Protocol:**
1. Extract all stated objectives from business plan modules
2. Map each objective to its implied resource allocation
3. Flag pairs where resource claims conflict (>30% budget overlap to different goals)
4. Verify each objective traces to Rumelt's kernel: diagnosis → guiding policy → coherent actions

### 3.2 Causalidade Financeira (Financial Causality)

Verify causal chain: market opportunity (TAM/SAM/SOM) → revenue projection → pricing covers CAC + operational costs.

**Audit Protocol:**
1. Trace TAM/SAM/SOM inputs through revenue model
2. Verify pricing model mathematically covers: CAC + variable costs + fixed cost allocation + margin
3. Test income statement integrity: inputs → calculations → outputs
4. Cross-reference balance sheet with cash flow projections
5. Flag "hockey stick" revenue assumptions without supporting causal mechanism

### 3.3 Capacity-Strategy Fit

Verify technical architecture supports the value proposition.

**Audit Protocol:**
1. Extract technical architecture decisions from code/docs
2. Map each architecture decision to the business capability it enables
3. Flag capabilities claimed in business plan with no architectural support
4. Flag architectural components with no corresponding business capability

### 3.4 Consonância Externa (External Consonance)

Alignment with macro trends, regulatory changes, competitive scenarios.

**Audit Protocol:**
1. Verify market assumptions reference current data (not stale >2 years)
2. Check competitive analysis against `audit-competitive-intel` findings
3. Validate regulatory assumptions against `audit-regulatory-*` squad outputs
4. Flag external factors assumed stable that are actively changing

### 3.5 Viabilidade Operacional (Operational Viability)

Resource adequacy, timeline realism vs. capital.

**Audit Protocol:**
1. Map hiring plan timeline to capital runway
2. Verify infrastructure scaling plan matches growth projections
3. Cross-check operational milestones with `audit-product-roadmap`
4. Flag timeline dependencies without slack buffers

### 3.6 Vantagem Competitiva (Competitive Advantage)

Tangible differentiation factors, sustainability analysis.

**Audit Protocol:**
1. Extract all claimed competitive advantages
2. Classify each: structural, positional, execution-based
3. Assess sustainability horizon (<1yr, 1-3yr, 3+yr)
4. Flag advantages that are merely "best practices" (not differentiating)

---

## Section 4: Dimension 2 — Technical Consistency (DDD)

Reference: Eric Evans, *Domain-Driven Design* (2003); Martin Fowler, *Patterns of Enterprise Application Architecture*.

### 4.1 Linguagem Ubíqua (Ubiquitous Language)

Cross-reference terminology between business plan modules and code.

**Audit Protocol:**
1. Extract domain terms from business plan modules (metrics.json keys, module headings, persona docs)
2. Extract corresponding terms from: BDD scenarios, backlog rules, class names, variable names, DB table/column names
3. Build a terminology matrix: business term → code term → test term → DB term
4. Flag dissonance: same concept with different names, or different concepts sharing a name
5. Verify metric registry IDs match code variable naming conventions

### 4.2 Invariantes de Domínio (Domain Invariants)

Domain rules that must ALWAYS be true. Check validation at correct layer.

**Audit Protocol:**
1. Extract all stated business rules from docs/modules
2. For each rule, identify where it is enforced (UI, API, domain, DB constraint)
3. Flag rules enforced only at UI/API layer (not in domain)
4. Flag rules with no enforcement mechanism (only documented, not coded)
5. Verify critical invariants have corresponding test coverage

### 4.3 Testes de Fronteira Arquitetural (Architectural Boundary Tests)

Verify domain layer purity.

**Audit Protocol:**
1. Scan domain layer for imports from infrastructure/UI namespaces
2. Flag commits where DTOs leak to UI layer without mapping
3. Verify dependency direction: UI → Application → Domain ← Infrastructure
4. Check that domain entities have no external framework dependencies
5. Validate repository interfaces defined in domain, implemented in infrastructure

---

## Section 5: Dimension 3 — AI Agent Cognitive Coherence

Reference: Multi-Agent Debate Consistency (MADC), Heterogeneous Consensus Verification (HCV), Wald Sequential Analysis stopping criteria.

### 5.1 MADC (Multi-Agent Debate Consistency)

Run multiple independent agent instances with small persona variations and evaluate path consistency.

**Protocol:**
1. Instantiate N ≥ 3 agent instances with perturbed persona parameters
2. Each instance independently audits the same deliverable
3. Compare findings: consensus items vs. divergent items
4. Consensus items → high confidence findings
5. Divergent items → candidates for HCV escalation

**Convergence Metrics:**
- Agreement ratio: `ρ = |consensus_findings| / |total_findings|`
- If ρ ≥ 0.8: high confidence in audit conclusions
- If ρ < 0.5: systemic ambiguity, likely indicates real inconsistency in the deliverable

### 5.2 HCV (Heterogeneous Consensus Verification)

Start with a pair of agents using different LLM providers. If both converge → quick validation. If divergent → expand debate.

**Protocol:**
1. Pair: Agent A (provider 1) + Agent B (provider 2)
2. Both audit same deliverable independently
3. **If convergent** (same critical findings): `confidence = high`, no further debate needed
4. **If divergent**: expand to 3+ agents, apply MADC protocol
5. Divergence itself is a finding: it signals the deliverable has genuine ambiguity

**Debate Escalation:**
- Level 1: 2 agents (quick pair)
- Level 2: 3 agents (MADC minimum)
- Level 3: 5 agents (full panel)
- Level 4: Escalate to human review

### 5.3 Wald Stopping Criteria

Monitor debate confidence π_t over successive rounds. Based on Wald's Sequential Probability Ratio Test (SPRT).

**Decision Rule:**
- **If π_t ≥ α** (upper threshold, default 0.85): STOP → APPROVE finding
- **If π_t ≤ β** (lower threshold, default 0.15): STOP → ESCALATE to human
- **If β < π_t < α**: CONTINUE debate → add another agent or round

**Parameters:**
- `α` (approval threshold): 0.85 by default, tunable per audit criticality
- `β` (escalation threshold): 0.15 by default
- `max_rounds`: 10 (safety limit to prevent infinite debate)

**Confidence Calculation:**
```
π_t = weighted_agreement(t) = Σ(agent_i.confidence × agent_i.weight) / N
```
Where weights reflect agent specialization relevance to the finding type.

---

## Section 6: Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Conflicting terminology across modules | high | Same domain concept with different names in different modules | Breaks ubiquitous language, causes confusion in cross-module communication | Establish shared glossary, enforce consistent naming across all layers |
| 2 | Strategy-practice misalignment | high | Business plan objectives with no corresponding code/architecture support | Strategy becomes theater, execution drifts from intent | Every strategic objective traces to architectural decisions and code artifacts |
| 3 | Agent cognitive drift | medium | Multi-agent audit sessions producing contradictory findings without resolution | Undermines audit credibility, wastes review cycles | Apply MADC/HCV protocols, use Wald stopping criteria |
| 4 | Orphaned invariants | high | Business rules documented but not enforced in code | Rules exist on paper only, violated in production | Every documented invariant has code enforcement + test coverage |
| 5 | Ghost capabilities | medium | Architecture components with no business capability mapping | Dead code, unnecessary complexity, wasted maintenance cost | Map every architectural component to business capability |
| 6 | Inconsistent severity across squads | medium | Same finding rated differently by different squads | Undermines consolidated report accuracy | Apply unified severity guidelines with escalation triggers |

---

## Section 7: Code Example — Terminology Matrix Audit

```python
import json
import re
from pathlib import Path
from collections import defaultdict


def build_terminology_matrix(project_root: Path) -> list[dict]:
    """Extract domain terms from multiple sources and flag inconsistencies."""
    matrix: dict[str, list[str]] = defaultdict(list)

    # Extract from Python class names (domain models)
    for py_file in project_root.rglob("domain/**/*.py"):
        for match in re.finditer(r"class (\w+)", py_file.read_text()):
            matrix[match.group(1).lower()].append(
                f"code:{py_file.relative_to(project_root)}"
            )

    # Extract from BDD feature files
    for feature in project_root.rglob("**/*.feature"):
        for line in feature.read_text().splitlines():
            if line.strip().startswith(("Given ", "When ")):
                for word in re.findall(r"\b[A-Z][a-z]+(?:[A-Z][a-z]+)+\b", line):
                    matrix[word.lower()].append(
                        f"bdd:{feature.relative_to(project_root)}"
                    )

    # Flag inconsistencies: same concept, different names
    inconsistencies = []
    for term, sources in matrix.items():
        source_types = set(s.split(":")[0] for s in sources)
        if len(source_types) < 2:
            inconsistencies.append({
                "severity": "medium",
                "file": str(sources[0]),
                "description": f"Term '{term}' only found in {len(source_types)} source type(s)",
                "recommendation": "Verify this term exists in both code and documentation",
                "technology": "CrossLanguage",
                "category": "architecture",
            })

    return inconsistencies
```

---

## Section 8: Output Format

All findings MUST use the standard audit finding schema from the shared template.

### 8.1 Additional Coherence-Specific Fields

```json
{
  "severity": "high",
  "file": "src/Modules/Billing/Models/Invoice.cs",
  "line": 42,
  "description": "Domain term 'Invoice' used in code but 'BillingStatement' used in business plan",
  "recommendation": "Align terminology: rename to 'Invoice' in business plan or update code",
  "technology": "CrossLanguage",
  "category": "architecture",
  "dimension": "ubiquitous-language",
  "confidence": 0.92
}
```

### 8.2 Coherence Summary Report

```json
{
  "summary": {
    "squad": "audit-consistency-coherence",
    "timestamp": "2026-06-04T10:00:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Python"],
    "dimensions": {
      "rumelt_coherence": { "findings": 5, "confidence": 0.78 },
      "ddd_consistency": { "findings": 3, "confidence": 0.91 },
      "agent_cognitive_coherence": {
        "madc_agreement_ratio": 0.85,
        "hcv_rounds": 2,
        "wald_decision": "APPROVE"
      }
    },
    "totalFindings": 8,
    "bySeverity": { "critical": 0, "high": 3, "medium": 4, "low": 1 }
  }
}
```

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

```
audit-security ─────────────────────┐
audit-architecture ─────────────────┤
audit-code-quality ─────────────────┤
audit-dependencies ─────────────────┤
audit-test-coverage ────────────────┤
audit-fault-tolerance ──────────────┤
audit-observability ────────────────┤
audit-privacy-data ─────────────────┤
audit-financial-model ──────────────┤
audit-cicd ─────────────────────────┤
audit-regulatory-* ─────────────────┤
audit-consolidated ◄─────────────────┤
audit-consistency-coherence ◄───────┘  (consumes ALL squad outputs)
```

### 9.2 Upstream Dependencies

This squad **consumes** outputs from ALL other squads:
- `audit-financial-model` → financial causality validation
- `audit-competitive-intel` → external consonance check
- `audit-architecture` → capacity-strategy fit, boundary tests
- `audit-code-quality` → DDD boundary test inputs
- `audit-test-coverage` → domain invariant coverage
- `audit-consolidated` → pre-aggregated findings for cross-domain validation
- All `audit-regulatory-*` squads → regulatory consonance

### 9.3 Downstream Consumers

- No squad consumes this squad's output directly — it is a terminal validation layer
- Human reviewers consume coherence reports for sign-off decisions

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-consistency-coherence | audit-architecture | Architecture checks layer structure; consistency checks cross-layer terminology |
| audit-consistency-coherence | audit-consolidated | Consolidated aggregates findings; consistency validates cross-domain alignment |
| audit-consistency-coherence | audit-code-quality | Code-quality checks implementation smells; consistency checks ubiquitous language |

### 9.5 Recommended Execution Order

This squad MUST run AFTER all other squads (including `audit-consolidated`):

1. All domain-specific squads run first
2. `audit-consolidated` aggregates findings
3. `audit-consistency-coherence` validates cross-domain alignment (LAST)

### 9.6 Execution Checklist

- [ ] Gather outputs from all relevant squads
- [ ] Run Dimension 1 (Rumelt) analysis on business plan documents
- [ ] Run Dimension 2 (DDD) analysis on codebase + docs alignment
- [ ] Run Dimension 3 (MADC/HCV/Wald) protocol for any critical findings
- [ ] Produce coherence report with findings, confidence scores, recommendations
- [ ] If any finding has π_t < α: prescribe additional squad investigation
- [ ] Archive debate trace for audit trail (LMAS traceability integration)
