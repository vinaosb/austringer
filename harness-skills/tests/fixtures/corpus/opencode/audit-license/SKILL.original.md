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
- GPL/AGPL/LGPL detection — flag viral copyleft licenses requiring source disclosure
- Dependency license compatibility — verify transitive dependency licenses compatible with project license
- IP risk assessment — evaluate intellectual property exposure from license choices
- License header verification — confirm source files have correct SPDX or proprietary headers
- NOTICE/attribution compliance — verify required attribution notices present and complete
- Dual-license conflict detection — identify dependencies under incompatible dual-licensing terms
- Vendored code licensing — audit copied/embedded code for license compliance
- Commercial license verification — confirm commercial dependencies have valid entitlements

### .NET/C# Specific

- NuGet package license metadata scanning (`dotnet list package --include-transitive`)
- `Directory.Packages.props` central package management license review
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

- `package.json` license field scanning across dependency tree
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
- Bill of Materials (BOM) license verification
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

Scan the repository for license-related files and metadata.

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
| `vendor/**`, `vendored/**`, `third_party/**` | Vendored code requiring license audit |
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
- [ ] `Analyzer` packages have MIT/Apache licenses (common pattern)
- [ ] NuGet packages with `<PackageReference PrivateAssets="All" />` — verify license allows embedding
- [ ] `GenerateDocumentationFile` projects include license headers
- [ ] No `.dll` references without corresponding license documentation

### Python Checklist

- [ ] All packages in `requirements.txt` have identifiable licenses
- [ ] No GPL/AGPL packages in proprietary deployments
- [ ] `pyproject.toml` `[tool.setuptools]` license field matches actual license
- [ ] Conda packages comply with channel-specific terms of service
- [ ] Vendored packages in `vendor/` have license documentation
- [ ] Wheels distributed to PyPI comply with downstream license terms
- [ ] `pip-licenses` report reviewed for transitive dependency conflicts
- [ ] Machine learning model weights have compatible licenses (not just code)

### JavaScript/TypeScript Checklist

- [ ] `package.json` `license` field set correctly for project
- [ ] All dependencies have `license` field in their `package.json`
- [ ] No AGPL dependencies in SaaS products (network copyleft)
- [ ] Monorepo workspaces have consistent licensing
- [ ] Frontend build plugins (webpack, vite) have compatible licenses
- [ ] CDN-hosted scripts have documented license + SRI hashes
- [ ] `node_modules` transitives scanned (not just direct deps)
- [ ] Lockfile (`package-lock.json`, `yarn.lock`) matches audited versions

### Go Checklist

- [ ] All `go.mod` dependencies have identifiable licenses
- [ ] `replace` directives pointing to vendored code audited
- [ ] CGO-linked C libraries have compatible licenses
- [ ] Standard library usage within Go license terms
- [ ] `go-licenses` report saved and reviewed
- [ ] No internal packages mistakenly exposed (license implications)

### Java Checklist

- [ ] Maven/Gradle dependency tree fully scanned
- [ ] BOM (Bill of Materials) imports have compatible licenses
- [ ] Spring Boot starters and auto-configuration licenses reviewed
- [ ] JAR manifests contain valid license metadata
- [ ] WAR/EAR distribution bundles comply with all embedded licenses
- [ ] Annotation processors have compatible licenses
- [ ] ServiceLoader META-INF services licensing reviewed

---

## Anti-Patterns

### Critical Anti-Patterns

| Anti-Pattern | Description | Impact |
|---|---|---|
| **GPL in proprietary** | GPL/AGPL dependency in closed-source product | Requires source disclosure, legal liability |
| **Missing LICENSE file** | Repository has no top-level LICENSE | Ambiguous licensing, others cannot legally use |
| **Conflicting dual-license** | Dependency under dual-license with incompatible terms | Usage rights unclear, may violate terms |
| **Unlicensed transitive dep** | Transitive dependency has no license metadata | Cannot determine usage rights |
| **AGPL in SaaS** | AGPL dependency in network service | Must provide source to network users |
| **Vendored without attribution** | Copied code without original license/NOTICE | Copyright infringement |

### High Anti-Patterns

| Anti-Pattern | Description | Impact |
|---|---|---|
| **Stale license manifest** | `THIRD_PARTY_LICENSES` file outdated | Missing new dependencies, incorrect versions |
| **Mismatched license headers** | Source files have different license than project | Ambiguous IP ownership |
| **Commercial without entitlement** | Commercial dependency without license key verification | Legal exposure, production risk |
| **License URL 404** | `licenseUrl` in package metadata returns 404 | Cannot verify actual license terms |
| **SSPL/BSL dependencies** | Non-OSI-approved source-available licenses | May restrict deployment/serving options |

### Medium Anti-Patterns

| Anti-Pattern | Description | Impact |
|---|---|---|
| **Inconsistent SPDX identifiers** | Mixed license naming (MIT vs "MIT License") | Automation failure in license scanning |
| **No NOTICE file** | Required attribution notices missing | Non-compliance with Apache-2.0, MIT attribution |
| **Dev dependency oversight** | Dev-only dependencies excluded from license audit | Build tool licensing gaps |
| **Pin version drift** | License changed between pinned and latest version | Newer version may have incompatible terms |

---

## Code Examples

### .NET — License Scanning Script

