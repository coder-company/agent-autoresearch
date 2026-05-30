use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::config::{RunConfig, RunMode};
use super::results::results_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoTarget {
    pub path: String,
    pub scope: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunContext {
    pub version: u32,
    pub active: bool,
    pub session_mode: Option<String>,
    pub workspace_root: String,
    pub artifact_root: String,
    pub primary_repo: String,
    pub repo_targets: Vec<RepoTarget>,
    pub verify_cwd: String,
    pub results_path: String,
    pub state_path: String,
    pub launch_path: String,
    pub runtime_path: String,
    pub log_path: String,
    pub updated_at: String,
}

pub fn write_context(workspace: &Path, config: Option<&RunConfig>) -> Result<PathBuf> {
    let results = results_dir(workspace);
    let workspace_root = absolute_display(
        config
            .and_then(|config| config.workspace_root.as_deref())
            .unwrap_or(workspace),
    );
    let primary_repo = absolute_display(
        config
            .and_then(|config| config.primary_repo.as_deref())
            .unwrap_or(workspace),
    );
    let scope = config
        .map(|config| config.scope.join(","))
        .filter(|scope| !scope.trim().is_empty())
        .unwrap_or_else(|| ".".to_string());
    let session_mode = config.and_then(|config| {
        config.run_mode.map(|mode| match mode {
            RunMode::Foreground => "foreground".to_string(),
            RunMode::Background => "background".to_string(),
        })
    });

    let context = RunContext {
        version: 2,
        active: true,
        session_mode,
        workspace_root,
        artifact_root: absolute_display(&results),
        primary_repo: primary_repo.clone(),
        repo_targets: vec![RepoTarget {
            path: primary_repo,
            scope,
            role: "primary".to_string(),
        }],
        verify_cwd: "workspace_root".to_string(),
        results_path: absolute_display(&results.join("results.tsv")),
        state_path: absolute_display(&results.join("state.json")),
        launch_path: absolute_display(&results.join("launch.json")),
        runtime_path: absolute_display(&results.join("runtime.json")),
        log_path: absolute_display(&results.join("runtime.log")),
        updated_at: Utc::now().to_rfc3339(),
    };

    let path = results.join("context.json");
    fs::write(&path, serde_json::to_string_pretty(&context)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn absolute_display(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path))
        .display()
        .to_string()
}
