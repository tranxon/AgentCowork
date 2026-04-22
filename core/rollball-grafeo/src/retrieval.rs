//! Associative diffusion retrieval.

use grafeo_common::types::NodeId;

use crate::error::Result;
use crate::grafeo::GrafeoStore;

/// Apply min_score filtering to search results.
/// Returns filtered results and the count of removed items.
fn apply_min_score(
    results: Vec<(NodeId, f64)>,
    min_score: Option<f32>,
) -> (Vec<(NodeId, f64)>, usize) {
    match min_score {
        Some(threshold) => {
            let original_len = results.len();
            let filtered: Vec<(NodeId, f64)> = results
                .into_iter()
                .filter(|(_, score)| *score >= threshold as f64)
                .collect();
            let removed = original_len - filtered.len();
            (filtered, removed)
        }
        None => (results, 0),
    }
}

impl GrafeoStore {
    /// Vector similarity search using the HNSW index.
    ///
    /// Returns up to `k` results as `(NodeId, distance)` pairs sorted by
    /// ascending distance (lower = more similar).
    pub fn vector_search(
        &self,
        label: &str,
        embedding: &[f32],
        k: usize,
        ef: Option<usize>,
    ) -> Result<Vec<(NodeId, f32)>> {
        let results = self.db.vector_search(label, "embedding", embedding, k, ef, None)?;
        Ok(results)
    }

    /// Full-text search using the BM25 index.
    ///
    /// Returns up to `k` results as `(NodeId, score)` pairs sorted by
    /// descending score (higher = more relevant).
    pub fn text_search(&self, label: &str, query: &str, k: usize) -> Result<Vec<(NodeId, f64)>> {
        let results = self.db.text_search(label, "content", query, k)?;
        Ok(results)
    }

