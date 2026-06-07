//! LSP relay module
//!
//! LSP protocol relay: WebSocket ↔ stdin/stdout of a language server process.
//!
//! LSP over stdio uses the Base Protocol frame format:
//! ```text
//! Content-Length: <length>\r\n\r\n<JSON-RPC message>
//! ```
//! WebSocket side (vscode-ws-jsonrpc) sends/receives plain JSON-RPC text messages.
//!
//! The relay converts between these two formats:
//! - **WS → stdin**: receive JSON text, prepend `Content-Length` header, write to stdin
//! - **stdout → WS**: parse `Content-Length` header, extract JSON body, send as text
//!
//! Architecture:
//! ```text
//! Monaco (webview) ← WS (JSON text) → Gateway ← stdin/stdout (framed) → LSP Server
//! ```

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use crate::http::routes::AppState;

// ── LSP server command lookup ──────────────────────────────────────────

/// Resolve the LSP server command for a given language.
///
/// Checks `PATH` first, returns `None` if the command is not found or
/// the language is not supported.
fn resolve_lsp_command(language: &str) -> Option<String> {
    // Priority 1: hard-coded known commands (checked against PATH)
    let candidates: &[&str] = match language.to_lowercase().as_str() {
        "rust" => &["rust-analyzer"],
        "python" => &["pylsp", "pyright-langserver", "python-lsp-server"],
        "typescript" | "javascript" | "ts" | "js" => {
            &["typescript-language-server", "typescript-language-server.cmd"]
        }
        "go" => &["gopls"],
        "c" | "cpp" | "c++" => &["clangd"],
        "json" => &["vscode-json-languageserver", "json-languageserver"],
        "yaml" | "yml" => &["yaml-language-server"],
        "html" => &["vscode-html-languageserver", "html-languageserver"],
        "css" | "scss" | "less" => &["vscode-css-languageserver", "css-languageserver"],
        "markdown" | "md" => &["marksman"],
        _ => return None,
    };

    // Find first candidate that exists on PATH
    for cmd in candidates {
        if find_on_path(cmd) {
            tracing::info!("[LSP] Found LSP command for '{language}': {cmd}");
            return Some(cmd.to_string());
        }
    }

    tracing::warn!("[LSP] No LSP command found for '{language}' (checked: {:?})", candidates);
    None
}

/// Check if a command exists on the system PATH
fn find_on_path(cmd: &str) -> bool {
    // On Windows, also try with .exe / .cmd / .bat extensions
    let candidates: Vec<String> = if cfg!(windows) {
        vec![
            format!("{}.exe", cmd),
            format!("{}.cmd", cmd),
            format!("{}.bat", cmd),
            cmd.to_string(),
        ]
    } else {
        vec![cmd.to_string()]
    };

    // Get PATH from environment
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        for name in &candidates {
            let full = dir.join(name);
            if full.is_file() {
                return true;
            }
        }
    }
    false
}

// ── Query parameters ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LspQuery {
    /// Agent ID to resolve workspace directory
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Optional workspace ID for additional workspace directories
    #[serde(default)]
    pub workspace_id: Option<String>,
}

// ── WebSocket handler ──────────────────────────────────────────────────

/// `GET /lsp/{language}` — WebSocket upgrade for LSP relay
///
/// Opens a WebSocket connection, spawns the appropriate language server,
/// and relays bytes bidirectionally.
pub async fn lsp_handler(
    ws: WebSocketUpgrade,
    Path(language): Path<String>,
    Query(query): Query<LspQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let lang_lower = language.to_lowercase();

    // Resolve workspace root
    let workspace_root = match resolve_workspace_root_for_lsp(&state, &query).await {
        Ok(root) => root,
        Err((status, msg)) => {
            let err_json = serde_json::json!({ "error": msg, "code": status.as_u16() });
            return (status, axum::Json(err_json)).into_response();
        }
    };

    // Resolve LSP command
    let lsp_cmd = match resolve_lsp_command(&lang_lower) {
        Some(cmd) => cmd,
        None => {
            let msg = format!(
                "No LSP server found for language: {}. \
                 Install one (e.g., 'rust-analyzer', 'pylsp', 'gopls') and ensure it is on PATH.",
                language
            );
            let err_json = serde_json::json!({ "error": msg, "code": 400u16 });
            return (StatusCode::BAD_REQUEST, axum::Json(err_json)).into_response();
        }
    };

    tracing::info!(
        "[LSP] Upgrading WebSocket for language='{}', cmd='{}', workspace='{}'",
        lang_lower, lsp_cmd, workspace_root
    );

    ws.on_upgrade(move |socket| lsp_relay(socket, lsp_cmd, workspace_root))
}

