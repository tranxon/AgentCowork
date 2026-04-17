//! rollball-memory — MemoryStore trait and shared memory types
//!
//! This crate defines the MemoryStore trait abstraction.
//! Grafeo (rollball-grafeo) is the primary implementation (Phase 2).

pub mod store;
pub mod types;

pub use store::MemoryStore;
pub use types::{MemoryNode, PrivacyLevel, MemoryZone};
