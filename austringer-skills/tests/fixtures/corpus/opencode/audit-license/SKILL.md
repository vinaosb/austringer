---
name: audit-license
description: >-
  License compliance auditing: open-source license scanning, GPL/restrictive
  license detection, dependency license compatibility verification, IP risk
  assessment, attribution and NOTICE file compliance across multi-language
  codebases.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# License Compliance Audit

## USE FOR

- Open-source license scanning — identify all licenses in dependency tree and vendored code
- GPL/AGPL/LGPL detection — flag viral copyleft requiring source disclosure
- Dependency license compatibility — transitive dep licenses compatible with project license
- IP risk assessment — evaluate IP exposure from license choices
- License header verification — source files have correct SPDX or proprietary headers
- NOTICE/attribution compliance — required attribution present and complete
- Dual-license conflict detection — incompatible dual-licensing terms
- Vendored code licensing — copied/embedded code license compliance
- Commercial license verification — confirm valid entitlements

### .NET/C# Specific

- NuGet package license metadata scanning (`dotnet list package --include-transitive`)
- `Directory.Packages.props` central package license review
- Source generator licensing (Roslyn analyzers, code generators)
- `*.csproj` PackageReference license validation
- Microsoft/dotnet runtime distribution licensing compliance

### Python Specific

- `requirements.txt`, `Pipfile`, `pyproject.toml` dependency license audit
- `pip-licenses` output analysis and compatibility matrix
- Virtual environment vendored package scanning
- Conda channel licensing terms
- Wheel/sdist bundled dependency license verification

### JavaScript/TypeScript Specific

- `package.json` license field scanning across tree
- `npm audit --licenses` / `yarn licenses list` output analysis
- Monorepo workspace license consistency
- Frontend bundle dependency licensing (webpack, vite, rollup plugins)
- CDN-linked script licensing (SRI integrity + license tracking)

### Go Specific

- `go.mod` module license scanning
- `go-licenses` tool output analysis
- Standard library vs third-party module boundary
- CGO linked C library licensing
- Replace directive vendored code licensing

### Java Specific

- Maven `pom.xml` dependency license scanning
- Gradle dependency license reports
- BOM license verification
- JAR manifest license metadata
- Spring Boot starter dependency licensing

## DO NOT USE FOR

- Dependency vulnerability scanning (use `audit-dependencies`)
- Security vulnerabilities (use `audit-security`)
- Privacy compliance (use `audit-privacy-data`)
- Supply chain integrity verification (use `audit-cicd`)
- Code quality issues (use `audit-code-quality`)

---

## Target Discovery

Scan repository for license-related files and metadata.

### Files to Locate

| File Pattern | Purpose |
|---|---|
| `LICENSE*`, `LICENCE*`, `COPYING*` | Project's own license |
| `NOTICE*`, `THIRD-PARTY*`, `ATTRIBUTION*` | Attribution notices |
| `package.json` | npm/yarn license fields |
| `requirements.txt`, `Pipfile`, `pyproject.toml` | Python dependency licenses |
| `*.csproj`, `Directory.Packages.props` | NuGet package licenses |
| `go.mod`, `go.sum` | Go module licenses |
| `pom.xml`, `build.gradle*` | Java dependency licenses |
| `Cargo.toml`, `Cargo.lock` | Rust crate licenses |
| `*.license`, `*.lic` | Embedded license files |
| `THIRD_PARTY_LICENSES*` | Pre-existing license manifests |
| `vendor/**`, `vendored/**`, `third_party/**` | Vendored code requiring audit |
| `spdx.*`, `.sbom.*` | Software Bill of Materials |

### Commands for License Inventory

```bash
# .NET — list all packages with license info
dotnet list package --include-transitive --format json

# Python — generate license report
pip-licenses --format=json --output-file=licenses.json

# Node.js — list all dependency licenses
npx license-checker --json --out licenses.json

# Go — scan module licenses
go run github.com/google/go-licenses@latest report ./...

# Java (Maven) — generate license report
mvn license:aggregate-third-party-report

# Java (Gradle) — generate license report
./gradlew generateLicenseReport
```

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] All NuGet packages have `license` or `licenseUrl` metadata
- [ ] No GPL/AGPL packages in proprietary projects
- [ ] `Directory.Packages.props` central version management reviewed
- [ ] Source generators (Roslyn analyzers) have compatible licenses
- [ ] Microsoft BCL packages comply with runtime distribution terms
- [ ] `Analyzer` packages have MIT/Apache licenses (common)
- [ ] NuGet `<PackageReference PrivateAssets="All" />` — license allows embedding
- [ ] `GenerateDocumentationFile` projects include license headers
- [ ] No `.dll` references without license documentation

### Python Checklist

- [ ] All packages in `requirements.txt` have identifiable licenses
- [ ] No GPL/AGPL packages in proprietary deployments
- [ ] `pyproject.toml` `[tool.setuptools]` license field matches actual
- [ ] Conda packages comply with channel-specific ToS
- [ ] Vendored packages in `vendor/` have license documentation
- [ ] Wheels distributed to PyPI comply with downstream terms
- [ ] `pip-licenses` report reviewed for transitive conflicts
- [ ] ML model weights have compatible licenses

