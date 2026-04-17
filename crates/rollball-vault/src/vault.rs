//! Vault main structure

use secrecy::SecretString;
use std::path::{Path, PathBuf};

use crate::error::{Result, VaultError};

/// Vault for storing API keys
pub struct Vault {
    vault_dir: PathBuf,
    master_key: Option<SecretString>,
}

impl Vault {
    /// Create or open a Vault
    pub fn open(vault_dir: &Path) -> Result<Self> {
        // TODO: Implement vault initialization
        Ok(Self {
            vault_dir: vault_dir.to_path_buf(),
            master_key: None,
        })
    }

    /// Unlock vault with password (derive master key)
    pub fn unlock(&mut self, password: &str) -> Result<()> {
        // TODO: Implement password-based key derivation
        self.master_key = Some(SecretString::new(password.to_string()));
        Ok(())
    }

    /// Store a key (encrypted)
    pub fn store(&self, key_name: &str, secret: &str) -> Result<()> {
        // TODO: Implement encrypted storage
        unimplemented!()
    }

    /// Retrieve a key (decrypted, returns SecretString)
    pub fn retrieve(&self, key_name: &str) -> Result<SecretString> {
        // TODO: Implement encrypted retrieval
        unimplemented!()
    }

    /// List all key names (does not return values)
    pub fn list(&self) -> Result<Vec<String>> {
        // TODO: Implement key listing
        unimplemented!()
    }
}
