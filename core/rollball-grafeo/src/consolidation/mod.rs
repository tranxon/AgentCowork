//! Consolidation pipeline — instant extraction and offline consolidation.
//!
//! The consolidation pipeline processes knowledge extracted by the LLM
//! during conversation (instant) and in background batches (offline).
//!
//! - **Instant extraction** (`instant`): processes `memory_store` tool calls
//!   from the LLM in real-time, performing dedup, conflict detection, and
//!   status assignment (Active/Pending).
//! - **Offline consolidation** (`offline`): upgrades Pending nodes to Active
//!   based on age and evidence. Full LLM-based re-evaluation is planned for
//!   Phase 3.

pub mod ambiguous;
pub mod instant;
pub mod offline;

pub use ambiguous::AmbiguousConflict;
pub use instant::{ConflictCandidate, MemoryStoreInput, ConflictResolutionDetail, ProcessResult};
pub use offline::{OfflineConsolidationConfig, OfflineConsolidationResult};
