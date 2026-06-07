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

// ─── File Tree Explorer API ─────────────────────────────────────────────

/// A single entry in a directory listing (file or subdirectory)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEntry {
    /// File or directory name
    pub name: String,
    /// "file" or "directory"
    #[serde(rename = "type")]
    pub entry_type: String,
    /// File size in bytes (None for directories)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Last modified timestamp (RFC3339, None if unavailable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    /// Number of direct children (only for directories, used for showing expansion arrow)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_count: Option<usize>,
}

/// Query parameters for the tree endpoint
#[derive(Debug, Deserialize, Default)]
pub struct TreeQuery {
    /// Relative path within the workspace root (empty or "." = root)
    #[serde(default)]
    pub path: Option<String>,
    /// Workspace ID to browse. "__agent_home__" or empty = agent home directory.
    #[serde(default)]
    pub workspace_id: Option<String>,
}

/// Response for the tree endpoint
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeResponse {
    /// Absolute path of the workspace root
    pub root: String,
    /// Relative path that was listed
    pub path: String,
    /// Directory entries (directories first, then files, both alphabetical)
    pub entries: Vec<TreeEntry>,
}

/// Resolve the absolute directory path for a tree request, ensuring it stays
/// within the allowed workspace root. Returns `(root, abs_path, rel_path)`.
fn resolve_tree_path(
    root: &str,
    requested_path: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf, String), String> {
    let root = std::path::Path::new(root);
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve workspace root: {}", e))?;

    let rel = requested_path.trim_start_matches("./").trim_start_matches("/");
    let abs = if rel.is_empty() || rel == "." {
        canonical_root.clone()
    } else {
        let candidate = canonical_root.join(rel);
        // Prevent path traversal: the canonicalized path must start with root
        let canonical_candidate = candidate
            .canonicalize()
            .map_err(|e| format!("Path not found: {}", e))?;
        if !canonical_candidate.starts_with(&canonical_root) {
            return Err("Path is outside the workspace root".to_string());
        }
        canonical_candidate
    };

    let rel_path = abs
        .strip_prefix(&canonical_root)
        .unwrap_or(std::path::Path::new(""))
        .to_string_lossy()
        .replace('\\', "/");

    Ok((canonical_root, abs, rel_path))
}

/// `GET /api/agents/{agent_id}/workspaces/tree` — list directory contents
///
/// Returns a flat list of entries for a single directory level (depth=1).
/// Security: only allows browsing within the workspace root directory.
/// The `path` query parameter is relative to the workspace root.
/// The `workspace_id` parameter selects which workspace to browse:
///   - empty or `"__agent_home__"` → agent installation directory
///   - a workspace ID (e.g. `"ws-abc123"`) → that workspace's path
pub async fn list_tree(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<TreeResponse>, (StatusCode, Json<ApiError>)> {
    // Determine the workspace root based on workspace_id
    let workspace_root = {
        let gw = state.gateway_state.read().await;
        let info = gw.running_agents.get(&agent_id).ok_or_else(|| {
            ApiError::not_found("Agent not running — cannot browse workspace")
        })?;

        let ws_id = query.workspace_id.as_deref().unwrap_or("");

        if ws_id.is_empty() || ws_id == "__agent_home__" {
            // Agent home directory
            info.workspace.clone()
        } else {
            // Look up workspace path from cached config
            let config = info.workspace_config_json.as_ref()
                .and_then(|json| serde_json::from_str::<WorkspaceConfig>(json).ok());
            match config {
                Some(cfg) => {
                    cfg.additional_dirs
                        .iter()
                        .find(|d| d.id == ws_id)
                        .map(|d| d.path.clone())
                        .ok_or_else(|| ApiError::not_found(&format!(
                            "Workspace directory not found: {}",
                            ws_id
                        )))?
                }
                None => {
                    return Err(ApiError::not_found(
                        "Agent workspace config not available yet",
                    ));
                }
            }
        }
    };

    let requested_path = query.path.as_deref().unwrap_or("").to_string();
    let (canonical_root, abs_path, rel_path) =
        resolve_tree_path(&workspace_root, &requested_path)
            .map_err(|e| ApiError::bad_request(&e))?;

    // Read directory entries
    let read_dir = match std::fs::read_dir(&abs_path) {
        Ok(rd) => rd,
        Err(e) => {
            return Err(ApiError::internal(&format!(
                "Failed to read directory: {}",
                e
            )))
        }
    };

    // Strip the Windows extended-length path prefix (\\?\) that canonicalize()
    // produces on Windows. This prefix is not valid in file URIs and breaks
    // LSP document URIs (e.g. "file:////?/C:/..." instead of "file:///C:/...").
    let canonical_str = canonical_root.to_string_lossy();
    let stripped = if canonical_str.starts_with(r"\\?\") {
        &canonical_str[4..]
    } else {
        canonical_str.as_ref()
    };
    let root_str = stripped.replace('\\', "/");
    let mut dirs: Vec<TreeEntry> = Vec::new();
    let mut files: Vec<TreeEntry> = Vec::new();

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // Skip unreadable entries
        };

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/dirs (starting with '.')
        if name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().ok();
        let is_dir = metadata.as_ref().map_or(false, |m| m.is_dir());

        if is_dir {
            // Count children for the expansion indicator
            let children_count = std::fs::read_dir(entry.path())
                .ok()
                .map(|rd| {
                    rd.filter(|e| {
                        e.as_ref()
                            .map(|e| {
                                !e.file_name()
                                    .to_string_lossy()
                                    .starts_with('.')
                            })
                            .unwrap_or(false)
                    })
                    .count()
                })
                .unwrap_or(0);

            dirs.push(TreeEntry {
                name,
                entry_type: "directory".to_string(),
                size: None,
                modified: metadata.and_then(|m| {
                    m.modified()
                        .ok()
                        .and_then(|t| {
                            t.duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .ok()
                                .map(|d| {
                                    chrono::DateTime::from_timestamp(
                                        d.as_secs() as i64,
                                        0,
                                    )
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_default()
                                })
                        })
                }),
                children_count: Some(children_count),
            });
        } else {
            files.push(TreeEntry {
                name,
                entry_type: "file".to_string(),
                size: metadata.as_ref().map(|m| m.len()),
                modified: metadata.and_then(|m| {
                    m.modified()
                        .ok()
                        .and_then(|t| {
                            t.duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .ok()
                                .map(|d| {
                                    chrono::DateTime::from_timestamp(
                                        d.as_secs() as i64,
                                        0,
                                    )
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_default()
                                })
                        })
                }),
                children_count: None,
            });
        }
    }

    // Sort: directories first, then files — both alphabetical (case-insensitive)
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let mut entries = dirs;
    entries.append(&mut files);

    Ok(Json(TreeResponse {
        root: root_str,
        path: rel_path,
        entries,
    }))
}

