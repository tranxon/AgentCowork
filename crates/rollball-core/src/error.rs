//! Unified error types for Rollball.AI

use thiserror::Error;

/// Main error type for Rollball
#[derive(Debug, Error)]
pub enum RollballError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Manifest error: {0}")]
    Manifest(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Loop detected: {0}")]
    LoopDetected(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Package error: {0}")]
    Package(String),

    #[error("Vault error: {0}")]
    Vault(String),

    #[error("Signing error: {0}")]
    Signing(String),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, RollballError>;
