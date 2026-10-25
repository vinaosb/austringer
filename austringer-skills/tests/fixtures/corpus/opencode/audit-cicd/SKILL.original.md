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

- Build pipeline validation — verify build steps, dependency resolution, and reproducibility
- Deployment safety — validate deployment scripts, environment separation, approval gates, rollback capability
- Artifact integrity — verify container image tags, signing, provenance, and immutability
- GitLab CI YAML validation — check job isolation, runner tags, artifact handling, stage ordering
- GitHub Actions security — check permissions model, secrets handling, action pinning, workflow triggers
- Docker-in-Docker TLS verification — ensure TLS is enabled for Docker socket exposure
- SSH host key verification — verify no TOFU (Trust On First Use) via `ssh-keyscan`
- Container image tag immutability — flag mutable tags (`latest`) in deployment configurations
- Environment file sourcing safety — verify `.env` files are not sourced as shell code
- Security scan configuration — verify Trivy, Snyk, or equivalent scans fail on HIGH/CRITICAL findings
- Secret management in pipelines — verify secrets are not leaked in logs, environment variables, or artifacts
- Pre-commit hook review — validate hook scripts, check for bypass mechanisms
- Jenkins pipeline groovy security — check for script approval bypass, unsafe groovy methods
- Azure Pipelines YAML validation — check for secure file upload, variable group handling
- Container registry access — verify authentication, image pull policies, private registry configuration

### DO NOT USE FOR

- Container runtime security (use `audit-security`)
- Infrastructure as Code validation — Terraform, CloudFormation (use `audit-security`)
- Dependency CVE scanning (use `audit-dependencies`)
- License compliance of third-party packages (use `audit-license`)
- Application code vulnerability scanning (use `audit-security`)
- Performance benchmarking (use `audit-code-quality`)

---

## Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (treat as JavaScript superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` containing `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` or `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` containing `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` containing `actix` | Actix Web |
| Gin | `go.mod` containing `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` containing `xunit`/`nunit`/`MSTest` | .NET test frameworks |
| pytest | `pytest.ini`, `pyproject.toml` with `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` files | Go testing |
| JUnit | `*Test.java` with `@Test` annotations | JUnit |
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
| OTel | `otel-collector-config.yml`, `opentelemetry` in dependencies | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root directory** for all file patterns above.
2. **Identify primary language** by file count (most source files wins).
3. **Identify frameworks** by config file analysis.
4. **Identify test framework** by test directory/config presence.
5. **Identify CI/CD** by workflow file presence.
6. **Identify infrastructure** by Docker/ORM/observability configs.
7. **Select the matching checklist** from Section 4 based on detected technologies.
8. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 .NET/C# CI/CD Checklist

- [ ] **GitLab CI YAML validation**: Verify `.gitlab-ci.yml` schema, stage ordering, job dependencies
- [ ] **Build configuration**: `dotnet build` uses `--configuration Release`, `TreatWarningsAsErrors` enforced
- [ ] **NuGet package sources**: No private feeds without authentication, no HTTP (only HTTPS) feeds
- [ ] **Docker image tags**: No mutable `latest` tags in deployment scripts or compose files
- [ ] **Docker-in-Docker TLS**: If using DinD service, `DOCKER_TLS_CERTDIR` is set (not empty)
- [ ] **SSH host key verification**: No `ssh-keyscan` without strict host key checking
- [ ] **Trivy scan configuration**: Security scan stage present, fails on HIGH/CRITICAL (`allow_failure: false`)
- [ ] **Deploy script security**: No secrets in deploy scripts, proper environment separation
- [ ] **Production config separation**: Production configs not mixed with development configs
- [ ] **Environment file sourcing**: `.env` files loaded safely, not `source .env` as shell code
- [ ] **Test stage**: Tests run with `--filter "Category!=Integration"` for non-Docker stages
- [ ] **Coverage reporting**: Test coverage reported and threshold enforced
- [ ] **Artifact retention**: Build artifacts have defined retention periods
- [ ] **Pipeline trigger conditions**: Only appropriate branches trigger full pipelines
- [ ] **Central package management**: `Directory.Packages.props` exists and NuGet restore uses it
- [ ] **Format check**: `dotnet format --verify-no-changes` runs in CI

### 4.2 Python CI/CD Checklist