// ─── File Content API ────────────────────────────────────────────────────

/// Maximum file size for read/write operations (5 MB)
const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// Text-based MIME types allowed for file editing
fn detect_mime(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "rs" => Some("text/x-rust"),
        "ts" | "tsx" => Some("text/typescript"),
        "js" | "jsx" => Some("text/javascript"),
        "json" => Some("application/json"),
        "toml" => Some("application/toml"),
        "yaml" | "yml" => Some("text/yaml"),
        "md" | "markdown" => Some("text/markdown"),
        "html" | "htm" => Some("text/html"),
        "css" | "scss" | "less" => Some("text/css"),
        "xml" => Some("text/xml"),
        "sh" | "bash" | "zsh" => Some("text/x-shellscript"),
        "ps1" | "psm1" | "psd1" => Some("text/x-powershell"),
        "bat" | "cmd" => Some("text/x-bat"),
        "py" => Some("text/x-python"),
        "rb" => Some("text/x-ruby"),
        "go" => Some("text/x-go"),
        "java" => Some("text/x-java"),
        "c" | "h" => Some("text/x-c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("text/x-cpp"),
        "cs" => Some("text/x-csharp"),
        "swift" => Some("text/x-swift"),
        "kt" | "kts" => Some("text/x-kotlin"),
        "sql" => Some("text/x-sql"),
        "graphql" | "gql" => Some("text/x-graphql"),
        "dockerfile" => Some("text/x-dockerfile"),
        "env" | "ini" | "cfg" | "conf" => Some("text/plain"),
        "txt" | "log" | "csv" => Some("text/plain"),
        "gitignore" | "editorconfig" => Some("text/plain"),
        _ => None,
    }
}

/// Query parameters for file read/write
#[derive(Debug, Deserialize, Default)]
pub struct FileQuery {
    /// Relative file path within the workspace
    pub path: Option<String>,
    /// Workspace ID. "__agent_home__" or empty = agent home directory
    pub workspace_id: Option<String>,
}

/// Response for file read
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResponse {
    pub content: String,
    pub size: u64,
    pub mime_type: String,
}

/// Request body for file write
#[derive(Debug, Deserialize)]
pub struct WriteFileRequest {
    pub content: String,
}

/// Response for file write
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileResponse {
    pub ok: bool,
    pub size: u64,
}

