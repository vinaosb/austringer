---
name: blazor-mudblazor
description: "Blazor + MudBlazor patterns for dark mode, theming, JS interop, shared components, WebRTC video sessions, localization, responsive layout, performance, and accessibility. USE FOR: dark mode implementation, MudBlazor theming, JS interop, CSS variable theming, shared component libraries, Blazor accessibility, focus management, WebRTC video sessions, localization/i18n, responsive layout, performance optimization, virtualized lists. DO NOT USE FOR: backend logic, database operations, API design, non-Blazor projects."
license: MIT
metadata:
  author: community
  version: "2.0.0"
  frameworks: ["Blazor", "MudBlazor"]
---

# Blazor + MudBlazor Patterns

## 1. Dark Mode Implementation

All parts are mandatory. Skipping any causes flash or broken theming.

**ThemeProvider setup** (MudBlazor v8+):
```razor
<MudThemeProvider @bind-IsDarkMode="isDarkMode" ObserveSystemDarkModeChange="false" />
```
`ObserveSystemDarkModeChange="false"` prevents OS preference from overriding the user's explicit choice.

**localStorage persistence** via `IJSRuntime`:
```csharp
private async Task ToggleDarkMode()
{
    isDarkMode = !isDarkMode;
    var state = isDarkMode ? "dark" : "light";
    await JS.InvokeVoidAsync("localStorage.setItem", "app-dark-mode", state);
    await JS.InvokeVoidAsync("document.documentElement.setAttribute", "data-mud-theme", state);
}
```

**Startup initialization** — read saved preference on load:
```csharp
protected override async Task OnInitializedAsync()
{
    var saved = await JS.InvokeAsync<string>("localStorage.getItem", "app-dark-mode");
    if (saved == "dark") isDarkMode = true;
}
```

**Flash prevention** (CRITICAL): inline `<script>` in `<head>` BEFORE any CSS `<link>`:
```html
<script>
  (function() {
    var t = localStorage.getItem('app-dark-mode');
    if (t === 'dark') document.documentElement.setAttribute('data-mud-theme', 'dark');
  })();
</script>
```
Without this, users see a white flash on every dark mode page load. The script runs synchronously before the renderer, so there's no FOUC.

**CSS variables**: define at `:root` (light defaults) and `[data-mud-theme="dark"]` (dark overrides):
```css
:root { --app-surface: #ffffff; --app-text: #1a1a2e; }
[data-mud-theme="dark"] { --app-surface: #1e1e2f; --app-text: #e0e0e0; }
```
Components reference `var(--app-*)` instead of hardcoded colors.

**PWA note**: Standalone WASM apps may not load app.css. ThemeProvider handles switching, but flash prevention in `index.html` is still required.

### Design System Theme with CSS Variables

Define a centralized `MudTheme` with coordinated palettes, typography, and layout. Expose design tokens as CSS custom properties so any component — MudBlazor or custom — references the same variables.

**Theme class** (in Shared RCL):
```csharp
public static class AppTheme
{
    public static MudTheme Default => new()
    {
        PaletteLight = new PaletteLight
        {
            Primary = "#1565C0",
            Secondary = "#42A5F5",
            Background = "#FAFAFA",
            Surface = "#FFFFFF",
            TextPrimary = "#212121",
            TextSecondary = "#757575",
            AppbarBackground = "#1565C0",
            DrawerBackground = "#FFFFFF",
            DrawerText = "#424242",
            Success = "#2E7D32",
            Warning = "#F57F17",
            Error = "#C62828",
            Info = "#0277BD"
        },
        PaletteDark = new PaletteDark
        {
            Primary = "#90CAF9",
            Secondary = "#64B5F6",
            Background = "#121212",
            Surface = "#1E1E1E",
            TextPrimary = "#E0E0E0",
            TextSecondary = "#9E9E9E",
            AppbarBackground = "#1A237E",
            DrawerBackground = "#1E1E1E",
            DrawerText = "#BDBDBD",
            Success = "#66BB6A",
            Warning = "#FFA726",
            Error = "#EF5350",
            Info = "#29B6F6"
        },
        Typography = new Typography
        {
            Default = new DefaultTypography { FontFamily = ["'Inter'", "sans-serif"] },
            H1 = new H1 { FontSize = "2.5rem", FontWeight = 700 },
            H2 = new H2 { FontSize = "2rem", FontWeight = 600 },
            H3 = new H3 { FontSize = "1.75rem", FontWeight = 600 },
            H4 = new H4 { FontSize = "1.5rem", FontWeight = 600 },
            H5 = new H5 { FontSize = "1.25rem", FontWeight = 500 },
            H6 = new H6 { FontSize = "1rem", FontWeight = 500 }
        },
        LayoutProperties = new LayoutProperties
        {
            DefaultBorderRadius = "8px",
            DrawerWidth = "260px",
            AppbarHeight = "64px"
        }
    };
}
```

**CSS custom properties** (in Shared RCL `wwwroot/css/app.css`):
```css
:root {
    /* Surfaces */
    --app-surface: #FFFFFF;
    --app-surface-alt: #F5F5F5;
    --app-background: #FAFAFA;
    /* Text */
    --app-text: #212121;
    --app-text-secondary: #757575;
    --app-text-on-primary: #FFFFFF;
    /* Brand */
    --app-primary: #1565C0;
    --app-primary-hover: #1976D2;
    --app-secondary: #42A5F5;
    /* Status */
    --app-success: #2E7D32;
    --app-warning: #F57F17;
    --app-error: #C62828;
    --app-info: #0277BD;
    /* Spacing & shape */
    --app-radius: 8px;
    --app-appbar-height: 64px;
    --app-drawer-width: 260px;
}

[data-mud-theme="dark"] {
    --app-surface: #1E1E1E;
    --app-surface-alt: #2C2C2C;
    --app-background: #121212;
    --app-text: #E0E0E0;
    --app-text-secondary: #9E9E9E;
    --app-text-on-primary: #121212;
    --app-primary: #90CAF9;
    --app-primary-hover: #BBDEFB;
    --app-secondary: #64B5F6;
    --app-success: #66BB6A;
    --app-warning: #FFA726;
    --app-error: #EF5350;
    --app-info: #29B6F6;
}
```

**Usage in Razor** — custom components reference design tokens instead of hardcoded values:
```razor
<MudPaper Class="app-card" Elevation="2">
    <MudText Typo="Typo.h5" Color="Color.Primary">Title</MudText>
    <p style="color: var(--app-text-secondary);">Subtitle</p>
</MudPaper>

<style>
    .app-card {
        background: var(--app-surface);
        border-radius: var(--app-radius);
        padding: 1.5rem;
    }
</style>
```

