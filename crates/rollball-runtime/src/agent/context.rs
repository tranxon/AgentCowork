//! Context building (system prompt + history + memory + identity + skills)

use rollball_core::AgentManifest;

/// Build complete context for LLM request
pub fn build_context(manifest: &AgentManifest) -> Result<String, String> {
    // TODO: Implement context building
    unimplemented!()
}