/// Bidirectional LSP relay: WebSocket ↔ LSP process stdin/stdout
///
/// Converts between LSP Base Protocol frames (stdin/stdout) and
/// plain JSON-RPC text messages (WebSocket).
async fn lsp_relay(socket: WebSocket, lsp_cmd: String, workspace_root: String) {
    // Spawn the LSP server process
    let mut child = match spawn_lsp(&lsp_cmd, &workspace_root).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[LSP] Failed to spawn '{}': {}", lsp_cmd, e);
            return;
        }
    };

    // Split the child process stdin/stdout
    let mut child_stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            tracing::error!("[LSP] Failed to take stdin from child process");
            let _ = child.kill().await;
            return;
        }
    };
    let child_stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            tracing::error!("[LSP] Failed to take stdout from child process");
            let _ = child.kill().await;
            return;
        }
    };

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Task 1: WebSocket (plain JSON) → LSP stdin (Content-Length framed)
    let stdin_cmd = lsp_cmd.clone();
    let ws_to_stdin = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            let text: String = match msg {
                Ok(Message::Text(t)) => t.to_string(),
                Ok(Message::Binary(data)) => {
                    // Fallback: treat binary as UTF-8 text
                    match String::from_utf8(data.to_vec()) {
                        Ok(s) => s,
                        Err(_) => continue,
                    }
                }
                Ok(Message::Close(_)) => break,
                _ => continue,
            };

            // Frame the JSON-RPC message with Content-Length header
            let header = format!("Content-Length: {}\r\n\r\n", text.len());
            if let Err(e) = child_stdin.write_all(header.as_bytes()).await {
                tracing::warn!("[LSP] Write header to '{}' stdin failed: {}", stdin_cmd, e);
                break;
            }
            if let Err(e) = child_stdin.write_all(text.as_bytes()).await {
                tracing::warn!("[LSP] Write body to '{}' stdin failed: {}", stdin_cmd, e);
                break;
            }
            let _ = child_stdin.flush().await;
        }
        // Close stdin to signal EOF to the LSP process
        let _ = child_stdin.shutdown().await;
    });

    // Task 2: LSP stdout (Content-Length framed) → WebSocket (plain JSON)
    let stdout_cmd = lsp_cmd.clone();
    let stdout_to_ws = tokio::spawn(async move {
        let mut reader = BufReader::new(child_stdout);
        loop {
            // 1. Read Content-Length header line
            let mut header_line = String::new();
            match reader.read_line(&mut header_line).await {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("[LSP] Read header from '{}' stdout failed: {}", stdout_cmd, e);
                    break;
                }
            }

            // Parse "Content-Length: N"
            let content_length = match parse_content_length(&header_line) {
                Some(n) => n,
                None => {
                    // Skip unknown header lines (e.g., Content-Type)
                    continue;
                }
            };

            // 2. Read remaining header lines until empty line (\r\n)
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        if line == "\r\n" || line == "\n" {
                            break;
                        }
                        // Skip other headers
                    }
                    Err(_) => break,
                };
            }

            // 3. Read the body (exactly content_length bytes)
            let mut body = vec![0u8; content_length];
            if let Err(e) = reader.read_exact(&mut body).await {
                tracing::warn!(
                    "[LSP] Read body ({} bytes) from '{}' stdout failed: {}",
                    content_length, stdout_cmd, e
                );
                break;
            }

            // 4. Send as WebSocket text message
            let text = match String::from_utf8(body) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("[LSP] Non-UTF8 body from '{}': {}", stdout_cmd, e);
                    break;
                }
            };

            if let Err(e) = ws_tx.send(Message::Text(text.into())).await {
                tracing::warn!("[LSP] WebSocket send failed for '{}': {}", stdout_cmd, e);
                break;
            }
        }
        // Send close frame
        let _ = ws_tx.send(Message::Close(None)).await;
    });

    // Wait for both tasks to complete
    tokio::select! {
        _ = ws_to_stdin => {}
        _ = stdout_to_ws => {}
    }

    // Clean up the child process
    if let Err(e) = child.kill().await {
        tracing::warn!("[LSP] Failed to kill '{}' process: {}", lsp_cmd, e);
    } else {
        tracing::info!("[LSP] Process '{}' terminated", lsp_cmd);
    }
}

