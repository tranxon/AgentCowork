//! Chat/conversation HTTP API handlers
//!
//! Implements the conversation endpoints:
//! - POST /api/agents/:id/message — send a message (fire-and-forget)
//! - GET  /api/agents/:id/stream  — WebSocket upgrade for streaming
//!
//! WebSocket message format:
//!   Client → Server:  { "type": "message", "content": "..." }
//!   Server → Client:  { "type": "chunk", "delta": "...", "message_id": "..." }
//!                     { "type": "tool_call", "name": "...", "params": {...} }
//!                     { "type": "tool_result", "name": "...", "result": {...} }
//!                     { "type": "done", "message_id": "...", "usage": {...} }

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use serde::{Deserialize, Serialize};

use crate::http::routes::{ApiError, AppState};

/// Build the chat/conversation router
pub fn chat_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agents/{id}/message", post(send_message))
        .route("/api/agents/{id}/stream", get(agent_stream_ws))
}

// ── Request/Response types ────────────────────────────────────────────

/// Request body for sending a message
#[derive(Deserialize)]
pub struct SendMessageRequest {
    /// The message content
    pub content: String,
    /// Optional conversation ID for multi-turn
    #[serde(default)]
    pub conversation_id: Option<String>,
}

/// Response for send message
#[derive(Serialize)]
pub struct SendMessageResponse {
    /// Unique message ID for correlation
    pub message_id: String,
    /// Delivery status
    pub status: String,
}

/// WebSocket client message (inbound from Desktop App)
#[derive(Deserialize)]
struct WsClientMessage {
    #[serde(rename = "type")]
    msg_type: String,
    content: Option<String>,
}

// ── Handlers ──────────────────────────────────────────────────────────

/// `POST /api/agents/:id/message` — send a message to an agent
///
/// Validates the agent exists and is running, then pushes the message
/// to the agent's IPC session via the SessionManager.
/// Returns a message_id for correlation.
pub async fn send_message(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<SendMessageResponse>), (StatusCode, Json<ApiError>)> {
    // Validate agent exists and is running
    {
        let gw = state.gateway_state.read().await;
        if !gw.is_installed(&agent_id) {
            return Err(ApiError::not_found(&format!("Agent not found: {}", agent_id)));
        }
        if !gw.is_running(&agent_id) {
            return Err(ApiError::bad_request(&format!(
                "Agent {} is not running",
                agent_id
            )));
        }
    }

    // Generate message ID
    let message_id = format!("msg-{}", uuid::Uuid::new_v4());

    // Push message to agent via SessionManager (if available)
    // S1.6 will implement the full response bridge
    if let Some(session_mgr) = &state.session_mgr {
        let mgr = session_mgr.lock().await;
        if let Some((conn_id, session)) = mgr.find_by_agent_id(&agent_id) {
            let intent = rollball_core::protocol::GatewayResponse::IntentReceived {
                from: "http-api".to_string(),
                action: "chat_message".to_string(),
                params: serde_json::json!({
                    "content": body.content,
                    "message_id": message_id,
                    "conversation_id": body.conversation_id,
                }),
            };
            let pushed = session.push_message(intent).await;
            if !pushed {
                tracing::warn!(
                    "Failed to push message to agent {} via conn {}",
                    agent_id,
                    conn_id
                );
            }
        } else {
            tracing::warn!("Agent {} is running but has no IPC session", agent_id);
        }
    }

    Ok((
        StatusCode::OK,
        Json(SendMessageResponse {
            message_id,
            status: "sent".to_string(),
        }),
    ))
}

/// `GET /api/agents/:id/stream` — WebSocket upgrade for streaming chat
///
/// Upgrades the HTTP connection to a WebSocket for bidirectional
/// streaming communication with an agent.
pub async fn agent_stream_ws(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // Validate agent exists
    {
        let gw = state.gateway_state.read().await;
        if !gw.is_installed(&agent_id) {
            return Err(ApiError::not_found(&format!("Agent not found: {}", agent_id)));
        }
    }

    // Upgrade to WebSocket
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, agent_id, state)))
}

