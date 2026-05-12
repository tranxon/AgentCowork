//! Tool execution module
//!
//! Extracted from [`super::loop_`] to provide reusable parallel tool execution
//! shared between production [`AgentLoop::run`] and debug `DebugSessionTask`.
//!
//! Handles:
//! - Permission validation (Phase 1)
//! - Parallel execution with per-tool timeout + iteration deadline (Phase 2-3)
//! - Result collection with original ordering

use std::sync::Arc;

use rollball_core::providers::traits::ToolCall;
use rollball_core::tools::traits::Tool;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use super::loop_::AgentLoop;

impl AgentLoop {
    /// Execute tool calls in parallel with per-tool timeout and iteration-level deadline.
    ///
    /// Phase 1: Permission check (batch — each tool checked independently)
    /// Phase 2: Approval gate (placeholder for future)
    /// Phase 3: Parallel execution with spawn + select + deadline
    ///
    /// Returns results in the same order as input tool calls.
    /// Individual tool failures are captured as error strings, not propagated.
    pub(crate) async fn execute_tools_parallel(
        &self,
        tool_calls: &[ToolCall],
    ) -> Vec<String> {
        if tool_calls.is_empty() {
            return Vec::new();
        }

        tracing::info!(
            tool_calls_count = tool_calls.len(),
            tools = ?tool_calls
                .iter()
                .map(|t| &t.function.name)
                .collect::<Vec<_>>(),
            "Executing tool calls"
        );

        // Phase 1: Permission check (batch)
        // Check each tool independently; denied tools get error results,
        // allowed tools proceed to parallel execution.
        let mut permission_results: Vec<Option<String>> =
            Vec::with_capacity(tool_calls.len());
        for tool_call in tool_calls {
            match crate::tools::permission::validate_permission(
                &self.core.manifest,
                &tool_call.function.name,
            ) {
                Ok(()) => permission_results.push(None),
                Err(e) => {
                    tracing::warn!(
                        "Permission denied for tool '{}': {}",
                        tool_call.function.name,
                        e
                    );
                    permission_results.push(Some(format!(
                        "Error: Permission denied — {}",
                        e
                    )));
                }
            }
        }

        // Collect indices of tools that passed permission check
        let allowed_indices: Vec<usize> = permission_results
            .iter()
            .enumerate()
            .filter_map(|(i, result)| if result.is_none() { Some(i) } else { None })
            .collect();

        // If no tools passed permission, return all error results immediately
        if allowed_indices.is_empty() {
            return permission_results
                .into_iter()
                .map(|r| r.unwrap_or_default())
                .collect();
        }

        // Phase 2: Approval gate (placeholder for future)
        // TODO(Phase 3): Implement approval gate for high-risk tools

        // Phase 3: Parallel execution with spawn + select + deadline
        let tool_timeout = Duration::from_millis(self.core.config.tool_timeout_ms);
        let iteration_timeout =
            Duration::from_millis(self.core.config.iteration_timeout_ms);

        // Channel to collect results from spawned tasks
        let (tx, mut rx) = mpsc::channel::<(usize, String)>(tool_calls.len());

        // Spawn each allowed tool as an independent task
        let handles: Vec<tokio::task::JoinHandle<()>> = allowed_indices
            .iter()
            .map(|&idx| {
                let tools = self.core.tools.clone();
                let tc = tool_calls[idx].clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let result = match tokio::time::timeout(
                        tool_timeout,
                        execute_single_tool(&tools, &tc),
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => format!(
                            "Error: Tool '{}' timed out after {}ms",
                            tc.function.name,
                            tool_timeout.as_millis()
                        ),
                    };
                    let _ = tx.send((idx, result)).await;
                })
            })
            .collect();

        // Drop the remaining sender so rx.recv() returns None when all tasks complete
        drop(tx);

        // Collect results with iteration-level deadline
        let deadline = Instant::now() + iteration_timeout;
        let mut collected: Vec<(usize, String)> =
            Vec::with_capacity(allowed_indices.len());
        let total = allowed_indices.len();

        while collected.len() < total {
            tokio::select! {
                // A result arrived from a spawned task
                entry = rx.recv() => {
                    match entry {
                        Some((idx, result)) => collected.push((idx, result)),
                        None => break, // All senders dropped
                    }
                }
                // Iteration-level deadline exceeded
                _ = tokio::time::sleep_until(deadline) => {
                    tracing::warn!(
                        "Iteration timeout reached ({}ms), aborting {} remaining tool(s)",
                        iteration_timeout.as_millis(),
                        total - collected.len()
                    );
                    // Abort all remaining spawned tasks
                    for handle in &handles {
                        handle.abort();
                    }
                    break;
                }
            }
        }

        // Build final results in original order
        let results: Vec<String> = permission_results
            .into_iter()
            .enumerate()
            .map(|(idx, perm_result)| {
                if let Some(err) = perm_result {
                    // Permission-denied tool
                    err
                } else if let Some(pos) =
                    collected.iter().find(|(i, _)| *i == idx)
                {
                    // Tool that completed successfully or with error
                    pos.1.clone()
                } else {
                    // Tool that didn't complete due to iteration timeout
                    format!(
                        "Error: iteration timed out, tool {} not completed",
                        tool_calls[idx].function.name
                    )
                }
            })
            .collect();

        // If iteration timed out with incomplete tools, add a system note
        let incomplete_count = results
            .iter()
            .filter(|r| r.contains("iteration timed out"))
            .count();
        if incomplete_count > 0 {
            tracing::warn!(
                incomplete_count,
                "Iteration timed out with incomplete tool(s)"
            );
        }

        results
    }
}

