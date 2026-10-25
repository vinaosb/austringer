---
name: audit-security
description: >-
  Security auditing: injection vulnerabilities, exposed secrets, sanitization issues,
  unsafe operations, cryptographic weaknesses, and authentication/authorization flaws
  across all major programming languages and frameworks.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Security Audit Squad

## USE FOR

- Input validation — unsanitized user input, missing bounds checks, SQL/XSS/command injection
- Exposed credentials — hardcoded API keys, passwords in source, secrets in config
- SQL injection — raw queries with string interpolation, FromSqlRaw with unsanitized input
- XSS/output encoding — innerHTML with user data, unescaped template output, CSV formula injection
- Command injection — subprocess shell=True, Process.Start with user input, os.system
- Crypto issues — weak hashes (MD5/SHA1), AES-ECB, hardcoded salts/IVs, insecure RNG
- Auth/authorization flaws — missing [Authorize], blanket [AllowAnonymous], CSRF absence
- Insecure deserialization — pickle.loads on untrusted data, BinaryFormatter, TypeNameHandling.Auto
- Info leakage — verbose errors in prod, stack traces in API responses, PII in logs
- Dep vuln scanning — known CVEs in manifests (shallow; use audit-dependencies for deep)
- Security headers — missing CSP, HSTS, X-Content-Type-Options, insecure cookie flags
- **.NET/C#**: CA3001-3008, CA3075, CA3125 violations, SqlClient encryption, antiforgery, TLS
- **Python**: Bandit linting, SQLAlchemy text() injection, Jinja2 escaping, session cookies
- **JavaScript/TypeScript**: ESLint security plugin, DOMPurify, path traversal, npm audit
- **Go**: gosec scanning, SQL injection via database/sql, context timeout implications
- **Java**: Spring Security config, PreparedStatement compliance, JCA/JCE providers
- **Rust**: Unsafe block audit, crypto crate audit, input validation in unsafe contexts

### DO NOT USE FOR

- Deep CVE analysis and SBOM (use `audit-dependencies`)
- Architectural layer violations (use `audit-architecture`)
- Code style and naming (use `audit-code-quality`)
- CI/CD pipeline security (use `audit-cicd`)
- PII/privacy compliance (use `audit-privacy-data`)
- License compliance (use `audit-license`)
- Fault tolerance/resilience (use `audit-fault-tolerance`)

---

## Section 3: Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repo names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |

### 3.2 Security-Relevant Framework Detection

| Signal | File Pattern | Security Concern |
|--------|-------------|------------------|
| Auth config | `*Auth*`, `*Identity*`, `*Security*`, `*Middleware*` | Auth/authorization |
| Input handlers | `*Controller*`, `*Handler*`, `*Endpoint*`, `*Service*` | Input validation |
| Crypto code | `*Crypto*`, `*Encrypt*`, `*Hash*`, `*Certificate*` | Crypto implementation |
| Config files | `appsettings*.json`, `*.env`, `web.config`, `application.yml` | Exposed secrets |
| DB context | `*DbContext*`, `*Repository*`, `*Context*` | SQL injection vectors |
| DI registration | `*Registration*`, `*Startup*`, `Program.cs` | Service lifetimes, auth middleware |
| Middleware | `*Middleware*`, `*Filter*`, `*Interceptor*` | Request pipeline security |
| Exports | `*Export*`, `*Download*`, `*Report*` | CSV injection, data exposure |
| CI config | `.gitlab-ci.yml`, `.github/workflows/*.yml` | Pipeline secrets |

### 3.3 Security Tool Detection

| Signal | Pattern | Tool |
|--------|---------|------|
| .NET Roslyn analyzers | `*.csproj` with `Microsoft.CodeAnalysis.*` or `EnforceCodeStyleInBuild` | Security analyzers |
| Bandit | `.bandit`, `pyproject.toml` with `[tool.bandit]` | Python security |
| ESLint security | `.eslintrc` with `eslint-plugin-security` | JS/TS security |
| gosec | `gosec` in Makefile or CI | Go security |
| FindSecBugs | `find-sec-bugs` in `pom.xml`/`build.gradle` | Java security |
| cargo-audit | `cargo audit` in CI | Rust dep vulns |
| Trivy | `.trivyignore`, `trivy` in CI | Container/filesystem vulns |

