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

Transversal overlay. Consumes all squad outputs. Validates cross-domain alignment across 3 dimensions: strategic coherence, technical consistency, AI agent cognitive coherence.

---

## USE FOR / DO NOT USE FOR

### USE FOR

- Cross-domain coherence audit: business plan vs code implementation
- Terminology consistency: specs, code, tests (DDD ubiquitous language)
- MADC/HCV debate protocols for critical findings
- Wald stopping criteria for multi-agent debates
- Rumelt's criteria review (diagnosis, guiding policy, coherent actions)
- Domain invariant validation across layers, any language
- Multi-agent debate consistency before final deliverables
- Conflicting terminology across modules in polyglot projects
- Strategy-practice alignment across languages/frameworks

### DO NOT USE FOR

- Replacing other squads (overlay, not substitute — use specific squad)
- Code quality / security (use `audit-code-quality`, `audit-security`)
- Financial model accuracy (use `audit-financial-model`)
- PII scanning (use `audit-privacy-data`)
- Unit test execution (use `audit-test-coverage`)
- Dependency auditing (use `audit-dependencies`)

---

## Target Discovery

Auto-detect. Do NOT hardcode.

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

### Domain Document Detection

| Signal | Pattern | Type |
|--------|---------|------|
| Business plan | `*business-plan*`, `*businessplan*` | Strategic docs |
| Metrics registry | `metrics.json`, `*metrics*.json` | KPI definitions |
| BDD scenarios | `*.feature`, `*.gherkin` | Behavioral specs |
| API contracts | `*.swagger.json`, `openapi.yaml`, `*.proto` | Interface defs |
| Domain glossary | `*glossary*`, `*ubiquitous-language*` | Terminology ref |

### Discovery Procedure

1. Scan root for all language/doc patterns
2. Primary lang by file count
3. Identify domain docs for cross-ref
4. Collect squad outputs from audit dir
5. Build terminology matrix: business → code → test → DB
6. Run all 3 dimensions

---

## Dimension 1 — Business Plan Coherence (Rumelt Criteria)

Ref: Rumelt, *Good Strategy Bad Strategy* (2011). Strategy kernel = diagnosis + guiding policy + coherent actions.

### Inconsistência Estrutural

Mutually exclusive objectives:
- "Premium positioning" + "aggressive cost cutting"
- "Rapid expansion" + "conservative capital allocation"
- Value prop contradicts persona

**Audit:**
1. Extract objectives from business plan
2. Map each to implied resource allocation
3. Flag conflicting resource claims (>30% overlap to diff goals)
4. Verify each traces Rumelt: diagnosis → policy → actions

### Causalidade Financeira

Chain: TAM/SAM/SOM → revenue → pricing covers CAC + ops costs.

**Audit:**
1. Trace TAM/SAM/SOM through revenue model
2. Verify pricing covers: CAC + variable + fixed + margin
3. Test income statement integrity
4. Cross-ref balance sheet with cash flow
5. Flag "hockey stick" assumptions without mechanism

### Capacity-Strategy Fit

Architecture supports value prop.

**Audit:**
1. Extract architecture decisions from code/docs
2. Map each to business capability enabled
3. Flag capabilities claimed but unsupported
4. Flag components with no business mapping

### Consonância Externa

Alignment with macro trends, regulatory, competitive.

**Audit:**
1. Market assumptions current (not >2yr stale)
2. Check competitive analysis vs `audit-competitive-intel`
3. Validate regulatory vs `audit-regulatory-*` outputs
4. Flag unstable external factors assumed stable

### Viabilidade Operacional

Resource adequacy, timeline vs capital.

**Audit:**
1. Map hiring plan to capital runway
2. Infrastructure scaling matches growth
3. Cross-check milestones with `audit-product-roadmap`
4. Flag timeline deps without slack

### Vantagem Competitiva

Differentiation factors, sustainability.

**Audit:**
1. Extract all claimed advantages
2. Classify: structural / positional / execution-based
3. Sustainability horizon: <1yr / 1-3yr / 3+yr
4. Flag "best practices" as non-differentiating

---

## Dimension 2 — Technical Consistency (DDD)

Ref: Evans, *Domain-Driven Design* (2003); Fowler, *PoEAA*.

### Linguagem Ubíqua

Terminology cross-ref: business plan vs code.

**Audit:**
1. Extract domain terms from business plan (metrics.json keys, headings, persona docs)
2. Extract from: BDD scenarios, backlog rules, class names, vars, DB tables
3. Build matrix: business → code → test → DB
4. Flag dissonance: same concept diff names, diff concept same name
5. Metric registry IDs match code naming conventions

### Invariantes de Domínio

Rules ALWAYS true. Check enforcement layer.

**Audit:**
1. Extract business rules from docs/modules
2. Where enforced: UI / API / domain / DB constraint
3. Flag rules only at UI/API layer (missing domain)
4. Flag rules documented but uncoded
5. Critical invariants have test coverage

