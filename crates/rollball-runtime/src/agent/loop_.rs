//! Agent main loop (9 steps)
//!
//! References ZeroClaw agent/loop_.rs but simplified for IPC architecture.

use crate::error::Result;

/// Execute one iteration of the agent loop
pub async fn run_loop_iteration() -> Result<()> {
    // TODO: Implement 9-step loop:
    // ① Budget pre-check
    // ② Build context
    // ②.5 Preemptive Trim (if context overflow)
    // ③ Call LLM
    // ④ Parse response
    // ⑤ Tool dispatch
    // ⑥ Append to history
    // ⑦ Usage report (async)
    // ⑧ Loop detection
    // ⑨ DevMode control
    unimplemented!()
}
