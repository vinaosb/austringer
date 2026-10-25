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

All parts mandatory. Skipping any causes flash or broken theming.

**ThemeProvider setup** (MudBlazor v8+):
```razor
<MudThemeProvider @bind-IsDarkMode="isDarkMode" ObserveSystemDarkModeChange="false" />
```
`ObserveSystemDarkModeChange="false"` prevents OS preference overriding user's explicit choice.

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

**Startup init** — read saved preference on load:
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
Without this, white flash on every dark mode page load. Script runs synchronously before renderer — no FOUC.

**CSS variables**: define at `:root` (light defaults) and `[data-mud-theme="dark"]` (dark overrides):
```css
:root { --app-surface: #ffffff; --app-text: #1a1a2e; }
[data-mud-theme="dark"] { --app-surface: #1e1e2f; --app-text: #e0e0e0; }
```
Components reference `var(--app-*)` instead of hardcoded colors.

**PWA note**: Standalone WASM may not load app.css. ThemeProvider handles switching, but flash prevention in `index.html` still required.

### Design System Theme with CSS Variables

Define centralized `MudTheme` with coordinated palettes, typography, layout. Expose design tokens as CSS custom properties so any component — MudBlazor or custom — references same variables.

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
    --app-surface: #FFFFFF;
    --app-surface-alt: #F5F5F5;
    --app-background: #FAFAFA;
    --app-text: #212121;
    --app-text-secondary: #757575;
    --app-text-on-primary: #FFFFFF;
    --app-primary: #1565C0;
    --app-primary-hover: #1976D2;
    --app-secondary: #42A5F5;
    --app-success: #2E7D32;
    --app-warning: #F57F17;
    --app-error: #C62828;
    --app-info: #0277BD;
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

**Usage in Razor** — custom components reference design tokens:
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

Ensures MudBlazor components and custom CSS share identical colors. Theme switch updates CSS variables automatically via `[data-mud-theme]` attribute.

---

## 2. JS Interop (ES Modules)

- All custom JS as ES modules under `wwwroot/js/`
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

Never call module exports as `window.someFunction()`. Always through `IJSObjectReference`.

---

## 3. Shared Component Library Rules

Shared project consumed by both Server and WASM clients:

- `[Inject] private ILogger<T>? Logger { get; set; }` with `Logger?.Log(...)`. Never `null!`.
- No server-side deps in Shared: no Infrastructure, no Application, no scoped service refs.
- JS interop source files in host project's `wwwroot/js/`, not in Shared. Shared components load at runtime via `IJSObjectReference`.
- Theme class returns `MudTheme` with light + dark palettes. Both WCAG-compliant (see section 5).

### Shared DataProtection for SSO

Multiple Blazor Server apps sharing SSO via OIDC must share DataProtection key ring. Without this, encrypted cookie from App A cannot be decrypted by App B.

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

**Cookie config** — every app must use same cookie name + `SameSite`:
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

**Why**: ASP.NET Core `IDataProtectionProvider` derives encryption keys from app name + local key material. If each app generates own keys, cookies from one app meaningless to another. `SetApplicationName` + shared Redis ensures all apps use same encryption key.

**Common failure**: App A (port 5101) auths user. Navigation to App B (port 5102) sends cookie, but App B's DataProtection cannot decrypt → 401 redirect loop. Fix: same `SetApplicationName`, same key store, same cookie name.

---

## 4. MudBlazor Accessibility Quirks

- **Explicit generics required in v8+**: `MudTextField<string>`, `MudChip<string>`, `MudSelect<T>`. Omitting type param breaks accessibility attributes.
- **Known axe-core violations** (MudBlazor defaults):
  - `link-in-text-block`: internal links in text blocks lack sufficient click target
  - `color-contrast`: some default palette combos fail AA on dark backgrounds
  - `label`: `MudTextField` with no `Label` or `aria-label` fails
  - `button-name`: icon-only `MudIconButton` needs `aria-label`
  Document each suppressed rule with reason. Don't blanket-disable.
- **MudDialog**: set `Autofocus="true"` or manually call `.FocusAsync()` on first input.
- **ThemeProvider** handles theme switching but does NOT prevent flash. Separate concern (section 1).

---

## 5. WCAG-Compliant Theme Palette

