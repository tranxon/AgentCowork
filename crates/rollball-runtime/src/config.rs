//! Agent Runtime configuration

use serde::{Deserialize, Serialize};

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub agent_id: String,
    pub manifest_path: String,
    pub work_dir: String,
    pub gateway_socket: String,
    #[serde(default)]
    pub dev_mode: bool,
    #[serde(default)]
    pub log_level: String,
}
