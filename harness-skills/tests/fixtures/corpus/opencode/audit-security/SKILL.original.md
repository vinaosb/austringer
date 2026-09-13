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

- Input validation audit — unsanitized user input, missing bounds checks, SQL/XSS/command injection vectors
- Exposed credentials detection — hardcoded API keys, passwords in source, secrets in config files
- SQL injection — raw queries with string interpolation/concatenation, FromSqlRaw with unsanitized input
- XSS and output encoding — innerHTML with user data, unescaped template output, CSV formula injection
- Command injection — subprocess shell=True, Process.Start with user input, os.system calls
- Cryptographic issues — weak hash algorithms (MD5/SHA1), AES-ECB mode, hardcoded salts/IVs, insecure RNG
- Authentication/authorization flaws — missing [Authorize], blanket [AllowAnonymous], CSRF token absence
- Insecure deserialization — pickle.loads on untrusted data, BinaryFormatter, TypeNameHandling.Auto
- Information leakage — verbose error messages in production, stack traces in API responses, PII in logs
- Dependency vulnerability scanning — known CVEs in requirements/package manifests (shallow; use audit-dependencies for deep analysis)
- Security header audit — missing CSP, HSTS, X-Content-Type-Options, insecure cookie flags
- **.NET/C#**: Analyzer warning violations related to security (CA3001-3008, CA3075, CA3125), SqlClient encryption, anti-forgery token validation, TLS version enforcement
- **Python**: Bandit security linting, SQLAlchemy text() injection, Jinja2 template escaping, session cookie configuration
- **JavaScript/TypeScript**: ESLint security plugin rules, DOMPurify usage, Node.js path traversal, npm audit findings
- **Go**: `go-sec` / `gosec` scanning, SQL injection via `database/sql`, context timeout security implications
- **Java**: Spring Security configuration, PreparedStatement compliance, JCA/JCE provider validation
- **Rust**: Unsafe block audit for security implications, crypto crate audit, input validation in unsafe contexts

### DO NOT USE FOR

- Deep CVE analysis and SBOM generation (use `audit-dependencies`)
- Architectural layer violations (use `audit-architecture`)
- Code style and naming conventions (use `audit-code-quality`)
- CI/CD pipeline security (use `audit-cicd`)
- PII data handling and privacy compliance (use `audit-privacy-data`)
- License compliance (use `audit-license`)
- Fault tolerance and resilience patterns (use `audit-fault-tolerance`)

---

## Section 3: Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

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
| Auth config | `*Auth*`, `*Identity*`, `*Security*`, `*Middleware*` | Authentication/authorization |
| Input handlers | `*Controller*`, `*Handler*`, `*Endpoint*`, `*Service*` | Input validation |
| Crypto code | `*Crypto*`, `*Encrypt*`, `*Hash*`, `*Certificate*` | Cryptographic implementation |
| Config files | `appsettings*.json`, `*.env`, `web.config`, `application.yml` | Exposed secrets |
| DB context | `*DbContext*`, `*Repository*`, `*Context*` | SQL injection vectors |
| DI registration | `*Registration*`, `*Startup*`, `Program.cs` | Service lifetimes, auth middleware |
| Middleware | `*Middleware*`, `*Filter*`, `*Interceptor*` | Request pipeline security |
| Exports | `*Export*`, `*Download*`, `*Report*` | CSV injection, data exposure |
| CI config | `.gitlab-ci.yml`, `.github/workflows/*.yml` | Pipeline secrets |

### 3.3 Security Tool Detection

| Signal | Pattern | Tool |
|--------|---------|------|
| .NET Roslyn analyzers | `*.csproj` containing `Microsoft.CodeAnalysis.*` or `EnforceCodeStyleInBuild` | Security analyzers |
| Bandit | `.bandit`, `pyproject.toml` with `[tool.bandit]` | Python security scanning |
| ESLint security | `.eslintrc` with `eslint-plugin-security` | JS/TS security linting |
| gosec | `gosec` in Makefile or CI config | Go security scanning |
| FindSecBugs | `find-sec-bugs` in `pom.xml` or `build.gradle` | Java security scanning |
| cargo-audit | `cargo audit` in CI config | Rust dependency vulnerability scanning |
| Trivy | `.trivyignore`, `trivy` in CI config | Container/filesystem vulnerability scanning |