- Light mode: test every text/background pair for 7:1 (AAA) or 4.5:1 (AA) contrast.
- Dark mode: test again. Colors passing in light often fail in dark due to luminance shifts.
- Hover/active states: never lighten borderline pair — may drop below threshold.
- Contrast ratio formula:
  ```
  L = 0.2126*R + 0.7152*G + 0.0722*B  (linearized sRGB)
  ratio = (L1 + 0.05) / (L2 + 0.05)   where L1 > L2
  ```
- Use browser DevTools contrast checker or axe-core during development. Automate in CI if possible.

---

## 6. Focus Management

- Programmatic focus: `@ref="myInput"` then `await myInput.FocusAsync()`.
- MudDialog: `Autofocus="true"` moves focus to dialog on open.
- Route changes: move focus to `<main>` or content heading so screen readers announce new page.
- Dynamic content: `aria-live="polite"` for non-urgent, `aria-live="assertive"` for critical.
- Modal traps: MudDialog handles focus trap automatically. Custom modals need manual implementation.

---

## 7. Error Boundaries

Wrap pages/layouts in `<ErrorBoundary>` for graceful degradation:
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

- Custom `ErrorContent` should use `role="alert"` for screen readers.
- MudBlazor `MudSnackbar` or custom toast can surface non-fatal errors.
- Log errors in `ErrorContent` with `ILogger` — never show stack traces to end users.

---

## 8. WebRTC Video Session Pattern

Blazor component wrapping WebRTC via JS interop with SignalR hub as signaling relay.

**Component state machine**: `Idle → Connecting → Connected → Ended`

**JS interop module** (`webrtc-interop.js`):
```javascript
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

**ICE config**: STUN server (`stun:stun.l.google.com:19302`) minimum. Add TURN server for production where symmetric NAT blocks direct peer connections.

**Browser compat**: load [adapter.js](https://github.com/webrtcHacks/adapter) as module import to normalize WebRTC APIs across browsers.

**Security**: WebRTC media runs entirely in browser. Audio/video travels peer-to-peer. Server only relays signaling messages (SDP offers/answers, ICE candidates).

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

**Shared resources**:
```csharp
public class SharedResource { }
```
With `Resources/SharedResource.resx` and `Resources/SharedResource.pt-BR.resx`.

**Injection** in Razor:
```razor
@inject IStringLocalizer<Dashboard> Loc
<p>@Loc["WelcomeMessage"]</p>
```

**Shared project code-behind** (nullable pattern):
```csharp
[Inject] private IStringLocalizer<Dashboard>? Loc { get; set; }
private string GetText(string key) => Loc?[key] ?? key;
```

**Culture management** via `IJSRuntime`:
```csharp
private async Task SetCulture(string culture)
{
    await JS.InvokeVoidAsync("localStorage.setItem", "app-culture", culture);
    await JS.InvokeVoidAsync("document.documentElement.setAttribute", "lang", culture);
}
```

Init on startup: read from localStorage, call `CultureInfo.DefaultThreadCurrentCulture`.

**Accept-Language header**: configure request culture providers in middleware so server-side rendering respects browser language preference.

**Fallback chain**: requested culture (e.g., `pt-BR`) → parent culture (`pt`) → default. Framework handles automatically when resource files follow naming convention.

### JSON-Based I18n (Alternative to IStringLocalizer)

For projects preferring JSON over `.resx`, use custom `I18nService` loading locale files with dot-notation keys. Avoids resx compilation step, makes translations easier to edit.

**JSON locale files** (`Resources/pt-BR.json`):
```json
{
    "app": { "title": "Meu Aplicativo", "welcome": "Bem-vindo, {0}!" },
    "nav": { "home": "Início", "settings": "Configurações", "logout": "Sair" },
    "validation": { "required": "Este campo é obrigatório.", "minLength": "Mínimo de {0} caracteres." }
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

    public I18nService(IWebHostEnvironment env) { _env = env; }
    public string Culture => _culture;

    public async Task InitializeAsync(string? savedCulture = null)
    {
        if (savedCulture is not null) _culture = savedCulture;
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
            if (_culture != "pt-BR")
            {
                var fallback = ResolveKeyFromCulture("pt-BR", key);
                if (fallback is not null) value = fallback;
            }
            value ??= key;
        }
        return args.Length > 0 ? string.Format(value, args) : value;
    }

    private string? ResolveKey(string key)
        => _translations?.TryGetValue(key, out var val) == true ? val : null;

    private string? ResolveKeyFromCulture(string culture, string key)
    {
        var dict = LoadTranslationsSync(culture);
        return dict.TryGetValue(key, out var val) ? val : null;
    }

    private async Task LoadTranslationsAsync(string culture)
    {
        var path = Path.Combine(_env.WebRootPath, "Resources", $"{culture}.json");
        if (!File.Exists(path)) path = Path.Combine(_env.WebRootPath, "Resources", "pt-BR.json");
        var json = await File.ReadAllTextAsync(path);
        var raw = JsonSerializer.Deserialize<Dictionary<string, JsonElement>>(json);
        _translations = Flatten(raw);
    }

    private Dictionary<string, string> LoadTranslationsSync(string culture)
    {
        var path = Path.Combine(_env.WebRootPath, "Resources", $"{culture}.json");
        if (!File.Exists(path)) path = Path.Combine(_env.WebRootPath, "Resources", "pt-BR.json");
        var json = File.ReadAllText(path);
        return Flatten(JsonSerializer.Deserialize<Dictionary<string, JsonElement>>(json));
    }

    private static Dictionary<string, string> Flatten(Dictionary<string, JsonElement>? source, string prefix = "")
    {
        var result = new Dictionary<string, string>();
        if (source is null) return result;
        foreach (var (key, value) in source)
        {
            var fullKey = string.IsNullOrEmpty(prefix) ? key : $"{prefix}.{key}";
            if (value.ValueKind == JsonValueKind.Object)
                foreach (var (k, v) in Flatten(JsonSerializer.Deserialize<Dictionary<string, JsonElement>>(value.GetRawText())!, fullKey))
                    result[k] = v;
            else
                result[fullKey] = value.GetString() ?? fullKey;
        }
        return result;
    }
}
```

**Registration**:
```csharp
builder.Services.AddScoped<I18nService>();
```

**Language switcher component**:
```razor
@inject I18nService I18n
@inject IJSRuntime JS

