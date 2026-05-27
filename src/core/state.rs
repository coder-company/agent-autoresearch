use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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
        Self {
            iteration: 0,
            baseline_metric: metric,
            best_metric: metric,
            best_iteration: 0,
            current_metric: metric,
            last_commit: commit,
            last_trial_commit: None,
            last_trial_metric: None,
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

    /// Record a keep decision.
    pub fn record_keep(&mut self, metric: Decimal, commit: String) {
        self.iteration += 1;
        self.current_metric = metric;
        self.last_commit = commit.clone();
        self.last_trial_commit = Some(commit);
        self.last_trial_metric = Some(metric);
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

    /// Record a discard decision.
    pub fn record_discard(&mut self, trial_metric: Decimal, trial_commit: Option<String>) {
        self.iteration += 1;
        self.last_trial_metric = Some(trial_metric);
        self.last_trial_commit = trial_commit;
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

/// Status of a single iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IterationStatus {
    Baseline,
    Keep,
    Discard,
    Crash,
    NoOp,
    Blocked,
    Pivot,
    Refine,
    Search,
}

impl IterationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Keep => "keep",
            Self::Discard => "discard",
            Self::Crash => "crash",
            Self::NoOp => "no-op",
            Self::Blocked => "blocked",
            Self::Pivot => "pivot",
            Self::Refine => "refine",
            Self::Search => "search",
        }
    }
}
