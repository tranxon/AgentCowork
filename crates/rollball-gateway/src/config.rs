//! Gateway configuration

use serde::{Deserialize, Serialize};

/// Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub socket_path: String,
    pub vault_dir: String,
    pub packages_dir: String,
    #[serde(default)]
    pub log_level: String,
}