- [ ] **pip/uv dependency installation**: `requirements.txt` or `pyproject.toml` with pinned versions
- [ ] **Virtual environment isolation**: CI uses virtual environments, not system Python
- [ ] **Linting stage**: `ruff`, `flake8`, or `pylint` runs before tests
- [ ] **Security scanning**: `bandit` or `safety` check in pipeline
- [ ] **Type checking**: `mypy` or `pyright` runs in CI
- [ ] **Coverage threshold**: `pytest --cov` with minimum coverage enforced
- [ ] **Lock file committed**: `uv.lock` or `poetry.lock` present and committed
- [ ] **No secrets in environment**: `.env.example` exists, `.env` in `.gitignore`

### 4.3 JavaScript/TypeScript CI/CD Checklist

- [ ] **Lock file committed**: `package-lock.json` or `yarn.lock` present and committed
- [ ] **npm audit**: `npm audit` or `yarn audit` runs in CI, fails on high/critical
- [ ] **Linting**: ESLint runs in CI with no warnings allowed
- [ ] **Build verification**: Production build succeeds before deployment
- [ ] **Bundle size check**: Size limits enforced for production bundles
- [ ] **Type checking**: `tsc --noEmit` runs for TypeScript projects
- [ ] **No secrets**: No API keys or tokens in source or CI environment without masking

### 4.4 Go CI/CD Checklist

- [ ] **Go version pinned**: `go.mod` has explicit Go version, no `replace` in production
- [ ] **Vet and lint**: `go vet` and `golangci-lint` run in CI
- [ ] **Race detector**: Tests run with `-race` flag
- [ ] **Static analysis**: `staticcheck` or equivalent configured
- [ ] **Build reproducibility**: `go build` with `-trimpath` and `CGO_ENABLED=0` for containers
- [ ] **Security scanning**: `gosec` or `govulncheck` runs in pipeline

### 4.5 Java CI/CD Checklist

- [ ] **Dependency pinning**: Maven/Gradle with pinned versions, no SNAPSHOT in production
- [ ] **Security scanning**: OWASP Dependency-Check runs in pipeline
- [ ] **Test coverage**: JaCoCo with minimum threshold enforced
- [ ] **Build reproducibility**: `mvn package` or `gradle build` succeeds deterministically
- [ ] **Container image tags**: Immutable tags used for Java base images

### 4.6 Rust CI/CD Checklist

- [ ] **Clippy lint**: `cargo clippy` passes with no warnings
- [ ] **Security audit**: `cargo audit` runs and fails on vulnerabilities
- [ ] **Lock file committed**: `Cargo.lock` committed for binary crates
- [ ] **Cross-compilation**: Build matrix covers target platforms
- [ ] **Release profile**: Optimized builds with `--release` flag

---

## Anti-Patterns

### 5.1 CI/CD-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Docker-in-Docker with TLS disabled | critical | `DOCKER_TLS_CERTDIR: ""` in GitLab CI | Docker socket exposed without TLS, allows container escape | Set `DOCKER_TLS_CERTDIR: "/certs"` and use TLS-secured Docker daemon |
| 2 | Mutable `latest` tag for deployment | high | Using `image:myapp:latest` in production compose | Non-reproducible deployments, cannot rollback | Use specific version tags or SHA256 digests |
| 3 | SSH TOFU via `ssh-keyscan` | high | `ssh-keyscan >> ~/.ssh/known_hosts` without verification | Man-in-the-middle attack during deployment | Use pre-verified known host keys stored in CI variables |
| 4 | `.env` sourced as shell code | high | `source .env` or `. .env` in deploy scripts | Environment files with special characters can execute arbitrary code | Use `export $(grep -v '^#' .env | xargs)` or dedicated env loaders |
| 5 | Security scans with `allow_failure: true` | high | Trivy/Snyk stage set to allow failure | Security vulnerabilities pass silently | Set `allow_failure: false` for security scan stages |
| 6 | Secrets in CI logs | critical | `echo $DEPLOY_KEY` or unmasked variables in scripts | Secrets leaked in CI job logs visible to all developers | Use CI masked variables, never echo secrets, use secret management |
| 7 | Unpinned GitHub Actions | medium | `uses: actions/checkout@v3` (moving tag) | Tag can be moved to malicious commit by maintainer | Pin to SHA: `uses: actions/checkout@ac593985615ec2ede58e132d2e21d2b1cbd6127c` |
| 8 | Overly broad workflow permissions | high | `permissions: write-all` in GitHub Actions | Workflow can modify repository, push tags, release packages | Use minimum required permissions per job |
| 9 | `pull_request_target` with checkout | critical | `pull_request_target` + `actions/checkout` of PR head | Fork can inject malicious code into trusted context | Avoid `pull_request_target` with code checkout, or use hard filters |
| 10 | Missing deploy approval gates | medium | Auto-deploy to production without manual approval | Accidental or malicious deployment to production | Add `when: manual` (GitLab) or `environment: production` with reviewers (GitHub) |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets | critical | API keys, passwords, tokens in source code | Secrets leak via version control, logs, error reports | Use environment variables or secret manager (Vault, OpenBao, AWS Secrets Manager) |
| 2 | Missing artifact signing | high | Build artifacts published without cryptographic signatures | Supply chain attack — tampered artifacts undetectable | Sign artifacts with GPG or Cosign, verify in downstream pipelines |
| 3 | No build cache invalidation | medium | CI caches dependencies indefinitely | Stale dependencies, potential security vulnerabilities | Pin cache keys to lockfile hash for deterministic invalidation |

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
# ❌ BAD: Overly broad permissions — workflow can push code and manage secrets
on: pull_request
permissions: write-all
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

