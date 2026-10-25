---
name: audit-cicd
description: >-
  CI/CD pipeline security auditing: build pipeline validation, deployment safety,
  artifact integrity verification, supply-chain security, and secret management
  across all major CI/CD platforms.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# CI/CD & Pipeline Security Audit

## USE FOR

- Build pipeline validation — build steps, dependency resolution, reproducibility
- Deployment safety — deploy scripts, environment separation, approval gates, rollback
- Artifact integrity — container image tags, signing, provenance, immutability
- GitLab CI YAML validation — job isolation, runner tags, artifact handling, stage ordering
- GitHub Actions security — permissions, secrets, action pinning, workflow triggers
- Docker-in-Docker TLS — ensure TLS for Docker socket exposure
- SSH host key verification — no TOFU via `ssh-keyscan`
- Container image tag immutability — flag mutable tags (`latest`) in deployments
- Environment file sourcing — `.env` files not sourced as shell code
- Security scan config — Trivy/Snyk/equivalent fail on HIGH/CRITICAL
- Secret management — secrets not leaked in logs, env vars, or artifacts
- Pre-commit hook review — validate hooks, check bypass mechanisms
- Jenkins groovy security — script approval bypass, unsafe methods
- Azure Pipelines validation — secure file upload, variable group handling
- Container registry — authentication, image pull policies, private registry

### DO NOT USE FOR

