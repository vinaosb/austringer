---
name: keycloak-oidc
description: "Keycloak OIDC authentication and authorization patterns for ASP.NET Core. Cookie + OIDC + JWT Bearer multi-scheme, realm role mapping, admin REST API provisioning, health checks, startup validation. USE FOR: Keycloak integration, OIDC setup, SSO, realm role claims, user provisioning via Keycloak admin API, multi-scheme auth (cookie + bearer), logout flow, token validation. DO NOT USE FOR: IdentityServer, Auth0, Azure AD, non-Keycloak IdP, frontend-only auth."
license: MIT
metadata:
  author: community
  version: "2.0.0"
  frameworks: [".NET", "Keycloak"]
---

# Keycloak OIDC for ASP.NET Core

Keycloak as identity provider for .NET apps.

## Architecture Overview

Three auth schemes:

1. **Cookie** (default) — session after OIDC callback. Blazor Server, MVC, Razor Pages.
2. **OIDC** (challenge) — redirect to Keycloak login. Auth-code flow + PKCE.
3. **JWT Bearer** (additional) — validate access tokens for API clients.

Flow: protected page → OIDC challenge → Keycloak auth → code → tokens → cookie → userinfo → claims.

> JWT Bearer optional. Browser-only Blazor Server: Cookie + OIDC sufficient. API clients (mobile/SPA/third-party) need JWT Bearer.

## ASP.NET Core Configuration

```csharp
builder.Services.AddAuthentication(options =>
{
    options.DefaultScheme = CookieAuthenticationDefaults.AuthenticationScheme;
    options.DefaultChallengeScheme = OpenIdConnectDefaults.AuthenticationScheme;
})
.AddCookie()
.AddOpenIdConnect(options =>
{
    options.Authority = configuration["Keycloak:Authority"];
    options.ClientId = configuration["Keycloak:ClientId"];
    options.ClientSecret = configuration["Keycloak:ClientSecret"];
    options.ResponseType = "code";
    options.SaveTokens = true;
    options.GetClaimsFromUserInfoEndpoint = true;

    options.Scope.Add("openid");
    options.Scope.Add("profile");
    options.Scope.Add("email");
    options.Scope.Add("roles");

    options.TokenValidationParameters = new TokenValidationParameters
    {
        NameClaimType = "preferred_username",
        RoleClaimType = "realm_roles",
        ValidIssuer = configuration["Keycloak:Authority"],
        ValidAudience = configuration["Keycloak:ClientId"],
    };
});
```

`SaveTokens = true` → tokens in cookie. `GetClaimsFromUserInfoEndpoint = true` → profile claims merged.

## Realm Role → ClaimTypes.Role Mapping

Keycloak puts roles in `realm_roles` claim. `[Authorize(Roles = "...")]` expects `ClaimTypes.Role`. Bridge via `RoleClaimType = "realm_roles"` on `TokenValidationParameters` (simplest).

Finer control via `OnTokenValidated`:

```csharp
options.Events.OnTokenValidated = ctx =>
{
    var realmRoles = ctx.Principal?.FindAll("realm_roles").ToList();
    if (realmRoles is not null)
    {
        var identity = ctx.Principal!.Identity as ClaimsIdentity;
        foreach (var role in realmRoles)
            identity?.AddClaim(new Claim(ClaimTypes.Role, role.Value));
    }
    return Task.CompletedTask;
};
```

Post-validation, pre-cookie. Same pattern for `resource_access.{clientId}.roles`.

### Claims Transformation for Multi-Tenancy

Shared realm, multiple tenants → centralized `IClaimsTransformation`:

