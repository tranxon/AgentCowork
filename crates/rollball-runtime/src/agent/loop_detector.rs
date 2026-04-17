//! Loop detection (Exact Repeat / Ping-Pong / No Progress)
//!
//! Adapted from ZeroClaw agent/loop_detector.rs

/// Loop detector
pub struct LoopDetector {
    // TODO: Add detection state
}

impl LoopDetector {
    /// Create new loop detector
    pub fn new() -> Self {
        unimplemented!()
    }

    /// Check for loops in conversation history
    pub fn check(&self, history: &[String]) -> LoopDetectionResult {
        unimplemented!()
    }
}

/// Loop detection result
#[derive(Debug)]
pub enum LoopDetectionResult {
    NoLoop,
    ExactRepeat { count: u32 },
    PingPong { iterations: u32 },
    NoProgress { iterations: u32 },
}
