//! Gateway Service API client

use rollball_core::protocol::{GatewayRequest, GatewayResponse};
use crate::ipc::transport::Transport;

/// IPC client for Gateway communication
pub struct GatewayClient {
    transport: Box<dyn Transport>,
}

impl GatewayClient {
    /// Create new client
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self { transport }
    }

    /// Send request to Gateway
    pub async fn send_request(
        &self,
        _request: GatewayRequest,
    ) -> Result<GatewayResponse, String> {
        // TODO: Implement request/response
        unimplemented!()
    }
}
