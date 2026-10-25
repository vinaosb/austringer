//! The wasmtime embedding core: engine construction (fuel always on),
//! the 32 MiB `ResourceLimiter`, per-call stores, and the invoke loop that
//! maps every trap to a typed [`crate::error::WasmError`].
//!
//! Each call gets a FRESH store + instance: fuel and memory budgets are
//! per-call by construction, a leak or hog in one call cannot tax the
//! next, and hot-reload swaps only affect calls that have not yet
//! snapshotted their module.

use wasmtime::{Engine, Instance, Linker, Memory, Module, ResourceLimiter, Store, Trap};

use crate::abi;
use crate::error::WasmError;
use crate::hostfns::{HostCtx, LogLine, ResolvedScope};
use crate::hosttypes::HostEvent;
use crate::limits::PluginLimits;

/// A shared wasmtime engine with fuel metering enabled. Cheap to keep per
/// plugin handle; all stores instantiated from it enforce fuel.
pub struct WasmEngine {
    engine: Engine,
}

impl WasmEngine {
    /// Build the engine. Fuel consumption is enabled engine-wide; the
    /// per-call budget is set on each store in [`invoke`].
    pub fn new() -> Result<Self, WasmError> {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|e| WasmError::EngineInit {
            detail: e.to_string(),
        })?;
        Ok(Self { engine })
    }

    /// Compile wasm bytes or WAT text (the `wat` feature parses `.wat`
    /// natively - fixtures are authored in WAT).
    pub fn compile(&self, plugin: &str, bytes: &[u8]) -> Result<Module, WasmError> {
        Module::new(&self.engine, bytes).map_err(|e| WasmError::CompileFailed {
            plugin: plugin.into(),
            detail: e.to_string(),
        })
    }
}

/// Enforces the per-call memory budget. Sets `denied` when a growth
/// request is refused so traps can be attributed to the cap.
pub(crate) struct MemLimiter {
    limit_bytes: usize,
    denied: bool,
}

impl ResourceLimiter for MemLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.limit_bytes {
            self.denied = true;
            return Ok(false);
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= 10_000)
    }
}

/// Per-store host state: the limiter plus the host-function context.
pub(crate) struct StoreData {
    /// Memory budget enforcement.
    pub limiter: MemLimiter,
    /// Host-fn surface state (scopes, logs, audit events).
    pub host: HostCtx,
}

/// Everything one tool call produced, typed.
#[derive(Debug)]
pub struct CallReport {
    /// The tool output bytes, or the typed error that terminated the call.
    pub output: Result<Vec<u8>, WasmError>,
    /// Fuel consumed by the call (budget minus remaining).
    pub fuel_consumed: u64,
    /// Bounded log lines the plugin emitted (untrusted, lossy UTF-8).
    pub logs: Vec<LogLine>,
    /// Host-side audit events (file reads/writes/denials).
    pub events: Vec<HostEvent>,
}

impl CallReport {
    fn failed(error: WasmError) -> Self {
        Self {
            output: Err(error),
            fuel_consumed: 0,
            logs: Vec::new(),
            events: Vec::new(),
        }
    }
}

pub(crate) fn fresh_store(
    module: &Module,
    limits: &PluginLimits,
) -> Result<(Store<StoreData>, Linker<StoreData>), WasmError> {
    let mut store = Store::new(
        module.engine(),
        StoreData {
            limiter: MemLimiter {
                limit_bytes: limits.memory_bytes,
                denied: false,
            },
            host: HostCtx::new(Vec::new()),
        },
    );
    store.limiter(|data| &mut data.limiter);
    store
        .set_fuel(limits.fuel_per_call)
        .map_err(|e| WasmError::EngineInit {
            detail: e.to_string(),
        })?;
    let mut linker = Linker::new(module.engine());
    crate::hostlink::register(&mut linker).map_err(|e| WasmError::EngineInit {
        detail: e.to_string(),
    })?;
    Ok((store, linker))
}

