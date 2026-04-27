//! Chat commands

use tauri::State;

use crate::gateway_client::SendMessageResponse;
use crate::state::AppState;

/// Send a message to an agent (HTTP POST, non-streaming)
#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    agent_id: String,
    content: String,
) -> Result<SendMessageResponse, String> {
    let client = state.gateway.read().await;
    client.send_message(&agent_id, &content).await.map_err(|e| e.to_string())
}
