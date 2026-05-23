//! MCP Catalog HTTP API handlers
//!
//! Manages the global MCP server catalog — a shared registry of server
//! definitions (including credentials/API keys) that all agents can
//! selectively activate. Analogous to the Vault for LLM providers.
//!
//! - GET    /api/mcp-catalog         — list all catalog entries (env values masked)
//! - PUT    /api/mcp-catalog         — replace the entire catalog
//! - POST   /api/mcp-catalog         — add a single server entry
//! - DELETE /api/mcp-catalog/{name}   — remove a server entry

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::http::routes::{ApiError, AppState};
use rollball_core::protocol::McpServerConfigDef;

/// Build the MCP catalog router
pub fn mcp_catalog_routes() -> Router<AppState> {
    Router::new()
        .route("/api/mcp-catalog", get(list_catalog).put(replace_catalog).post(add_catalog_entry))
        .route("/api/mcp-catalog/{name}", delete(remove_catalog_entry).put(update_catalog_entry))
}

// ── Persistence helpers ──────────────────────────────────────────────

/// Build the path to the MCP catalog file.
fn catalog_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("mcp_catalog.json")
}

/// Load the MCP catalog from disk.
/// Returns an empty Vec if the file does not exist.
pub fn load_mcp_catalog(data_dir: &std::path::Path) -> Result<Vec<McpServerConfigDef>, String> {
    let path = catalog_path(data_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read MCP catalog: {}", e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse MCP catalog: {}", e))
}

/// Save the MCP catalog to disk.
pub fn save_mcp_catalog(
    data_dir: &std::path::Path,
    catalog: &[McpServerConfigDef],
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(catalog)
        .map_err(|e| format!("Failed to serialize MCP catalog: {}", e))?;
    std::fs::write(catalog_path(data_dir), json)
        .map_err(|e| format!("Failed to write MCP catalog: {}", e))?;
    tracing::info!(count = catalog.len(), "MCP catalog saved");
    Ok(())
}

// ── Masking helper ───────────────────────────────────────────────────

/// Mask sensitive env values for API responses.
/// Returns a copy of the config with env values containing "key", "token",
/// "secret", or "password" in their key name replaced with "••••".
fn mask_sensitive_env(config: &McpServerConfigDef) -> McpServerConfigDef {
    let sensitive_keywords = ["key", "token", "secret", "password"];
    let masked_env: std::collections::HashMap<String, String> = config
        .env
        .iter()
        .map(|(k, v)| {
            let lower = k.to_lowercase();
            let is_sensitive = sensitive_keywords.iter().any(|kw| lower.contains(kw));
            (k.clone(), if is_sensitive { "••••".to_string() } else { v.clone() })
        })
        .collect();

    McpServerConfigDef {
        name: config.name.clone(),
        transport: config.transport.clone(),
        url: config.url.clone(),
        command: config.command.clone(),
        args: config.args.clone(),
        env: masked_env,
        headers: config.headers.clone(),
        tool_timeout_secs: config.tool_timeout_secs,
    }
}

// ── Response types ───────────────────────────────────────────────────

/// Catalog entry response (env values with sensitive fields masked)
#[derive(Serialize)]
pub struct McpCatalogEntryResponse {
    #[serde(flatten)]
    pub config: McpServerConfigDef,
    /// Whether this entry has sensitive env vars that are masked
    pub has_secrets: bool,
}

/// Full catalog response
#[derive(Serialize)]
pub struct McpCatalogResponse {
    pub servers: Vec<McpCatalogEntryResponse>,
}

/// Request to add a single MCP server entry
#[derive(Deserialize)]
pub struct AddCatalogEntryRequest {
    #[serde(flatten)]
    pub config: McpServerConfigDef,
}

/// Request to update a single MCP server entry
#[derive(Deserialize)]
pub struct UpdateCatalogEntryRequest {
    #[serde(flatten)]
    pub config: McpServerConfigDef,
}

/// Generic message response
#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// ── Handlers ──────────────────────────────────────────────────────────

