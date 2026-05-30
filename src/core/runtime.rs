use anyhow::{Context, Result};
use chrono::Utc;
use regex::Regex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;

use super::config::RunConfig;
use super::criteria::evaluate_criteria;
use super::git::{GitRepo, WorktreeStatus};
use super::health;
use super::results::{ensure_results_dir_protected, results_dir};
use super::state::{RunPhase, RunState, StopReason};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervisor: Option<SupervisorStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorStatus {
    pub decision: String,
    pub reason: String,
    pub terminal_reason: String,
    pub should_continue: bool,
    pub restart_count: u32,
    pub stagnation_count: u32,
    pub last_signature: String,
    pub checked_at: String,
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
    let (manifest, paths) =
        prepare_runtime_launch(workspace, execution_policy, codex_bin, dry_run)?;

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
            supervisor: None,
        };
        write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
        return Ok((manifest, snapshot));
    }

    let mut child = spawn_runtime_child(workspace, &paths, &manifest, codex_bin)?;
    write_runtime_prompt(&mut child, &manifest)?;

    let snapshot = running_snapshot(&paths, child.id());
    write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
    Ok((manifest, snapshot))
}

pub fn run_runtime_loop(
    workspace: &Path,
    execution_policy: &str,
    codex_bin: &str,
    max_restarts: u32,
    max_stagnation: u32,
) -> Result<(RuntimeSnapshot, SupervisorStatus)> {
    loop {
        run_runtime_turn(workspace, execution_policy, codex_bin)?;
        let (snapshot, supervisor) = supervise_runtime(workspace, true, max_stagnation)?;

        if supervisor.should_continue && supervisor.restart_count > max_restarts {
            return mark_restart_cap(workspace, supervisor);
        }

        if !supervisor.should_continue {
            return Ok((snapshot, supervisor));
        }
    }
}

fn run_runtime_turn(
    workspace: &Path,
    execution_policy: &str,
    codex_bin: &str,
) -> Result<RuntimeSnapshot> {
    let (manifest, paths) = prepare_runtime_launch(workspace, execution_policy, codex_bin, false)?;
    let mut child = spawn_runtime_child(workspace, &paths, &manifest, codex_bin)?;
    write_runtime_prompt(&mut child, &manifest)?;

    let mut snapshot = running_snapshot(&paths, child.id());
    write_runtime_snapshot(&paths.runtime_path, &snapshot)?;

    let exit = child.wait().context("failed to wait for codex exec")?;
    snapshot.stopped_at = Some(Utc::now().to_rfc3339());

    if exit.success() {
        snapshot.status = "stopped".to_string();
        append_log(
            &paths.log_path,
            &format!(
                "{} codex exec exited successfully\n",
                Utc::now().to_rfc3339()
            ),
        )?;
    } else {
        let message = format!("codex exec exited with status {exit}");
        snapshot.status = "needs_human".to_string();
        snapshot.last_error = Some(message.clone());
        snapshot.supervisor = Some(SupervisorStatus {
            decision: "needs_human".to_string(),
            reason: "codex_exit_failed".to_string(),
            terminal_reason: "codex_exit_failed".to_string(),
            should_continue: false,
            restart_count: 0,
            stagnation_count: 0,
            last_signature: String::new(),
            checked_at: Utc::now().to_rfc3339(),
        });
        append_log(
            &paths.log_path,
            &format!("{} {message}\n", Utc::now().to_rfc3339()),
        )?;
    }

    write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
    Ok(snapshot)
}

fn prepare_runtime_launch(
    workspace: &Path,
    execution_policy: &str,
    codex_bin: &str,
    dry_run: bool,
) -> Result<(LaunchManifest, RuntimePaths)> {
    let health = health::run_health_check(workspace, None, 500)?;
    if health.has_blockers() {
        let codes = health
            .blockers
            .iter()
            .map(|finding| finding.code)
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("runtime preflight blocked: {codes}");
    }
    if !health.has_context {
        anyhow::bail!("runtime preflight blocked: missing_context");
    }
    ensure_clean_launch_worktree(workspace)?;

    let manifest = create_launch_manifest(workspace, execution_policy, codex_bin)?;
    let paths = write_launch_manifest(workspace, &manifest)?;

    append_log(
        &paths.log_path,
        &format!(
            "{} runtime start requested dry_run={dry_run}\n",
            Utc::now().to_rfc3339()
        ),
    )?;

    Ok((manifest, paths))
}

