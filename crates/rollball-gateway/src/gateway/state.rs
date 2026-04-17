//! Gateway global state

use std::collections::HashMap;

/// Gateway state
pub struct GatewayState {
    pub installed_agents: HashMap<String, AgentInfo>,
    pub running_agents: HashMap<String, RunningAgentInfo>,
}

pub struct AgentInfo {
    pub agent_id: String,
    pub version: String,
    pub install_path: String,
}

pub struct RunningAgentInfo {
    pub agent_id: String,
    pub pid: u32,
}
