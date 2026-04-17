//! Gateway Service API server (Unix Socket)

use tokio::net::UnixListener;

/// IPC server
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    /// Create new IPC server
    pub fn new(socket_path: &str) -> Result<Self, String> {
        // TODO: Create Unix socket listener
        unimplemented!()
    }

    /// Start accepting connections
    pub async fn run(&self) -> Result<(), String> {
        // TODO: Implement connection handling loop
        unimplemented!()
    }
}
