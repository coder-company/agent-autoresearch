//! Product improvement engine mode.
//!
//! Research ICP (Ideal Customer Profile) challenges, score and rank
//! improvements by impact, and generate a structured improvement plan.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Improvement priority tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementTier {
    /// Critical must-have — blocks adoption or retention.
    MustHave = 0,
    /// Nice-to-have — improves experience significantly.
    NiceToHave = 1,
    /// Moonshot — differentiating, high-risk/high-reward.
    Moonshot = 2,
}

impl ImprovementTier {
    pub fn label(&self) -> &'static str {
        match self {
            Self::MustHave => "Must-Have",
            Self::NiceToHave => "Nice-to-Have",
            Self::Moonshot => "Moonshot",
        }
    }
}

/// Impact score dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactScore {
    /// User impact (0-10).
    pub user_impact: u8,
    /// Business impact (0-10).
    pub business_impact: u8,
    /// Implementation effort (0-10, lower = easier).
    pub effort: u8,
    /// Confidence in the estimate (0-10).
    pub confidence: u8,
}

impl ImpactScore {
    /// Composite score: (user_impact + business_impact) * confidence / effort.
    /// Higher is better.
    pub fn composite(&self) -> f64 {
        let impact = (self.user_impact as f64 + self.business_impact as f64) / 2.0;
        let effort = (self.effort as f64).max(1.0);
        let confidence = self.confidence as f64 / 10.0;
        impact * confidence / effort * 10.0
    }
}

/// A single improvement item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    /// Short title.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// Which tier this improvement belongs to.
    pub tier: ImprovementTier,
    /// Impact scoring.
    pub score: ImpactScore,
    /// Evidence or research supporting this improvement.
    pub evidence: Vec<String>,
    /// Specific ICP challenge this addresses.
    pub icp_challenge: Option<String>,
    /// Relevant files or areas of the codebase.
    pub relevant_areas: Vec<String>,
}

/// ICP (Ideal Customer Profile) challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcpChallenge {
    /// Description of the challenge.
    pub description: String,
    /// How painful this is (0-10).
    pub pain_level: u8,
    /// Frequency (how often encountered).
    pub frequency: String,
    /// Current workaround, if any.
    pub workaround: Option<String>,
}

/// Improvement session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImproveSession {
    /// Product/feature being improved.
    pub subject: String,
    /// ICP challenges identified.
    pub challenges: Vec<IcpChallenge>,
    /// Improvements discovered and ranked.
    pub improvements: Vec<Improvement>,
}

impl ImproveSession {
    /// Create a new improvement session.
    pub fn new(subject: String) -> Self {
        Self {
            subject,
            challenges: Vec::new(),
            improvements: Vec::new(),
        }
    }

    /// Get improvements sorted by composite score (highest first).
    pub fn ranked_improvements(&self) -> Vec<&Improvement> {
        let mut sorted: Vec<&Improvement> = self.improvements.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .composite()
                .partial_cmp(&a.score.composite())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    }

    /// Filter improvements by tier.
    pub fn by_tier(&self, tier: ImprovementTier) -> Vec<&Improvement> {
        self.improvements
            .iter()
            .filter(|i| i.tier == tier)
            .collect()
    }

    /// Count improvements per tier.
    pub fn count_by_tier(&self, tier: ImprovementTier) -> usize {
        self.improvements.iter().filter(|i| i.tier == tier).count()
    }
}

/// The product improvement engine mode.
#[derive(Debug, Clone, Default)]
pub struct ImproveMode;

impl ModeRunner for ImproveMode {
    fn name(&self) -> &'static str {
        "improve"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Improve mode requires a goal (product/feature to improve)");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "improve",
            purpose: "Product improvement: research ICP challenges, score and rank improvements",
            default_iterations: None,
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
            goal: "Improve onboarding flow".into(),
            scope: vec!["src/onboarding/**".into()],
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
        let mode = ImproveMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_goal() {
        let mode = ImproveMode;
        let mut config = make_config();
        config.goal = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_impact_composite_score() {
        let score = ImpactScore {
            user_impact: 9,
            business_impact: 7,
            effort: 2,
            confidence: 8,
        };
        let composite = score.composite();
        // (9+7)/2 * 0.8 / 2 * 10 = 8 * 0.8 / 2 * 10 = 32.0
        assert!(composite > 30.0 && composite < 33.0);
    }

    #[test]
    fn test_tier_ordering() {
        assert!(ImprovementTier::MustHave < ImprovementTier::NiceToHave);
        assert!(ImprovementTier::NiceToHave < ImprovementTier::Moonshot);
    }

    #[test]
    fn test_ranked_improvements() {
        let mut session = ImproveSession::new("Test".into());
        session.improvements.push(Improvement {
            title: "Low score".into(),
            description: "Low".into(),
            tier: ImprovementTier::Moonshot,
            score: ImpactScore {
                user_impact: 2,
                business_impact: 2,
                effort: 8,
                confidence: 5,
            },
            evidence: vec![],
            icp_challenge: None,
            relevant_areas: vec![],
        });
        session.improvements.push(Improvement {
            title: "High score".into(),
            description: "High".into(),
            tier: ImprovementTier::MustHave,
            score: ImpactScore {
                user_impact: 9,
                business_impact: 9,
                effort: 1,
                confidence: 9,
            },
            evidence: vec![],
            icp_challenge: None,
            relevant_areas: vec![],
        });

        let ranked = session.ranked_improvements();
        assert_eq!(ranked[0].title, "High score");
        assert_eq!(ranked[1].title, "Low score");
    }
}
