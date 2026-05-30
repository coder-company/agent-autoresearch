use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::config::{Direction, RunConfig};

/// The state machine for an autoresearch run.
/// Invalid transitions are impossible at the type level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum RunPhase {
    /// Pre-launch: scanning repo, asking questions.
    Setup,
    /// Baseline measured, artifacts initialized, ready to iterate.
    Baseline { metric: Decimal },
    /// Actively iterating.
    Iterating {
        iteration: u32,
        current_metric: Decimal,
        best_metric: Decimal,
        best_iteration: u32,
    },
    /// Run completed (goal reached, cap hit, or user stopped).
    Complete { reason: StopReason },
    /// Run blocked (hard blocker detected).
    Blocked { reason: String },
}

/// Why a run stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    GoalReached,
    IterationCap,
    UserInterrupt,
    SoftBlocker,
    HardBlocker(String),
}

/// Persistent state snapshot (written to state.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    pub iteration: u32,
    pub baseline_metric: Decimal,
    pub best_metric: Decimal,
    pub best_iteration: u32,
    pub current_metric: Decimal,
    pub last_commit: String,
    pub last_trial_commit: Option<String>,
    pub last_trial_metric: Option<Decimal>,
    #[serde(default)]
    pub current_metrics: BTreeMap<String, Decimal>,
    #[serde(default)]
    pub last_trial_metrics: BTreeMap<String, Decimal>,
    #[serde(default)]
    pub current_labels: Vec<String>,
    #[serde(default)]
    pub last_trial_labels: Vec<String>,
    pub keeps: u32,
    pub discards: u32,
    pub crashes: u32,
    pub no_ops: u32,
    pub blocked: u32,
    pub consecutive_discards: u32,
    pub pivot_count: u32,
    pub last_status: IterationStatus,
    pub phase: RunPhase,
    /// Metric optimization direction.
    #[serde(default = "default_direction")]
    pub direction: Direction,
    /// Run configuration for resume support.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RunConfig>,
}

fn default_direction() -> Direction {
    Direction::Higher
}

impl RunState {
    /// Create initial state from baseline measurement.
    pub fn from_baseline(metric: Decimal, commit: String, config: Option<RunConfig>) -> Self {
        let direction = config
            .as_ref()
            .map(|c| c.direction)
            .unwrap_or(Direction::Higher);
        let initial_metrics = initial_metric_map(metric, config.as_ref());
        Self {
            iteration: 0,
            baseline_metric: metric,
            best_metric: metric,
            best_iteration: 0,
            current_metric: metric,
            last_commit: commit,
            last_trial_commit: None,
            last_trial_metric: None,
            current_metrics: initial_metrics.clone(),
            last_trial_metrics: initial_metrics,
            current_labels: Vec::new(),
            last_trial_labels: Vec::new(),
            keeps: 0,
            discards: 0,
            crashes: 0,
            no_ops: 0,
            blocked: 0,
            consecutive_discards: 0,
            pivot_count: 0,
            last_status: IterationStatus::Baseline,
            phase: RunPhase::Baseline { metric },
            direction,
            config,
        }
    }

    /// Replace retained and last-trial metrics with a structured measurement.
    pub fn set_current_metrics(&mut self, metrics: BTreeMap<String, Decimal>) {
        self.current_metrics = metrics.clone();
        self.last_trial_metrics = metrics;
    }

    /// Record a keep decision.
    pub fn record_keep(&mut self, metric: Decimal, commit: String) {
        self.record_keep_with_labels(metric, commit, Vec::new());
    }