### 3.4 Infrastructure Detection (Security-Relevant)

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml` | Container security |
| Secret manager | `vault`, `openbao`, `aws secretsmanager` in config | Credential management |
| TLS certs | `*.pem`, `*.crt`, `*.key`, `*.pfx` | Certificate management |
| CSP headers | `Content-Security-Policy` in web config | CSP enforcement |
| HSTS | `Strict-Transport-Security` in web config | HTTPS enforcement |

### 3.5 Discovery Procedure

1. **Scan root directory** for all file patterns above.
2. **Identify primary language** by source file count.
3. **Identify frameworks** by config file analysis (see shared template Section 3.2).
4. **Locate security-relevant directories**: auth, middleware, crypto, config, handlers.
5. **Check for secret management**: Vault/OpenBao integration, environment variable usage, `.env.example`.
6. **Identify security tooling**: SAST scanners, secret scanners, dependency scanners.
7. **Check for security headers**: CSP, HSTS, X-Content-Type-Options in middleware/config.
8. **Select the matching checklist** from Section 4 based on detected technologies.
9. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Security Checklist

- [ ] **SQL injection via FromSqlRaw**: Search for `FromSqlRaw`, `FromSqlInterpolated`, `ExecuteSqlRaw`
  with string interpolation or concatenation. All parameters must use the typed parameter overload.
- [ ] **Command injection via Process.Start**: Search for `Process.Start` with user-controllable
  arguments. Use `ArgumentList` instead of string concatenation for `ProcessStartInfo`.
- [ ] **Hardcoded connection strings**: Search `appsettings.json`, `appsettings.*.json` for connection
  strings committed to git. Connection strings should reference environment variables or secret manager.
- [ ] **Weak crypto algorithms**: Search for `MD5`, `SHA1`, `AesManaged` with ECB mode, `DESCryptoServiceProvider`,
  `RC2`, `TripleDES`. Use `SHA256`/`SHA512`, `AesGcm` or `AesCbc` with random IV.
- [ ] **[AllowAnonymous] misuse**: Search for `[AllowAnonymous]` on endpoints that handle sensitive
  operations. Every anonymous endpoint should have documented justification.
- [ ] **Missing CSRF/antiforgery tokens**: Verify `[ValidateAntiForgeryToken]` or `AutoValidateAntiforgery`
  on all POST/PUT/DELETE endpoints in MVC/Razor Pages contexts.
- [ ] **Insecure deserialization**: Search for `BinaryFormatter`, `SoapFormatter`, `NetDataContractSerializer`,
  `TypeNameHandling.Auto`/`All` in Newtonsoft.Json. Use `System.Text.Json` with typed deserialization.
- [ ] **CSV formula injection**: Search for CSV export code that doesn't sanitize cells starting with
  `=`, `+`, `-`, `@`, `\t`, `\r`. Prefix dangerous characters with a single quote or escape.
- [ ] **Secrets as plaintext env vars**: Environment variables containing connection strings, API keys,
  or tokens should be validated against secret manager usage. Prefer Vault/OpenBao transit secrets.
- [ ] **Export endpoints lacking per-module RBAC**: Verify export/download endpoints enforce role-based
  access with module-specific claims, not just blanket `[Authorize]`.
- [ ] **PII in log output**: Search for `LogInformation`, `logger.Info` with user data (email, name, SSN,
  phone). Use structured logging with redacted fields or PII-safe identifiers.
- [ ] **Missing security headers**: Verify CSP (with nonce for Blazor), HSTS, X-Content-Type-Options,
  X-Frame-Options configured in middleware pipeline.
- [ ] **HttpClient with TLS validation disabled**: Search for `ServerCertificateCustomValidationCallback`
  returning `true`, or `SslProtocols.None`.
- [ ] **CA1031 suppression without justification**: Search for `#pragma warning disable CA1031` — catch-all
  exception handlers must have documented justification for why specific exceptions cannot be used.
- [ ] **Weak password hashing**: Search for `MD5`/`SHA1`/`SHA256` used for password hashing. Use BCrypt,
  Argon2, or PBKDF2 with per-user salt.
- [ ] **TLS version enforcement**: Search for `ServicePointManager.SecurityProtocol`. Enforce
  `Tls12` or `Tls13` minimum. No `Ssl3` or `Tls` (TLS 1.0).

