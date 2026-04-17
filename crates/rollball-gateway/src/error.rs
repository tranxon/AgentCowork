//! Error types for rollball-gateway

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Core error: {0}")]
    Core(#[from] rollball_core::RollballError),

    #[error("Sign error: {0}")]
    Sign(String),

    #[error("Vault error: {0}")]
    Vault(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Lifecycle error: {0}")]
    Lifecycle(String),
}

pub type Result<T> = std::result::Result<T, GatewayError>;
