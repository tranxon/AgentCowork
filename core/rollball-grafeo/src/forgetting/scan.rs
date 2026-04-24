//! Background decay scanning.
//!
//! Implements periodic scans that evaluate memory nodes against the decay
//! formula and transition them between lifecycle states.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use grafeo_common::types::{NodeId, Value};

use crate::error::Result;
use crate::grafeo::GrafeoStore;
use crate::types::{labels, NodeStatus};

use super::decay::{compute_decay_score, DecayConfig};

/// Labels that participate in decay scanning.
/// Autobiographical nodes are skipped (always Active).
const DECAY_LABELS: &[&str] = &[
    labels::EPISODIC,
    labels::KNOWLEDGE,
    labels::PROCEDURAL,
];

impl GrafeoStore {
    /// Scan all nodes and update decay scores.
    ///
    /// Iterates over all memory labels that participate in decay, computes
    /// the current decay score for each Active node, and transitions those
    /// whose score falls below the threshold to Dormant.
    ///
    /// Returns the number of nodes transitioned to Dormant.
    pub fn run_decay_scan(&self, config: &DecayConfig) -> Result<usize> {
        let mut transitioned = 0;
        let now = Utc::now();

        for label in DECAY_LABELS {
            let graph = self.db.graph_store();
            for node_id in graph.nodes_by_label(label) {
                if let Some(node) = self.db.get_node(node_id) {
                    // Skip nodes that are already Dormant or Pending.
                    if let Some(Value::String(s)) = node.properties.get(&"status".into())
                        && s.as_str() != crate::types::NodeStatus::Active.as_str()
                    {
                        continue;
                    }

                    let importance = node
                        .properties
                        .get(&"importance".into())
                        .and_then(|v| v.as_float64())
                        .unwrap_or(0.5) as f32;

                    let last_accessed = node
                        .properties
                        .get(&"last_accessed".into())
                        .and_then(|v| v.as_timestamp());

                    let access_count = node
                        .properties
                        .get(&"access_count".into())
                        .and_then(|v| v.as_int64())
                        .unwrap_or(0) as u32;

                    let hours_since = last_accessed
                        .and_then(|ts| {
                            DateTime::from_timestamp_micros(ts.as_micros())
                                .map(|dt| (now - dt).num_seconds() as f64 / 3600.0)
                        })
                        .unwrap_or_else(|| {
                            // Fallback to created_at if last_accessed is missing.
                            node.properties
                                .get(&"created_at".into())
                                .and_then(|v| v.as_timestamp())
                                .and_then(|ts| {
                                    DateTime::from_timestamp_micros(ts.as_micros())
                                        .map(|dt| (now - dt).num_seconds() as f64 / 3600.0)
                                })
                                .unwrap_or(0.0)
                        });

                    let score = compute_decay_score(config, importance, hours_since, access_count);

                    if score < config.dormant_threshold {
                        self.transition_to_dormant(node_id)?;
                        transitioned += 1;
                    }
                }
            }
        }

        Ok(transitioned)
    }

    /// Get nodes that are candidates for dormancy (score < threshold).
    ///
    /// This is a read-only operation: it computes decay scores but does not
    /// modify any node state.
    pub fn get_dormant_candidates(&self, config: &DecayConfig) -> Result<Vec<(NodeId, f32)>> {
        let mut candidates = Vec::new();
        let now = Utc::now();

        for label in DECAY_LABELS {
            let graph = self.db.graph_store();
            for node_id in graph.nodes_by_label(label) {
                if let Some(node) = self.db.get_node(node_id) {
                    // Only consider Active nodes.
                    if let Some(Value::String(s)) = node.properties.get(&"status".into())
                        && s.as_str() != crate::types::NodeStatus::Active.as_str()
                    {
                        continue;
                    }

                    let importance = node
                        .properties
                        .get(&"importance".into())
                        .and_then(|v| v.as_float64())
                        .unwrap_or(0.5) as f32;

                    let last_accessed = node
                        .properties
                        .get(&"last_accessed".into())
                        .and_then(|v| v.as_timestamp());

                    let access_count = node
                        .properties
                        .get(&"access_count".into())
                        .and_then(|v| v.as_int64())
                        .unwrap_or(0) as u32;

                    let hours_since = last_accessed
                        .and_then(|ts| {
                            DateTime::from_timestamp_micros(ts.as_micros())
                                .map(|dt| (now - dt).num_seconds() as f64 / 3600.0)
                        })
                        .unwrap_or_else(|| {
                            node.properties
                                .get(&"created_at".into())
                                .and_then(|v| v.as_timestamp())
                                .and_then(|ts| {
                                    DateTime::from_timestamp_micros(ts.as_micros())
                                        .map(|dt| (now - dt).num_seconds() as f64 / 3600.0)
                                })
                                .unwrap_or(0.0)
                        });

                    let score = compute_decay_score(config, importance, hours_since, access_count);

                    if score < config.dormant_threshold {
                        candidates.push((node_id, score));
                    }
                }
            }
        }

        Ok(candidates)
    }

    /// Transition a node from Active to Dormant.
    ///
    /// Updates the `status` property and records `dormant_since`.
    pub fn transition_to_dormant(&self, node_id: NodeId) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as i64;
        let ts = grafeo_common::types::Timestamp::from_micros(now);