### 4.2 Python Security Checklist

- [ ] **SQL injection via f-strings/format**: Search for `cursor.execute(f"`, `cursor.execute("...%s" %`,
  `.format(` inside SQL strings. Use parameterized queries: `cursor.execute("SELECT ... WHERE id = %s", (id,))`.
- [ ] **Command injection via shell=True**: Search for `subprocess.run(`, `subprocess.call(`, `subprocess.Popen(`
  with `shell=True`. Use `shell=False` with argument lists.
- [ ] **Hardcoded API keys/tokens**: Search for `api_key =`, `secret =`, `token =`, `password =`
  with literal string values. Use environment variables or secret manager.
- [ ] **Pickle deserialization**: Search for `pickle.loads`, `pickle.load` on data from network/file.
  Use `json.loads` for untrusted data. Pickle is safe only for trusted internal serialization.
- [ ] **Insecure temp files**: Search for `tempfile.mktemp` (race condition). Use `tempfile.mkstemp`
  or `NamedTemporaryFile` with `delete=True`.
- [ ] **eval/exec on user input**: Search for `eval(`, `exec(` with variables containing user data.
  Use `ast.literal_eval()` for literal evaluation, or explicit parsing.
- [ ] **JWT without verification**: Search for `jwt.decode(` without `algorithms` parameter or with
  `verify=False`. Always specify algorithms and verify signature.
- [ ] **Flask DEBUG mode in production**: Search for `app.run(debug=True)` or `DEBUG = True` in
  production config. Use environment-variable-driven config.
- [ ] **SQLAlchemy text() with string concatenation**: Search for `text(f"`, `text("...{}".format(`.
  Use `text("...WHERE id = :id")` with bind parameters.
- [ ] **Missing rate limiting**: Verify API endpoints have rate limiting middleware for
  authentication, registration, and password reset routes.
- [ ] **Insecure cookie flags**: Verify `SESSION_COOKIE_SECURE = True`, `SESSION_COOKIE_HTTPONLY = True`
  in Django/Flask config.
- [ ] **Jinja2 autoescape disabled**: Search for `autoescape=False` in Jinja2 template config.
  Keep autoescape enabled or manually escape user data.
- [ ] **YAML load without SafeLoader**: Search for `yaml.load(` without `Loader=yaml.SafeLoader`.
  Prefer `yaml.safe_load()` to avoid arbitrary code execution.
- [ ] **XXE in XML parsing**: Search for `xml.etree.ElementTree.parse` or `lxml.etree.parse`
  without disabling external entities.

### 4.3 JavaScript/TypeScript Security Checklist

- [ ] **XSS via innerHTML**: Search for `.innerHTML =` with variables containing user input.
  Use `.textContent`, `.innerText`, or DOMPurify sanitization.
- [ ] **eval() usage**: Search for `eval(`, `new Function(`, `setTimeout(string)`, `setInterval(string)`.
  Replace with safe alternatives (JSON.parse, callback functions).
- [ ] **Prototype pollution**: Search for `Object.assign({}, userInput)`, `_.merge(target, userInput)`,
  `deepExtend` without property validation. Validate keys against whitelist.
- [ ] **DOM-based injection**: Search for `document.write(`, `location.href = userInput`,
  `document.cookie` reads. Use safe DOM APIs and HttpOnly cookies.
- [ ] **Insecure cookie flags**: Verify `Secure`, `HttpOnly`, `SameSite=Strict/Lax` on all
  session/authentication cookies. No cookies with `SameSite=None` without `Secure`.
- [ ] **SQL injection in ORM raw queries**: Search for `sequelize.query(`, `knex.raw(` with string
  concatenation. Use parameterized bindings.
- [ ] **Client-side secrets**: Search for `apiKey`, `apiSecret`, `password` in `.js`/`.ts` source
  files. API keys should be server-side only; use proxy endpoints.
- [ ] **Missing Content Security Policy**: Verify CSP header or meta tag is set. No `unsafe-inline`
  or `unsafe-eval` without nonce/hash justification.
- [ ] **JWT stored in localStorage**: Tokens in localStorage are accessible to XSS. Use HttpOnly
  cookies for JWT storage, or short-lived tokens with refresh rotation.
- [ ] **Unvalidated redirects**: Search for `res.redirect(userInput)`, `window.location = userInput`.
  Validate redirect URLs against whitelist.
