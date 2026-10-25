//! Resource budgets applied to every plugin call.

/// Per-plugin resource limits. Both values are enforced on every single
/// call (each call gets a fresh store, so a leak or hog in one call can
/// never tax the next).
#[derive(Debug, Clone, Copy)]
pub struct PluginLimits {
    /// Hard cap on guest linear memory growth (the wasmtime
    /// `ResourceLimiter`). Default: 32 MiB.
    pub memory_bytes: usize,
    /// Instruction-fuel budget per tool call (wasmtime fuel metering).
    /// Default: 10 million instructions - generous for real work, terminal
    /// for unbounded loops.
    pub fuel_per_call: u64,
}

impl Default for PluginLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 32 * 1024 * 1024,
            fuel_per_call: 10_000_000,
        }
    }
}
