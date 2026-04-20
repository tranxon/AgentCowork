//! Integration tests for Rollball.AI
//!
//! These tests validate cross-crate interactions.
//! Per-crate integration tests are located in each crate's tests/ directory.
//!
//! Test inventory:
//! - test_sign_and_verify_roundtrip     → rollball-sign/tests/sign_roundtrip.rs
//! - test_weather_agent_sign_and_verify → rollball-sign/tests/sign_roundtrip.rs
//! - test_vault_store_retrieve          → rollball-vault/tests/vault_roundtrip.rs
//! - test_manifest_parse                → rollball-core/tests/manifest_parse.rs
//! - test_runtime_main_loop_step        → rollball-runtime/tests/runtime_main_loop.rs
//! - test_gateway_install_package       → rollball-gateway/tests/gateway_package.rs
//! - test_gateway_lifecycle             → rollball-gateway/tests/gateway_lifecycle.rs
//! - test_e2e_weather_agent             → rollball-runtime/tests/e2e_weather_agent.rs