```powershell
# Scan all NuGet packages for license information
$packages = dotnet list package --include-transive --format json | ConvertFrom-Json

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
import json
import subprocess
from pathlib import Path

# Known incompatible licenses for proprietary projects
COPYLEFT = {"GPL-3.0", "GPL-2.0", "AGPL-3.0", "LGPL-3.0", "LGPL-2.1", "SSPL-1.0", "BSL-1.1"}

result = subprocess.run(
    ["pip-licenses", "--format=json"],
    capture_output=True, text=True
)
licenses = json.loads(result.stdout)

for pkg in licenses:
    if pkg["License"].upper() in COPYLEFT:
        print(f"CRITICAL: {pkg['Name']}@{pkg['Version']} uses {pkg['License']}")
```

### Node.js — license-checker Filter

```javascript
const checker = require('license-checker');

// Known problematic licenses for proprietary/saaS
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

// Check go.mod dependencies for copyleft licenses
func main() {
    copyleft := []string{"GPL", "AGPL", "LGPL", "SSPL", "BSL"}
    // go-licenses report ./... outputs: module, license, url
    // Filter for copyleft matches
    for _, cl := range copyleft {
        fmt.Printf("Checking for %s violations...\n", cl)
    }
    log.Println("Run: go-licenses report ./... > licenses.csv")
}
```

### Maven — License Report Generation

```xml
<!-- pom.xml — Add license-maven-plugin -->
<plugin>
    <groupId>org.codehaus.mojo</groupId>
    <artifactId>license-maven-plugin</artifactId>
    <version>2.4.0</version>
    <executions>
        <execution>
            <id>aggregate-download-licenses</id>
            <goals>
                <goal>aggregate-download-licenses</goal>
            </goals>
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
DESCRIPTION: {what was found and why it matters}
RECOMMENDATION: {specific action to resolve}
REFERENCE: {relevant license clause or legal reference}
```

### Example Findings

```
SEVERITY: critical
CATEGORY: viral-license
FILE: src/Web/Web.csproj
PACKAGE: SomeLibrary@3.2.1
LICENSE: GPL-3.0
DESCRIPTION: GPL-3.0 dependency in proprietary project.
  Viral copyleft requires source disclosure for distribution.
RECOMMENDATION: Replace with MIT/Apache-2.0 alternative or obtain
  commercial license from vendor.
REFERENCE: GPL-3.0 Section 5 (conveying covered works)

SEVERITY: high
CATEGORY: missing-license
FILE: package.json (dependency: unlicensed-lib@1.0.0)
LICENSE: UNKNOWN
DESCRIPTION: Package has no license field in metadata.
  Cannot determine usage rights.
RECOMMENDATION: Contact maintainer for license clarification.
  Add to denylist until resolved. Document in risk register.

SEVERITY: medium
CATEGORY: attribution
FILE: NOTICE
LICENSE: Apache-2.0 (dependency: log4net@2.0.15)
DESCRIPTION: Apache-2.0 dependency requires attribution in NOTICE file.
  Current NOTICE file does not include this dependency.
RECOMMENDATION: Add attribution entry for log4net to NOTICE file.
REFERENCE: Apache-2.0 Section 4 (redistribution conditions)
```

---

## Severity Guidelines

| Severity | Criteria | Example |
|---|---|---|
| **Critical** | License requires source disclosure or prohibits commercial use in current context | GPL/AGPL in proprietary product; SSPL in SaaS offering |
| **High** | License metadata missing or clearly incompatible; legal exposure likely | Unlicensed dependency; commercial dependency without entitlement |
| **Medium** | Attribution or documentation gaps; stale manifests | Missing NOTICE entry; outdated THIRD_PARTY_LICENSES file |
| **Low** | Cosmetic issues; minor inconsistencies | Mixed SPDX identifier formats; dev dependency not in manifest |

### Escalation Rules

- **Critical** findings → Must be resolved before any release. Escalate to legal.
- **High** findings → Must be documented in risk register. Resolve within sprint.
- **Medium** findings → Track in backlog. Resolve within milestone.
- **Low** findings → Fix opportunistically. No deadline pressure.

---

## Integration Notes

### Related Audit Squads

| Squad | Overlap | Boundary |
|---|---|---|
| `audit-dependencies` | Both scan dependencies | This squad: license terms only. Dependencies: versions, CVEs, freshness |
| `audit-security` | Supply chain risk | This squad: license compliance. Security: vulnerability scanning |
| `audit-cicd` | Build pipeline compliance | This squad: dependency licenses. CI/CD: pipeline security, artifact integrity |
| `audit-privacy-data` | Data handling terms | This squad: license terms. Privacy: data protection compliance |

### Automation Integration

- **CI/CD Gate**: Run license scan on every PR that modifies dependency files (`package.json`, `*.csproj`, `requirements.txt`, `go.mod`, `pom.xml`)
- **SBOM Generation**: License audit output feeds into Software Bill of Materials (SBOM) for compliance reporting
- **Pre-commit Hook**: Block commits that introduce GPL/AGPL dependencies in proprietary projects
- **Dashboard**: License compliance metrics (total deps, copyleft count, unknown count) in observability dashboards

### License Compatibility Matrix (Quick Reference)

| Project License | Can Use MIT? | Can Use Apache-2.0? | Can Use GPL-3.0? | Can Use AGPL-3.0? |
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
6. Confirm commercial dependencies have valid entitlements
7. Review lockfiles match audited versions
8. Generate SBOM with license metadata