<MudSelect T="string" Label="@I18n["nav.settings"]" Value="@I18n.Culture"
           ValueChanged="@OnCultureChanged" Variant="Variant.Outlined">
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

**Init** in layout or `App.razor`:
```csharp
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

**Usage**:
```razor
@inject I18nService I18n
<MudText Typo="Typo.h4">@I18n["app.title"]</MudText>
<MudText>@I18n.Translate("app.welcome", userName)</MudText>
```

**Fallback chain**: current culture JSON → default (`pt-BR.json`) → raw key.

### Culture Switcher with Cookie Persistence

For server-side Blazor, cookie works better than localStorage — middleware pipeline reads it on every request without JS interop.

**Culture switcher** using `CookieRequestCultureProvider`:
```csharp
public async Task SetCultureAsync(string culture)
{
    var cookie = new KeyValuePair<string, string?>(".AspNetCore.Culture",
        CookieRequestCultureProvider.MakeCookieValue(new RequestCulture(culture)));
    await JSRuntime.InvokeVoidAsync("document.cookie", $"{cookie.Key}={cookie.Value}; path=/; max-age=31536000");
    NavigationManager.NavigateTo(NavigationManager.Uri, forceLoad: true);
}
```

**Supported cultures** in `Program.cs`:
```csharp
builder.Services.Configure<RequestLocalizationOptions>(options =>
{
    var supportedCultures = new[]
    {
        new CultureInfo("en"), new CultureInfo("pt-BR"), new CultureInfo("ar") // RTL
    };
    options.DefaultRequestCulture = new RequestCulture("en");
    options.SupportedCultures = supportedCultures;
    options.SupportedUICultures = supportedCultures;
});
```

Add middleware: `app.UseRequestLocalization();`

Survives page reloads, works across tabs, no JS dependency before first render.

---

## 10. Responsive Layout (MudGrid Breakpoints)

MudBlazor v8 12-column grid with five breakpoints:
```razor
<MudGrid>
    <MudItem xs="12" sm="6" md="4" lg="3" xl="2">
        <MudCard>...</MudCard>
    </MudItem>