/// `GET /api/mcp-catalog` — list all MCP server definitions (env values masked)
pub async fn list_catalog(
    State(state): State<AppState>,
) -> Result<Json<McpCatalogResponse>, (StatusCode, Json<ApiError>)> {
    let data_dir = get_data_dir(&state).await?;
    let catalog = load_mcp_catalog(&data_dir)
        .map_err(|e| ApiError::internal(&e))?;

    let sensitive_keywords = ["key", "token", "secret", "password"];
    let servers: Vec<McpCatalogEntryResponse> = catalog
        .iter()
        .map(|c| {
            let masked = mask_sensitive_env(c);
            let has_secrets = c.env.keys().any(|k| {
                let lower = k.to_lowercase();
                sensitive_keywords.iter().any(|kw| lower.contains(kw))
            });
            McpCatalogEntryResponse { config: masked, has_secrets }
        })
        .collect();

    Ok(Json(McpCatalogResponse { servers }))
}

/// `PUT /api/mcp-catalog` — replace the entire catalog
pub async fn replace_catalog(
    State(state): State<AppState>,
    Json(new_catalog): Json<Vec<McpServerConfigDef>>,
) -> Result<Json<McpCatalogResponse>, (StatusCode, Json<ApiError>)> {
    // Validate: no duplicate names
    let mut seen = std::collections::HashSet::new();
    for entry in &new_catalog {
        if !seen.insert(entry.name.clone()) {
            return Err(ApiError::bad_request(&format!(
                "Duplicate MCP server name: '{}'", entry.name
            )));
        }
        if entry.name.is_empty() {
            return Err(ApiError::bad_request("MCP server name must not be empty"));
        }
    }

    let data_dir = get_data_dir(&state).await?;
    save_mcp_catalog(&data_dir, &new_catalog)
        .map_err(|e| ApiError::internal(&e))?;

    // Hot-push MCP config to all running agents
    hot_push_mcp_config(&state).await;

    // Return masked response
    let sensitive_keywords = ["key", "token", "secret", "password"];
    let servers: Vec<McpCatalogEntryResponse> = new_catalog
        .iter()
        .map(|c| {
            let masked = mask_sensitive_env(c);
            let has_secrets = c.env.keys().any(|k| {
                let lower = k.to_lowercase();
                sensitive_keywords.iter().any(|kw| lower.contains(kw))
            });
            McpCatalogEntryResponse { config: masked, has_secrets }
        })
        .collect();

    Ok(Json(McpCatalogResponse { servers }))
}

/// `POST /api/mcp-catalog` — add a single server entry
pub async fn add_catalog_entry(
    State(state): State<AppState>,
    Json(body): Json<AddCatalogEntryRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), (StatusCode, Json<ApiError>)> {
    if body.config.name.is_empty() {
        return Err(ApiError::bad_request("MCP server name must not be empty"));
    }

    let data_dir = get_data_dir(&state).await?;
    let mut catalog = load_mcp_catalog(&data_dir)
        .map_err(|e| ApiError::internal(&e))?;

    // Check for duplicate name
    if catalog.iter().any(|c| c.name == body.config.name) {
        return Err(ApiError::bad_request(&format!(
            "MCP server '{}' already exists in catalog", body.config.name
        )));
    }

    let name = body.config.name.clone();
    catalog.push(body.config);
    save_mcp_catalog(&data_dir, &catalog)
        .map_err(|e| ApiError::internal(&e))?;

    // Hot-push MCP config to all running agents
    hot_push_mcp_config(&state).await;

    Ok((StatusCode::CREATED, Json(MessageResponse {
        message: format!("MCP server '{}' added to catalog", name),
    })))
}

/// `PUT /api/mcp-catalog/{name}` — update a single server entry
pub async fn update_catalog_entry(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<UpdateCatalogEntryRequest>,
) -> Result<Json<MessageResponse>, (StatusCode, Json<ApiError>)> {
    let data_dir = get_data_dir(&state).await?;
    let mut catalog = load_mcp_catalog(&data_dir)
        .map_err(|e| ApiError::internal(&e))?;

    // If the name is being changed, check for conflicts first
    if body.config.name != name {
        if catalog.iter().any(|c| c.name == body.config.name) {
            return Err(ApiError::bad_request(&format!(
                "MCP server '{}' already exists in catalog", body.config.name
            )));
        }
    }

    // Find the existing entry index
    let idx = catalog.iter().position(|c| c.name == name)
        .ok_or_else(|| ApiError::not_found(&format!(
            "MCP server '{}' not found in catalog", name
        )))?;

    // Preserve sensitive env values that were sent as "••••" (masked)
    // If the user didn't change a secret field, keep the old value
    let old_env = catalog[idx].env.clone();
    let merged_env: std::collections::HashMap<String, String> = body
        .config
        .env
        .into_iter()
        .map(|(k, v)| {
            if v == "••••" {
                // Keep the old value for this key
                let old_val = old_env.get(&k).cloned().unwrap_or_default();
                (k, old_val)
            } else {
                (k, v)
            }
        })
        .collect();

    let new_name = body.config.name.clone();
    catalog[idx] = McpServerConfigDef {
        name: body.config.name,
        transport: body.config.transport,
        url: body.config.url,
        command: body.config.command,
        args: body.config.args,
        env: merged_env,
        headers: body.config.headers,
        tool_timeout_secs: body.config.tool_timeout_secs,
    };

    save_mcp_catalog(&data_dir, &catalog)
        .map_err(|e| ApiError::internal(&e))?;

    // Hot-push MCP config to all running agents
    hot_push_mcp_config(&state).await;

    Ok(Json(MessageResponse {
        message: format!("MCP server '{}' updated in catalog", new_name),
    }))
}