### 3.4 Infrastructure Detection (Security-Relevant)

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml` | Container security |
| Secret manager | `vault`, `openbao`, `aws secretsmanager` in config | Credential management |
| TLS certs | `*.pem`, `*.crt`, `*.key`, `*.pfx` | Certificate management |
| CSP headers | `Content-Security-Policy` in web config | CSP enforcement |
| HSTS | `Strict-Transport-Security` in web config | HTTPS enforcement |

### 3.5 Discovery Procedure

1. Scan root dir for all file patterns above.
2. Identify primary language by source file count.
3. Identify frameworks by config analysis.
4. Locate security-relevant dirs: auth, middleware, crypto, config, handlers.
5. Check for secret management: Vault/OpenBao, env vars, `.env.example`.
6. Identify security tooling: SAST scanners, secret scanners, dep scanners.
7. Check for security headers: CSP, HSTS, X-Content-Type-Options.
8. Select matching checklist from Section 4 based on detected tech.
9. If multiple languages (monorepo), run all applicable checklists.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Security Checklist

- [ ] **SQL injection via FromSqlRaw**: Search `FromSqlRaw`, `FromSqlInterpolated`, `ExecuteSqlRaw` with string interpolation. All params must use typed overload.
- [ ] **Command injection via Process.Start**: Search `Process.Start` with user-controllable args. Use `ArgumentList` not string concat.
- [ ] **Hardcoded connection strings**: Search `appsettings.json` for conn strings committed to git. Reference env vars or secret manager.
- [ ] **Weak crypto**: Search for `MD5`, `SHA1`, `AesManaged` ECB, `DESCryptoServiceProvider`, `RC2`, `TripleDES`. Use `SHA256`/`SHA512`, `AesGcm`/`AesCbc` with random IV.
- [ ] **[AllowAnonymous] misuse**: Search `[AllowAnonymous]` on sensitive endpoints. Every anonymous endpoint needs documented justification.
- [ ] **Missing CSRF/antiforgery**: Verify `[ValidateAntiForgeryToken]` or `AutoValidateAntiforgery` on all POST/PUT/DELETE in MVC/Razor Pages.
- [ ] **Insecure deserialization**: Search `BinaryFormatter`, `SoapFormatter`, `NetDataContractSerializer`, `TypeNameHandling.Auto/All`. Use `System.Text.Json` with typed deserialization.
- [ ] **CSV formula injection**: Search CSV export that doesn't sanitize cells starting with `=`, `+`, `-`, `@`, `\t`, `\r`. Prefix dangerous chars with single quote.
- [ ] **Secrets as plaintext env vars**: Validate conn strings, API keys, tokens against secret manager usage. Prefer Vault/OpenBao transit.
- [ ] **Export endpoints lacking per-module RBAC**: Verify export/download endpoints enforce module-specific claims, not just blanket `[Authorize]`.
- [ ] **PII in log output**: Search `LogInformation`, `logger.Info` with user data (email, name, SSN, phone). Use structured logging with redacted fields.
- [ ] **Missing security headers**: Verify CSP (with nonce for Blazor), HSTS, X-Content-Type-Options, X-Frame-Options.
- [ ] **HttpClient with TLS disabled**: Search `ServerCertificateCustomValidationCallback` returning true, or `SslProtocols.None`.
- [ ] **CA1031 suppression without justification**: Search `#pragma warning disable CA1031` — catch-all handlers need documented justification.
- [ ] **Weak password hashing**: Search `MD5`/`SHA1`/`SHA256` for password hashing. Use BCrypt, Argon2, or PBKDF2 with per-user salt.
- [ ] **TLS version enforcement**: Search `ServicePointManager.SecurityProtocol`. Enforce Tls12 or Tls13 minimum.

### 4.2 Python Security Checklist