### JavaScript/TypeScript Checklist

- [ ] `package.json` `license` field set correctly
- [ ] All dependencies have `license` in their `package.json`
- [ ] No AGPL dependencies in SaaS products
- [ ] Monorepo workspaces have consistent licensing
- [ ] Frontend build plugins have compatible licenses
- [ ] CDN-hosted scripts have documented license + SRI hashes
- [ ] `node_modules` transitives scanned
- [ ] Lockfile matches audited versions

### Go Checklist

- [ ] All `go.mod` dependencies have identifiable licenses
- [ ] `replace` directives pointing to vendored code audited
- [ ] CGO-linked C libraries have compatible licenses
- [ ] Standard library usage within Go license terms
- [ ] `go-licenses` report saved and reviewed
- [ ] No internal packages mistakenly exposed

### Java Checklist

- [ ] Maven/Gradle dependency tree fully scanned
- [ ] BOM imports have compatible licenses
- [ ] Spring Boot starters and auto-configuration licenses reviewed
- [ ] JAR manifests contain valid license metadata
- [ ] WAR/EAR distribution bundles comply with embedded licenses
- [ ] Annotation processors have compatible licenses
- [ ] ServiceLoader META-INF services licensing reviewed

---

## Anti-Patterns

### Critical Anti-Patterns

| Anti-Pattern | Description | Impact |
|---|---|---|
| **GPL in proprietary** | GPL/AGPL dep in closed-source product | Source disclosure required, legal liability |
| **Missing LICENSE file** | No top-level LICENSE | Ambiguous, others cannot legally use |
| **Conflicting dual-license** | Dep under dual-license with incompatible terms | Usage rights unclear |
| **Unlicensed transitive dep** | No license metadata | Cannot determine usage rights |
| **AGPL in SaaS** | AGPL dep in network service | Must provide source to network users |
| **Vendored without attribution** | Copied code without original license/NOTICE | Copyright infringement |

### High Anti-Patterns

| Anti-Pattern | Description | Impact |
|---|---|---|
| **Stale license manifest** | `THIRD_PARTY_LICENSES` outdated | Missing new deps, wrong versions |
| **Mismatched license headers** | Source files different license than project | Ambiguous IP ownership |
| **Commercial without entitlement** | Commercial dep without license key verification | Legal exposure |
| **License URL 404** | `licenseUrl` returns 404 | Cannot verify actual terms |
| **SSPL/BSL dependencies** | Non-OSI source-available licenses | May restrict deployment |

### Medium Anti-Patterns

| Anti-Pattern | Description | Impact |
|---|---|---|
| **Inconsistent SPDX identifiers** | Mixed naming (MIT vs "MIT License") | Automation failure |
| **No NOTICE file** | Required attribution missing | Non-compliance with Apache-2.0, MIT |
| **Dev dependency oversight** | Dev-only deps excluded from audit | Build tool licensing gaps |
| **Pin version drift** | License changed between pinned and latest | May have incompatible terms |

---

## Code Examples

### .NET — License Scanning Script

```powershell
$packages = dotnet list package --include-transitive --format json | ConvertFrom-Json
$violations = @()
foreach ($pkg in $packages.projects | ForEach-Object { $_.packages }) {
    if ($pkg.license -match 'GPL|AGPL|SSPL') {
        $violations += [PSCustomObject]@{
            Package   = $pkg.id
            Version   = $pkg.resolvedVersion
            License   = $pkg.license
            Severity  = if ($pkg.license -match 'AGPL') { 'CRITICAL' } else { 'HIGH' }
        }
    }
}
$violations | Format-Table -AutoSize
```

### Python — pip-licenses with Compatibility Matrix

```python
import json, subprocess
COPYLEFT = {"GPL-3.0", "GPL-2.0", "AGPL-3.0", "LGPL-3.0", "LGPL-2.1", "SSPL-1.0", "BSL-1.1"}
result = subprocess.run(["pip-licenses", "--format=json"], capture_output=True, text=True)
licenses = json.loads(result.stdout)
for pkg in licenses:
    if pkg["License"].upper() in COPYLEFT:
        print(f"CRITICAL: {pkg['Name']}@{pkg['Version']} uses {pkg['License']}")
```

### Node.js — license-checker Filter

```javascript
const checker = require('license-checker');
const BLOCKED = /GPL|AGPL|SSPL|BSL|CC-BY-NC/i;
checker.init({ start: process.cwd() }, (err, packages) => {
    if (err) throw err;
    for (const [name, info] of Object.entries(packages)) {
        if (BLOCKED.test(info.licenses)) {
            console.log(`CRITICAL: ${name} uses ${info.licenses}`);
        }
    }
});
```

### Go — go-licenses Report

