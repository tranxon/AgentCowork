//! Error types for rollball-runtime

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Core error: {0}")]
    Core(#[from] rollball_core::RollballError),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("IPC error: {0}")]
    Ipc(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
