//! Token counting module
//!
//! Uses a unified model→ratio lookup table for token estimation:
//! `tokens ≈ chars / ratio`. The ratio is calibrated from LLM API feedback
//! after each request and persisted to disk for reuse across sessions.
//!
//! # Unified API
//!
//! **All** token counting in ACowork MUST go through [`count_text`].
//! Do NOT use `content.len() / 4` or any other ad-hoc heuristic —
//! they cause the debug panel and status panel to show contradictory numbers.
pub mod counter;
pub mod ratio_store;

pub use counter::{TokenCounter, estimate_image_tokens};
pub use ratio_store::ModelRatioStore;

/// The single unified entry point for token counting in ACowork.
///
/// Uses model-aware ratio-based counting:
/// - `tokens = ceil(chars / ratio)` where ratio is calibrated from API feedback
/// - Uncalibrated models fall back to default ratio 3.5
/// - Ratio is shared via ModelRatioStore for consistency across all counting paths
///
/// # Why a unified API matters
///
/// Before this function existed, token counting was scattered across:
/// - `content.len() / 4` in debug panel → overestimates Chinese text by ~2.9x
/// - `chars / 3.5` in context builder safety checks → inconsistent with debug
/// - `TokenCounter::count_text()` in history manager → the only correct path
///
/// Two different numbers displayed to the user for the same session is a UX bug.
/// This function ensures **one source of truth** for all token estimates.
pub fn count_text(text: &str, model: &str) -> usize {
    TokenCounter::new().count_text(text, model) as usize
}
