//! Consolidation markers and cleanup for episodic memory.
#![allow(clippy::collapsible_if)]

use chrono::TimeDelta;
use grafeo_common::types::{NodeId, Value};

use crate::error::Result;
use crate::grafeo::GrafeoStore;
// labels not needed in this module

impl GrafeoStore {
    /// Mark an episode as consolidated (transferred to semantic layer).
    pub fn mark_episode_consolidated(&self, episode_id: NodeId) -> Result<()> {
        self.db
            .set_node_property(episode_id, "consolidated", Value::from(true));
        Ok(())
    }

    /// Retrieve unconsolidated episodes, ordered by timestamp ascending.
    ///
    /// These are candidates for the offline consolidation pipeline.
    pub fn get_unconsolidated_episodes(&self, limit: usize) -> Result<Vec<crate::types::Episode>> {
        let session = self.db.session();
        let gql = format!(
            "MATCH (e:Episodic) WHERE e.consolidated = false RETURN e ORDER BY e.timestamp ASC LIMIT {}",
            limit
        );
        let result = session.execute(&gql)?;

        let mut episodes = Vec::new();
        for row in result.rows() {
            if let Some(Value::Map(map)) = row.first() {
                if let Ok(ep) = crate::episodic::value_to_episode(&Value::Map(map.clone())) {
                    episodes.push(ep);
                }
            }
        }
        Ok(episodes)
    }

    /// Remove old consolidated episodes beyond the retention period.
    ///
    /// Returns the number of deleted episodes.
    pub fn cleanup_old_episodes(&self, retention_days: u32) -> Result<usize> {
        let cutoff = chrono::Utc::now() - TimeDelta::days(i64::from(retention_days));
        let cutoff_us = cutoff.timestamp_micros();

        // Find candidate episodes and filter by timestamp in Rust.
        let session = self.db.session();
        let gql = "MATCH (e:Episodic) WHERE e.consolidated = true RETURN e";
        let result = session.execute(gql)?;

        let mut ids = Vec::new();
        for row in result.rows() {
            if let Some(Value::Map(map)) = row.first() {
                if let Ok(ep) = crate::episodic::value_to_episode(&Value::Map(map.clone())) {
                    if ep.timestamp.timestamp_micros() < cutoff_us {
                        if let Some(id) = ep.id {
                            ids.push(id);
                        }
                    }
                }
            }
        }

        let count = ids.len();
        for id in ids {
            self.db.delete_node(id);
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentType, EMBEDDING_DIM, Episode};
    use chrono::{DateTime, Utc};
    use std::collections::HashMap;

    fn test_store() -> GrafeoStore {
        GrafeoStore::new_in_memory().unwrap()
    }

    fn test_dt() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap()
    }

    fn make_episode(session_id: &str, content: &str, ts: DateTime<Utc>) -> Episode {
        Episode {
            id: None,
            session_id: session_id.to_string(),
            turn_index: 0,
            role: "user".to_string(),
            content: content.to_string(),
            content_type: ContentType::Informational,
            embedding: Some(vec![0.1f32; EMBEDDING_DIM]),
            timestamp: ts,
            consolidated: false,
            metadata: HashMap::new(),
            artifact_refs: vec![],
            importance: 0.5,
        }
    }

    #[test]
    fn test_mark_consolidated() {
        let store = test_store();
        let ep = make_episode("s1", "hello", test_dt());
        let id = store.store_episode(&ep).unwrap();

        store.mark_episode_consolidated(id).unwrap();

        let unconsolidated = store.get_unconsolidated_episodes(10).unwrap();
        assert!(unconsolidated.is_empty());
    }

    #[test]
    fn test_get_unconsolidated_episodes() {
        let store = test_store();
        let base = test_dt();

        let ep1 = make_episode("s1", "first", base);
        let mut ep2 = make_episode("s1", "second", base + TimeDelta::minutes(1));
        ep2.consolidated = true;
        store.store_episode(&ep1).unwrap();
        store.store_episode(&ep2).unwrap();

        let results = store.get_unconsolidated_episodes(10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "first");
    }

    #[test]
    fn test_cleanup_old_episodes() {
        let store = test_store();
        let now = Utc::now();

        let mut ep1 = make_episode("s1", "old", now - TimeDelta::days(30));
        ep1.consolidated = true;
        let mut ep2 = make_episode("s1", "recent", now - TimeDelta::days(5));
        ep2.consolidated = true;
        let mut ep3 = make_episode("s1", "unconsolidated old", now - TimeDelta::days(30));
        ep3.consolidated = false;
        store.store_episode(&ep1).unwrap();
        store.store_episode(&ep2).unwrap();
        store.store_episode(&ep3).unwrap();

        let deleted = store.cleanup_old_episodes(14).unwrap();
        assert_eq!(deleted, 1); // only ep1 is old AND consolidated

        let remaining = store.search_episodes_by_session("s1", 10).unwrap();
        assert_eq!(remaining.len(), 2);
    }
}
