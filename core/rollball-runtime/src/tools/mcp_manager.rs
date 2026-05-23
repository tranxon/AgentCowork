//! MCP (Model Context Protocol) manager — connection lifecycle and tool injection.
//!
//! Manages MCP server connections and provides [`McpToolWrapper`] instances
//! that implement the built-in [`Tool`](rollball_core::tools::traits::Tool) trait,
//! enabling MCP tools to be dispatched transparently alongside native RollBall tools.

use std::sync::Arc;

use rollball_core::protocol::McpServerConfigDef;
use rollball_core::tools::traits::Tool;
use rollball_mcp::client::McpRegistry;
use rollball_mcp::config::{McpServerConfig, McpTransport};
use rollball_mcp::wrapper::McpToolWrapper;

/// Converts a wire-format [`McpServerConfigDef`] (from rollball-core) into
/// the rollball-mcp crate's [`McpServerConfig`].
fn convert_config(def: &McpServerConfigDef) -> McpServerConfig {
    McpServerConfig {
        name: def.name.clone(),
        transport: match def.transport {
            rollball_core::protocol::McpTransportDef::Stdio => McpTransport::Stdio,
            rollball_core::protocol::McpTransportDef::Http => McpTransport::Http,
            rollball_core::protocol::McpTransportDef::Sse => McpTransport::Sse,
        },
        url: def.url.clone(),
        command: def.command.clone(),
        args: def.args.clone(),
        env: def.env.clone(),
        headers: def.headers.clone(),
        tool_timeout_secs: def.tool_timeout_secs,
    }
}

/// MCP connection manager.
///
/// Holds a shared [`McpRegistry`] and provides helpers for connecting
/// servers and building tool wrappers.
pub struct McpManager {
    registry: Option<Arc<McpRegistry>>,
}

impl McpManager {
    /// Create an empty MCP manager (no servers connected).
    pub fn new() -> Self {
        Self { registry: None }
    }

    /// Connect to MCP servers and create tool wrappers.
    ///
    /// - `configs`: list of MCP server configurations (wire format).
    ///
    /// Returns a tuple of:
    ///   - `Arc<McpRegistry>` — shared registry for tool dispatch
    ///   - `Vec<McpToolWrapper>` — one wrapper per MCP tool
    ///   - `Vec<(String, serde_json::Value)>` — tool specs for LLM definitions
    ///
    /// On connection failure, individual servers are skipped (logged as errors).
    /// The returned registry may be empty if no servers connected successfully.
    pub async fn connect(
        &mut self,
        configs: &[McpServerConfigDef],
    ) -> (Arc<McpRegistry>, Vec<McpToolWrapper>, Vec<(String, serde_json::Value)>) {
        let mcp_configs: Vec<McpServerConfig> = configs.iter().map(convert_config).collect();

        let registry = match McpRegistry::connect_all(&mcp_configs).await {
            Ok(reg) => Arc::new(reg),
            Err(e) => {
                tracing::error!("MCP manager: failed to connect to any server: {:#}", e);
                Arc::new(
                    McpRegistry::connect_all(&[])
                        .await
                        .unwrap_or_else(|_| panic!("empty connect_all must not fail")),
                )
            }
        };

        // Build tool wrappers and specs from the registry
        let mut wrappers = Vec::new();
        let mut specs = Vec::new();

        for prefixed_name in registry.tool_names() {
            let prefixed = prefixed_name.clone();
            if let Some(def) = registry.get_tool_def(&prefixed).await {
                let wrapper = McpToolWrapper::new(prefixed.clone(), def, registry.clone());
                let spec = wrapper.spec();
                let serialized = serde_json::to_value(&spec).unwrap_or_default();
                specs.push((spec.name.clone(), serialized));
                wrappers.push(wrapper);
            }
        }

        tracing::info!(
            server_count = registry.server_count(),
            tool_count = wrappers.len(),
            "MCP manager: connected"
        );

        self.registry = Some(registry.clone());
        (registry, wrappers, specs)
    }

    /// Get the current MCP registry, if any servers are connected.
    pub fn registry(&self) -> Option<&Arc<McpRegistry>> {
        self.registry.as_ref()
    }

    /// Check whether any MCP servers are connected.
    pub fn is_connected(&self) -> bool {
        self.registry.as_ref().map_or(false, |r| !r.is_empty())
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rollball_core::protocol::McpTransportDef;

    #[test]
    fn convert_config_basic_stdio() {
        let def = McpServerConfigDef {
            name: "test-server".to_string(),
            transport: McpTransportDef::Stdio,
            url: None,
            command: "test-cmd".to_string(),
            args: vec!["--verbose".to_string()],
            env: Default::default(),
            headers: Default::default(),
            tool_timeout_secs: Some(30),
        };
        let cfg = convert_config(&def);
        assert_eq!(cfg.name, "test-server");
        assert_eq!(cfg.command, "test-cmd");
        assert_eq!(cfg.args, vec!["--verbose"]);
        assert_eq!(cfg.tool_timeout_secs, Some(30));
        assert!(matches!(cfg.transport, McpTransport::Stdio));
        assert!(cfg.url.is_none());
    }

    #[test]
    fn convert_config_http_transport() {
        let def = McpServerConfigDef {
            name: "http-srv".to_string(),
            transport: McpTransportDef::Http,
            url: Some("http://localhost:8080/mcp".to_string()),
            command: String::new(),
            args: vec![],
            env: Default::default(),
            headers: Default::default(),
            tool_timeout_secs: None,
        };
        let cfg = convert_config(&def);
        assert!(matches!(cfg.transport, McpTransport::Http));
        assert_eq!(cfg.url, Some("http://localhost:8080/mcp".to_string()));
    }

    #[test]
    fn mcp_manager_default_is_not_connected() {
        let mgr = McpManager::default();
        assert!(!mgr.is_connected());
        assert!(mgr.registry().is_none());
    }

    #[tokio::test]
    async fn connect_empty_yields_empty_registry() {
        let mut mgr = McpManager::new();
        let (registry, wrappers, specs) = mgr.connect(&[]).await;
        assert!(registry.is_empty());
        assert!(wrappers.is_empty());
        assert!(specs.is_empty());
        assert!(!mgr.is_connected());
    }
}