**Apply theme** in layout:
```razor
<MudThemeProvider Theme="@AppTheme.Default" @bind-IsDarkMode="_isDarkMode"
                  ObserveSystemDarkModeChange="false" />
```

This ensures MudBlazor components and custom CSS share identical color values. When the theme switches, CSS variables update automatically via the `[data-mud-theme]` attribute.

---

## 2. JS Interop (ES Modules)

- All custom JS lives as ES modules under `wwwroot/js/`
- Import via `IJSObjectReference`, not global function calls
- Pattern:
```csharp
@implements IAsyncDisposable
@inject IJSRuntime JS

private IJSObjectReference? _module;

protected override async Task OnAfterRenderAsync(bool firstRender)
{
    if (firstRender)
        _module = await JS.InvokeAsync<IJSObjectReference>("import", "./js/myModule.js");
}

private async Task CallJs() => await _module!.InvokeVoidAsync("doThing", arg);

public async ValueTask DisposeAsync()
{
    if (_module is not null) await _module.DisposeAsync();
}
```

Never call module exports as `window.someFunction()`. Always go through `IJSObjectReference`.

---

## 3. Shared Component Library Rules

When a Shared project is consumed by both Server and WASM clients:

- `[Inject] private ILogger<T>? Logger { get; set; }` with `Logger?.Log(...)`. Never use `null!`.
- No server-side dependencies in Shared: no Infrastructure, no Application, no scoped service references.
- JS interop source files live in the host project's `wwwroot/js/`, not in Shared. Shared components load them at runtime via `IJSObjectReference`.
- Theme class returns a `MudTheme` with light and dark palettes. Both must be WCAG-compliant (see section 5).

### Shared DataProtection for SSO

When multiple Blazor Server apps share a single sign-on session via OIDC, they must share the same DataProtection key ring. Without this, an encrypted cookie produced by App A cannot be decrypted by App B — the authentication cookie appears invalid and the user is redirected back to login.

**Shared hosting extension** (in `{Project}.Hosting`):
```csharp
public static IServiceCollection AddSharedDataProtection(
    this IServiceCollection services, IConfiguration config)
{
    var redis = ConnectionMultiplexer.Connect(
        config.GetConnectionString("Redis")!);

    services.AddDataProtection()
        .SetApplicationName("MySuite")       // MUST be identical across all apps
        .PersistKeysToStackExchangeRedis(redis, "DataProtection-Keys");

    return services;
}
```

**Cookie configuration** — every app must use the same cookie name and `SameSite`:
```csharp
builder.Services.AddAuthentication(oidcOptions =>
{
    oidcOptions.DefaultScheme = CookieAuthenticationDefaults.AuthenticationScheme;
    oidcOptions.DefaultChallengeScheme = OpenIdConnectDefaults.AuthenticationScheme;
})
.AddCookie(options =>
{
    options.Cookie.Name = "MyAuth";         // MUST be identical across all apps
    options.Cookie.SameSite = SameSiteMode.Lax;
    options.Cookie.HttpOnly = true;
    options.Cookie.SecurePolicy = CookieSecurePolicy.SameAsRequest;
    options.ExpireTimeSpan = TimeSpan.FromHours(8);
    options.SlidingExpiration = true;
});
```

**Why this matters**: ASP.NET Core's `IDataProtectionProvider` derives encryption keys from the application name and a local key material. If each app generates its own keys, cookies encrypted by one app are meaningless to another. `SetApplicationName` + shared Redis key storage ensures all apps use the same encryption key material.

**Common failure mode**: App A (port 5101) authenticates the user. Navigation to App B (port 5102) sends the cookie, but App B's DataProtection cannot decrypt it → 401 redirect loop. The fix is always: same `SetApplicationName`, same key store, same cookie name.

---

## 4. MudBlazor Accessibility Quirks

- **Explicit generics required in v8+**: `MudTextField<string>`, `MudChip<string>`, `MudSelect<T>`. Omitting the type parameter breaks accessibility attributes.
- **Known axe-core violations** (MudBlazor defaults):
  - `link-in-text-block`: internal links inside text blocks lack sufficient click target size
  - `color-contrast`: some default palette combinations fail AA on dark backgrounds
  - `label`: `MudTextField` with no `Label` or `aria-label` fails
  - `button-name`: icon-only `MudIconButton` needs `aria-label`
  Document each suppressed rule with the reason. Don't blanket-disable.
- **MudDialog**: set `Autofocus="true"` on the dialog or manually call `.FocusAsync()` on the first input.
- **ThemeProvider** handles theme switching but does NOT prevent flash. Flash prevention is a separate concern (see section 1).

---

## 5. WCAG-Compliant Theme Palette

- Light mode: test every text/background pair for 7:1 (AAA) or 4.5:1 (AA) contrast.
- Dark mode: test again. Colors that pass in light often fail in dark due to luminance shifts.
- Hover/active states: never lighten a borderline pair. It may drop below threshold.
- Contrast ratio formula:
  ```
  L = 0.2126*R + 0.7152*G + 0.0722*B  (linearized sRGB)
  ratio = (L1 + 0.05) / (L2 + 0.05)   where L1 > L2
  ```
- Use browser DevTools contrast checker or axe-core during development. Automate in CI if possible.

---

## 6. Focus Management

- Programmatic focus: `@ref="myInput"` then `await myInput.FocusAsync()`.
- MudDialog: `Autofocus="true"` moves focus to the dialog on open.
- Route changes: move focus to the `<main>` or content heading so screen readers announce the new page.
- Dynamic content updates: use `aria-live="polite"` regions for non-urgent announcements, `aria-live="assertive"` for critical ones.
- Modal traps: MudDialog handles focus trap automatically. Custom modals need manual trap implementation.

---

## 7. Error Boundaries

Wrap pages or layouts in `<ErrorBoundary>` for graceful degradation:
```razor
<ErrorBoundary>
    <ChildContent>
        <Router ... />
    </ChildContent>
    <ErrorContent Context="ex">
        <p role="alert">An unexpected error has occurred.</p>
    </ErrorContent>
</ErrorBoundary>
```

- Custom `ErrorContent` should use `role="alert"` so screen readers announce the error.
- MudBlazor's `MudSnackbar` or custom toast can also surface non-fatal errors.
- Log errors in `ErrorContent` with `ILogger` — never show stack traces to end users.

---

## 8. WebRTC Video Session Pattern

Blazor component wrapping WebRTC via JS interop with a SignalR hub as signaling relay.