# ✅ GOOD: Minimal permissions — only what the job needs
on: pull_request
permissions:
  contents: read
  pull-requests: write  # Only needed to comment PR results
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@ac593985615ec2ede58e132d2e21d2b1cbd6127c
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/.gitlab-ci.yml",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the vulnerable CI/CD pattern.",
  "recommendation": "Specific fix with YAML/bash example. Reference CI/CD security best practice.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "security|architecture|performance|compliance|maintainability|testing|observability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the problematic configuration fragment. No hedging ("might be a problem").
- **recommendation**: Actionable. Include a config snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns that span multiple files in different languages.
- **category**: Primary category. Use the most specific category.

### 7.3 Summary Report

After all findings, emit a summary:

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
| **critical** | Immediate supply-chain attack vector or production compromise | Docker socket exposed without TLS, secrets in CI logs, `pull_request_target` with checkout |
| **high** | Significant pipeline security risk with clear attack vector | Mutable tags in production, SSH TOFU, security scan bypassed, overly broad permissions |
| **medium** | Degraded pipeline reliability or defense-in-depth gap | Unpinned actions, missing deploy approval gates, missing coverage threshold |
| **low** | Best practice deviation, minor improvement | Missing artifact retention policy, verbose build logs |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in production deployment pipeline | +1 level | All technologies |
| Finding affects secrets or credentials | +1 level | All technologies |
| Finding allows supply-chain attack | +1 level | All technologies |
| Finding is in a public repository | +1 level | Open-source projects |
| Finding has known CVE associated with CI tool | +1 level | Dependency vulnerabilities |
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in documentation/comments | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Missing `TreatWarningsAsErrors` in CI build → medium (quality gate bypass)
- Unpinned NuGet package versions → high (supply-chain risk)
- Docker `latest` tag in production compose → high (non-reproducible)

#### Python
- Missing `bandit` security scan in pipeline → medium (security gap)
- Unpinned `requirements.txt` in production → high (reproducibility)
- `shell=True` in CI scripts → critical (command injection)

#### JavaScript/TypeScript
- Missing `npm audit` in CI → medium (vulnerability blind spot)
- Unpinned GitHub Actions → medium (supply-chain risk)
- `permissions: write-all` → high (excessive access)

#### Go
- Missing `govulncheck` in CI → medium (vulnerability gap)
- Unbounded `go test` without timeout → low (CI resource exhaustion)
- Missing `-race` flag in test stage → medium (data race in production)

#### Java
- SNAPSHOT dependencies in production builds → high (non-reproducible)
- Missing OWASP Dependency-Check → medium (vulnerability blind spot)

#### Rust
- Missing `cargo audit` in CI → medium (vulnerability gap)
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
audit-consolidated ◄────────────────┘  (consumes all other squad outputs)
audit-consistency-coherence            (consumes all other squad outputs)
```

### 9.2 Upstream Dependencies

- **audit-dependencies**: Provides CVE data that may inform CI/CD security scan configuration review
- **audit-security**: May reference CI/CD findings for deployment security context

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-cicd | audit-security | CI/CD focuses on pipeline configuration; security focuses on application code vulnerabilities |
| audit-cicd | audit-dependencies | CI/CD focuses on pipeline security scanning setup; dependencies focuses on CVE/license analysis |
| audit-cicd | audit-observability | CI/CD focuses on pipeline config; observability focuses on runtime telemetry instrumentation |

### 9.5 Recommended Execution Order

For a full audit, run squads in this sequence:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security (this squad)
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-consolidated** — Aggregate all findings
13. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.
