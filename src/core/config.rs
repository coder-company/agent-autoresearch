use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Direction of metric optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Higher,
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
    pub rollback_strategy: RollbackStrategy,
    #[serde(default)]
    pub run_mode: Option<RunMode>,
    #[serde(default)]
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub primary_repo: Option<PathBuf>,
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
    Evals,
    Exec,
}

impl Mode {
    pub fn default_iterations(&self) -> Option<u32> {
        match self {
            Mode::Loop => Some(500),
            Mode::Debug => Some(500),
            Mode::Fix => Some(500),
            Mode::Security => Some(500),
            Mode::Scenario => Some(500),
            Mode::Learn => Some(500),
            Mode::Reason => Some(500),
            Mode::Probe => Some(500),
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
            Mode::Evals => "evals",
            Mode::Exec => "exec",
        }
    }
}