/// `DELETE /api/mcp-catalog/{name}` — remove a server entry
pub async fn remove_catalog_entry(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<MessageResponse>, (StatusCode, Json<ApiError>)> {
    let data_dir = get_data_dir(&state).await?;
    let mut catalog = load_mcp_catalog(&data_dir)
        .map_err(|e| ApiError::internal(&e))?;

    let original_len = catalog.len();
    catalog.retain(|c| c.name != name);
    if catalog.len() == original_len {
        return Err(ApiError::not_found(&format!(
            "MCP server '{}' not found in catalog", name
        )));
    }

    save_mcp_catalog(&data_dir, &catalog)
        .map_err(|e| ApiError::internal(&e))?;

    // Hot-push MCP config to all running agents
    hot_push_mcp_config(&state).await;

    Ok(Json(MessageResponse {
        message: format!("MCP server '{}' removed from catalog", name),
    }))
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Get the data_dir from Gateway state
async fn get_data_dir(state: &AppState) -> Result<PathBuf, (StatusCode, Json<ApiError>)> {
    let gw = state.gateway_state.read().await;
    Ok(gw
        .config
        .as_ref()
        .map(|c| PathBuf::from(&c.data_dir))
        .unwrap_or_else(|| PathBuf::from("./data")))
}

/// Hot-push MCP config to all running agents after a catalog change.
///
/// Optimization strategy:
/// 1. Load catalog once (not per-agent)
/// 2. Collect push targets under a brief lock, then release
/// 3. Skip agents with no active MCP servers
/// 4. Concurrent push via `tokio::JoinSet` (O(1) wall-clock vs O(N) serial)
/// 5. Each push clones the `mpsc::Sender` so the SessionManager lock is not held during I/O
async fn hot_push_mcp_config(state: &AppState) {
    use crate::http::agent_config;
    use rollball_core::protocol::GatewayResponse;
    use tokio::task::JoinSet;

    let session_mgr = match &state.session_mgr {
        Some(mgr) => mgr.clone(),
        None => return,
    };

    // ── Phase 1: Collect data under brief locks ──────────────────────
    let agent_ids: Vec<String> = {
        let gw = state.gateway_state.read().await;
        gw.running_agents.keys().cloned().collect()
    };

    let data_dir = match get_data_dir(state).await {
        Ok(d) => d,
        Err(_) => return,
    };

    // Load catalog once
    let catalog = match load_mcp_catalog(&data_dir) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to load MCP catalog for hot-push: {}", e);
            return;
        }
    };

    // Build per-agent merged server lists (skip agents with no MCP)
    // Returns Vec<(agent_id, merged_servers)>
    //
    // Note: `load_agent_config` does synchronous file I/O. We run it on a
    // blocking thread to avoid starving the tokio runtime when many agents
    // are running concurrently.
    let data_dir_clone = data_dir.clone();
    let agent_ids_clone = agent_ids.clone();
    let per_agent_configs: Vec<(String, Vec<McpServerConfigDef>)> =
        tokio::task::spawn_blocking(move || {
            let mut results = Vec::new();
            for agent_id in &agent_ids_clone {
                let per_agent = agent_config::load_agent_config(&data_dir_clone, agent_id)
                    .unwrap_or(None);
                let active_servers = match &per_agent {
                    Some(cfg) => cfg.mcp_servers.clone().unwrap_or_default(),
                    None => Vec::new(),
                };
                results.push((agent_id.clone(), active_servers));
            }
            results
        })
        .await
        .unwrap_or_default();

    let mut push_targets: Vec<(String, Vec<McpServerConfigDef>)> = Vec::new();
    for (agent_id, active_servers) in per_agent_configs {
        if active_servers.is_empty() {
            continue; // Skip agents with no MCP servers
        }

        // Catalog-first merge: if the server name exists in catalog,
        // use the (possibly updated) catalog version. Otherwise keep
        // the per-agent version (backward compat for pre-catalog servers).
        let merged: Vec<McpServerConfigDef> = active_servers
            .into_iter()
            .map(|s| {
                catalog.iter().find(|c| c.name == s.name).cloned().unwrap_or(s)
            })
            .collect();

        push_targets.push((agent_id, merged));
    }

    if push_targets.is_empty() {
        return; // Nothing to push
    }

    // ── Phase 2: Clone senders under brief lock, then release ────────
    let senders: Vec<(String, crate::ipc::session::PushSender)> = {
        let mgr = session_mgr.lock().await;
        push_targets
            .iter()
            .filter_map(|(agent_id, _)| {
                mgr.find_by_agent_id(agent_id)
                    .and_then(|(_, session)| session.push_sender().cloned())
                    .map(|tx| (agent_id.clone(), tx))
            })
            .collect()
    };

    if senders.is_empty() {
        return;
    }

    // Build the push payloads (one per agent, already merged)
    let payload_map: std::collections::HashMap<String, Vec<McpServerConfigDef>> = push_targets
        .into_iter()
        .collect();

    // ── Phase 3: Concurrent push (lock-free) ─────────────────────────
    let mut join_set = JoinSet::new();
    for (agent_id, sender) in senders {
        let merged = match payload_map.get(&agent_id) {
            Some(s) => s.clone(),
            None => continue,
        };
        join_set.spawn(async move {
            let result = sender
                .send(GatewayResponse::RuntimeConfigUpdate {
                    mcp_servers: Some(merged),
                    max_output_tokens: None,
                    max_iterations: None,
                    temperature: None,
                    system_prompt_override: None,
                    active_tools: None,
                    shell_approval_threshold: None,
                    model: None,
                    provider: None,
                })
                .await;
            (agent_id, result.is_ok())
        });
    }

    // Await all pushes and log results
    let mut pushed = 0u32;
    let mut failed = 0u32;
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok((agent_id, true)) => {
                tracing::info!(agent = %agent_id, "Hot-pushed MCP config update");
                pushed += 1;
            }
            Ok((agent_id, false)) => {
                tracing::warn!(agent = %agent_id, "Hot-push MCP config failed (channel closed)");
                failed += 1;
            }
            Err(e) => {
                tracing::warn!("Hot-push MCP config task panicked: {}", e);
                failed += 1;
            }
        }
    }

    if pushed > 0 || failed > 0 {
        tracing::info!(
            pushed,
            failed,
            total = pushed + failed,
            "Hot-push MCP catalog update complete"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_sensitive_env() {
        let config = McpServerConfigDef {
            name: "github".to_string(),
            transport: rollball_core::protocol::McpTransportDef::Stdio,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()],
            env: std::collections::HashMap::from([
                ("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), "ghp_abc123".to_string()),
                ("SOME_OTHER_VAR".to_string(), "visible_value".to_string()),
            ]),
            ..Default::default()
        };

        let masked = mask_sensitive_env(&config);
        assert_eq!(masked.env.get("GITHUB_PERSONAL_ACCESS_TOKEN"), Some(&"••••".to_string()));
        assert_eq!(masked.env.get("SOME_OTHER_VAR"), Some(&"visible_value".to_string()));
    }

    #[test]
    fn test_catalog_save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("rollball-test-mcp-catalog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let catalog = vec![
            McpServerConfigDef {
                name: "filesystem".to_string(),
                transport: rollball_core::protocol::McpTransportDef::Stdio,
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
                ..Default::default()
            },
        ];

        save_mcp_catalog(&dir, &catalog).unwrap();
        let loaded = load_mcp_catalog(&dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "filesystem");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_nonexistent_catalog() {
        let dir = std::env::temp_dir().join(format!("rollball-test-mcp-catalog-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let loaded = load_mcp_catalog(&dir).unwrap();
        assert!(loaded.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
