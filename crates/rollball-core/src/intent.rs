//! Intent message structure for cross-Agent communication

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Intent message for cross-Agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// Target agent ID (or pattern for broadcast)
    pub target: String,
    /// Action name
    pub action: String,
    /// Action parameters
    pub params: Value,
    /// Whether this is async (fire-and-forget)
    #[serde(rename = "async", default)]
    pub async_: bool,
    /// Optional message ID for tracking
    #[serde(default)]
    pub message_id: Option<String>,
    /// Source agent ID (set by Gateway)
    #[serde(default)]
    pub source: Option<String>,
}