- [ ] **Dependency injection for security-relevant services**: Verify `helmet` (Express),
  `express-rate-limit`, `csurf` or equivalent are configured.
- [ ] **GraphQL introspection enabled in production**: Search for `introspection: true` in production
  GraphQL config. Disable introspection in production.
- [ ] **Server-side request forgery (SSRF)**: Search for `fetch(userInput)`, `axios.get(userInput)` where
  user controls the URL. Validate against allowlist.

### 4.4 Go Security Checklist

- [ ] **SQL injection via fmt.Sprintf**: Search for `fmt.Sprintf` inside SQL query strings.
  Use `database/sql` parameterized queries with `$1`, `$2` placeholders.
- [ ] **Command injection via exec.Command**: Search for `exec.Command` with string concatenation.
  Pass arguments as separate parameters, never shell-interpolated strings.
- [ ] **Path traversal**: Search for `os.Open(userInput)`, `filepath.Join(baseDir, userInput)`.
  Validate paths with `filepath.Abs` and prefix check against allowed base directory.
- [ ] **Hardcoded credentials**: Search for `password =`, `api_key =`, `secret =` with literal strings.
  Use environment variables or secret manager.
- [ ] **JWT without signature verification**: Search for `jwt.Parse` without key function or with
  `jwt.WithoutSignatureValidation()`. Always validate signature.
- [ ] **Missing input validation in HTTP handlers**: Search for `r.URL.Query().Get("` without
  subsequent length/format validation.
- [ ] **HTTPS not enforced**: Verify TLS configuration on HTTP server. Search for `http.ListenAndServe`
  without TLS vs `http.ListenAndServeTLS`.
- [ ] **Insecure random number generation**: Search for `rand.Intn` used for security-sensitive values
  (tokens, passwords). Use `crypto/rand` instead.
- [ ] **CORS misconfiguration**: Search for `AllowAllOrigins()` or `Access-Control-Allow-Origin: *`
  with credentials. Restrict origins to known domains.

### 4.5 Java Security Checklist

- [ ] **SQL injection via string concatenation**: Search for `"SELECT ... " + userInput` in JDBC calls.
  Use `PreparedStatement` with bind parameters.
- [ ] **Insecure deserialization**: Search for `ObjectInputStream.readObject()` on untrusted data.
  Use JSON serialization or `ValidatingObjectInputStream` with class whitelist.
- [ ] **Hardcoded JNDI strings**: Search for JNDI lookup strings with hardcoded URLs.
  Use environment-driven configuration.
- [ ] **XXE injection**: Verify XML parsers have `FEATURE_SECURE_PROCESSING` enabled and external
  entities disabled. Use `DocumentBuilderFactory.setFeature(XMLConstants.FEATURE_SECURE_PROCESSING, true)`.
- [ ] **Spring Security configuration**: Verify `@PreAuthorize`, `@Secured`, or `@RolesAllowed` on
  sensitive endpoints. Check `SecurityFilterChain` order.
- [ ] **Missing CSRF protection in Spring**: Verify `http.csrf()` is not disabled without justification
  for stateless APIs.
- [ ] **Sensitive data in toString()**: Search for `toString()` methods that include passwords, tokens,
  or personally identifiable information.
- [ ] **Log injection**: Search for logger calls with user input in message template.
  Use `%s` placeholders, not string concatenation.
- [ ] **Insecure cipher configuration**: Search for `Cipher.getInstance("AES/ECB/"` or `DES` without
  authenticated encryption. Use `AES/GCM/NoPadding` with random IV.

### 4.6 Rust Security Checklist

- [ ] **Unsafe block security audit**: Verify `unsafe` blocks have documented safety invariants.
  No unnecessary dereferences of raw pointers from untrusted sources.
- [ ] **SQL injection via format! in SQL**: Search for `format!(` or `format_args!(` inside query strings.
  Use `sqlx::query!("SELECT ... WHERE id = $1", id)` or `diesel` type-safe queries.
- [ ] **Command injection**: Search for `Command::new` with arguments from user input.
  Use argument list, not shell string.
- [ ] **Hardcoded secrets**: Search for `const` or `static` variables containing keys or passwords.
  Use environment variables or secure config crate.
