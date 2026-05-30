use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::config::RunConfig;
use super::results::{ensure_results_dir_protected, results_dir};
use super::state::RunState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchManifest {
    pub version: u32,
    pub created_at: String,
    pub workspace_root: String,
    pub primary_repo: String,
    pub results_path: String,
    pub state_path: String,
    pub launch_path: String,
    pub runtime_path: String,
    pub log_path: String,
    pub execution_policy: String,
    pub codex_bin: String,
    pub codex_args: Vec<String>,
    pub config: Option<RunConfig>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub version: u32,
    pub status: String,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub launch_path: String,
    pub runtime_path: String,
    pub log_path: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub results_path: PathBuf,
    pub state_path: PathBuf,
    pub launch_path: PathBuf,
    pub runtime_path: PathBuf,
    pub log_path: PathBuf,
}

pub fn paths(workspace: &Path) -> RuntimePaths {
    let results_path = results_dir(workspace);
    RuntimePaths {
        state_path: results_path.join("state.json"),
        launch_path: results_path.join("launch.json"),
        runtime_path: results_path.join("runtime.json"),
        log_path: results_path.join("runtime.log"),
        results_path,
    }
}

pub fn create_launch_manifest(
    workspace: &Path,
    execution_policy: &str,
    codex_bin: &str,
) -> Result<LaunchManifest> {
    let paths = paths(workspace);
    let state = read_state(&paths.state_path)?;
    let config = state.config.clone();
    let prompt = runtime_prompt(&state);
    let codex_args = codex_args(execution_policy)?;

    Ok(LaunchManifest {
        version: 1,
        created_at: Utc::now().to_rfc3339(),
        workspace_root: workspace.display().to_string(),
        primary_repo: config
            .as_ref()
            .and_then(|config| config.primary_repo.as_ref())
            .unwrap_or(&workspace.to_path_buf())
            .display()
            .to_string(),
        results_path: paths.results_path.display().to_string(),
        state_path: paths.state_path.display().to_string(),
        launch_path: paths.launch_path.display().to_string(),
        runtime_path: paths.runtime_path.display().to_string(),
        log_path: paths.log_path.display().to_string(),
        execution_policy: execution_policy.to_string(),
        codex_bin: codex_bin.to_string(),
        codex_args,
        config,
        prompt,
    })
}

pub fn write_launch_manifest(workspace: &Path, manifest: &LaunchManifest) -> Result<RuntimePaths> {
    let results_path = ensure_results_dir_protected(workspace)?;
    let paths = RuntimePaths {
        state_path: results_path.join("state.json"),
        launch_path: results_path.join("launch.json"),
        runtime_path: results_path.join("runtime.json"),
        log_path: results_path.join("runtime.log"),
        results_path,
    };
    fs::write(&paths.launch_path, serde_json::to_string_pretty(manifest)?)
        .with_context(|| format!("failed to write {}", paths.launch_path.display()))?;
    Ok(paths)
}

pub fn start_runtime(
    workspace: &Path,
    execution_policy: &str,
    codex_bin: &str,
    dry_run: bool,
) -> Result<(LaunchManifest, RuntimeSnapshot)> {
    let manifest = create_launch_manifest(workspace, execution_policy, codex_bin)?;
    let paths = write_launch_manifest(workspace, &manifest)?;

    append_log(
        &paths.log_path,
        &format!(
            "{} runtime start requested dry_run={dry_run}\n",
            Utc::now().to_rfc3339()
        ),
    )?;

    if dry_run {
        let snapshot = RuntimeSnapshot {
            version: 1,
            status: "ready".to_string(),
            pid: None,
            started_at: None,
            stopped_at: None,
            launch_path: paths.launch_path.display().to_string(),
            runtime_path: paths.runtime_path.display().to_string(),
            log_path: paths.log_path.display().to_string(),
            last_error: None,
        };
        write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
        return Ok((manifest, snapshot));
    }

    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_path)
        .with_context(|| format!("failed to open {}", paths.log_path.display()))?;
    let err_log = log
        .try_clone()
        .context("failed to clone runtime log handle")?;

    let mut child = Command::new(codex_bin)
        .arg("exec")
        .args(&manifest.codex_args)
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err_log))
        .spawn()
        .with_context(|| format!("failed to launch {codex_bin} exec"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(manifest.prompt.as_bytes())
            .context("failed to write runtime prompt to codex stdin")?;
    }

    let snapshot = RuntimeSnapshot {
        version: 1,
        status: "running".to_string(),
        pid: Some(child.id()),
        started_at: Some(Utc::now().to_rfc3339()),
        stopped_at: None,
        launch_path: paths.launch_path.display().to_string(),
        runtime_path: paths.runtime_path.display().to_string(),
        log_path: paths.log_path.display().to_string(),
        last_error: None,
    };
    write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
    Ok((manifest, snapshot))
}

