use anyhow::Result;
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
                let staged_artifacts = repo.staged_owned_artifacts()?;
                if !staged_artifacts.is_empty() {
                    blockers.push(HealthFinding {
                        code: "staged_autoresearch_artifacts",
                        message: format!(
                            "autoresearch-owned artifacts are staged: {}",
                            staged_artifacts.join(", ")
                        ),
                    });
                }

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
    check_disk_space(free_mb, min_free_mb, &mut warnings, &mut blockers);

    let has_results = tsv_path.exists();
    let has_state = state_path.exists();
    let has_context = context_path.exists();
    let mut main_rows = 0usize;
    let mut expected_rows = None;
    let mut state_verify = None;

    if has_results {
        let log = ResultsLog::open(tsv_path.clone())?;
        if let Err(err) = log.validate() {
            blockers.push(HealthFinding {
                code: "results_corrupt",
                message: err.to_string(),
            });
        }
        main_rows = log.count()?;
    }
    if has_state {
        match std::fs::read_to_string(&state_path) {
            Ok(content) => match serde_json::from_str::<RunState>(&content) {
                Ok(state) => {
                    expected_rows = Some(state.iteration + 1);
                    state_verify = state.config.as_ref().map(|config| config.verify.clone());
                }
                Err(err) => blockers.push(HealthFinding {
                    code: "state_corrupt",
                    message: format!("failed to parse {}: {err}", state_path.display()),
                }),
            },
            Err(err) => blockers.push(HealthFinding {
                code: "state_unreadable",
                message: format!("failed to read {}: {err}", state_path.display()),
            }),
        }
    }
    if has_context {
        match std::fs::read_to_string(&context_path) {
            Ok(content) => match serde_json::from_str::<RunContext>(&content) {
                Ok(context) => {
                    if !context.active {
                        warnings.push(HealthFinding {
                            code: "inactive_context",
                            message: "context.json is marked inactive".to_string(),
                        });
                    }
                    if let Some(message) =
                        context_path_mismatch(&context, &results_path, &state_path)
                    {
                        blockers.push(HealthFinding {
                            code: "context_mismatch",
                            message,
                        });
                    }
                }
                Err(err) => blockers.push(HealthFinding {
                    code: "context_corrupt",
                    message: format!("failed to parse {}: {err}", context_path.display()),
                }),
            },
            Err(err) => blockers.push(HealthFinding {
                code: "context_unreadable",
                message: format!("failed to read {}: {err}", context_path.display()),
            }),
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
        Some(command) => {
            if let Err(err) = verify::screen_command(command) {
                blockers.push(HealthFinding {
                    code: "verify_command_unsafe",
                    message: err.to_string(),
                });
            } else if !verify::command_exists(command) {
                blockers.push(HealthFinding {
                    code: "verify_command_missing",
                    message: format!("verify command binary not found for: {command}"),
                });
            }
        }
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

fn context_path_mismatch(
    context: &RunContext,
    results_path: &Path,
    state_path: &Path,
) -> Option<String> {
    let expected = [
        (
            "artifact_root",
            context.artifact_root.as_str(),
            results_path.to_path_buf(),
        ),
        (
            "results_path",
            context.results_path.as_str(),
            results_path.join("results.tsv"),
        ),
        (
            "state_path",
            context.state_path.as_str(),
            state_path.to_path_buf(),
        ),
        (
            "launch_path",
            context.launch_path.as_str(),
            results_path.join("launch.json"),
        ),
        (
            "runtime_path",
            context.runtime_path.as_str(),
            results_path.join("runtime.json"),
        ),
        (
            "log_path",
            context.log_path.as_str(),
            results_path.join("runtime.log"),
        ),
    ];

    expected
        .into_iter()
        .find(|(_, actual, expected)| *actual != absolute_path(expected))
        .map(|(field, actual, expected)| {
            format!(
                "context.json {field} points to {actual}; expected {}",
                absolute_path(&expected)
            )
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

fn check_disk_space(
    free_mb: Option<u64>,
    min_free_mb: u64,
    warnings: &mut Vec<HealthFinding>,
    blockers: &mut Vec<HealthFinding>,
) {
    match free_mb {
        Some(value) if value < min_free_mb => blockers.push(HealthFinding {
            code: "low_disk",
            message: format!("only {value} MB free; require at least {min_free_mb} MB"),
        }),
        Some(value) if value < max_warning_free_mb(min_free_mb) => warnings.push(HealthFinding {
            code: "disk_low_warning",
            message: format!("{value} MB free; disk headroom is below warning threshold"),
        }),
        None => warnings.push(HealthFinding {
            code: "disk_unknown",
            message: "could not determine free disk space".to_string(),
        }),
        _ => {}
    }
}

fn max_warning_free_mb(min_free_mb: u64) -> u64 {
    std::cmp::max(min_free_mb.saturating_mul(2), 1000)
}

fn display_path(path: &Path) -> String {
    PathBuf::from(path).display().to_string()
}

fn absolute_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path))
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_space_blocks_below_minimum() {
        let mut warnings = Vec::new();
        let mut blockers = Vec::new();

        check_disk_space(Some(499), 500, &mut warnings, &mut blockers);

        assert!(warnings.is_empty());
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0].code, "low_disk");
    }

    #[test]
    fn disk_space_warns_below_headroom_threshold() {
        let mut warnings = Vec::new();
        let mut blockers = Vec::new();

        check_disk_space(Some(750), 500, &mut warnings, &mut blockers);

        assert!(blockers.is_empty());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "disk_low_warning");
    }

    #[test]
    fn disk_space_accepts_enough_headroom() {
        let mut warnings = Vec::new();
        let mut blockers = Vec::new();

        check_disk_space(Some(1000), 500, &mut warnings, &mut blockers);

        assert!(warnings.is_empty());
        assert!(blockers.is_empty());
    }
}
