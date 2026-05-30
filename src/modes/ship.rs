//! 8-phase ship workflow mode.
//!
//! Phases: Preflight → Test → Lint → Build → Changelog → Version → Commit → Push/PR.
//! Each phase gates the next (must pass to proceed). Supports --dry-run,
//! --checklist-only, and --auto flags.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// The 8 ship phases, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShipPhase {
    /// Check preconditions: clean worktree, correct branch, etc.
    Preflight,
    /// Run tests.
    Test,
    /// Run linter.
    Lint,
    /// Run build.
    Build,
    /// Update changelog.
    Changelog,
    /// Bump version.
    Version,
    /// Create commit.
    Commit,
    /// Push and/or create PR.
    PushPr,
}

impl ShipPhase {
    /// All phases in execution order.
    pub fn all() -> &'static [ShipPhase] {
        &[
            Self::Preflight,
            Self::Test,
            Self::Lint,
            Self::Build,
            Self::Changelog,
            Self::Version,
            Self::Commit,
            Self::PushPr,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Preflight => "Preflight",
            Self::Test => "Test",
            Self::Lint => "Lint",
            Self::Build => "Build",
            Self::Changelog => "Changelog",
            Self::Version => "Version",
            Self::Commit => "Commit",
            Self::PushPr => "Push/PR",
        }
    }
}

/// Outcome of a single ship phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseOutcome {
    Pass,
    Fail,
    Skip,
    Pending,
}

/// Record of a completed phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseRecord {
    pub phase: ShipPhase,
    pub outcome: PhaseOutcome,
    /// Optional detail message.
    pub detail: Option<String>,
}

/// Ship mode flags.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ShipFlags {
    /// Run all steps but don't actually push/commit.
    pub dry_run: bool,
    /// Only output the checklist, don't execute anything.
    pub checklist_only: bool,
    /// Auto-approve all interactive prompts.
    pub auto: bool,
}

/// Ship session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipSession {
    /// Flags for this ship run.
    pub flags: ShipFlags,
    /// Completed phase records.
    pub phases: Vec<PhaseRecord>,
    /// Current phase index.
    pub current_phase_index: usize,
}

impl ShipSession {
    /// Create a new ship session.
    pub fn new(flags: ShipFlags) -> Self {
        Self {
            flags,
            phases: Vec::new(),
            current_phase_index: 0,
        }
    }

    /// Get the current phase.
    pub fn current_phase(&self) -> Option<ShipPhase> {
        ShipPhase::all().get(self.current_phase_index).copied()
    }

    /// Record a phase result and advance if passed.
    pub fn record_phase(&mut self, outcome: PhaseOutcome, detail: Option<String>) {
        if let Some(phase) = self.current_phase() {
            self.phases.push(PhaseRecord {
                phase,
                outcome,
                detail,
            });
            if outcome == PhaseOutcome::Pass || outcome == PhaseOutcome::Skip {
                self.current_phase_index += 1;
            }
        }
    }

    /// Whether all phases are complete.
    pub fn is_complete(&self) -> bool {
        self.current_phase_index >= ShipPhase::all().len()
    }

    /// Whether any phase failed.
    pub fn has_failures(&self) -> bool {
        self.phases.iter().any(|p| p.outcome == PhaseOutcome::Fail)
    }

    /// Generate a checklist summary.
    pub fn checklist(&self) -> Vec<String> {
        ShipPhase::all()
            .iter()
            .map(|phase| {
                let record = self.phases.iter().find(|r| r.phase == *phase);
                let status = match record {
                    Some(r) => match r.outcome {
                        PhaseOutcome::Pass => "✅",
                        PhaseOutcome::Fail => "❌",
                        PhaseOutcome::Skip => "⏭️",
                        PhaseOutcome::Pending => "⏳",
                    },
                    None => "⬜",
                };
                format!("{status} {}", phase.label())
            })
            .collect()
    }
}

/// The 8-phase ship workflow mode.
#[derive(Debug, Clone, Default)]
pub struct ShipMode;

impl ModeRunner for ShipMode {
    fn name(&self) -> &'static str {
        "ship"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.scope.is_empty() {
            bail!("Ship mode requires at least one scope pattern");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "ship",
            purpose: "8-phase ship workflow: Preflight → Test → Lint → Build → Changelog → Version → Commit → Push/PR",
            default_iterations: None,
            required_fields: &["scope"],
            optional_fields: &["goal", "guard", "verify"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config() -> RunConfig {
        RunConfig {
            goal: "Ship v1.2.0".into(),
            scope: vec!["src/**/*.rs".into()],
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
            rollback_strategy: Default::default(),
            run_mode: None,
            workspace_root: None,
            primary_repo: None,
        }
    }

    #[test]
    fn test_validate_valid() {
        let mode = ShipMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_scope() {
        let mode = ShipMode;
        let mut config = make_config();
        config.scope = vec![];
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_ship_phases_count() {
        assert_eq!(ShipPhase::all().len(), 8);
    }

    #[test]
    fn test_ship_session_advance() {
        let mut session = ShipSession::new(ShipFlags::default());
        assert_eq!(session.current_phase(), Some(ShipPhase::Preflight));

        session.record_phase(PhaseOutcome::Pass, None);
        assert_eq!(session.current_phase(), Some(ShipPhase::Test));

        session.record_phase(PhaseOutcome::Fail, Some("3 tests failed".into()));
        // Should NOT advance on failure.
        assert_eq!(session.current_phase(), Some(ShipPhase::Test));
    }

    #[test]
    fn test_ship_session_complete() {
        let mut session = ShipSession::new(ShipFlags::default());
        for _ in ShipPhase::all() {
            session.record_phase(PhaseOutcome::Pass, None);
        }
        assert!(session.is_complete());
        assert!(!session.has_failures());
    }

    #[test]
    fn test_checklist_generation() {
        let mut session = ShipSession::new(ShipFlags::default());
        session.record_phase(PhaseOutcome::Pass, None);
        session.record_phase(PhaseOutcome::Fail, None);

        let checklist = session.checklist();
        assert_eq!(checklist.len(), 8);
        assert!(checklist[0].contains('✅'));
        assert!(checklist[1].contains('❌'));
        assert!(checklist[2].contains('⬜'));
    }
}
