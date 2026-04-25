//! rollball-tool-sdk — SDK for building RollBall WASM tools
//!
//! Provides the `#[tool]` declarative macro and `ToolInput`/`ToolOutput`
//! types for building WASM tools that run in the RollBall sandbox.
//!
//! # Quick Start
//!
//! ```ignore
//! use rollball_tool_sdk::{tool, ToolInput, ToolOutput, ToolError};
//!
//! #[tool(name = "image_filter")]
//! fn execute(input: ToolInput) -> Result<ToolOutput, ToolError> {
//!     let filter: String = input.get("filter")?;
//!     Ok(ToolOutput::from(json!({"status": "ok"})))
//! }
//! ```
//!
//! # Building
//!
//! ```bash
//! cargo build --target wasm32-wasip2 --release
//! ```
//!
//! The resulting .wasm file goes into the .agent package's `tools/` directory.

pub mod tool;
pub mod exports;

// Re-export core types
pub use tool::{ToolInput, ToolOutput, ToolError};