    /// Record a keep decision with structured labels.
    pub fn record_keep_with_labels(
        &mut self,
        metric: Decimal,
        commit: String,
        labels: Vec<String>,
    ) {
        self.iteration += 1;
        self.current_metric = metric;
        self.last_commit = commit.clone();
        self.last_trial_commit = Some(commit);
        self.last_trial_metric = Some(metric);
        self.current_labels = labels.clone();
        self.last_trial_labels = labels;
        self.keeps += 1;
        self.consecutive_discards = 0;
        self.last_status = IterationStatus::Keep;

        if self.is_new_best(metric, self.direction) {
            self.best_metric = metric;
            self.best_iteration = self.iteration;
        }

        self.phase = RunPhase::Iterating {
            iteration: self.iteration,
            current_metric: self.current_metric,
            best_metric: self.best_metric,
            best_iteration: self.best_iteration,
        };
    }

    /// Record a keep decision with the full structured metric payload.
    pub fn record_keep_with_metrics(
        &mut self,
        metric: Decimal,
        commit: String,
        metrics: BTreeMap<String, Decimal>,
    ) {
        self.record_keep_with_metrics_and_labels(metric, commit, metrics, Vec::new());
    }

    /// Record a keep decision with the full structured metric payload and labels.
    pub fn record_keep_with_metrics_and_labels(
        &mut self,
        metric: Decimal,
        commit: String,
        metrics: BTreeMap<String, Decimal>,
        labels: Vec<String>,
    ) {
        self.record_keep_with_labels(metric, commit, labels);
        self.set_current_metrics(metrics);
    }

    /// Record a discard decision.
    pub fn record_discard(&mut self, trial_metric: Decimal, trial_commit: Option<String>) {
        self.record_discard_with_labels(trial_metric, trial_commit, Vec::new());
    }

    /// Record a discard decision with structured labels from the failed trial.
    pub fn record_discard_with_labels(
        &mut self,
        trial_metric: Decimal,
        trial_commit: Option<String>,
        labels: Vec<String>,
    ) {
        self.iteration += 1;
        self.last_trial_metric = Some(trial_metric);
        self.last_trial_commit = trial_commit;
        self.last_trial_labels = labels;
        self.discards += 1;
        self.consecutive_discards += 1;
        self.last_status = IterationStatus::Discard;

        self.phase = RunPhase::Iterating {
            iteration: self.iteration,
            current_metric: self.current_metric,
            best_metric: self.best_metric,
            best_iteration: self.best_iteration,
        };
    }

    /// Record a discard decision with the full structured trial metric payload.
    pub fn record_discard_with_metrics(
        &mut self,
        trial_metric: Decimal,
        trial_commit: Option<String>,
        metrics: BTreeMap<String, Decimal>,
    ) {
        self.record_discard_with_metrics_and_labels(
            trial_metric,
            trial_commit,
            metrics,
            Vec::new(),
        );
    }

    /// Record a discard decision with the full structured trial metric payload and labels.
    pub fn record_discard_with_metrics_and_labels(
        &mut self,
        trial_metric: Decimal,
        trial_commit: Option<String>,
        metrics: BTreeMap<String, Decimal>,
        labels: Vec<String>,
    ) {
        self.record_discard_with_labels(trial_metric, trial_commit, labels);
        self.last_trial_metrics = metrics;
    }

    /// Record a crash.
    pub fn record_crash(&mut self) {
        self.iteration += 1;
        self.crashes += 1;
        self.consecutive_discards += 1;
        self.last_status = IterationStatus::Crash;

        self.phase = RunPhase::Iterating {
            iteration: self.iteration,
            current_metric: self.current_metric,
            best_metric: self.best_metric,
            best_iteration: self.best_iteration,
        };
    }

    /// Record a no-op.
    pub fn record_no_op(&mut self) {
        self.iteration += 1;
        self.no_ops += 1;
        self.consecutive_discards += 1;
        self.last_status = IterationStatus::NoOp;

        self.phase = RunPhase::Iterating {
            iteration: self.iteration,
            current_metric: self.current_metric,
            best_metric: self.best_metric,
            best_iteration: self.best_iteration,
        };
    }

    /// Record a block.
    pub fn record_blocked(&mut self, reason: String) {
        self.iteration += 1;
        self.blocked += 1;
        self.last_status = IterationStatus::Blocked;
        self.phase = RunPhase::Blocked { reason };
    }

