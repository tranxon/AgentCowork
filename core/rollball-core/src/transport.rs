//! Platform-agnostic async transport traits for IPC.
//!
//! These traits define the contract between the IPC protocol layer
//! (Frame, GatewayRequest/Response) and the platform-specific transport
//! implementations (Unix Socket, Named Pipe, Local TCP).
//!
//! Design principle: `#[cfg(unix)]` and `#[cfg(windows)]` must ONLY appear
//! inside transport implementation files, never in server.rs or client.rs.

use crate::protocol::Frame;
use crate::error::RollballError;

/// A single bidirectional connection (Gateway-side or Runtime-side).
///
/// Both Gateway (after accept) and Runtime (after connect) hold one of these.
/// The trait is dyn-compatible (`Box<dyn AsyncTransportConnection>`).
#[async_trait::async_trait]
pub trait AsyncTransportConnection: Send + Sync {
    /// Receive the next frame. Returns `Ok(None)` on clean close.
    async fn recv_frame(&mut self) -> Result<Option<Frame>, RollballError>;

    /// Send a frame.
    async fn send_frame(&mut self, frame: &Frame) -> Result<(), RollballError>;

    /// Human-readable description of the remote peer (for logging).
    fn peer_desc(&self) -> String;
}

/// Server-side listener that accepts incoming connections.
///
/// Only the Gateway creates one of these. The Runtime uses
/// `AsyncTransportConnection` directly after connecting.
#[async_trait::async_trait]
pub trait AsyncTransportServer: Send + Sync {
    /// Start listening on the configured endpoint.
    async fn listen(&mut self) -> Result<(), RollballError>;

    /// Wait for and accept the next incoming connection.
    async fn accept(&mut self) -> Result<Box<dyn AsyncTransportConnection>, RollballError>;

    /// Human-readable description of the listening endpoint (for logging).
    fn endpoint_desc(&self) -> String;
}

/// Factory: create the platform-appropriate server transport.
///
/// The endpoint format determines the transport:
/// - Path containing `/` or ending in `.sock` → Unix Socket (Linux/macOS)
/// - Path starting with `\\.\pipe\` → Named Pipe (Windows)
pub fn create_server(_endpoint: &str) -> Result<Box<dyn AsyncTransportServer>, RollballError> {
    // Delegated to platform-specific implementations in gateway/runtime crates.
    // This function is a logical placeholder; actual creation happens in
    // the crate that knows which platforms it's compiled for.
    //
    // We do NOT put #[cfg] here because rollball-core must compile on all platforms.
    // The gateway and runtime crates each provide their own `create_server()`.
    Err(RollballError::Ipc(
        "create_server() must be called from the gateway/runtime crate, not rollball-core".to_string()
    ))
}

/// Classify an endpoint string into a transport kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// Unix domain socket (Linux/macOS)
    UnixSocket,
    /// Windows Named Pipe
    NamedPipe,
    /// Local TCP (future: mobile)
    LocalTcp,
}

/// Determine the transport kind from an endpoint string.
pub fn classify_endpoint(endpoint: &str) -> TransportKind {
    if endpoint.starts_with(r"\\.\pipe\") || endpoint.starts_with("pipe://") {
        TransportKind::NamedPipe
    } else if endpoint.starts_with("tcp://") {
        TransportKind::LocalTcp
    } else {
        // Default: Unix socket (path-based)
        TransportKind::UnixSocket
    }
}

/// Get the platform-appropriate default IPC endpoint.
pub fn default_endpoint() -> String {
    if cfg!(windows) {
        r"\\.\pipe\rollball-gateway".to_string()
    } else {
        let base = directories::ProjectDirs::from("com", "rollball", "rollball-gateway")
            .map(|pd| pd.config_dir().to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from(".rollball-gateway"));
        base.join("gateway.sock").to_string_lossy().to_string()
    }
}
