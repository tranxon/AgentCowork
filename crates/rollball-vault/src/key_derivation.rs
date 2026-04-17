//! Key derivation (password → master key)

use crate::error::Result;

/// Derive master key from password using Argon2id
pub fn derive_key(password: &str, salt: &[u8]) -> Result<Vec<u8>> {
    // TODO: Implement Argon2id key derivation
    unimplemented!()
}

/// Generate random salt
pub fn generate_salt() -> Vec<u8> {
    // TODO: Generate cryptographically secure random salt
    unimplemented!()
}