```csharp
public sealed class KeycloakClaimsTransformation : IClaimsTransformation
{
    private const string RealmAccessClaimType = "realm_access";
    private const string IssuerPattern = "/realms/";

    public Task<ClaimsPrincipal> TransformAsync(ClaimsPrincipal principal)
    {
        var identity = principal.Identity as ClaimsIdentity;
        if (identity is null || !identity.IsAuthenticated)
            return Task.FromResult(principal);

        if (identity.HasClaim(c => c.Type == "tenant_id"))
            return Task.FromResult(principal);

        // 1. realm_access → ClaimTypes.Role
        var realmAccessClaim = identity.FindFirst(RealmAccessClaimType);
        if (realmAccessClaim is not null)
        {
            var realmAccess = JsonSerializer.Deserialize<RealmAccess>(realmAccessClaim.Value);
            if (realmAccess?.Roles is not null)
                foreach (var role in realmAccess.Roles)
                    identity.AddClaim(new Claim(ClaimTypes.Role, role));
        }

        // 2. Issuer → tenant_id
        var issuer = identity.FindFirst("iss")?.Value ?? identity.FindFirst("aud")?.Value;
        if (issuer is not null)
        {
            var realmName = ExtractRealmFromIssuer(issuer);
            if (realmName is not null)
                identity.AddClaim(new Claim("tenant_id", realmName));
        }

        // 3. Custom attributes
        var country = identity.FindFirst("country")?.Value;
        if (country is not null)
            identity.AddClaim(new Claim("user_country", country));

        return Task.FromResult(principal);
    }

    private static string? ExtractRealmFromIssuer(string issuer)
    {
        var index = issuer.IndexOf(IssuerPattern, StringComparison.Ordinal);
        if (index < 0) return null;
        var realm = issuer[(index + IssuerPattern.Length)..];
        return string.IsNullOrWhiteSpace(realm) ? null : realm.TrimEnd('/');
    }

    private sealed record RealmAccess(List<string>? Roles);
}
```

```csharp
builder.Services.AddScoped<IClaimsTransformation, KeycloakClaimsTransformation>();
```

Runs every request. `tenant_id` guard prevents duplicate injection.

## JWT Bearer for API Endpoints

Second scheme for non-browser clients:

```csharp
builder.Services.AddAuthentication()
    .AddJwtBearer(options =>
    {
        options.Authority = configuration["Keycloak:Authority"];
        options.Audience = configuration["Keycloak:ClientId"];
        options.TokenValidationParameters = new TokenValidationParameters
        {
            RoleClaimType = "realm_roles",
            ValidIssuer = configuration["Keycloak:Authority"],
            ValidAudience = configuration["Keycloak:ClientId"],
        };
    });
```

> `Microsoft.AspNetCore.Authentication.JwtBearer` NOT in `Microsoft.AspNetCore.App`. Add explicit NuGet.

Controllers:
```csharp
[Authorize(AuthenticationSchemes = "Bearer,Cookie")]
public class MyApiController : ControllerBase { }
```

Minimal APIs:
```csharp
app.MapGet("/api/data", () => Results.Ok())
   .RequireAuthorization(new[] { "Bearer", "Cookie" });
```

### ApiScheme Policy

```csharp
builder.Services.AddAuthorization(options =>
{
    options.AddPolicy("ApiScheme", policy =>
    {
        policy.AddAuthenticationSchemes("Bearer");
        policy.RequireAuthenticatedUser();
    });
});
```

`.RequireAuthorization("ApiScheme")`. Note: `/api/auth/*` endpoints handle own auth — can't blanket-apply to `MapAuthEndpoints()`.

## Keycloak Admin REST API: User Provisioning

Password grant → admin token. Server-to-server only.

```csharp
public interface IKeycloakAdminService
{
    Task<string> CreateUserAsync(string email, string firstName, string lastName,
        string temporaryPassword, string realmRole);
    Task DisableUserAsync(string keycloakUserId);
    Task AssignRealmRoleAsync(string keycloakUserId, string roleName);
}
```

