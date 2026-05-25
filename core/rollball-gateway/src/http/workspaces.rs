//! Workspace directory management API
//!
//! Manages additional directories that agents can access beyond their workspace.
//!
//! **ADR-009 (v2)**: Gateway is pure pass-through for workspace config.
//! No persistence to disk. Workspace config is maintained by Agent Runtime
//! (in `agent_workspaces.json`). Gateway caches the config in `RunningAgentInfo`
//! (in-memory only, cleared on disconnect) to serve HTTP API requests.
//! CRUD operations serialize the full config → push `WorkspaceConfigUpdate` via IPC.
//! Agent must be running (HTTP API returns 409 if not).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::path::Path as StdPath;
use uuid::Uuid;

use crate::http::routes::{ApiError, AppState};

/// Workspace directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDir {
    pub id: String,
    pub path: String,
    pub alias: Option<String>,
    pub access: AccessLevel,
    pub added_at: String,
    /// Deprecated: replaced by session-level workspace selection.
    /// Renamed from `is_current` for backward-compatible JSON reading.
    /// Frontend should read `sessionWorkspaceMap` instead.
    #[serde(default, alias = "is_current")]
    pub last_active: bool,
    /// Cumulative selection count for context ranking
    #[serde(default)]
    pub select_count: u32,
    /// Last selection timestamp (RFC3339), None if never selected
    #[serde(default)]
    pub last_selected_at: Option<String>,
}

/// Access level for workspace directories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AccessLevel {
    ReadOnly,
    ReadWrite,
}

/// Workspace configuration file structure (for JSON serialization)
#[derive(Debug, Serialize, Deserialize)]
struct WorkspaceConfig {
    pub version: String,
    pub additional_dirs: Vec<WorkspaceDir>,
}

/// Request to add a workspace directory
#[derive(Debug, Deserialize)]
pub struct AddWorkspaceRequest {
    pub path: String,
    pub alias: Option<String>,
    pub access: AccessLevel,
}

/// Request to set the current (active) workspace
#[derive(Debug, Deserialize)]
pub struct SetCurrentWorkspaceRequest {
    pub workspace_id: String,
}

/// Query parameters for set_current_workspace (optional session_id for per-session selection).
#[derive(Debug, Deserialize, Default)]
pub struct SetCurrentWorkspaceQuery {
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Request to update a workspace directory
#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub access: Option<AccessLevel>,
    pub alias: Option<String>,
}

/// List of workspace directories
#[derive(Debug, Serialize)]
pub struct WorkspaceListResponse {
    pub agent_id: String,
    pub workspaces: Vec<WorkspaceDir>,
}

/// Helper: get workspace config from RunningAgentInfo cache.
/// Returns None if agent not running.
async fn get_cached_config(state: &AppState, agent_id: &str) -> Option<WorkspaceConfig> {
    let gw = state.gateway_state.read().await;
    let info = gw.running_agents.get(agent_id)?;
    let json = info.workspace_config_json.as_ref()?;
    serde_json::from_str(json).ok()
}

/// Helper: push WorkspaceConfigUpdate to Runtime and update the cache.
///
/// ADR-009: IPC push is synchronous — we await the result before updating
/// the in-memory cache. This avoids TOCTOU where the cache shows a config
/// that Runtime never received (e.g. channel closed mid-push).
async fn push_and_cache(state: &AppState, agent_id: &str, config: &WorkspaceConfig) -> Result<(), String> {
    let config_json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    // Push to Runtime via IPC first — only update cache on success
    if let Some(ref session_mgr) = state.session_mgr {
        let push_tx = {
            let mgr = session_mgr.lock().await;
            mgr.find_by_agent_id(agent_id)
                .and_then(|(_, session)| session.push_sender().cloned())
        };
        if let Some(push_tx) = push_tx {
            let push_msg = rollball_core::protocol::GatewayResponse::WorkspaceConfigUpdate {
                config_json: config_json.clone(),
            };
            if push_tx.send(push_msg).await.is_err() {
                tracing::warn!(
                    "Failed to push WorkspaceConfigUpdate to agent={} (channel closed)",
                    agent_id
                );
                return Err(format!(
                    "Agent {} is not reachable (IPC channel closed), cannot update workspace",
                    agent_id
                ));
            }
            tracing::info!(
                "Pushed WorkspaceConfigUpdate to agent={}",
                agent_id
            );
        } else {
            // Agent has no active IPC session — cannot update workspace
            return Err(format!(
                "Agent {} has no active IPC session, cannot update workspace",
                agent_id
            ));
        }
    } else {
        return Err("No session manager available".to_string());
    }

    // IPC push succeeded — now update in-memory cache
    {
        let mut gw = state.gateway_state.write().await;
        if let Some(info) = gw.running_agents.get_mut(agent_id) {
            info.workspace_config_json = Some(config_json);
        }
    }

    Ok(())
}