- [ ] **SQL injection via f-strings**: Search `cursor.execute(f"`, `cursor.execute("...%s" %`, `.format(` in SQL strings. Use parameterized queries.
- [ ] **Command injection via shell=True**: Search `subprocess.run(`, `.call(`, `.Popen(` with `shell=True`. Use `shell=False` with arg lists.
- [ ] **Hardcoded API keys/tokens**: Search `api_key =`, `secret =`, `token =`, `password =` with literal strings. Use env vars or secret manager.
- [ ] **Pickle deserialization**: Search `pickle.loads`, `pickle.load` on untrusted data. Use `json.loads` for untrusted data.
- [ ] **Insecure temp files**: Search `tempfile.mktemp` (race condition). Use `tempfile.mkstemp` or `NamedTemporaryFile` with `delete=True`.
- [ ] **eval/exec on user input**: Search `eval(`, `exec(` with user data. Use `ast.literal_eval()` or explicit parsing.
- [ ] **JWT without verification**: Search `jwt.decode(` without `algorithms` param or with `verify=False`. Always specify algorithms, verify signature.
- [ ] **Flask DEBUG in production**: Search `app.run(debug=True)` or `DEBUG = True` in production config. Use env-driven config.
- [ ] **SQLAlchemy text() with string concat**: Search `text(f"`, `text("...{}".format(`. Use bind parameters: `text("...WHERE id = :id")`.
- [ ] **Missing rate limiting**: Verify API endpoints have rate limiting middleware for auth, registration, password reset.
- [ ] **Insecure cookie flags**: Verify `SESSION_COOKIE_SECURE = True`, `SESSION_COOKIE_HTTPONLY = True` in Django/Flask.
- [ ] **Jinja2 autoescape disabled**: Search `autoescape=False` in Jinja2 config. Keep enabled or manually escape.
- [ ] **YAML load without SafeLoader**: Search `yaml.load(` without `Loader=yaml.SafeLoader`. Prefer `yaml.safe_load()`.
- [ ] **XXE in XML parsing**: Search `xml.etree.ElementTree.parse` or `lxml.etree.parse` without disabling external entities.

### 4.3 JavaScript/TypeScript Security Checklist

- [ ] **XSS via innerHTML**: Search `.innerHTML =` with user input vars. Use `.textContent`, `.innerText`, or DOMPurify.
- [ ] **eval() usage**: Search `eval(`, `new Function(`, `setTimeout(string)`, `setInterval(string)`. Replace with safe alternatives.
- [ ] **Prototype pollution**: Search `Object.assign({}, userInput)`, `_.merge(target, userInput)`, `deepExtend` without validation. Validate keys against whitelist.
- [ ] **DOM-based injection**: Search `document.write(`, `location.href = userInput`, `document.cookie` reads. Use safe DOM APIs and HttpOnly cookies.
- [ ] **Insecure cookie flags**: Verify `Secure`, `HttpOnly`, `SameSite=Strict/Lax` on session/auth cookies. No `SameSite=None` without `Secure`.
- [ ] **SQL injection in ORM raw queries**: Search `sequelize.query(`, `knex.raw(` with string concat. Use parameterized bindings.
- [ ] **Client-side secrets**: Search `apiKey`, `apiSecret`, `password` in `.js`/`.ts`. API keys should be server-side only.
- [ ] **Missing CSP**: Verify CSP header or meta tag set. No `unsafe-inline`/`unsafe-eval` without nonce/hash justification.
- [ ] **JWT in localStorage**: Tokens in localStorage accessible to XSS. Use HttpOnly cookies or short-lived tokens with refresh rotation.
- [ ] **Unvalidated redirects**: Search `res.redirect(userInput)`, `window.location = userInput`. Validate against whitelist.
- [ ] **Security middleware**: Verify `helmet`, `express-rate-limit`, `csurf` or equivalents configured.
- [ ] **GraphQL introspection in production**: Search `introspection: true` in production config. Disable in production.
- [ ] **SSRF**: Search `fetch(userInput)`, `axios.get(userInput)` where user controls URL. Validate against allowlist.

### 4.4 Go Security Checklist