| Action | Method | Path |
|--------|--------|------|
| Create user | POST | `/admin/realms/{realm}/users` |
| Set password | PUT | `/admin/realms/{realm}/users/{id}/reset-password` |
| Assign role | POST | `/admin/realms/{realm}/users/{id}/role-mappings/realm` |
| Get by email | GET | `/admin/realms/{realm}/users?email={email}` |
| Disable | UPDATE | `/admin/realms/{realm}/users/{id}` (`enabled: false`) |

```csharp
builder.Services.AddHttpClient<IKeycloakAdminService, KeycloakAdminService>();
builder.Services.Configure<KeycloakAdminOptions>(configuration.GetSection("Keycloak:AdminApi"));
```

### Finbuckle Tenant Resolution

```csharp
services.AddMultiTenant<TenantInfo>()
    .WithClaimStrategy("tenant_id")
    .WithConfigurationStore();

services.AddTransient<IClaimsTransformation, KeycloakClaimsTransformation>();
```

`WithClaimStrategy("tenant_id")` resolves tenant from claim. Must map as Keycloak user attribute → ID token via protocol mapper.

## Configuration Pattern

`appsettings.json` (committed, empty):
```json
{
  "Keycloak": {
    "Authority": "",
    "ClientId": "",
    "ClientSecret": "",
    "RequireHttpsMetadata": true,
    "AdminApi": {
      "BaseUrl": "",
      "ClientId": "admin-cli",
      "ClientSecret": "",
      "Username": "",
      "Password": ""
    }
  }
}
```

Dev: `appsettings.Development.json` (gitignored). Prod: env vars / secrets manager.

## Startup Validation

```csharp
var authority = configuration["Keycloak:Authority"];
var clientId = configuration["Keycloak:ClientId"];

if (string.IsNullOrWhiteSpace(authority))
    throw new InvalidOperationException("Keycloak:Authority is required.");

if (string.IsNullOrWhiteSpace(clientId))
    throw new InvalidOperationException("Keycloak:ClientId is required.");
```

In `StartupValidator` or inline after `builder.Build()`. Fail fast.

## Health Check

```csharp
builder.Services.AddHealthChecks()
    .AddUrlGroup(
        new Uri($"{authority}/.well-known/openid-configuration"),
        "keycloak", tags: ["ready"]);
```

```csharp
app.MapHealthChecks("/health/live", new HealthCheckOptions { Predicate = _ => false });
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready")
});
```

`/health/live` = process alive. `/health/ready` = dependencies reachable.

## Logout Flow

Local cookie signout + Keycloak end_session redirect:

```csharp
public async Task Logout(HttpContext context, IConfiguration configuration)
{
    await context.SignOutAsync(CookieAuthenticationDefaults.AuthenticationScheme);

    var authority = configuration["Keycloak:Authority"]!;
    var redirectUri = Uri.EscapeDataString("https://yourapp.com");

    var logoutUrl =
        $"{authority}/protocol/openid-connect/logout" +
        $"?client_id={configuration["Keycloak:ClientId"]}" +
        $"&post_logout_redirect_uri={redirectUri}";

    context.Response.Redirect(logoutUrl);
}
```

Without Keycloak redirect → session alive → silent re-auth on next visit.

> Use `OnRedirectToIdentityProviderForSignOut` instead of `context.Response.Redirect()` — latter blocks OIDC flow:
> ```csharp
> .AddOpenIdConnect(options =>
> {
>     options.Events.OnRedirectToIdentityProviderForSignOut = ctx =>
>     {
>         ctx.ProtocolMessage.IssuerAddress =
>             $"{authority}/protocol/openid-connect/logout";
>         return Task.CompletedTask;
>     };
> });
> ```

## Token Refresh / Sliding Expiration

OIDC middleware auto-refreshes with `SaveTokens = true`. Cookie stores refresh token.

```csharp
var accessToken = await context.HttpContext.GetTokenAsync("access_token");
var refreshToken = await context.HttpContext.GetTokenAsync("refresh_token");
```

Sliding (keep alive during use):
```csharp
.AddCookie(options =>
{
    options.SlidingExpiration = true;
    options.ExpireTimeSpan = TimeSpan.FromHours(8);
});
```

