//! Multi-signal conflict detection for memory nodes.
//!
//! Implements a three-layer conflict detection strategy:
//! - Layer 1: Semantic similarity (embedding cosine distance)
//! - Layer 2: Temporal conflict (same subject within time window)
//! - Layer 3: Context negation (negation words in source episode)

use rollball_memory::{ConflictSignal, ConflictType};

/// Default semantic similarity thresholds by node sub-type.
pub const FACT_THRESHOLD: f32 = 0.85;
pub const PREFERENCE_THRESHOLD: f32 = 0.80;
pub const RELATION_THRESHOLD: f32 = 0.90;

/// Default temporal conflict window in hours.
pub const TEMPORAL_WINDOW_HOURS: u64 = 24;

/// Negation keywords that suggest a correction.
pub const NEGATION_KEYWORDS: &[&str] = &[
    "不是", "其实", "改为", "错了", "实际上", "不对",
    "not", "actually", "changed", "wrong", "incorrect", "no longer",
];

/// Keywords that suggest evolution (gradual change) rather than correction.
const EVOLUTION_KEYWORDS: &[&str] = &[
    "搬", "换", "升", "调", "变成",
    "moved", "switched", "updated", "evolved", "became",
];

/// Time threshold (in hours) beyond which a conflict is more likely evolution.
const EVOLUTION_TIME_THRESHOLD_HOURS: f64 = 168.0; // 7 days

/// Detect conflict signals between a new node and an existing node.
///
/// Returns `None` if the semantic score is below the given threshold (no conflict).
/// Otherwise, applies the three-layer detection strategy and returns a heuristic
/// conflict classification.
///
/// # Arguments
/// * `semantic_score` — Embedding cosine similarity between the two nodes.
/// * `threshold` — Minimum semantic similarity to consider a conflict.
/// * `time_diff_hours` — Time difference between the two nodes in hours.
/// * `source_content` — Text content of the new (incoming) node.
pub fn detect_conflict(
    semantic_score: f32,
    threshold: f32,
    time_diff_hours: f64,
    source_content: &str,
) -> Option<ConflictSignal> {
    // Layer 1: Check semantic similarity — must exceed threshold.
    if semantic_score < threshold {
        return None;
    }

    // Layer 2: Temporal conflict — same subject within time window.
    let temporal_conflict = time_diff_hours < TEMPORAL_WINDOW_HOURS as f64;

    // Layer 3: Context negation — negation words in source episode.
    let context_negation = contains_negation(source_content);

    // Heuristic rules to determine conflict type.
    let (suggested_type, heuristic_confidence) = if temporal_conflict && context_negation {
        // Recent + negation → likely a correction of wrong information.
        (ConflictType::Correction, 0.9)
    } else if time_diff_hours > EVOLUTION_TIME_THRESHOLD_HOURS && contains_evolution_keyword(source_content) {
        // Long time gap + change keywords → likely knowledge evolution.
        (ConflictType::Evolution, 0.8)
    } else {
        // Semantic overlap alone — ambiguous, needs user confirmation.
        (ConflictType::Ambiguous, 0.5)
    };

    Some(ConflictSignal {
        semantic_score,
        temporal_conflict,
        context_negation,
        suggested_type,
        heuristic_confidence,
    })
}

/// Check if source content contains negation keywords.
fn contains_negation(content: &str) -> bool {
    NEGATION_KEYWORDS.iter().any(|kw| content.contains(kw))
}

/// Check if source content contains evolution (gradual change) keywords.
fn contains_evolution_keyword(content: &str) -> bool {
    EVOLUTION_KEYWORDS.iter().any(|kw| content.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_conflict_below_threshold() {
        let result = detect_conflict(0.5, 0.85, 1.0, "some content");
        assert!(result.is_none());
    }

    #[test]
    fn test_correction_conflict() {
        let result = detect_conflict(0.90, 0.85, 2.0, "实际上不是这样的");
        assert!(result.is_some());
        let signal = result.unwrap();
        assert!(signal.temporal_conflict);
        assert!(signal.context_negation);
        assert_eq!(signal.suggested_type, ConflictType::Correction);
        assert!((signal.heuristic_confidence - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_evolution_conflict() {
        let result = detect_conflict(0.88, 0.85, 200.0, "我搬到了北京");
        assert!(result.is_some());
        let signal = result.unwrap();
        assert!(!signal.temporal_conflict);
        assert_eq!(signal.suggested_type, ConflictType::Evolution);
        assert!((signal.heuristic_confidence - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ambiguous_conflict() {
        let result = detect_conflict(0.87, 0.85, 50.0, "some neutral content");
        assert!(result.is_some());
        let signal = result.unwrap();
        assert_eq!(signal.suggested_type, ConflictType::Ambiguous);
        assert!((signal.heuristic_confidence - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_contains_negation() {
        assert!(contains_negation("这不是真的"));
        assert!(contains_negation("that is wrong"));
        assert!(!contains_negation("hello world"));
    }

    #[test]
    fn test_contains_evolution_keyword() {
        assert!(contains_evolution_keyword("我搬到了上海"));
        assert!(contains_evolution_keyword("I moved to Berlin"));
        assert!(!contains_evolution_keyword("hello world"));
    }
}
