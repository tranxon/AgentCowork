//! Tool permission validation

use rollball_core::{AgentManifest, Permission};

/// Validate tool call against manifest permissions
pub fn validate_permission(
    manifest: &AgentManifest,
    tool_name: &str,
) -> Result<(), String> {
    // TODO: Implement permission checking
    unimplemented!()
}
