//! Agent process lifecycle manager

use std::collections::HashMap;
use tokio::process::Child;

/// Lifecycle manager
pub struct LifecycleManager {
    processes: HashMap<String, AgentProcess>,
}

struct AgentProcess {
    child: Child,
    workspace: std::path::PathBuf,
}

impl LifecycleManager {
    /// Create new lifecycle manager
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    /// Start an agent process
    pub async fn start_agent(&mut self, agent_id: &str) -> Result<(), String> {
        // TODO: Spawn agent process
        unimplemented!()
    }

    /// Stop an agent process
    pub async fn stop_agent(&mut self, agent_id: &str) -> Result<(), String> {
        // TODO: Kill agent process
        unimplemented!()
    }
}