- [ ] **SQL injection via fmt.Sprintf**: Search `fmt.Sprintf` in SQL query strings. Use `database/sql` parameterized with `$1`, `$2`.
- [ ] **Command injection via exec.Command**: Search `exec.Command` with string concat. Pass args as separate params.
- [ ] **Path traversal**: Search `os.Open(userInput)`, `filepath.Join(baseDir, userInput)`. Validate with `filepath.Abs` and prefix check.
- [ ] **Hardcoded credentials**: Search `password =`, `api_key =`, `secret =` with literal strings. Use env vars or secret manager.
- [ ] **JWT without verification**: Search `jwt.Parse` without key function or with `WithoutSignatureValidation()`. Always validate signature.
- [ ] **Missing input validation in HTTP handlers**: Search `r.URL.Query().Get("` without length/format validation.
- [ ] **HTTPS not enforced**: Verify TLS config. Search `http.ListenAndServe` vs `http.ListenAndServeTLS`.
- [ ] **Insecure RNG**: Search `rand.Intn` for security-sensitive values. Use `crypto/rand`.
- [ ] **CORS misconfig**: Search `AllowAllOrigins()` or `Access-Control-Allow-Origin: *` with credentials. Restrict origins.

### 4.5 Java Security Checklist

- [ ] **SQL injection via string concat**: Search `"SELECT ... " + userInput` in JDBC. Use `PreparedStatement` with bind params.
- [ ] **Insecure deserialization**: Search `ObjectInputStream.readObject()` on untrusted data. Use JSON or `ValidatingObjectInputStream`.
- [ ] **Hardcoded JNDI strings**: Search JNDI lookup strings with hardcoded URLs. Use env-driven config.
- [ ] **XXE injection**: Verify XML parsers have `FEATURE_SECURE_PROCESSING` enabled. Use `DocumentBuilderFactory.setFeature(...)`.
- [ ] **Spring Security config**: Verify `@PreAuthorize`, `@Secured`, `@RolesAllowed` on sensitive endpoints. Check `SecurityFilterChain` order.
- [ ] **Missing CSRF in Spring**: Verify `http.csrf()` not disabled without justification for stateless APIs.
- [ ] **Sensitive data in toString()**: Search `toString()` including passwords, tokens, or PII.
- [ ] **Log injection**: Search logger calls with user input in message template. Use `%s` placeholders.
- [ ] **Insecure cipher**: Search `Cipher.getInstance("AES/ECB/"` or `DES`. Use `AES/GCM/NoPadding` with random IV.

### 4.6 Rust Security Checklist

- [ ] **Unsafe block audit**: Verify `unsafe` blocks have documented safety invariants. No unnecessary raw pointer dereferences from untrusted sources.
- [ ] **SQL injection via format! in SQL**: Search `format!(` or `format_args!(` inside query strings. Use `sqlx::query!` type-safe queries.
- [ ] **Command injection**: Search `Command::new` with user input args. Use arg list, not shell string.
- [ ] **Hardcoded secrets**: Search `const`/`static` vars containing keys or passwords. Use env vars or secure config crate.
- [ ] **Timing-safe comparison**: Search `==` comparison of secrets/tokens/passwords. Use `constant_time_eq` from `subtle`.
- [ ] **Insecure randomness**: Search `rand::thread_rng()` for keys/tokens. Use `OsRng` from `rand_chacha` or `ring`.
- [ ] **Unsafe transmute**: Search `std::mem::transmute` on untrusted input. Verify type safety.

---

## Section 5: Anti-Patterns

### 5.1 Cross-Language Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Secrets in source code | critical | Keys, passwords, tokens hardcoded in source | Leak via VCS, logs, error reports, code search | Env vars, secret manager (Vault, OpenBao, AWS Secrets Manager) |
| 2 | String concat in SQL | critical | Building SQL with string interpolation | SQL injection: data exfiltration, auth bypass, data destruction | Parameterized queries with typed params |
| 3 | Catch-all without logging | high | Empty catch blocks or catch-all with no logging | Security incidents undetected, errors silently swallowed | Catch specific exceptions, log with context, rethrow/handle |
| 4 | Verbose errors to clients | high | Stack traces, SQL errors, internal paths in API responses | Info leakage aids attacker recon | Generic error responses; detailed errors logged server-side |
| 5 | Missing input validation | high | No validation/sanitization on user input before processing | Enables injection, overflow, logic bypass | Validate type, length, format, range at entry point |
| 6 | CSV formula injection | medium | Exporting unescaped `=`, `+`, `-`, `@` in CSV cells | Spreadsheets execute formulas when opening CSV | Prefix dangerous chars with single quote or tab escape |
| 7 | PII in log output | medium | Logging personally identifiable information (email, SSN, phone) | Privacy violations; data exposure in log aggregation | Redact PII; use correlation IDs instead of identifiers |

