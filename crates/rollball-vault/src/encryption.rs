//! ChaCha20-Poly1305 AEAD encryption layer

use crate::error::Result;

/// Encrypt data with ChaCha20-Poly1305
/// Format: nonce (12B) + ciphertext + tag (16B)
pub fn encrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    // TODO: Implement encryption
    unimplemented!()
}

/// Decrypt data with ChaCha20-Poly1305
pub fn decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    // TODO: Implement decryption
    unimplemented!()
}
