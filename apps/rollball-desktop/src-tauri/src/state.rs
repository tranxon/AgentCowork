//! Application state shared across Tauri commands

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::gateway_client::GatewayClient;

/// Gateway connection status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayStatus {
    Connected,
    Disconnected,
    Error(String),
}

/// Shared application state
pub struct AppState {
    /// Gateway HTTP client
    pub gateway: Arc<RwLock<GatewayClient>>,
    /// Current Gateway connection status
    pub status: Arc<RwLock<GatewayStatus>>,
}

impl AppState {
    /// Create a new AppState with default Gateway URL
    pub fn new() -> Self {
        Self {
            gateway: Arc::new(RwLock::new(GatewayClient::new())),
            status: Arc::new(RwLock::new(GatewayStatus::Disconnected)),
        }
    }
}
