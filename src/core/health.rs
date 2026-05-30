use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::context::RunContext;
use super::git::{GitRepo, WorktreeStatus};
use super::results::{results_dir, ResultsLog};
use super::state::RunState;
use super::verify;

#[derive(Debug, Serialize)]
pub struct HealthFinding {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub decision: &'static str,
    pub workspace: String,
    pub results_path: String,
    pub state_path: String,
    pub context_path: String,
    pub has_results: bool,
    pub has_state: bool,
    pub has_context: bool,
    pub main_rows: usize,
    pub expected_rows: Option<u32>,
    pub git_state: String,
    pub free_mb: Option<u64>,
    pub verify_command: Option<String>,
    pub warnings: Vec<HealthFinding>,
    pub blockers: Vec<HealthFinding>,
}

impl HealthReport {
    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }
}

pub fn run_health_check(
    workspace: &Path,
    verify_command: Option<&str>,
    min_free_mb: u64,
) -> Result<HealthReport> {
    let results_path = results_dir(workspace);
    let state_path = results_path.join("state.json");
    let context_path = results_path.join("context.json");
    let tsv_path = results_path.join("results.tsv");

    let mut warnings = Vec::new();
    let mut blockers = Vec::new();

    let git_state = match GitRepo::open(workspace) {
        Ok(repo) => {
            if repo.head_detached()? {
                blockers.push(HealthFinding {
                    code: "detached_head",
                    message: "HEAD is detached; checkout a branch before launching autoresearch"
                        .to_string(),
                });
            }

            let lock_files = repo.lock_files();
            if !lock_files.is_empty() {
                blockers.push(HealthFinding {
                    code: "git_lock_file",
                    message: format!(
                        "stale git lock files found: {}",
                        lock_files
                            .iter()
                            .map(|path| display_path(path))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
                "locked".to_string()
            } else {
                match repo.worktree_status()? {
                    WorktreeStatus::Clean => "clean".to_string(),
                    WorktreeStatus::OnlyArtifacts => "only_artifacts".to_string(),
                    WorktreeStatus::Dirty(files) => {
                        warnings.push(HealthFinding {
                            code: "dirty_worktree",
                            message: format!("unexpected worktree changes: {}", files.join(", ")),
                        });
                        "dirty".to_string()
                    }
                }
            }
        }
        Err(err) => {
            blockers.push(HealthFinding {
                code: "git_unavailable",
                message: err.to_string(),
            });
            "unavailable".to_string()
        }
    };

    let free_mb = disk_free_mb(workspace);
    match free_mb {
        Some(value) if value < min_free_mb => blockers.push(HealthFinding {
            code: "low_disk",
            message: format!("only {value} MB free; require at least {min_free_mb} MB"),
        }),
        None => warnings.push(HealthFinding {
            code: "disk_unknown",
            message: "could not determine free disk space".to_string(),
        }),
        _ => {}
    }

    let has_results = tsv_path.exists();
    let has_state = state_path.exists();
    let has_context = context_path.exists();
    let mut main_rows = 0usize;
    let mut expected_rows = None;
    let mut state_verify = None;

    if has_results {
        let log = ResultsLog::open(tsv_path.clone())?;
        main_rows = log.count()?;
    }
    if has_state {
        let state: RunState = serde_json::from_str(
            &std::fs::read_to_string(&state_path)
                .with_context(|| format!("failed to read {}", state_path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", state_path.display()))?;
        expected_rows = Some(state.iteration + 1);
        state_verify = state.config.as_ref().map(|config| config.verify.clone());
    }
    if has_context {
        let context: RunContext = serde_json::from_str(
            &std::fs::read_to_string(&context_path)
                .with_context(|| format!("failed to read {}", context_path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", context_path.display()))?;
        if !context.active {
            warnings.push(HealthFinding {
                code: "inactive_context",
                message: "context.json is marked inactive".to_string(),
            });
        }
    } else if has_results || has_state {
        warnings.push(HealthFinding {
            code: "missing_context",
            message: "context.json is missing; resume can still use state.json".to_string(),
        });
    }

    match (has_results, has_state) {
        (false, false) => warnings.push(HealthFinding {
            code: "no_run_artifacts",
            message: "no autoresearch-results/results.tsv or state.json found".to_string(),
        }),
        (true, false) => blockers.push(HealthFinding {
            code: "missing_state",
            message: "results.tsv exists but state.json is missing".to_string(),
        }),
        (false, true) => blockers.push(HealthFinding {
            code: "missing_results",
            message: "state.json exists but results.tsv is missing".to_string(),
        }),
        (true, true) => {
            if let Some(expected) = expected_rows {
                if main_rows as u32 != expected {
                    blockers.push(HealthFinding {
                        code: "artifact_mismatch",
                        message: format!(
                            "results.tsv has {main_rows} data rows, state expects {expected}"
                        ),
                    });
                }
            }
        }
    }

    let effective_verify = verify_command
        .map(ToOwned::to_owned)
        .or(state_verify)
        .filter(|command| !command.trim().is_empty());

    match effective_verify.as_deref() {
        Some(command) if !verify::command_exists(command) => blockers.push(HealthFinding {
            code: "verify_command_missing",
            message: format!("verify command binary not found for: {command}"),
        }),
        Some(_) => {}
        None => warnings.push(HealthFinding {
            code: "verify_command_unknown",
            message: "no verify command supplied and none found in state.json".to_string(),
        }),
    }

    let decision = if !blockers.is_empty() {
        "block"
    } else if !warnings.is_empty() {
        "warn"
    } else {
        "ok"
    };

    Ok(HealthReport {
        decision,
        workspace: display_path(workspace),
        results_path: display_path(&results_path),
        state_path: display_path(&state_path),
        context_path: display_path(&context_path),
        has_results,
        has_state,
        has_context,
        main_rows,
        expected_rows,
        git_state,
        free_mb,
        verify_command: effective_verify,
        warnings,
        blockers,
    })
}

fn disk_free_mb(path: &Path) -> Option<u64> {
    let output = Command::new("df").arg("-Pk").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().nth(1)?;
    let available_kb = line.split_whitespace().nth(3)?.parse::<u64>().ok()?;
    Some(available_kb / 1024)
}

fn display_path(path: &Path) -> String {
    PathBuf::from(path).display().to_string()
}
