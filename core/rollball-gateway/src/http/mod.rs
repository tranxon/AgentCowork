//! HTTP API module
//!
//! Provides REST + WebSocket API for Desktop App and CLI access.
//! Shares `Arc<RwLock<GatewayState>>` with the IPC server.

pub mod server;
pub mod routes;
pub mod auth;
pub mod agents;
pub mod chat;
pub mod vault_api;
pub mod config_api;
pub mod permission_api;
