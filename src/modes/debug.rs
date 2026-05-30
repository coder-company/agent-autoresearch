//! Bug hunting mode.
//!
//! Systematic debugging through hypothesize → test → falsify → repeat.
//! Tracks findings in a structured format across four phases:
//! gather evidence, hypothesize, test, fix.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Debug investigation phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugPhase {
    /// Gather evidence: symptoms, stack traces, recent changes.
    GatherEvidence,
    /// Form top N ranked hypotheses.
    Hypothesize,
    /// Focused modification to confirm or deny a hypothesis.
    TestHypothesis,
    /// After root cause is found, implement the fix.
    Fix,
}

impl DebugPhase {
    /// All phases in order.
    pub fn all() -> &'static [DebugPhase] {
        &[
            Self::GatherEvidence,
            Self::Hypothesize,
            Self::TestHypothesis,
            Self::Fix,
        ]
    }
}

/// A single hypothesis about the root cause.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    /// Rank (1 = most likely).
    pub rank: u32,
    /// Short description.
    pub description: String,
    /// Supporting evidence.
    pub evidence: Vec<String>,
    /// How to test this hypothesis.
    pub test_plan: String,
    /// Result of testing (None if untested).
    pub result: Option<HypothesisResult>,
}

/// Outcome of testing a hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisResult {
    Confirmed,
    Denied,
    Inconclusive,
}

/// Structured evidence gathered during investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugEvidence {
    /// Symptoms observed.
    pub symptoms: Vec<String>,
    /// Stack traces or error messages.
    pub stack_traces: Vec<String>,
    /// Recent changes (commits) that may relate.
    pub recent_changes: Vec<String>,
    /// Relevant file paths.
    pub relevant_files: Vec<String>,
}

/// Complete debug session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSession {
    /// Current phase.
    pub phase: DebugPhase,
    /// Evidence collected.
    pub evidence: DebugEvidence,
    /// Hypotheses (ranked).
    pub hypotheses: Vec<Hypothesis>,
    /// Root cause description (set when confirmed).
    pub root_cause: Option<String>,
    /// Fix description (set after fix phase).
    pub fix_description: Option<String>,
}

impl DebugSession {
    /// Create a new empty debug session.
    pub fn new() -> Self {
        Self {
            phase: DebugPhase::GatherEvidence,
            evidence: DebugEvidence {
                symptoms: Vec::new(),
                stack_traces: Vec::new(),
                recent_changes: Vec::new(),
                relevant_files: Vec::new(),
            },
            hypotheses: Vec::new(),
            root_cause: None,
            fix_description: None,
        }
    }

    /// Advance to the next phase.
    pub fn advance_phase(&mut self) {
        self.phase = match self.phase {
            DebugPhase::GatherEvidence => DebugPhase::Hypothesize,
            DebugPhase::Hypothesize => DebugPhase::TestHypothesis,
            DebugPhase::TestHypothesis => {
                if self.root_cause.is_some() {
                    DebugPhase::Fix
                } else {
                    // Loop back to hypothesize with new evidence.
                    DebugPhase::Hypothesize
                }
            }
            DebugPhase::Fix => DebugPhase::Fix, // Terminal.
        };
    }
}

impl Default for DebugSession {
    fn default() -> Self {
        Self::new()
    }
}

/// The debug / bug-hunting mode.
#[derive(Debug, Clone, Default)]
pub struct DebugMode;

impl ModeRunner for DebugMode {
    fn name(&self) -> &'static str {
        "debug"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Debug mode requires a goal (bug description)");
        }
        if config.scope.is_empty() {
            bail!("Debug mode requires at least one scope pattern");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "debug",
            purpose: "Bug hunting: hypothesize → test → falsify → repeat until root cause found",
            default_iterations: Some(15),
            required_fields: &["goal", "scope"],
            optional_fields: &["verify", "guard", "iterations"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config(goal: &str, scope: Vec<&str>) -> RunConfig {
        RunConfig {
            goal: goal.into(),
            scope: scope.into_iter().map(String::from).collect(),
            metric: String::new(),
            direction: Direction::Lower,
            verify: String::new(),
            guard: None,
            iterations: Some(15),
            run_tag: None,
            stop_condition: None,
            verify_format: Default::default(),
            primary_metric_key: None,
            acceptance_criteria: Vec::new(),
            required_keep_criteria: Vec::new(),
            required_keep_labels: Vec::new(),
            required_stop_labels: Vec::new(),
            rollback_strategy: Default::default(),
            run_mode: None,
            workspace_root: None,
            primary_repo: None,
        }
    }

    #[test]
    fn test_validate_valid() {
        let mode = DebugMode;
        let config = make_config("Login returns 500", vec!["src/**/*.ts"]);
        assert!(mode.validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_missing_goal() {
        let mode = DebugMode;
        let config = make_config("", vec!["src/**/*.ts"]);
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_missing_scope() {
        let mode = DebugMode;
        let config = make_config("Login returns 500", vec![]);
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_debug_session_phase_advance() {
        let mut session = DebugSession::new();
        assert_eq!(session.phase, DebugPhase::GatherEvidence);

        session.advance_phase();
        assert_eq!(session.phase, DebugPhase::Hypothesize);

        session.advance_phase();
        assert_eq!(session.phase, DebugPhase::TestHypothesis);

        // Without root cause, loops back
        session.advance_phase();
        assert_eq!(session.phase, DebugPhase::Hypothesize);

        // With root cause, advances to fix
        session.root_cause = Some("Null pointer in auth middleware".into());
        session.phase = DebugPhase::TestHypothesis;
        session.advance_phase();
        assert_eq!(session.phase, DebugPhase::Fix);
    }
}
