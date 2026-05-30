use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
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
        #[arg(long)]
        metric: String,
        /// Delta from previous
        #[arg(long, default_value = "0")]
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
        /// Decision: auto, keep, discard, crash, no-op
        #[arg(long, default_value = "auto")]
        decision: String,
        /// Trial metric value
        #[arg(long)]
        metric: String,
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
            cwd,
        } => cmd_decide(
            &decision,
            &metric,
            metrics_json.as_deref(),
            commit.as_deref(),
            &description,
            &rollback,
            &guard,
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
    iterations: Option<u32>,
    run_tag: Option<String>,
    stop_condition: Option<String>,
    run_mode: Option<String>,
    workspace_root: Option<PathBuf>,
    primary_repo: Option<PathBuf>,
    rollback: &str,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_cwd(cwd);
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

    // Safety screen
    verify::screen_command(verify_cmd)?;

    // Verify git repo
    let git = GitRepo::open(&workspace).context("autoresearch requires a git repository")?;
    let head = git.head_short()?;

    // Measure baseline
    let result = verify::run_verify(verify_cmd, fmt, key, &workspace)
        .context("Baseline verification failed")?;

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
        rollback_strategy,
        run_mode,
        workspace_root,
        primary_repo,
    };
    let acceptance_criteria_count = run_config.acceptance_criteria.len();
    let required_keep_criteria_count = run_config.required_keep_criteria.len();

    // Write state.json
    let state = RunState::from_baseline(result.metric, head.clone(), Some(run_config));
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
    let workspace = resolve_cwd(cwd);
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
    metric_str: &str,
    metrics_json: Option<&str>,
    commit: Option<&str>,
    description: &str,
    rollback_str: &str,
    guard_str: &str,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_cwd(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");

    let metric =
        Decimal::from_str(metric_str).with_context(|| format!("Invalid metric: {metric_str}"))?;

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
    let (primary_metric_key, verify_format, acceptance_criteria, required_keep_criteria) =
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
                config.acceptance_criteria.clone(),
                config.required_keep_criteria.clone(),
            ),
            None => (
                "metric".to_string(),
                VerifyFormat::Scalar,
                Vec::new(),
                Vec::new(),
            ),
        };

    let delta = metric - state.current_metric;
    let trial_metrics =
        build_trial_metrics(metric, metrics_json, &primary_metric_key, verify_format)?;
    let acceptance = criteria::evaluate_criteria(&acceptance_criteria, &trial_metrics);
    let required_keep = criteria::evaluate_criteria(&required_keep_criteria, &trial_metrics);
    let decision = if decision == "auto" {
        if state.direction.is_improvement(delta) {
            "keep"
        } else {
            "discard"
        }
    } else {
        decision
    };
    let decision = if decision == "keep" && !required_keep.satisfied {
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
            state.record_keep(metric, resolved_commit.clone().unwrap());
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
            state.record_discard(metric, resolved_commit.clone());
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
        other => anyhow::bail!("Unknown decision: {other}. Use keep, discard, crash, or no-op."),
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
        description: description.to_string(),
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
            let default = cwd.join("autoresearch-results/results.tsv");
            if default.exists() {
                default
            } else {
                anyhow::bail!("No results.tsv found. Provide a path or run from project root.");
            }
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

    let efficiency = if total > 1 {
        (keeps as f64 / (total - 1) as f64 * 100.0).round() as u32
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
    } else if recent_keeps.windows(2).all(|w| w[0] >= w[1]) {
        "improving"
    } else if recent_keeps.windows(2).all(|w| w[0] <= w[1]) {
        "declining"
    } else {
        "flat"
    };

    match format {
        "json" => {
            let out = serde_json::json!({
                "direction": direction,
                "total_iterations": total - 1,
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
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        _ => {
            println!("## Autoresearch Evals");
            println!();
            println!("| Stat | Value |");
            println!("|------|-------|");
            println!("| Direction | {direction} |");
            println!("| Iterations | {} |", total.saturating_sub(1));
            println!("| Kept | {keeps} |");
            println!("| Discarded | {discards} |");
            println!("| Crashes | {crashes} |");
            println!("| Efficiency | {efficiency}% |");
            println!("| Baseline | {baseline} |");
            println!("| Final | {final_metric} |");
            println!("| Best | {best} |");
            println!("| Trend | {trend} |");
            println!("| Longest plateau | {longest_plateau} iterations |");
            println!();
            if !top_keeps.is_empty() {
                println!("### Top Improvements");
                println!();
                for (i, (delta, desc)) in top_keeps.iter().take(5).enumerate() {
                    println!("{}. **{delta}** — {desc}", i + 1);
                }
                println!();
            }
            // Recommendations
            println!("### Recommendations");
            println!();
            if longest_plateau >= 5 {
                println!("- ⚠️ Plateau of {longest_plateau} iterations detected. Consider a PIVOT strategy.");
            }
            if crashes > keeps {
                println!("- ⚠️ More crashes than keeps. Check verify command reliability.");
            }
            if efficiency < 20 && total > 10 {
                println!(
                    "- ⚠️ Low efficiency ({efficiency}%). Hypotheses may need better grounding."
                );
            }
            if trend == "declining" {
                println!("- ⚠️ Declining trend. Recent changes may be counterproductive.");
            }
            if trend == "improving" && efficiency > 30 {
                println!("- ✅ Strong trajectory. Continue current approach.");
            }
            if longest_plateau < 3 && efficiency > 40 {
                println!("- ✅ Healthy run. Good keep rate with no extended plateaus.");
            }
        }
    }

    Ok(())
}

// ── Status ────────────────────────────────────────────────────────────

fn cmd_status(cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_cwd(cwd);
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
    let workspace = resolve_cwd(cwd);
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
            let workspace = resolve_cwd(cwd);
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
            let workspace = resolve_cwd(cwd);
            let snapshot = runtime::runtime_status(&workspace)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
        RuntimeCommands::Supervise {
            after_run,
            max_stagnation,
            cwd,
        } => {
            let workspace = resolve_cwd(cwd);
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
            let workspace = resolve_cwd(cwd);
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
            let workspace = resolve_cwd(cwd);
            let snapshot = runtime::stop_runtime(&workspace)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
    }
    Ok(())
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
    let workspace = resolve_cwd(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");

    if !state_path.exists() {
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

// ── Progress ─────────────────────────────────────────────────────────

fn cmd_progress(cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_cwd(cwd);
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
        } else if last5.windows(2).all(|w| w[0] >= w[1]) {
            "improving"
        } else if last5.windows(2).all(|w| w[0] <= w[1]) {
            "declining"
        } else {
            "flat"
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
    let workspace = resolve_cwd(cwd);
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
    let workspace = resolve_cwd(cwd);
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
    let workspace = resolve_cwd(cwd);

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

    let state = RunState::from_baseline(result.metric, head.clone(), Some(config));
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

fn build_trial_metrics(
    primary_metric: Decimal,
    metrics_json: Option<&str>,
    primary_metric_key: &str,
    verify_format: VerifyFormat,
) -> Result<BTreeMap<String, Decimal>> {
    match metrics_json {
        Some(raw) => autoresearch::core::metrics::parse_json_metrics_map(raw),
        None if verify_format == VerifyFormat::MetricsJson => Ok(BTreeMap::from([(
            primary_metric_key.to_string(),
            primary_metric,
        )])),
        None => Ok(BTreeMap::from([(
            primary_metric_key.to_string(),
            primary_metric,
        )])),
    }
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
