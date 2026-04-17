//! Error types for rollball-vault

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Vault not unlocked")]
    NotUnlocked,

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid password")]
    InvalidPassword,
}

pub type Result<T> = std::result::Result<T, VaultError>;