/// Resolve workspace root path for a given agent + workspace_id.
/// Shared between tree and file APIs.
async fn resolve_workspace_root(
    state: &AppState,
    agent_id: &str,
    workspace_id: Option<&str>,
) -> Result<String, (StatusCode, Json<ApiError>)> {
    let gw = state.gateway_state.read().await;
    let info = gw.running_agents.get(agent_id).ok_or_else(|| {
        ApiError::not_found("Agent not running — cannot access workspace")
    })?;

    let ws_id = workspace_id.unwrap_or("");
    if ws_id.is_empty() || ws_id == "__agent_home__" {
        Ok(info.workspace.clone())
    } else {
        let config = info
            .workspace_config_json
            .as_ref()
            .and_then(|json| serde_json::from_str::<WorkspaceConfig>(json).ok());
        match config {
            Some(cfg) => cfg
                .additional_dirs
                .iter()
                .find(|d| d.id == ws_id)
                .map(|d| d.path.clone())
                .ok_or_else(|| ApiError::not_found(&format!(
                    "Workspace directory not found: {}",
                    ws_id
                ))),
            None => Err(ApiError::not_found(
                "Agent workspace config not available yet",
            )),
        }
    }
}

/// `GET /api/agents/{agent_id}/workspaces/file` — read a file's content
pub async fn read_file(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Result<Json<FileResponse>, (StatusCode, Json<ApiError>)> {
    let file_rel_path = query.path.as_deref().unwrap_or("");
    if file_rel_path.is_empty() {
        return Err(ApiError::bad_request("Missing required 'path' parameter"));
    }

    let workspace_root =
        resolve_workspace_root(&state, &agent_id, query.workspace_id.as_deref()).await?;

    let (_canonical_root, abs_path, _rel_path) =
        resolve_tree_path(&workspace_root, file_rel_path)
            .map_err(|e| ApiError::bad_request(&e))?;

    // Verify it's a file
    if !abs_path.is_file() {
        return Err(ApiError::bad_request("Path is not a file"));
    }

    // Check file size
    let metadata = std::fs::metadata(&abs_path)
        .map_err(|e| ApiError::internal(&format!("Cannot read metadata: {}", e)))?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ApiError {
                error: format!(
                    "File too large ({} bytes, max {} bytes)",
                    metadata.len(),
                    MAX_FILE_SIZE
                ),
                code: 413,
            }),
        ));
    }

    // Detect MIME type
    let ext = abs_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let mime_type = detect_mime(ext).unwrap_or("text/plain").to_string();

    // Read content
    let content = std::fs::read_to_string(&abs_path).map_err(|e| {
        ApiError::internal(&format!("Failed to read file: {}", e))
    })?;

    Ok(Json(FileResponse {
        content,
        size: metadata.len(),
        mime_type,
    }))
}

/// `PUT /api/agents/{agent_id}/workspaces/file` — write content to a file
pub async fn write_file(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<FileQuery>,
    Json(req): Json<WriteFileRequest>,
) -> Result<Json<WriteFileResponse>, (StatusCode, Json<ApiError>)> {
    let file_rel_path = query.path.as_deref().unwrap_or("");
    if file_rel_path.is_empty() {
        return Err(ApiError::bad_request("Missing required 'path' parameter"));
    }

    // Check content size
    let content_bytes = req.content.len() as u64;
    if content_bytes > MAX_FILE_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ApiError {
                error: format!(
                    "Content too large ({} bytes, max {} bytes)",
                    content_bytes, MAX_FILE_SIZE
                ),
                code: 413,
            }),
        ));
    }

    let workspace_root =
        resolve_workspace_root(&state, &agent_id, query.workspace_id.as_deref()).await?;

    // Check workspace access level (read-only → reject writes)
    {
        let gw = state.gateway_state.read().await;
        if let Some(info) = gw.running_agents.get(&agent_id) {
            let ws_id = query.workspace_id.as_deref().unwrap_or("");
            if !ws_id.is_empty() && ws_id != "__agent_home__" {
                if let Some(config) = info
                    .workspace_config_json
                    .as_ref()
                    .and_then(|json| serde_json::from_str::<WorkspaceConfig>(json).ok())
                {
                    if let Some(dir) = config.additional_dirs.iter().find(|d| d.id == ws_id) {
                        if dir.access == AccessLevel::ReadOnly {
                            return Err(ApiError::bad_request(
                                "Workspace is read-only, cannot write files",
                            ));
                        }
                    }
                }
            }
        }
    }

    let (_canonical_root, abs_path, _rel_path) =
        resolve_tree_path(&workspace_root, file_rel_path)
            .map_err(|e| ApiError::bad_request(&e))?;

    // Verify it's a file (must exist for write)
    if !abs_path.is_file() {
        return Err(ApiError::bad_request("Path is not a file or does not exist"));
    }

    // Write content
    std::fs::write(&abs_path, &req.content).map_err(|e| {
        ApiError::internal(&format!("Failed to write file: {}", e))
    })?;

    Ok(Json(WriteFileResponse {
        ok: true,
        size: content_bytes,
    }))
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
        .route(
            "/api/agents/{agent_id}/workspaces/tree",
            get(list_tree),
        )
        .route(
            "/api/agents/{agent_id}/workspaces/file",
            get(read_file).put(write_file),
        )
}
