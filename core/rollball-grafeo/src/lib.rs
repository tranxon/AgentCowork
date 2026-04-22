//! rollball-grafeo — Grafeo graph database engine
//!
//! Phase 2: Full graph database implementation with:
//! - Three-layer five-type biomimetic memory
//! - Forgetting mechanism (decay)
//! - Associative diffusion retrieval
//! - Privacy level filtering

pub mod conflict;
pub mod decay;
pub mod error;
pub mod grafeo;
pub mod graph;
pub mod retrieval;

pub use conflict::{detect_conflict, FACT_THRESHOLD, NEGATION_KEYWORDS, PREFERENCE_THRESHOLD, RELATION_THRESHOLD, TEMPORAL_WINDOW_HOURS};
pub use rollball_memory::{ConflictSignal, ConflictType};
pub use decay::DecayConfig;
pub use error::{GrafeoError, Result};
pub use grafeo::GrafeoStore;