Absolute → force periodic re-auth, limits compromised session window.

## PKCE

Default in ASP.NET Core OIDC handler. `code_challenge` on auth request, verified on exchange. No config needed.

## DI Registration Pattern

```csharp
public static class KeycloakServiceExtensions
{
    public static IServiceCollection AddKeycloakServices(
        this IServiceCollection services, IConfiguration configuration)
    {
        var authority = configuration["Keycloak:Authority"]
            ?? throw new InvalidOperationException("Keycloak:Authority is required.");

        services.AddAuthentication(/* cookie + oidc + bearer */);
        services.AddHttpClient<IKeycloakAdminService, KeycloakAdminService>();
        services.Configure<KeycloakAdminOptions>(
            configuration.GetSection("Keycloak:AdminApi"));
        services.AddHealthChecks()
            .AddUrlGroup(new Uri($"{authority}/.well-known/openid-configuration"), "keycloak");

        return services;
    }
}
```

`builder.Services.AddKeycloakServices(builder.Configuration);`

## Multi-App SSO with Shared DataProtection

Multiple apps sharing one Keycloak realm → share DataProtection keys + identical cookie settings. Without: App A cookie → App B can't decrypt → SSO breaks.

### Prerequisites

- Same root domain (e.g. `app1.mycompany.com`, `app2.mycompany.com`).
- Same Redis for DataProtection.
- Same realm, different ClientIds.

### Shared DataProtection via Redis

```xml
<PackageReference Include="Microsoft.AspNetCore.DataProtection.StackExchangeRedis" Version="*" />
```

```csharp
var redis = ConnectionMultiplexer.Connect(configuration.GetConnectionString("Redis")!);

builder.Services.AddDataProtection()
    .PersistKeysToStackExchangeRedis(redis, "DataProtection-Keys")
    .SetApplicationName("MySharedAuthApp");
```

- `PersistKeysToStackExchangeRedis` → shared key ring.
- `SetApplicationName` **identical** across apps. Scopes key ring.
- Redis key name same in every app.

### Cookie Configuration

Same cookie name → browser sends to all subdomains:

```csharp
builder.Services.AddAuthentication(options =>
{
    options.DefaultScheme = CookieAuthenticationDefaults.AuthenticationScheme;
    options.DefaultChallengeScheme = OpenIdConnectDefaults.AuthenticationScheme;
})
.AddCookie(options =>
{
    options.Cookie.Name = ".MySharedAuth";
    options.Cookie.Domain = ".mycompany.com";
    options.Cookie.SameSite = SameSiteMode.Lax;
    options.Cookie.HttpOnly = true;
    options.Cookie.SecurePolicy = CookieSecurePolicy.SameAsRequest;
    options.SlidingExpiration = true;
    options.ExpireTimeSpan = TimeSpan.FromHours(8);
})
.AddOpenIdConnect(options =>
{
    options.Authority = configuration["Keycloak:Authority"];
    options.ClientId = configuration["Keycloak:ClientId"];
    options.ClientSecret = configuration["Keycloak:ClientSecret"];
    options.ResponseType = "code";
    options.SaveTokens = true;
    options.GetClaimsFromUserInfoEndpoint = true;

    options.Scope.Add("openid");
    options.Scope.Add("profile");
    options.Scope.Add("email");
    options.Scope.Add("roles");

    options.TokenValidationParameters = new TokenValidationParameters
    {
        NameClaimType = "preferred_username",
        RoleClaimType = "realm_roles",
        ValidIssuer = configuration["Keycloak:Authority"],
        ValidAudience = configuration["Keycloak:ClientId"],
    };
});
```

### Per-App ClientId

Each app reads own `ClientId` from `appsettings.json`:

**App A:** `"ClientId": "myapp-launchpad"` → **App B:** `"ClientId": "myapp-accounting"`

Both point to same realm, different clients. App A auth → shared cookie → App B decrypts → authenticated.