// ─── Handlers ────────────────────────────────────────────────────────────

/// `GET /api/agents/{agent_id}/workspaces` — list workspace directories for an agent
pub async fn list_workspaces(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<WorkspaceListResponse>, (StatusCode, Json<ApiError>)> {
    // ADR-009 v2: Read from RunningAgentInfo in-memory cache
    // If agent is running → return its workspace config
    // If agent exists but not running → return empty list (per ADR-009)
    // If agent doesn't exist → return 404
    let config = get_cached_config(&state, &agent_id).await;

    match config {
        Some(cfg) => Ok(Json(WorkspaceListResponse {
            agent_id,
            workspaces: cfg.additional_dirs,
        })),
        None => {
            // Check if agent exists (installed but not running)
            let gw = state.gateway_state.read().await;
            if gw.installed_agents.contains_key(&agent_id) {
                // Agent exists but not running → empty list per ADR-009
                Ok(Json(WorkspaceListResponse {
                    agent_id,
                    workspaces: vec![],
                }))
            } else {
                Err(ApiError::not_found("Agent not found"))
            }
        }
    }
}

/// `POST /api/agents/{agent_id}/workspaces` — add a workspace directory
pub async fn add_workspace(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(req): Json<AddWorkspaceRequest>,
) -> Result<(StatusCode, Json<WorkspaceDir>), (StatusCode, Json<ApiError>)> {
    // Validate path exists and is a directory
    if !StdPath::new(&req.path).is_dir() {
        return Err(ApiError::bad_request(&format!("Directory not found: {}", req.path)));
    }

    // Load current config from cache
    let mut config = get_cached_config(&state, &agent_id).await
        .ok_or_else(|| ApiError::not_found("Agent not running — cannot add workspace"))?;

    // Check for duplicate path
    if config.additional_dirs.iter().any(|d| d.path == req.path) {
        return Err(ApiError::bad_request("Directory already exists in workspace list"));
    }

    // Create new entry
    let new_dir = WorkspaceDir {
        id: format!("ws-{}", &Uuid::new_v4().to_string().replace("-", "")[..12]),
        path: req.path.clone(),
        alias: req.alias,
        access: req.access,
        added_at: chrono::Utc::now().to_rfc3339(),
        last_active: false,
        select_count: 0,
        last_selected_at: None,
    };

    let result = new_dir.clone();
    config.additional_dirs.push(new_dir);

    // Push to Runtime + update cache
    push_and_cache(&state, &agent_id, &config).await
        .map_err(|e| ApiError::internal(&e))?;

    Ok((StatusCode::CREATED, Json(result)))
}

/// `PUT /api/agents/{agent_id}/workspaces/{ws_id}` — update a workspace directory
pub async fn update_workspace(
    State(state): State<AppState>,
    Path((agent_id, ws_id)): Path<(String, String)>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> Result<Json<WorkspaceDir>, (StatusCode, Json<ApiError>)> {
    let mut config = get_cached_config(&state, &agent_id).await
        .ok_or_else(|| ApiError::not_found("Agent not running — cannot update workspace"))?;

    // Find and update directory
    let dir = config.additional_dirs.iter_mut()
        .find(|d| d.id == ws_id)
        .ok_or_else(|| ApiError::not_found(&format!("Workspace directory not found: {}", ws_id)))?;

    if let Some(access) = req.access {
        dir.access = access;
    }
    if let Some(alias) = req.alias {
        dir.alias = Some(alias);
    }

    let updated = dir.clone();

    // Push to Runtime + update cache
    push_and_cache(&state, &agent_id, &config).await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(Json(updated))
}

/// `PUT /api/agents/{agent_id}/workspaces/current` — set the current (active) workspace
///
/// Optional query param `session_id` enables per-session workspace selection.
/// When provided, Gateway also sends `SetSessionWorkspace` IPC to the Runtime
/// in addition to the `WorkspaceConfigUpdate` (which updates list stats).
pub async fn set_current_workspace(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<SetCurrentWorkspaceQuery>,
    Json(req): Json<SetCurrentWorkspaceRequest>,
) -> Result<Json<WorkspaceListResponse>, (StatusCode, Json<ApiError>)> {
    let mut config = get_cached_config(&state, &agent_id).await
        .ok_or_else(|| ApiError::not_found("Agent not running — cannot set workspace"))?;

    // Validate workspace: either "__agent_home__" or an existing workspace ID
    let is_agent_home = req.workspace_id == "__agent_home__";
    if !is_agent_home && !config.additional_dirs.iter().any(|d| d.id == req.workspace_id) {
        return Err(ApiError::not_found(&format!(
            "Workspace directory not found: {}",
            req.workspace_id
        )));
    }

    // When session_id is provided, push SetSessionWorkspace to Runtime
    if let Some(ref session_id) = query.session_id {
        if let Some(ref session_mgr) = state.session_mgr {
            let push_tx = {
                let mgr = session_mgr.lock().await;
                mgr.find_by_agent_id(&agent_id)
                    .and_then(|(_, session)| session.push_sender().cloned())
            };
            if let Some(push_tx) = push_tx {
                let push_msg = rollball_core::protocol::GatewayResponse::SetSessionWorkspace {
                    session_id: session_id.clone(),
                    workspace_id: req.workspace_id.clone(),
                };
                if push_tx.send(push_msg).await.is_err() {
                    tracing::warn!(
                        agent_id = %agent_id,
                        session_id = %session_id,
                        "Failed to push SetSessionWorkspace (channel closed)"
                    );
                } else {
                    tracing::info!(
                        agent_id = %agent_id,
                        session_id = %session_id,
                        workspace_id = %req.workspace_id,
                        "Pushed SetSessionWorkspace to Runtime"
                    );
                }
            }
        }
    }

    // Update select_count and last_selected_at for the selected workspace (if it's a user workspace)
    if !is_agent_home {
        let now = chrono::Utc::now().to_rfc3339();
        for dir in &mut config.additional_dirs {
            if dir.id == req.workspace_id {
                dir.last_active = true;
                dir.select_count += 1;
                dir.last_selected_at = Some(now.clone());
            } else {
                dir.last_active = false;
            }
        }
    }

    // Push WorkspaceConfigUpdate to Runtime (updates list stats + cache)
    push_and_cache(&state, &agent_id, &config).await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(Json(WorkspaceListResponse {
        agent_id,
        workspaces: config.additional_dirs,
    }))
}

/// `DELETE /api/agents/{agent_id}/workspaces/{ws_id}` — remove a workspace directory
pub async fn delete_workspace(
    State(state): State<AppState>,
    Path((agent_id, ws_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let mut config = get_cached_config(&state, &agent_id).await
        .ok_or_else(|| ApiError::not_found("Agent not running — cannot delete workspace"))?;

    // Check if exists
    if !config.additional_dirs.iter().any(|d| d.id == ws_id) {
        return Err(ApiError::not_found(&format!("Workspace directory not found: {}", ws_id)));
    }

    // Remove directory
    config.additional_dirs.retain(|d| d.id != ws_id);

    // Push to Runtime + update cache
    push_and_cache(&state, &agent_id, &config).await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(StatusCode::NO_CONTENT)
}

// ─── Routes ─────────────────────────────────────────────────────────────

use axum::routing::put;
use axum::Router;

/// Create workspace management routes
pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/agents/{agent_id}/workspaces",
            get(list_workspaces).post(add_workspace),
        )
        .route(
            "/api/agents/{agent_id}/workspaces/current",
            put(set_current_workspace),
        )
        .route(
            "/api/agents/{agent_id}/workspaces/{ws_id}",
            put(update_workspace).delete(delete_workspace),
        )
}