**Component state machine**: `Idle → Connecting → Connected → Ended`

**JS interop module** (`webrtc-interop.js`):
```javascript
// Exported functions consumed via IJSObjectReference
export async function initialize(iceServers) {
    return new RTCPeerConnection({ iceServers });
}

export async function startLocalMedia(connection, constraints) {
    const stream = await navigator.mediaDevices.getUserMedia(constraints);
    stream.getTracks().forEach(t => connection.addTrack(t, stream));
    return stream;
}

export async function createOffer(connection) {
    const offer = await connection.createOffer();
    await connection.setLocalDescription(offer);
    return offer;
}

export async function createAnswer(connection, remoteSdp) {
    await connection.setRemoteDescription(remoteSdp);
    const answer = await connection.createAnswer();
    await connection.setLocalDescription(answer);
    return answer;
}

export async function addIceCandidate(connection, candidate) {
    await connection.addIceCandidate(candidate);
}

export function toggleMute(stream, muted) {
    stream.getAudioTracks().forEach(t => t.enabled = !muted);
}

export function toggleCamera(stream, off) {
    stream.getVideoTracks().forEach(t => t.enabled = !off);
}

export async function endCall(connection, stream) {
    stream?.getTracks().forEach(t => t.stop());
    connection?.close();
}
```

**SignalR hub** as signaling relay:
```csharp
[Authorize]
public class SessionHub : Hub
{
    public async Task JoinSession(string sessionId)
        => await Groups.AddToGroupAsync(Context.ConnectionId, sessionId);

    public async Task SendOffer(string sessionId, string sdp)
        => await Clients.Group(sessionId).SendAsync("ReceiveOffer", sdp);

    public async Task SendAnswer(string sessionId, string sdp)
        => await Clients.Group(sessionId).SendAsync("ReceiveAnswer", sdp);

    public async Task SendIceCandidate(string sessionId, object candidate)
        => await Clients.Group(sessionId).SendAsync("ReceiveIceCandidate", candidate);

    public async Task EndCall(string sessionId)
        => await Clients.Group(sessionId).SendAsync("CallEnded");
}
```

**ICE configuration**: STUN server (`stun:stun.l.google.com:19302`) as minimum. Add a TURN server for production environments where symmetric NAT blocks direct peer connections.