</MudGrid>
```

**Breakpoint reference**:

| Breakpoint | Min Width | Target |
|-----------|-----------|--------|
| `xs` | 0px | Mobile portrait |
| `sm` | 600px | Mobile landscape |
| `md` | 960px | Tablet |
| `lg` | 1280px | Desktop |
| `xl` | 1920px | Large desktop |

**Mobile-first**: define `xs` baseline, layer breakpoint overrides:
```razor
<MudItem xs="12" md="6" lg="4">
```
Full width mobile, half tablet, one-third desktop.

**Show/hide** with `MudHidden`/`MudVisible`:
```razor
<MudHidden Breakpoint="Breakpoint.SmAndDown">
    <DesktopOnlyContent />
</MudHidden>
```

**Custom CSS media queries**:
```css
@media (max-width: 599.95px)  { /* xs→sm */ }
@media (max-width: 959.95px)  { /* sm→md */ }
@media (max-width: 1279.95px) { /* md→lg */ }
@media (max-width: 1919.95px) { /* lg→xl */ }
```

**Safe area insets** for PWA on notched devices:
```css
.padding-bottom-safe { padding-bottom: env(safe-area-inset-bottom, 0px); }
```

---

## 11. Performance Optimization

**Virtualize long lists**:
```razor
<MudVirtualize Items="@largeList" Context="item" OverscanCount="20">
    <MudListItem>@item.Name</MudListItem>
</MudVirtualize>
```
`OverscanCount` default 3. Increase to 15-20 for fast scrolling to reduce visible blank space.

**`MudVirtualize<T>` uses `ChildContent`** (`RenderFragment<T>`), NOT `<ItemContent>`. For server-side data with `ItemsProvider`:
```razor
<MudVirtualize ItemsProvider="LoadItems" Context="item">
    <MudCard><MudCardContent>@item.Name</MudCardContent></MudCard>
</MudVirtualize>
```

**Lazy-loaded assemblies** in WASM via `Router.AdditionalAssemblies`:
```razor
<Router AppAssembly="@typeof(App).Assembly"
        AdditionalAssemblies="new[] { typeof(HeavyFeature.Module).Assembly }">
```

**Prevent unnecessary re-renders**:
```csharp
private bool _shouldRender = true;
protected override bool ShouldRender() => _shouldRender;
// After bulk update: _shouldRender = true; StateHasChanged();
```

**Call `StateHasChanged()` only when data actually changes** — single most impactful Blazor Server optimization.

**Deferred rendering** for non-critical sections:
```razor
@if (_mainContentLoaded) { @await RenderAnalyticsWidgetAsync() }
```

**Debounce rapid input**:
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
Or use MudBlazor's built-in `DebounceInterval` on `MudTextField`.

---

## 12. Charts with MudChart

`MudChart` supports `Line`, `Bar`, `Pie`, `Donut`.

**Newer MudBlazor** requires strongly-typed `ChartSeries<T>`:
```razor
@code {
    private List<double> revenueData = [12000, 15000, 11000, 18000, 22000, 16000];
}

<MudChart ChartType="ChartType.Line"
          ChartSeries="@(new List<ChartSeries<double>> { new() { Name = "Revenue", Data = revenueData } })" />
```

Do NOT use non-generic `ChartSeries` — produces MUD0002 analyzer errors. Generic param must match data type (typically `double` or `int`).

---

## 13. State Management

**Cascading values** for ancestor→descendant flow:
```razor
<CascadingValue Value="@CurrentUser" Name="CurrentUser">
    @Body
</CascadingValue>

[CascadingParameter(Name = "CurrentUser")] public User? CurrentUser { get; set; }
```

**State container** for sibling communication (no JS needed):
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

// Register as scoped:
builder.Services.AddScoped<AppState>();
```

Components inject `AppState`, read/write properties, subscribe to `OnChange`.

**Avoid over-notifying**: suppress notifications during batch updates, fire once at end.

### MVVM Pattern with CommunityToolkit.Mvvm

For pages with meaningful logic, use ViewModel layer backed by `CommunityToolkit.Mvvm`. Keeps Razor thin, makes logic testable.

**Base ViewModel** (in Shared UI):
```csharp
using CommunityToolkit.Mvvm.ComponentModel;

public abstract class ViewModelBase : ObservableObject
{
    private bool _isBusy;
    public bool IsBusy { get => _isBusy; set => SetProperty(ref _isBusy, value); }

    private string? _errorMessage;
    public string? ErrorMessage { get => _errorMessage; set => SetProperty(ref _errorMessage, value); }

    protected async Task RunAsync(Func<Task> action)
    {
        IsBusy = true;
        ErrorMessage = null;
        try { await action(); }
        catch (Exception ex) { ErrorMessage = ex.Message; }
        finally { IsBusy = false; }
    }

    public virtual Task InitializeAsync() => Task.CompletedTask;
}
```

