use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    write_pointer(&context.primary_repo, &context.workspace_root, &path)?;
    Ok(path)
}

fn write_pointer(primary_repo: &str, workspace_root: &str, context_path: &Path) -> Result<()> {
    let primary_repo = Path::new(primary_repo);
    protect_pointer_dir(primary_repo)?;
    let pointer_dir = primary_repo.join(".codex-autoresearch");
    fs::create_dir_all(&pointer_dir)
        .with_context(|| format!("failed to create {}", pointer_dir.display()))?;
    let pointer_path = pointer_dir.join("pointer.json");
    let payload = json!({
        "version": 1,
        "workspace_root": workspace_root,
        "context_path": absolute_display(context_path),
        "updated_at": Utc::now().to_rfc3339(),
    });
    fs::write(&pointer_path, serde_json::to_string_pretty(&payload)?)
        .with_context(|| format!("failed to write {}", pointer_path.display()))
}

fn protect_pointer_dir(primary_repo: &Path) -> Result<()> {
    let git_exclude = primary_repo.join(".git/info/exclude");
    if git_exclude.exists() {
        let content = fs::read_to_string(&git_exclude)
            .with_context(|| format!("failed to read {}", git_exclude.display()))?;
        if !content
            .lines()
            .any(|line| line.trim() == ".codex-autoresearch/")
        {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&git_exclude)
                .with_context(|| format!("failed to open {}", git_exclude.display()))?;
            if !content.ends_with('\n') && !content.is_empty() {
                writeln!(file)?;
            }
            writeln!(file, ".codex-autoresearch/")?;
        }
        return Ok(());
    }

    let pointer_dir = primary_repo.join(".codex-autoresearch");
    fs::create_dir_all(&pointer_dir)
        .with_context(|| format!("failed to create {}", pointer_dir.display()))?;
    fs::write(pointer_dir.join(".gitignore"), "*\n")
        .with_context(|| format!("failed to protect {}", pointer_dir.display()))
}

fn absolute_display(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path))
        .display()
        .to_string()
}
