//! Core iteration loop mode.
//!
//! The main autoresearch loop: modify → verify → keep/discard → repeat.
//! Validates that all required config is present and generates a /goal
//! condition string for bounded/unbounded execution.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::{Direction, RunConfig};

use super::{ModeDescription, ModeRunner};

/// Phases of the iteration protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopPhase {
    /// Read context: git log, results TSV, lessons.
    ReadContext,
    /// Plan a single atomic change.
    Plan,
    /// Execute the change within scope.
    Execute,
    /// Run verify command and parse metric.
    Verify,
    /// Decide: keep (improvement) or discard (regression/no-op).
    Decide,
    /// Run guard command if configured.
    Guard,
    /// Commit or rollback based on decision.
    Commit,
    /// Log results and check stop conditions.
    Log,
}

impl LoopPhase {
    /// All phases in execution order.
    pub fn all() -> &'static [LoopPhase] {
        &[
            Self::ReadContext,
            Self::Plan,
            Self::Execute,
            Self::Verify,
            Self::Decide,
            Self::Guard,
            Self::Commit,
            Self::Log,
        ]
    }
}

/// The core iteration loop mode.
#[derive(Debug, Clone, Default)]
pub struct LoopMode;

impl LoopMode {
    /// Generate the /goal condition string from config.
    pub fn goal_condition(config: &RunConfig) -> String {
        let direction_verb = match config.direction {
            Direction::Higher => "increase",
            Direction::Lower => "decrease",
        };
        let bounds = match config.iterations {
            Some(n) => format!(" (max {n} iterations)"),
            None => " (unbounded)".to_string(),
        };
        format!(
            "Goal: {} metric '{}' via: {}{bounds}",
            direction_verb, config.metric, config.goal
        )
    }

    /// Check whether the run should be bounded or unbounded.
    pub fn is_bounded(config: &RunConfig) -> bool {
        config.is_bounded()
    }
}

impl ModeRunner for LoopMode {
    fn name(&self) -> &'static str {
        "loop"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Loop mode requires a goal");
        }
        if config.scope.is_empty() {
            bail!("Loop mode requires at least one scope pattern");
        }
        if config.verify.is_empty() {
            bail!("Loop mode requires a verify command");
        }
        if config.metric.is_empty() {
            bail!("Loop mode requires a metric name");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "loop",
            purpose: "Core iteration loop: modify → verify → keep/discard → repeat",
            default_iterations: Some(25),
            required_fields: &["goal", "scope", "verify", "metric", "direction"],
            optional_fields: &[
                "guard",
                "iterations",
                "stop_condition",
                "verify_format",
                "rollback_strategy",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_valid_config() -> RunConfig {
        RunConfig {
            goal: "Increase test coverage to 90%".into(),
            scope: vec!["src/**/*.ts".into()],
            metric: "coverage".into(),
            direction: Direction::Higher,
            verify: "npm test -- --coverage | tail -1".into(),
            guard: None,
            iterations: Some(25),
            run_tag: None,
            stop_condition: None,
            verify_format: Default::default(),
            primary_metric_key: None,
            acceptance_criteria: Vec::new(),
            required_keep_criteria: Vec::new(),
            rollback_strategy: Default::default(),
            run_mode: None,
            workspace_root: None,
            primary_repo: None,
        }
    }

    #[test]
    fn test_validate_valid_config() {
        let mode = LoopMode;
        let config = make_valid_config();
        assert!(mode.validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_missing_goal() {
        let mode = LoopMode;
        let mut config = make_valid_config();
        config.goal = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_missing_scope() {
        let mode = LoopMode;
        let mut config = make_valid_config();
        config.scope = vec![];
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_missing_verify() {
        let mode = LoopMode;
        let mut config = make_valid_config();
        config.verify = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_goal_condition_bounded() {
        let config = make_valid_config();
        let cond = LoopMode::goal_condition(&config);
        assert!(cond.contains("increase"));
        assert!(cond.contains("coverage"));
        assert!(cond.contains("max 25 iterations"));
    }

    #[test]
    fn test_goal_condition_unbounded() {
        let mut config = make_valid_config();
        config.iterations = None;
        let cond = LoopMode::goal_condition(&config);
        assert!(cond.contains("unbounded"));
    }

    #[test]
    fn test_loop_phases_order() {
        let phases = LoopPhase::all();
        assert_eq!(phases[0], LoopPhase::ReadContext);
        assert_eq!(phases[phases.len() - 1], LoopPhase::Log);
    }
}
