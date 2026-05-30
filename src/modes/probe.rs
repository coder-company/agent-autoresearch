//! Requirement interrogation mode.
//!
//! 8 personas probe requirements from different angles until constraint
//! saturation is detected. Emits an autoresearch-ready RunConfig on completion.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Interrogation persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbePersona {
    EndUser,
    EdgeCaseHunter,
    SecurityAnalyst,
    PerformanceTester,
    BusinessAnalyst,
    Skeptic,
    ComplianceOfficer,
    DevOpsEngineer,
}

impl ProbePersona {
    /// All 8 personas.
    pub fn all() -> &'static [ProbePersona] {
        &[
            Self::EndUser,
            Self::EdgeCaseHunter,
            Self::SecurityAnalyst,
            Self::PerformanceTester,
            Self::BusinessAnalyst,
            Self::Skeptic,
            Self::ComplianceOfficer,
            Self::DevOpsEngineer,
        ]
    }

    /// Human-readable title.
    pub fn title(&self) -> &'static str {
        match self {
            Self::EndUser => "End User",
            Self::EdgeCaseHunter => "Edge Case Hunter",
            Self::SecurityAnalyst => "Security Analyst",
            Self::PerformanceTester => "Performance Tester",
            Self::BusinessAnalyst => "Business Analyst",
            Self::Skeptic => "Skeptic",
            Self::ComplianceOfficer => "Compliance Officer",
            Self::DevOpsEngineer => "DevOps Engineer",
        }
    }

    /// What this persona focuses on.
    pub fn focus(&self) -> &'static str {
        match self {
            Self::EndUser => "Usability, workflows, expectations, frustrations",
            Self::EdgeCaseHunter => "Unusual inputs, corner cases, boundary conditions",
            Self::SecurityAnalyst => "Attack vectors, data exposure, trust assumptions",
            Self::PerformanceTester => "Load, latency, throughput, resource limits",
            Self::BusinessAnalyst => "ROI, priorities, constraints, stakeholder needs",
            Self::Skeptic => "Assumptions, hidden complexity, scope creep, feasibility",
            Self::ComplianceOfficer => "Regulations, data privacy, audit trails, consent",
            Self::DevOpsEngineer => "Deployment, monitoring, rollback, infrastructure",
        }
    }
}

/// A constraint or requirement extracted during interrogation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Which persona surfaced this constraint.
    pub source: ProbePersona,
    /// The constraint itself.
    pub description: String,
    /// Priority (1 = highest).
    pub priority: u8,
    /// Whether this was already known before probing.
    pub was_known: bool,
}

/// A question asked by a persona.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeQuestion {
    /// Which persona asks.
    pub persona: ProbePersona,
    /// The question.
    pub question: String,
    /// Answer (None if unanswered).
    pub answer: Option<String>,
    /// Constraints extracted from the answer.
    pub extracted_constraints: Vec<Constraint>,
}

/// Probe session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeSession {
    /// The requirement/feature being interrogated.
    pub subject: String,
    /// All questions asked.
    pub questions: Vec<ProbeQuestion>,
    /// All constraints extracted.
    pub constraints: Vec<Constraint>,
    /// Saturation tracker: new constraints found per round.
    pub constraints_per_round: Vec<u32>,
    /// Whether saturation has been detected.
    pub saturated: bool,
}

impl ProbeSession {
    /// Create a new probe session.
    pub fn new(subject: String) -> Self {
        Self {
            subject,
            questions: Vec::new(),
            constraints: Vec::new(),
            constraints_per_round: Vec::new(),
            saturated: false,
        }
    }

    /// Record a round's constraint count and check saturation.
    /// Saturation = 2 consecutive rounds with 0 new constraints.
    pub fn record_round(&mut self, new_constraints: u32) {
        self.constraints_per_round.push(new_constraints);
        self.check_saturation();
    }

    /// Check if constraint saturation has been reached.
    fn check_saturation(&mut self) {
        let len = self.constraints_per_round.len();
        if len >= 2 {
            let last_two = &self.constraints_per_round[len - 2..];
            if last_two[0] == 0 && last_two[1] == 0 {
                self.saturated = true;
            }
        }
    }

    /// Total unique constraints found.
    pub fn total_constraints(&self) -> usize {
        self.constraints.len()
    }

    /// Personas that haven't asked questions yet.
    pub fn unused_personas(&self) -> Vec<ProbePersona> {
        ProbePersona::all()
            .iter()
            .filter(|p| !self.questions.iter().any(|q| q.persona == **p))
            .copied()
            .collect()
    }
}

/// The requirement interrogation mode.
#[derive(Debug, Clone, Default)]
pub struct ProbeMode;

impl ModeRunner for ProbeMode {
    fn name(&self) -> &'static str {
        "probe"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Probe mode requires a goal (requirement or feature to interrogate)");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "probe",
            purpose: "Requirement interrogation: 8 personas probe until constraint saturation",
            default_iterations: Some(15),
            required_fields: &["goal"],
            optional_fields: &["scope", "iterations"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config() -> RunConfig {
        RunConfig {
            goal: "User authentication system".into(),
            scope: vec![],
            metric: String::new(),
            direction: Direction::Higher,
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
            companion_repos: Vec::new(),
        }
    }

    #[test]
    fn test_validate_valid() {
        let mode = ProbeMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_goal() {
        let mode = ProbeMode;
        let mut config = make_config();
        config.goal = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_eight_personas() {
        assert_eq!(ProbePersona::all().len(), 8);
    }

    #[test]
    fn test_saturation_detection() {
        let mut session = ProbeSession::new("Auth".into());
        session.record_round(5);
        assert!(!session.saturated);
        session.record_round(2);
        assert!(!session.saturated);
        session.record_round(0);
        assert!(!session.saturated);
        session.record_round(0);
        assert!(session.saturated);
    }

    #[test]
    fn test_no_saturation_if_still_finding() {
        let mut session = ProbeSession::new("Auth".into());
        session.record_round(0);
        session.record_round(1);
        assert!(!session.saturated);
    }

    #[test]
    fn test_unused_personas() {
        let mut session = ProbeSession::new("Auth".into());
        assert_eq!(session.unused_personas().len(), 8);
        session.questions.push(ProbeQuestion {
            persona: ProbePersona::EndUser,
            question: "How do they log in?".into(),
            answer: None,
            extracted_constraints: vec![],
        });
        assert_eq!(session.unused_personas().len(), 7);
    }
}