fn ensure_clean_launch_worktree(workspace: &Path) -> Result<()> {
    let git = GitRepo::open(workspace)?;
    if let WorktreeStatus::Dirty(files) = git.worktree_status()? {
        anyhow::bail!(
            "runtime preflight blocked: unexpected worktree changes before launch: {}",
            files.join(", ")
        );
    }
    Ok(())
}

fn spawn_runtime_child(
    workspace: &Path,
    paths: &RuntimePaths,
    manifest: &LaunchManifest,
    codex_bin: &str,
) -> Result<Child> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_path)
        .with_context(|| format!("failed to open {}", paths.log_path.display()))?;
    let err_log = log
        .try_clone()
        .context("failed to clone runtime log handle")?;

    let child_result = Command::new(codex_bin)
        .arg("exec")
        .args(&manifest.codex_args)
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err_log))
        .spawn();

    let child = match child_result {
        Ok(child) => child,
        Err(err) => {
            let message = format!("failed to launch {codex_bin} exec: {err}");
            let snapshot = RuntimeSnapshot {
                version: 1,
                status: "needs_human".to_string(),
                pid: None,
                started_at: None,
                stopped_at: Some(Utc::now().to_rfc3339()),
                launch_path: paths.launch_path.display().to_string(),
                runtime_path: paths.runtime_path.display().to_string(),
                log_path: paths.log_path.display().to_string(),
                last_error: Some(message.clone()),
                supervisor: Some(SupervisorStatus {
                    decision: "needs_human".to_string(),
                    reason: "spawn_failed".to_string(),
                    terminal_reason: "spawn_failed".to_string(),
                    should_continue: false,
                    restart_count: 0,
                    stagnation_count: 0,
                    last_signature: String::new(),
                    checked_at: Utc::now().to_rfc3339(),
                }),
            };
            write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
            append_log(
                &paths.log_path,
                &format!("{} {message}\n", Utc::now().to_rfc3339()),
            )?;
            anyhow::bail!(message);
        }
    };

    Ok(child)
}

fn write_runtime_prompt(child: &mut Child, manifest: &LaunchManifest) -> Result<()> {
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(manifest.prompt.as_bytes())
            .context("failed to write runtime prompt to codex stdin")?;
    }
    Ok(())
}

fn running_snapshot(paths: &RuntimePaths, pid: u32) -> RuntimeSnapshot {
    RuntimeSnapshot {
        version: 1,
        status: "running".to_string(),
        pid: Some(pid),
        started_at: Some(Utc::now().to_rfc3339()),
        stopped_at: None,
        launch_path: paths.launch_path.display().to_string(),
        runtime_path: paths.runtime_path.display().to_string(),
        log_path: paths.log_path.display().to_string(),
        last_error: None,
        supervisor: previous_supervisor(paths),
    }
}

fn previous_supervisor(paths: &RuntimePaths) -> Option<SupervisorStatus> {
    let snapshot: RuntimeSnapshot =
        serde_json::from_str(&fs::read_to_string(&paths.runtime_path).ok()?).ok()?;
    snapshot.supervisor
}

pub fn runtime_status(workspace: &Path) -> Result<RuntimeSnapshot> {
    let paths = paths(workspace);
    if !paths.runtime_path.exists() {
        anyhow::bail!("No runtime.json found at {}", paths.runtime_path.display());
    }
    let content = fs::read_to_string(&paths.runtime_path)?;
    let mut snapshot: RuntimeSnapshot = match serde_json::from_str(&content) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return Ok(needs_human_snapshot(
                &paths,
                "invalid_runtime_state",
                format!("failed to parse {}: {err}", paths.runtime_path.display()),
            ));
        }
    };
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

