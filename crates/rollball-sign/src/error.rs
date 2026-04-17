//! Error types for rollball-sign

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Ed25519 error: {0}")]
    Ed25519(String),

    #[error("Certificate error: {0}")]
    Certificate(String),

    #[error("Invalid package: {0}")]
    InvalidPackage(String),

    #[error("Signature verification failed: {0}")]
    VerificationFailed(String),
}

pub type Result<T> = std::result::Result<T, SignError>;
