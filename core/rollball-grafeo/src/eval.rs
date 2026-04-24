//! LongMemEval 5-dimension evaluation framework (skeleton for Phase 2).

use std::collections::HashMap;

/// LongMemEval dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvalDimension {
    /// Information Extraction — ability to extract facts from conversations.
    IE,
    /// Memory Retrieval — ability to recall relevant past information.
    MR,
    /// Temporal Reasoning — ability to reason about time-ordered events.
    TR,
    /// Knowledge Update — ability to integrate new and corrected knowledge.
    KU,
    /// Abstraction — ability to generalize from specific episodes.
    Abs,
}

impl EvalDimension {
    /// Returns the human-readable name of the dimension.
    pub fn name(&self) -> &'static str {
        match self {
            EvalDimension::IE => "Information Extraction",
            EvalDimension::MR => "Memory Retrieval",
            EvalDimension::TR => "Temporal Reasoning",
            EvalDimension::KU => "Knowledge Update",
            EvalDimension::Abs => "Abstraction",
        }
    }
}

/// Result of a single evaluation run.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    /// Per-dimension scores [0.0, 100.0].
    pub dimension_scores: HashMap<EvalDimension, f32>,
    /// Overall composite score [0.0, 100.0].
    pub overall_score: f32,
    /// Whether the result meets Phase 2 targets.
    pub passed: bool,
}

/// Target thresholds for Phase 2 evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalConfig {
    /// Minimum required overall score.
    pub min_overall: f32,
    /// Minimum required score for each dimension.
    pub min_per_dimension: f32,
    /// Minimum required score for the Abstraction dimension.
    pub min_abs: f32,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            min_overall: 65.0,
            min_per_dimension: 50.0,
            min_abs: 60.0,
        }
    }
}

impl EvalResult {
    /// Evaluate whether this result meets the configured thresholds.
    pub fn check_pass(&self, config: &EvalConfig) -> bool {
        if self.overall_score < config.min_overall {
            return false;
        }
        for dim in [
            EvalDimension::IE,
            EvalDimension::MR,
            EvalDimension::TR,
            EvalDimension::KU,
            EvalDimension::Abs,
        ] {
            let score = self.dimension_scores.get(&dim).copied().unwrap_or(0.0);
            if score < config.min_per_dimension {
                return false;
            }
            if dim == EvalDimension::Abs && score < config.min_abs {
                return false;
            }
        }
        true
    }
}

/// Run evaluation on a set of example test cases (placeholder for Phase 2).
///
/// Returns a synthetic result for framework validation.
pub fn run_eval(_config: &EvalConfig) -> EvalResult {
    let mut scores = HashMap::new();
    scores.insert(EvalDimension::IE, 70.0);
    scores.insert(EvalDimension::MR, 72.0);
    scores.insert(EvalDimension::TR, 60.0);
    scores.insert(EvalDimension::KU, 65.0);
    scores.insert(EvalDimension::Abs, 65.0);

    let overall = scores.values().sum::<f32>() / scores.len() as f32;
    let mut result = EvalResult {
        dimension_scores: scores,
        overall_score: overall,
        passed: false,
    };
    result.passed = result.check_pass(&EvalConfig::default());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_dimension_name() {
        assert_eq!(EvalDimension::IE.name(), "Information Extraction");
        assert_eq!(EvalDimension::MR.name(), "Memory Retrieval");
        assert_eq!(EvalDimension::TR.name(), "Temporal Reasoning");
        assert_eq!(EvalDimension::KU.name(), "Knowledge Update");
        assert_eq!(EvalDimension::Abs.name(), "Abstraction");
    }

    #[test]
    fn test_eval_config_default() {
        let config = EvalConfig::default();
        assert!((config.min_overall - 65.0).abs() < f32::EPSILON);
        assert!((config.min_per_dimension - 50.0).abs() < f32::EPSILON);
        assert!((config.min_abs - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_eval_result_pass() {
        let mut scores = HashMap::new();
        scores.insert(EvalDimension::IE, 70.0);
        scores.insert(EvalDimension::MR, 70.0);
        scores.insert(EvalDimension::TR, 60.0);
        scores.insert(EvalDimension::KU, 60.0);
        scores.insert(EvalDimension::Abs, 65.0);
        let result = EvalResult {
            dimension_scores: scores,
            overall_score: 65.0,
            passed: false,
        };
        assert!(result.check_pass(&EvalConfig::default()));
    }

    #[test]
    fn test_eval_result_fail_overall() {
        let mut scores = HashMap::new();
        scores.insert(EvalDimension::IE, 40.0);
        scores.insert(EvalDimension::MR, 40.0);
        scores.insert(EvalDimension::TR, 40.0);
        scores.insert(EvalDimension::KU, 40.0);
        scores.insert(EvalDimension::Abs, 40.0);
        let result = EvalResult {
            dimension_scores: scores,
            overall_score: 40.0,
            passed: false,
        };
        assert!(!result.check_pass(&EvalConfig::default()));
    }

    #[test]
    fn test_eval_result_fail_abs() {
        let mut scores = HashMap::new();
        scores.insert(EvalDimension::IE, 70.0);
        scores.insert(EvalDimension::MR, 70.0);
        scores.insert(EvalDimension::TR, 60.0);
        scores.insert(EvalDimension::KU, 60.0);
        scores.insert(EvalDimension::Abs, 55.0); // below 60
        let result = EvalResult {
            dimension_scores: scores,
            overall_score: 63.0,
            passed: false,
        };
        assert!(!result.check_pass(&EvalConfig::default()));
    }

    #[test]
    fn test_eval_result_fail_per_dimension() {
        let mut scores = HashMap::new();
        scores.insert(EvalDimension::IE, 70.0);
        scores.insert(EvalDimension::MR, 70.0);
        scores.insert(EvalDimension::TR, 45.0); // below 50
        scores.insert(EvalDimension::KU, 60.0);
        scores.insert(EvalDimension::Abs, 65.0);
        let result = EvalResult {
            dimension_scores: scores,
            overall_score: 62.0,
            passed: false,
        };
        assert!(!result.check_pass(&EvalConfig::default()));
    }

    #[test]
    fn test_run_eval_framework() {
        let config = EvalConfig::default();
        let result = run_eval(&config);
        assert!(result.overall_score > 0.0);
        assert!(!result.dimension_scores.is_empty());
        assert!(result.passed);
    }
}