- Container runtime security (use `audit-security`)
- IaC validation — Terraform, CloudFormation (use `audit-security`)
- Dependency CVE scanning (use `audit-dependencies`)
- License compliance (use `audit-license`)
- App code vulnerability scanning (use `audit-security`)
- Performance benchmarking (use `audit-code-quality`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (JS superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` with `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` with `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` or `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` with `actix` | Actix Web |
| Gin | `go.mod` with `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` with `xunit`/`nunit`/`MSTest` | .NET test frameworks |
| pytest | `pytest.ini`, `pyproject.toml` with `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` files | Go testing |
| JUnit | `*Test.java` with `@Test` | JUnit |
| Cargo test | `Cargo.toml` with `[dev-dependencies]` | Rust test |

### 3.4 CI/CD Detection

| Signal | File Pattern | Platform |
|--------|-------------|----------|
| GitLab CI | `.gitlab-ci.yml` | GitLab |
| GitHub Actions | `.github/workflows/*.yml` | GitHub |
| Jenkins | `Jenkinsfile` | Jenkins |
| CircleCI | `.circleci/config.yml` | CircleCI |
| Azure Pipelines | `azure-pipelines.yml`, `.azure-pipelines.yml` | Azure DevOps |

### 3.5 Infrastructure Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml`, `docker-compose*.yml` | Containerization |
| Kubernetes | `*.yaml` with `apiVersion` + `kind` | Orchestration |
| Terraform | `*.tf` | Infrastructure as Code |
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` in deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root** for all file patterns above.
2. **Identify language** by file count.
3. **Identify frameworks** by config analysis.
4. **Identify test framework** by test dir/config presence.
5. **Identify CI/CD** by workflow files.
6. **Identify infrastructure** by Docker/ORM/observability configs.
7. **Select checklist** from Section 4.
8. **Multiple languages** → run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 .NET/C# CI/CD Checklist

- [ ] **GitLab CI YAML validation**: Verify `.gitlab-ci.yml` schema, stage ordering, job deps
- [ ] **Build configuration**: `dotnet build` uses `--configuration Release`, `TreatWarningsAsErrors`
- [ ] **NuGet package sources**: No private feeds without auth, no HTTP (HTTPS only)
- [ ] **Docker image tags**: No mutable `latest` in deploy scripts or compose files
- [ ] **Docker-in-Docker TLS**: DinD service must set `DOCKER_TLS_CERTDIR` (not empty)
- [ ] **SSH host key verification**: No `ssh-keyscan` without strict host key checking
- [ ] **Trivy scan config**: Security scan stage present, fails on HIGH/CRITICAL (`allow_failure: false`)
- [ ] **Deploy script security**: No secrets in deploy scripts, proper env separation
- [ ] **Production config separation**: Prod configs not mixed with dev
- [ ] **Environment file sourcing**: `.env` loaded safely, not `source .env` as shell
- [ ] **Test stage**: Tests with `--filter "Category!=Integration"` for non-Docker stages
- [ ] **Coverage reporting**: Coverage reported, threshold enforced
- [ ] **Artifact retention**: Defined retention periods
- [ ] **Pipeline triggers**: Only appropriate branches trigger full pipelines
- [ ] **Central package management**: `Directory.Packages.props` exists, NuGet restore uses it
- [ ] **Format check**: `dotnet format --verify-no-changes` in CI

### 4.2 Python CI/CD Checklist

- [ ] **pip/uv deps**: `requirements.txt` or `pyproject.toml` with pinned versions
- [ ] **Virtual env isolation**: CI uses virtual envs, not system Python
- [ ] **Linting**: `ruff`, `flake8`, or `pylint` before tests
- [ ] **Security scanning**: `bandit` or `safety` in pipeline
- [ ] **Type checking**: `mypy` or `pyright` in CI
- [ ] **Coverage threshold**: `pytest --cov` with minimum enforced
- [ ] **Lock file committed**: `uv.lock` or `poetry.lock` present
- [ ] **No secrets in env**: `.env.example` exists, `.env` in `.gitignore`

### 4.3 JavaScript/TypeScript CI/CD Checklist

- [ ] **Lock file committed**: `package-lock.json` or `yarn.lock`
- [ ] **npm audit**: `npm audit`/`yarn audit` in CI, fails on high/critical
- [ ] **Linting**: ESLint in CI, no warnings allowed
- [ ] **Build verification**: Prod build succeeds before deploy
- [ ] **Bundle size**: Size limits enforced
- [ ] **Type checking**: `tsc --noEmit` for TypeScript
- [ ] **No secrets**: No API keys/tokens in source or CI without masking

### 4.4 Go CI/CD Checklist

- [ ] **Go version pinned**: Explicit version in `go.mod`, no `replace` in prod
- [ ] **Vet and lint**: `go vet` and `golangci-lint` in CI
- [ ] **Race detector**: Tests with `-race` flag
- [ ] **Static analysis**: `staticcheck` or equivalent
- [ ] **Build reproducibility**: `go build -trimpath` and `CGO_ENABLED=0` for containers
- [ ] **Security scanning**: `gosec` or `govulncheck`

### 4.5 Java CI/CD Checklist

- [ ] **Dependency pinning**: Pinned versions, no SNAPSHOT in prod
- [ ] **Security scanning**: OWASP Dependency-Check in pipeline
- [ ] **Test coverage**: JaCoCo with minimum threshold
- [ ] **Build reproducibility**: `mvn package`/`gradle build` deterministic
- [ ] **Container tags**: Immutable tags for Java base images

### 4.6 Rust CI/CD Checklist

- [ ] **Clippy lint**: `cargo clippy` passes, no warnings
- [ ] **Security audit**: `cargo audit` fails on vulnerabilities
- [ ] **Lock file committed**: `Cargo.lock` for binary crates
- [ ] **Cross-compilation**: Build matrix covers target platforms
- [ ] **Release profile**: Optimized builds with `--release`

---

## Anti-Patterns

### 5.1 CI/CD-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | DinD with TLS disabled | critical | `DOCKER_TLS_CERTDIR: ""` in GitLab CI | Docker socket exposed, container escape | Set `DOCKER_TLS_CERTDIR: "/certs"`, TLS daemon |
| 2 | Mutable `latest` tag | high | `image:myapp:latest` in prod compose | Non-reproducible, cannot rollback | Specific version tags or SHA256 digests |
| 3 | SSH TOFU via `ssh-keyscan` | high | `ssh-keyscan >> known_hosts` without verification | MITM during deployment | Pre-verified host keys in CI variables |
| 4 | `.env` sourced as shell | high | `source .env` or `. .env` in deploy scripts | Special chars execute arbitrary code | `export $(grep -v '^#' .env | xargs)` or env loaders |
| 5 | Security scans `allow_failure: true` | high | Trivy/Snyk stage allows failure | Vulns pass silently | `allow_failure: false` for security stages |
| 6 | Secrets in CI logs | critical | `echo $DEPLOY_KEY` or unmasked vars | Secrets visible to all devs | CI masked variables, never echo secrets |
| 7 | Unpinned GitHub Actions | medium | `uses: actions/checkout@v3` (moving tag) | Tag can move to malicious commit | Pin to SHA: `actions/checkout@ac593985...` |
| 8 | `permissions: write-all` | high | Broad workflow permissions | Can modify repo, push tags, release | Minimum required permissions per job |
| 9 | `pull_request_target` + checkout | critical | PR target with code checkout | Fork injects malicious code into trusted context | Avoid or use hard filters |
| 10 | Missing deploy approval | medium | Auto-deploy to prod without approval | Accidental/malicious prod deployment | `when: manual` (GitLab) or env reviewers (GitHub) |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets | critical | API keys, passwords, tokens in source | Leak via git, logs, errors | Env vars or secret manager (Vault, OpenBao) |
| 2 | Missing artifact signing | high | Artifacts without crypto signatures | Supply chain attack, tampered artifacts | Sign with GPG/Cosign, verify downstream |
| 3 | No cache invalidation | medium | CI caches deps indefinitely | Stale deps, security vulns | Pin cache keys to lockfile hash |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Docker-in-Docker TLS Configuration

```yaml
# ❌ BAD: TLS disabled — Docker socket exposed without encryption
test:
  services:
    - docker:dind
  variables:
    DOCKER_TLS_CERTDIR: ""
    DOCKER_HOST: tcp://docker:2375
  script:
    - docker build -t myapp .

# ✅ GOOD: TLS enabled — secure Docker daemon communication
test:
  services:
    - docker:dind
  variables:
    DOCKER_TLS_CERTDIR: "/certs"
    DOCKER_HOST: tcp://docker:2376
    DOCKER_TLS_VERIFY: 1
    DOCKER_CERT_PATH: "$DOCKER_TLS_CERTDIR/client"
  script:
    - docker build -t myapp:v1.2.3 .
```

#### Example 2: Security Scan Stage Configuration

```yaml
# ❌ BAD: Security scan allowed to fail — vulnerabilities pass silently
trivy_scan:
  stage: security
  image: aquasec/trivy:latest
  script:
    - trivy image --exit-code 0 --severity HIGH,CRITICAL myapp:latest
  allow_failure: true

# ✅ GOOD: Security scan fails pipeline on HIGH/CRITICAL findings
trivy_scan:
  stage: security
  image: aquasec/trivy:0.49.1
  script:
    - trivy image --exit-code 1 --severity HIGH,CRITICAL --ignorefile .trivyignore myapp:v1.2.3
  allow_failure: false
  rules:
    - if: $CI_PIPELINE_SOURCE == "schedule"
      allow_failure: true  # Scheduled scans report but don't block
```

### 6.2 Python Examples

#### Example 1: SSH Host Key Verification

```bash
# ❌ BAD: Trust On First Use — MITM attack possible
mkdir -p ~/.ssh
ssh-keyscan ${DEPLOY_HOST} >> ~/.ssh/known_hosts
ssh deploy@${DEPLOY_HOST} "cd /app && docker compose pull && docker compose up -d"

# ✅ GOOD: Pre-verified host key from CI variable
mkdir -p ~/.ssh
echo "${SSH_KNOWN_HOSTS}" > ~/.ssh/known_hosts
chmod 600 ~/.ssh/known_hosts
ssh deploy@${DEPLOY_HOST} "cd /app && docker compose pull && docker compose up -d"
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: GitHub Actions Permission Scoping

```yaml
# ❌ BAD: Overly broad permissions
on: pull_request
permissions: write-all
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

# ✅ GOOD: Minimal permissions
on: pull_request
permissions:
  contents: read
  pull-requests: write
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@ac593985615ec2ede58e132d2e21d2b1cbd6127c
```

---

## Output Format

All findings as structured JSON. One per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/.gitlab-ci.yml",
  "line": 42,
  "endLine": 45,
  "description": "What was found. Include the vulnerable CI/CD pattern.",
  "recommendation": "Fix with YAML/bash example. Reference CI/CD security practice.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "security|architecture|performance|compliance|maintainability|testing|observability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative path. Forward slashes. Never absolute.
- **line**: 1-based start line.
- **endLine**: Optional end line.
- **description**: Factual, specific. Include problematic config fragment. No hedging.
- **recommendation**: Actionable. Config snippet for fix when possible.
- **technology**: Primary technology. `CrossLanguage` only for multi-file patterns.
- **category**: Most specific category.

### 7.3 Summary Report

After all findings, emit:

```json
{
  "summary": {
    "squad": "audit-cicd",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "GitLab CI"],
    "totalFindings": 15,
    "bySeverity": {
      "critical": 1,
      "high": 3,
      "medium": 7,
      "low": 4
    },
    "byCategory": {
      "security": 12,
      "compliance": 2,
      "maintainability": 1
    },
    "filesScanned": 8,
    "filesWithFindings": 5
  }
}
```

---

## Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Immediate supply-chain attack or production compromise | Docker socket without TLS, secrets in logs, `pull_request_target` + checkout |
| **high** | Significant pipeline risk with clear attack vector | Mutable tags in prod, SSH TOFU, security scan bypass, broad permissions |
| **medium** | Degraded reliability or defense-in-depth gap | Unpinned actions, missing approval gates, missing coverage threshold |
| **low** | Best practice deviation, minor improvement | Missing retention policy, verbose build logs |

### 8.2 Severity Escalation Triggers

Modifiers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Production deployment pipeline | +1 level | All technologies |
| Affects secrets/credentials | +1 level | All technologies |
| Supply-chain attack possible | +1 level | All technologies |
| Public repository | +1 level | Open-source |
| Known CVE in CI tool | +1 level | Dependency vulns |
| Auth/authorization code | +1 level | All technologies |
| Data encryption/key management | +1 level | All technologies |
| Multiple tenants affected | +1 level | Multi-tenant |
| Public-facing endpoint | +1 level | Web apps |
| Financial/billing data | +1 level | Financial systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Non-production/test only | -1 level (min low) | All technologies |
| Mitigating control documented | -1 level (min low) | All technologies |
| Deprecated code for removal | -1 level (min low) | All technologies |
| Documentation/comments only | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Missing `TreatWarningsAsErrors` → medium (quality gate bypass)
- Unpinned NuGet versions → high (supply-chain risk)
- Docker `latest` in prod compose → high (non-reproducible)

#### Python
- Missing `bandit` scan → medium (security gap)
- Unpinned `requirements.txt` → high (reproducibility)
- `shell=True` in CI scripts → critical (command injection)

#### JavaScript/TypeScript
- Missing `npm audit` → medium (vuln blind spot)
- Unpinned Actions → medium (supply-chain)
- `permissions: write-all` → high (excessive access)

#### Go
- Missing `govulncheck` → medium (vuln gap)
- No timeout on `go test` → low (resource exhaustion)
- Missing `-race` flag → medium (data race in prod)

#### Java
- SNAPSHOT deps in prod → high (non-reproducible)
- Missing OWASP Dependency-Check → medium (vuln blind spot)

#### Rust
- Missing `cargo audit` → medium (vuln gap)
- Missing `Cargo.lock` for binary → medium (reproducibility)

---

## Integration Notes

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
audit-cicd ─────────────────────────┤
audit-license ──────────────────────┤
audit-refactoring ──────────────────┤
audit-product-roadmap ──────────────┤
audit-acquisition-metrics ──────────┤
audit-ai-cost ──────────────────────┤
audit-regulatory-* ─────────────────┤
audit-lmas-traceability ────────────┤
audit-schema-metrics ───────────────┤
audit-consolidated ◄────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### 9.2 Upstream Dependencies

- **audit-dependencies**: CVE data informs CI/CD scan config review
- **audit-security**: May reference CI/CD findings for deployment context

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all findings
- **audit-consistency-coherence**: Validates cross-domain alignment

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-cicd | audit-security | CI/CD: pipeline config; security: app code vulns |
| audit-cicd | audit-dependencies | CI/CD: pipeline scanning setup; dependencies: CVE/license analysis |
| audit-cicd | audit-observability | CI/CD: pipeline config; observability: runtime telemetry |

### 9.5 Recommended Execution Order

For full audit:

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security (this squad)
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-consolidated** — Aggregate findings
13. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When running sequentially:
1. Read previous findings from shared output.
2. Deduplicate: skip already-reported findings.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in Section 7 format.
