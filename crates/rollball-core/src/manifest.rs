//! manifest.toml data structures for .agent packages

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::permission::Permission;

/// Complete manifest.toml structure for .agent packages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub agent_id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub runtime_version: String,
    pub permissions: Vec<Permission>,
    pub triggers: Vec<Trigger>,
    pub llm: LlmConfig,
    pub memory: MemoryConfig,
    pub identity_deps: Vec<String>,
    pub tools: Vec<ToolDeclaration>,
    pub capabilities: HashMap<String, CapabilityDef>,
    pub resources: ResourceLimits,
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub dev: bool,
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub fallback_providers: Vec<String>,
}

/// Memory system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub retention_days: Option<u32>,
}

fn default_memory_enabled() -> bool {
    true
}

/// Tool declaration in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDeclaration {
    pub name: String,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
}

/// Capability definition for Intent routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDef {
    pub description: String,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
}

/// Resource limits for Agent process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    #[serde(default)]
    pub max_memory_mb: Option<u64>,
    #[serde(default)]
    pub max_cpu_percent: Option<f64>,
    #[serde(default)]
    pub idle_timeout_secs: Option<u64>,
}

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub work_dir: Option<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}

/// Trigger definition (cron, event-based, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    #[serde(rename = "type")]
    pub trigger_type: String,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_deserialize() {
        let toml_str = r#"
            agent_id = "com.example.weather"
            version = "1.0.0"
            name = "Weather Agent"
            description = "A weather query agent"
            author = "test"
            runtime_version = "0.1.0"
            permissions = []
            triggers = []
            identity_deps = []
            tools = []
            capabilities = {}
            
            [llm]
            provider = "openai"
            model = "gpt-4"
            
            [memory]
            enabled = true
            
            [resources]
            
            [sandbox]
            enabled = false
        "#;
        
        let manifest: AgentManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.agent_id, "com.example.weather");
        assert_eq!(manifest.llm.provider, "openai");
    }
}
