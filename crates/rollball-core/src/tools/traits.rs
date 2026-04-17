//! Tool trait and related types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;

/// Tool specification (name, description, input schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Tool name (unique identifier)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the execution was successful
    pub ok: bool,
    /// Result content (text or structured data)
    pub content: String,
    /// Optional error message
    #[serde(default)]
    pub error: Option<String>,
    /// Token usage statistics
    #[serde(default)]
    pub token_usage: Option<TokenUsage>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Core Tool trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get tool specification
    fn spec(&self) -> ToolSpec;

    /// Get tool name (convenience method)
    fn name(&self) -> String {
        self.spec().name.clone()
    }

    /// Execute the tool with given parameters
    async fn execute(&self, params: Value) -> Result<ToolResult>;
}
