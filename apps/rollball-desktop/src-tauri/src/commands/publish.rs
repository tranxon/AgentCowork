//! Publish commands — prepare, build, export

use tauri::State;

use crate::gateway_client::{BuildPublishResponse, ExportPackageResponse, PreparePublishResponse};
use crate::state::AppState;

/// Run publish prepare (checks + optional cleanup)
#[tauri::command]
pub async fn prepare_publish(
    state: State<'_, AppState>,
    agent_id: String,
    clean: Option<bool>,
) -> Result<PreparePublishResponse, String> {
    let client = state.gateway.read().await;
    client
        .prepare_publish(&agent_id, clean.unwrap_or(false))
        .await
        .map_err(|e| format!("Failed to prepare publish: {}", e))
}

/// Build a .agent package
#[tauri::command]
pub async fn build_publish(
    state: State<'_, AppState>,
    agent_id: String,
    sign: Option<bool>,
    key_dir: Option<String>,
) -> Result<BuildPublishResponse, String> {
    let client = state.gateway.read().await;
    client
        .build_publish(&agent_id, sign.unwrap_or(false), key_dir.as_deref())
        .await
        .map_err(|e| format!("Failed to build package: {}", e))
}

/// Export the built .agent file path
#[tauri::command]
pub async fn export_package(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<ExportPackageResponse, String> {
    let client = state.gateway.read().await;
    client
        .export_package(&agent_id)
        .await
        .map_err(|e| format!("Failed to export package: {}", e))
}