```go
//go:build ignore
package main
import (
    "fmt"
    "log"
    "os"
    "strings"
    "github.com/google/go-licenses/licenses"
)
func main() {
    copyleft := []string{"GPL", "AGPL", "LGPL", "SSPL", "BSL"}
    for _, cl := range copyleft {
        fmt.Printf("Checking for %s violations...\n", cl)
    }
    log.Println("Run: go-licenses report ./... > licenses.csv")
}
```

### Maven — License Report Generation

```xml
<plugin>
    <groupId>org.codehaus.mojo</groupId>
    <artifactId>license-maven-plugin</artifactId>
    <version>2.4.0</version>
    <executions>
        <execution>
            <id>aggregate-download-licenses</id>
            <goals><goal>aggregate-download-licenses</goal></goals>
        </execution>
    </executions>
</plugin>
```

---

## Output Format

### Finding Structure

```
SEVERITY: {critical|high|medium|low}
CATEGORY: {viral-license|missing-license|attribution|compatibility|stale-manifest|commercial}
FILE: {path to relevant file}
PACKAGE: {affected package/dependency}
LICENSE: {detected license identifier}
DESCRIPTION: {what and why}
RECOMMENDATION: {action to resolve}
REFERENCE: {relevant license clause or legal reference}
```

### Example Findings

```
SEVERITY: critical
CATEGORY: viral-license
FILE: src/Web/Web.csproj
PACKAGE: SomeLibrary@3.2.1
LICENSE: GPL-3.0
DESCRIPTION: GPL-3.0 dep in proprietary project. Viral copyleft.
RECOMMENDATION: Replace with MIT/Apache-2.0 alt or get commercial license.
REFERENCE: GPL-3.0 Section 5 (conveying covered works)

SEVERITY: high
CATEGORY: missing-license
FILE: package.json (dep: unlicensed-lib@1.0.0)
LICENSE: UNKNOWN
DESCRIPTION: Package has no license field. Cannot determine usage rights.
RECOMMENDATION: Contact maintainer. Add to denylist. Document in risk register.

SEVERITY: medium
CATEGORY: attribution
FILE: NOTICE
LICENSE: Apache-2.0 (dep: log4net@2.0.15)
DESCRIPTION: Apache-2.0 dep requires NOTICE attribution. Missing.
RECOMMENDATION: Add log4net attribution to NOTICE file.
REFERENCE: Apache-2.0 Section 4 (redistribution conditions)
```

---

## Severity Guidelines

| Severity | Criteria | Example |
|---|---|---|
| **Critical** | License requires source disclosure or prohibits commercial use | GPL/AGPL in proprietary; SSPL in SaaS |
| **High** | License metadata missing or clearly incompatible | Unlicensed dep; commercial without entitlement |
| **Medium** | Attribution or documentation gaps | Missing NOTICE; outdated THIRD_PARTY_LICENSES |
| **Low** | Cosmetic, minor inconsistencies | Mixed SPDX formats; dev dep not in manifest |

### Escalation Rules

- **Critical** → Must resolve before release. Escalate to legal.
- **High** → Document in risk register. Resolve within sprint.
- **Medium** → Track in backlog. Resolve within milestone.
- **Low** → Fix opportunistically. No deadline.

---

## Integration Notes

### Related Audit Squads

| Squad | Overlap | Boundary |
|---|---|---|
| `audit-dependencies` | Both scan deps | This: license terms only. Dependencies: versions, CVEs, freshness |
| `audit-security` | Supply chain risk | This: license compliance. Security: vuln scanning |
| `audit-cicd` | Build pipeline compliance | This: dep licenses. CI/CD: pipeline security, artifact integrity |
| `audit-privacy-data` | Data handling terms | This: license terms. Privacy: data protection |

### Automation Integration

- **CI/CD Gate**: Run license scan on every PR modifying dependency files
- **SBOM Generation**: License audit output feeds into SBOM for compliance reporting
- **Pre-commit Hook**: Block commits introducing GPL/AGPL in proprietary projects
- **Dashboard**: License compliance metrics (total deps, copyleft count, unknown count)

### License Compatibility Matrix (Quick Reference)

| Project License | MIT | Apache-2.0 | GPL-3.0 | AGPL-3.0 |
|---|---|---|---|---|
| **Proprietary** | ✅ Yes | ✅ Yes | ❌ No | ❌ No |
| **MIT** | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |
| **Apache-2.0** | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |
| **GPL-3.0** | ✅ Yes | ✅ Yes | ✅ Yes | ❌ No (AGPL upgrades) |
| **AGPL-3.0** | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |

### Audit Checklist Summary

1. Inventory all dependency licenses (direct + transitive)
2. Flag GPL/AGPL/SSPL/BSL in proprietary projects
3. Verify LICENSE file exists and matches project intent
4. Check NOTICE/attribution completeness
5. Validate vendored code has license documentation
6. Confirm commercial deps have valid entitlements
7. Review lockfiles match audited versions
8. Generate SBOM with license metadata