    /// Hybrid search combining BM25 text relevance and vector similarity.
    ///
    /// Uses Reciprocal Rank Fusion (RRF) by default.
    /// Returns up to `k` results as `(NodeId, fused_score)` pairs sorted by
    /// descending fused score.
    pub fn hybrid_search(
        &self,
        label: &str,
        text_prop: &str,
        vec_prop: &str,
        query: &str,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(NodeId, f64)>> {
        let results = self.db.hybrid_search(
            label,
            text_prop,
            vec_prop,
            query,
            Some(embedding),
            k,
            None,
        )?;
        Ok(results)
    }

    /// Maximal Marginal Relevance (MMR) search.
    ///
    /// Balances relevance (similarity to query) with diversity
    /// (dissimilarity among selected results).
    ///
    /// `lambda` controls the trade-off:
    /// - `1.0` = pure relevance
    /// - `0.0` = pure diversity
    pub fn mmr_search(
        &self,
        label: &str,
        embedding: &[f32],
        k: usize,
        lambda: Option<f32>,
    ) -> Result<Vec<(NodeId, f32)>> {
        let results = self
            .db
            .mmr_search(label, "embedding", embedding, k, None, lambda, None, None)?;
        Ok(results)
    }

    /// Graph expansion: traverse from a start node up to `max_hops` away.
    ///
    /// Returns a deduplicated list of reachable [`NodeId`]s (excluding the
    /// start node itself). The `threshold` parameter is reserved for future
    /// score-based pruning and currently has no effect.
    pub fn graph_expand(
        &self,
        start_id: NodeId,
        max_hops: usize,
        _threshold: f32,
    ) -> Result<Vec<NodeId>> {
        let session = self.db.session();
        let gql = format!(
            "MATCH (m)-[r*1..{}]-(other) WHERE id(m) = {} RETURN DISTINCT id(other)",
            max_hops,
            start_id.as_u64()
        );
        let result = session.execute(&gql)?;

        let mut nodes = Vec::new();
        for row in result.rows() {
            if let Some(grafeo_common::types::Value::Int64(id)) = row.first() {
                let node_id = NodeId::new(*id as u64);
                if node_id != start_id {
                    nodes.push(node_id);
                }
            }
        }
        Ok(nodes)
    }

    /// Full-text search with optional min_score filtering.
    ///
    /// Results with a score below `min_score` are removed. Returns the
    /// filtered results along with the number of removed entries.
    #[allow(clippy::too_many_arguments)]
    pub fn text_search_filtered(
        &self,
        label: &str,
        query: &str,
        k: usize,
        min_score: Option<f32>,
    ) -> Result<Vec<(NodeId, f64)>> {
        let results = self.db.text_search(label, "content", query, k)?;
        let (filtered, _) = apply_min_score(results, min_score);
        Ok(filtered)
    }

    /// Hybrid search with optional min_score filtering.
    ///
    /// Results with a fused score below `min_score` are removed.
    #[allow(clippy::too_many_arguments)]
    pub fn hybrid_search_filtered(
        &self,
        label: &str,
        text_prop: &str,
        vec_prop: &str,
        query: &str,
        embedding: &[f32],
        k: usize,
        min_score: Option<f32>,
    ) -> Result<Vec<(NodeId, f64)>> {
        let results = self.db.hybrid_search(
            label,
            text_prop,
            vec_prop,
            query,
            Some(embedding),
            k,
            None,
        )?;
        let (filtered, _) = apply_min_score(results, min_score);
        Ok(filtered)
    }

    /// Perform a weighted hybrid search combining text and vector search with custom weights.
    ///
    /// The fusion method can be adjusted based on the query hint type.
    /// After RRF fusion, scores are adjusted by `text_weight` and `vector_weight`,
    /// then results below `min_score` are filtered out.
    #[allow(clippy::too_many_arguments)]
    pub fn hybrid_search_weighted(
        &self,
        label: &str,
        text_prop: &str,
        vec_prop: &str,
        query: &str,
        embedding: &[f32],
        k: usize,
        text_weight: f32,
        vector_weight: f32,
        min_score: Option<f32>,
    ) -> Result<Vec<(NodeId, f64)>> {
        let mut results = self.db.hybrid_search(
            label,
            text_prop,
            vec_prop,
            query,
            Some(embedding),
            k,
            None,
        )?;

        // Apply weight adjustment: scale scores by the combined weight factor.
        // The weight factor represents how much each signal should influence the final score.
        let weight_factor = (text_weight + vector_weight) / 2.0;
        for (_, score) in &mut results {
            *score *= weight_factor as f64;
        }

        let (filtered, _) = apply_min_score(results, min_score);
        Ok(filtered)
    }

    /// Perform a search and collect retrieval metrics.
    ///
    /// Uses hybrid search internally, then applies min_score filtering and
    /// computes statistics about the result set including whether abstention
    /// was triggered (all results filtered out).
    pub fn search_with_metrics(
        &self,
        label: &str,
        query: &str,
        embedding: &[f32],
        k: usize,
        min_score: Option<f32>,
    ) -> Result<(Vec<(NodeId, f64)>, rollball_memory::RetrievalMetrics)> {
        let results = self.db.hybrid_search(
            label,
            "content",
            "embedding",
            query,
            Some(embedding),
            k,
            None,
        )?;

        let (filtered, filtered_count) = apply_min_score(results, min_score);

        let result_count = filtered.len();
        let max_score = filtered
            .iter()
            .map(|(_, s)| *s as f32)
            .fold(0.0_f32, f32::max);
        let avg_score = if result_count > 0 {
            filtered.iter().map(|(_, s)| *s as f32).sum::<f32>() / result_count as f32
        } else {
            0.0
        };
        let abstention_triggered = result_count == 0 && min_score.is_some();

        let metrics = rollball_memory::RetrievalMetrics {
            result_count,
            avg_score,
            max_score,
            abstention_triggered,
            filtered_count,
        };

        Ok((filtered, metrics))
    }
}
