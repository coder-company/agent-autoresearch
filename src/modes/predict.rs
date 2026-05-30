//! Multi-persona expert debate mode.
//!
//! Five personas analyze a proposal from their perspective. Results
//! are synthesized into areas of agreement, disagreement, and
//! recommendations. One-shot (no iteration loop).

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Expert persona for analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Persona {
    Architect,
    SecurityExpert,
    PerformanceEngineer,
    UxDesigner,
    DevilsAdvocate,
}

impl Persona {
    /// All 5 personas.
    pub fn all() -> &'static [Persona] {
        &[
            Self::Architect,
            Self::SecurityExpert,
            Self::PerformanceEngineer,
            Self::UxDesigner,
            Self::DevilsAdvocate,
        ]
    }

    /// Human-readable title.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Architect => "Software Architect",
            Self::SecurityExpert => "Security Expert",
            Self::PerformanceEngineer => "Performance Engineer",
            Self::UxDesigner => "UX Designer",
            Self::DevilsAdvocate => "Devil's Advocate",
        }
    }

    /// What this persona focuses on.
    pub fn focus(&self) -> &'static str {
        match self {
            Self::Architect => "System design, modularity, extensibility, maintainability",
            Self::SecurityExpert => "Attack surface, trust boundaries, data protection",
            Self::PerformanceEngineer => "Latency, throughput, resource usage, scalability",
            Self::UxDesigner => "User experience, accessibility, cognitive load, delight",
            Self::DevilsAdvocate => {
                "Failure modes, worst cases, hidden assumptions, what could go wrong"
            }
        }
    }
}

/// A single persona's analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaAnalysis {
    /// Which persona provided this analysis.
    pub persona: Persona,
    /// Key observations from this perspective.
    pub observations: Vec<String>,
    /// Risks identified.
    pub risks: Vec<String>,
    /// Recommendations.
    pub recommendations: Vec<String>,
    /// Confidence score (0-10).
    pub confidence: u8,
}

/// Synthesis of all persona analyses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictSynthesis {
    /// Points where all/most personas agree.
    pub agreements: Vec<String>,
    /// Points of disagreement between personas.
    pub disagreements: Vec<String>,
    /// Combined recommendations (priority ordered).
    pub recommendations: Vec<String>,
    /// Overall risk level.
    pub overall_risk: RiskLevel,
}

/// Risk level assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Predict session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictSession {
    /// The proposal being analyzed.
    pub proposal: String,
    /// Individual persona analyses.
    pub analyses: Vec<PersonaAnalysis>,
    /// Final synthesis (None until all analyses are complete).
    pub synthesis: Option<PredictSynthesis>,
}

impl PredictSession {
    /// Create a new predict session.
    pub fn new(proposal: String) -> Self {
        Self {
            proposal,
            analyses: Vec::new(),
            synthesis: None,
        }
    }

    /// Check if all personas have analyzed.
    pub fn is_analysis_complete(&self) -> bool {
        Persona::all()
            .iter()
            .all(|p| self.analyses.iter().any(|a| a.persona == *p))
    }

    /// Get personas that haven't analyzed yet.
    pub fn remaining_personas(&self) -> Vec<Persona> {
        Persona::all()
            .iter()
            .filter(|p| !self.analyses.iter().any(|a| &a.persona == *p))
            .copied()
            .collect()
    }
}

/// The multi-persona analysis mode.
#[derive(Debug, Clone, Default)]
pub struct PredictMode;

impl ModeRunner for PredictMode {
    fn name(&self) -> &'static str {
        "predict"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Predict mode requires a goal (proposal to analyze)");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "predict",
            purpose: "Multi-persona expert debate: 5 perspectives analyze, then synthesize",
            default_iterations: None,
            required_fields: &["goal"],
            optional_fields: &["scope"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config() -> RunConfig {
        RunConfig {
            goal: "Migrate from REST to GraphQL".into(),
            scope: vec!["src/api/**".into()],
            metric: String::new(),
            direction: Direction::Higher,
            verify: String::new(),
            guard: None,
            iterations: None,
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
    fn test_validate_valid() {
        let mode = PredictMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_goal() {
        let mode = PredictMode;
        let mut config = make_config();
        config.goal = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_five_personas() {
        assert_eq!(Persona::all().len(), 5);
    }

    #[test]
    fn test_session_remaining_personas() {
        let mut session = PredictSession::new("Test proposal".into());
        assert_eq!(session.remaining_personas().len(), 5);

        session.analyses.push(PersonaAnalysis {
            persona: Persona::Architect,
            observations: vec!["Good modularity".into()],
            risks: vec![],
            recommendations: vec![],
            confidence: 8,
        });
        assert_eq!(session.remaining_personas().len(), 4);
        assert!(!session.is_analysis_complete());
    }

    #[test]
    fn test_session_complete() {
        let mut session = PredictSession::new("Test".into());
        for persona in Persona::all() {
            session.analyses.push(PersonaAnalysis {
                persona: *persona,
                observations: vec![],
                risks: vec![],
                recommendations: vec![],
                confidence: 7,
            });
        }
        assert!(session.is_analysis_complete());
    }
}