- [ ] **Timing-safe comparison**: Search for `==` comparison of secrets, tokens, or passwords.
  Use `constant_time_eq` from `subtle` crate.
- [ ] **Insecure randomness**: Search for `rand::thread_rng()` for cryptographic keys/tokens.
  Use `OsRng` from `rand_chacha` or `ring` crate.
- [ ] **Unsafe transmute**: Search for `std::mem::transmute` on untrusted input. Verify type safety.

---

## Section 5: Anti-Patterns

### 5.1 Cross-Language Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Secrets in source code | critical | API keys, passwords, tokens hardcoded in source files | Secrets leak via version control, logs, error reports, code search | Use environment variables, secret manager (Vault, OpenBao, AWS Secrets Manager) |
| 2 | String concatenation in SQL | critical | Building SQL queries with string interpolation/concatenation | SQL injection enables data exfiltration, authentication bypass, data destruction | Parameterized queries with typed parameters |
| 3 | Catch-all without logging | high | Empty catch blocks or catch-all with no logging | Security incidents go undetected; errors silently swallowed | Catch specific exceptions, log with context, rethrow or handle explicitly |
| 4 | Verbose error messages to clients | high | Stack traces, SQL errors, internal paths returned in API responses | Information leakage aids attacker reconnaissance | Generic error responses to clients; detailed errors logged server-side only |
| 5 | Missing input validation | high | No validation/sanitization on user input before processing | Enables injection, overflow, logic bypass attacks | Validate input type, length, format, range at entry point |
| 6 | CSV formula injection | medium | Exporting unescaped `=`, `+`, `-`, `@` in CSV cells | Spreadsheet applications execute formulas when opening CSV files | Prefix dangerous characters with single quote or tab escape |
| 7 | PII in log output | medium | Logging personally identifiable information (email, SSN, phone) | Violates privacy regulations; data exposure in log aggregation | Redact PII in logs; use correlation IDs instead of identifiers |

### 5.2 .NET/C# Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Blanket [AllowAnonymous] | high | Applying [AllowAnonymous] to entire controllers or sensitive endpoints | Bypasses all authentication for those endpoints | Apply to specific actions only with documented justification |
| 2 | Trivy ignore suppressing all CVEs by year range | critical | `.trivyignore` with broad CVE patterns suppressing entire severity ranges | Known critical vulnerabilities ignored in production | Triage individually; suppress only with documented justification and expiry |
| 3 | Health endpoints requiring auth | medium | Health check endpoints behind [Authorize] | Monitoring tools cannot probe health without credentials, breaking alerting | Health check endpoints (basic liveness) should be anonymous; readiness can check auth-protected dependencies |
| 4 | Export endpoints with blanket RequireAuthorization only | medium | Export/download endpoints with generic [Authorize] but no module-specific RBAC | Any authenticated user can export any module's data | Enforce per-module claims/policies on export endpoints |
| 5 | HttpResponseMessage not disposed | medium | Not calling Dispose or not using `using` on HttpResponseMessage | Connection leaks, socket exhaustion under load | Use `using var response = await client.SendAsync(...)` |
| 6 | FromSqlRaw with interpolated string | critical | `FromSqlRaw($"SELECT * FROM table WHERE id = {id}")` | Direct SQL injection via string interpolation | Use parameterized overload: `FromSqlRaw("SELECT ... WHERE id = {0}", id)` |
| 7 | Weak hashing for passwords | critical | Using MD5/SHA1 for password hashing | Trivially reversible; no salting | Use BCrypt, Argon2, or PBKDF2 with per-user salt |

### 5.3 Python Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | shell=True with user input | critical | subprocess.run(cmd, shell=True) where cmd contains user data | Command injection via shell metacharacters | Use shell=False with argument list |
| 2 | pickle.loads on network data | critical | Deserializing untrusted pickle data | Arbitrary code execution | Use json or msgpack for untrusted data |
| 3 | eval() on user input | critical | Dynamically evaluating user-provided strings | Remote code execution | Use ast.literal_eval() or explicit parsing |
| 4 | Flask debug in production | high | app.run(debug=True) or DEBUG=True in production | Interactive debugger exposes source code and allows code execution | Use environment-driven config; disable debug in production |
| 5 | SQL via f-strings | critical | f"SELECT * FROM users WHERE id = {user_id}" | SQL injection through string interpolation | Parameterized queries: cursor.execute("...WHERE id = %s", (user_id,)) |