### 5.2 .NET/C# Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Blanket [AllowAnonymous] | high | Applying to entire controllers or sensitive endpoints | Bypasses all auth for those endpoints | Apply to specific actions only with justification |
| 2 | Trivy ignore suppressing CVEs by year range | critical | `.trivyignore` with broad CVE patterns | Known critical vulns ignored in prod | Triage individually; suppress only with justification + expiry |
| 3 | Health endpoints requiring auth | medium | Health checks behind [Authorize] | Monitoring tools can't probe health, breaking alerting | Basic liveness anonymous; readiness checks can use auth-protected deps |
| 4 | Export endpoints with blanket RequireAuthorization | medium | Export/download with generic [Authorize] no per-module RBAC | Any auth user can export any module's data | Enforce per-module claims/policies on exports |
| 5 | HttpResponseMessage not disposed | medium | No Dispose or not using `using` | Connection leaks, socket exhaustion under load | `using var response = await client.SendAsync(...)` |
| 6 | FromSqlRaw with interpolated string | critical | `FromSqlRaw($"SELECT ... WHERE id = {id}")` | Direct SQL injection | Use parameterized: `FromSqlRaw("... WHERE id = {0}", id)` |
| 7 | Weak hashing for passwords | critical | MD5/SHA1 for password hashing | Trivially reversible; no salting | BCrypt, Argon2, or PBKDF2 with per-user salt |

### 5.3 Python Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | shell=True with user input | critical | subprocess.run(cmd, shell=True) with user data | Command injection via shell metacharacters | shell=False with arg list |
| 2 | pickle.loads on network data | critical | Deserializing untrusted pickle data | Arbitrary code execution | json or msgpack for untrusted data |
| 3 | eval() on user input | critical | Dynamic eval of user-provided strings | Remote code execution | ast.literal_eval() or explicit parsing |
| 4 | Flask debug in production | high | app.run(debug=True) or DEBUG=True in prod | Debugger exposes source, allows code execution | env-driven config; disable debug in prod |
| 5 | SQL via f-strings | critical | `f"SELECT ... WHERE id = {user_id}"` | SQL injection | Parameterized: `cursor.execute("...WHERE id = %s", (user_id,))` |

### 5.4 JavaScript/TypeScript Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | eval() with user input | critical | Executing user-provided strings | RCE | JSON.parse for data, template literals for display |
| 2 | innerHTML with user input | high | Setting innerHTML from untrusted source | XSS | textContent, createElement, or DOMPurify |
| 3 | JWT in localStorage | medium | Auth tokens in localStorage | Accessible to any XSS; no HttpOnly | HttpOnly secure cookies with SameSite |
| 4 | Prototype pollution | high | Merging untrusted objects into prototypes | Can override methods, inject properties, DoS | Validate keys against whitelist before merging |
| 5 | Unvalidated redirects | medium | res.redirect(userInput) | Open redirect for phishing | Validate URLs against whitelist |

### 5.5 Go Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | fmt.Sprintf in SQL | critical | Building SQL with Sprintf | SQL injection | Parameterized with `$1`, `$2` |
| 2 | Hardcoded credentials in const/var | critical | API keys, passwords as constants | Leaked in binaries, logs, VCS | Env vars or secret store |
| 3 | Path traversal with os.Open | high | Opening files based on user-controlled path | Unauthorized file access | Validate with filepath.Abs + prefix check |

