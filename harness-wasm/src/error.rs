//! Typed errors for the WASM plugin runtime. Every failure mode a plugin
//! can produce (or a host can reject) has a named variant - nothing from
//! the sandbox is surfaced as a stringly-typed or panicking error.

use crate::manifest::ManifestError;

/// Everything that can go wrong loading or calling a plugin.
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    /// The wasmtime engine failed to initialize (config/build problem).
    #[error("wasm engine failed to initialize: {detail}")]
    EngineInit {
        /// Failure detail from the engine.
        detail: String,
    },
    /// The plugin bytes are neither valid wasm nor valid WAT.
    #[error("plugin `{plugin}` failed to compile: {detail}")]
    CompileFailed {
        /// Plugin the bytes came from.
        plugin: String,
        /// Parser/compiler detail.
        detail: String,
    },
    /// The module compiled but could not be instantiated (missing exports,
    /// link errors, or an initial memory over the cap).
    #[error("plugin `{plugin}` failed to instantiate: {detail}")]
    InstantiateFailed {
        /// Plugin that failed.
        plugin: String,
        /// Instantiation detail.
        detail: String,
    },
    /// The manifest region is empty - the plugin declares nothing.
    #[error("plugin `{plugin}` declares no manifest")]
    ManifestMissing {
        /// Plugin that declared nothing.
        plugin: String,
    },
    /// The manifest exists but violates the schema.
    #[error("plugin manifest invalid: {0}")]
    Manifest(#[from] ManifestError),
    /// The plugin targets an interface older than the host's minimum.
    #[error(
        "plugin `{plugin}` interface {declared} is older than the minimum supported ({min}); rebuild the plugin against interface {min}"
    )]
    InterfaceTooOld {
        /// Plugin name.
        plugin: String,
        /// Interface the plugin declares.
        declared: u8,
        /// Oldest interface this host runs.
        min: u8,
    },
    /// The plugin targets an interface newer than the host's maximum.
    #[error(
        "plugin `{plugin}` interface {declared} is newer than this host supports (max {max}); upgrade the host to run this plugin"
    )]
    InterfaceTooNew {
        /// Plugin name.
        plugin: String,
        /// Interface the plugin declares.
        declared: u8,
        /// Newest interface this host runs.
        max: u8,
    },
    /// Declared-but-not-granted capability: the load fails, never escalates.
    #[error(
        "plugin `{plugin}` declares capability `{capability}` that was not granted; grant it explicitly or rebuild the plugin without it"
    )]
    CapabilityNotGranted {
        /// Plugin name.
        plugin: String,
        /// The ungranted capability.
        capability: String,
    },
    /// The plugin requires a preopen scope the host did not supply.
    #[error("preopen scope `{scope}` required by plugin `{plugin}` was not supplied by the host")]
    PreopenUnsatisfied {
        /// Plugin name.
        plugin: String,
        /// Missing scope name.
        scope: String,
    },
    /// The plugin needs read-write access to a scope granted read-only.
    #[error("preopen scope `{scope}` requires read-write but the host granted read-only")]
    PreopenModeDenied {
        /// Plugin name.
        plugin: String,
        /// The scope with the mode mismatch.
        scope: String,
    },
    /// Linear memory exceeded the configured cap; the call was terminated.
    #[error("plugin `{plugin}` exceeded its {limit_bytes}-byte memory budget and was terminated")]
    MemoryCapExceeded {
        /// Plugin name.
        plugin: String,
        /// The enforced limit in bytes.
        limit_bytes: usize,
    },
    /// Instruction fuel ran out; the call was terminated.
    #[error(
        "plugin `{plugin}` exhausted its {budget} instruction fuel budget (consumed ~{consumed}) and was terminated"
    )]
    FuelExhausted {
        /// Plugin name.
        plugin: String,
        /// The fuel budget that was set for the call.
        budget: u64,
        /// Fuel consumed before termination.
        consumed: u64,
    },
    /// The guest trapped for a reason other than fuel or the memory cap.
    #[error("plugin `{plugin}` tool `{tool}` trapped: {detail}")]
    GuestTrap {
        /// Plugin name.
        plugin: String,
        /// Tool being called.
        tool: String,
        /// Trap detail.
        detail: String,
    },
    /// The guest returned a negative tool result code.
    #[error("plugin `{plugin}` tool `{tool}` failed with guest code {code}")]
    GuestError {
        /// Plugin name.
        plugin: String,
        /// Tool being called.
        tool: String,
        /// Guest-defined error code.
        code: i32,
    },
    /// The guest claimed more output than the interface allows.
    #[error(
        "plugin `{plugin}` claimed a {len}-byte result, over the {capacity}-byte interface limit"
    )]
    OutputTooLarge {
        /// Plugin name.
        plugin: String,
        /// Claimed length.
        len: usize,
        /// Maximum allowed.
        capacity: usize,
    },
    /// Host-side input exceeded the interface input region.
    #[error("tool input of {len} bytes exceeds the {capacity}-byte plugin input region")]
    InputTooLarge {
        /// Actual length.
        len: usize,
        /// Maximum allowed.
        capacity: usize,
    },
    /// The tool name is not declared in the plugin manifest.
    #[error("plugin `{plugin}` declares no tool `{tool}`")]
    ToolNotDeclared {
        /// Plugin name.
        plugin: String,
        /// Unknown tool name.
        tool: String,
    },
    /// Filesystem error on the host side of a load or reload.
    #[error("host file error: {detail}")]
    HostIo {
        /// io::Error detail.
        detail: String,
    },
    /// The reload watcher thread could not be spawned.
    #[error("failed to spawn plugin watcher: {detail}")]
    WatcherSpawn {
        /// io::Error detail.
        detail: String,
    },
}
