//! Local budget pre-check

use rollball_core::Budget;

/// Budget guard for local token estimation
pub struct BudgetGuard {
    budget: Budget,
    current_usage: u64,
}

impl BudgetGuard {
    /// Create new budget guard
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            current_usage: 0,
        }
    }

    /// Check if budget allows new request
    pub fn check(&self, estimated_tokens: u64) -> Result<bool, String> {
        // TODO: Implement budget checking
        unimplemented!()
    }

    /// Update usage
    pub fn update_usage(&mut self, tokens: u64) {
        self.current_usage += tokens;
    }
}
