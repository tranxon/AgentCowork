//! Memory types

use serde::{Deserialize, Serialize};

/// Memory zone categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryZone {
    /// Working memory (transient)
    Working,
    /// Episodic memory (experiences)
    Episodic,
    /// Semantic memory (facts)
    Semantic,
    /// Procedural memory (skills)
    Procedural,
    /// Autobiographical memory (self-knowledge)
    Autobiographical,
    /// Work-related memory
    Work,
}

pub use rollball_core::memory::traits::{MemoryNode, PrivacyLevel};
