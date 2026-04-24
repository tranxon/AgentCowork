//! Memory module (Grafeo client)

pub mod manager;

pub use manager::{
    ConversationRecord, InjectedMemory, MemoryManager, MemoryManagerConfig, RetrievedMemory,
    RetrievalResult,
};
