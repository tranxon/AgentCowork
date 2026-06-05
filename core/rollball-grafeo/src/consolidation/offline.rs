//! Offline consolidation — background upgrade of Pending knowledge nodes.
//!
//! Phase 2 implements a simple age-and-evidence upgrade strategy.
//! Phase 3 adds full LLM-based re-evaluation and generalization.

use chrono::{TimeDelta, Utc};
use grafeo_common::types::Value;

use crate::consolidation::generalization::GeneralizationConfig;
use crate::consolidation::triple_extraction::TripleExtractorLlm;
use crate::error::Result;
use crate::grafeo::GrafeoStore;
use crate::types::{labels, AutobioCategory, AutobiographicalNode, KnowledgeNode, NodeStatus};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Offline consolidation configuration.
#[derive(Debug, Clone)]
pub struct OfflineConsolidationConfig {
    /// Maximum number of pending nodes to process per batch.
    /// Default: 50.
    pub batch_size: usize,
    /// Minimum age (in hours) before a Pending node is eligible for
    /// offline processing. Default: 1.
    pub min_pending_age_hours: u64,
}

impl Default for OfflineConsolidationConfig {
    fn default() -> Self {
        Self {
            batch_size: 50,
            min_pending_age_hours: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Result of an offline consolidation run.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct OfflineConsolidationResult {
    /// Number of nodes upgraded from Pending → Active.
    pub upgraded: usize,
    /// Number of nodes kept as Pending (not old enough or not enough evidence).
    pub kept_pending: usize,
    /// Number of nodes marked Dormant (low confidence after re-evaluation).
    pub marked_dormant: usize,
    /// Number of new ProceduralNodes created by generalization.
    pub procedural_created: usize,
    /// Number of existing ProceduralNodes boosted by generalization.
    pub procedural_boosted: usize,
    /// Number of History nodes compressed into summaries.
    pub history_compressed: usize,
}

// ---------------------------------------------------------------------------
// GrafeoStore methods
// ---------------------------------------------------------------------------

impl GrafeoStore {
    /// Run offline consolidation on pending nodes, including generalization.
    ///
    /// Phase 3 enhancement: after upgrading/downgrading pending KnowledgeNodes,
    /// runs experience generalization to extract ProceduralNodes from
    /// unconsolidated episodes (step ④ in the design doc).
    ///
    /// The generalization step requires an embedding function. If `None` is
    /// provided, generalization is skipped.
    #[allow(clippy::type_complexity)]
    pub async fn run_offline_consolidation_with_generalization(
        &self,
        config: &OfflineConsolidationConfig,
        llm: Option<&dyn TripleExtractorLlm>,
        embedding_fn: Option<&(dyn Fn(&str) -> Vec<f32> + Send + Sync)>,
        gen_config: Option<&GeneralizationConfig>,
    ) -> Result<OfflineConsolidationResult> {
        // Step 1: Standard offline consolidation (upgrade/downgrade Pending nodes)
        let mut result = self.run_offline_consolidation(config)?;

        // Step 2: Experience generalization (if embedding function provided)
        if let Some(emb_fn) = embedding_fn {
            let gen_config = gen_config.cloned().unwrap_or_default();
            let gen_result = self
                .run_generalization(llm, emb_fn, &gen_config)
                .await?;
            result.procedural_created = gen_result.nodes_created;
            result.procedural_boosted = gen_result.nodes_boosted;
        }

        // Step 3: Compress History nodes if there are too many (> 10).
        result.history_compressed = self.compress_history_nodes(10)?;

        Ok(result)
    }

    /// Run offline consolidation on pending nodes.
    ///
    /// Phase 2 strategy: upgrade Pending nodes to Active if they are older
    /// than `min_pending_age_hours` and have a confidence >= 0.7 (basic
    /// evidence threshold). Nodes with very low confidence (< 0.3) are
    /// downgraded to Dormant.
    ///
    /// Phase 3: Full LLM-based re-evaluation is available via
    /// `run_offline_consolidation_with_generalization`.
    pub fn run_offline_consolidation(
        &self,
        config: &OfflineConsolidationConfig,
    ) -> Result<OfflineConsolidationResult> {
        let pending_nodes = self.get_pending_for_consolidation(
            config.min_pending_age_hours,
            config.batch_size,
        )?;

        let mut result = OfflineConsolidationResult::default();

        for mut node in pending_nodes {
            if node.confidence < 0.3 {
                // Very low confidence → mark Dormant.
                node.status = NodeStatus::Dormant;
                node.updated_at = Utc::now();
                self.update_knowledge(&node)?;
                result.marked_dormant += 1;
            } else if node.confidence >= 0.7 {
                // Reasonable confidence and old enough → upgrade to Active.
                node.status = NodeStatus::Active;
                node.updated_at = Utc::now();
                self.update_knowledge(&node)?;
                result.upgraded += 1;
            } else {
                // Between 0.3 and 0.7 — keep Pending, wait for more evidence.
                result.kept_pending += 1;
            }
        }

        Ok(result)
    }

    /// Get pending knowledge nodes that are old enough for offline processing.
    ///
    /// Returns up to `limit` nodes whose `created_at` is at least
    /// `min_age_hours` hours ago and whose status is `Pending`.
    pub fn get_pending_for_consolidation(
        &self,
        min_age_hours: u64,
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>> {
        let cutoff = Utc::now() - TimeDelta::hours(min_age_hours as i64);
        let cutoff_us = cutoff.timestamp_micros();

        let graph = self.db.graph_store();
        let node_ids = graph.nodes_by_label(labels::KNOWLEDGE);

        let mut pending = Vec::new();

        for id in node_ids {
            if pending.len() >= limit {
                break;
            }

            if let Some(n) = self.db.get_node(id) {
                // Check status == Pending.
                let status_match = n
                    .get_property("status")
                    .and_then(Value::as_str)
                    .map(|s| s == "Pending")
                    .unwrap_or(false);

                if !status_match {
                    continue;
                }

                // Check created_at is old enough.
                let is_old_enough = n
                    .get_property("created_at")
                    .and_then(|v| v.as_timestamp())
                    .map(|ts| ts.as_micros() <= cutoff_us)
                    .unwrap_or(false);

                if !is_old_enough {
                    continue;
                }

                // Reconstruct the full KnowledgeNode.
                let props: Vec<(String, Value)> = n
                    .properties_as_btree()
                    .into_iter()
                    .map(|(k, v)| (k.as_str().to_string(), v))
                    .collect();
                let kn = KnowledgeNode::from_properties(id, &props)?;
                pending.push(kn);
            }
        }

        Ok(pending)
    }

    /// Compress History autobiographical nodes when they exceed a threshold.
    ///
    /// When there are more than `max_history_nodes` (default 10) History
    /// nodes, this method groups them by month and creates summary nodes.
    /// The original History nodes are marked Dormant (not deleted).
    ///
    /// This is a rule-based compression (no LLM). Phase 3 will add
    /// LLM-based summarization for richer compression.
    ///
    /// Returns the number of History nodes compressed (marked Dormant).
    pub fn compress_history_nodes(&self, max_history_nodes: usize) -> Result<usize> {
        // Find all History autobiographical nodes.
        let history_nodes = self.find_autobiographical_by_category(AutobioCategory::History)?;

        if history_nodes.len() <= max_history_nodes {
            return Ok(0); // Nothing to compress
        }

        // Group by month (YYYY-MM format).
        let mut monthly: std::collections::BTreeMap<String, Vec<AutobiographicalNode>> =
            std::collections::BTreeMap::new();

        for node in &history_nodes {
            let month_key = node.created_at.format("%Y-%m").to_string();
            monthly.entry(month_key).or_default().push(node.clone());
        }

        let mut compressed = 0usize;

        for (month, nodes) in monthly {
            if nodes.len() <= 1 {
                // Single node in a month — keep it Active.
                continue;
            }

            // Create a summary node for this month.
            let summary_value = nodes
                .iter()
                .map(|n| n.value.as_str())
                .collect::<Vec<_>>()
                .join("；");

            // Truncate to 200 chars to avoid bloat.
            let truncated = if summary_value.len() > 200 {
                format!("{}…", &summary_value[..200])
            } else {
                summary_value
            };

            let summary_key = format!("history_summary_{}", month);
            let summary_node = AutobiographicalNode {
                id: None,
                category: AutobioCategory::History,
                key: summary_key,
                value: truncated,
                confidence: 0.9,
                source_episode_id: None,
                embedding: None,
                status: NodeStatus::Active,
                created_at: nodes[0].created_at,
                updated_at: Utc::now(),
                metadata: std::collections::HashMap::new(),
            };

            self.store_autobiographical(&summary_node)?;

            // Mark original nodes as Dormant.
            for mut node in nodes {
                if node.id.is_some() {
                    node.status = NodeStatus::Dormant;
                    node.updated_at = Utc::now();
                    self.update_autobiographical(&node)?;
                    compressed += 1;
                }
            }
        }

        Ok(compressed)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AutobioCategory, AutobiographicalNode, KnowledgeSubType, DEFAULT_EMBEDDING_DIM};

    fn test_store() -> GrafeoStore {
        GrafeoStore::new_in_memory().unwrap()
    }

    fn test_embedding() -> Vec<f32> {
        vec![0.1f32; DEFAULT_EMBEDDING_DIM]
    }

    // =====================================================================
    // Test: Offline consolidation upgrades old pending nodes
    // =====================================================================

    #[test]
    fn test_offline_consolidation_upgrade_pending_to_active() {
        let store = test_store();

        // Create a Pending node that is old enough.
        let old_time = Utc::now() - TimeDelta::hours(2);
        let node = KnowledgeNode {
            id: None,
            subject: "user".to_string(),
            predicate: "likes".to_string(),
            object: "coffee".to_string(),
            sub_type: KnowledgeSubType::Preference,
            confidence: 0.75,
            source_episode_id: None,
            embedding: Some(test_embedding()),
            status: NodeStatus::Pending,
            created_at: old_time,
            updated_at: old_time,
            metadata: std::collections::HashMap::new(),
        };
        store.store_knowledge(&node).unwrap();

        let config = OfflineConsolidationConfig {
            batch_size: 50,
            min_pending_age_hours: 1,
        };

        let result = store.run_offline_consolidation(&config).unwrap();
        assert_eq!(result.upgraded, 1);
        assert_eq!(result.kept_pending, 0);
        assert_eq!(result.marked_dormant, 0);
    }

    // =====================================================================
    // Test: Low confidence pending node → Dormant
    // =====================================================================

    #[test]
    fn test_offline_consolidation_low_confidence_to_dormant() {
        let store = test_store();

        let old_time = Utc::now() - TimeDelta::hours(2);
        let node = KnowledgeNode {
            id: None,
            subject: "user".to_string(),
            predicate: "likes".to_string(),
            object: "something".to_string(),
            sub_type: KnowledgeSubType::Fact,
            confidence: 0.2,
            source_episode_id: None,
            embedding: Some(test_embedding()),
            status: NodeStatus::Pending,
            created_at: old_time,
            updated_at: old_time,
            metadata: std::collections::HashMap::new(),
        };
        store.store_knowledge(&node).unwrap();

        let config = OfflineConsolidationConfig::default();
        let result = store.run_offline_consolidation(&config).unwrap();
        assert_eq!(result.marked_dormant, 1);
    }

    // =====================================================================
    // Test: Recent pending node → not processed
    // =====================================================================

    #[test]
    fn test_offline_consolidation_recent_pending_kept() {
        let store = test_store();

        // A Pending node that is too new.
        let node = KnowledgeNode {
            id: None,
            subject: "user".to_string(),
            predicate: "likes".to_string(),
            object: "tea".to_string(),
            sub_type: KnowledgeSubType::Preference,
            confidence: 0.75,
            source_episode_id: None,
            embedding: Some(test_embedding()),
            status: NodeStatus::Pending,
            created_at: Utc::now(), // just created
            updated_at: Utc::now(),
            metadata: std::collections::HashMap::new(),
        };
        store.store_knowledge(&node).unwrap();

        let config = OfflineConsolidationConfig {
            batch_size: 50,
            min_pending_age_hours: 1,
        };

        let result = store.run_offline_consolidation(&config).unwrap();
        assert_eq!(result.upgraded, 0);
        assert_eq!(result.kept_pending, 0); // not even returned by get_pending
    }

    // =====================================================================
    // Test: Active nodes are not affected
    // =====================================================================

    #[test]
    fn test_offline_consolidation_active_nodes_untouched() {
        let store = test_store();

        let old_time = Utc::now() - TimeDelta::hours(2);
        let node = KnowledgeNode {
            id: None,
            subject: "user".to_string(),
            predicate: "likes".to_string(),
            object: "chocolate".to_string(),
            sub_type: KnowledgeSubType::Fact,
            confidence: 0.75,
            source_episode_id: None,
            embedding: Some(test_embedding()),
            status: NodeStatus::Active,
            created_at: old_time,
            updated_at: old_time,
            metadata: std::collections::HashMap::new(),
        };
        let id = store.store_knowledge(&node).unwrap();

        let config = OfflineConsolidationConfig::default();
        let result = store.run_offline_consolidation(&config).unwrap();
        assert_eq!(result.upgraded, 0);

        // Active node should remain Active.
        let fetched = store.get_knowledge(id).unwrap().unwrap();
        assert_eq!(fetched.status, NodeStatus::Active);
    }

    // =====================================================================
    // Test: Default config values
    // =====================================================================

    #[test]
    fn test_offline_consolidation_default_config() {
        let config = OfflineConsolidationConfig::default();
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.min_pending_age_hours, 1);
    }

    // =====================================================================
    // Test: get_pending_for_consolidation respects limit
    // =====================================================================

    #[test]
    fn test_get_pending_for_consolidation_respects_limit() {
        let store = test_store();

        let old_time = Utc::now() - TimeDelta::hours(2);
        for i in 0..5 {
            let node = KnowledgeNode {
                id: None,
                subject: "user".to_string(),
                predicate: format!("item_{i}"),
                object: "value".to_string(),
                sub_type: KnowledgeSubType::Fact,
                confidence: 0.6,
                source_episode_id: None,
                embedding: Some(test_embedding()),
                status: NodeStatus::Pending,
                created_at: old_time,
                updated_at: old_time,
                metadata: std::collections::HashMap::new(),
            };
            store.store_knowledge(&node).unwrap();
        }

        let pending = store.get_pending_for_consolidation(1, 3).unwrap();
        assert_eq!(pending.len(), 3, "should respect limit of 3");
    }

    // =====================================================================
    // Test: Medium confidence (0.3-0.7) kept as pending
    // =====================================================================

    #[test]
    fn test_offline_consolidation_medium_confidence_kept_pending() {
        let store = test_store();

        let old_time = Utc::now() - TimeDelta::hours(2);
        let node = KnowledgeNode {
            id: None,
            subject: "user".to_string(),
            predicate: "likes".to_string(),
            object: "maybe".to_string(),
            sub_type: KnowledgeSubType::Preference,
            confidence: 0.5,
            source_episode_id: None,
            embedding: Some(test_embedding()),
            status: NodeStatus::Pending,
            created_at: old_time,
            updated_at: old_time,
            metadata: std::collections::HashMap::new(),
        };
        store.store_knowledge(&node).unwrap();

        let config = OfflineConsolidationConfig {
            batch_size: 50,
            min_pending_age_hours: 1,
        };

        let result = store.run_offline_consolidation(&config).unwrap();
        assert_eq!(result.kept_pending, 1);
        assert_eq!(result.upgraded, 0);
        assert_eq!(result.marked_dormant, 0);
    }

    // =====================================================================
    // Test: compress_history_nodes — no compression when ≤ 10 nodes
    // =====================================================================

    #[test]
    fn test_compress_history_nodes_no_compression_needed() {
        let store = test_store();

        // Create 5 History nodes (below threshold).
        for i in 0..5 {
            let node = AutobiographicalNode {
                id: None,
                category: AutobioCategory::History,
                key: format!("milestone_{}", i),
                value: format!("Event {}", i),
                confidence: 0.9,
                source_episode_id: None,
                embedding: None,
                status: NodeStatus::Active,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                metadata: std::collections::HashMap::new(),
            };
            store.store_autobiographical(&node).unwrap();
        }

        let compressed = store.compress_history_nodes(10).unwrap();
        assert_eq!(compressed, 0, "no compression should happen with ≤ 10 nodes");
    }

    // =====================================================================
    // Test: compress_history_nodes — compresses when > 10 nodes
    // =====================================================================

    #[test]
    fn test_compress_history_nodes_compresses_over_threshold() {
        let store = test_store();

        // Create 12 History nodes in the same month.
        // Since there are > 10 and > 1 in a month, they should be compressed.
        let base_time = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        for i in 0..12 {
            let node = AutobiographicalNode {
                id: None,
                category: AutobioCategory::History,
                key: format!("milestone_{}", i),
                value: format!("Event {}", i),
                confidence: 0.9,
                source_episode_id: None,
                embedding: None,
                status: NodeStatus::Active,
                created_at: base_time + TimeDelta::days(i),
                updated_at: base_time + TimeDelta::days(i),
                metadata: std::collections::HashMap::new(),
            };
            store.store_autobiographical(&node).unwrap();
        }

        let compressed = store.compress_history_nodes(10).unwrap();
        assert!(compressed > 0, "should compress some History nodes");

        // Verify: some original nodes should be Dormant now.
        let history = store.find_autobiographical_by_category(AutobioCategory::History).unwrap();
        let dormant_count = history.iter().filter(|n| n.status == NodeStatus::Dormant).count();
        assert!(dormant_count > 0, "some original nodes should be Dormant");

        // Verify: a summary node should exist.
        let summary = store.find_autobiographical_by_key("history_summary_2023-11").unwrap();
        assert!(summary.is_some(), "a summary node should be created for the month");
    }
}
