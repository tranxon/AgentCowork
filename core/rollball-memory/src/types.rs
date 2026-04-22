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

/// Query parameters for memory retrieval.
#[derive(Debug, Clone)]
pub struct MemoryQuery {
    /// The query text for keyword/semantic search.
    pub query_text: String,
    /// Optional embedding vector for semantic search.
    pub embedding: Option<Vec<f32>>,
    /// Maximum number of results to return.
    pub k: usize,
    /// Abstention threshold [0.0, 1.0]. Results below this score are filtered.
    /// When all results are filtered, abstention is triggered.
    pub min_score: Option<f32>,
    /// Whether abstention mode is enabled.
    pub abstention_enabled: bool,
    /// Memory hint type driving retrieval weights: s(semantic), f(fact), r(relational), i(identity).
    pub hint_type: Option<String>,
}

/// Metrics collected after a retrieval operation.
#[derive(Debug, Clone, Default)]
pub struct RetrievalMetrics {
    /// Number of results returned (after filtering).
    pub result_count: usize,
    /// Average relevance score of returned results.
    pub avg_score: f32,
    /// Maximum relevance score among returned results.
    pub max_score: f32,
    /// Whether abstention was triggered (all results below min_score).
    pub abstention_triggered: bool,
    /// Number of results filtered by min_score.
    pub filtered_count: usize,
}

/// Type of conflict detected between memory nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictType {
    /// Knowledge evolved over time (e.g., moved to a new city).
    Evolution,
    /// Previous knowledge was incorrect and corrected.
    Correction,
    /// Conflict is unclear, needs user confirmation.
    Ambiguous,
}

/// Multi-signal conflict detection result.
#[derive(Debug, Clone)]
pub struct ConflictSignal {
    /// Embedding cosine similarity between conflicting nodes.
    pub semantic_score: f32,
    /// Whether a temporal conflict was detected (same subject within time window).
    pub temporal_conflict: bool,
    /// Whether negation words were found in the source episode.
    pub context_negation: bool,
    /// Suggested conflict type based on heuristic rules.
    pub suggested_type: ConflictType,
    /// Confidence of the heuristic suggestion [0.0, 1.0].
    pub heuristic_confidence: f32,
}
