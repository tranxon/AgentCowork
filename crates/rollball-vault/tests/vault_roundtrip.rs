//! Vault store + retrieve roundtrip integration test

use secrecy::ExposeSecret;
use std::fs;

#[test]
fn test_vault_store_retrieve_roundtrip() {
    let temp_dir = std::env::temp_dir().join("rollball-test-vault-roundtrip");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let vault_dir = temp_dir.join("vault");
    let mut vault = rollball_vault::Vault::open(&vault_dir).expect("Failed to open vault");
    vault.unlock("test-password-123").expect("Failed to unlock vault");
    assert!(vault.is_unlocked(), "Vault should be unlocked");

    vault.store("openai", "sk-proj-test-key-1234567890").expect("Failed to store key");

    let secret = vault.retrieve("openai").expect("Failed to retrieve key");
    assert_eq!(secret.expose_secret(), "sk-proj-test-key-1234567890");

    let keys = vault.list().expect("Failed to list keys");
    assert!(keys.contains(&"openai".to_string()));

    vault.delete("openai").expect("Failed to delete key");
    assert!(!vault.exists("openai"), "Key should no longer exist");
}

#[test]
fn test_vault_wrong_password_fails() {
    let temp_dir = std::env::temp_dir().join("rollball-test-vault-wrong-pw");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let vault_dir = temp_dir.join("vault");
    let mut vault = rollball_vault::Vault::open(&vault_dir).expect("Failed to open vault");
    vault.unlock("password-A").expect("Failed to unlock vault");
    vault.store("test_key", "secret-value").expect("Failed to store key");

    vault.lock();
    vault.unlock("password-B").expect("Unlock should succeed");

    let result = vault.retrieve("test_key");
    assert!(result.is_err(), "Retrieval with wrong password should fail");
}

#[test]
fn test_vault_locked_operations_fail() {
    let temp_dir = std::env::temp_dir().join("rollball-test-vault-locked");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let vault_dir = temp_dir.join("vault");
    let vault = rollball_vault::Vault::open(&vault_dir).expect("Failed to open vault");
    assert!(!vault.is_unlocked(), "Vault should be locked");

    assert!(vault.store("test", "value").is_err());
    assert!(vault.retrieve("test").is_err());
}
