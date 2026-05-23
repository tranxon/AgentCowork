// Adapted from zeroclaw/src/tools/mcp_tool.rs
// Rollball deviation: implements rollball_core::tools::Tool trait instead

use std::sync::Arc;

use async_trait::async_trait;

use crate::client::McpRegistry;
use crate::protocol::McpToolDef;

/// A RollBall [`Tool`](rollball_core::tools::Tool) backed by an MCP server tool.
///
/// The `prefixed_name` (e.g. `filesystem__read_file`) is what the agent sees.
/// The registry knows how to route it to the correct server.
pub struct McpToolWrapper {
    /// Prefixed name: `<server_name>__<tool_name>`.
    prefixed_name: String,
    /// Description extracted from the MCP tool definition.
    description: String,
    /// JSON schema for the tool's input parameters.
    input_schema: serde_json::Value,
    /// Shared registry — used to dispatch actual tool calls.
    registry: Arc<McpRegistry>,
}

impl McpToolWrapper {
    pub fn new(prefixed_name: String, def: McpToolDef, registry: Arc<McpRegistry>) -> Self {
        let description = def.description.unwrap_or_else(|| "MCP tool".to_string());
        Self {
            prefixed_name,
            description,
            input_schema: def.input_schema,
            registry,
        }
    }
}

#[async_trait]
impl rollball_core::tools::Tool for McpToolWrapper {
    fn spec(&self) -> rollball_core::tools::ToolSpec {
        rollball_core::tools::ToolSpec {
            name: self.prefixed_name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
        }
    }

    async fn execute(
        &self,
        params: serde_json::Value,
    ) -> rollball_core::error::Result<rollball_core::tools::ToolResult> {
        // Strip the `approved` field before forwarding to the MCP server.
        // RollBall's security model may inject `approved: bool` into tool calls
        // for approval flows. MCP servers are unaware of this field.
        let args = match params {
            serde_json::Value::Object(mut map) => {
                map.remove("approved");
                serde_json::Value::Object(map)
            }
            other => other,
        };

        match self.registry.call_tool(&self.prefixed_name, args).await {
            Ok(output) => Ok(rollball_core::tools::ToolResult {
                ok: true,
                content: output,
                error: None,
                token_usage: None,
            }),
            Err(e) => Ok(rollball_core::tools::ToolResult {
                ok: false,
                content: String::new(),
                error: Some(e.to_string()),
                token_usage: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rollball_core::tools::Tool;
    use serde_json::json;

    fn make_def(name: &str, description: Option<&str>, schema: serde_json::Value) -> McpToolDef {
        McpToolDef {
            name: name.to_string(),
            description: description.map(str::to_string),
            input_schema: schema,
        }
    }

    async fn empty_registry() -> Arc<McpRegistry> {
        Arc::new(
            McpRegistry::connect_all(&[])
                .await
                .expect("empty connect_all should succeed"),
        )
    }

    #[tokio::test]
    async fn spec_returns_prefixed_name_and_description() {
        let registry = empty_registry().await;
        let def = make_def("read_file", Some("Reads a file"), json!({}));
        let wrapper = McpToolWrapper::new("filesystem__read_file".to_string(), def, registry);
        let spec = wrapper.spec();
        assert_eq!(spec.name, "filesystem__read_file");
        assert_eq!(spec.description, "Reads a file");
    }

    #[tokio::test]
    async fn description_falls_back_to_default_when_none() {
        let registry = empty_registry().await;
        let def = make_def("mystery", None, json!({}));
        let wrapper = McpToolWrapper::new("srv__mystery".to_string(), def, registry);
        let spec = wrapper.spec();
        assert_eq!(spec.description, "MCP tool");
    }

    #[tokio::test]
    async fn spec_returns_input_schema_as_parameters() {
        let registry = empty_registry().await;
        let schema = json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        });
        let def = make_def("read_file", Some("Read"), schema.clone());
        let wrapper = McpToolWrapper::new("fs__read_file".to_string(), def, registry);
        let spec = wrapper.spec();
        assert_eq!(spec.input_schema, schema);
    }

    #[tokio::test]
    async fn execute_returns_non_fatal_error_for_unknown_tool() {
        let registry = empty_registry().await;
        let def = make_def("ghost", Some("Ghost tool"), json!({}));
        let wrapper = McpToolWrapper::new("nowhere__ghost".to_string(), def, registry);
        let result = wrapper
            .execute(json!({}))
            .await
            .expect("execute should be non-fatal");
        assert!(!result.ok);
        let err_msg = result.error.expect("error message should be present");
        assert!(
            err_msg.contains("unknown MCP tool"),
            "unexpected error: {err_msg}"
        );
        assert!(result.content.is_empty());
    }

    #[tokio::test]
    async fn execute_strips_approved_field_from_object_args() {
        let registry = empty_registry().await;
        let def = make_def("do_thing", Some("Do a thing"), json!({}));
        let wrapper = McpToolWrapper::new("srv__do_thing".to_string(), def, registry);
        let result = wrapper
            .execute(json!({ "approved": true, "param": "value" }))
            .await
            .expect("execute must be non-fatal even with approved field");
        assert!(!result.ok);
        let err = result.error.unwrap_or_default();
        assert!(
            !err.to_lowercase().contains("approved"),
            "approved field should have been stripped, but got: {err}"
        );
    }

    #[tokio::test]
    async fn execute_handles_non_object_args_without_panic() {
        let registry = empty_registry().await;
        let def = make_def("noop", None, json!({}));
        let wrapper = McpToolWrapper::new("srv__noop".to_string(), def, registry);
        for non_obj in [json!(null), json!("a string"), json!([1, 2, 3])] {
            let result = wrapper
                .execute(non_obj.clone())
                .await
                .expect("non-object args must not propagate Err");
            assert!(!result.ok, "expected non-fatal failure for {non_obj}");
        }
    }
}
