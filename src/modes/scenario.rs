//! Edge case exploration mode.
//!
//! Generates scenarios across 12 dimensions of potential failure.
//! Outputs structured scenarios in multiple formats: use-cases,
//! user-stories, test-scenarios, threat-scenarios.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// The 12 dimensions of edge case exploration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    BoundaryValues,
    NullEmpty,
    Concurrency,
    Timing,
    Scale,
    Permissions,
    Network,
    DataCorruption,
    StateTransitions,
    Encoding,
    ResourceExhaustion,
    Dependencies,
}

impl Dimension {
    /// All 12 dimensions.
    pub fn all() -> &'static [Dimension] {
        &[
            Self::BoundaryValues,
            Self::NullEmpty,
            Self::Concurrency,
            Self::Timing,
            Self::Scale,
            Self::Permissions,
            Self::Network,
            Self::DataCorruption,
            Self::StateTransitions,
            Self::Encoding,
            Self::ResourceExhaustion,
            Self::Dependencies,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BoundaryValues => "Boundary Values",
            Self::NullEmpty => "Null/Empty",
            Self::Concurrency => "Concurrency",
            Self::Timing => "Timing",
            Self::Scale => "Scale",
            Self::Permissions => "Permissions",
            Self::Network => "Network",
            Self::DataCorruption => "Data Corruption",
            Self::StateTransitions => "State Transitions",
            Self::Encoding => "Encoding",
            Self::ResourceExhaustion => "Resource Exhaustion",
            Self::Dependencies => "Dependencies",
        }
    }

    /// Description of what this dimension explores.
    pub fn description(&self) -> &'static str {
        match self {
            Self::BoundaryValues => "Min/max values, off-by-one, overflow, underflow",
            Self::NullEmpty => "Null references, empty strings, empty collections, missing fields",
            Self::Concurrency => "Race conditions, deadlocks, thread safety, atomicity",
            Self::Timing => "Timeouts, ordering, clock skew, slow operations",
            Self::Scale => "Large inputs, many users, high throughput, data volume",
            Self::Permissions => "Missing auth, wrong role, expired token, cross-tenant",
            Self::Network => "Disconnection, partial response, DNS failure, TLS errors",
            Self::DataCorruption => "Malformed input, truncated data, encoding errors, bit rot",
            Self::StateTransitions => "Invalid state, re-entrant calls, interrupted operations",
            Self::Encoding => "Unicode, UTF-8/16, emoji, RTL, null bytes, control chars",
            Self::ResourceExhaustion => "Memory, disk, file descriptors, connections, CPU",
            Self::Dependencies => "Version mismatch, unavailable service, API changes",
        }
    }
}

/// Output format for generated scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioFormat {
    UseCase,
    UserStory,
    TestScenario,
    ThreatScenario,
}

impl ScenarioFormat {
    /// All formats.
    pub fn all() -> &'static [ScenarioFormat] {
        &[
            Self::UseCase,
            Self::UserStory,
            Self::TestScenario,
            Self::ThreatScenario,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::UseCase => "Use Case",
            Self::UserStory => "User Story",
            Self::TestScenario => "Test Scenario",
            Self::ThreatScenario => "Threat Scenario",
        }
    }
}

/// A single generated scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    /// Which dimension this explores.
    pub dimension: Dimension,
    /// Output format.
    pub format: ScenarioFormat,
    /// Title.
    pub title: String,
    /// Description of the scenario.
    pub description: String,
    /// Expected behavior / acceptance criteria.
    pub expected: String,
    /// Relevant files or components.
    pub relevant_files: Vec<String>,
}

/// Scenario generation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSession {
    /// Target to explore (feature, module, or system description).
    pub target: String,
    /// Which dimensions to explore.
    pub dimensions: Vec<Dimension>,
    /// Output format to use.
    pub format: ScenarioFormat,
    /// Generated scenarios.
    pub scenarios: Vec<Scenario>,
}

impl ScenarioSession {
    /// Create a new session exploring all dimensions.
    pub fn new(target: String, format: ScenarioFormat) -> Self {
        Self {
            target,
            dimensions: Dimension::all().to_vec(),
            format,
            scenarios: Vec::new(),
        }
    }

    /// Count scenarios per dimension.
    pub fn count_by_dimension(&self, dimension: Dimension) -> usize {
        self.scenarios
            .iter()
            .filter(|s| s.dimension == dimension)
            .count()
    }

    /// Total scenario count.
    pub fn total_count(&self) -> usize {
        self.scenarios.len()
    }
}

/// The edge case exploration mode.
#[derive(Debug, Clone, Default)]
pub struct ScenarioMode;

impl ModeRunner for ScenarioMode {
    fn name(&self) -> &'static str {
        "scenario"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Scenario mode requires a goal (feature or system to explore)");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "scenario",
            purpose: "Edge case exploration across 12 dimensions of potential failure",
            default_iterations: Some(20),
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
            goal: "Authentication system".into(),
            scope: vec!["src/auth/**".into()],
            metric: String::new(),
            direction: Direction::Higher,
            verify: String::new(),
            guard: None,
            iterations: Some(20),
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
        let mode = ScenarioMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_goal() {
        let mode = ScenarioMode;
        let mut config = make_config();
        config.goal = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_twelve_dimensions() {
        assert_eq!(Dimension::all().len(), 12);
    }

    #[test]
    fn test_four_formats() {
        assert_eq!(ScenarioFormat::all().len(), 4);
    }

    #[test]
    fn test_session_count() {
        let mut session = ScenarioSession::new("auth".into(), ScenarioFormat::TestScenario);
        session.scenarios.push(Scenario {
            dimension: Dimension::BoundaryValues,
            format: ScenarioFormat::TestScenario,
            title: "Max password length".into(),
            description: "Test with 10000-char password".into(),
            expected: "Reject gracefully".into(),
            relevant_files: vec!["src/auth/password.rs".into()],
        });
        assert_eq!(session.total_count(), 1);
        assert_eq!(session.count_by_dimension(Dimension::BoundaryValues), 1);
        assert_eq!(session.count_by_dimension(Dimension::NullEmpty), 0);
    }
}
