//! MCP server configuration types.
//!
//! Re-exports [`McpServerConfigDef`] and [`McpTransportDef`] from
//! `rollball_core::protocol` so there is a single source of truth.
//! Compatibility aliases are provided for the old names.

// Single source of truth: rollball-core defines the wire format.
pub use rollball_core::protocol::{McpServerConfigDef, McpTransportDef};

/// Compatibility alias — prefer `McpServerConfigDef` directly.
pub type McpServerConfig = McpServerConfigDef;

/// Compatibility alias — prefer `McpTransportDef` directly.
pub type McpTransport = McpTransportDef;

/// MCP client configuration (top-level, used by config file parsing).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpConfig {
    /// Enable MCP tool loading.
    #[serde(default)]
    pub enabled: bool,
    /// Configured MCP servers.
    #[serde(default, alias = "mcpServers")]
    pub servers: Vec<McpServerConfigDef>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            servers: Vec::new(),
        }
    }
}
