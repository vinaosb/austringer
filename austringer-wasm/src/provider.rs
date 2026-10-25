//! The T6 bridge: plugin tools register into
//! [`austringer_core::tools::ToolRegistry`] through the standard
//! [`ToolProvider`] trait - the versioned-contract registry, IoCap byte
//! caps, and the T7 inspector chain downstream apply to plugin tools
//! exactly as to built-in ones.

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::tools::{
    ProviderError, RegistryError, ToolProvider, ToolRegistry, ToolSchema,
};
use serde_json::Value;

use crate::error::WasmError;
use crate::manifest::ToolDecl;
use crate::plugin::PluginHandle;

/// One manifest tool exposed as a T6 provider. Many providers can share a
/// single [`PluginHandle`] (one per tool of the plugin).
pub struct WasmToolProvider {
    handle: PluginHandle,
    schema: ToolSchema,
}

impl WasmToolProvider {
    /// Wrap `tool` of `handle` as a registry-mountable provider.
    pub fn new(handle: PluginHandle, tool: ToolDecl) -> Self {
        Self {
            handle,
            schema: ToolSchema {
                name: tool.name,
                description: tool.description,
                parameters: tool.input_schema,
            },
        }
    }
}

#[async_trait]
impl ToolProvider for WasmToolProvider {
    fn schema(&self) -> ToolSchema {
        self.schema.clone()
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        let encoded =
            serde_json::to_vec(arguments).map_err(|e| ProviderError::InvalidArguments {
                message: format!("arguments not serializable: {e}"),
            })?;
        let report = self.handle.call_raw(&self.schema.name, &encoded);
        for line in &report.logs {
            // Plugin log lines are untrusted text: bounded and lossy at the
            // ABI layer, routed through the tracing facade only.
            tracing::debug!(target: "austringer::wasm", level = ?line.level, "{}", line.text);
        }
        match report.output {
            Ok(bytes) => Ok(bytes),
            Err(WasmError::InputTooLarge { len, capacity }) => {
                Err(ProviderError::InvalidArguments {
                    message: format!(
                        "arguments serialized to {len} bytes, over the {capacity}-byte plugin input region"
                    ),
                })
            }
            Err(e) => Err(ProviderError::Execution {
                message: e.to_string(),
            }),
        }
    }
}

/// Mount every tool of `handle` into the registry, one provider per tool.
/// Name collisions fail typed (`RegistryError::DuplicateTool`) - plugin
/// tools share the global tool namespace with no silent shadowing.
pub fn mount_plugin(
    registry: &mut ToolRegistry,
    handle: &PluginHandle,
) -> Result<(), RegistryError> {
    for tool in &handle.manifest().tools {
        registry.register(Arc::new(WasmToolProvider::new(
            handle.clone(),
            tool.clone(),
        )))?;
    }
    Ok(())
}
