use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::path::PathBuf;

use crate::core::config::{RollbackStrategy, RunConfig};
use crate::core::git::{GitRepo, WorktreeStatus};
use crate::core::results::{GuardResult, ResultRow, ResultsLog};
use crate::core::state::{IterationStatus, RunState, StopReason};
use crate::core::verify::{run_guard, run_verify, screen_command};
use crate::escalation::lessons::{self, LessonsLog};
use crate::escalation::pivot::{EscalationAction, EscalationState};

/// Run the core autoresearch iteration loop.
///
/// This is the protocol implementation that both Claude Code and Codex call.
/// The agent provides the config, this function runs the mechanical loop.
pub fn run(_args: &[String]) -> Result<()> {
    // In practice, this function is invoked by the CLI with a pre-validated config.
    // The agent (Claude/Codex) handles the interactive setup and passes config as JSON.
    eprintln!("autoresearch loop: waiting for config on stdin (JSON)");

    let config: RunConfig = serde_json::from_reader(std::io::stdin().lock())
        .context("Failed to parse run config from stdin")?;

    run_loop(config)
}

/// Execute the loop with a validated configuration.
pub fn run_loop(config: RunConfig) -> Result<()> {
    let workspace = config
        .workspace_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Phase 0: Preconditions
    let git = GitRepo::open(&workspace).context("autoresearch requires a git repository")?;

    // Safety screen
    screen_command(&config.verify)?;
    if let Some(ref guard) = config.guard {
        screen_command(guard)?;
    }

    // Worktree check
    match git.worktree_status()? {
        WorktreeStatus::Clean | WorktreeStatus::OnlyArtifacts => {}
        WorktreeStatus::Dirty(files) => {
            anyhow::bail!(
                "Working tree has uncommitted changes outside autoresearch artifacts:\n  {}",
                files.join("\n  ")
            );
        }
    }

    // Phase 2: Baseline
    eprintln!("Establishing baseline...");
    let baseline_result = run_verify(
        &config.verify,
        config.verify_format,
        config.primary_metric_key.as_deref(),
        &workspace,
    )
    .context("Baseline verification failed")?;

    let baseline_commit = git.head_short()?;

    // Initialize state
    let mut state = RunState::from_baseline(baseline_result.metric, baseline_commit.clone());

    // Initialize results log
    let results_dir = crate::core::results::results_dir(&workspace);
    let log = ResultsLog::create(&results_dir, config.direction)?;

    let baseline_row = ResultRow {
        iteration: 0,
        commit: Some(baseline_commit),
        metric: baseline_result.metric,
        delta: Decimal::ZERO,
        guard: GuardResult::Skip,
        status: IterationStatus::Baseline,
        description: "initial state".to_string(),
    };
    log.append(&baseline_row)?;

    // Initialize lessons
    let lessons_log = LessonsLog::open_or_create(&results_dir)?;

    // Save initial state
    let state_json = serde_json::to_string_pretty(&state)?;
    std::fs::write(results_dir.join("state.json"), &state_json)?;

    eprintln!(
        "Baseline: {} (direction: {})",
        baseline_result.metric,
        config.direction.as_str()
    );

    // Escalation state
    let mut escalation = EscalationState::default();

    // Phase 9: Iteration loop
    let max_iter = config.effective_iterations();
    let mut current_iter: u32 = 0;

    loop {
        current_iter += 1;

        if let Some(max) = max_iter {
            if current_iter > max {
                state.complete(StopReason::IterationCap);
                break;
            }
        }

        // The agent performs Phase 1 (Read), Phase 3 (Ideate), Phase 4 (Modify)
        // externally. This binary handles Phase 5-8 mechanically.
        //
        // In the full integration, the agent invokes:
        //   autoresearch verify   — runs verify + guard, returns JSON result
        //   autoresearch decide   — applies keep/discard based on result
        //   autoresearch log      — records the iteration
        //
        // For standalone operation, the loop runs verify on whatever the
        // current HEAD contains and reports the result.

        // Phase 6: Verify
        let verify_result = match run_verify(
            &config.verify,
            config.verify_format,
            config.primary_metric_key.as_deref(),
            &workspace,
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Verify failed: {e}");
                state.record_crash();
                let action = escalation.record_crash();
                handle_escalation_action(action, &mut state)?;

                log.append(&ResultRow {
                    iteration: current_iter,
                    commit: None,
                    metric: state.current_metric,
                    delta: Decimal::ZERO,
                    guard: GuardResult::Skip,
                    status: IterationStatus::Crash,
                    description: format!("verify error: {e}"),
                })?;
                continue;
            }
        };

        let delta = verify_result.metric - state.current_metric;
        let improved = config.direction.is_improvement(delta);

        // Phase 6.5: Guard
        let guard_result = if let Some(ref guard_cmd) = config.guard {
            if improved {
                match run_guard(guard_cmd, &workspace) {
                    Ok(r) => {
                        if r.passed {
                            GuardResult::Pass
                        } else {
                            GuardResult::Fail
                        }
                    }
                    Err(_) => GuardResult::Fail,
                }
            } else {
                GuardResult::Skip
            }
        } else {
            GuardResult::Skip
        };

        // Phase 7: Decide
        let (status, description) = if !improved {
            // Discard
            rollback(&git, config.rollback_strategy)?;
            state.record_discard(verify_result.metric, None);
            let action = escalation.record_discard();
            handle_escalation_action(action, &mut state)?;
            (
                IterationStatus::Discard,
                format!("metric {}: {delta}", if delta.is_zero() { "flat" } else { "regressed" }),
            )
        } else if guard_result == GuardResult::Fail {
            // Guard failed
            rollback(&git, config.rollback_strategy)?;
            state.record_discard(verify_result.metric, None);
            let action = escalation.record_discard();
            handle_escalation_action(action, &mut state)?;
            (IterationStatus::Discard, "guard failed".to_string())
        } else if improved
            && crate::core::metrics::is_marginal(verify_result.metric, state.baseline_metric)
        {
            // Marginal gain — check simplicity override
            // For now, keep marginal gains (agent decides complexity)
            let commit = git.head_short()?;
            state.record_keep(verify_result.metric, commit);
            escalation.record_keep();

            // Extract lesson
            let lesson =
                lessons::extract_keep_lesson("marginal improvement", &delta.to_string());
            let _ = lessons_log.append(&lesson);

            (
                IterationStatus::Keep,
                format!("marginal keep: {delta}"),
            )
        } else {
            // Keep!
            let commit = git.head_short()?;
            state.record_keep(verify_result.metric, commit.clone());
            escalation.record_keep();

            let desc = format!("improvement: {delta}");
            let lesson = lessons::extract_keep_lesson(&desc, &delta.to_string());
            let _ = lessons_log.append(&lesson);

            (IterationStatus::Keep, desc)
        };

        // Phase 8: Log
        let row = ResultRow {
            iteration: current_iter,
            commit: if status == IterationStatus::Keep {
                Some(git.head_short()?)
            } else {
                None
            },
            metric: verify_result.metric,
            delta,
            guard: guard_result,
            status,
            description,
        };
        log.append(&row)?;

        // Update persisted state
        let state_json = serde_json::to_string_pretty(&state)?;
        std::fs::write(results_dir.join("state.json"), &state_json)?;

        // Progress report every 5 iterations
        if current_iter % 5 == 0 {
            eprintln!(
                "--- Progress (iteration {current_iter}) ---\n\
                 Baseline: {} → Current: {} (best: {})\n\
                 Kept: {} | Discarded: {} | Crashes: {}",
                state.baseline_metric,
                state.current_metric,
                state.best_metric,
                state.keeps,
                state.discards,
                state.crashes,
            );
        }

        // Check if escalation triggered a stop
        if matches!(state.phase, crate::core::state::RunPhase::Blocked { .. }) {
            break;
        }
    }

    // Completion summary
    let summary = crate::core::results::completion_summary(
        state.baseline_metric,
        state.current_metric,
        state.best_metric,
        state.keeps,
        state.discards,
        state.crashes,
        current_iter.saturating_sub(1),
        config.direction,
    );
    println!("{summary}");

    // Final state persist
    let state_json = serde_json::to_string_pretty(&state)?;
    std::fs::write(results_dir.join("state.json"), &state_json)?;

    Ok(())
}

fn rollback(git: &GitRepo, strategy: RollbackStrategy) -> Result<()> {
    match strategy {
        RollbackStrategy::HardReset => git.hard_reset_head(),
        RollbackStrategy::Revert => git.revert_head(),
    }
}

fn handle_escalation_action(action: EscalationAction, state: &mut RunState) -> Result<()> {
    match action {
        EscalationAction::None => {}
        EscalationAction::Refine => {
            eprintln!("⚡ REFINE: {}", action.guidance());
        }
        EscalationAction::Pivot => {
            eprintln!("🔄 PIVOT: {}", action.guidance());
        }
        EscalationAction::WebSearch => {
            eprintln!("🔍 WEB SEARCH: {}", action.guidance());
        }
        EscalationAction::SoftBlocker => {
            eprintln!("🛑 SOFT BLOCKER: {}", action.guidance());
            state.record_blocked(action.guidance().to_string());
        }
    }
    Ok(())
}