pub(crate) fn instantiate(
    store: &mut Store<StoreData>,
    linker: &Linker<StoreData>,
    module: &Module,
    plugin: &str,
    scopes: Vec<ResolvedScope>,
) -> Result<(Instance, Memory), WasmError> {
    store.data_mut().host = HostCtx::new(scopes);
    let instance = linker.instantiate(&mut *store, module).map_err(|e| {
        if store.data().limiter.denied {
            WasmError::MemoryCapExceeded {
                plugin: plugin.into(),
                limit_bytes: store.data().limiter.limit_bytes,
            }
        } else {
            WasmError::InstantiateFailed {
                plugin: plugin.into(),
                detail: e.to_string(),
            }
        }
    })?;
    let memory =
        instance
            .get_memory(&mut *store, "memory")
            .ok_or_else(|| WasmError::InstantiateFailed {
                plugin: plugin.into(),
                detail: "no exported linear memory".into(),
            })?;
    Ok((instance, memory))
}

/// Run one tool call end to end. `tool_id` indexes the manifest tools.
pub(crate) fn invoke(
    module: &Module,
    plugin: &str,
    tool_id: u32,
    tool: &str,
    input: &[u8],
    scopes: Vec<ResolvedScope>,
    limits: &PluginLimits,
) -> CallReport {
    if input.len() > abi::INPUT_CAPACITY {
        return CallReport::failed(WasmError::InputTooLarge {
            len: input.len(),
            capacity: abi::INPUT_CAPACITY,
        });
    }
    let (mut store, linker) = match fresh_store(module, limits) {
        Ok(pair) => pair,
        Err(e) => return CallReport::failed(e),
    };
    let (instance, memory) = match instantiate(&mut store, &linker, module, plugin, scopes) {
        Ok(pair) => pair,
        Err(e) => return CallReport::failed(e),
    };
    let budget = limits.fuel_per_call;
    let outcome = run_tool_call(
        &mut store, &instance, &memory, plugin, tool, tool_id, input, budget,
    );
    let consumed = budget.saturating_sub(store.get_fuel().unwrap_or(0));
    let (logs, events) = store.into_data().host.into_parts();
    CallReport {
        output: outcome,
        fuel_consumed: consumed,
        logs,
        events,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_tool_call(
    store: &mut Store<StoreData>,
    instance: &Instance,
    memory: &Memory,
    plugin: &str,
    tool: &str,
    tool_id: u32,
    input: &[u8],
    budget: u64,
) -> Result<Vec<u8>, WasmError> {
    let func = instance
        .get_typed_func::<(i32, i32), i32>(&mut *store, "tool_call")
        .map_err(|e| WasmError::InstantiateFailed {
            plugin: plugin.into(),
            detail: format!("no usable `tool_call` export: {e}"),
        })?;
    if let Err(e) = memory.write(&mut *store, abi::INPUT_OFFSET, input) {
        return Err(WasmError::InstantiateFailed {
            plugin: plugin.into(),
            detail: format!("cannot place input in guest memory: {e}"),
        });
    }
    match func.call(&mut *store, (tool_id as i32, input.len() as i32)) {
        Ok(len) if len < 0 => Err(WasmError::GuestError {
            plugin: plugin.into(),
            tool: tool.into(),
            code: len,
        }),
        Ok(len) => {
            let len = len as usize;
            if len > abi::OUTPUT_CAPACITY {
                return Err(WasmError::OutputTooLarge {
                    plugin: plugin.into(),
                    len,
                    capacity: abi::OUTPUT_CAPACITY,
                });
            }
            let mut out = vec![0_u8; len];
            memory
                .read(&mut *store, abi::OUTPUT_OFFSET, &mut out)
                .map_err(|e| WasmError::GuestTrap {
                    plugin: plugin.into(),
                    tool: tool.into(),
                    detail: format!("output region unreadable: {e}"),
                })?;
            Ok(out)
        }
        Err(e) => {
            let consumed = budget.saturating_sub(store.get_fuel().unwrap_or(0));
            if e.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
                return Err(WasmError::FuelExhausted {
                    plugin: plugin.into(),
                    budget,
                    consumed,
                });
            }
            if store.data().limiter.denied {
                return Err(WasmError::MemoryCapExceeded {
                    plugin: plugin.into(),
                    limit_bytes: store.data().limiter.limit_bytes,
                });
            }
            Err(WasmError::GuestTrap {
                plugin: plugin.into(),
                tool: tool.into(),
                detail: e.to_string(),
            })
        }
    }
}
