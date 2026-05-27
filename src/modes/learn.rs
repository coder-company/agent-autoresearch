//! Documentation engine mode.
//!
//! Four sub-modes: init (full scan), update (incremental), check (validate),
//! summarize. Scouts for undocumented code, generates docs, validates
//! accuracy, and corrects errors.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Documentation sub-mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnSubMode {
    /// Full scan: discover and document everything.
    Init,
    /// Incremental: document only changed/new code.
    Update,
    /// Validate existing docs against code.
    Check,
    /// Generate a summary of the project.
    Summarize,
}

/// Documentation phase within each sub-mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnPhase {
    /// Identify undocumented code.
    Scout,
    /// Generate documentation.
    Generate,
    /// Verify accuracy of generated docs.
    Validate,
    /// Correct errors found during validation.
    Fix,
}

impl LearnPhase {
    /// All phases in order.
    pub fn all() -> &'static [LearnPhase] {
        &[Self::Scout, Self::Generate, Self::Validate, Self::Fix]
    }
}

/// A documentation gap found during scouting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocGap {
    /// File path.
    pub file: String,
    /// What's missing (e.g., "module doc", "function doc", "type doc").
    pub kind: String,
    /// The symbol or construct that lacks documentation.
    pub symbol: String,
    /// Line number.
    pub line: Option<u32>,
    /// Whether docs were generated for this gap.
    pub resolved: bool,
}

/// Validation issue found when checking docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocIssue {
    /// File path.
    pub file: String,
    /// Line number.
    pub line: Option<u32>,
    /// What's wrong.
    pub problem: String,
    /// Whether this was fixed.
    pub fixed: bool,
}

/// Learn session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnSession {
    /// Active sub-mode.
    pub sub_mode: LearnSubMode,
    /// Current phase.
    pub phase: LearnPhase,
    /// Documentation gaps found.
    pub gaps: Vec<DocGap>,
    /// Validation issues found.
    pub issues: Vec<DocIssue>,
    /// Files documented so far.
    pub documented_files: Vec<String>,
}

impl LearnSession {
    /// Create a new learn session.
    pub fn new(sub_mode: LearnSubMode) -> Self {
        Self {
            sub_mode,
            phase: LearnPhase::Scout,
            gaps: Vec::new(),
            issues: Vec::new(),
            documented_files: Vec::new(),
        }
    }

    /// Count unresolved gaps.
    pub fn unresolved_gaps(&self) -> usize {
        self.gaps.iter().filter(|g| !g.resolved).count()
    }

    /// Count unfixed issues.
    pub fn unfixed_issues(&self) -> usize {
        self.issues.iter().filter(|i| !i.fixed).count()
    }

    /// Advance to the next phase.
    pub fn advance_phase(&mut self) {
        self.phase = match self.phase {
            LearnPhase::Scout => LearnPhase::Generate,
            LearnPhase::Generate => LearnPhase::Validate,
            LearnPhase::Validate => LearnPhase::Fix,
            LearnPhase::Fix => LearnPhase::Fix, // Terminal.
        };
    }
}

/// The documentation engine mode.
#[derive(Debug, Clone, Default)]
pub struct LearnMode;

impl ModeRunner for LearnMode {
    fn name(&self) -> &'static str {
        "learn"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.scope.is_empty() {
            bail!("Learn mode requires at least one scope pattern");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "learn",
            purpose: "Documentation engine: scout → generate → validate → fix",
            default_iterations: Some(10),
            required_fields: &["scope"],
            optional_fields: &["goal", "iterations"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config() -> RunConfig {
        RunConfig {
            goal: "Document all public APIs".into(),
            scope: vec!["src/**/*.rs".into()],
            metric: String::new(),
            direction: Direction::Higher,
            verify: String::new(),
            guard: None,
            iterations: Some(10),
            run_tag: None,
            stop_condition: None,
            verify_format: Default::default(),
            primary_metric_key: None,
            rollback_strategy: Default::default(),
            run_mode: None,
            workspace_root: None,
            primary_repo: None,
        }
    }

    #[test]
    fn test_validate_valid() {
        let mode = LearnMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_scope() {
        let mode = LearnMode;
        let mut config = make_config();
        config.scope = vec![];
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_learn_phases_count() {
        assert_eq!(LearnPhase::all().len(), 4);
    }

    #[test]
    fn test_session_advance() {
        let mut session = LearnSession::new(LearnSubMode::Init);
        assert_eq!(session.phase, LearnPhase::Scout);
        session.advance_phase();
        assert_eq!(session.phase, LearnPhase::Generate);
        session.advance_phase();
        assert_eq!(session.phase, LearnPhase::Validate);
        session.advance_phase();
        assert_eq!(session.phase, LearnPhase::Fix);
        // Terminal
        session.advance_phase();
        assert_eq!(session.phase, LearnPhase::Fix);
    }

    #[test]
    fn test_unresolved_gaps() {
        let mut session = LearnSession::new(LearnSubMode::Init);
        session.gaps.push(DocGap {
            file: "src/lib.rs".into(),
            kind: "module doc".into(),
            symbol: "core".into(),
            line: Some(1),
            resolved: false,
        });
        session.gaps.push(DocGap {
            file: "src/main.rs".into(),
            kind: "function doc".into(),
            symbol: "main".into(),
            line: Some(5),
            resolved: true,
        });
        assert_eq!(session.unresolved_gaps(), 1);
    }
}
