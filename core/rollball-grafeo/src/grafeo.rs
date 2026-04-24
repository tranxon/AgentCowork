//! GrafeoStore — GrafeoDB-backed memory storage engine.

use std::path::Path;

use grafeo_engine::GrafeoDB;

use crate::error::Result;
use crate::index_config::{HnswConfig, EPISODIC_TEXT_FIELDS, KNOWLEDGE_TEXT_FIELDS, VECTOR_METRIC};
use crate::types::labels;

/// Grafeo graph database backed by grafeo-engine.
pub struct GrafeoStore {
    /// Underlying GrafeoDB engine instance.
    pub(crate) db: GrafeoDB,
    /// HNSW index configuration used for this store.
    hnsw_config: HnswConfig,
}

impl GrafeoStore {
    /// Open or create a persistent Grafeo database at the given path.
    ///
    /// Automatically initializes the schema (labels, vector indexes, text indexes).
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_config(path, HnswConfig::default())
    }

    /// Open or create a persistent Grafeo database with custom HNSW config.
    pub fn open_with_config(path: &Path, config: HnswConfig) -> Result<Self> {
        let db = GrafeoDB::open(path)?;
        let store = Self { db, hnsw_config: config };
        store.init_schema()?;
        Ok(store)
    }

    /// Create a new in-memory Grafeo database (useful for tests).
    ///
    /// Automatically initializes the schema.
    pub fn new_in_memory() -> Result<Self> {
        Self::new_in_memory_with_config(HnswConfig::default())
    }

    /// Create a new in-memory Grafeo database with custom HNSW config.
    pub fn new_in_memory_with_config(config: HnswConfig) -> Result<Self> {
        let db = GrafeoDB::new_in_memory();
        let store = Self { db, hnsw_config: config };
        store.init_schema()?;
        Ok(store)
    }

    /// Close the database, flushing all pending writes.
    ///
    /// For persistent databases this ensures everything is safely on disk.
    pub fn close(&self) -> Result<()> {
        self.db.close().map_err(Into::into)
    }

    /// Initialize schema: create HNSW vector indexes and BM25 text indexes.
    ///
    /// Vector indexes are only created for labels that store embeddings
    /// (Episodic, Knowledge, Procedural, Autobiographical).
    /// Text indexes are created for searchable text fields defined in
    /// [`EPISODIC_TEXT_FIELDS`] and [`KNOWLEDGE_TEXT_FIELDS`].
    fn init_schema(&self) -> Result<()> {
        let cfg = &self.hnsw_config;

        // HNSW vector indexes on the "embedding" property.
        for label in [
            labels::EPISODIC,
            labels::KNOWLEDGE,
            labels::PROCEDURAL,
            labels::AUTOBIOGRAPHICAL,
        ] {
            self.db.create_vector_index(
                label,
                "embedding",
                Some(cfg.dim),
                Some(VECTOR_METRIC),
                Some(cfg.m),
                Some(cfg.ef_construction),
                None,
            )?;
        }

        // BM25 text indexes for Episodic fields.
        for field in EPISODIC_TEXT_FIELDS {
            self.db.create_text_index(labels::EPISODIC, field)?;
        }

        // BM25 text indexes for Knowledge fields.
        for field in KNOWLEDGE_TEXT_FIELDS {
            self.db.create_text_index(labels::KNOWLEDGE, field)?;
        }

        Ok(())
    }

    /// Return the HNSW config used by this store.
    pub fn hnsw_config(&self) -> &HnswConfig {
        &self.hnsw_config
    }

    /// Return a reference to the underlying GrafeoDB.
    pub fn db(&self) -> &GrafeoDB {
        &self.db
    }
}
