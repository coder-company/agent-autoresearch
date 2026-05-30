//! Interactive wizard mode for generating a RunConfig.
//!
//! Scans the repository structure, suggests metrics based on common
//! project patterns, validates the verify command with a dry-run,
//! and outputs a fully populated RunConfig.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// A suggested metric detected from repo structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSuggestion {
    /// Human-readable name.
    pub name: String,
    /// The metric to optimize.
    pub metric: String,
    /// Suggested verify command.
    pub verify_command: String,
    /// Whether to go higher or lower.
    pub direction: &'static str,
    /// Why this metric was suggested.
    pub rationale: String,
}

/// Well-known project patterns and their metric suggestions.
pub fn builtin_suggestions() -> Vec<MetricSuggestion> {
    vec![
        MetricSuggestion {
            name: "Test Coverage".into(),
            metric: "coverage".into(),
            verify_command: "npm test -- --coverage | tail -1".into(),
            direction: "higher",
            rationale: "Found test configuration files".into(),
        },
        MetricSuggestion {
            name: "Error Count".into(),
            metric: "errors".into(),
            verify_command: "npm run build 2>&1 | grep -c 'error' || echo 0".into(),
            direction: "lower",
            rationale: "Reduce build or runtime errors".into(),
        },
        MetricSuggestion {
            name: "Lint Warnings".into(),
            metric: "warnings".into(),
            verify_command:
                "npx eslint src/ --format compact 2>&1 | tail -1 | grep -oP '\\d+' | head -1".into(),
            direction: "lower",
            rationale: "Found linter configuration".into(),
        },
        MetricSuggestion {
            name: "Type Errors".into(),
            metric: "type_errors".into(),
            verify_command: "npx tsc --noEmit 2>&1 | grep -c 'error TS' || echo 0".into(),
            direction: "lower",
            rationale: "Found TypeScript configuration".into(),
        },
        MetricSuggestion {
            name: "Bundle Size (KB)".into(),
            metric: "bundle_size".into(),
            verify_command: "npm run build 2>/dev/null && du -sk dist/ | cut -f1".into(),
            direction: "lower",
            rationale: "Found build output directory".into(),
        },
    ]
}

/// File patterns that indicate a particular metric may be relevant.
pub const PATTERN_INDICATORS: &[(&str, &str)] = &[
    ("jest.config*", "Test Coverage"),
    ("vitest.config*", "Test Coverage"),
    ("pytest.ini", "Test Coverage"),
    ("Cargo.toml", "Test Coverage"),
    ("tsconfig.json", "Type Errors"),
    (".eslintrc*", "Lint Warnings"),
    ("webpack.config*", "Bundle Size (KB)"),
    ("vite.config*", "Bundle Size (KB)"),
];

/// Scan glob patterns against the workspace to find files.
pub fn scan_repo_files(workspace: &std::path::Path, patterns: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for pattern in patterns {
        let full = format!("{}/{}", workspace.display(), pattern);
        if let Ok(entries) = glob::glob(&full) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name() {
                    found.push(name.to_string_lossy().to_string());
                }
            }
        }
    }
    found
}

/// Match found files against known indicators to suggest relevant metrics.
pub fn suggest_metrics(found_files: &[String]) -> Vec<MetricSuggestion> {
    let all = builtin_suggestions();
    let mut relevant = Vec::new();

    for (pattern_base, metric_name) in PATTERN_INDICATORS {
        // Check if any found file matches the indicator pattern basename
        let prefix = pattern_base.trim_end_matches('*');
        if found_files.iter().any(|f| f.starts_with(prefix)) {
            if let Some(suggestion) = all.iter().find(|s| s.name == *metric_name) {
                if !relevant
                    .iter()
                    .any(|r: &MetricSuggestion| r.name == suggestion.name)
                {
                    relevant.push(suggestion.clone());
                }
            }
        }
    }

    relevant
}

/// The interactive plan/wizard mode.
#[derive(Debug, Clone, Default)]
pub struct PlanMode;

impl ModeRunner for PlanMode {
    fn name(&self) -> &'static str {
        "plan"
    }

    fn validate_config(&self, _config: &RunConfig) -> Result<()> {
        // Plan mode is the *generator* of config — it has no required fields.
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "plan",
            purpose: "Interactive wizard: scan repo, suggest metrics, output RunConfig",
            default_iterations: None,
            required_fields: &[],
            optional_fields: &["workspace_root"],
        }
    }
}

/// Validate that a verify command can execute (dry-run).
pub fn dry_run_verify(command: &str, cwd: &std::path::Path) -> Result<String> {
    use std::process::Command;
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Verify command failed dry-run: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_suggestions_not_empty() {
        assert!(!builtin_suggestions().is_empty());
    }

    #[test]
    fn test_suggest_metrics_typescript() {
        let files = vec!["tsconfig.json".to_string()];
        let suggestions = suggest_metrics(&files);
        assert!(suggestions.iter().any(|s| s.name == "Type Errors"));
    }

    #[test]
    fn test_suggest_metrics_jest() {
        let files = vec!["jest.config.js".to_string()];
        let suggestions = suggest_metrics(&files);
        assert!(suggestions.iter().any(|s| s.name == "Test Coverage"));
    }

    #[test]
    fn test_suggest_metrics_no_match() {
        let files = vec!["README.md".to_string()];
        let suggestions = suggest_metrics(&files);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_plan_mode_validate_always_ok() {
        let mode = PlanMode;
        let config = RunConfig {
            goal: String::new(),
            scope: vec![],
            metric: String::new(),
            direction: crate::core::config::Direction::Higher,
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
        };
        assert!(mode.validate_config(&config).is_ok());
    }
}
