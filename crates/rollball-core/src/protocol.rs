//! Gateway Service API message definitions (contract layer, transport-agnostic)

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::budget::UsageReport;

/// Gateway Service API request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GatewayRequest {
    KeyRelease { provider: String },
    IntentSend {
        target: String,
        action: String,
        params: Value,
        #[serde(rename = "async")]
        async_: bool,
    },
    BudgetQuery { provider: String },
    UsageReport(UsageReport),
    RateAcquire { provider: String },
    PermissionRequest { permission: String, reason: String },
}

/// Gateway Service API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GatewayResponse {
    KeyReleaseResult { api_key: String },
    IntentDelivered { message_id: String },
    IntentReceived {
        from: String,
        action: String,
        params: Value,
    },
    BudgetInfo {
        remaining_tokens: u64,
        remaining_cost_usd: f64,
    },
    UsageReportAck {},
    RateToken {
        granted: bool,
        retry_after_ms: Option<u64>,
    },
    PermissionResult {
        granted: bool,
        reason: Option<String>,
    },
}

/// Transport layer frame format
pub struct Frame {
    pub body_len: u32,    // 4 bytes big-endian
    pub msg_type: u8,     // 0=request, 1=response, 2=stream_chunk, 3=error
    pub body: Vec<u8>,    // JSON payload
}

impl Frame {
    /// Message type constants
    pub const TYPE_REQUEST: u8 = 0;
    pub const TYPE_RESPONSE: u8 = 1;
    pub const TYPE_STREAM_CHUNK: u8 = 2;
    pub const TYPE_ERROR: u8 = 3;

    /// Create a new Frame from JSON-serializable data
    pub fn from_message<T: Serialize>(msg_type: u8, msg: &T) -> Result<Self, serde_json::Error> {
        let body = serde_json::to_vec(msg)?;
        let body_len = body.len() as u32;
        Ok(Self {
            body_len,
            msg_type,
            body,
        })
    }

    /// Decode Frame body into typed message
    pub fn to_message<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }
}