pub fn runtime_status(workspace: &Path) -> Result<RuntimeSnapshot> {
    let paths = paths(workspace);
    if !paths.runtime_path.exists() {
        anyhow::bail!("No runtime.json found at {}", paths.runtime_path.display());
    }
    let mut snapshot: RuntimeSnapshot =
        serde_json::from_str(&fs::read_to_string(&paths.runtime_path)?)?;
    if snapshot.status == "running" {
        if let Some(pid) = snapshot.pid {
            if !process_is_alive(pid) {
                snapshot.status = "stopped".to_string();
                snapshot.stopped_at = Some(Utc::now().to_rfc3339());
                snapshot.last_error = Some("process is no longer running".to_string());
                write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
            }
        }
    }
    Ok(snapshot)
}

pub fn stop_runtime(workspace: &Path) -> Result<RuntimeSnapshot> {
    let paths = paths(workspace);
    let mut snapshot = runtime_status(workspace)?;
    if snapshot.status == "running" {
        if let Some(pid) = snapshot.pid {
            terminate_process(pid)?;
        }
    }
    snapshot.status = "stopped".to_string();
    snapshot.stopped_at = Some(Utc::now().to_rfc3339());
    write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
    append_log(
        &paths.log_path,
        &format!("{} runtime stop requested\n", Utc::now().to_rfc3339()),
    )?;
    Ok(snapshot)
}

fn read_state(path: &Path) -> Result<RunState> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

fn runtime_prompt(state: &RunState) -> String {
    let config = state.config.as_ref();
    let goal = config
        .map(|config| config.goal.as_str())
        .filter(|goal| !goal.trim().is_empty())
        .unwrap_or("Continue the active autoresearch run.");
    let verify = config
        .map(|config| config.verify.as_str())
        .filter(|verify| !verify.trim().is_empty())
        .unwrap_or("<missing verify command>");

    format!(
        "$autoresearch loop\nGoal: {goal}\nVerify: {verify}\nResume from autoresearch-results/state.json and continue autonomously until the configured stop condition, iteration cap, blocker, or user stop request.\n"
    )
}

fn codex_args(execution_policy: &str) -> Result<Vec<String>> {
    match execution_policy {
        "danger_full_access" => Ok(vec![
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
        ]),
        "workspace_write" => Ok(Vec::new()),
        _ => anyhow::bail!(
            "Unknown execution policy: {execution_policy}. Use danger_full_access or workspace_write."
        ),
    }
}

fn write_runtime_snapshot(path: &Path, snapshot: &RuntimeSnapshot) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(snapshot)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn append_log(path: &Path, text: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    file.write_all(text.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<()> {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .context("failed to invoke kill")?;
    if !status.success() {
        anyhow::bail!("failed to terminate process {pid}");
    }
    Ok(())
}

#[cfg(not(unix))]
fn terminate_process(_pid: u32) -> Result<()> {
    anyhow::bail!("runtime stop is not supported on this platform")
}
