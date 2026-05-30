//! Error crushing mode.
//!
//! Auto-detect error sources, prioritize by severity, fix one error per
//! iteration, verify after each fix, and stop when the error count reaches 0.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Category of errors, ordered by severity (highest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Runtime crashes or panics.
    Crash = 0,
    /// Test failures.
    TestFailure = 1,
    /// Type/compilation errors.
    TypeError = 2,
    /// Lint errors.
    LintError = 3,
    /// Build warnings (lowest priority).
    Warning = 4,
}

impl ErrorCategory {
    /// All categories in priority order.
    pub fn priority_order() -> &'static [ErrorCategory] {
        &[
            Self::Crash,
            Self::TestFailure,
            Self::TypeError,
            Self::LintError,
            Self::Warning,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Crash => "crash",
            Self::TestFailure => "test failure",
            Self::TypeError => "type error",
            Self::LintError => "lint error",
            Self::Warning => "warning",
        }
    }
}

/// A single detected error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedError {
    /// Category/priority.
    pub category: ErrorCategory,
    /// File path where the error occurs.
    pub file: Option<String>,
    /// Line number.
    pub line: Option<u32>,
    /// Error message.
    pub message: String,
    /// Whether this error has been fixed.
    pub fixed: bool,
}

/// Fix session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSession {
    /// All detected errors.
    pub errors: Vec<DetectedError>,
    /// Number of errors fixed so far.
    pub fixed_count: u32,
    /// Initial total error count.
    pub initial_count: u32,
    /// Current error count (decreasing).
    pub current_count: u32,
}

impl FixSession {
    /// Create a new fix session with detected errors.
    pub fn new(errors: Vec<DetectedError>) -> Self {
        let count = errors.len() as u32;
        Self {
            errors,
            fixed_count: 0,
            initial_count: count,
            current_count: count,
        }
    }

    /// Get the next error to fix (highest priority unfixed).
    pub fn next_error(&self) -> Option<&DetectedError> {
        self.errors
            .iter()
            .filter(|e| !e.fixed)
            .min_by_key(|e| e.category)
    }

    /// Mark an error as fixed and update counts.
    pub fn mark_fixed(&mut self, index: usize) {
        if let Some(error) = self.errors.get_mut(index) {
            if !error.fixed {
                error.fixed = true;
                self.fixed_count += 1;
                self.current_count = self.current_count.saturating_sub(1);
            }
        }
    }

    /// Check if all errors are resolved.
    pub fn is_complete(&self) -> bool {
        self.current_count == 0
    }
}

/// Common verify commands for detecting errors by category.
pub fn detect_commands() -> Vec<(ErrorCategory, &'static str, &'static str)> {
    vec![
        (
            ErrorCategory::TestFailure,
            "npm test 2>&1 | grep -c 'FAIL' || echo 0",
            "Node.js test failures",
        ),
        (
            ErrorCategory::TestFailure,
            "cargo test 2>&1 | grep -c 'FAILED' || echo 0",
            "Rust test failures",
        ),
        (
            ErrorCategory::TypeError,
            "npx tsc --noEmit 2>&1 | grep -c 'error TS' || echo 0",
            "TypeScript type errors",
        ),
        (
            ErrorCategory::TypeError,
            "cargo build 2>&1 | grep -c '^error' || echo 0",
            "Rust compilation errors",
        ),
        (
            ErrorCategory::LintError,
            "npx eslint . --format compact 2>&1 | grep -c 'Error' || echo 0",
            "ESLint errors",
        ),
        (
            ErrorCategory::LintError,
            "cargo clippy 2>&1 | grep -c '^error' || echo 0",
            "Clippy errors",
        ),
    ]
}

/// The error-crushing fix mode.
#[derive(Debug, Clone, Default)]
pub struct FixMode;

impl ModeRunner for FixMode {
    fn name(&self) -> &'static str {
        "fix"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.goal.is_empty() {
            bail!("Fix mode requires a goal (error description or 'all errors')");
        }
        if config.scope.is_empty() {
            bail!("Fix mode requires at least one scope pattern");
        }
        if config.verify.is_empty() {
            bail!("Fix mode requires a verify command to count remaining errors");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "fix",
            purpose: "Error crushing: one fix per iteration until error count reaches 0",
            default_iterations: Some(20),
            required_fields: &["goal", "scope", "verify"],
            optional_fields: &["guard", "iterations", "direction"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config() -> RunConfig {
        RunConfig {
            goal: "Fix all type errors".into(),
            scope: vec!["src/**/*.ts".into()],
            metric: "type_errors".into(),
            direction: Direction::Lower,
            verify: "npx tsc --noEmit 2>&1 | grep -c 'error TS' || echo 0".into(),
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
        let mode = FixMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_verify() {
        let mode = FixMode;
        let mut config = make_config();
        config.verify = String::new();
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_error_priority_order() {
        assert!(ErrorCategory::Crash < ErrorCategory::TestFailure);
        assert!(ErrorCategory::TestFailure < ErrorCategory::TypeError);
        assert!(ErrorCategory::TypeError < ErrorCategory::LintError);
        assert!(ErrorCategory::LintError < ErrorCategory::Warning);
    }

    #[test]
    fn test_fix_session_next_error() {
        let errors = vec![
            DetectedError {
                category: ErrorCategory::LintError,
                file: Some("src/foo.ts".into()),
                line: Some(10),
                message: "unused var".into(),
                fixed: false,
            },
            DetectedError {
                category: ErrorCategory::Crash,
                file: Some("src/bar.ts".into()),
                line: Some(5),
                message: "null ref".into(),
                fixed: false,
            },
        ];
        let session = FixSession::new(errors);
        let next = session.next_error().unwrap();
        assert_eq!(next.category, ErrorCategory::Crash);
    }

    #[test]
    fn test_fix_session_mark_fixed() {
        let errors = vec![DetectedError {
            category: ErrorCategory::TypeError,
            file: None,
            line: None,
            message: "type mismatch".into(),
            fixed: false,
        }];
        let mut session = FixSession::new(errors);
        assert_eq!(session.current_count, 1);
        session.mark_fixed(0);
        assert_eq!(session.current_count, 0);
        assert!(session.is_complete());
    }
}