### 5.6 Java Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | SQL string concat in JDBC | critical | `"SELECT ... WHERE id = " + id` | SQL injection | PreparedStatement with bind variables |
| 2 | ObjectInputStream.readObject() | critical | Deserializing untrusted data | RCE via gadget chains | JSON serialization or whitelist |
| 3 | XXE enabled in XML parsers | high | External entity processing enabled | File disclosure, SSRF | Disable external entities: FEATURE_DISALLOW_DOCTYPE_DECL=true |

---

## Section 6: Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: SQL Injection — FromSqlRaw with String Interpolation

```csharp
// ❌ BAD: SQL injection via string interpolation in FromSqlRaw
var results = await _dbContext.Users
    .FromSqlRaw($"SELECT * FROM users WHERE email = '{email}'")
    .ToListAsync();

// ✅ GOOD: Parameterized query with typed parameter
var results = await _dbContext.Users
    .FromSqlRaw("SELECT * FROM users WHERE email = {0}", email)
    .ToListAsync();

// ✅ BEST: Use LINQ instead of raw SQL when possible
var results = await _dbContext.Users
    .Where(u => u.Email == email)
    .AsNoTracking()
    .ToListAsync();
```

#### Example 2: Secret Management — Hardcoded vs Environment Variables

```csharp
// ❌ BAD: Hardcoded connection string in appsettings.json committed to git
{
  "ConnectionStrings": {
    "DefaultConnection": "Host=localhost;Database=mydb;Username=admin;Password=P@ssw0rd123"
  }
}

// ✅ GOOD: Reference environment variable or secret manager
{
  "ConnectionStrings": {
    "DefaultConnection": ""
  }
}
// In Program.cs or Startup:
var connectionString = builder.Configuration["ConnectionStrings:DefaultConnection"]
    ?? throw new InvalidOperationException("Connection string not configured");
// Set via environment variable: ConnectionStrings__DefaultConnection=Host=...
```

#### Example 3: CSV Formula Injection — Unescaped Dangerous Characters

```csharp
// ❌ BAD: Direct CSV export without sanitization
var csv = string.Join("\n", records.Select(r =>
    $"{r.Name},{r.Email},{r.Balance}"));

// ✅ GOOD: Sanitize cells to prevent formula injection
static string SanitizeCsvCell(string value)
{
    if (string.IsNullOrEmpty(value)) return value;
    var firstChar = value[0];
    if (firstChar is '=' or '+' or '-' or '@' or '\t' or '\r')
        return "'" + value;
    return value;
}

var csv = string.Join("\n", records.Select(r =>
    $"{SanitizeCsvCell(r.Name)},{SanitizeCsvCell(r.Email)},{SanitizeCsvCell(r.Balance.ToString())}"));
```

#### Example 4: Crypto — Weak vs Strong Hashing

```csharp
// ❌ BAD: MD5 for password hashing
using var md5 = MD5.Create();
var hash = Convert.ToBase64String(md5.ComputeHash(Encoding.UTF8.GetBytes(password)));

// ✅ GOOD: BCrypt with per-user salt (automatic)
var hash = BCrypt.HashPassword(password, workFactor: 12);

// Verification:
var isValid = BCrypt.Verify(inputPassword, storedHash);
```

### 6.2 Python Examples

#### Example 1: SQL Injection — f-string vs Parameterized

```python
# ❌ BAD: SQL injection via f-string
def get_user(email):
    cursor.execute(f"SELECT * FROM users WHERE email = '{email}'")
    return cursor.fetchone()

# ✅ GOOD: Parameterized query
def get_user(email: str) -> dict | None:
    cursor.execute("SELECT * FROM users WHERE email = %s", (email,))
    return cursor.fetchone()
```

#### Example 2: Command Injection — shell=True vs Argument List

```python
# ❌ BAD: Command injection via shell=True
import subprocess

def convert_file(filename):
    subprocess.run(f"convert {filename} output.png", shell=True)

# ✅ GOOD: shell=False with argument list
import subprocess
import os

def convert_file(filename: str) -> None:
    # Validate filename for path traversal
    if ".." in filename or "/" in filename or "\\" in filename:
        raise ValueError("Invalid filename")
    subprocess.run(["convert", filename, "output.png"], shell=False, check=True)
```

#### Example 3: Secret Management — Hardcoded vs Environment