### Testes de Fronteira Arquitetural

Domain layer purity.

**Audit:**
1. Scan domain for infra/UI imports
2. Flag DTOs leaking to UI without mapping
3. Verify dep direction: UI → Application → Domain ← Infrastructure
4. Domain entities no framework deps
5. Repository interfaces in domain, impl in infra

---

## Dimension 3 — AI Agent Cognitive Coherence

Ref: MADC, HCV, Wald Sequential Analysis.

### MADC (Multi-Agent Debate Consistency)

N ≥ 3 independent agents with perturbed personas.

**Protocol:**
1. N ≥ 3 instances, varied persona params
2. Each audits same deliverable
3. Compare: consensus vs divergent
4. Consensus → high confidence
5. Divergent → HCV escalation

**Convergence:**
- `ρ = |consensus| / |total|`
- ρ ≥ 0.8: high confidence
- ρ < 0.5: systemic ambiguity

### HCV (Heterogeneous Consensus Verification)

Pair agents from diff LLM providers. Converge → quick. Diverge → expand.

**Protocol:**
1. Agent A (provider 1) + Agent B (provider 2)
2. Independently audit same deliverable
3. **Convergent** (same critical findings): `confidence = high`, stop
4. **Divergent**: expand to 3+ agents, MADC
5. Divergence = finding (genuine ambiguity)

**Escalation:**
- L1: 2 agents (quick pair)
- L2: 3 agents (MADC min)
- L3: 5 agents (full panel)
- L4: Human review

### Wald Stopping Criteria

Debate confidence π_t. Based on SPRT.

**Decision:**
- **π_t ≥ α** (0.85 default): STOP → APPROVE
- **π_t ≤ β** (0.15 default): STOP → ESCALATE
- **β < π_t < α**: CONTINUE (add agent/round)

**Params:**
- `α`: 0.85, tunable per criticality
- `β`: 0.15
- `max_rounds`: 10

**Calc:**
```
π_t = Σ(agent_i.confidence × agent_i.weight) / N
```
Weights = agent specialization relevance.

---

## Anti-Patterns

| # | Anti-Pattern | Sev | Desc | Why Harmful | Fix |
|---|-------------|-----|------|-------------|-----|
| 1 | Conflicting terminology across modules | high | Same concept, different names in diff modules | Breaks ubiquitous language, cross-module confusion | Shared glossary, enforce consistent naming |
| 2 | Strategy-practice misalignment | high | Biz plan objectives with no code/arch support | Strategy = theater, execution drifts | Each objective traces to arch decisions + code |
| 3 | Agent cognitive drift | medium | Multi-agent contradictory findings unresolved | Undermines audit credibility, wastes cycles | MADC/HCV protocols, Wald stopping |
| 4 | Orphaned invariants | high | Business rules documented but uncoded | Rules on paper, violated in prod | Every invariant has code + test coverage |
| 5 | Ghost capabilities | medium | Arch components with no business mapping | Dead code, waste | Map every component to business capability |
| 6 | Inconsistent severity across squads | medium | Same finding rated differently | Undermines consolidated accuracy | Unified severity guidelines |

---

## Code Example — Terminology Matrix Audit

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

## Output Format

Use standard audit finding schema.

### Coherence-Specific Fields

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

### Coherence Summary Report

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

## Integration Notes

### Squad Dependency Graph

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
audit-consistency-coherence ◄───────┘  (consumes ALL)
```

### Dependencies

**Upstream** (consumes ALL):
- `audit-financial-model` → financial causality
- `audit-competitive-intel` → external consonance
- `audit-architecture` → capacity-strategy, boundary tests
- `audit-code-quality` → DDD boundary test inputs
- `audit-test-coverage` → domain invariant coverage
- `audit-consolidated` → pre-aggregated findings
- `audit-regulatory-*` → regulatory consonance

**Downstream**: None (terminal validation layer). Human reviewers sign off.

### Overlaps

| Squad | Overlap | Boundary |
|-------|---------|----------|
| audit-consistency-coherence | audit-architecture | Arch: layer structure; Consistency: cross-layer terminology |
| audit-consistency-coherence | audit-consolidated | Consolidated: aggregates; Consistency: cross-domain alignment |
| audit-consistency-coherence | audit-code-quality | Code-quality: smells; Consistency: ubiquitous language |

### Execution Order

MUST run LAST (after all squads + consolidated):

1. All domain-specific squads first
2. `audit-consolidated` aggregates
3. `audit-consistency-coherence` validates alignment (LAST)

### Checklist

- [ ] Gather outputs from all relevant squads
- [ ] Run Dimension 1 (Rumelt) on business plan docs
- [ ] Run Dimension 2 (DDD) on codebase + docs
- [ ] Run Dimension 3 (MADC/HCV/Wald) for critical findings
- [ ] Produce coherence report with findings, confidence scores, recs
- [ ] Finding with π_t < α → prescribe additional squad investigation
- [ ] Archive debate trace for LMAS audit trail