/// Handle the WebSocket connection lifecycle
///
/// Receives messages from the client, pushes them to the Agent's IPC session,
/// and streams responses back. Full IPC bridge (S1.6) will forward Agent
/// responses here.
async fn handle_ws(mut socket: WebSocket, agent_id: String, state: AppState) {
    tracing::info!("WebSocket connected for agent: {}", agent_id);

    // Send initial connection acknowledgment
    let welcome = serde_json::json!({
        "type": "connected",
        "agent_id": agent_id,
    });
    let _ = socket.send(Message::Text(welcome.to_string().into())).await;

    loop {
        let msg = socket.recv().await;
        match msg {
            Some(Ok(Message::Text(text))) => {
                handle_ws_text(&mut socket, &agent_id, &state, &text).await;
            }
            Some(Ok(Message::Close(_))) | None => {
                tracing::info!("WebSocket closed for agent: {}", agent_id);
                break;
            }
            Some(Ok(Message::Ping(data))) => {
                let _ = socket.send(Message::Pong(data)).await;
            }
            _ => {
                // Ignore binary, pong, etc.
            }
        }
    }
}

/// Handle a single text message from the WebSocket client
async fn handle_ws_text(
    socket: &mut WebSocket,
    agent_id: &str,
    state: &AppState,
    text: &str,
) {
    // Parse client message
    let client_msg: WsClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            let err = serde_json::json!({
                "type": "error",
                "message": format!("Invalid message format: {}", e),
            });
            let _ = socket.send(Message::Text(err.to_string().into())).await;
            return;
        }
    };

    if client_msg.msg_type != "message" {
        let err = serde_json::json!({
            "type": "error",
            "message": format!("Unknown message type: {}", client_msg.msg_type),
        });
        let _ = socket.send(Message::Text(err.to_string().into())).await;
        return;
    }

    let content = client_msg.content.unwrap_or_default();
    let message_id = format!("msg-{}", uuid::Uuid::new_v4());

    // Push to agent via SessionManager
    let mut pushed_ok = false;
    if let Some(session_mgr) = &state.session_mgr {
        let mgr = session_mgr.lock().await;
        if let Some((_, session)) = mgr.find_by_agent_id(agent_id) {
            let intent = rollball_core::protocol::GatewayResponse::IntentReceived {
                from: "http-ws".to_string(),
                action: "chat_message".to_string(),
                params: serde_json::json!({
                    "content": content,
                    "message_id": message_id,
                }),
            };
            pushed_ok = session.push_message(intent).await;
        }
    }

    if !pushed_ok {
        let err = serde_json::json!({
            "type": "error",
            "message": format!("Agent {} is not connected via IPC", agent_id),
            "message_id": message_id,
        });
        let _ = socket.send(Message::Text(err.to_string().into())).await;
        return;
    }

    // Acknowledge message received
    let ack = serde_json::json!({
        "type": "ack",
        "message_id": message_id,
    });
    let _ = socket.send(Message::Text(ack.to_string().into())).await;

    // S1.6: Full streaming bridge will forward Agent responses as
    // chunk/tool_call/tool_result messages here.
    // For S1.3, send a placeholder "done" since the Agent response
    // cannot yet be streamed back through this WebSocket.
    let done = serde_json::json!({
        "type": "done",
        "message_id": message_id,
        "usage": null,
    });
    let _ = socket.send(Message::Text(done.to_string().into())).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_message_request_deserialization() {
        let json = r#"{"content": "Hello, agent!"}"#;
        let req: SendMessageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.content, "Hello, agent!");
        assert!(req.conversation_id.is_none());
    }

    #[test]
    fn test_send_message_request_with_conversation_id() {
        let json = r#"{"content": "Hello!", "conversation_id": "conv-123"}"#;
        let req: SendMessageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.content, "Hello!");
        assert_eq!(req.conversation_id, Some("conv-123".to_string()));
    }

    #[test]
    fn test_send_message_response_serialization() {
        let resp = SendMessageResponse {
            message_id: "msg-abc".to_string(),
            status: "sent".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("msg-abc"));
        assert!(json.contains("sent"));
    }

    #[test]
    fn test_ws_client_message_deserialization() {
        let json = r#"{"type": "message", "content": "Hi there"}"#;
        let msg: WsClientMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_type, "message");
        assert_eq!(msg.content, Some("Hi there".to_string()));
    }
}
