//! Adversarial debate mode.
//!
//! Generates competing candidate solutions, judged by a panel.
//! Convergence detection stops when N consecutive rounds have the
//! same winner. Supports convergent, creative, and debate modes.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Reasoning mode variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    /// Converge on the single best answer.
    Convergent,
    /// Explore divergent creative solutions.
    Creative,
    /// Structured adversarial debate.
    Debate,
}

/// Domain context for the reasoning session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonDomain {
    Software,
    Product,
    Business,
    Security,
    Research,
    Content,
}

impl ReasonDomain {
    /// All domains.
    pub fn all() -> &'static [ReasonDomain] {
        &[
            Self::Software,
            Self::Product,
            Self::Business,
            Self::Security,
            Self::Research,
            Self::Content,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Software => "Software Engineering",
            Self::Product => "Product Design",
            Self::Business => "Business Strategy",
            Self::Security => "Security",
            Self::Research => "Research",
            Self::Content => "Content & Writing",
        }
    }
}

/// A candidate solution in the debate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Unique identifier within this session.
    pub id: u32,
    /// Short title.
    pub title: String,
    /// Full description of the approach.
    pub description: String,
    /// Strengths identified.
    pub strengths: Vec<String>,
    /// Weaknesses identified.
    pub weaknesses: Vec<String>,
    /// Win count across rounds.
    pub wins: u32,
}

/// A judge's verdict for a single round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    /// Round number.
    pub round: u32,
    /// Winning candidate ID.
    pub winner_id: u32,
    /// Rationale for the choice.
    pub rationale: String,
    /// Confidence (0-10).
    pub confidence: u8,
}

/// Reason session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonSession {
    /// The question or problem being debated.
    pub question: String,
    /// Active reasoning mode.
    pub mode: ReasoningMode,
    /// Domain context.
    pub domain: ReasonDomain,
    /// Judge panel size (odd preferred).
    pub panel_size: u32,
    /// Convergence threshold (N consecutive wins to declare winner).
    pub convergence_threshold: u32,
    /// Competing candidates.
    pub candidates: Vec<Candidate>,
    /// Verdicts from each round.
    pub verdicts: Vec<Verdict>,
    /// Final winner (None until convergence).
    pub winner: Option<u32>,
}

impl ReasonSession {
    /// Create a new reason session.
    pub fn new(
        question: String,
        mode: ReasoningMode,
        domain: ReasonDomain,
        panel_size: u32,
        convergence_threshold: u32,
    ) -> Self {
        Self {
            question,
            mode,
            domain,
            panel_size,
            convergence_threshold,
            candidates: Vec::new(),
            verdicts: Vec::new(),
            winner: None,
        }
    }

    /// Check if convergence has been reached.
    pub fn check_convergence(&mut self) -> bool {
        if self.verdicts.len() < self.convergence_threshold as usize {
            return false;
        }

        let threshold = self.convergence_threshold as usize;
        let recent: Vec<u32> = self
            .verdicts
            .iter()
            .rev()
            .take(threshold)
            .map(|v| v.winner_id)
            .collect();

        if recent.len() == threshold && recent.iter().all(|&id| id == recent[0]) {
            self.winner = Some(recent[0]);
            true
        } else {
            false
        }
    }

    /// Current round number.
    pub fn current_round(&self) -> u32 {
        self.verdicts.len() as u32 + 1
    }

    /// Is the debate concluded?
    pub fn is_concluded(&self) -> bool {
        self.winner.is_some()
    }
}

/// The adversarial debate mode.
#[derive(Debug, Clone, Default)]
pub struct ReasonMode;

impl ModeRunner for ReasonMode {
    fn name(&self) -> &'static str {
        "reason"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Reason mode requires a goal (question to debate)");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "reason",
            purpose: "Adversarial debate: competing candidates, judge panel, convergence detection",
            default_iterations: Some(8),
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
            goal: "Best database for this use case".into(),
            scope: vec![],
            metric: String::new(),
            direction: Direction::Higher,
            verify: String::new(),
            guard: None,
            iterations: Some(8),
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
        let mode = ReasonMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_goal() {
        let mode = ReasonMode;
        let mut config = make_config();
        config.goal = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_convergence_detection() {
        let mut session = ReasonSession::new(
            "Which DB?".into(),
            ReasoningMode::Convergent,
            ReasonDomain::Software,
            3,
            3,
        );

        // Add 3 verdicts for the same winner.
        for round in 1..=3 {
            session.verdicts.push(Verdict {
                round,
                winner_id: 1,
                rationale: "Better fit".into(),
                confidence: 8,
            });
        }

        assert!(session.check_convergence());
        assert_eq!(session.winner, Some(1));
    }

    #[test]
    fn test_no_convergence_with_mixed_winners() {
        let mut session = ReasonSession::new(
            "Which DB?".into(),
            ReasoningMode::Debate,
            ReasonDomain::Software,
            3,
            3,
        );

        session.verdicts.push(Verdict {
            round: 1,
            winner_id: 1,
            rationale: "A".into(),
            confidence: 7,
        });
        session.verdicts.push(Verdict {
            round: 2,
            winner_id: 2,
            rationale: "B".into(),
            confidence: 6,
        });
        session.verdicts.push(Verdict {
            round: 3,
            winner_id: 1,
            rationale: "A".into(),
            confidence: 8,
        });

        assert!(!session.check_convergence());
        assert!(session.winner.is_none());
    }

    #[test]
    fn test_domains_count() {
        assert_eq!(ReasonDomain::all().len(), 6);
    }
}
