//! Permission declaration and validation types

use serde::{Deserialize, Serialize};

/// Permission types that Agents can declare in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Permission {
    /// Network access with optional URL whitelist
    /// e.g., "network:https://api.weather.com"
    Network(String),
    
    /// Filesystem read access with optional path restriction
    /// e.g., "filesystem:read:~/Documents"
    FilesystemRead(String),
    
    /// Filesystem write access with optional path restriction
    /// e.g., "filesystem:write:~/workdir"
    FilesystemWrite(String),
    
    /// Memory read access
    MemoryRead,
    
    /// Memory write access
    MemoryWrite,
    
    /// Intent send to specific agent
    /// e.g., "intent:send:com.example.calendar"
    IntentSend(String),
    
    /// Intent receive from specific agent
    /// e.g., "intent:receive:com.example.weather"
    IntentReceive(String),
    
    /// Shell command execution
    Shell,
}

impl Permission {
    /// Check if this permission matches a requested permission
    pub fn matches(&self, other: &Permission) -> bool {
        match (self, other) {
            (Permission::Network(_), Permission::Network(_)) => true,
            (Permission::FilesystemRead(_), Permission::FilesystemRead(_)) => true,
            (Permission::FilesystemWrite(_), Permission::FilesystemWrite(_)) => true,
            (Permission::MemoryRead, Permission::MemoryRead) => true,
            (Permission::MemoryWrite, Permission::MemoryWrite) => true,
            (Permission::IntentSend(_), Permission::IntentSend(_)) => true,
            (Permission::IntentReceive(_), Permission::IntentReceive(_)) => true,
            (Permission::Shell, Permission::Shell) => true,
            _ => false,
        }
    }
}