```python
# ❌ BAD: Hardcoded API key
API_KEY = "sk-abc123secret456"

def call_external_service(data):
    headers = {"Authorization": f"Bearer {API_KEY}"}
    response = requests.post("https://api.example.com/process", headers=headers, json=data)
    return response.json()

# ✅ GOOD: Environment variable with validation
import os

API_KEY = os.environ.get("EXTERNAL_SERVICE_API_KEY")
if not API_KEY:
    raise RuntimeError("EXTERNAL_SERVICE_API_KEY environment variable not set")

def call_external_service(data: dict) -> dict:
    headers = {"Authorization": f"Bearer {API_KEY}"}
    response = requests.post("https://api.example.com/process", headers=headers, json=data)
    response.raise_for_status()
    return response.json()
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: XSS — innerHTML vs textContent

```javascript
// ❌ BAD: XSS via innerHTML with user input
function displaySearchResults(results) {
    const container = document.getElementById("results");
    results.forEach(result => {
        container.innerHTML += `<div>${result.title}</div>`;
    });
}

// ✅ GOOD: textContent escapes HTML automatically
function displaySearchResults(results) {
    const container = document.getElementById("results");
    results.forEach(result => {
        const div = document.createElement("div");
        div.textContent = result.title;
        container.appendChild(div);
    });
}
```

#### Example 2: Insecure Cookie Configuration

```javascript
// ❌ BAD: Missing Secure, HttpOnly, SameSite flags
app.use(session({
    secret: process.env.SESSION_SECRET,
    cookie: {}
}));

// ✅ GOOD: Secure cookie configuration
app.use(session({
    secret: process.env.SESSION_SECRET,
    cookie: {
        secure: true,      // HTTPS only
        httpOnly: true,    // No JS access
        sameSite: "strict", // CSRF protection
        maxAge: 3600000    // 1 hour
    }
}));
```

#### Example 3: JWT Handling — localStorage vs HttpOnly Cookie

```javascript
// ❌ BAD: JWT stored in localStorage (accessible to XSS)
function storeToken(token) {
    localStorage.setItem("auth_token", token);
}

// ✅ GOOD: Server sets JWT as HttpOnly cookie
// Server-side:
app.post("/api/login", (req, res) => {
    const token = generateJwt(req.user);
    res.cookie("auth_token", token, {
        httpOnly: true,
        secure: true,
        sameSite: "strict",
        maxAge: 3600000
    });
    res.json({ success: true });
});
```

### 6.4 Go Examples

#### Example 1: SQL injection via Sprintf vs Parameterized

```go
// ❌ BAD: String concatenation in SQL
func getUser(db *sql.DB, email string) (*User, error) {
    query := fmt.Sprintf("SELECT id, email FROM users WHERE email = '%s'", email)
    row := db.QueryRow(query)
}

// ✅ GOOD: Parameterized query
func getUser(db *sql.DB, email string) (*User, error) {
    query := "SELECT id, email FROM users WHERE email = $1"
    row := db.QueryRow(query, email)
}
```

#### Example 2: Path Traversal — Direct User Input vs Validation

```go
// ❌ BAD: Direct user-controlled file path
func readFile(filename string) ([]byte, error) {
    return os.ReadFile(filename) // User could pass "../../etc/passwd"
}

// ✅ GOOD: Validate path against allowed base directory
func readFile(filename string, baseDir string) ([]byte, error) {
    cleanPath := filepath.Clean(filepath.Join(baseDir, filename))
    absPath, _ := filepath.Abs(cleanPath)
    absBase, _ := filepath.Abs(baseDir)
    if !strings.HasPrefix(absPath, absBase) {
        return nil, fmt.Errorf("path traversal detected: %s", filename)
    }
    return os.ReadFile(absPath)
}
```

---

## Section 7: Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include vulnerable code pattern.",
  "recommendation": "Specific fix with brief code example if applicable.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "security"
}
```

### 7.2 Field Rules

- **severity**: One of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Line where finding ends.
- **description**: Factual, specific. Include vulnerable code fragment. No hedging.
- **recommendation**: Actionable. Include code snippet when possible.
- **technology**: Primary tech. `CrossLanguage` only for multi-file patterns.
- **category**: Always `security`.