pub fn supervise_runtime(
    workspace: &Path,
    after_run: bool,
    max_stagnation: u32,
) -> Result<(RuntimeSnapshot, SupervisorStatus)> {
    let paths = paths(workspace);
    let mut snapshot = runtime_status(workspace)?;
    let state = read_state(&paths.state_path)?;
    let previous = snapshot.supervisor.clone();
    let signature = progress_signature(&state);

    let mut restart_count = previous.as_ref().map_or(0, |status| status.restart_count);
    let mut stagnation_count = previous
        .as_ref()
        .map_or(0, |status| status.stagnation_count);
    if after_run {
        if previous
            .as_ref()
            .is_some_and(|status| status.last_signature == signature)
        {
            stagnation_count += 1;
        } else {
            stagnation_count = 0;
        }
    }

    let (decision, reason, terminal_reason) = if snapshot.status == "needs_human" {
        (
            "needs_human".to_string(),
            snapshot
                .supervisor
                .as_ref()
                .map(|status| status.reason.clone())
                .unwrap_or_else(|| "runtime_needs_human".to_string()),
            snapshot
                .supervisor
                .as_ref()
                .map(|status| status.terminal_reason.clone())
                .unwrap_or_else(|| "runtime_needs_human".to_string()),
        )
    } else {
        supervisor_decision(&state, stagnation_count, max_stagnation)
    };
    if after_run && decision == "relaunch" {
        restart_count += 1;
    }

    let status = SupervisorStatus {
        should_continue: decision == "relaunch",
        decision,
        reason,
        terminal_reason,
        restart_count,
        stagnation_count,
        last_signature: signature,
        checked_at: Utc::now().to_rfc3339(),
    };

    match status.decision.as_str() {
        "stop" => {
            snapshot.status = "stopped".to_string();
            snapshot
                .stopped_at
                .get_or_insert_with(|| Utc::now().to_rfc3339());
        }
        "needs_human" => {
            snapshot.status = "needs_human".to_string();
            snapshot.last_error = Some(status.reason.clone());
        }
        _ => {}
    }
    snapshot.supervisor = Some(status.clone());
    write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
    Ok((snapshot, status))
}

