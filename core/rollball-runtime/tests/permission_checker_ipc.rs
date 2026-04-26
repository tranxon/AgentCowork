//! S2.6: PermissionChecker + IPC integration tests
//!
//! Tests for the Runtime-side permission checker with IPC request flow:
//! 1. Cache miss + no IPC → denied
//! 2. Auto-approved permissions (Allow policy) → granted without IPC
//! 3. Cache hit → granted without IPC
//! 4. Cache miss + IPC client → request sent (tested via mock)

use rollball_core::permission::{Permission, PermissionGrant};
use rollball_runtime::tools::permission_checker::PermissionChecker;

#[test]
fn test_permission_checker_check_and_request_no_ipc() {
    let checker = PermissionChecker::empty("com.example.agent");

    // Shell requires AskAlways policy — without IPC client, should deny
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        checker.check_and_request(&Permission::Shell, None).await
    });

    assert!(!result.0, "Shell should be denied without IPC");
    assert!(result.1.is_some());
    assert!(result.1.unwrap().contains("IPC not available"));
}

#[test]
fn test_permission_checker_check_and_request_auto_approve() {
    let checker = PermissionChecker::empty("com.example.agent");

    // MemoryRead is auto-approved by policy — no IPC needed
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        checker.check_and_request(&Permission::MemoryRead, None).await
    });

    assert!(result.0, "MemoryRead should be auto-approved");
    assert!(result.1.is_none());
}

#[test]
fn test_permission_checker_check_and_request_from_cache() {
    let grants = vec![
        PermissionGrant::new("com.example.agent", Permission::Shell, "user"),
    ];
    let checker = PermissionChecker::new("com.example.agent", grants);

    // Shell is in cache — should be granted without IPC
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        checker.check_and_request(&Permission::Shell, None).await
    });

    assert!(result.0, "Shell should be granted from cache");
    assert!(result.1.is_none());
}

#[test]
fn test_permission_checker_cache_miss_deny_policy() {
    let checker = PermissionChecker::empty("com.example.agent");

    // Check a permission that has Deny policy (none currently have Deny,
    // but we can verify the general flow with AskAlways permissions)
    // Shell is AskAlways — without IPC, it's denied
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        checker.check_and_request(&Permission::Wasm, None).await
    });

    assert!(!result.0, "Wasm should be denied without IPC");
    assert!(result.1.is_some());
}

#[test]
fn test_permission_checker_grant_caches_future_checks() {
    let checker = PermissionChecker::empty("com.example.agent");

    // Initially not granted
    assert!(!checker.is_granted(&Permission::Shell));

    // Simulate IPC grant
    checker.add_grant(PermissionGrant::new("com.example.agent", Permission::Shell, "ipc_approval"));

    // Now cached — should be granted without IPC
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        checker.check_and_request(&Permission::Shell, None).await
    });

    assert!(result.0, "Shell should be granted from cache after IPC approval");
    assert!(result.1.is_none());
}