/// Parse `Content-Length: N` from a header line.
pub fn parse_content_length(line: &str) -> Option<usize> {
    let line = line.trim();
    let prefix = "Content-Length:";
    if let Some(rest) = line.strip_prefix(prefix) {
        rest.trim().parse().ok()
    } else if let Some(rest) = line.strip_prefix("Content-length:") {
        // Some LSP servers use lowercase 'l'
        rest.trim().parse().ok()
    } else {
        None
    }
}

/// Spawn the LSP server process with the workspace as its working directory
async fn spawn_lsp(cmd: &str, workspace_root: &str) -> anyhow::Result<Child> {
    let child = Command::new(cmd)
        .current_dir(workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // Let stderr flow to Gateway's stderr for debugging
        .kill_on_drop(true)
        .spawn()?;

    tracing::info!(
        "[LSP] Spawned '{}' (PID {}) in workspace '{}'",
        cmd,
        child.id().unwrap_or(0),
        workspace_root
    );

    Ok(child)
}

// ── Workspace root resolution ─────────────────────────────────────────

/// Resolve workspace root directory for LSP process.
///
/// If `agent_id` is provided, look up the agent's workspace from running agents.
/// Otherwise, the LSP process runs in the current directory (fallback).
async fn resolve_workspace_root_for_lsp(
    state: &AppState,
    query: &LspQuery,
) -> Result<String, (StatusCode, String)> {
    // If no agent_id, use current directory as fallback
    let Some(agent_id) = &query.agent_id else {
        return Ok(".".to_string());
    };

    let gw = state.gateway_state.read().await;
    let info = gw.running_agents.get(agent_id).ok_or_else(|| {
        (StatusCode::NOT_FOUND, "Agent not running — cannot resolve workspace".to_string())
    })?;

    let ws_id = query.workspace_id.as_deref().unwrap_or("");
    if ws_id.is_empty() || ws_id == "__agent_home__" {
        Ok(info.workspace.clone())
    } else {
        // Look up in workspace_config_json
        if let Some(json) = &info.workspace_config_json {
            #[derive(Deserialize)]
            struct AdditionalDir {
                id: String,
                path: String,
            }
            #[derive(Deserialize)]
            struct WsConfig {
                #[serde(default)]
                additional_dirs: Vec<AdditionalDir>,
            }

            if let Ok(cfg) = serde_json::from_str::<WsConfig>(json) {
                for dir in &cfg.additional_dirs {
                    if dir.id == ws_id {
                        return Ok(dir.path.clone());
                    }
                }
            }
        }

        Err((StatusCode::NOT_FOUND, format!("Workspace directory not found: {}", ws_id)))
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_length_standard() {
        assert_eq!(parse_content_length("Content-Length: 42"), Some(42));
        assert_eq!(parse_content_length("Content-Length:0"), Some(0));
        assert_eq!(parse_content_length("Content-Length: 1234\r\n"), Some(1234));
    }

    #[test]
    fn test_parse_content_length_lowercase() {
        // Some LSP servers use lowercase 'l'
        assert_eq!(parse_content_length("Content-length: 99"), Some(99));
    }

    #[test]
    fn test_parse_content_length_invalid() {
        assert_eq!(parse_content_length("Content-Type: application/json"), None);
        assert_eq!(parse_content_length("X-Custom: 42"), None);
        assert_eq!(parse_content_length(""), None);
    }

    #[test]
    fn test_parse_content_length_not_a_number() {
        assert_eq!(parse_content_length("Content-Length: abc"), None);
    }

    #[test]
    fn test_resolve_lsp_command_known_languages() {
        let _ = resolve_lsp_command("rust");
        let _ = resolve_lsp_command("python");
        let _ = resolve_lsp_command("go");
    }

    #[test]
    fn test_resolve_lsp_command_unknown_language() {
        assert_eq!(resolve_lsp_command("brainfuck"), None);
        assert_eq!(resolve_lsp_command(""), None);
    }

    #[test]
    fn test_resolve_lsp_command_case_insensitive() {
        // Both "Rust" and "rust" should resolve to the same command
        let lower = resolve_lsp_command("rust");
        let upper = resolve_lsp_command("Rust");
        assert_eq!(lower, upper);
    }

    #[test]
    fn test_find_on_path_known_command() {
        // On Windows, `cmd` should always be on PATH
        #[cfg(windows)]
        assert!(find_on_path("cmd"));
        // On Unix, `ls` should always be on PATH
        #[cfg(not(windows))]
        assert!(find_on_path("ls"));
    }

    #[test]
    fn test_find_on_path_nonexistent() {
        assert!(!find_on_path("this_command_definitely_does_not_exist_12345"));
    }
}