        self.db
            .set_node_property(node_id, "status", Value::from("Dormant"));
        self.db
            .set_node_property(node_id, "dormant_since", Value::from(ts));
        Ok(())
    }

    /// Batch transition nodes to Dormant.
    ///
    /// Returns the number of nodes successfully transitioned.
    pub fn batch_transition_to_dormant(&self, node_ids: &[NodeId]) -> Result<usize> {
        let mut count = 0;
        for &node_id in node_ids {
            self.transition_to_dormant(node_id)?;
            count += 1;
        }
        Ok(count)
    }

    /// Reactivate a dormant node (set status back to Active, boost decay score).
    ///
    /// Clears `dormant_since` and increments `access_count`.
    pub fn reactivate_node(&self, node_id: NodeId) -> Result<()> {
        // Increment access_count.
        let new_access_count = self
            .db
            .get_node(node_id)
            .and_then(|n| n.properties.get(&"access_count".into()).and_then(|v| v.as_int64()))
            .unwrap_or(0)
            + 1;

        self.db.set_node_property(
            node_id,
            "status",
            Value::from(NodeStatus::Active.as_str()),
        );
        self.db
            .set_node_property(node_id, "dormant_since", Value::Null);
        self.db
            .set_node_property(node_id, "access_count", Value::from(new_access_count));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as i64;
        let ts = grafeo_common::types::Timestamp::from_micros(now);
        self.db
            .set_node_property(node_id, "last_accessed", Value::from(ts));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeStatus;

    fn test_store() -> GrafeoStore {
        GrafeoStore::new_in_memory().unwrap()
    }

    fn create_test_node(
        store: &GrafeoStore,
        label: &str,
        importance: f64,
        hours_old: f64,
        access_count: i64,
    ) -> NodeId {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as i64;
        let created_ts = grafeo_common::types::Timestamp::from_micros(
            now - (hours_old * 3600.0 * 1_000_000.0) as i64,
        );

        store
            .store_node(
                label,
                [
                    ("status", Value::from(NodeStatus::Active.as_str())),
                    ("importance", Value::from(importance)),
                    ("created_at", Value::from(created_ts)),
                    ("last_accessed", Value::from(created_ts)),
                    ("access_count", Value::from(access_count)),
                ],
            )
            .unwrap()
    }

    #[test]
    fn test_run_decay_scan() {
        let store = test_store();
        let config = DecayConfig::default();

        // Create a very old node with low importance -> should become Dormant.
        let _old = create_test_node(&store, labels::KNOWLEDGE, 0.2, 24.0 * 60.0, 0);
        // Create a fresh node with high importance -> should stay Active.
        let _fresh = create_test_node(&store, labels::KNOWLEDGE, 0.9, 0.0, 0);

        let count = store.run_decay_scan(&config).unwrap();
        assert_eq!(count, 1);

        // Verify the old node is now Dormant.
        let node = store.db.get_node(_old).unwrap();
        let status = node.properties.get(&"status".into()).unwrap().as_str().unwrap();
        assert_eq!(status, "Dormant");

        // Verify the fresh node is still Active.
        let node = store.db.get_node(_fresh).unwrap();
        let status = node.properties.get(&"status".into()).unwrap().as_str().unwrap();
        assert_eq!(status, "Active");
    }

    #[test]
    fn test_get_dormant_candidates() {
        let store = test_store();
        let config = DecayConfig::default();

        let _old = create_test_node(&store, labels::KNOWLEDGE, 0.2, 24.0 * 60.0, 0);
        let _fresh = create_test_node(&store, labels::KNOWLEDGE, 0.9, 0.0, 0);

        let candidates = store.get_dormant_candidates(&config).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, _old);
    }

    #[test]
    fn test_transition_to_dormant() {
        let store = test_store();
        let node_id = create_test_node(&store, labels::KNOWLEDGE, 0.5, 0.0, 0);

        store.transition_to_dormant(node_id).unwrap();

        let node = store.db.get_node(node_id).unwrap();
        let status = node.properties.get(&"status".into()).unwrap().as_str().unwrap();
        assert_eq!(status, "Dormant");
        assert!(node.properties.contains_key(&"dormant_since".into()));
    }

    #[test]
    fn test_batch_transition_to_dormant() {
        let store = test_store();
        let n1 = create_test_node(&store, labels::KNOWLEDGE, 0.5, 0.0, 0);
        let n2 = create_test_node(&store, labels::KNOWLEDGE, 0.5, 0.0, 0);

        let count = store.batch_transition_to_dormant(&[n1, n2]).unwrap();
        assert_eq!(count, 2);

        for id in [n1, n2] {
            let node = store.db.get_node(id).unwrap();
            let status = node.properties.get(&"status".into()).unwrap().as_str().unwrap();
            assert_eq!(status, "Dormant");
        }
    }

    #[test]
    fn test_reactivate_node() {
        let store = test_store();
        let node_id = create_test_node(&store, labels::KNOWLEDGE, 0.5, 0.0, 0);

        // Transition to Dormant first.
        store.transition_to_dormant(node_id).unwrap();

        // Reactivate.
        store.reactivate_node(node_id).unwrap();

        let node = store.db.get_node(node_id).unwrap();
        let status = node.properties.get(&"status".into()).unwrap().as_str().unwrap();
        assert_eq!(status, "Active");

        // dormant_since should be cleared.
        let dormant_since = node.properties.get(&"dormant_since".into());
        assert!(dormant_since.is_none() || dormant_since.unwrap() == &Value::Null);

        // access_count should be incremented.
        let access_count = node
            .properties
            .get(&"access_count".into())
            .unwrap()
            .as_int64()
            .unwrap();
        assert_eq!(access_count, 1);
    }
}