/// Execute a single tool call against the tool registry.
///
/// Returns the result content string (success or error message).
pub(crate) async fn execute_single_tool(tools: &[Arc<dyn Tool>], tool_call: &ToolCall) -> String {
    let tool_name = &tool_call.function.name;
    let params_str = &tool_call.function.arguments;

    // Parse arguments before tool lookup — we need to detect the
    // TOOL_CALL_INCOMPLETE marker (valid JSON injected by streaming assembler)
    // and reject genuinely unparseable arguments (e.g. LLM hallucinated output).
    // Early return on parse failure avoids the old silent "{}" degradation
    // that caused LLM retry loops.
    let params: serde_json::Value = match serde_json::from_str(params_str) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                tool = %tool_name,
                params_len = params_str.len(),
                params_preview = %&params_str[..params_str.len().min(200)],
                error = %e,
                "Failed to parse tool call arguments as JSON — returning error to LLM"
            );
            return format!(
                "Error: Tool '{}' arguments are not valid JSON and could not be parsed: {}. \
                 This call was NOT executed. \
                 Arguments preview (first 200 bytes): {}",
                tool_name,
                e,
                &params_str[..params_str.len().min(200)]
            );
        }
    };

    // Detect truncated/incomplete tool calls: the streaming assembler injects a
    // special error marker when arguments are truncated. Skip actual execution
    // and return a clear error so the LLM knows to regenerate (not retry blindly).
    if let Some("TOOL_CALL_INCOMPLETE") =
        params.get("error").and_then(|v| v.as_str())
    {
        return params
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Tool call arguments were truncated during streaming")
            .to_string();
    }

    // Find the tool
    let tool = tools.iter().find(|t| {
        let spec = t.spec();
        spec.name == *tool_name
    });

    match tool {
        Some(tool) => match tool.execute(params).await {
            Ok(result) => {
                if result.ok {
                    result.content
                } else {
                    // Include both the error output (stdout/stderr) and the error code
                    if result.content.is_empty() {
                        format!("Error: {}", result.error.unwrap_or_default())
                    } else {
                        format!(
                            "{}\nError: {}",
                            result.content,
                            result.error.as_deref().unwrap_or("unknown error")
                        )
                    }
                }
            }
            Err(e) => format!("Tool execution error: {e}"),
        },
        None => format!("Unknown tool: {tool_name}"),
    }
}
