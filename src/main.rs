use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use autoresearch::core::config::{Direction, RollbackStrategy, RunConfig, RunMode, VerifyFormat};
use autoresearch::core::context;
use autoresearch::core::criteria;
use autoresearch::core::git::{GitRepo, WorktreeStatus};
use autoresearch::core::health;
use autoresearch::core::results::{
    ensure_results_dir_protected, GuardResult, ResultRow, ResultsLog,
};
use autoresearch::core::runtime;
use autoresearch::core::state::{IterationStatus, RunPhase, RunState};
use autoresearch::core::verify;
use autoresearch::escalation::lessons::{self, LessonsLog};
use autoresearch::escalation::pivot::{EscalationAction, EscalationState};
use autoresearch::hooks;
use autoresearch::modes::evals::{parse_results_tsv, ParsedRow};

#[derive(Parser)]
#[command(
    name = "autoresearch",
    about = "Autonomous goal-directed iteration engine for coding agents",
    version,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new run: measure baseline, create results dir, write state
    Init {
        /// Verify command to establish baseline
        #[arg(long)]
        verify: String,
        /// Metric direction: higher or lower
        #[arg(long, default_value = "higher")]
        direction: String,
        /// Verify output format: scalar or metrics_json
        #[arg(long, default_value = "scalar")]
        format: String,
        /// Primary metric key (for metrics_json)
        #[arg(long)]
        key: Option<String>,
        /// Goal description
        #[arg(long)]
        goal: Option<String>,
        /// Scope glob patterns (repeatable)
        #[arg(long)]
        scope: Option<Vec<String>>,
        /// Metric description
        #[arg(long)]
        metric: Option<String>,
        /// Guard command
        #[arg(long)]
        guard: Option<String>,
        /// Acceptance criteria JSON array
        #[arg(long)]
        acceptance_criteria: Option<String>,
        /// Required keep criteria JSON array
        #[arg(long)]
        required_keep_criteria: Option<String>,
        /// Required label before an improved trial can be retained
        #[arg(long)]
        required_keep_label: Vec<String>,
        /// Required retained label before a stop condition can end the run
        #[arg(long)]
        required_stop_label: Vec<String>,
        /// Iteration cap
        #[arg(long)]
        iterations: Option<u32>,
        /// Run tag for grouping artifacts/lessons
        #[arg(long)]
        run_tag: Option<String>,
        /// Stop condition description
        #[arg(long)]
        stop_condition: Option<String>,
        /// Run mode: foreground or background
        #[arg(long)]
        run_mode: Option<String>,
        /// Workspace root for run artifacts
        #[arg(long)]
        workspace_root: Option<PathBuf>,
        /// Primary repository for scoped runs
        #[arg(long)]
        primary_repo: Option<PathBuf>,
        /// Rollback strategy: revert or hard-reset
        #[arg(long, default_value = "revert")]
        rollback: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Run a verify command and return the metric as JSON
    Verify {
        /// Command to run
        #[arg(long)]
        command: String,
        /// Output format: scalar or metrics_json
        #[arg(long, default_value = "scalar")]
        format: String,
        /// Primary metric key (for metrics_json)
        #[arg(long)]
        key: Option<String>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Run a guard command and return pass/fail as JSON
    Guard {
        /// Command to run
        #[arg(long)]
        command: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Record an iteration: append TSV row + update state.json
    Log {
        /// Iteration number
        #[arg(long)]
        iteration: u32,
        /// Commit hash (or "-" for discards)
        #[arg(long, default_value = "-")]
        commit: String,
        /// Metric value
        #[arg(long, allow_hyphen_values = true)]
        metric: String,
        /// Delta from previous
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        delta: String,
        /// Guard result: pass, fail, or skip
        #[arg(long, default_value = "skip")]
        guard: String,
        /// Status: keep, discard, crash, no-op, baseline, blocked, pivot, refine
        #[arg(long)]
        status: String,
        /// Description
        #[arg(long)]
        description: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Apply keep/discard decision: update state, revert if needed, track escalation
    Decide {
        /// Decision: auto, keep, discard, crash, no-op, blocked
        #[arg(long, default_value = "auto")]
        decision: String,
        /// Trial metric value. Required for auto, keep, and discard decisions.
        #[arg(long, allow_hyphen_values = true)]
        metric: Option<String>,
        /// Full metrics JSON object for criteria checks
        #[arg(long)]
        metrics_json: Option<String>,
        /// Commit hash of the trial
        #[arg(long)]
        commit: Option<String>,
        /// Description of the change
        #[arg(long)]
        description: String,
        /// Rollback strategy: revert or hard-reset
        #[arg(long, default_value = "revert")]
        rollback: String,
        /// Guard result: pass, fail, or skip
        #[arg(long, default_value = "skip")]
        guard: String,
        /// Structured label attached to this trial
        #[arg(long)]
        label: Vec<String>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Analyze a results TSV: trends, plateaus, efficiency, recommendations
    Evals {
        /// Path to results.tsv (auto-detected if omitted)
        path: Option<PathBuf>,
        /// Output format: text, json, or md
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Show current run status from state.json
    Status {
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Check runtime health: git state, artifacts, disk, and verify command
    Health {
        /// Verify command to check; defaults to state.json config when present
        #[arg(long)]
        verify: Option<String>,
        /// Minimum free disk space in MB
        #[arg(long, default_value_t = 500)]
        min_free_mb: u64,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Manage background runtime artifacts and detached Codex sessions
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommands,
    },

    /// Close out parallel experiment worker batches
    Parallel {
        #[command(subcommand)]
        command: ParallelCommands,
    },

    /// Screen a command for dangerous patterns
    Screen {
        /// Command to screen
        #[arg(long)]
        command: String,
    },

    /// Run a hook (called by Claude Code plugin system)
    Hook {
        /// Hook name
        name: String,
    },

    /// Detect if an interrupted run exists and return its state
    Resume {
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate a mid-run progress summary
    Progress {
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Query the lessons.md file for relevant strategies
    Lessons {
        /// Filter lessons containing this query (case-insensitive)
        #[arg(long)]
        search: Option<String>,
        /// Return last N lessons
        #[arg(long)]
        last: Option<usize>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Write a chain handoff.json file for downstream commands
    Handoff {
        /// Source mode (e.g. iterate, search, refine)
        #[arg(long)]
        source: String,
        /// Status: COMPLETE, GOAL_MET, BOUNDED, BLOCKED, ERROR
        #[arg(long)]
        status: String,
        /// Findings as JSON array string
        #[arg(long)]
        findings: Option<String>,
        /// Config as JSON object string
        #[arg(long)]
        config: Option<String>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Non-interactive CI/CD mode: read config from stdin, run loop, emit JSON lines
    Exec {
        /// Maximum iterations (required in exec mode)
        #[arg(long)]
        iterations: u32,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RuntimeCommands {
    /// Create launch/runtime artifacts and optionally spawn `codex exec`
    Start {
        /// Execution policy for nested Codex sessions
        #[arg(long, default_value = "danger_full_access")]
        execution_policy: String,
        /// Codex binary to launch
        #[arg(long, default_value = "codex")]
        codex_bin: String,
        /// Write launch/runtime artifacts without spawning Codex
        #[arg(long)]
        dry_run: bool,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Read runtime.json and report status
    Status {
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Recommend relaunch, stop, or needs_human from runtime/state artifacts
    Supervise {
        /// Treat this check as happening after a detached Codex turn finished
        #[arg(long)]
        after_run: bool,
        /// Consecutive no-progress exits tolerated before needs_human
        #[arg(long, default_value_t = 3)]
        max_stagnation: u32,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Run supervised Codex exec turns until stop, needs_human, or restart cap
    Run {
        /// Execution policy for nested Codex sessions
        #[arg(long, default_value = "danger_full_access")]
        execution_policy: String,
        /// Codex binary to launch
        #[arg(long, default_value = "codex")]
        codex_bin: String,
        /// Relaunches allowed after the first Codex turn
        #[arg(long, default_value_t = 25)]
        max_restarts: u32,
        /// Consecutive no-progress exits tolerated before needs_human
        #[arg(long, default_value_t = 3)]
        max_stagnation: u32,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Stop the recorded runtime process when one is running
    Stop {
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ParallelCommands {
    /// Select the best worker result and record the batch as one authoritative iteration
    Closeout {
        /// JSON array of worker results
        #[arg(long)]
        batch_file: PathBuf,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            verify: verify_cmd,
            direction,
            format,
            key,
            goal,
            scope,
            metric,
            guard,
            acceptance_criteria,
            required_keep_criteria,
            required_keep_label,
            required_stop_label,
            iterations,
            run_tag,
            stop_condition,
            run_mode,
            workspace_root,
            primary_repo,
            rollback,
            cwd,
        } => cmd_init(
            &verify_cmd,
            &direction,
            &format,
            key.as_deref(),
            goal.as_deref(),
            scope,
            metric.as_deref(),
            guard.as_deref(),
            acceptance_criteria,
            required_keep_criteria,
            required_keep_label,
            required_stop_label,
            iterations,
            run_tag,
            stop_condition,
            run_mode,
            workspace_root,
            primary_repo,
            &rollback,
            cwd,
        ),

        Commands::Verify {
            command,
            format,
            key,
            cwd,
        } => cmd_verify(&command, &format, key.as_deref(), cwd),

        Commands::Guard { command, cwd } => cmd_guard(&command, cwd),

        Commands::Log {
            iteration,
            commit,
            metric,
            delta,
            guard,
            status,
            description,
            cwd,
        } => cmd_log(
            iteration,
            &commit,
            &metric,
            &delta,
            &guard,
            &status,
            &description,
            cwd,
        ),

        Commands::Decide {
            decision,
            metric,
            metrics_json,
            commit,
            description,
            rollback,
            guard,
            label,
            cwd,
        } => cmd_decide(
            &decision,
            metric.as_deref(),
            metrics_json.as_deref(),
            commit.as_deref(),
            &description,
            &rollback,
            &guard,
            label,
            cwd,
        ),

        Commands::Evals { path, format } => cmd_evals(path, &format),

        Commands::Status { cwd } => cmd_status(cwd),

        Commands::Health {
            verify,
            min_free_mb,
            cwd,
        } => cmd_health(verify.as_deref(), min_free_mb, cwd),

        Commands::Runtime { command } => cmd_runtime(command),

        Commands::Parallel { command } => cmd_parallel(command),

        Commands::Screen { command } => cmd_screen(&command),

        Commands::Hook { name } => hooks::dispatch(&name),

        Commands::Resume { cwd } => cmd_resume(cwd),

        Commands::Progress { cwd } => cmd_progress(cwd),

        Commands::Lessons { search, last, cwd } => cmd_lessons(search.as_deref(), last, cwd),

        Commands::Handoff {
            source,
            status,
            findings,
            config,
            cwd,
        } => cmd_handoff(
            &source,
            &status,
            findings.as_deref(),
            config.as_deref(),
            cwd,
        ),

        Commands::Exec { iterations, cwd } => cmd_exec(iterations, cwd),
    }
}

// ── Init ──────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn cmd_init(
    verify_cmd: &str,
    direction_str: &str,
    format_str: &str,
    key: Option<&str>,
    goal: Option<&str>,
    scope: Option<Vec<String>>,
    metric_desc: Option<&str>,
    guard: Option<&str>,
    acceptance_criteria_raw: Option<String>,
    required_keep_criteria_raw: Option<String>,
    required_keep_label: Vec<String>,
    required_stop_label: Vec<String>,
    iterations: Option<u32>,
    run_tag: Option<String>,
    stop_condition: Option<String>,
    run_mode: Option<String>,
    workspace_root: Option<PathBuf>,
    primary_repo: Option<PathBuf>,
    rollback: &str,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let direction = parse_direction(direction_str)?;
    let fmt = parse_format(format_str);
    let run_mode = run_mode
        .as_deref()
        .map(parse_run_mode)
        .transpose()
        .context("Invalid run mode")?;
    let rollback_strategy = parse_rollback_strategy(rollback)?;
    let acceptance_criteria =
        criteria::parse_criteria_json(acceptance_criteria_raw.as_deref(), "acceptance_criteria")?;
    let required_keep_criteria = criteria::parse_criteria_json(
        required_keep_criteria_raw.as_deref(),
        "required_keep_criteria",
    )?;
    let required_keep_labels = normalize_labels(required_keep_label);
    let required_stop_labels = normalize_labels(required_stop_label);

    // Safety screen
    verify::screen_command(verify_cmd)?;
    if let Some(guard_cmd) = guard {
        verify::screen_command(guard_cmd)?;
    }

    // Verify git repo
    let git = GitRepo::open(&workspace).context("autoresearch requires a git repository")?;
    let lock_files = git.lock_files();
    if !lock_files.is_empty() {
        anyhow::bail!(
            "init preflight blocked: stale git lock files found: {}",
            lock_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if git.head_detached()? {
        anyhow::bail!("init preflight blocked: detached_head");
    }
    let staged_artifacts = git.staged_owned_artifacts()?;
    if !staged_artifacts.is_empty() {
        anyhow::bail!(
            "init preflight blocked: autoresearch-owned artifacts are staged: {}",
            staged_artifacts.join(", ")
        );
    }
    if let WorktreeStatus::Dirty(files) = git.worktree_status()? {
        anyhow::bail!(
            "init preflight blocked: unexpected worktree changes before launch: {}",
            files.join(", ")
        );
    }
    let head = git.head_short()?;

    // Measure baseline
    let result = verify::run_verify(verify_cmd, fmt, key, &workspace)
        .context("Baseline verification failed")?;
    if fmt == VerifyFormat::MetricsJson {
        let metrics = result
            .metrics
            .as_ref()
            .context("verify_format=metrics_json requires structured baseline metrics")?;
        let primary_metric_key = key
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("metric");
        ensure_metrics_json_keys(
            metrics,
            primary_metric_key,
            &acceptance_criteria,
            &required_keep_criteria,
        )?;
    }

    // Create results directory + protect from git staging
    let results_dir = ensure_results_dir_protected(&workspace)?;

    // Write TSV with header + baseline row
    let log = ResultsLog::create(&results_dir, direction)?;
    let baseline_row = ResultRow {
        iteration: 0,
        commit: Some(head.clone()),
        metric: result.metric,
        delta: Decimal::ZERO,
        guard: GuardResult::Skip,
        status: IterationStatus::Baseline,
        description: "initial state".to_string(),
    };
    log.append(&baseline_row)?;

    // Build run config from init parameters
    let run_config = RunConfig {
        goal: goal.unwrap_or("").to_string(),
        scope: scope.unwrap_or_default(),
        metric: metric_desc.unwrap_or("").to_string(),
        direction,
        verify: verify_cmd.to_string(),
        guard: guard.map(|g| g.to_string()),
        iterations,
        run_tag,
        stop_condition,
        verify_format: fmt,
        primary_metric_key: key.map(|k| k.to_string()),
        acceptance_criteria,
        required_keep_criteria,
        required_keep_labels,
        required_stop_labels,
        rollback_strategy,
        run_mode,
        workspace_root,
        primary_repo,
    };
    let acceptance_criteria_count = run_config.acceptance_criteria.len();
    let required_keep_criteria_count = run_config.required_keep_criteria.len();
    let required_keep_labels_count = run_config.required_keep_labels.len();
    let required_stop_labels_count = run_config.required_stop_labels.len();

    // Write state.json
    let mut state = RunState::from_baseline(result.metric, head.clone(), Some(run_config));
    if let Some(metrics) = result.metrics.clone() {
        state.set_current_metrics(metrics);
    }
    let state_json = serde_json::to_string_pretty(&state)?;
    std::fs::write(results_dir.join("state.json"), &state_json)?;
    let context_path = context::write_context(&workspace, state.config.as_ref())?;

    // Initialize lessons.md
    LessonsLog::open_or_create(&results_dir)?;

    // Output
    let out = serde_json::json!({
        "status": "ok",
        "baseline_metric": result.metric.to_string(),
        "baseline_commit": head,
        "direction": direction_str,
        "iterations": iterations,
        "acceptance_criteria_count": acceptance_criteria_count,
        "required_keep_criteria_count": required_keep_criteria_count,
        "required_keep_labels_count": required_keep_labels_count,
        "required_stop_labels_count": required_stop_labels_count,
        "run_mode": run_mode.map(|mode| match mode {
            RunMode::Foreground => "foreground",
            RunMode::Background => "background",
        }),
        "results_dir": results_dir.display().to_string(),
        "context_path": context_path.display().to_string(),
        "verify_duration_ms": result.duration.as_millis(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

// ── Verify ────────────────────────────────────────────────────────────

fn cmd_verify(
    command: &str,
    format_str: &str,
    key: Option<&str>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_cwd(cwd);
    let fmt = parse_format(format_str);

    verify::screen_command(command)?;
    let result = verify::run_verify(command, fmt, key, &workspace)?;

    let out = serde_json::json!({
        "metric": result.metric.to_string(),
        "metrics": result.metrics.as_ref().map(|metrics| {
            metrics
                .iter()
                .map(|(key, value)| (key.clone(), value.to_string()))
                .collect::<std::collections::BTreeMap<_, _>>()
        }),
        "exit_code": result.exit_code,
        "duration_ms": result.duration.as_millis(),
        "stdout_tail": result.stdout.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
        "stderr_tail": result.stderr.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}

// ── Guard ─────────────────────────────────────────────────────────────

fn cmd_guard(command: &str, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_cwd(cwd);
    verify::screen_command(command)?;
    let result = verify::run_guard(command, &workspace)?;

    let out = serde_json::json!({
        "passed": result.passed,
        "duration_ms": result.duration.as_millis(),
        "stdout_tail": result.stdout.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
        "stderr_tail": result.stderr.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}

// ── Log ───────────────────────────────────────────────────────────────
// NOTE: `decide` is the primary closeout path for iterations.
// `log` is the low-level escape hatch for manually recording rows.

#[allow(clippy::too_many_arguments)]
fn cmd_log(
    iteration: u32,
    commit: &str,
    metric_str: &str,
    delta_str: &str,
    guard_str: &str,
    status_str: &str,
    description: &str,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");

    // Validate iteration sequence to prevent double-counting with cmd_decide
    let state_path = results_dir.join("state.json");
    if state_path.exists() {
        let content = std::fs::read_to_string(&state_path)?;
        let existing: RunState = serde_json::from_str(&content)?;
        let expected = existing.iteration + 1;
        if iteration != expected {
            anyhow::bail!(
                "Iteration mismatch: requested {iteration} but state expects {expected}. \
                 Use `decide` for normal closeout; `log` is the low-level escape hatch."
            );
        }
    }

    let metric =
        Decimal::from_str(metric_str).with_context(|| format!("Invalid metric: {metric_str}"))?;
    let delta = Decimal::from_str(delta_str.trim_start_matches('+'))
        .with_context(|| format!("Invalid delta: {delta_str}"))?;

    let commit_val = if commit == "-" {
        None
    } else {
        Some(commit.to_string())
    };

    let guard = match guard_str {
        "pass" => GuardResult::Pass,
        "fail" => GuardResult::Fail,
        _ => GuardResult::Skip,
    };

    let status = parse_status(status_str)?;

    let row = ResultRow {
        iteration,
        commit: commit_val.clone(),
        metric,
        delta,
        guard,
        status,
        description: description.to_string(),
    };

    let log = ResultsLog::open(results_dir.join("results.tsv"))?;
    log.append(&row)?;

    // Update state.json
    let state_path = results_dir.join("state.json");
    if state_path.exists() {
        let content = std::fs::read_to_string(&state_path)?;
        let mut state: RunState = serde_json::from_str(&content)?;

        match status {
            IterationStatus::Keep => {
                state.record_keep(metric, commit.to_string());
            }
            IterationStatus::Discard => {
                state.record_discard(metric, commit_val);
            }
            IterationStatus::Crash => {
                state.record_crash();
            }
            IterationStatus::NoOp => {
                state.record_no_op();
            }
            IterationStatus::Blocked => {
                state.record_blocked(description.to_string());
            }
            _ => {} // baseline, pivot, refine, search — state updated by decide
        }

        std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
    }

    println!(r#"{{"status":"ok","iteration":{iteration}}}"#);
    Ok(())
}

// ── Decide ────────────────────────────────────────────────────────────

fn cmd_decide(
    decision: &str,
    metric_str: Option<&str>,
    metrics_json: Option<&str>,
    commit: Option<&str>,
    description: &str,
    rollback_str: &str,
    guard_str: &str,
    labels: Vec<String>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");

    // Parse guard result
    let guard = match guard_str {
        "pass" => GuardResult::Pass,
        "fail" => GuardResult::Fail,
        _ => GuardResult::Skip,
    };

    // Load state
    let content = std::fs::read_to_string(&state_path)
        .context("No state.json found — run `autoresearch init` first")?;
    let mut state: RunState = serde_json::from_str(&content)?;
    let metric = parse_decide_metric(metric_str, decision, state.current_metric)?;
    let (
        primary_metric_key,
        verify_format,
        acceptance_criteria,
        required_keep_criteria,
        required_keep_labels,
    ) = match state.config.as_ref() {
        Some(config) => (
            config
                .primary_metric_key
                .clone()
                .or_else(|| {
                    if config.metric.trim().is_empty() {
                        None
                    } else {
                        Some(config.metric.clone())
                    }
                })
                .unwrap_or_else(|| "metric".to_string()),
            config.verify_format,
            config.acceptance_criteria.clone(),
            config.required_keep_criteria.clone(),
            config.required_keep_labels.clone(),
        ),
        None => (
            "metric".to_string(),
            VerifyFormat::Scalar,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    };

    let delta = metric - state.current_metric;
    let requires_trial_metrics = !matches!(decision, "blocked" | "crash" | "no-op");
    let trial_metrics = if requires_trial_metrics {
        build_trial_metrics(metric, metrics_json, &primary_metric_key, verify_format)?
    } else {
        retained_trial_metrics(&state, metric, &primary_metric_key)
    };
    if requires_trial_metrics && verify_format == VerifyFormat::MetricsJson {
        ensure_metrics_json_keys(
            &trial_metrics,
            &primary_metric_key,
            &acceptance_criteria,
            &required_keep_criteria,
        )?;
    }
    let acceptance = criteria::evaluate_criteria(&acceptance_criteria, &trial_metrics);
    let required_keep = criteria::evaluate_criteria(&required_keep_criteria, &trial_metrics);
    let trial_labels = normalize_labels(labels);
    let decision = if decision == "auto" {
        if state.direction.is_improvement(delta) {
            "keep"
        } else {
            "discard"
        }
    } else {
        decision
    };
    let missing_required_keep_labels = if decision == "keep" {
        missing_required_labels(&required_keep_labels, &trial_labels)
    } else {
        Vec::new()
    };
    let required_keep_labels_satisfied = missing_required_keep_labels.is_empty();
    let decision =
        if decision == "keep" && (!required_keep.satisfied || !required_keep_labels_satisfied) {
            "discard"
        } else if guard == GuardResult::Fail && decision == "keep" {
            "discard"
        } else {
            decision
        };

    // Load escalation state
    let esc_path = results_dir.join("escalation.json");
    let mut escalation: EscalationState = if esc_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&esc_path)?)?
    } else {
        EscalationState::default()
    };

    // Load lessons
    let lessons_log = LessonsLog::open_or_create(&results_dir)?;

    let git = GitRepo::open(&workspace)?;
    let iteration = state.iteration + 1;

    // Resolve the actual commit for keep decisions (Bug 4 fix)
    let resolved_commit: Option<String> = if decision == "keep" {
        Some(match commit {
            Some(c) => c.to_string(),
            None => git.head_short()?,
        })
    } else {
        commit.map(|s| s.to_string())
    };

    let (status, needs_rollback, escalation_action) = match decision {
        "keep" => {
            state.record_keep_with_metrics_and_labels(
                metric,
                resolved_commit.clone().unwrap(),
                trial_metrics.clone(),
                trial_labels.clone(),
            );
            escalation.record_keep();

            // Extract positive lesson
            let lesson = lessons::extract_keep_lesson(
                description,
                &autoresearch::core::metrics::format_delta(delta),
            );
            let _ = lessons_log.append(&lesson);

            (IterationStatus::Keep, false, None)
        }
        "discard" => {
            state.record_discard_with_metrics_and_labels(
                metric,
                resolved_commit.clone(),
                trial_metrics.clone(),
                trial_labels.clone(),
            );
            let action = escalation.record_discard();
            (IterationStatus::Discard, true, Some(action))
        }
        "crash" => {
            state.record_crash();
            let action = escalation.record_crash();
            (IterationStatus::Crash, true, Some(action))
        }
        "no-op" => {
            state.record_no_op();
            let action = escalation.record_no_op();
            (IterationStatus::NoOp, false, Some(action))
        }
        "blocked" => {
            state.record_blocked(description.to_string());
            (IterationStatus::Blocked, false, None)
        }
        other => {
            anyhow::bail!("Unknown decision: {other}. Use keep, discard, crash, no-op, or blocked.")
        }
    };
    if escalation_action == Some(EscalationAction::Pivot) {
        let lesson = lessons::extract_pivot_lesson(description, EscalationAction::Pivot.guidance());
        let _ = lessons_log.append(&lesson);
        escalation.acknowledge_pivot();
        state.pivot_count = escalation.pivot_count;
        state.consecutive_discards = escalation.consecutive_discards;
    }

    // Apply rollback if needed
    if needs_rollback {
        if let Some(expected_commit) = resolved_commit.as_deref() {
            if !git.head_matches(expected_commit)? {
                anyhow::bail!(
                    "Refusing rollback: trial commit {expected_commit} is not current HEAD"
                );
            }
        }
        let head_summary = git.head_summary()?;
        if !head_summary.starts_with("experiment:") {
            anyhow::bail!(
                "Refusing rollback: current HEAD is not an experiment commit ({head_summary:?})"
            );
        }

        let strategy = match rollback_str {
            "hard-reset" => RollbackStrategy::HardReset,
            _ => RollbackStrategy::Revert,
        };
        match strategy {
            RollbackStrategy::HardReset => {
                if let Err(e) = git.hard_reset_head() {
                    eprintln!("Hard reset failed: {e}. Falling back to revert.");
                    git.revert_head()?;
                }
            }
            RollbackStrategy::Revert => {
                git.revert_head()?;
            }
        }
        // Update state to reflect actual HEAD after rollback
        state.last_commit = git.head_short()?;
    }

    // Append to TSV
    let log = ResultsLog::open(results_dir.join("results.tsv"))?;
    log.append(&ResultRow {
        iteration,
        commit: if status == IterationStatus::Keep {
            resolved_commit.clone()
        } else {
            None
        },
        metric,
        delta,
        guard,
        status,
        description: label_description(description, &trial_labels, &missing_required_keep_labels),
    })?;

    // Persist state + escalation
    std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
    std::fs::write(&esc_path, serde_json::to_string_pretty(&escalation)?)?;

    // Build response
    let escalation_guidance = escalation_action.map(|a| {
        serde_json::json!({
            "action": format!("{:?}", a),
            "guidance": a.guidance(),
            "is_terminal": a.is_terminal(),
        })
    });

    let out = serde_json::json!({
        "status": "ok",
        "decision": decision,
        "iteration": iteration,
        "metric": metric.to_string(),
        "delta": autoresearch::core::metrics::format_delta(delta),
        "current_metric": state.current_metric.to_string(),
        "best_metric": state.best_metric.to_string(),
        "best_iteration": state.best_iteration,
        "keeps": state.keeps,
        "discards": state.discards,
        "crashes": state.crashes,
        "consecutive_discards": state.consecutive_discards,
        "rollback_applied": needs_rollback,
        "acceptance": acceptance,
        "required_keep": required_keep,
        "required_keep_labels": {
            "satisfied": required_keep_labels_satisfied,
            "missing": missing_required_keep_labels,
        },
        "escalation": escalation_guidance,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

// ── Evals ─────────────────────────────────────────────────────────────

fn cmd_evals(path: Option<PathBuf>, format: &str) -> Result<()> {
    let tsv_path = match path {
        Some(p) => p,
        None => {
            let cwd = std::env::current_dir()?;
            default_results_tsv(&cwd)
                .context("No results.tsv found. Provide a path or run inside a git repo.")?
        }
    };

    let content = std::fs::read_to_string(&tsv_path)
        .with_context(|| format!("Cannot read {}", tsv_path.display()))?;

    // Parse direction from header
    let direction = content
        .lines()
        .find(|l| l.starts_with("# metric_direction:"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim())
        .unwrap_or("higher");

    // Parse data rows
    let rows: Vec<&str> = content
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("iteration\t") && !l.is_empty())
        .filter(|l| {
            l.split('\t')
                .next()
                .is_some_and(|iteration| iteration.parse::<u32>().is_ok())
        })
        .collect();

    if rows.is_empty() {
        anyhow::bail!("No data rows in results TSV.");
    }

    // Parse metrics from each row
    let mut metrics: Vec<(u32, &str, Decimal, &str)> = Vec::new(); // (iter, status, metric, desc)
    for row in &rows {
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() < 7 {
            continue;
        }
        let iter: u32 = cols[0].parse().unwrap_or(0);
        let metric = Decimal::from_str(cols[2]).unwrap_or_default();
        let status = cols[5];
        let desc = cols[6];
        metrics.push((iter, status, metric, desc));
    }

    let total = metrics.len();
    let has_baseline = metrics
        .first()
        .is_some_and(|(iteration, status, _, _)| *iteration == 0 || *status == "baseline");
    let total_iterations = total.saturating_sub(usize::from(has_baseline));
    let keeps = metrics.iter().filter(|m| m.1 == "keep").count();
    let discards = metrics.iter().filter(|m| m.1 == "discard").count();
    let crashes = metrics.iter().filter(|m| m.1 == "crash").count();
    let baseline = metrics.first().map(|m| m.2).unwrap_or_default();
    let final_metric = metrics.last().map(|m| m.2).unwrap_or_default();
    let best = if direction == "higher" {
        metrics.iter().map(|m| m.2).max().unwrap_or_default()
    } else {
        metrics.iter().map(|m| m.2).min().unwrap_or_default()
    };

    // Find longest plateau (consecutive non-keep)
    let mut longest_plateau = 0u32;
    let mut current_plateau = 0u32;
    for m in &metrics {
        if m.1 != "keep" && m.1 != "baseline" {
            current_plateau += 1;
            longest_plateau = longest_plateau.max(current_plateau);
        } else {
            current_plateau = 0;
        }
    }

    // Top improvements (keeps sorted by absolute delta)
    let keep_rows: Vec<&str> = rows
        .iter()
        .filter(|r| r.contains("\tkeep\t"))
        .copied()
        .collect();
    let mut top_keeps: Vec<(Decimal, &str)> = keep_rows
        .iter()
        .filter_map(|row| {
            let cols: Vec<&str> = row.split('\t').collect();
            if cols.len() >= 7 {
                let delta = Decimal::from_str(cols[3].trim_start_matches('+')).ok()?;
                Some((delta.abs(), cols[6]))
            } else {
                None
            }
        })
        .collect();
    top_keeps.sort_by_key(|entry| std::cmp::Reverse(entry.0));

    let efficiency = if total_iterations > 0 {
        (keeps as f64 / total_iterations as f64 * 100.0).round() as u32
    } else {
        0
    };

    // Determine trend from last 5 keeps
    let recent_keeps: Vec<Decimal> = metrics
        .iter()
        .filter(|m| m.1 == "keep")
        .rev()
        .take(5)
        .map(|m| m.2)
        .collect();
    let trend = if recent_keeps.len() < 2 {
        "insufficient data"
    } else {
        match direction {
            "lower" if recent_keeps.windows(2).all(|w| w[0] <= w[1]) => "improving",
            "lower" if recent_keeps.windows(2).all(|w| w[0] >= w[1]) => "declining",
            _ if recent_keeps.windows(2).all(|w| w[0] >= w[1]) => "improving",
            _ if recent_keeps.windows(2).all(|w| w[0] <= w[1]) => "declining",
            _ => "flat",
        }
    };
    let summary_dir = tsv_path.parent().unwrap_or_else(|| Path::new("."));

    match format {
        "json" => {
            let out = serde_json::json!({
                "direction": direction,
                "total_iterations": total_iterations,
                "keeps": keeps,
                "discards": discards,
                "crashes": crashes,
                "baseline": baseline.to_string(),
                "final": final_metric.to_string(),
                "best": best.to_string(),
                "efficiency_pct": efficiency,
                "longest_plateau": longest_plateau,
                "trend": trend,
                "top_improvements": top_keeps.iter().take(5).map(|(d, desc)| {
                    serde_json::json!({"delta": d.to_string(), "description": desc})
                }).collect::<Vec<_>>(),
            });
            let json = serde_json::to_string_pretty(&out)?;
            std::fs::write(summary_dir.join("evals-summary.json"), &json)?;
            println!("{json}");
        }
        "md" => {
            let report = render_evals_markdown(EvalsReport {
                direction,
                total_iterations,
                keeps,
                discards,
                crashes,
                efficiency,
                baseline,
                final_metric,
                best,
                trend,
                longest_plateau,
                top_keeps: &top_keeps,
            });
            std::fs::write(summary_dir.join("evals-summary.md"), &report)?;
            print!("{report}");
        }
        _ => {
            let report = render_evals_markdown(EvalsReport {
                direction,
                total_iterations,
                keeps,
                discards,
                crashes,
                efficiency,
                baseline,
                final_metric,
                best,
                trend,
                longest_plateau,
                top_keeps: &top_keeps,
            });
            print!("{report}");
        }
    }

    Ok(())
}

struct EvalsReport<'a> {
    direction: &'a str,
    total_iterations: usize,
    keeps: usize,
    discards: usize,
    crashes: usize,
    efficiency: u32,
    baseline: Decimal,
    final_metric: Decimal,
    best: Decimal,
    trend: &'a str,
    longest_plateau: u32,
    top_keeps: &'a [(Decimal, &'a str)],
}

fn render_evals_markdown(report: EvalsReport<'_>) -> String {
    let mut out = String::new();
    writeln!(out, "## Autoresearch Evals").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Stat | Value |").unwrap();
    writeln!(out, "|------|-------|").unwrap();
    writeln!(out, "| Direction | {} |", report.direction).unwrap();
    writeln!(out, "| Iterations | {} |", report.total_iterations).unwrap();
    writeln!(out, "| Kept | {} |", report.keeps).unwrap();
    writeln!(out, "| Discarded | {} |", report.discards).unwrap();
    writeln!(out, "| Crashes | {} |", report.crashes).unwrap();
    writeln!(out, "| Efficiency | {}% |", report.efficiency).unwrap();
    writeln!(out, "| Baseline | {} |", report.baseline).unwrap();
    writeln!(out, "| Final | {} |", report.final_metric).unwrap();
    writeln!(out, "| Best | {} |", report.best).unwrap();
    writeln!(out, "| Trend | {} |", report.trend).unwrap();
    writeln!(
        out,
        "| Longest plateau | {} iterations |",
        report.longest_plateau
    )
    .unwrap();
    writeln!(out).unwrap();

    if !report.top_keeps.is_empty() {
        writeln!(out, "### Top Improvements").unwrap();
        writeln!(out).unwrap();
        for (i, (delta, desc)) in report.top_keeps.iter().take(5).enumerate() {
            writeln!(out, "{}. **{}** - {}", i + 1, delta, desc).unwrap();
        }
        writeln!(out).unwrap();
    }

    writeln!(out, "### Recommendations").unwrap();
    writeln!(out).unwrap();
    if report.longest_plateau >= 5 {
        writeln!(
            out,
            "- Plateau of {} iterations detected. Consider a PIVOT strategy.",
            report.longest_plateau
        )
        .unwrap();
    }
    if report.crashes > report.keeps {
        writeln!(
            out,
            "- More crashes than keeps. Check verify command reliability."
        )
        .unwrap();
    }
    if report.efficiency < 20 && report.total_iterations > 10 {
        writeln!(
            out,
            "- Low efficiency ({}%). Hypotheses may need better grounding.",
            report.efficiency
        )
        .unwrap();
    }
    if report.trend == "declining" {
        writeln!(
            out,
            "- Declining trend. Recent changes may be counterproductive."
        )
        .unwrap();
    }
    if report.trend == "improving" && report.efficiency > 30 {
        writeln!(out, "- Strong trajectory. Continue current approach.").unwrap();
    }
    if report.longest_plateau < 3 && report.efficiency > 40 {
        writeln!(
            out,
            "- Healthy run. Good keep rate with no extended plateaus."
        )
        .unwrap();
    }
    out
}

// ── Status ────────────────────────────────────────────────────────────

fn cmd_status(cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");

    if !state_path.exists() {
        println!(r#"{{"active":false,"message":"No active autoresearch run."}}"#);
        return Ok(());
    }

    let state_content = std::fs::read_to_string(&state_path)?;
    let state: RunState = serde_json::from_str(&state_content)?;

    // Also read escalation state if it exists
    let esc_path = results_dir.join("escalation.json");
    let escalation: Option<EscalationState> = if esc_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&esc_path)?).ok()
    } else {
        None
    };

    // Read last few TSV rows
    let tsv_path = results_dir.join("results.tsv");
    let tail = if tsv_path.exists() {
        let log = ResultsLog::open(tsv_path)?;
        log.tail(5)?
    } else {
        vec![]
    };

    let out = serde_json::json!({
        "active": true,
        "iteration": state.iteration,
        "baseline_metric": state.baseline_metric.to_string(),
        "current_metric": state.current_metric.to_string(),
        "best_metric": state.best_metric.to_string(),
        "best_iteration": state.best_iteration,
        "keeps": state.keeps,
        "discards": state.discards,
        "crashes": state.crashes,
        "no_ops": state.no_ops,
        "consecutive_discards": state.consecutive_discards,
        "last_status": state.last_status.as_str(),
        "phase": state.phase,
        "config": state.config,
        "escalation": escalation,
        "recent_rows": tail,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

// ── Health ────────────────────────────────────────────────────────────

fn cmd_health(verify: Option<&str>, min_free_mb: u64, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let report = health::run_health_check(&workspace, verify, min_free_mb)?;
    let has_blockers = report.has_blockers();
    println!("{}", serde_json::to_string_pretty(&report)?);
    if has_blockers {
        std::process::exit(2);
    }
    Ok(())
}

// ── Runtime ───────────────────────────────────────────────────────────

fn cmd_runtime(command: RuntimeCommands) -> Result<()> {
    match command {
        RuntimeCommands::Start {
            execution_policy,
            codex_bin,
            dry_run,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let (manifest, snapshot) =
                runtime::start_runtime(&workspace, &execution_policy, &codex_bin, dry_run)?;
            let out = serde_json::json!({
                "status": "ok",
                "runtime": snapshot,
                "launch": {
                    "path": manifest.launch_path,
                    "execution_policy": manifest.execution_policy,
                    "codex_bin": manifest.codex_bin,
                    "codex_args": manifest.codex_args,
                }
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        RuntimeCommands::Status { cwd } => {
            let workspace = resolve_results_workspace(cwd);
            let snapshot = runtime::runtime_status(&workspace)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
        RuntimeCommands::Supervise {
            after_run,
            max_stagnation,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let (snapshot, supervisor) =
                runtime::supervise_runtime(&workspace, after_run, max_stagnation)?;
            let out = serde_json::json!({
                "decision": supervisor.decision,
                "reason": supervisor.reason,
                "terminal_reason": supervisor.terminal_reason,
                "should_continue": supervisor.should_continue,
                "restart_count": supervisor.restart_count,
                "stagnation_count": supervisor.stagnation_count,
                "runtime": snapshot,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        RuntimeCommands::Run {
            execution_policy,
            codex_bin,
            max_restarts,
            max_stagnation,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let (snapshot, supervisor) = runtime::run_runtime_loop(
                &workspace,
                &execution_policy,
                &codex_bin,
                max_restarts,
                max_stagnation,
            )?;
            let out = serde_json::json!({
                "status": "ok",
                "decision": supervisor.decision,
                "reason": supervisor.reason,
                "terminal_reason": supervisor.terminal_reason,
                "should_continue": supervisor.should_continue,
                "restart_count": supervisor.restart_count,
                "stagnation_count": supervisor.stagnation_count,
                "runtime": snapshot,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        RuntimeCommands::Stop { cwd } => {
            let workspace = resolve_results_workspace(cwd);
            let snapshot = runtime::stop_runtime(&workspace)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
    }
    Ok(())
}

// ── Parallel ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct ParallelWorkerInput {
    worker_id: String,
    description: String,
    #[serde(default = "default_completed_status")]
    status: String,
    #[serde(default)]
    guard: Option<String>,
    #[serde(default)]
    commit: Option<String>,
    #[serde(default)]
    metric: Option<serde_json::Value>,
    #[serde(default)]
    metrics: Option<serde_json::Value>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    diff_size: Option<u64>,
}

#[derive(Debug, Clone)]
struct ParallelWorkerRecord {
    worker_id: String,
    description: String,
    commit: Option<String>,
    metric: Decimal,
    metrics: Option<BTreeMap<String, Decimal>>,
    labels: Vec<String>,
    guard: GuardResult,
    status: IterationStatus,
    diff_size: u64,
}

fn default_completed_status() -> String {
    "completed".to_string()
}

fn cmd_parallel(command: ParallelCommands) -> Result<()> {
    match command {
        ParallelCommands::Closeout { batch_file, cwd } => {
            let workspace = resolve_results_workspace(cwd);
            let out = cmd_parallel_closeout(&workspace, &batch_file)?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
    }
    Ok(())
}

fn cmd_parallel_closeout(
    workspace: &std::path::Path,
    batch_file: &std::path::Path,
) -> Result<serde_json::Value> {
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    let tsv_path = results_dir.join("results.tsv");

    let health = health::run_health_check(workspace, None, 500)?;
    if health.has_blockers() {
        let codes = health
            .blockers
            .iter()
            .map(|finding| finding.code)
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("parallel batch preflight blocked: {codes}");
    }
    let git = GitRepo::open(workspace)?;
    if let WorktreeStatus::Dirty(files) = git.worktree_status()? {
        anyhow::bail!(
            "parallel batch preflight blocked: unexpected worktree changes before parallel batch: {}",
            files.join(", ")
        );
    }

    let mut state: RunState = serde_json::from_str(
        &std::fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read {}", state_path.display()))?,
    )?;
    let batch: Vec<ParallelWorkerInput> = serde_json::from_str(
        &std::fs::read_to_string(batch_file)
            .with_context(|| format!("failed to read {}", batch_file.display()))?,
    )
    .with_context(|| format!("failed to parse {}", batch_file.display()))?;
    if batch.is_empty() {
        anyhow::bail!("parallel batch file must contain at least one worker result");
    }

    let next_iteration = state.iteration + 1;
    let current_metric = state.current_metric;
    let (primary_metric_key, verify_format, required_keep_criteria, required_keep_labels) =
        match state.config.as_ref() {
            Some(config) => (
                config
                    .primary_metric_key
                    .clone()
                    .or_else(|| {
                        if config.metric.trim().is_empty() {
                            None
                        } else {
                            Some(config.metric.clone())
                        }
                    })
                    .unwrap_or_else(|| "metric".to_string()),
                config.verify_format,
                config.required_keep_criteria.clone(),
                config.required_keep_labels.clone(),
            ),
            None => (
                "metric".to_string(),
                VerifyFormat::Scalar,
                Vec::new(),
                Vec::new(),
            ),
        };
    let mut records = Vec::with_capacity(batch.len());
    let mut candidates = Vec::new();

    for item in batch {
        validate_worker_id(&item.worker_id)?;
        let guard = parse_guard_result(item.guard.as_deref())?;
        let normalized_status = item.status.trim().to_ascii_lowercase();
        let status = match normalized_status.as_str() {
            "completed" => {
                let metric_value = item
                    .metric
                    .as_ref()
                    .with_context(|| format!("worker {} is missing metric", item.worker_id))?;
                let metric = parse_decimal_json(metric_value)
                    .with_context(|| format!("invalid metric for worker {}", item.worker_id))?;
                let delta = metric - current_metric;
                let trial_metrics = build_parallel_trial_metrics(
                    metric,
                    item.metrics.as_ref(),
                    &primary_metric_key,
                    verify_format,
                )
                .with_context(|| format!("invalid metrics for worker {}", item.worker_id))?;
                let required_keep =
                    criteria::evaluate_criteria(&required_keep_criteria, &trial_metrics);
                let trial_labels = normalize_labels(item.labels);
                let missing_labels = missing_required_labels(&required_keep_labels, &trial_labels);
                let mut description = item.description;
                let status = if guard == GuardResult::Fail {
                    IterationStatus::Discard
                } else if state.direction.is_improvement(delta) {
                    if required_keep.satisfied && missing_labels.is_empty() {
                        IterationStatus::Keep
                    } else {
                        if !required_keep.satisfied {
                            description = format!(
                                "{description} [KEEP-CRITERIA miss] {}",
                                required_keep.failures.join("; ")
                            );
                        }
                        IterationStatus::Discard
                    }
                } else {
                    IterationStatus::Discard
                };
                if !trial_labels.is_empty() || !missing_labels.is_empty() {
                    description = label_description(&description, &trial_labels, &missing_labels);
                }
                let record = ParallelWorkerRecord {
                    worker_id: item.worker_id,
                    description,
                    commit: normalize_optional_commit(item.commit),
                    metric,
                    metrics: Some(trial_metrics),
                    labels: trial_labels,
                    guard,
                    status,
                    diff_size: item.diff_size.unwrap_or(u64::MAX),
                };
                if status == IterationStatus::Keep {
                    candidates.push(record.clone());
                }
                records.push(record);
                continue;
            }
            "crash" | "timeout" => IterationStatus::Crash,
            other => anyhow::bail!(
                "worker {} has unsupported status {other:?}; use completed, crash, or timeout",
                item.worker_id
            ),
        };
        records.push(ParallelWorkerRecord {
            worker_id: item.worker_id,
            description: item.description,
            commit: normalize_optional_commit(item.commit),
            metric: current_metric,
            metrics: None,
            labels: normalize_labels(item.labels),
            guard,
            status,
            diff_size: item.diff_size.unwrap_or(u64::MAX),
        });
    }

    let winner = select_parallel_winner(&candidates, state.direction);
    let selected_worker = winner.as_ref().map(|record| record.worker_id.clone());
    let best_completed = if winner.is_none() {
        select_best_completed_worker(&records, state.direction)
    } else {
        None
    };

    let mut worker_rows = Vec::with_capacity(records.len());
    for record in &records {
        let row_status = if record.status == IterationStatus::Keep {
            if Some(&record.worker_id) == selected_worker.as_ref() {
                IterationStatus::Keep
            } else {
                IterationStatus::Discard
            }
        } else {
            record.status
        };
        worker_rows.push((
            format!("{next_iteration}{}", record.worker_id),
            ResultRow {
                iteration: next_iteration,
                commit: if row_status == IterationStatus::Keep {
                    record.commit.clone()
                } else {
                    None
                },
                metric: record.metric,
                delta: record.metric - current_metric,
                guard: record.guard,
                status: row_status,
                description: format!(
                    "[PARALLEL worker-{}] {}",
                    record.worker_id, record.description
                ),
            },
        ));
    }

    let (main_status, main_metric, main_metrics, main_labels, main_commit, main_guard, main_description) = match winner {
        Some(winner_record) => {
            let Some(commit) = winner_record.commit.clone() else {
                anyhow::bail!(
                    "worker {} improved the metric but did not report a commit",
                    winner_record.worker_id
                );
            };
            (
                IterationStatus::Keep,
                winner_record.metric,
                winner_record.metrics.clone(),
                winner_record.labels.clone(),
                Some(commit),
                winner_record.guard,
                format!(
                    "[PARALLEL batch] selected worker-{}: {}",
                    winner_record.worker_id, winner_record.description
                ),
            )
        }
        None => match best_completed {
            Some(best) => (
                IterationStatus::Discard,
                best.metric,
                best.metrics.clone(),
                best.labels.clone(),
                best.commit.clone(),
                best.guard,
                format!(
                    "[PARALLEL batch] no worker produced a keepable improvement; best discarded worker-{}: {}",
                    best.worker_id, best.description
                ),
            ),
            None => (
                IterationStatus::Discard,
                current_metric,
                None,
                Vec::new(),
                None,
                GuardResult::Skip,
                "[PARALLEL batch] no worker completed successfully".to_string(),
            ),
        },
    };

    let main_row = ResultRow {
        iteration: next_iteration,
        commit: if main_status == IterationStatus::Keep {
            main_commit.clone()
        } else {
            None
        },
        metric: main_metric,
        delta: main_metric - current_metric,
        guard: main_guard,
        status: main_status,
        description: main_description,
    };

    let log = ResultsLog::open(tsv_path)?;
    for (label, row) in &worker_rows {
        log.append_labeled(label, row)?;
    }
    log.append(&main_row)?;

    let esc_path = results_dir.join("escalation.json");
    let mut escalation: EscalationState = if esc_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&esc_path)?)?
    } else {
        EscalationState::default()
    };
    let lessons_log = LessonsLog::open_or_create(&results_dir)?;

    match main_status {
        IterationStatus::Keep => {
            if let Some(metrics) = main_metrics.clone() {
                state.record_keep_with_metrics_and_labels(
                    main_metric,
                    main_commit.clone().unwrap(),
                    metrics,
                    main_labels.clone(),
                );
            } else {
                state.record_keep_with_labels(
                    main_metric,
                    main_commit.clone().unwrap(),
                    main_labels.clone(),
                );
            }
            escalation.record_keep();
            let lesson = lessons::extract_keep_lesson(
                &main_row.description,
                &autoresearch::core::metrics::format_delta(main_row.delta),
            );
            let _ = lessons_log.append(&lesson);
        }
        IterationStatus::Discard => {
            if let Some(metrics) = main_metrics.clone() {
                state.record_discard_with_metrics_and_labels(
                    main_metric,
                    main_commit.clone(),
                    metrics,
                    main_labels.clone(),
                );
            } else {
                state.record_discard_with_labels(main_metric, main_commit.clone(), main_labels);
            }
            if escalation.record_discard() == EscalationAction::Pivot {
                let lesson = lessons::extract_pivot_lesson(
                    &main_row.description,
                    EscalationAction::Pivot.guidance(),
                );
                let _ = lessons_log.append(&lesson);
                escalation.acknowledge_pivot();
            }
            state.pivot_count = escalation.pivot_count;
            state.consecutive_discards = escalation.consecutive_discards;
        }
        _ => {}
    }

    std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
    std::fs::write(&esc_path, serde_json::to_string_pretty(&escalation)?)?;

    Ok(serde_json::json!({
        "status": "ok",
        "iteration": next_iteration,
        "selected_worker": selected_worker,
        "decision": main_status.as_str(),
        "main_status": main_status.as_str(),
        "retained_metric": state.current_metric.to_string(),
        "trial_metric": state.last_trial_metric.map(|metric| metric.to_string()),
        "batch_file": batch_file.display().to_string(),
        "message": format!("Parallel batch recorded at iteration {next_iteration}."),
    }))
}

fn validate_worker_id(worker_id: &str) -> Result<()> {
    if worker_id.is_empty()
        || !worker_id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() && ch.is_ascii_alphabetic())
    {
        anyhow::bail!("worker_id must contain only lowercase ASCII letters: {worker_id:?}");
    }
    Ok(())
}

fn parse_guard_result(value: Option<&str>) -> Result<GuardResult> {
    let normalized = value.unwrap_or("-").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "pass" => Ok(GuardResult::Pass),
        "fail" => Ok(GuardResult::Fail),
        "-" | "skip" => Ok(GuardResult::Skip),
        other => anyhow::bail!("unknown guard result {other:?}; use pass, fail, or skip"),
    }
}

fn normalize_optional_commit(commit: Option<String>) -> Option<String> {
    commit.and_then(|commit| {
        let trimmed = commit.trim();
        if trimmed.is_empty() || trimmed == "-" {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_decimal_json(value: &serde_json::Value) -> Result<Decimal> {
    match value {
        serde_json::Value::String(text) => Decimal::from_str(text),
        serde_json::Value::Number(number) => Decimal::from_str(&number.to_string()),
        _ => anyhow::bail!("metric must be a JSON string or number"),
    }
    .context("invalid decimal")
}

fn build_parallel_trial_metrics(
    primary_metric: Decimal,
    metrics: Option<&serde_json::Value>,
    primary_metric_key: &str,
    verify_format: VerifyFormat,
) -> Result<BTreeMap<String, Decimal>> {
    match metrics {
        Some(value) => parse_parallel_metrics_object(value),
        None => build_trial_metrics(primary_metric, None, primary_metric_key, verify_format),
    }
}

fn parse_parallel_metrics_object(value: &serde_json::Value) -> Result<BTreeMap<String, Decimal>> {
    let Some(object) = value.as_object() else {
        anyhow::bail!("metrics must be a JSON object");
    };
    let mut metrics = BTreeMap::new();
    for (key, value) in object {
        let metric = parse_decimal_json(value).with_context(|| format!("invalid {key} metric"))?;
        metrics.insert(key.clone(), metric);
    }
    Ok(metrics)
}

fn select_parallel_winner(
    candidates: &[ParallelWorkerRecord],
    direction: Direction,
) -> Option<ParallelWorkerRecord> {
    candidates
        .iter()
        .cloned()
        .min_by(|left, right| compare_parallel_records(left, right, direction))
}

fn select_best_completed_worker(
    records: &[ParallelWorkerRecord],
    direction: Direction,
) -> Option<ParallelWorkerRecord> {
    records
        .iter()
        .filter(|record| record.status != IterationStatus::Crash)
        .cloned()
        .min_by(|left, right| compare_parallel_records(left, right, direction))
}

fn compare_parallel_records(
    left: &ParallelWorkerRecord,
    right: &ParallelWorkerRecord,
    direction: Direction,
) -> std::cmp::Ordering {
    let metric_order = match direction {
        Direction::Higher => right.metric.cmp(&left.metric),
        Direction::Lower => left.metric.cmp(&right.metric),
    };
    metric_order
        .then_with(|| guard_rank(left.guard).cmp(&guard_rank(right.guard)))
        .then_with(|| left.diff_size.cmp(&right.diff_size))
        .then_with(|| left.worker_id.cmp(&right.worker_id))
}

fn guard_rank(guard: GuardResult) -> u8 {
    match guard {
        GuardResult::Pass => 0,
        GuardResult::Skip => 1,
        GuardResult::Fail => 2,
    }
}

// ── Screen ────────────────────────────────────────────────────────────

fn cmd_screen(command: &str) -> Result<()> {
    match verify::screen_command(command) {
        Ok(()) => {
            println!(r#"{{"safe":true}}"#);
        }
        Err(e) => {
            println!(
                "{}",
                serde_json::json!({"safe": false, "reason": e.to_string()})
            );
            std::process::exit(2);
        }
    }
    Ok(())
}

// ── Resume ────────────────────────────────────────────────────────────

fn cmd_resume(cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");

    if !state_path.exists() {
        let tsv_path = results_dir.join("results.tsv");
        if tsv_path.exists() {
            let log = ResultsLog::open(tsv_path.clone())?;
            if let Err(err) = log.validate() {
                let out = serde_json::json!({
                    "resumable": false,
                    "recommendation": "fresh_start",
                    "reason": "results_corrupt",
                    "error": err.to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
                return Ok(());
            }

            let content = std::fs::read_to_string(&tsv_path)?;
            let rows = parse_results_tsv(&content)?;
            if let Some(out) = tsv_fallback_resume(&rows, &content, log.tail(5)?) {
                println!("{}", serde_json::to_string_pretty(&out)?);
                return Ok(());
            }
        }
        println!(r#"{{"resumable":false}}"#);
        return Ok(());
    }

    let state: RunState = serde_json::from_str(&std::fs::read_to_string(&state_path)?)?;

    let is_resumable = matches!(
        state.phase,
        RunPhase::Baseline { .. } | RunPhase::Iterating { .. }
    );

    // Read last 5 rows from results.tsv
    let tsv_path = results_dir.join("results.tsv");
    let recent_rows: Vec<String> = if tsv_path.exists() {
        let log = ResultsLog::open(tsv_path)?;
        log.tail(5)?
    } else {
        vec![]
    };

    // Read last 5 lessons
    let lessons_path = results_dir.join("lessons.md");
    let recent_lessons: Vec<String> = if lessons_path.exists() {
        let content = std::fs::read_to_string(&lessons_path)?;
        content
            .lines()
            .filter(|l| l.starts_with("- "))
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    } else {
        vec![]
    };

    let recommendation = if is_resumable && state.consecutive_discards < 10 {
        "resume"
    } else {
        "fresh_start"
    };

    let out = serde_json::json!({
        "resumable": is_resumable,
        "iteration": state.iteration,
        "current_metric": state.current_metric.to_string(),
        "best_metric": state.best_metric.to_string(),
        "keeps": state.keeps,
        "discards": state.discards,
        "last_status": state.last_status.as_str(),
        "recent_rows": recent_rows,
        "recent_lessons": recent_lessons,
        "recommendation": recommendation,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn tsv_fallback_resume(
    rows: &[ParsedRow],
    content: &str,
    recent_rows: Vec<String>,
) -> Option<serde_json::Value> {
    let baseline = rows.iter().find(|row| row.status == "baseline")?;
    let direction = results_tsv_direction(content);
    let mut current_metric = baseline.metric;
    let mut best_metric = baseline.metric;
    let mut best_iteration = baseline.iteration;
    let mut keeps = 0u32;
    let mut discards = 0u32;
    let mut crashes = 0u32;
    let mut no_ops = 0u32;
    let mut blocked = 0u32;

    for row in rows {
        match row.status.as_str() {
            "keep" => {
                keeps += 1;
                current_metric = row.metric;
                if metric_is_better(row.metric, best_metric, direction) {
                    best_metric = row.metric;
                    best_iteration = row.iteration;
                }
            }
            "discard" => discards += 1,
            "crash" => crashes += 1,
            "no-op" => no_ops += 1,
            "blocked" => blocked += 1,
            _ => {}
        }
    }

    let last = rows.last()?;
    Some(serde_json::json!({
        "resumable": true,
        "source": "results.tsv",
        "recommendation": "tsv_fallback",
        "iteration": last.iteration,
        "current_metric": current_metric.to_string(),
        "best_metric": best_metric.to_string(),
        "best_iteration": best_iteration,
        "keeps": keeps,
        "discards": discards,
        "crashes": crashes,
        "no_ops": no_ops,
        "blocked": blocked,
        "last_status": last.status,
        "recent_rows": recent_rows,
    }))
}

fn results_tsv_direction(content: &str) -> Direction {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# metric_direction:"))
        .map(str::trim)
        .map(|value| match value {
            "lower" => Direction::Lower,
            _ => Direction::Higher,
        })
        .unwrap_or(Direction::Higher)
}

fn metric_is_better(candidate: Decimal, current_best: Decimal, direction: Direction) -> bool {
    match direction {
        Direction::Higher => candidate > current_best,
        Direction::Lower => candidate < current_best,
    }
}

// ── Progress ─────────────────────────────────────────────────────────

fn cmd_progress(cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");

    if !state_path.exists() {
        anyhow::bail!("No active run (state.json not found)");
    }

    let state: RunState = serde_json::from_str(&std::fs::read_to_string(&state_path)?)?;

    // Determine escalation level
    let esc_path = results_dir.join("escalation.json");
    let escalation_label = if esc_path.exists() {
        let esc: EscalationState = serde_json::from_str(&std::fs::read_to_string(&esc_path)?)?;
        format!("{:?}", esc.last_action).to_lowercase()
    } else {
        "none".to_string()
    };

    // Compute trend from last 5 keep metrics in TSV
    let tsv_path = results_dir.join("results.tsv");
    let trend = if tsv_path.exists() {
        let content = std::fs::read_to_string(&tsv_path)?;
        let keep_metrics: Vec<Decimal> = content
            .lines()
            .filter(|l| l.contains("\tkeep\t"))
            .filter(|l| {
                l.split('\t')
                    .next()
                    .is_some_and(|iteration| iteration.parse::<u32>().is_ok())
            })
            .filter_map(|l| {
                let cols: Vec<&str> = l.split('\t').collect();
                if cols.len() >= 3 {
                    Decimal::from_str(cols[2]).ok()
                } else {
                    None
                }
            })
            .collect();
        let last5: Vec<&Decimal> = keep_metrics.iter().rev().take(5).collect();
        if last5.len() < 2 {
            "insufficient_data"
        } else {
            match state.direction {
                Direction::Lower if last5.windows(2).all(|w| w[0] <= w[1]) => "improving",
                Direction::Lower if last5.windows(2).all(|w| w[0] >= w[1]) => "declining",
                _ if last5.windows(2).all(|w| w[0] >= w[1]) => "improving",
                _ if last5.windows(2).all(|w| w[0] <= w[1]) => "declining",
                _ => "flat",
            }
        }
    } else {
        "insufficient_data"
    };

    println!("--- Progress (iteration {}) ---", state.iteration);
    println!(
        "Metric: {} → {} (best: {})",
        state.baseline_metric, state.current_metric, state.best_metric
    );
    println!(
        "Kept: {} | Discarded: {} | Crashes: {}",
        state.keeps, state.discards, state.crashes
    );
    println!(
        "Trend: {} | Consecutive discards: {}",
        trend, state.consecutive_discards
    );
    println!("Escalation: {}", escalation_label);
    println!("---");
    Ok(())
}

// ── Lessons ──────────────────────────────────────────────────────────

fn cmd_lessons(search: Option<&str>, last: Option<usize>, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let log = LessonsLog::open_or_create(&results_dir)?;

    let entries = match search {
        Some(q) => log.search(q)?,
        None => log.read_all()?,
    };

    let n = last.unwrap_or(10);
    let tail: Vec<&String> = entries
        .iter()
        .rev()
        .take(n)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let out = serde_json::to_string_pretty(&tail)?;
    println!("{out}");
    Ok(())
}

// ── Handoff ──────────────────────────────────────────────────────────

fn cmd_handoff(
    source: &str,
    status: &str,
    findings: Option<&str>,
    config: Option<&str>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    std::fs::create_dir_all(&results_dir)?;

    let findings_val: serde_json::Value =
        serde_json::from_str(findings.unwrap_or("[]")).context("Invalid findings JSON")?;
    let config_val: serde_json::Value =
        serde_json::from_str(config.unwrap_or("{}")).context("Invalid config JSON")?;

    let timestamp = chrono::Utc::now().to_rfc3339();

    let handoff = serde_json::json!({
        "version": "0.1.0",
        "source": source,
        "timestamp": timestamp,
        "status": status,
        "results_tsv": "autoresearch-results/results.tsv",
        "findings": findings_val,
        "config": config_val,
    });

    let handoff_path = results_dir.join("handoff.json");
    std::fs::write(&handoff_path, serde_json::to_string_pretty(&handoff)?)?;

    println!(r#"{{"status":"ok","path":"autoresearch-results/handoff.json"}}"#);
    Ok(())
}

// ── Exec ─────────────────────────────────────────────────────────────

fn cmd_exec(iterations: u32, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);

    // Read config from stdin
    let config: RunConfig = serde_json::from_reader(std::io::stdin().lock())
        .context("exec: failed to parse RunConfig from stdin")?;

    // Extract display values before moving config
    let direction = config.direction;
    let verify_cmd = config.verify.clone();
    let fmt = config.verify_format;
    let primary_key = config.primary_metric_key.clone();

    // Screen
    if let Err(e) = verify::screen_command(&verify_cmd) {
        let out =
            serde_json::json!({"type":"error","code":"unsafe_command","reason":e.to_string()});
        println!("{}", serde_json::to_string(&out)?);
        std::process::exit(2);
    }

    // Git check
    let git = GitRepo::open(&workspace).context("exec: requires a git repository")?;
    match git.worktree_status()? {
        WorktreeStatus::Clean | WorktreeStatus::OnlyArtifacts => {}
        WorktreeStatus::Dirty(files) => {
            let out = serde_json::json!({"type":"error","code":"dirty_worktree","files":files});
            println!("{}", serde_json::to_string(&out)?);
            std::process::exit(2);
        }
    }

    // Baseline
    let result = verify::run_verify(&verify_cmd, fmt, primary_key.as_deref(), &workspace)
        .context("exec: baseline verification failed")?;
    let head = git.head_short()?;

    // Init artifacts + protect from git staging
    let results_dir = ensure_results_dir_protected(&workspace)?;

    let log = ResultsLog::create(&results_dir, direction)?;
    log.append(&ResultRow {
        iteration: 0,
        commit: Some(head.clone()),
        metric: result.metric,
        delta: Decimal::ZERO,
        guard: GuardResult::Skip,
        status: IterationStatus::Baseline,
        description: "initial state".to_string(),
    })?;

    let mut state = RunState::from_baseline(result.metric, head.clone(), Some(config));
    if let Some(metrics) = result.metrics.clone() {
        state.set_current_metrics(metrics);
    }
    std::fs::write(
        results_dir.join("state.json"),
        serde_json::to_string_pretty(&state)?,
    )?;
    context::write_context(&workspace, state.config.as_ref())?;
    LessonsLog::open_or_create(&results_dir)?;

    // Emit JSON line
    let out = serde_json::json!({
        "type": "started",
        "baseline": result.metric.to_string(),
        "commit": head,
        "direction": direction.as_str(),
        "iterations": iterations,
        "results_dir": "autoresearch-results",
        "verify_duration_ms": result.duration.as_millis(),
    });
    println!("{}", serde_json::to_string(&out)?);

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────

fn resolve_cwd(cwd: Option<PathBuf>) -> PathBuf {
    cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn resolve_workspace_root(cwd: Option<PathBuf>) -> PathBuf {
    let workspace = resolve_cwd(cwd);
    GitRepo::open(&workspace)
        .ok()
        .and_then(|repo| repo.workdir())
        .unwrap_or(workspace)
}

fn resolve_results_workspace(cwd: Option<PathBuf>) -> PathBuf {
    let workspace = resolve_cwd(cwd);
    if workspace.join("autoresearch-results").exists() {
        return workspace;
    }
    GitRepo::open(&workspace)
        .ok()
        .and_then(|repo| repo.workdir())
        .filter(|root| root.join("autoresearch-results").exists())
        .unwrap_or(workspace)
}

fn default_results_tsv(cwd: &Path) -> Option<PathBuf> {
    let cwd_default = cwd.join("autoresearch-results/results.tsv");
    if cwd_default.exists() {
        return Some(cwd_default);
    }
    GitRepo::open(cwd)
        .ok()
        .and_then(|repo| repo.workdir())
        .map(|root| root.join("autoresearch-results/results.tsv"))
        .filter(|path| path.exists())
}

fn parse_decide_metric(
    metric_str: Option<&str>,
    decision: &str,
    retained_metric: Decimal,
) -> Result<Decimal> {
    match metric_str {
        Some(value) => Decimal::from_str(value).with_context(|| format!("Invalid metric: {value}")),
        None if matches!(decision, "auto" | "discard" | "keep") => {
            anyhow::bail!("--metric is required for {decision} decisions")
        }
        None => Ok(retained_metric),
    }
}

fn retained_trial_metrics(
    state: &RunState,
    metric: Decimal,
    primary_metric_key: &str,
) -> BTreeMap<String, Decimal> {
    let mut metrics = state.current_metrics.clone();
    metrics
        .entry(primary_metric_key.to_string())
        .or_insert(metric);
    metrics.entry("metric".to_string()).or_insert(metric);
    metrics
}

fn normalize_labels(labels: Vec<String>) -> Vec<String> {
    labels
        .into_iter()
        .map(|label| label.trim().to_ascii_lowercase())
        .filter(|label| !label.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn missing_required_labels(required: &[String], labels: &[String]) -> Vec<String> {
    let present = labels.iter().cloned().collect::<BTreeSet<_>>();
    required
        .iter()
        .map(|label| label.trim().to_ascii_lowercase())
        .filter(|label| !label.is_empty() && !present.contains(label))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn label_description(description: &str, labels: &[String], missing_required: &[String]) -> String {
    let mut parts = Vec::new();
    if !labels.is_empty() {
        parts.push(format!("[labels: {}]", labels.join(", ")));
    }
    if !missing_required.is_empty() {
        parts.push(format!(
            "[KEEP-LABEL miss] missing required labels: {}",
            missing_required.join(", ")
        ));
    }
    parts.push(description.to_string());
    parts.join(" ")
}

fn build_trial_metrics(
    primary_metric: Decimal,
    metrics_json: Option<&str>,
    primary_metric_key: &str,
    verify_format: VerifyFormat,
) -> Result<BTreeMap<String, Decimal>> {
    match metrics_json {
        Some(raw) => {
            let metrics = autoresearch::core::metrics::parse_json_metrics_map(raw)?;
            if verify_format == VerifyFormat::MetricsJson {
                let actual = metrics.get(primary_metric_key).with_context(|| {
                    format!(
                        "verify_format=metrics_json requires metrics key {primary_metric_key:?}"
                    )
                })?;
                if *actual != primary_metric {
                    anyhow::bail!(
                        "Primary metric mismatch: --metric {primary_metric} but metrics_json[{primary_metric_key:?}] is {actual}"
                    );
                }
            }
            Ok(metrics)
        }
        None if verify_format == VerifyFormat::MetricsJson => {
            anyhow::bail!("verify_format=metrics_json requires --metrics-json")
        }
        None => Ok(BTreeMap::from([(
            primary_metric_key.to_string(),
            primary_metric,
        )])),
    }
}

fn ensure_metrics_json_keys(
    metrics: &BTreeMap<String, Decimal>,
    primary_metric_key: &str,
    acceptance_criteria: &[autoresearch::core::config::MetricCriterion],
    required_keep_criteria: &[autoresearch::core::config::MetricCriterion],
) -> Result<()> {
    let mut required = BTreeSet::from([primary_metric_key.to_string()]);
    for criterion in acceptance_criteria.iter().chain(required_keep_criteria) {
        required.insert(criterion.metric_key.clone());
    }

    let missing = required
        .into_iter()
        .filter(|key| !metrics.contains_key(key))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        anyhow::bail!(
            "verify_format=metrics_json requires metrics keys: {}",
            missing.join(", ")
        );
    }
    Ok(())
}

fn parse_direction(s: &str) -> Result<Direction> {
    match s {
        "higher" | "higher_is_better" => Ok(Direction::Higher),
        "lower" | "lower_is_better" => Ok(Direction::Lower),
        _ => anyhow::bail!("Unknown direction: {s}. Use 'higher' or 'lower'."),
    }
}

fn parse_format(s: &str) -> VerifyFormat {
    match s {
        "metrics_json" => VerifyFormat::MetricsJson,
        _ => VerifyFormat::Scalar,
    }
}

fn parse_run_mode(s: &str) -> Result<RunMode> {
    match s {
        "foreground" => Ok(RunMode::Foreground),
        "background" => Ok(RunMode::Background),
        _ => anyhow::bail!("Unknown run mode: {s}. Use 'foreground' or 'background'."),
    }
}

fn parse_rollback_strategy(s: &str) -> Result<RollbackStrategy> {
    match s {
        "revert" => Ok(RollbackStrategy::Revert),
        "hard-reset" | "hard_reset" => Ok(RollbackStrategy::HardReset),
        _ => anyhow::bail!("Unknown rollback strategy: {s}. Use 'revert' or 'hard-reset'."),
    }
}

fn parse_status(s: &str) -> Result<IterationStatus> {
    match s {
        "baseline" => Ok(IterationStatus::Baseline),
        "keep" => Ok(IterationStatus::Keep),
        "discard" => Ok(IterationStatus::Discard),
        "crash" => Ok(IterationStatus::Crash),
        "no-op" => Ok(IterationStatus::NoOp),
        "blocked" => Ok(IterationStatus::Blocked),
        "pivot" => Ok(IterationStatus::Pivot),
        "refine" => Ok(IterationStatus::Refine),
        "search" => Ok(IterationStatus::Search),
        _ => anyhow::bail!("Unknown status: {s}"),
    }
}