pub fn stop_runtime(workspace: &Path) -> Result<RuntimeSnapshot> {
    let paths = paths(workspace);
    let mut snapshot = runtime_status(workspace)?;
    if snapshot.status == "needs_human" {
        return Ok(snapshot);
    }
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

fn needs_human_snapshot(paths: &RuntimePaths, reason: &str, error: String) -> RuntimeSnapshot {
    RuntimeSnapshot {
        version: 1,
        status: "needs_human".to_string(),
        pid: None,
        started_at: None,
        stopped_at: None,
        launch_path: paths.launch_path.display().to_string(),
        runtime_path: paths.runtime_path.display().to_string(),
        log_path: paths.log_path.display().to_string(),
        last_error: Some(error),
        supervisor: Some(SupervisorStatus {
            decision: "needs_human".to_string(),
            reason: reason.to_string(),
            terminal_reason: reason.to_string(),
            should_continue: false,
            restart_count: 0,
            stagnation_count: 0,
            last_signature: String::new(),
            checked_at: Utc::now().to_rfc3339(),
        }),
    }
}

fn mark_restart_cap(
    workspace: &Path,
    previous: SupervisorStatus,
) -> Result<(RuntimeSnapshot, SupervisorStatus)> {
    let paths = paths(workspace);
    let mut snapshot = runtime_status(workspace)?;
    let status = SupervisorStatus {
        decision: "needs_human".to_string(),
        reason: "restart_cap".to_string(),
        terminal_reason: "restart_cap".to_string(),
        should_continue: false,
        restart_count: previous.restart_count,
        stagnation_count: previous.stagnation_count,
        last_signature: previous.last_signature,
        checked_at: Utc::now().to_rfc3339(),
    };

    snapshot.status = "needs_human".to_string();
    snapshot.last_error = Some("restart_cap".to_string());
    snapshot.supervisor = Some(status.clone());
    write_runtime_snapshot(&paths.runtime_path, &snapshot)?;
    append_log(
        &paths.log_path,
        &format!("{} runtime restart cap reached\n", Utc::now().to_rfc3339()),
    )?;
    Ok((snapshot, status))
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

fn supervisor_decision(
    state: &RunState,
    stagnation_count: u32,
    max_stagnation: u32,
) -> (String, String, String) {
    match &state.phase {
        RunPhase::Complete { reason } => {
            return (
                "stop".to_string(),
                "run_complete".to_string(),
                stop_reason_label(reason).to_string(),
            );
        }
        RunPhase::Blocked { .. } => {
            return (
                "needs_human".to_string(),
                "blocked".to_string(),
                "blocked".to_string(),
            );
        }
        _ => {}
    }

    if state.pivot_count >= 3 {
        return (
            "needs_human".to_string(),
            "soft_blocked".to_string(),
            "soft_blocker".to_string(),
        );
    }

    if stagnation_count >= max_stagnation {
        return (
            "needs_human".to_string(),
            "stagnated".to_string(),
            "stagnated".to_string(),
        );
    }

    if state
        .config
        .as_ref()
        .and_then(|config| config.iterations)
        .is_some_and(|cap| state.iteration >= cap)
    {
        return (
            "stop".to_string(),
            "iteration_cap".to_string(),
            "iteration_cap".to_string(),
        );
    }

    if acceptance_satisfied(state) {
        return (
            "stop".to_string(),
            "acceptance_criteria".to_string(),
            "goal_reached".to_string(),
        );
    }

    if simple_stop_condition_satisfied(state) {
        return (
            "stop".to_string(),
            "stop_condition".to_string(),
            "goal_reached".to_string(),
        );
    }

    (
        "relaunch".to_string(),
        "non_terminal".to_string(),
        "none".to_string(),
    )
}

fn acceptance_satisfied(state: &RunState) -> bool {
    let Some(config) = &state.config else {
        return false;
    };
    if config.acceptance_criteria.is_empty() {
        return false;
    }
    let primary_key = config.primary_metric_key.as_deref().unwrap_or("metric");
    let mut metrics = BTreeMap::new();
    metrics.insert(primary_key.to_string(), state.current_metric);
    metrics.insert("metric".to_string(), state.current_metric);
    evaluate_criteria(&config.acceptance_criteria, &metrics).satisfied
}

fn simple_stop_condition_satisfied(state: &RunState) -> bool {
    let Some(config) = &state.config else {
        return false;
    };
    let Some(stop_condition) = &config.stop_condition else {
        return false;
    };
    let Ok(operator_pattern) = Regex::new(r"(<=|>=|==|<|>)\s*(-?(?:\d+(?:\.\d+)?|\.\d+))") else {
        return false;
    };
    if let Some(captures) = operator_pattern.captures(stop_condition) {
        let operator = captures.get(1).map(|m| m.as_str());
        let Some(target) = captures
            .get(2)
            .and_then(|m| Decimal::from_str(m.as_str()).ok())
        else {
            return false;
        };

        return match operator {
            Some("<") => state.current_metric < target,
            Some("<=") => state.current_metric <= target,
            Some(">") => state.current_metric > target,
            Some(">=") => state.current_metric >= target,
            Some("==") => state.current_metric == target,
            _ => false,
        };
    }

    let Ok(number_pattern) = Regex::new(r"-?(?:\d+(?:\.\d+)?|\.\d+)") else {
        return false;
    };
    let Some(target) = number_pattern
        .find_iter(stop_condition)
        .last()
        .and_then(|m| Decimal::from_str(m.as_str()).ok())
    else {
        return false;
    };

    match config.direction {
        super::config::Direction::Lower => state.current_metric <= target,
        super::config::Direction::Higher => state.current_metric >= target,
    }
}

fn progress_signature(state: &RunState) -> String {
    serde_json::json!({
        "iteration": state.iteration,
        "last_status": state.last_status.as_str(),
        "last_trial_commit": state.last_trial_commit,
        "last_trial_metric": state.last_trial_metric.map(|metric| metric.to_string()),
    })
    .to_string()
}

fn stop_reason_label(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::GoalReached => "goal_reached",
        StopReason::IterationCap => "iteration_cap",
        StopReason::UserInterrupt => "user_interrupt",
        StopReason::SoftBlocker => "soft_blocker",
        StopReason::HardBlocker(_) => "hard_blocker",
    }
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