**Browser compatibility**: load [adapter.js](https://github.com/webrtcHacks/adapter) as a module import to normalize WebRTC APIs across browsers. It patches `RTCPeerConnection`, `getUserMedia`, and other APIs behind the scenes.

**Security note**: WebRTC media runs entirely in the browser. Audio and video streams travel peer-to-peer between clients. No media data touches the server. The server only relays signaling messages (SDP offers/answers, ICE candidates) through the hub.

---

## 9. Localization (IStringLocalizer)

**Registration** in `Program.cs`:
```csharp
builder.Services.AddLocalization(options => options.ResourcesPath = "Resources");
```

**Resource file naming**: `ComponentName.{culture}.resx`
```
Resources/
  Dashboard.resx          (default/fallback)
  Dashboard.pt-BR.resx
  Dashboard.es.resx
```

**Shared resources** for strings reused across components:
```csharp
public class SharedResource { }
```
With `Resources/SharedResource.resx` and `Resources/SharedResource.pt-BR.resx`.

**Injection** in Razor:
```razor
@inject IStringLocalizer<Dashboard> Loc
<p>@Loc["WelcomeMessage"]</p>
```

**Injection** in Shared project code-behind (nullable pattern):
```csharp
[Inject] private IStringLocalizer<Dashboard>? Loc { get; set; }

private string GetText(string key) => Loc?[key] ?? key;
```

**Shared project** uses `ResourceManagerStringLocalizerFactory` with the shared resource type:
```csharp
services.AddSingleton(typeof(IStringLocalizer),
    typeof(ResourceManagerStringLocalizerFactory));
```

**Culture management** via `IJSRuntime`:
```csharp
private async Task SetCulture(string culture)
{
    await JS.InvokeVoidAsync("localStorage.setItem", "app-culture", culture);
    await JS.InvokeVoidAsync("document.documentElement.setAttribute", "lang", culture);
}
```

Initialize on startup by reading from localStorage and calling `CultureInfo.DefaultThreadCurrentCulture`.

**Accept-Language header**: configure request culture providers in the middleware pipeline so server-side rendering respects the browser's language preference.

**Fallback chain**: requested culture (e.g., `pt-BR`) → parent culture (`pt`) → default culture. The framework handles this automatically when resource files follow the naming convention.

### JSON-Based I18n (Alternative to IStringLocalizer)

For projects that prefer JSON over `.resx` files, use a custom `I18nService` that loads locale files with dot-notation keys. This avoids the resx compilation step and makes translations easier to edit.

**JSON locale files** (`Resources/pt-BR.json`):
```json
{
    "app": {
        "title": "Meu Aplicativo",
        "welcome": "Bem-vindo, {0}!"
    },
    "nav": {
        "home": "Início",
        "settings": "Configurações",
        "logout": "Sair"
    },
    "validation": {
        "required": "Este campo é obrigatório.",
        "minLength": "Mínimo de {0} caracteres."
    }
}
```

**JSON locale files** (`Resources/es.json`):
```json
{
    "app": {
        "title": "Mi Aplicación",
        "welcome": "¡Bienvenido, {0}!"
    },
    "nav": {
        "home": "Inicio",
        "settings": "Configuración",
        "logout": "Salir"
    },
    "validation": {
        "required": "Este campo es obligatorio.",
        "minLength": "Mínimo de {0} caracteres."
    }
}
```

**I18nService** — loads JSON, resolves dot-notation keys, supports interpolation:
```csharp
public class I18nService
{
    private readonly IWebHostEnvironment _env;
    private Dictionary<string, string>? _translations;
    private string _culture = "pt-BR";

    public event Action? OnChanged;

    public I18nService(IWebHostEnvironment env)
    {
        _env = env;
    }

    public string Culture => _culture;

    public async Task InitializeAsync(string? savedCulture = null)
    {
        if (savedCulture is not null)
            _culture = savedCulture;

        await LoadTranslationsAsync(_culture);
    }

    public async Task SetCultureAsync(string culture)
    {
        _culture = culture;
        await LoadTranslationsAsync(culture);
        OnChanged?.Invoke();
    }

    public string this[string key] => Translate(key);

    public string Translate(string key, params object[] args)
    {
        var value = ResolveKey(key);
        if (value is null)
        {
            // Fallback: requested culture → default "pt-BR" → raw key
            if (_culture != "pt-BR")
            {
                var fallback = ResolveKeyFromCulture("pt-BR", key);
                if (fallback is not null)
                    value = fallback;
            }
            value ??= key;
        }

        return args.Length > 0 ? string.Format(value, args) : value;
    }

    private string? ResolveKey(string key)
    {
        if (_translations is null) return null;
        return _translations.TryGetValue(key, out var val) ? val : null;
    }

    private string? ResolveKeyFromCulture(string culture, string key)
    {
        // Lazy-load fallback culture only when needed
        var dict = LoadTranslationsSync(culture);
        return dict.TryGetValue(key, out var val) ? val : null;
    }

    private async Task LoadTranslationsAsync(string culture)
    {
        var path = Path.Combine(_env.WebRootPath, "Resources", $"{culture}.json");
        if (!File.Exists(path))
            path = Path.Combine(_env.WebRootPath, "Resources", "pt-BR.json");

        var json = await File.ReadAllTextAsync(path);
        var raw = JsonSerializer.Deserialize<Dictionary<string, JsonElement>>(json);
        _translations = Flatten(raw);
    }

    private Dictionary<string, string> LoadTranslationsSync(string culture)
    {
        var path = Path.Combine(_env.WebRootPath, "Resources", $"{culture}.json");
        if (!File.Exists(path))
            path = Path.Combine(_env.WebRootPath, "Resources", "pt-BR.json");

        var json = File.ReadAllText(path);
        var raw = JsonSerializer.Deserialize<Dictionary<string, JsonElement>>(json);
        return Flatten(raw);
    }

    private static Dictionary<string, string> Flatten(
        Dictionary<string, JsonElement>? source, string prefix = "")
    {
        var result = new Dictionary<string, string>();
        if (source is null) return result;

        foreach (var (key, value) in source)
        {
            var fullKey = string.IsNullOrEmpty(prefix) ? key : $"{prefix}.{key}";
            if (value.ValueKind == JsonValueKind.Object)
            {
                var nested = JsonSerializer.Deserialize<Dictionary<string, JsonElement>>(
                    value.GetRawText())!;
                foreach (var (k, v) in Flatten(nested, fullKey))
                    result[k] = v;
            }
            else
            {
                result[fullKey] = value.GetString() ?? fullKey;
            }
        }
        return result;
    }
}
```

**Registration** in `Program.cs`:
```csharp
builder.Services.AddScoped<I18nService>();
```

**Language switcher component**:
```razor
@inject I18nService I18n
@inject IJSRuntime JS

<MudSelect T="string"
           Label="@I18n["nav.settings"]"
           Value="@I18n.Culture"
           ValueChanged="@OnCultureChanged"
           Variant="Variant.Outlined">
    <MudSelectItem Value="@("pt-BR")">Português (BR)</MudSelectItem>
    <MudSelectItem Value="@("es")">Español</MudSelectItem>
    <MudSelectItem Value="@("en")">English</MudSelectItem>
</MudSelect>

@code {
    private async Task OnCultureChanged(string culture)
    {
        await I18n.SetCultureAsync(culture);
        await JS.InvokeVoidAsync("localStorage.setItem", "app-culture", culture);
    }
}
```

**Initialization** in layout or `App.razor`:
```csharp
@inject I18nService I18n
@inject IJSRuntime JS

protected override async Task OnAfterRenderAsync(bool firstRender)
{
    if (firstRender)
    {
        var saved = await JS.InvokeAsync<string?>("localStorage.getItem", "app-culture");
        await I18n.InitializeAsync(saved);
        StateHasChanged();
    }
}
```

**Usage in any component**:
```razor
@inject I18nService I18n

<MudText Typo="Typo.h4">@I18n["app.title"]</MudText>
<MudText>@I18n.Translate("app.welcome", userName)</MudText>
```

**Fallback chain**: `I18n["nav.home"]` resolves as:
1. Look up `nav.home` in current culture JSON (e.g., `es.json`)
2. If missing, look up in default culture (`pt-BR.json`)
3. If still missing, return raw key `nav.home`

### Culture Switcher with Cookie Persistence

For server-side Blazor, persisting culture in a cookie works better than localStorage because the middleware pipeline can read it on every request without JS interop.

**Culture switcher** using `CookieRequestCultureProvider`:
```csharp
public async Task SetCultureAsync(string culture)
{
    var cookie = new KeyValuePair<string, string?>(".AspNetCore.Culture",
        CookieRequestCultureProvider.MakeCookieValue(new RequestCulture(culture)));
    await JSRuntime.InvokeVoidAsync("document.cookie", $"{cookie.Key}={cookie.Value}; path=/; max-age=31536000");

    // Force reload so the new culture takes effect everywhere
    NavigationManager.NavigateTo(NavigationManager.Uri, forceLoad: true);
}
```

**Supported cultures** registered in `Program.cs`:
```csharp
builder.Services.Configure<RequestLocalizationOptions>(options =>
{
    var supportedCultures = new[]
    {
        new CultureInfo("en"),
        new CultureInfo("pt-BR"),
        new CultureInfo("ar") // RTL
    };
    options.DefaultRequestCulture = new RequestCulture("en");
    options.SupportedCultures = supportedCultures;
    options.SupportedUICultures = supportedCultures;
});
```

Then add the middleware in the pipeline: `app.UseRequestLocalization();`

This approach survives page reloads, works across tabs, and doesn't depend on JS running before the first render. The `forceLoad: true` navigation forces a full page reload so server-side resources pick up the new culture immediately.

---

## 10. Responsive Layout (MudGrid Breakpoints)

MudBlazor v8 provides a 12-column grid system with five breakpoints:

```razor
<MudGrid>
    <MudItem xs="12" sm="6" md="4" lg="3" xl="2">
        <MudCard>...</MudCard>
    </MudItem>
</MudGrid>
```

**Breakpoint reference**:

| Breakpoint | Min Width | Target          |
|-----------|-----------|-----------------|
| `xs`      | 0px       | Mobile portrait |
| `sm`      | 600px     | Mobile landscape |
| `md`      | 960px     | Tablet           |
| `lg`      | 1280px    | Desktop          |
| `xl`      | 1920px    | Large desktop    |

**Mobile-first approach**: define `xs` behavior as the baseline, then layer on breakpoint overrides:
```razor
<MudItem xs="12" md="6" lg="4">
```
This gives full width on mobile, half on tablet, one-third on desktop.

**Breakpoint show/hide** with `MudHidden` and `MudVisible`:
```razor
<MudHidden Breakpoint="Breakpoint.SmAndDown">
    <DesktopOnlyContent />
</MudHidden>
<MudVisible Breakpoint="Breakpoint.SmAndDown">
    <MobileOnlyContent />
</MudVisible>
```

**Custom CSS media queries** for layouts outside MudGrid:
```css
@media (max-width: 599.95px)  { /* xs → sm boundary */ }
@media (max-width: 959.95px)  { /* sm → md boundary */ }
@media (max-width: 1279.95px) { /* md → lg boundary */ }
@media (max-width: 1919.95px) { /* lg → xl boundary */ }
```

**Safe area insets** for PWA on notched devices:
```css
.padding-bottom-safe {
    padding-bottom: env(safe-area-inset-bottom, 0px);
}
```

---

## 11. Performance Optimization

**Virtualize long lists** to avoid rendering thousands of DOM nodes:
```razor
<MudVirtualize Items="@largeList" Context="item" OverscanCount="20">
    <MudListItem>@item.Name</MudListItem>
</MudVirtualize>
```
`OverscanCount` controls how many extra items render outside the viewport. Default is 3. Increase to 15-20 for fast scrolling lists to reduce visible blank space during rapid scroll.

**`MudVirtualize<T>` uses `ChildContent`** (which is `RenderFragment<T>`), NOT `<ItemContent>`. The `Context` parameter sets the item variable name. Content goes directly inside the component tags. For server-side data loading with `ItemsProvider`:
```razor
<MudVirtualize ItemsProvider="LoadItems" Context="item">
    <MudCard>
        <MudCardContent>@item.Name</MudCardContent>
    </MudCard>
</MudVirtualize>
```

**Lazy-loaded assemblies** in WASM via `Router.AdditionalAssemblies`:
```razor
<Router AppAssembly="@typeof(App).Assembly"
        AdditionalAssemblies="new[] { typeof(HeavyFeature.Module).Assembly }">
```
This defers loading feature assemblies until the router navigates to a page in that assembly.

**Prevent unnecessary re-renders**:
```csharp
private bool _shouldRender = true;
protected override bool ShouldRender() => _shouldRender;

// After a bulk update completes:
_shouldRender = true;
StateHasChanged();
```
Set `_shouldRender = false` before batch operations, then flip it back and call `StateHasChanged()` once.

**Call `StateHasChanged()` only when data actually changes**, not on every timer tick or event callback. This is the single most impactful optimization for Blazor Server.

**Deferred rendering** for non-critical sections:
```razor
@if (_mainContentLoaded)
{
    @await RenderAnalyticsWidgetAsync()
}
```
Use `@await` on async methods to let the main UI render first, then fill in secondary content.

**Debounce rapid input** (search boxes, sliders):
```csharp
private Timer? _debounceTimer;

private void OnInputChanged(string value)
{
    _debounceTimer?.Dispose();
    _debounceTimer = new Timer(_ =>
    {
        InvokeAsync(() => { SearchTerm = value; StateHasChanged(); });
    }, null, 300, Timeout.Infinite);
}
```
Or use MudBlazor's built-in `DebounceInterval` parameter on `MudTextField` where available.

---

## 12. Charts with MudChart

`MudChart` supports `Line`, `Bar`, `Pie`, and `Donut` chart types.

**Newer MudBlazor versions** require strongly-typed `ChartSeries<T>` with explicit generic type and constructor syntax:
```razor
@code {
    private List<double> revenueData = [12000, 15000, 11000, 18000, 22000, 16000];
}

<MudChart ChartType="ChartType.Line"
          ChartSeries="@(new List<ChartSeries<double>> { new() { Name = "Revenue", Data = revenueData } })" />
```

Do NOT use non-generic `ChartSeries` — it produces MUD0002 analyzer errors in newer MudBlazor versions. The generic type parameter must match the data element type (typically `double` or `int`).

---

## 13. State Management

**Cascading values** for ancestor→descendant data flow:
```razor
<!-- Parent layout -->
<CascadingValue Value="@CurrentUser" Name="CurrentUser">
    @Body
</CascadingValue>

<!-- Child component -->
[CascadingParameter(Name = "CurrentUser")] public User? CurrentUser { get; set; }
```

**State container pattern** for sibling component communication (no JS needed):
```csharp
public class AppState
{
    public event Action? OnChange;
    private void NotifyChanged() => OnChange?.Invoke();

    private int _notificationCount;
    public int NotificationCount
    {
        get => _notificationCount;
        set { _notificationCount = value; NotifyChanged(); }
    }
}

// Register as scoped in Program.cs:
builder.Services.AddScoped<AppState>();
```

Components inject `AppState`, read/write properties, and subscribe to `OnChange` to re-render when state changes. This avoids JS interop for cross-component communication.

**Avoid over-notifying**: If a batch update changes multiple properties, suppress notifications during the batch and fire once at the end. Otherwise each property setter triggers a re-render.

### MVVM Pattern with CommunityToolkit.Mvvm

For pages with meaningful logic, use a ViewModel layer backed by `CommunityToolkit.Mvvm`. This keeps Razor files thin and makes the logic testable.

**Base ViewModel** (in Shared UI library):
```csharp
using CommunityToolkit.Mvvm.ComponentModel;

public abstract class ViewModelBase : ObservableObject
{
    private bool _isBusy;
    public bool IsBusy
    {
        get => _isBusy;
        set => SetProperty(ref _isBusy, value);
    }

    private string? _errorMessage;
    public string? ErrorMessage
    {
        get => _errorMessage;
        set => SetProperty(ref _errorMessage, value);
    }

    /// <summary>
    /// Runs an async operation with busy/error state management.
    /// Catches all exceptions and sets ErrorMessage on failure.
    /// </summary>
    protected async Task RunAsync(Func<Task> action)
    {
        IsBusy = true;
        ErrorMessage = null;
        try
        {
            await action();
        }
        catch (Exception ex)
        {
            ErrorMessage = ex.Message;
        }
        finally
        {
            IsBusy = false;
        }
    }

    // Optional: parameterless init for component lifecycle
    public virtual Task InitializeAsync() => Task.CompletedTask;
}
```

**Feature ViewModel example**:
```csharp
public class CustomerListViewModel : ViewModelBase
{
    private readonly ICustomerService _customerService;
    private List<Customer> _customers = [];

    public List<Customer> Customers
    {
        get => _customers;
        private set => SetProperty(ref _customers, value);
    }

    public CustomerListViewModel(ICustomerService customerService)
    {
        _customerService = customerService;
    }

    public override async Task InitializeAsync()
    {
        await RunAsync(async () =>
        {
            Customers = await _customerService.GetAllAsync();
        });
    }

    public async Task DeleteCustomerAsync(Guid id)
    {
        await RunAsync(async () =>
        {
            await _customerService.DeleteAsync(id);
            Customers = [.. Customers.Where(c => c.Id != id)];
        });
    }
}
```

**Component base that owns a ViewModel**:
```csharp
public abstract class ViewModelComponentBase<TViewModel> : ComponentBase, IDisposable
    where TViewModel : ViewModelBase
{
    [Inject] protected TViewModel ViewModel { get; set; } = default!;

    protected override async Task OnInitializedAsync()
    {
        ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        await ViewModel.InitializeAsync();
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        InvokeAsync(StateHasChanged);
    }

    public void Dispose()
    {
        ViewModel.PropertyChanged -= OnViewModelPropertyChanged;
    }
}
```

**Usage in a page**:
```razor
@inherits ViewModelComponentBase<CustomerListViewModel>

<MudTable Items="@ViewModel.Customers" IsLoading="@ViewModel.IsBusy">
    <ToolBarContent>
        <MudText Typo="Typo.h6">Customers</MudText>
    </ToolBarContent>
    <HeaderContent>
        <MudTh>Name</MudTh>
        <MudTh>Email</MudTh>
    </HeaderContent>
    <RowTemplate>
        <MudTd>@context.Name</MudTd>
        <MudTd>@context.Email</MudTd>
    </RowTemplate>
    <NoRecordsContent>
        <MudAlert Severity="Severity.Info">No customers found.</MudAlert>
    </NoRecordsContent>
</MudTable>

@if (ViewModel.ErrorMessage is not null)
{
    <MudAlert Severity="Severity.Error" Class="mt-4">@ViewModel.ErrorMessage</MudAlert>
}
```

---

## 14. File Upload

**MudBlazor file upload** with validation:
```razor
<MudFileUpload T="IBrowserFile" Accept=".pdf,.jpg,.png"
              FilesChanged="@OnFilesChanged"
              MaximumFileCount="5" MaximumFileSize="10_000_000" />
```

**Server-side processing**: Stream files directly from `IBrowserFile.OpenReadStream()` to avoid loading entire files into memory. Use `CopyToAsync` to write to a `Stream` (disk, network, or database).

**Progress reporting**: For large files, wrap the stream in a progress-tracking decorator that reports upload percentage to the UI via `StateHasChanged`.

**Client-side WASM**: Files are fully available in memory via `IBrowserFile`. Be mindful of browser memory limits for large files. Stream to IndexedDB for files that shouldn't be held in memory.

---

## 15. Modular Blazor Server Architecture

Pattern for decomposing a monolithic Blazor Server app into N standalone Blazor Server apps sharing common infrastructure. Each module runs as a separate process with its own Kestrel endpoint, sharing authentication, theming, and UI components via RCL packages.

**Solution structure**:
```text
src/
├── {Project}.Domain/            # Shared domain (entities, commands, queries)
├── {Project}.Infrastructure/    # Shared infra (EF Core, auth, tenancy)
├── {Project}.Web.Shared/        # Shared RCL (layouts, theme, common components)
├── {Project}.Hosting/           # Shared DI extension methods
├── {Project}.Rcl/               # Shared ViewModels / business logic RCL
├── {Project}.Contracts/         # Shared DTOs / API contracts
├── {Project}.Launchpad/         # Navigation portal (Blazor Server, port 5100)
├── {Project}.App.ModuleA/       # Standalone Blazor Server (port 5101)
├── {Project}.App.ModuleB/       # Standalone Blazor Server (port 5102)
└── {Project}.App.ModuleC/       # Standalone Blazor Server (port 5103)
```

### Per-App Directory Structure

Every `App.{Module}` follows an identical layout:
```text
{Project}.App.ModuleA/
├── {Project}.App.ModuleA.csproj   # References: Domain, Infrastructure, Rcl, Contracts, Hosting, Web.Shared
├── Program.cs                      # Calls shared hosting extensions, registers module ViewModels, maps endpoints
├── appsettings.json                # ConnectionStrings, Redis, OIDC (per-app ClientId)
├── Components/
│   ├── App.razor                   # HTML shell with MudBlazor CSS/JS, flash prevention script
│   ├── Routes.razor                # Router with AuthorizeRouteView
│   ├── _Imports.razor              # Standard usings
│   ├── Layout/
│   │   ├── MainLayout.razor        # Composes shared shell + module nav
│   │   ├── ModuleANavMenu.razor    # "Back to Launchpad" + Home + module-specific links
│   │   └── BreadcrumbNav.razor     # Dynamic breadcrumb from URL segments
│   ├── Pages/
│   │   ├── Home.razor
│   │   └── Error.razor
│   └── I18n/
│       ├── I18nService.cs          # Loads Resources/{culture}.json (or inject shared)
│       └── LanguageSwitcher.razor
├── Endpoints/                      # Module-specific minimal-API mappings
├── Services/
│   └── HttpCurrentContext.cs       # ICurrentContext implementation
├── Resources/                      # Locale files (pt-BR.json, es.json)
└── Properties/
    └── launchSettings.json         # Dev port assignment
```

### Per-App Program.cs

Each app's `Program.cs` calls shared hosting extensions and adds module-specific registrations:
```csharp
var builder = WebApplication.CreateBuilder(args);

// --- Shared infrastructure (from Hosting project) ---
builder.AddSuiteBlazor();              // RazorComponents + MudServices
builder.AddSuiteAuthentication(builder.Configuration);  // OIDC + Cookie + DataProtection
builder.AddSuiteMultiTenancy();        // Finbuckle cookie-based multi-tenancy
builder.AddSuiteMessaging();           // Messaging (optional per module)
builder.AddSuiteProviders();           // Domain service providers
builder.AddSuitePipeline();            // Middleware pipeline
builder.AddSuiteDataProtection();      // Redis-backed shared key ring
builder.AddSuiteHealthChecks();        // Health endpoints

// --- Module-specific ---
builder.Services.AddScoped<ModuleAViewModel>();
builder.Services.AddI18nService();

var app = builder.Build();

app.UseSuitePipeline();                // Exception handler, multi-tenant, auth, etc.

// Module-specific minimal-API endpoints
app.MapGroup("/api/modulea").MapModuleAEndpoints();

app.Run();
```

### Shared Hosting Extension: AddBlazorComponents

The `{Project}.Hosting` project provides reusable DI wiring so every app registers services identically:
```csharp
public static class BlazorHostingExtensions
{
    public static WebApplicationBuilder AddSuiteBlazor(
        this WebApplicationBuilder builder)
    {
        builder.Services.AddRazorComponents()
            .AddInteractiveServerComponents();

        builder.Services.AddMudServices(config =>
        {
            config.SnackbarConfiguration.PositionClass = Defaults.Classes.Position.BottomRight;
            config.SnackbarConfiguration.VisibleStateDuration = 4000;
        });

        builder.Services.AddHttpContextAccessor();
        builder.Services.AddCascadingAuthenticationState();

        return builder;
    }
}
```

### Per-App Layout Composing Shared Shell

Each module's `MainLayout.razor` composes the shared theme and layout components from the RCL, then adds its own navigation:
```razor
@inherits LayoutComponentBase
@using MySuite.Web.Shared.Components
@inject I18nService I18n

<MudThemeProvider Theme="@AppTheme.Default"
                  @bind-IsDarkMode="_isDarkMode"
                  ObserveSystemDarkModeChange="false" />
<MudPopoverProvider />

<SuiteLayout Title="@I18n["moduleA.title"]">
    <NavMenuContent>
        <ModuleANavMenu />
    </NavMenuContent>
    <BodyContent>
        @Body
    </BodyContent>
</SuiteLayout>

@code {
    private bool _isDarkMode;
}
```

The shared `SuiteLayout` component (in `Web.Shared`) renders the gradient app bar, MudDrawer, user menu, and tenant selector. Each module injects its own `NavMenu` and page content.

### Shared App.razor (HTML Shell)

```razor
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="stylesheet" href="_content/MudBlazor/MudBlazor.min.css" />
    <link rel="stylesheet" href="_content/MySuite.Web.Shared/css/app.css" />
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" rel="stylesheet">
    <script>
        // Flash prevention: runs synchronously before CSS loads
        (function () {
            var t = localStorage.getItem('app-dark-mode');
            if (t === 'dark') document.documentElement.setAttribute('data-mud-theme', 'dark');
        })();
    </script>
    <HeadOutlet @rendermode="InteractiveServer" />
</head>
<body>
    <Routes @rendermode="InteractiveServer" />
    <script src="_content/MudBlazor/MudBlazor.min.js"></script>
    <script src="_framework/blazor.web.js"></script>
</body>
</html>
```

### Cross-App Concerns Summary

| Concern | Pattern | Key Requirement |
|---------|---------|-----------------|
| **SSO** | OIDC with per-app Keycloak clients in shared realm | All apps in same realm |
| **Auth Cookie** | Shared `Cookie.Name` across all apps | Same name + shared DataProtection |
| **DataProtection** | Redis-backed key ring with same `SetApplicationName` | Identical application name |
| **Theme** | Shared RCL `AppTheme` class + CSS variables | Single source of truth in RCL |
| **Layout** | Shared `SuiteLayout` in RCL, module-specific nav injected via render fragments | RCL references only |
| **Navigation** | "Back to Launchpad" link in each module's nav menu | Cross-app links use full URLs |
| **Tenancy** | Finbuckle cookie-based with shared `__tenant__` cookie | Same cookie name across apps |
| **I18n** | Per-app JSON files OR shared RCL I18nService | Consistent key naming |
| **Health** | Each app exposes `/health` endpoint | Docker health checks |

### Docker Compose

Each app is a separate service with its own health check:
```yaml
services:
  postgres:
    image: postgres:16-alpine
    # ...

  redis:
    image: dragonflydb/dragonfly
    # ...

  keycloak:
    image: quay.io/keycloak/keycloak:26
    # ...

  launchpad:
    build: ./src/MySuite.Launchpad
    ports: ["5100:5100"]
    depends_on: [postgres, redis, keycloak]
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:5100/health"]
      interval: 30s

  module-a:
    build: ./src/MySuite.App.ModuleA
    ports: ["5101:5101"]
    depends_on: [postgres, redis, keycloak]
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:5101/health"]
      interval: 30s

  module-b:
    build: ./src/MySuite.App.ModuleB
    ports: ["5102:5102"]
    depends_on: [postgres, redis, keycloak]
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:5102/health"]
      interval: 30s
```

---

## 16. Policy-Based Component Authorization

Wrap `AuthorizeView` in a reusable component that accepts a policy or role, and provides a default "not authorized" message. This avoids scattering auth logic across pages.

**AuthorizeGate.razor** (reusable auth gate):
```razor
@* AuthorizeGate.razor - Reusable component-level auth gate *@
@code {
    [Parameter] public string? Policy { get; set; }
    [Parameter] public string? Role { get; set; }
    [Parameter] public RenderFragment? ChildContent { get; set; }
    [Parameter] public RenderFragment? NotAuthorized { get; set; }
    [CascadingParameter] private Task<AuthenticationState> AuthState { get; set; } = default!;
}

<AuthorizeView Policy="@Policy" Roles="@Role">
    <Authorized>@ChildContent</Authorized>
    <NotAuthorized>
        @if (NotAuthorized is not null)
        {
            @NotAuthorized
        }
        else
        {
            <MudAlert Severity="Severity.Warning">You do not have permission to view this content.</MudAlert>
        }
    </NotAuthorized>
</AuthorizeView>
```

**Usage**:
```razor
<AuthorizeGate Policy="admin-only" Role="admin">
    <MudButton Variant="Variant.Filled" Color="Color.Error" OnClick="DeleteAll">
        Delete All
    </MudButton>
</AuthorizeGate>
```

Pass `Policy` for claims-based rules, `Role` for simple role checks, or both. Provide a custom `NotAuthorized` fragment to override the default warning alert.

---

## 17. Hybrid Blazor Render Modes and SSR Gotchas

When a Blazor app supports both Server and WebAssembly rendering (hybrid), pages default to SSR unless they opt in to an interactive render mode. Understanding when and how to apply render modes prevents dead buttons, broken bindings, and confusing SSR-only behavior.

### Per-Page `@rendermode` Directive

If `App.razor` has NO global `@rendermode` on `<Routes>`, every page renders as SSR-only by default. Pages with interactive event handlers (`@onclick`, `@bind-Value`, `OnValidSubmit`) MUST add `@rendermode InteractiveServer` at the top of the `.razor` file:

```razor
@page "/patient/book"
@rendermode InteractiveServer
@using static Microsoft.AspNetCore.Components.Web.RenderMode

<MudButton OnClick="SearchPsychologists">Search</MudButton>
<MudDatePicker @bind-Date="SelectedDate" />
```

**Without the directive**, the page renders as static HTML. Buttons are dead — clicks submit a form GET/POST instead of invoking C# handlers. Two-way bindings don't update.

**When to use per-page vs global rendermode:**

| Approach | When to use |
|----------|-------------|
| Global `@rendermode="InteractiveServer"` on `<Routes>` in `App.razor` | App is Blazor Server only (no WASM). Every page is interactive. |
| Per-page `@rendermode InteractiveServer` | Hybrid app (Server + WASM). Public pages stay SSR for performance; role-gated pages opt in for interactivity. |
| `@rendermode InteractiveAuto` | Pages that should work in both Server and WASM contexts. Requires the page's dependencies to be available in both. |

**Prerequisite**: The app must register both render modes in `Program.cs`:

```csharp
builder.Services.AddRazorComponents()
    .AddInteractiveServerRenderMode()
    .AddInteractiveWebAssemblyRenderMode();
```

And `Shared/_Imports.razor` (or each page) needs:

```razor
@using static Microsoft.AspNetCore.Components.Web.RenderMode
```

**Safety check**: Per-page `InteractiveServer` locks the page out of WASM rendering. For authenticated pages gated by `[Authorize(Roles = "...")]` that rely on server-side cookies, this is correct — WASM can't access Keycloak cookies. For public pages, prefer SSR-only (no directive) or `InteractiveAuto`.

### MudDialog + SSR Layout Limitation

When a layout component renders in SSR mode (no `@rendermode`), `MudSnackbarProvider` in that layout cannot display snackbars triggered by interactive child components. The layout's render tree is fixed at SSR time — snackbar additions from an interactive child's circuit don't propagate back to the SSR-rendered layout.

**Symptoms**: `Snackbar.Add("Success!")` executes without error, but no snackbar appears in the UI.

**Workaround**: Replace `MudDialog` with a conditional `MudPaper` panel. Verify the action by checking DOM state changes (panel disappears, status chip changes, element removal) instead of relying on snackbar appearance:

```razor
@* Instead of MudDialog for confirmations *@
@if (_showConfirmPanel)
{
    <MudPaper Class="pa-4 mt-4" Elevation="3">
        <MudText>Are you sure you want to withdraw consent?</MudText>
        <MudButton Color="Color.Error" OnClick="ConfirmWithdraw">Confirm</MudButton>
        <MudButton Variant="Variant.Text" OnClick="@(() => _showConfirmPanel = false)">Cancel</MudButton>
    </MudPaper>
}
```

**Testing pattern**: In Playwright E2E tests, verify the action succeeded by checking that the panel element was removed from the DOM:

```csharp
await page.Locator("text=Are you sure").WaitForAsync(new() { State = WaitForSelectorState.Visible });
await page.Locator("button:has-text('Confirm')").ClickAsync();
await Assertions.Expect(page.Locator("text=Are you sure")).ToBeHiddenAsync();
```

### MudList `@bind-SelectedValue` Testing Limitation

In Blazor Server, clicking a `.mud-list-item` via Playwright does NOT trigger `MudList @bind-SelectedValue`. The click lands on the rendered DOM element, but the Blazor Server circuit's internal event delegation doesn't fire the `SelectedValueChanged` callback. This is a known MudBlazor v8 + Blazor Server + Playwright gap.

**Workaround**: Test list selection flows via integration/unit tests instead. For E2E, mark the test as skipped with a clear explanation:

```csharp
[Fact(Skip = "MudList @bind-SelectedValue untestable via Playwright in Blazor Server — manual verification required")]
```

---

## Anti-Patterns

- **DO NOT call `IJSRuntime` during `OnInitializedAsync`**. In Blazor Server with SSR prerendering, JavaScript interop is unavailable during the prerender phase. `OnInitializedAsync` runs during prerender. Use `OnAfterRenderAsync(firstRender: true)` instead for all JS interop calls (localStorage, DOM manipulation, module imports). Violating this causes `InvalidOperationException: JavaScript interop calls cannot be issued during prerendering`.

- **DO NOT use `eval()` or `document.cookie` for reading cookies in production**. This exposes the application to XSS-based session hijacking. Use `IJSObjectReference` ES modules with `HttpOnly` cookies managed server-side via ASP.NET Core's cookie authentication middleware.

- **DO NOT create separate theme services per modular app**. In a multi-app architecture, each Blazor Server app must use the same shared theme class from the Shared RCL. Duplicating theme definitions causes visual inconsistency and maintenance burden — a color change requires editing N files instead of one.

- **DO NOT call `StateHasChanged()` inside `OnChange` event handlers that fire rapidly** (e.g., on every keystroke or mouse move). This causes excessive re-renders. Debounce the input first (see Section 11), then call `StateHasChanged()` once after the debounce period.

- **DO NOT use `[CascadingParameter]` for data that changes frequently across the component tree**. Every cascading value change triggers re-render of all descendants. Use a scoped state container (see Section 12) with explicit subscription instead.

- **DO NOT catch and swallow `ObjectDisposedException` silently**. If a component is disposed while async work is pending, log the occurrence or use `CancellationToken` to cancel gracefully. Silent swallowing hides real bugs.

- **NEVER skip PropertyChanged subscription in component base.** Without subscribing to `ViewModelBase.PropertyChanged`, the UI won't re-render when the ViewModel updates. Always call `InvokeAsync(StateHasChanged)` in the handler and unsubscribe in `Dispose()`.

- **NEVER use `IStringLocalizer` directly in ViewModels.** ViewModels should remain UI-framework-agnostic. Pass localized strings from the component layer or use a generic `ILocalizationService` interface that the ViewModel can call.

- **MUD0002 analyzer strictness:** The MudBlazor MUD0002 analyzer is strict about attribute names and generic usages. When upgrading MudBlazor versions, attributes and parameter names may change. Scan for MUD0002 errors across the project after upgrades — they are often repetitive and may require a coordinated update. Razor is case-sensitive for component attribute names (`Typo` vs `typo`, `T` vs `t`).

---

## Related Skills

- **signalr-realtime** — WebRTC video session pattern (Section 8 of this skill) with SignalR signaling hub
- **ui-ux-accessibility** — Full WCAG 2.2 compliance patterns, MudBlazor accessibility quirks (Section 4 of this skill), and testing methodology
- **dotnet-clean-arch** — Project structure where Shared components live and JS interop source files are organized
- **keycloak-oidc** — OIDC authentication setup for modular Blazor Server apps (Section 14), shared DataProtection (Section 3), cookie configuration
- **docker-deployment** — Docker Compose multi-service deployment for modular Blazor Server architecture (Section 14)
