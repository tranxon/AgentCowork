//! Semantic (consolidated) memory layer.
//!
//! Provides storage and retrieval for long-term memory nodes:
//! - [`KnowledgeNode`] — facts, preferences, relations
//! - [`ProceduralNode`] — behaviour patterns
//! - [`AutobiographicalNode`] — self-knowledge (forced Active)
//! - Graph operations — edges, weights, neighbour traversal

pub mod autobiographical;
pub mod graph;
pub mod knowledge;
pub mod procedural;