**Feature ViewModel**:
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

    public CustomerListViewModel(ICustomerService customerService) => _customerService = customerService;

    public override async Task InitializeAsync()
        => await RunAsync(async () => { Customers = await _customerService.GetAllAsync(); });

    public async Task DeleteCustomerAsync(Guid id)
        => await RunAsync(async () =>
        {
            await _customerService.DeleteAsync(id);
            Customers = [.. Customers.Where(c => c.Id != id)];
        });
}
```

**Component base** owning ViewModel:
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
        => InvokeAsync(StateHasChanged);

    public void Dispose() => ViewModel.PropertyChanged -= OnViewModelPropertyChanged;
}
```

**Usage**:
```razor
@inherits ViewModelComponentBase<CustomerListViewModel>

<MudTable Items="@ViewModel.Customers" IsLoading="@ViewModel.IsBusy">
    <ToolBarContent><MudText Typo="Typo.h6">Customers</MudText></ToolBarContent>
    <HeaderContent>
        <MudTh>Name</MudTh> <MudTh>Email</MudTh>
    </HeaderContent>
    <RowTemplate>
        <MudTd>@context.Name</MudTd> <MudTd>@context.Email</MudTd>
    </RowTemplate>
    <NoRecordsContent>
        <MudAlert Severity="Severity.Info">No customers found.</MudAlert>
    </NoRecordsContent>
</MudTable>

@if (ViewModel.ErrorMessage is not null)
    { <MudAlert Severity="Severity.Error" Class="mt-4">@ViewModel.ErrorMessage</MudAlert> }
```

---

## 14. File Upload

**MudBlazor file upload** with validation:
```razor
<MudFileUpload T="IBrowserFile" Accept=".pdf,.jpg,.png"
              FilesChanged="@OnFilesChanged"
              MaximumFileCount="5" MaximumFileSize="10_000_000" />
```

**Server-side**: Stream from `IBrowserFile.OpenReadStream()` to avoid loading entire file into memory. Use `CopyToAsync` to write to `Stream`.

**Progress**: For large files, wrap stream in progress-tracking decorator reporting to UI via `StateHasChanged`.

**Client-side WASM**: Files fully available via `IBrowserFile`. Mind browser memory limits. Stream to IndexedDB for files not held in memory.

---

## 15. Modular Blazor Server Architecture

Decompose monolithic Blazor Server into N standalone apps sharing common infra. Each module runs as separate process with own Kestrel endpoint, sharing auth, theming, UI components via RCL packages.

**Solution structure**:
```text
src/
├── {Project}.Domain/
├── {Project}.Infrastructure/
├── {Project}.Web.Shared/        # Shared RCL (layouts, theme, components)
├── {Project}.Hosting/           # Shared DI extension methods
├── {Project}.Rcl/               # Shared ViewModels / business logic RCL
├── {Project}.Contracts/         # Shared DTOs / API contracts
├── {Project}.Launchpad/         # Navigation portal (Blazor Server, port 5100)
├── {Project}.App.ModuleA/       # Standalone Blazor Server (port 5101)
├── {Project}.App.ModuleB/       # Standalone Blazor Server (port 5102)
└── {Project}.App.ModuleC/       # Standalone Blazor Server (port 5103)
```

### Per-App Directory Structure

Every `App.{Module}` follows identical layout:
```text
{Project}.App.ModuleA/
├── {Project}.App.ModuleA.csproj   # References: Domain, Infra, Rcl, Contracts, Hosting, Web.Shared
├── Program.cs                      # Shared hosting extensions, module ViewModels, endpoints
├── appsettings.json                # ConnectionStrings, Redis, OIDC (per-app ClientId)
├── Components/{App,Routes,_Imports,Layout/{MainLayout,ModuleANavMenu,BreadcrumbNav},Pages/{Home,Error},I18n/{I18nService,LanguageSwitcher}}.razor
├── Endpoints/                      # Module-specific minimal-API mappings
├── Services/HttpCurrentContext.cs
├── Resources/                      # Locale files
└── Properties/launchSettings.json  # Dev port assignment
```

### Per-App Program.cs