### 7.3 Summary Report

```json
{
  "summary": {
    "squad": "audit-security",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 15,
    "bySeverity": { "critical": 1, "high": 3, "medium": 7, "low": 4 },
    "byCategory": { "security": 15 },
    "filesScanned": 42,
    "filesWithFindings": 12
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Security-Specific Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Immediate exploitation risk, RCE, data breach, total compromise | SQL injection, command injection, exposed prod credentials, deserialization RCE |
| **high** | Significant risk with clear attack vector | XSS, path traversal, auth bypass, weak crypto in prod, CSRF on state-changing ops |
| **medium** | Degraded security posture, defense-in-depth gap | Missing input validation, CSV formula injection, verbose errors, insecure cookie flags |
| **low** | Best practice deviation, defense-in-depth improvement | Missing CSP on static pages, dev-only debug flags, informational logging |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding in auth/authorization code | +1 level | All tech |
| Finding in encryption/key management | +1 level | All tech |
| Finding affects multiple tenants/users | +1 level | Multi-tenant systems |
| Finding in public-facing endpoint | +1 level | Web apps |
| Finding involves financial/billing data | +1 level | Financial systems |
| Finding has known CVE | +1 level | Dep vulns |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in non-production/test code only | -1 level (min low) | All tech |
| Finding has existing mitigating control | -1 level (min low) | All tech |
| Finding in deprecated code for removal | -1 level (min low) | All tech |

### 8.4 Technology-Specific Severity Adjustments

| Technology | Pattern | Base Severity | Notes |
|-----------|---------|---------------|-------|
| **.NET/C#** | `FromSqlRaw` with interpolation | critical | Direct SQL injection |
| **.NET/C#** | MD5/SHA1 for passwords | critical | Trivially reversible |
| **.NET/C#** | [AllowAnonymous] on sensitive endpoint | high | Auth bypass |
| **.NET/C#** | Missing antiforgery token | high | CSRF |
| **.NET/C#** | PII in log output | medium | Privacy violation |
| **Python** | `shell=True` with external input | critical | Command injection |
| **Python** | `pickle.loads` on network data | critical | RCE |
| **Python** | `eval()` on user input | critical | RCE |
| **Python** | Missing rate limiting on auth | high | Brute force |
| **JavaScript** | `eval()` with user input | critical | RCE |
| **JavaScript** | `innerHTML` with user input | high | XSS |
| **JavaScript** | JWT in localStorage | medium | XSS-exposed tokens |
| **Go** | `fmt.Sprintf` in SQL | critical | SQL injection |
| **Go** | Path traversal via os.Open | high | Unauthorized file read |
| **Go** | `crypto/rand` not used for secrets | high | Weak random tokens |
| **Java** | SQL string concatenation | critical | SQL injection |
| **Java** | XXE in XML parsing | high | File disclosure |
| **Java** | `ObjectInputStream.readObject()` | critical | Deserialization RCE |

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
audit-cicd ─────────────────────────┤
audit-license ──────────────────────┤
audit-consolidated ◄────────────────┘
audit-consistency-coherence
```

### 9.2 Upstream Dependencies

- **audit-dependencies**: CVE data for third-party packages. Cross-reference code findings with dep CVEs.
- **audit-cicd**: Pipeline security context. May reference pipeline secret handling.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Cross-domain alignment validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-security | audit-dependencies | Security: code vulns; dependencies: CVE/license in third-party |
| audit-security | audit-cicd | Security: code; cicd: pipeline config and deploy security |
| audit-security | audit-privacy-data | Security: vulns; privacy-data: PII handling and regulatory compliance |
| audit-security | audit-fault-tolerance | Security: exploitation; fault-tolerance: error handling and resilience |

### 9.5 Recommended Execution Order

Security squad runs early (position 2):

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** ← This squad — Code-level vuln scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Impl quality
5. **audit-test-coverage** — Test assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-consolidated** — Aggregate all findings
13. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip already reported findings.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in standardized Section 7 format for aggregation.