### 5.4 JavaScript/TypeScript Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | eval() with user input | critical | Executing user-provided strings | Remote code execution | Use JSON.parse for data, template literals for display |
| 2 | innerHTML with user input | high | Setting innerHTML from untrusted source | XSS vulnerability | Use textContent, createElement, or DOMPurify |
| 3 | JWT in localStorage | medium | Storing auth tokens in localStorage | Accessible to any XSS attack; no HttpOnly protection | HttpOnly secure cookies with SameSite |
| 4 | Prototype pollution | high | Merging untrusted objects into prototypes | Can override methods, inject properties, cause DoS | Validate object keys against whitelist before merging |
| 5 | Unvalidated redirects | medium | res.redirect(userInput) | Open redirect for phishing attacks | Validate redirect URLs against whitelist |

### 5.5 Go Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | fmt.Sprintf in SQL queries | critical | Building SQL with Sprintf | SQL injection | Parameterized queries with `$1`, `$2` placeholders |
| 2 | Hardcoded credentials in const/var | critical | API keys, passwords as constants | Leaked in binaries, logs, version control | Environment variables or secret store |
| 3 | Path traversal with os.Open | high | Opening files based on user-controlled path | Unauthorized file access | Validate path with filepath.Abs + prefix check |

### 5.6 Java Security Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | SQL string concatenation in JDBC | critical | "SELECT * FROM ... WHERE id = " + id | SQL injection | PreparedStatement with bind variables |
| 2 | ObjectInputStream.readObject() | critical | Deserializing untrusted data | Remote code execution via gadget chains | JSON serialization or whitelist approach |
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
    // ...
}

// ✅ GOOD: Parameterized query
func getUser(db *sql.DB, email string) (*User, error) {
    query := "SELECT id, email FROM users WHERE email = $1"
    row := db.QueryRow(query, email)
    // ...
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
  "description": "What was found and why it matters. Be specific — include the vulnerable code pattern.",
  "recommendation": "Specific fix with brief code example if applicable.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "security"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the vulnerable code fragment. No hedging ("might be a problem").
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns that span multiple files.
- **category**: Always `security` for this squad.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-security",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 15,
    "bySeverity": {
      "critical": 1,
      "high": 3,
      "medium": 7,
      "low": 4
    },
    "byCategory": {
      "security": 15
    },
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
| **critical** | Immediate exploitation risk, RCE, data breach, or total system compromise | SQL injection, command injection, exposed production credentials, deserialization RCE |
| **high** | Significant security risk with clear attack vector | XSS, path traversal, authentication bypass, weak crypto in production, CSRF on state-changing operations |
| **medium** | Degraded security posture or defense-in-depth gap | Missing input validation, CSV formula injection, verbose error messages, insecure cookie flags |
| **low** | Best practice deviation, defense-in-depth improvement | Missing CSP header on static pages, development-only debug flags, informational logging of auth events |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |
| Finding has known CVE associated | +1 level | Dependency vulnerabilities |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

| Technology | Pattern | Base Severity | Notes |
|-----------|---------|---------------|-------|
| **.NET/C#** | `FromSqlRaw` with interpolation | critical | Direct SQL injection |
| **.NET/C#** | MD5/SHA1 for passwords | critical | Trivially reversible |
| **.NET/C#** | [AllowAnonymous] on sensitive endpoint | high | Auth bypass |
| **.NET/C#** | Missing antiforgery token | high | CSRF vulnerability |
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

- **audit-dependencies**: Provides CVE data for third-party packages. Security squad should cross-reference code findings with dependency CVEs.
- **audit-cicd**: Provides pipeline security context. Security squad may reference pipeline secret handling.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-security | audit-dependencies | Security focuses on code vulnerabilities; dependencies focuses on CVE/license in third-party packages |
| audit-security | audit-cicd | Security focuses on code; cicd focuses on pipeline configuration and deployment security |
| audit-security | audit-privacy-data | Security focuses on vulnerabilities; privacy-data focuses on PII handling and regulatory compliance |
| audit-security | audit-fault-tolerance | Security focuses on exploitation; fault-tolerance focuses on error handling and resilience |

### 9.5 Recommended Execution Order

Security squad runs early (position 2 in full audit sequence):

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** ← This squad — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
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

1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.