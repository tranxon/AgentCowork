//! Conversation history management (FIFO trimming + Tool Result folding)

/// History manager for conversation
pub struct HistoryManager {
    // TODO: Add history storage
}

impl HistoryManager {
    /// Create new history manager
    pub fn new(max_tokens: u64) -> Self {
        unimplemented!()
    }

    /// Add message to history
    pub fn append(&mut self, message: String) {
        unimplemented!()
    }

    /// Trim history using FIFO strategy
    pub fn trim(&mut self) {
        unimplemented!()
    }

    /// Fold old tool results (keep last 4 complete, summarize older)
    pub fn fold_tool_results(&mut self) {
        unimplemented!()
    }
}