    /// Record metric drift during resume or health recovery.
    pub fn record_drift(&mut self, metric: Decimal) {
        self.iteration += 1;
        self.current_metric = metric;
        self.last_trial_metric = Some(metric);
        self.last_trial_commit = None;
        self.set_current_metrics(initial_metric_map(metric, self.config.as_ref()));
        self.last_status = IterationStatus::Drift;

        if self.is_new_best(metric, self.direction) {
            self.best_metric = metric;
            self.best_iteration = self.iteration;
        }

        self.phase = RunPhase::Iterating {
            iteration: self.iteration,
            current_metric: self.current_metric,
            best_metric: self.best_metric,
            best_iteration: self.best_iteration,
        };
    }

    /// Record a non-experiment control-plane row such as refine, pivot, or search.
    pub fn record_meta_status(&mut self, status: IterationStatus, metric: Decimal) {
        self.iteration += 1;
        self.last_trial_metric = Some(metric);
        self.last_trial_commit = None;
        self.last_status = status;

        self.phase = RunPhase::Iterating {
            iteration: self.iteration,
            current_metric: self.current_metric,
            best_metric: self.best_metric,
            best_iteration: self.best_iteration,
        };
    }

    /// Mark run complete.
    pub fn complete(&mut self, reason: StopReason) {
        self.phase = RunPhase::Complete { reason };
    }

    /// Reset escalation counters (called on successful keep).
    pub fn reset_escalation(&mut self) {
        self.consecutive_discards = 0;
    }

    fn is_new_best(&self, metric: Decimal, direction: Direction) -> bool {
        match direction {
            Direction::Higher => metric > self.best_metric,
            Direction::Lower => metric < self.best_metric,
        }
    }
}

fn initial_metric_map(metric: Decimal, config: Option<&RunConfig>) -> BTreeMap<String, Decimal> {
    let mut metrics = BTreeMap::from([("metric".to_string(), metric)]);
    if let Some(primary_key) = config
        .and_then(|config| config.primary_metric_key.as_ref())
        .map(|key| key.trim())
        .filter(|key| !key.is_empty())
    {
        metrics.insert(primary_key.to_string(), metric);
    }
    metrics
}

/// Status of a single iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IterationStatus {
    Baseline,
    Keep,
    #[serde(rename = "keep (reworked)")]
    KeepReworked,
    Discard,
    Crash,
    #[serde(rename = "no-op", alias = "noop")]
    NoOp,
    Blocked,
    #[serde(rename = "hook-blocked")]
    HookBlocked,
    #[serde(rename = "metric-error")]
    MetricError,
    Pivot,
    Refine,
    Search,
    Drift,
}

impl IterationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Keep => "keep",
            Self::KeepReworked => "keep (reworked)",
            Self::Discard => "discard",
            Self::Crash => "crash",
            Self::NoOp => "no-op",
            Self::Blocked => "blocked",
            Self::HookBlocked => "hook-blocked",
            Self::MetricError => "metric-error",
            Self::Pivot => "pivot",
            Self::Refine => "refine",
            Self::Search => "search",
            Self::Drift => "drift",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_status_accepts_legacy_noop_alias() {
        let status: IterationStatus = serde_json::from_str("\"noop\"").unwrap();

        assert_eq!(status, IterationStatus::NoOp);
        assert_eq!(serde_json::to_string(&status).unwrap(), "\"no-op\"");
    }

    #[test]
    fn iteration_status_serializes_legacy_result_statuses() {
        assert_eq!(
            serde_json::to_string(&IterationStatus::KeepReworked).unwrap(),
            "\"keep (reworked)\""
        );
        assert_eq!(
            serde_json::to_string(&IterationStatus::HookBlocked).unwrap(),
            "\"hook-blocked\""
        );
        assert_eq!(
            serde_json::to_string(&IterationStatus::MetricError).unwrap(),
            "\"metric-error\""
        );
    }
}