Each app calls shared hosting extensions + module-specific registrations:
```csharp
var builder = WebApplication.CreateBuilder(args);

builder.AddSuiteBlazor();
builder.AddSuiteAuthentication(builder.Configuration);
builder.AddSuiteMultiTenancy();
builder.AddSuiteMessaging();
builder.AddSuiteProviders();
builder.AddSuitePipeline();
builder.AddSuiteDataProtection();
builder.AddSuiteHealthChecks();

builder.Services.AddScoped<ModuleAViewModel>();
builder.Services.AddI18nService();

var app = builder.Build();
app.UseSuitePipeline();
app.MapGroup("/api/modulea").MapModuleAEndpoints();
app.Run();
```

### Shared Hosting Extension

```csharp
public static class BlazorHostingExtensions
{
    public static WebApplicationBuilder AddSuiteBlazor(this WebApplicationBuilder builder)
    {
        builder.Services.AddRazorComponents().AddInteractiveServerComponents();
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

```razor
@inherits LayoutComponentBase
@using MySuite.Web.Shared.Components
@inject I18nService I18n

<MudThemeProvider Theme="@AppTheme.Default"
                  @bind-IsDarkMode="_isDarkMode"
                  ObserveSystemDarkModeChange="false" />
<MudPopoverProvider />

<SuiteLayout Title="@I18n["moduleA.title"]">
    <NavMenuContent><ModuleANavMenu /></NavMenuContent>
    <BodyContent>@Body</BodyContent>
</SuiteLayout>

@code { private bool _isDarkMode; }
```

Shared `SuiteLayout` renders app bar, MudDrawer, user menu, tenant selector. Each module injects own nav + page content.

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
| **SSO** | OIDC with per-app Keycloak clients in shared realm | All apps same realm |
| **Auth Cookie** | Shared `Cookie.Name` across apps | Same name + shared DataProtection |
| **DataProtection** | Redis-backed key ring, same `SetApplicationName` | Identical app name |
| **Theme** | Shared RCL `AppTheme` + CSS variables | Single source of truth |
| **Layout** | Shared `SuiteLayout` in RCL, module nav via render fragments | RCL references only |
| **Navigation** | "Back to Launchpad" link in each module | Cross-app links use full URLs |
| **Tenancy** | Finbuckle cookie-based, shared `__tenant__` cookie | Same cookie name across apps |
| **I18n** | Per-app JSON or shared RCL I18nService | Consistent key naming |
| **Health** | Each app exposes `/health` endpoint | Docker health checks |

### Docker Compose

Each app as separate service with own health check:
```yaml
services:
  postgres:   image: postgres:16-alpine
  redis:      image: dragonflydb/dragonfly
  keycloak:   image: quay.io/keycloak/keycloak:26
  launchpad:
    build: ./src/MySuite.Launchpad
    ports: ["5100:5100"]
    depends_on: [postgres, redis, keycloak]
    healthcheck: { test: ["CMD", "curl", "-f", "http://localhost:5100/health"], interval: 30s }
  module-a:
    build: ./src/MySuite.App.ModuleA
    ports: ["5101:5101"]
    depends_on: [postgres, redis, keycloak]
    healthcheck: { test: ["CMD", "curl", "-f", "http://localhost:5101/health"], interval: 30s }
  module-b:
    build: ./src/MySuite.App.ModuleB
    ports: ["5102:5102"]
    depends_on: [postgres, redis, keycloak]
    healthcheck: { test: ["CMD", "curl", "-f", "http://localhost:5102/health"], interval: 30s }
```

---

## 16. Policy-Based Component Authorization

Wrap `AuthorizeView` in reusable component accepting policy or role. Avoids scattering auth logic across pages.

**AuthorizeGate.razor**:
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
        @if (NotAuthorized is not null) { @NotAuthorized }
        else { <MudAlert Severity="Severity.Warning">You do not have permission to view this content.</MudAlert> }
    </NotAuthorized>
</AuthorizeView>
```

**Usage**:
```razor
<AuthorizeGate Policy="admin-only" Role="admin">
    <MudButton Variant="Variant.Filled" Color="Color.Error" OnClick="DeleteAll">Delete All</MudButton>
</AuthorizeGate>
```

Pass `Policy` for claims-based, `Role` for simple role checks, or both. Custom `NotAuthorized` fragment overrides default warning.

---

## 17. Hybrid Blazor Render Modes and SSR Gotchas

### Per-Page `@rendermode` Directive

