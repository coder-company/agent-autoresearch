use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Direction of metric optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[serde(alias = "higher_is_better")]
    Higher,
    #[serde(alias = "lower_is_better")]
    Lower,
}

impl Direction {
    pub fn is_improvement(&self, delta: rust_decimal::Decimal) -> bool {
        use rust_decimal::Decimal;
        match self {
            Direction::Higher => delta > Decimal::ZERO,
            Direction::Lower => delta < Decimal::ZERO,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Higher => "higher",
            Direction::Lower => "lower",
        }
    }
}

/// Run mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Foreground,
    Background,
}

/// Verify output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerifyFormat {
    #[default]
    Scalar,
    MetricsJson,
}

/// Rollback strategy approved during setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RollbackStrategy {
    /// `git reset --hard HEAD~1` — only for dedicated experiment branches.
    HardReset,
    /// `git revert --no-edit HEAD` — safe for shared branches.
    #[default]
    Revert,
}

/// A numeric metric criterion used for acceptance and required keep gates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricCriterion {
    pub metric_key: String,
    pub operator: String,
    pub target: rust_decimal::Decimal,
}

/// A repository managed by a run, including its editable scope and role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoTargetConfig {
    pub path: PathBuf,
    pub scope: String,
    pub role: String,
}

/// Complete run configuration for the autoresearch loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    pub goal: String,
    pub scope: Vec<String>,
    pub metric: String,
    pub direction: Direction,
    pub verify: String,
    #[serde(default)]
    pub guard: Option<String>,
    #[serde(default)]
    pub iterations: Option<u32>,
    #[serde(default)]
    pub run_tag: Option<String>,
    #[serde(default)]
    pub stop_condition: Option<String>,
    #[serde(default)]
    pub verify_format: VerifyFormat,
    #[serde(default)]
    pub primary_metric_key: Option<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<MetricCriterion>,
    #[serde(default)]
    pub required_keep_criteria: Vec<MetricCriterion>,
    #[serde(default)]
    pub required_keep_labels: Vec<String>,
    #[serde(default)]
    pub required_stop_labels: Vec<String>,
    #[serde(default)]
    pub rollback_strategy: RollbackStrategy,
    #[serde(default)]
    pub run_mode: Option<RunMode>,
    #[serde(default)]
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub primary_repo: Option<PathBuf>,
    #[serde(default)]
    pub companion_repos: Vec<RepoTargetConfig>,
}

impl RunConfig {
    /// Returns true if this is a bounded run.
    pub fn is_bounded(&self) -> bool {
        self.iterations.is_some()
    }

    /// Default iteration count when not specified.
    pub fn effective_iterations(&self) -> Option<u32> {
        self.iterations
    }
}

/// Mode-specific configuration that extends base RunConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Loop,
    Plan,
    Debug,
    Fix,
    Security,
    Ship,
    Scenario,
    Predict,
    Learn,
    Reason,
    Probe,
    Improve,
    Evals,
    Exec,
}

impl Mode {
    pub fn default_iterations(&self) -> Option<u32> {
        match self {
            Mode::Loop => Some(25),
            Mode::Debug => Some(15),
            Mode::Fix => Some(20),
            Mode::Security => Some(15),
            Mode::Scenario => Some(20),
            Mode::Improve => Some(20),
            Mode::Learn => Some(10),
            Mode::Reason => Some(8),
            Mode::Probe => Some(15),
            Mode::Plan | Mode::Ship | Mode::Predict | Mode::Evals | Mode::Exec => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Loop => "loop",
            Mode::Plan => "plan",
            Mode::Debug => "debug",
            Mode::Fix => "fix",
            Mode::Security => "security",
            Mode::Ship => "ship",
            Mode::Scenario => "scenario",
            Mode::Predict => "predict",
            Mode::Learn => "learn",
            Mode::Reason => "reason",
            Mode::Probe => "probe",
            Mode::Improve => "improve",
            Mode::Evals => "evals",
            Mode::Exec => "exec",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;

    #[test]
    fn mode_default_iterations_match_command_defaults() {
        assert_eq!(Mode::Loop.default_iterations(), Some(25));
        assert_eq!(Mode::Debug.default_iterations(), Some(15));
        assert_eq!(Mode::Fix.default_iterations(), Some(20));
        assert_eq!(Mode::Security.default_iterations(), Some(15));
        assert_eq!(Mode::Scenario.default_iterations(), Some(20));
        assert_eq!(Mode::Improve.default_iterations(), Some(20));
        assert_eq!(Mode::Learn.default_iterations(), Some(10));
        assert_eq!(Mode::Reason.default_iterations(), Some(8));
        assert_eq!(Mode::Probe.default_iterations(), Some(15));

        assert_eq!(Mode::Plan.default_iterations(), None);
        assert_eq!(Mode::Ship.default_iterations(), None);
        assert_eq!(Mode::Predict.default_iterations(), None);
        assert_eq!(Mode::Evals.default_iterations(), None);
        assert_eq!(Mode::Exec.default_iterations(), None);
    }

    #[test]
    fn mode_catalog_includes_improve() {
        assert_eq!(Mode::Improve.as_str(), "improve");
    }
}