### Per-App Client Configuration

One public client per app. Realm roles shared.

```json
{
  "realm": "myrealm",
  "enabled": true,
  "clients": [
    {
      "clientId": "myapp-launchpad",
      "publicClient": true,
      "redirectUris": ["https://launchpad.mycompany.com/signin-oidc"],
      "webOrigins": ["https://launchpad.mycompany.com"],
      "standardFlowEnabled": true,
      "directAccessGrantsEnabled": false
    },
    {
      "clientId": "myapp-accounting",
      "publicClient": true,
      "redirectUris": ["https://accounting.mycompany.com/signin-oidc"],
      "webOrigins": ["https://accounting.mycompany.com"],
      "standardFlowEnabled": true,
      "directAccessGrantsEnabled": false
    },
    {
      "clientId": "myapp-payments",
      "publicClient": true,
      "redirectUris": ["https://payments.mycompany.com/signin-oidc"],
      "webOrigins": ["https://payments.mycompany.com"],
      "standardFlowEnabled": true,
      "directAccessGrantsEnabled": false
    }
  ],
  "roles": {
    "realm": [
      { "name": "admin", "description": "Full access" },
      { "name": "accountant", "description": "Accounting module" },
      { "name": "viewer", "description": "Read-only" }
    ]
  }
}
```

Docker import — mount + `--import-realm`:
```yaml
services:
  keycloak:
    image: quay.io/keycloak/keycloak:26.0
    command: start-dev --import-realm
    environment:
      KC_DB: postgres
      KC_DB_URL: jdbc:postgresql://db:5432/keycloak
      KC_DB_USERNAME: keycloak
      KC_DB_PASSWORD: keycloak_password
      KEYCLOAK_ADMIN: admin
      KEYCLOAK_ADMIN_PASSWORD: admin
    volumes:
      - ./keycloak/realm-export.json:/opt/keycloak/data/import/realm-export.json
    ports:
      - "8080:8080"
```

- Same realm = shared user store + roles.
- `publicClient: true` avoids secrets for server-side Blazor (PKCE).
- `--import-realm` required.
- Prod: `start` + TLS (not `start-dev`).

## Anti-Patterns

- **NEVER** password grant for user login. Server-to-server admin only.
- **NEVER** commit admin creds/secrets/tokens. Env vars / vault only.
- **NEVER** `SkipUnrecognizedRequests = true` or disable token validation.
- **NEVER** log OIDC tokens, refresh tokens, admin credentials.
- **NEVER** `start-dev` in production. No TLS, no caching, in-memory DB.
- **NEVER** hardcode realm URLs / client IDs. Always configuration.
- **DO NOT** `[AllowAnonymous]` as dev shortcut. Ships to prod.
- **DO NOT** client-side-only role checks. Validate server-side.
- **DO NOT** different `SetApplicationName` across SSO apps. Cookies undecryptable.
- **DO NOT** hardcode client IDs in `Program.cs`. Use `appsettings.json`.
- **DO NOT** skip `--import-realm`. Empty `master` realm results.
- **DO NOT** different cookie names/domains across SSO apps. Breaks SSO.
- **NEVER** skip `KeycloakClaimsTransformation` registration. `realm_roles` won't map → `[Authorize(Roles)]` silently fails.
- **NEVER** hardcode tenant IDs. `WithClaimStrategy(configuration["MultiTenancy:ClaimKey"] ?? "tenant_id")`.

## Related Skills

- **dotnet-clean-arch** — Keycloak in Program.cs, auth in InfrastructureModule
- **docker-deployment** — Docker Compose + Keycloak multi-app
- **blazor-mudblazor** — Auth state revalidation, per-app NavMenu
- **lgpd-data-privacy** — User identity tracking via Keycloak ID, consent lifecycle
- **aspnetcore-health** — Keycloak health via OIDC discovery probe
- **signalr-realtime** — Hub auth with same cookie+OIDC+bearer