If `App.razor` has NO global `@rendermode` on `<Routes>`, every page SSR-only by default. Pages with interactive event handlers MUST add `@rendermode InteractiveServer`:

```razor
@page "/patient/book"
@rendermode InteractiveServer
@using static Microsoft.AspNetCore.Components.Web.RenderMode

<MudButton OnClick="SearchPsychologists">Search</MudButton>
<MudDatePicker @bind-Date="SelectedDate" />
```

**Without directive**: static HTML. Buttons dead (form GET/POST instead of C# handlers). Two-way bindings don't update.

**When to use per-page vs global rendermode:**

| Approach | When |
|----------|------|
| Global `@rendermode="InteractiveServer"` on `<Routes>` | Blazor Server only. Every page interactive. |
| Per-page `@rendermode InteractiveServer` | Hybrid (Server+WASM). Public pages SSR for perf; role-gated pages opt in. |
| `@rendermode InteractiveAuto` | Pages working in both Server and WASM. Dependencies available in both. |

**Prerequisite**: Register both render modes:
```csharp
builder.Services.AddRazorComponents()
    .AddInteractiveServerRenderMode()
    .AddInteractiveWebAssemblyRenderMode();
```

And `_Imports.razor`:
```razor
@using static Microsoft.AspNetCore.Components.Web.RenderMode
```

**Safety**: Per-page `InteractiveServer` locks page out of WASM rendering. For auth-gated pages relying on server-side cookies, this is correct.

### MudDialog + SSR Layout Limitation

SSR layout rendering: `MudSnackbarProvider` in SSR layout cannot display snackbars triggered by interactive child components. Layout render tree fixed at SSR time.

**Symptoms**: `Snackbar.Add("Success!")` executes without error, no snackbar appears.

**Workaround**: Replace `MudDialog` with conditional `MudPaper` panel. Verify action via DOM state changes:
```razor
@if (_showConfirmPanel)
{
    <MudPaper Class="pa-4 mt-4" Elevation="3">
        <MudText>Are you sure you want to withdraw consent?</MudText>
        <MudButton Color="Color.Error" OnClick="ConfirmWithdraw">Confirm</MudButton>
        <MudButton Variant="Variant.Text" OnClick="@(() => _showConfirmPanel = false)">Cancel</MudButton>
    </MudPaper>
}
```

**Testing**: Playwright E2E — verify panel element removed from DOM:
```csharp
await page.Locator("text=Are you sure").WaitForAsync(new() { State = WaitForSelectorState.Visible });
await page.Locator("button:has-text('Confirm')").ClickAsync();
await Assertions.Expect(page.Locator("text=Are you sure")).ToBeHiddenAsync();
```

### MudList `@bind-SelectedValue` Testing Limitation

Clicking `.mud-list-item` via Playwright does NOT trigger `MudList @bind-SelectedValue`. Known MudBlazor v8 + Blazor Server + Playwright gap.

**Workaround**: Test list selection via integration/unit tests. Mark E2E test skipped with explanation:
```csharp
[Fact(Skip = "MudList @bind-SelectedValue untestable via Playwright in Blazor Server — manual verification required")]
```

---

## Anti-Patterns

- **DO NOT call `IJSRuntime` during `OnInitializedAsync`**. JS interop unavailable during SSR prerender. Use `OnAfterRenderAsync(firstRender: true)` instead. Violation causes `InvalidOperationException`.
- **DO NOT use `eval()` or `document.cookie` for reading cookies in production**. Exposes XSS-based session hijacking. Use `IJSObjectReference` ES modules with `HttpOnly` cookies server-side.
- **DO NOT create separate theme services per modular app**. Multi-app architecture must use same shared theme class from Shared RCL. Duplication causes visual inconsistency.
- **DO NOT call `StateHasChanged()` inside rapidly-firing `OnChange` handlers**. Debounce first, then call once after debounce period.
- **DO NOT use `[CascadingParameter]` for frequently-changing data across component tree**. Every change re-renders all descendants. Use scoped state container with explicit subscription instead.
- **DO NOT catch and swallow `ObjectDisposedException`**. Log or use `CancellationToken` to cancel gracefully.
- **NEVER skip PropertyChanged subscription in component base**. Without subscribing to `ViewModelBase.PropertyChanged`, UI won't re-render. Subscribe and call `InvokeAsync(StateHasChanged)`.