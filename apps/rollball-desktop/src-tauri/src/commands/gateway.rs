//! Gateway health and status commands

use tauri::State;

use crate::gateway_client::{HealthResponse, SystemStatusResponse};
use crate::state::AppState;

/// Check Gateway health
#[tauri::command]
pub async fn gateway_health(state: State<'_, AppState>) -> Result<HealthResponse, String> {
    let client = state.gateway.read().await;
    let result = client.health().await;
    match &result {
        Ok(health) => {
            let mut status = state.status.write().await;
            *status = crate::state::GatewayStatus::Connected;
            Ok(health.clone())
        }
        Err(e) => {
            let mut status = state.status.write().await;
            *status = crate::state::GatewayStatus::Error(e.to_string());
            Err(e.to_string())
        }
    }
}

/// Get Gateway system status
#[tauri::command]
pub async fn gateway_status(state: State<'_, AppState>) -> Result<SystemStatusResponse, String> {
    let client = state.gateway.read().await;
    client.system_status().await.map_err(|e| e.to_string())
}
