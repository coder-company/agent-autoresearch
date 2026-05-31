use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::Write as IoWrite;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::time::{Duration, Instant};

use autoresearch::core::config::{
    Direction, RepoTargetConfig, RollbackStrategy, RunConfig, RunMode, VerifyFormat,
};
use autoresearch::core::context;
use autoresearch::core::criteria;
use autoresearch::core::git::{GitRepo, WorktreeStatus};
use autoresearch::core::health;
use autoresearch::core::results::{
    ensure_results_dir_protected, parse_metric_direction_value, worker_iteration_prefix,
    GuardResult, ResultRow, ResultsLog,
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

#[derive(Debug, Default, serde::Deserialize)]
struct ProjectConfig {
    goal: Option<String>,
    scope: Option<Vec<String>>,
    metric: Option<String>,
    direction: Option<String>,
    verify: Option<String>,
    guard: Option<String>,
    iterations: Option<u32>,
    run_tag: Option<String>,
    stop_condition: Option<String>,
    environment_summary: Option<String>,
    run_mode: Option<String>,
    format: Option<String>,
    verify_format: Option<String>,
    key: Option<String>,
    primary_metric_key: Option<String>,
    acceptance_criteria: Option<String>,
    required_keep_criteria: Option<String>,
    required_keep_label: Option<Vec<String>>,
    required_stop_label: Option<Vec<String>>,
    companion_repo_scope: Option<Vec<String>>,
    rollback: Option<String>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Initialize a new run: measure baseline, create results dir, write state
    Init {
        /// Verify command to establish baseline
        #[arg(long)]
        verify: Option<String>,
        /// Metric direction: higher or lower
        #[arg(long)]
        direction: Option<String>,
        /// Verify output format: scalar or metrics_json
        #[arg(long)]
        format: Option<String>,
        /// Primary metric key (for metrics_json)
        #[arg(long)]
        key: Option<String>,
        /// Project defaults file (default: .autoresearch.toml when present)
        #[arg(long)]
        config: Option<PathBuf>,
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
        /// Environment profile summary to persist in results.tsv metadata
        #[arg(long)]
        environment_summary: Option<String>,
        /// Run mode: foreground or background
        #[arg(long)]
        run_mode: Option<String>,
        /// Workspace root for run artifacts
        #[arg(long)]
        workspace_root: Option<PathBuf>,
        /// Primary repository for scoped runs
        #[arg(long)]
        primary_repo: Option<PathBuf>,
        /// Companion repository and editable scope, as PATH=SCOPE (repeatable)
        #[arg(long)]
        companion_repo_scope: Vec<String>,
        /// Rollback strategy: revert or hard-reset
        #[arg(long)]
        rollback: Option<String>,
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

    /// Suggest guard command presets for primary and companion repos
    GuardPresets {
        /// Output format: json or text
        #[arg(long, default_value = "json")]
        format: String,
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
        /// Print compact status fields only
        #[arg(long)]
        summary: bool,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Check runtime health: git state, artifacts, disk, and verify command
    Health {
        /// Verify command to check; defaults to state.json config when present
        #[arg(long)]
        verify: Option<String>,
        /// Exit non-zero when warnings are present
        #[arg(long)]
        strict: bool,
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

    /// Serve Autoresearch tools over MCP stdio
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
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

    /// Tail the active results.tsv for live run monitoring
    Watch {
        /// Number of recent data rows to print on startup
        #[arg(long, default_value_t = 20)]
        lines: usize,
        /// Output format: tsv or jsonl
        #[arg(long, default_value = "tsv")]
        format: String,
        /// Print once and exit instead of following
        #[arg(long)]
        once: bool,
        /// Poll interval while following
        #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(1..))]
        interval_ms: u64,
        /// Serve watch events over a WebSocket instead of writing rows to stdout
        #[arg(long)]
        websocket: bool,
        /// WebSocket bind address
        #[arg(long, default_value = "127.0.0.1:8765")]
        websocket_addr: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Query the lessons.md file for relevant strategies
    Lessons {
        /// Append a lesson strategy/context entry
        #[arg(long)]
        add: Option<String>,
        /// Lesson category for --add: positive, negative, or strategic
        #[arg(long, default_value = "strategic")]
        category: String,
        /// Lesson outcome for --add: success, failure, or neutral
        #[arg(long, default_value = "neutral")]
        outcome: String,
        /// Context for --add
        #[arg(long, default_value = "manual")]
        context: String,
        /// Filter lessons containing this query (case-insensitive)
        #[arg(long)]
        search: Option<String>,
        /// Return last N lessons
        #[arg(long)]
        last: Option<usize>,
        /// Include workspace root and repo target metadata in lesson query output
        #[arg(long)]
        workspace_context: bool,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Run a configurable web search helper with local result caching
    Search {
        /// Search query. Omit with --from-state to derive one from run state.
        #[arg(long)]
        query: Option<String>,
        /// Build the query from state.json and escalation.json
        #[arg(long)]
        from_state: bool,
        /// Provider command. Receives AUTORESEARCH_SEARCH_QUERY and AUTORESEARCH_SEARCH_LIMIT.
        #[arg(long)]
        provider_command: Option<String>,
        /// Maximum result count hint passed to the provider
        #[arg(long, default_value_t = 5)]
        limit: usize,
        /// Ignore cached results and run the provider again
        #[arg(long)]
        refresh: bool,
        /// Append a search meta-row to the active run log
        #[arg(long)]
        log: bool,
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
        /// Comma-separated downstream command targets
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
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

    /// Generate shell completions for bash, zsh, fish, elvish, or PowerShell
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Generate Unix man pages for local packaging
    Manpages {
        /// Directory where man pages should be written
        #[arg(long)]
        output_dir: PathBuf,
    },

    /// Print the stable CLI API manifest for wrappers and agents
    Api {
        /// Output format: json or md
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Manage project-level autoresearch configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Load and validate local mode plugin manifests
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },

    /// Inspect workspace-aware run scopes
    Scope {
        #[command(subcommand)]
        command: ScopeCommands,
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
    /// Create isolated git worktrees and worker result files for a parallel batch
    Prepare {
        /// Number of workers to prepare
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(1..=3))]
        workers: u8,
        /// Directory for worker worktrees. Relative paths resolve from the run workspace.
        #[arg(long)]
        worktree_root: Option<PathBuf>,
        /// Output manifest path. Relative paths resolve from the run workspace.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Output worker batch JSON path. Relative paths resolve from the run workspace.
        #[arg(long)]
        batch_file: Option<PathBuf>,
        /// Branch name prefix for worker branches
        #[arg(long, default_value = "autoresearch/parallel")]
        branch_prefix: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Remove parallel worker worktrees and branches from a prepare manifest
    Cleanup {
        /// Manifest written by `autoresearch parallel prepare`
        #[arg(long)]
        manifest: PathBuf,
        /// Keep worker branches after removing worktrees
        #[arg(long)]
        keep_branches: bool,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Run prepared worker prompts through `codex exec` in their worktrees
    Run {
        /// Manifest written by `autoresearch parallel prepare`
        #[arg(long)]
        manifest: PathBuf,
        /// Execution policy for nested Codex workers
        #[arg(long, default_value = "danger_full_access")]
        execution_policy: String,
        /// Codex binary to launch
        #[arg(long, default_value = "codex")]
        codex_bin: String,
        /// Kill workers that run longer than this many seconds
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_seconds: Option<u64>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Generate an editable worker batch JSON template for parallel closeout
    Template {
        /// Number of workers to include in the template
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(1..=3))]
        workers: u8,
        /// Optional output file. Relative paths resolve from the run workspace.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Select the best worker result and record the batch as one authoritative iteration
    Closeout {
        /// JSON array of worker results
        #[arg(long)]
        batch_file: PathBuf,
        /// Merge strategy: cherry-pick, fast-forward, squash, or rebase
        #[arg(long, default_value = "cherry-pick")]
        merge_strategy: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Print or write a starter .autoresearch.toml
    Template {
        /// Output path to write instead of printing to stdout
        #[arg(long)]
        output: Option<PathBuf>,
        /// Replace an existing output file
        #[arg(long)]
        force: bool,
    },
    /// Validate a project defaults file without running verify or guard
    Validate {
        /// Config path (default: .autoresearch.toml in the workspace root)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ScopeCommands {
    /// Expand active primary and companion repo scope globs
    Expand {
        /// Package boundary filename. Repeat to override the defaults.
        #[arg(long = "package-boundary")]
        package_boundary: Vec<String>,
        /// Output format: json or text
        #[arg(long, default_value = "json")]
        format: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List plugin manifests from .autoresearch/plugins or a custom directory
    List {
        /// Plugin directory. Relative paths resolve from the workspace root.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Validate one plugin manifest
    Validate {
        /// Plugin manifest TOML path
        #[arg(long)]
        path: PathBuf,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Validate a plugin marketplace TOML index
    Marketplace {
        /// Marketplace TOML path. Defaults to .autoresearch/plugins/marketplace.toml.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum McpCommands {
    /// Start an MCP stdio server exposing read-only Autoresearch tools
    Serve {
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Call one tool on an external MCP stdio server
    Call {
        /// Command that starts the MCP stdio server
        #[arg(long)]
        server_command: String,
        /// Tool name to call
        #[arg(long)]
        tool: String,
        /// Tool arguments as a JSON object
        #[arg(long, default_value = "{}")]
        arguments: String,
        /// Working directory for the MCP server process
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
            config,
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
            environment_summary,
            run_mode,
            workspace_root,
            primary_repo,
            companion_repo_scope,
            rollback,
            cwd,
        } => cmd_init(
            verify_cmd,
            direction,
            format,
            key,
            config,
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
            environment_summary,
            run_mode,
            workspace_root,
            primary_repo,
            companion_repo_scope,
            rollback,
            cwd,
        ),

        Commands::Verify {
            command,
            format,
            key,
            cwd,
        } => cmd_verify(&command, &format, key.as_deref(), cwd),

        Commands::Guard { command, cwd } => cmd_guard(&command, cwd),

        Commands::GuardPresets { format, cwd } => cmd_guard_presets(&format, cwd),

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

        Commands::Status { summary, cwd } => cmd_status(cwd, summary),

        Commands::Health {
            verify,
            strict,
            min_free_mb,
            cwd,
        } => cmd_health(verify.as_deref(), strict, min_free_mb, cwd),

        Commands::Runtime { command } => cmd_runtime(command),

        Commands::Parallel { command } => cmd_parallel(command),

        Commands::Mcp { command } => match command {
            McpCommands::Serve { cwd } => cmd_mcp_serve(cwd),
            McpCommands::Call {
                server_command,
                tool,
                arguments,
                cwd,
            } => cmd_mcp_call(&server_command, &tool, &arguments, cwd),
        },

        Commands::Screen { command } => cmd_screen(&command),

        Commands::Hook { name } => hooks::dispatch(&name),

        Commands::Resume { cwd } => cmd_resume(cwd),

        Commands::Progress { cwd } => cmd_progress(cwd),

        Commands::Watch {
            lines,
            format,
            once,
            interval_ms,
            websocket,
            websocket_addr,
            cwd,
        } => cmd_watch(
            cwd,
            lines,
            &format,
            once,
            interval_ms,
            websocket,
            &websocket_addr,
        ),

        Commands::Lessons {
            add,
            category,
            outcome,
            context,
            search,
            last,
            workspace_context,
            cwd,
        } => cmd_lessons(
            add.as_deref(),
            &category,
            &outcome,
            &context,
            search.as_deref(),
            last,
            workspace_context,
            cwd,
        ),

        Commands::Search {
            query,
            from_state,
            provider_command,
            limit,
            refresh,
            log,
            cwd,
        } => cmd_search(
            query,
            from_state,
            provider_command,
            limit,
            refresh,
            log,
            cwd,
        ),

        Commands::Handoff {
            source,
            status,
            findings,
            config,
            chain,
            evals,
            evals_interval,
            cwd,
        } => cmd_handoff(
            &source,
            &status,
            findings.as_deref(),
            config.as_deref(),
            chain.as_deref(),
            evals,
            evals_interval,
            cwd,
        ),

        Commands::Exec { iterations, cwd } => cmd_exec(iterations, cwd),

        Commands::Completions { shell } => cmd_completions(shell),
        Commands::Manpages { output_dir } => cmd_manpages(&output_dir),
        Commands::Api { format } => cmd_api(&format),
        Commands::Config { command } => match command {
            ConfigCommands::Template { output, force } => cmd_config_template(output, force),
            ConfigCommands::Validate { path, cwd } => cmd_config_validate(path, cwd),
        },
        Commands::Plugin { command } => match command {
            PluginCommands::List { dir, cwd } => cmd_plugin_list(dir, cwd),
            PluginCommands::Validate { path, cwd } => cmd_plugin_validate(path, cwd),
            PluginCommands::Marketplace { path, cwd } => cmd_plugin_marketplace(path, cwd),
        },
        Commands::Scope { command } => match command {
            ScopeCommands::Expand {
                package_boundary,
                format,
                cwd,
            } => cmd_scope_expand(package_boundary, &format, cwd),
        },
    }
}

fn cmd_completions(shell: clap_complete::Shell) -> Result<()> {
    let mut command = Cli::command();
    let mut stdout = std::io::stdout();
    clap_complete::generate(shell, &mut command, "autoresearch", &mut stdout);
    Ok(())
}

fn cmd_manpages(output_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let page_path = output_dir.join("autoresearch.1");
    let mut page = Vec::new();
    clap_mangen::Man::new(Cli::command())
        .render(&mut page)
        .context("failed to render autoresearch man page")?;
    std::fs::write(&page_path, page)
        .with_context(|| format!("failed to write {}", page_path.display()))?;

    println!(
        "{}",
        serde_json::json!({
            "generated": [page_path],
        })
    );
    Ok(())
}

fn cmd_api(format: &str) -> Result<()> {
    let manifest = cli_api_manifest();
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&manifest)?);
            Ok(())
        }
        "md" => {
            print!("{}", render_cli_api_markdown(&manifest));
            Ok(())
        }
        other => anyhow::bail!("Invalid api format {other:?}; use json or md"),
    }
}

fn cli_api_manifest() -> serde_json::Value {
    let command = Cli::command();
    let mut commands = Vec::new();
    collect_cli_commands(&command, Vec::new(), &mut commands);
    serde_json::json!({
        "schema_version": 1,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "stability": "stable",
        "semver_policy": {
            "breaking_changes": "major version",
            "additive_commands_or_flags": "minor version",
            "bugfixes_and_documentation": "patch version",
            "output_format_changes": "major version for stable commands",
        },
        "commands": commands,
    })
}

fn collect_cli_commands(
    command: &clap::Command,
    prefix: Vec<String>,
    commands: &mut Vec<serde_json::Value>,
) {
    for subcommand in command.get_subcommands() {
        let mut path = prefix.clone();
        path.push(subcommand.get_name().to_string());
        commands.push(serde_json::json!({
            "path": path,
            "about": subcommand.get_about().map(|about| about.to_string()),
            "args": subcommand
                .get_arguments()
                .map(cli_arg_manifest)
                .collect::<Vec<_>>(),
        }));
        collect_cli_commands(subcommand, path, commands);
    }
}

fn cli_arg_manifest(arg: &clap::Arg) -> serde_json::Value {
    let default_values = arg
        .get_default_values()
        .iter()
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let value_names = arg.get_value_names().map(|names| {
        names
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    });
    serde_json::json!({
        "id": arg.get_id().as_str(),
        "long": arg.get_long(),
        "short": arg.get_short().map(|short| short.to_string()),
        "required": arg.is_required_set(),
        "action": format!("{:?}", arg.get_action()),
        "value_names": value_names,
        "default_values": default_values,
        "help": arg.get_help().map(|help| help.to_string()),
    })
}

fn render_cli_api_markdown(manifest: &serde_json::Value) -> String {
    let mut out = String::new();
    writeln!(out, "# Autoresearch CLI API").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Schema version: {}",
        manifest["schema_version"].as_u64().unwrap_or(0)
    )
    .unwrap();
    writeln!(
        out,
        "- CLI version: {}",
        manifest["cli_version"].as_str().unwrap_or("unknown")
    )
    .unwrap();
    writeln!(
        out,
        "- Stability: {}",
        manifest["stability"].as_str().unwrap_or("unknown")
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Commands").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Command | Args |").unwrap();
    writeln!(out, "|---------|------|").unwrap();
    if let Some(commands) = manifest["commands"].as_array() {
        for command in commands {
            let path = command["path"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| part.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let args = command["args"]
                .as_array()
                .map(|args| {
                    args.iter()
                        .filter_map(|arg| arg["long"].as_str().or_else(|| arg["id"].as_str()))
                        .map(|arg| format!("`--{arg}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|args| !args.is_empty())
                .unwrap_or_else(|| "-".to_string());
            writeln!(out, "| `{path}` | {args} |").unwrap();
        }
    }
    out
}

fn cmd_config_template(output: Option<PathBuf>, force: bool) -> Result<()> {
    let template = project_config_template();
    let Some(path) = output else {
        print!("{template}");
        return Ok(());
    };

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true);
    if force {
        options.truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(template.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;

    println!(
        "{}",
        serde_json::json!({
            "written": path,
        })
    );
    Ok(())
}

fn project_config_template() -> &'static str {
    r#"# Autoresearch project defaults.
# CLI flags passed to `autoresearch init` override these values.
goal = "Reduce failing tests"
scope = ["src/**/*.rs", "tests/**/*.rs"]
metric = "failing test count"
direction = "lower"
verify = "cargo test 2>&1 | tail -1"
guard = "cargo fmt -- --check"
iterations = 25
run_tag = "local"

# Optional:
# format = "scalar"
# key = "coverage"
# acceptance_criteria = "[{\"metric\":\"coverage\",\"op\":\">=\",\"value\":\"90\"}]"
# required_keep_criteria = "[{\"metric\":\"failing\",\"op\":\"<=\",\"value\":\"0\"}]"
"#
}

fn cmd_config_validate(path: Option<PathBuf>, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let config_path = resolve_project_config_path(&workspace, path);
    if !config_path.exists() {
        anyhow::bail!("config file not found: {}", config_path.display());
    }

    let config = load_project_config(&workspace, Some(config_path.clone()))?;
    validate_project_config(&config)?;
    println!(
        "{}",
        serde_json::json!({
            "valid": true,
            "path": config_path,
        })
    );
    Ok(())
}

fn validate_project_config(config: &ProjectConfig) -> Result<()> {
    if let Some(direction) = &config.direction {
        parse_direction(direction).context("invalid direction in .autoresearch.toml")?;
    }
    if let Some(format) = &config.format {
        parse_format(format).context("invalid format in .autoresearch.toml")?;
    }
    if let Some(format) = &config.verify_format {
        parse_format(format).context("invalid verify_format in .autoresearch.toml")?;
    }
    if let Some(run_mode) = &config.run_mode {
        parse_run_mode(run_mode).context("invalid run_mode in .autoresearch.toml")?;
    }
    if let Some(rollback) = &config.rollback {
        parse_rollback_strategy(rollback).context("invalid rollback in .autoresearch.toml")?;
    }
    if config.iterations == Some(0) {
        anyhow::bail!("iterations must be greater than zero in .autoresearch.toml");
    }
    criteria::parse_criteria_json(config.acceptance_criteria.as_deref(), "acceptance_criteria")?;
    criteria::parse_criteria_json(
        config.required_keep_criteria.as_deref(),
        "required_keep_criteria",
    )?;
    if let Some(verify_cmd) = &config.verify {
        verify::screen_command(verify_cmd).context("unsafe verify in .autoresearch.toml")?;
    }
    if let Some(guard_cmd) = &config.guard {
        verify::screen_command(guard_cmd).context("unsafe guard in .autoresearch.toml")?;
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ModePluginManifest {
    name: String,
    version: String,
    mode: String,
    command: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    scopes: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PluginMarketplace {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    plugins: Vec<PluginMarketplaceEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct PluginMarketplaceEntry {
    name: String,
    path: PathBuf,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

fn cmd_plugin_list(dir: Option<PathBuf>, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let plugin_dir = dir
        .map(|path| resolve_workspace_path(&workspace, path))
        .unwrap_or_else(|| workspace.join(".autoresearch/plugins"));
    let mut plugins = Vec::new();
    if plugin_dir.exists() {
        for entry in std::fs::read_dir(&plugin_dir)
            .with_context(|| format!("failed to read {}", plugin_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("marketplace.toml") {
                continue;
            }
            let manifest = load_plugin_manifest(&path)?;
            plugins.push(serde_json::json!({
                "path": path.display().to_string(),
                "manifest": manifest,
            }));
        }
    }
    plugins.sort_by_key(|plugin| {
        plugin["manifest"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "plugin_dir": plugin_dir.display().to_string(),
            "plugins": plugins,
        }))?
    );
    Ok(())
}

fn cmd_plugin_marketplace(path: Option<PathBuf>, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let path = path
        .map(|path| resolve_workspace_path(&workspace, path))
        .unwrap_or_else(|| workspace.join(".autoresearch/plugins/marketplace.toml"));
    let marketplace: PluginMarketplace = toml::from_str(
        &std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    if marketplace.plugins.is_empty() {
        anyhow::bail!("plugin marketplace must contain at least one [[plugins]] entry");
    }

    let base_dir = path.parent().unwrap_or(&workspace);
    let mut plugins = Vec::new();
    let mut seen_names = BTreeSet::new();
    for entry in marketplace.plugins {
        if entry.name.trim().is_empty() {
            anyhow::bail!("plugin marketplace entry name must not be empty");
        }
        if !seen_names.insert(entry.name.clone()) {
            anyhow::bail!("duplicate plugin marketplace entry {}", entry.name);
        }
        let manifest_path = if entry.path.is_absolute() {
            entry.path.clone()
        } else {
            base_dir.join(&entry.path)
        };
        let manifest = load_plugin_manifest(&manifest_path)
            .with_context(|| format!("marketplace entry {}", entry.name))?;
        if manifest.name != entry.name {
            anyhow::bail!(
                "marketplace entry {} points to manifest named {}",
                entry.name,
                manifest.name
            );
        }
        plugins.push(serde_json::json!({
            "name": entry.name,
            "source": entry.source.unwrap_or_else(|| "local".to_string()),
            "description": entry.description,
            "tags": entry.tags,
            "path": manifest_path.display().to_string(),
            "manifest": manifest,
        }));
    }
    plugins.sort_by_key(|plugin| plugin["name"].as_str().unwrap_or_default().to_string());

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "valid": true,
            "name": marketplace.name.unwrap_or_else(|| "local".to_string()),
            "path": path.display().to_string(),
            "plugins": plugins,
        }))?
    );
    Ok(())
}

fn cmd_plugin_validate(path: PathBuf, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let path = resolve_workspace_path(&workspace, path);
    let manifest = load_plugin_manifest(&path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "valid": true,
            "path": path.display().to_string(),
            "manifest": manifest,
        }))?
    );
    Ok(())
}

fn load_plugin_manifest(path: &Path) -> Result<ModePluginManifest> {
    let manifest: ModePluginManifest = toml::from_str(
        &std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_plugin_manifest(&manifest)
        .with_context(|| format!("invalid plugin manifest {}", path.display()))?;
    Ok(manifest)
}

fn validate_plugin_manifest(manifest: &ModePluginManifest) -> Result<()> {
    for (field, value) in [
        ("name", manifest.name.as_str()),
        ("version", manifest.version.as_str()),
        ("mode", manifest.mode.as_str()),
        ("command", manifest.command.as_str()),
    ] {
        if value.trim().is_empty() {
            anyhow::bail!("plugin {field} must not be empty");
        }
    }
    if !manifest
        .name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        anyhow::bail!("plugin name must contain only lowercase ASCII letters, digits, '-' or '_'");
    }
    verify::screen_command(&manifest.command).context("unsafe plugin command")?;
    Ok(())
}

fn cmd_scope_expand(
    package_boundary: Vec<String>,
    format: &str,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let run_context = load_run_context(&workspace)?;
    let boundaries = normalized_package_boundaries(package_boundary);
    let mut targets = Vec::new();

    for target in &run_context.repo_targets {
        let repo = PathBuf::from(&target.path);
        let mut files = Vec::new();
        for pattern in split_scope_patterns(&target.scope) {
            files.extend(expand_scope_pattern(&repo, pattern)?);
        }
        files.sort();
        files.dedup();
        let file_entries = files
            .iter()
            .map(|path| {
                serde_json::json!({
                    "path": display_repo_relative(&repo, path),
                    "package_root": package_root_for_file(&repo, path, &boundaries),
                })
            })
            .collect::<Vec<_>>();
        targets.push(serde_json::json!({
            "role": target.role,
            "path": target.path,
            "scope": target.scope,
            "file_count": file_entries.len(),
            "files": file_entries,
        }));
    }

    let out = serde_json::json!({
        "workspace_root": run_context.workspace_root,
        "package_boundaries": boundaries,
        "repo_targets": targets,
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&out)?),
        "text" => print!("{}", render_scope_expand_text(&out)),
        other => anyhow::bail!("Invalid scope expand format {other:?}; use json or text"),
    }
    Ok(())
}

fn normalized_package_boundaries(package_boundary: Vec<String>) -> Vec<String> {
    let boundaries = if package_boundary.is_empty() {
        vec![
            "Cargo.toml".to_string(),
            "package.json".to_string(),
            "pyproject.toml".to_string(),
            "go.mod".to_string(),
        ]
    } else {
        package_boundary
    };
    boundaries
        .into_iter()
        .map(|boundary| boundary.trim().to_string())
        .filter(|boundary| !boundary.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn split_scope_patterns(scope: &str) -> Vec<&str> {
    scope
        .split(',')
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .collect()
}

fn expand_scope_pattern(repo: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let pattern_path = if Path::new(pattern).is_absolute() {
        PathBuf::from(pattern)
    } else {
        repo.join(pattern)
    };
    let pattern = pattern_path.to_string_lossy().to_string();
    let mut files = Vec::new();
    for entry in glob::glob(&pattern).with_context(|| format!("invalid scope glob {pattern:?}"))? {
        let path = entry.with_context(|| format!("failed to read scope glob {pattern:?}"))?;
        if path.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn display_repo_relative(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn package_root_for_file(repo: &Path, path: &Path, boundaries: &[String]) -> String {
    let mut current = path.parent().unwrap_or(repo);
    loop {
        if boundaries
            .iter()
            .any(|boundary| current.join(boundary).is_file())
        {
            return display_repo_relative(repo, current);
        }
        if current == repo {
            return ".".to_string();
        }
        let Some(parent) = current.parent() else {
            return ".".to_string();
        };
        current = parent;
    }
}

fn render_scope_expand_text(value: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(targets) = value["repo_targets"].as_array() {
        for target in targets {
            let role = target["role"].as_str().unwrap_or("repo");
            let path = target["path"].as_str().unwrap_or("");
            let file_count = target["file_count"].as_u64().unwrap_or(0);
            writeln!(out, "{role}: {path} ({file_count} files)").unwrap();
            if let Some(files) = target["files"].as_array() {
                for file in files {
                    writeln!(
                        out,
                        "  {} [{}]",
                        file["path"].as_str().unwrap_or(""),
                        file["package_root"].as_str().unwrap_or(".")
                    )
                    .unwrap();
                }
            }
        }
    }
    out
}

fn load_run_context(workspace: &Path) -> Result<context::RunContext> {
    let context_path = workspace.join("autoresearch-results/context.json");
    serde_json::from_str(
        &std::fs::read_to_string(&context_path)
            .with_context(|| format!("failed to read {}", context_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", context_path.display()))
}

// ── Init ──────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn cmd_init(
    verify_cmd: Option<String>,
    direction_str: Option<String>,
    format_str: Option<String>,
    key: Option<String>,
    config_path: Option<PathBuf>,
    goal: Option<String>,
    scope: Option<Vec<String>>,
    metric_desc: Option<String>,
    guard: Option<String>,
    acceptance_criteria_raw: Option<String>,
    required_keep_criteria_raw: Option<String>,
    required_keep_label: Vec<String>,
    required_stop_label: Vec<String>,
    iterations: Option<u32>,
    run_tag: Option<String>,
    stop_condition: Option<String>,
    environment_summary: Option<String>,
    run_mode: Option<String>,
    workspace_root: Option<PathBuf>,
    primary_repo: Option<PathBuf>,
    companion_repo_scope: Vec<String>,
    rollback: Option<String>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let project_config = load_project_config(&workspace, config_path)?;
    let verify_cmd = verify_cmd
        .or(project_config.verify.clone())
        .context("init requires --verify or verify in .autoresearch.toml")?;
    let direction_str = direction_str
        .or(project_config.direction.clone())
        .unwrap_or_else(|| "higher".to_string());
    let format_str = format_str
        .or(project_config.format.clone())
        .or(project_config.verify_format.clone())
        .unwrap_or_else(|| "scalar".to_string());
    let key = key
        .or(project_config.key.clone())
        .or(project_config.primary_metric_key.clone());
    let goal = goal.or(project_config.goal.clone());
    let scope = scope.or(project_config.scope.clone());
    let metric_desc = metric_desc.or(project_config.metric.clone());
    let guard = guard.or(project_config.guard.clone());
    let acceptance_criteria_raw =
        acceptance_criteria_raw.or(project_config.acceptance_criteria.clone());
    let required_keep_criteria_raw =
        required_keep_criteria_raw.or(project_config.required_keep_criteria.clone());
    let required_keep_label = if required_keep_label.is_empty() {
        project_config
            .required_keep_label
            .clone()
            .unwrap_or_default()
    } else {
        required_keep_label
    };
    let required_stop_label = if required_stop_label.is_empty() {
        project_config
            .required_stop_label
            .clone()
            .unwrap_or_default()
    } else {
        required_stop_label
    };
    let iterations = iterations.or(project_config.iterations);
    let run_tag = run_tag.or(project_config.run_tag.clone());
    let stop_condition = stop_condition.or(project_config.stop_condition.clone());
    let environment_summary = environment_summary.or(project_config.environment_summary.clone());
    let run_mode = run_mode.or(project_config.run_mode.clone());
    let companion_repo_scope = if companion_repo_scope.is_empty() {
        project_config
            .companion_repo_scope
            .clone()
            .unwrap_or_default()
    } else {
        companion_repo_scope
    };
    let rollback = rollback
        .or(project_config.rollback.clone())
        .unwrap_or_else(|| "revert".to_string());
    let direction = parse_direction(&direction_str)?;
    let fmt = parse_format(&format_str)?;
    let run_mode = run_mode
        .as_deref()
        .map(parse_run_mode)
        .transpose()
        .context("Invalid run mode")?;
    let rollback_strategy = parse_rollback_strategy(&rollback)?;
    let acceptance_criteria =
        criteria::parse_criteria_json(acceptance_criteria_raw.as_deref(), "acceptance_criteria")?;
    let required_keep_criteria = criteria::parse_criteria_json(
        required_keep_criteria_raw.as_deref(),
        "required_keep_criteria",
    )?;
    let required_keep_labels = normalize_labels(required_keep_label);
    let required_stop_labels = normalize_labels(required_stop_label);
    let companion_repos = parse_companion_repo_scopes(&workspace, companion_repo_scope)?;
    if iterations == Some(0) {
        anyhow::bail!("init preflight blocked: --iterations must be greater than zero");
    }

    // Safety screen
    verify::screen_command(&verify_cmd)?;
    if let Some(guard_cmd) = guard.as_deref() {
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
    let legacy_artifacts = legacy_run_artifacts(&workspace);
    if !legacy_artifacts.is_empty() {
        anyhow::bail!(
            "init preflight blocked: legacy autoresearch artifacts found: {}. \
             This version uses workspace-owned autoresearch-results/.",
            legacy_artifacts.join(", ")
        );
    }
    if let WorktreeStatus::Dirty(files) = git.worktree_status()? {
        anyhow::bail!(
            "init preflight blocked: unexpected worktree changes before launch: {}",
            files.join(", ")
        );
    }
    let existing_artifacts = existing_core_run_artifacts(&workspace);
    if !existing_artifacts.is_empty() {
        anyhow::bail!(
            "init preflight blocked: existing autoresearch run artifacts found: {}",
            existing_artifacts.join(", ")
        );
    }
    let head = git.head_short()?;

    // Measure baseline
    let result = verify::run_verify(&verify_cmd, fmt, key.as_deref(), &workspace)
        .context("Baseline verification failed")?;
    if fmt == VerifyFormat::MetricsJson {
        let metrics = result
            .metrics
            .as_ref()
            .context("verify_format=metrics_json requires structured baseline metrics")?;
        let primary_metric_key = key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("metric");
        ensure_metrics_json_keys(
            metrics,
            primary_metric_key,
            &acceptance_criteria,
            &required_keep_criteria,
        )?;
    }
    let baseline_guard =
        run_baseline_guard(guard.as_deref(), &workspace).context("Baseline guard failed")?;

    // Create results directory + protect from git staging
    let results_dir = ensure_results_dir_protected(&workspace)?;

    // Write TSV with header + baseline row
    let log = match environment_summary.as_deref() {
        Some(summary) if !summary.trim().is_empty() => {
            ResultsLog::create_with_metadata(&results_dir, direction, &[("environment", summary)])?
        }
        _ => ResultsLog::create(&results_dir, direction)?,
    };
    let baseline_row = ResultRow {
        iteration: 0,
        commit: Some(head.clone()),
        metric: result.metric,
        delta: Decimal::ZERO,
        guard: baseline_guard,
        status: IterationStatus::Baseline,
        description: "initial state".to_string(),
    };
    log.append(&baseline_row)?;

    // Build run config from init parameters
    let run_config = RunConfig {
        goal: goal.unwrap_or_default(),
        scope: scope.unwrap_or_default(),
        metric: metric_desc.unwrap_or_default(),
        direction,
        verify: verify_cmd,
        guard,
        iterations,
        run_tag,
        stop_condition,
        verify_format: fmt,
        primary_metric_key: key,
        acceptance_criteria,
        required_keep_criteria,
        required_keep_labels,
        required_stop_labels,
        rollback_strategy,
        run_mode,
        workspace_root,
        primary_repo,
        companion_repos,
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

fn load_project_config(workspace: &Path, config_path: Option<PathBuf>) -> Result<ProjectConfig> {
    let required = config_path.is_some();
    let path = resolve_project_config_path(workspace, config_path);

    if !path.exists() {
        if required {
            anyhow::bail!("config file not found: {}", path.display());
        }
        return Ok(ProjectConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

fn resolve_project_config_path(workspace: &Path, config_path: Option<PathBuf>) -> PathBuf {
    match config_path {
        Some(path) => resolve_workspace_path(workspace, path),
        None => workspace.join(".autoresearch.toml"),
    }
}

fn existing_core_run_artifacts(workspace: &Path) -> Vec<String> {
    [
        "results.tsv",
        "state.json",
        "context.json",
        "launch.json",
        "runtime.json",
        "runtime.log",
    ]
    .into_iter()
    .map(|name| workspace.join("autoresearch-results").join(name))
    .filter(|path| path.exists())
    .map(|path| display_workspace_path(workspace, &path))
    .collect()
}

fn legacy_run_artifacts(workspace: &Path) -> Vec<String> {
    [
        "research-results.tsv",
        "autoresearch-state.json",
        "autoresearch-launch.json",
        "autoresearch-runtime.json",
        "autoresearch-runtime.log",
    ]
    .into_iter()
    .map(|name| workspace.join(name))
    .filter(|path| path.exists())
    .map(|path| display_workspace_path(workspace, &path))
    .collect()
}

fn display_workspace_path(workspace: &Path, path: &Path) -> String {
    path.strip_prefix(workspace)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn run_baseline_guard(guard: Option<&str>, workspace: &Path) -> Result<GuardResult> {
    let Some(command) = guard.map(str::trim).filter(|command| !command.is_empty()) else {
        return Ok(GuardResult::Skip);
    };

    let result = verify::run_guard(command, workspace)?;
    if !result.passed {
        let stderr_tail = result
            .stderr
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        if stderr_tail.is_empty() {
            anyhow::bail!("baseline guard command exited non-zero");
        }
        anyhow::bail!("baseline guard command exited non-zero. stderr: {stderr_tail}");
    }

    Ok(GuardResult::Pass)
}

// ── Verify ────────────────────────────────────────────────────────────

fn cmd_verify(
    command: &str,
    format_str: &str,
    key: Option<&str>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_cwd(cwd);
    let fmt = parse_format(format_str)?;

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

fn cmd_guard_presets(format: &str, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let run_context = load_run_context(&workspace)?;
    let repo_targets = run_context
        .repo_targets
        .iter()
        .map(|target| {
            let repo = PathBuf::from(&target.path);
            serde_json::json!({
                "role": target.role,
                "path": target.path,
                "scope": target.scope,
                "presets": guard_presets_for_repo(&repo),
            })
        })
        .collect::<Vec<_>>();
    let out = serde_json::json!({
        "workspace_root": run_context.workspace_root,
        "repo_targets": repo_targets,
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&out)?),
        "text" => print!("{}", render_guard_presets_text(&out)),
        other => anyhow::bail!("Invalid guard-presets format {other:?}; use json or text"),
    }
    Ok(())
}

fn guard_presets_for_repo(repo: &Path) -> Vec<serde_json::Value> {
    let mut presets = Vec::new();
    let cwd = repo.to_string_lossy().to_string();
    let cd = shell_cd(repo);
    if repo.join("Cargo.toml").is_file() {
        presets.push(guard_preset(
            "rust_tests",
            &cwd,
            &format!("{cd} cargo test"),
            "Cargo.toml detected",
        ));
        presets.push(guard_preset(
            "rust_format",
            &cwd,
            &format!("{cd} cargo fmt -- --check"),
            "Cargo.toml detected",
        ));
    }
    if repo.join("package.json").is_file() {
        presets.push(guard_preset(
            "node_tests",
            &cwd,
            &format!("{cd} npm test -- --runInBand"),
            "package.json detected",
        ));
        presets.push(guard_preset(
            "node_lint",
            &cwd,
            &format!("{cd} npm run lint --if-present"),
            "package.json detected",
        ));
    }
    if repo.join("pyproject.toml").is_file() || repo.join("setup.py").is_file() {
        presets.push(guard_preset(
            "python_tests",
            &cwd,
            &format!("{cd} pytest"),
            "Python project metadata detected",
        ));
    }
    if repo.join("go.mod").is_file() {
        presets.push(guard_preset(
            "go_tests",
            &cwd,
            &format!("{cd} go test ./..."),
            "go.mod detected",
        ));
    }
    if repo.join("Makefile").is_file() {
        presets.push(guard_preset(
            "make_test",
            &cwd,
            &format!("{cd} make test"),
            "Makefile detected",
        ));
    }
    presets
}

fn guard_preset(name: &str, cwd: &str, command: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "cwd": cwd,
        "command": command,
        "reason": reason,
    })
}

fn shell_cd(path: &Path) -> String {
    format!("cd {} &&", shell_quote(&path.to_string_lossy()))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn render_guard_presets_text(value: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(targets) = value["repo_targets"].as_array() {
        for target in targets {
            let role = target["role"].as_str().unwrap_or("repo");
            let path = target["path"].as_str().unwrap_or("");
            writeln!(out, "{role}: {path}").unwrap();
            if let Some(presets) = target["presets"].as_array() {
                if presets.is_empty() {
                    writeln!(out, "  - no presets detected").unwrap();
                }
                for preset in presets {
                    writeln!(
                        out,
                        "  - {}: {}",
                        preset["name"].as_str().unwrap_or("preset"),
                        preset["command"].as_str().unwrap_or("")
                    )
                    .unwrap();
                }
            }
        }
    }
    out
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

    let guard = parse_guard_result(Some(guard_str))?;

    let status = parse_status(status_str)?;
    if status == IterationStatus::Baseline {
        anyhow::bail!("baseline log rows are created by init");
    }
    if matches!(
        status,
        IterationStatus::Keep | IterationStatus::KeepReworked
    ) && commit_val.is_none()
    {
        anyhow::bail!("keep log rows require a commit");
    }

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
        let mut escalation_update = None;

        match status {
            IterationStatus::Keep | IterationStatus::KeepReworked => {
                state.record_keep(metric, commit.to_string());
                state.last_status = status;
            }
            IterationStatus::Discard => {
                state.record_discard(metric, commit_val);
            }
            IterationStatus::Crash
            | IterationStatus::HookBlocked
            | IterationStatus::MetricError => {
                state.record_crash();
                state.last_trial_metric = Some(metric);
                state.last_status = status;
            }
            IterationStatus::NoOp => {
                state.record_no_op();
            }
            IterationStatus::Blocked => {
                state.record_blocked(description.to_string());
            }
            IterationStatus::Drift => {
                state.record_drift(metric);
            }
            IterationStatus::Pivot | IterationStatus::Refine | IterationStatus::Search => {
                state.record_meta_status(status, metric);
                if status == IterationStatus::Pivot {
                    let esc_path = results_dir.join("escalation.json");
                    let mut escalation: EscalationState = if esc_path.exists() {
                        serde_json::from_str(&std::fs::read_to_string(&esc_path)?)?
                    } else {
                        EscalationState::default()
                    };
                    escalation.acknowledge_pivot();
                    state.pivot_count = escalation.pivot_count;
                    state.consecutive_discards = escalation.consecutive_discards;
                    escalation_update = Some((esc_path, escalation));
                }
            }
            _ => {}
        }

        std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
        if let Some((esc_path, escalation)) = escalation_update {
            std::fs::write(&esc_path, serde_json::to_string_pretty(&escalation)?)?;
        }
    }

    println!(r#"{{"status":"ok","iteration":{iteration}}}"#);
    Ok(())
}

// ── Decide ────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
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
    let guard = parse_guard_result(Some(guard_str))?;

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
    let decision = if decision == "keep"
        && (guard == GuardResult::Fail
            || !required_keep.satisfied
            || !required_keep_labels_satisfied)
    {
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
            let retained_commit = resolved_commit
                .clone()
                .context("keep decision requires a retained commit")?;
            state.record_keep_with_metrics_and_labels(
                metric,
                retained_commit,
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

    let auto_search = maybe_run_auto_web_search(&workspace, escalation_action, state.iteration);

    // Build response
    let escalation_guidance = escalation_action.map(|a| {
        serde_json::json!({
            "action": format!("{:?}", a),
            "guidance": a.guidance(),
            "is_terminal": a.is_terminal(),
        })
    });

    let mut out = serde_json::json!({
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
    if let Some(auto_search) = auto_search {
        if let Some(object) = out.as_object_mut() {
            object.insert("auto_search".to_string(), auto_search);
        }
    }
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
    let direction = evals_direction(&content)?;
    let unknown_columns = evals_unknown_columns(&content);

    let has_data_rows = content
        .lines()
        .any(|l| !l.starts_with('#') && !l.starts_with("iteration\t") && !l.is_empty());
    if !has_data_rows {
        anyhow::bail!("No data rows in results TSV.");
    }

    let metrics = parse_results_tsv(&content)?;

    if metrics.is_empty() {
        anyhow::bail!("No main data rows in results TSV.");
    }

    let total = metrics.len();
    let has_baseline = metrics
        .first()
        .is_some_and(|row| row.iteration == 0 || row.status == "baseline");
    let total_iterations = total.saturating_sub(usize::from(has_baseline));
    let keeps = metrics
        .iter()
        .filter(|row| is_keep_status(&row.status))
        .count();
    let reworked_keeps = metrics
        .iter()
        .filter(|row| row.status == "keep (reworked)")
        .count();
    let discards = metrics.iter().filter(|row| row.status == "discard").count();
    let crashes = metrics
        .iter()
        .filter(|row| is_failure_status(&row.status))
        .count();
    let guard_failures = metrics
        .iter()
        .filter(|row| row.guard.as_deref() == Some("fail"))
        .count();
    let guard_failed_improvements = metrics
        .iter()
        .filter(|row| {
            row.guard.as_deref() == Some("fail")
                && match direction {
                    "lower" => row.delta < Decimal::ZERO,
                    _ => row.delta > Decimal::ZERO,
                }
        })
        .count();
    let mut longest_keep_streak = 0u32;
    let mut current_keep_streak = 0u32;
    let mut longest_failure_streak = 0u32;
    let mut current_failure_streak = 0u32;
    for row in &metrics {
        if is_keep_status(&row.status) {
            current_keep_streak += 1;
            longest_keep_streak = longest_keep_streak.max(current_keep_streak);
        } else if row.status != "baseline" {
            current_keep_streak = 0;
        }

        if row.status == "discard" || is_failure_status(&row.status) {
            current_failure_streak += 1;
            longest_failure_streak = longest_failure_streak.max(current_failure_streak);
        } else if row.status != "baseline" {
            current_failure_streak = 0;
        }
    }
    let baseline = metrics.first().map(|row| row.metric).unwrap_or_default();
    let final_metric = metrics.last().map(|row| row.metric).unwrap_or_default();
    let improvement = if direction == "higher" {
        final_metric - baseline
    } else {
        baseline - final_metric
    };
    let improvement_pct = (baseline != Decimal::ZERO).then(|| {
        ((improvement / baseline.abs()) * Decimal::from(100))
            .round_dp(2)
            .to_string()
    });
    let best = if direction == "higher" {
        metrics
            .iter()
            .map(|row| row.metric)
            .max()
            .unwrap_or_default()
    } else {
        metrics
            .iter()
            .map(|row| row.metric)
            .min()
            .unwrap_or_default()
    };

    // Find longest plateau (consecutive non-keep)
    let mut longest_plateau = 0u32;
    let mut current_plateau = 0u32;
    for row in &metrics {
        if !is_keep_status(&row.status) && row.status != "baseline" {
            current_plateau += 1;
            longest_plateau = longest_plateau.max(current_plateau);
        } else {
            current_plateau = 0;
        }
    }

    // Top improvements (keeps sorted by absolute delta)
    let mut top_keeps: Vec<(Decimal, &str)> = metrics
        .iter()
        .filter(|row| is_keep_status(&row.status))
        .map(|row| (row.delta.abs(), row.description.as_str()))
        .collect();
    top_keeps.sort_by_key(|entry| std::cmp::Reverse(entry.0));

    let mut top_regressions: Vec<(Decimal, &str)> = metrics
        .iter()
        .filter_map(|row| {
            let regression = match direction {
                "lower" if row.delta > Decimal::ZERO => row.delta,
                "higher" if row.delta < Decimal::ZERO => row.delta.abs(),
                _ => return None,
            };
            Some((regression, row.description.as_str()))
        })
        .collect();
    top_regressions.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    let parallel_workers = analyze_parallel_worker_rows(&content, direction)?;

    let efficiency = if total_iterations > 0 {
        (keeps as f64 / total_iterations as f64 * 100.0).round() as u32
    } else {
        0
    };
    let rework_rate = if total_iterations > 0 {
        (reworked_keeps as f64 / total_iterations as f64 * 100.0).round() as u32
    } else {
        0
    };

    // Determine trend from last 5 keeps
    let recent_keeps: Vec<Decimal> = metrics
        .iter()
        .filter(|row| is_keep_status(&row.status))
        .rev()
        .take(5)
        .map(|row| row.metric)
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
    let recommendation = evals_recommendation(
        longest_plateau,
        crashes,
        keeps,
        efficiency,
        total_iterations,
        trend,
    );
    let summary_dir = tsv_path.parent().unwrap_or_else(|| Path::new("."));

    match format {
        "json" => {
            let out = serde_json::json!({
                "direction": direction,
                "total_iterations": total_iterations,
                "keeps": keeps,
                "reworked_keeps": reworked_keeps,
                "rework_rate_pct": rework_rate,
                "discards": discards,
                "crashes": crashes,
                "guard_failures": guard_failures,
                "guard_failed_improvements": guard_failed_improvements,
                "longest_keep_streak": longest_keep_streak,
                "longest_failure_streak": longest_failure_streak,
                "baseline": baseline.to_string(),
                "final": final_metric.to_string(),
                "best": best.to_string(),
                "improvement": improvement.to_string(),
                "improvement_pct": improvement_pct.as_deref(),
                "efficiency_pct": efficiency,
                "longest_plateau": longest_plateau,
                "trend": trend,
                "recommendation": recommendation,
                "unknown_columns": &unknown_columns,
                "parallel_workers": &parallel_workers,
                "top_improvements": top_keeps.iter().take(5).map(|(d, desc)| {
                    serde_json::json!({"delta": d.to_string(), "description": desc})
                }).collect::<Vec<_>>(),
                "top_regressions": top_regressions.iter().take(5).map(|(d, desc)| {
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
                reworked_keeps,
                rework_rate,
                discards,
                crashes,
                guard_failures,
                guard_failed_improvements,
                longest_keep_streak,
                longest_failure_streak,
                efficiency,
                baseline,
                final_metric,
                best,
                improvement,
                improvement_pct: improvement_pct.as_deref(),
                trend,
                longest_plateau,
                unknown_columns: &unknown_columns,
                parallel_workers: &parallel_workers,
                top_keeps: &top_keeps,
                top_regressions: &top_regressions,
            });
            std::fs::write(summary_dir.join("evals-summary.md"), &report)?;
            print!("{report}");
        }
        "text" => {
            let report = render_evals_markdown(EvalsReport {
                direction,
                total_iterations,
                keeps,
                reworked_keeps,
                rework_rate,
                discards,
                crashes,
                guard_failures,
                guard_failed_improvements,
                longest_keep_streak,
                longest_failure_streak,
                efficiency,
                baseline,
                final_metric,
                best,
                improvement,
                improvement_pct: improvement_pct.as_deref(),
                trend,
                longest_plateau,
                unknown_columns: &unknown_columns,
                parallel_workers: &parallel_workers,
                top_keeps: &top_keeps,
                top_regressions: &top_regressions,
            });
            print!("{report}");
        }
        other => anyhow::bail!("Invalid evals format {other:?}; use text, json, or md"),
    }

    Ok(())
}

fn parse_evals_direction(value: Option<&str>) -> Result<&'static str> {
    match value {
        Some("higher" | "higher_is_better") => Ok("higher"),
        Some("lower" | "lower_is_better") => Ok("lower"),
        Some(other) => anyhow::bail!("Invalid metric_direction: {other}"),
        None => anyhow::bail!("results TSV is missing # metric_direction header"),
    }
}

fn evals_direction(content: &str) -> Result<&'static str> {
    let explicit = content
        .lines()
        .find(|line| line.starts_with("# metric_direction:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|value| value.trim());
    if explicit.is_some() {
        return parse_evals_direction(explicit);
    }

    infer_evals_direction_from_header(content)
        .context("results TSV is missing # metric_direction header")
}

fn infer_evals_direction_from_header(content: &str) -> Option<&'static str> {
    let header = content.lines().find(|line| {
        !line.starts_with('#') && !line.trim().is_empty() && line.starts_with("iteration\t")
    })?;
    let columns: BTreeSet<&str> = header.split('\t').collect();
    if columns.contains("error_count") {
        Some("lower")
    } else if columns.contains("metric") || columns.contains("metric_value") {
        Some("higher")
    } else {
        None
    }
}

fn evals_unknown_columns(content: &str) -> Vec<String> {
    let Some(header) = content.lines().find(|line| {
        !line.starts_with('#') && !line.trim().is_empty() && line.starts_with("iteration\t")
    }) else {
        return Vec::new();
    };
    let known = BTreeSet::from([
        "iteration",
        "timestamp",
        "commit",
        "metric",
        "metric_value",
        "error_count",
        "delta",
        "guard",
        "guard-metric",
        "status",
        "description",
        "severity",
        "hypothesis",
        "owasp",
        "stride",
        "technique",
        "dimension",
        "candidate_label",
        "judge_verdict",
        "error_type",
        "classification",
        "convergence_count",
        "finding",
        "evidence",
        "file_line",
    ]);
    header
        .split('\t')
        .filter(|column| !known.contains(*column))
        .map(str::to_string)
        .collect()
}

struct EvalsReport<'a> {
    direction: &'a str,
    total_iterations: usize,
    keeps: usize,
    reworked_keeps: usize,
    rework_rate: u32,
    discards: usize,
    crashes: usize,
    guard_failures: usize,
    guard_failed_improvements: usize,
    longest_keep_streak: u32,
    longest_failure_streak: u32,
    efficiency: u32,
    baseline: Decimal,
    final_metric: Decimal,
    best: Decimal,
    improvement: Decimal,
    improvement_pct: Option<&'a str>,
    trend: &'a str,
    longest_plateau: u32,
    unknown_columns: &'a [String],
    top_keeps: &'a [(Decimal, &'a str)],
    top_regressions: &'a [(Decimal, &'a str)],
    parallel_workers: &'a ParallelWorkerStats,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ParallelWorkerStats {
    total: usize,
    batches: usize,
    improved: usize,
    regressed: usize,
    flat: usize,
    improvement_rate_pct: u32,
    sign_test: Option<ParallelSignTest>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ParallelSignTest {
    n: usize,
    improvements: usize,
    p_value: String,
    conclusion: &'static str,
}

fn analyze_parallel_worker_rows(content: &str, direction: &str) -> Result<ParallelWorkerStats> {
    let mut header: Option<BTreeMap<String, usize>> = None;
    let mut batches = BTreeSet::new();
    let mut improved = 0usize;
    let mut regressed = 0usize;
    let mut flat = 0usize;
    let mut total = 0usize;

    for line in content.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        if columns.first() == Some(&"iteration") {
            header = Some(
                columns
                    .iter()
                    .enumerate()
                    .map(|(index, column)| ((*column).to_string(), index))
                    .collect(),
            );
            continue;
        }
        let iteration_index = header
            .as_ref()
            .and_then(|header| header.get("iteration").copied())
            .unwrap_or(0);
        let delta_index = header
            .as_ref()
            .and_then(|header| header.get("delta").copied())
            .unwrap_or(3);
        let Some(iteration_label) = columns.get(iteration_index) else {
            continue;
        };
        let Some(batch_iteration) = worker_iteration_prefix(iteration_label) else {
            continue;
        };
        let Some(delta_raw) = columns.get(delta_index) else {
            continue;
        };

        let delta = Decimal::from_str(delta_raw.trim_start_matches('+')).with_context(|| {
            format!("Invalid parallel worker delta at iteration {iteration_label}")
        })?;
        total += 1;
        batches.insert(batch_iteration);
        match direction {
            "lower" if delta < Decimal::ZERO => improved += 1,
            "lower" if delta > Decimal::ZERO => regressed += 1,
            "higher" if delta > Decimal::ZERO => improved += 1,
            "higher" if delta < Decimal::ZERO => regressed += 1,
            _ => flat += 1,
        }
    }

    let improvement_rate_pct = if total > 0 {
        (improved as f64 / total as f64 * 100.0).round() as u32
    } else {
        0
    };
    let sign_test_n = improved + regressed;
    let sign_test = (sign_test_n >= 3).then(|| {
        let p_value = binomial_upper_tail(improved, sign_test_n);
        ParallelSignTest {
            n: sign_test_n,
            improvements: improved,
            p_value: format!("{p_value:.6}"),
            conclusion: sign_test_conclusion(p_value, improved, regressed),
        }
    });

    Ok(ParallelWorkerStats {
        total,
        batches: batches.len(),
        improved,
        regressed,
        flat,
        improvement_rate_pct,
        sign_test,
    })
}

fn binomial_upper_tail(successes: usize, n: usize) -> f64 {
    if n == 0 || successes > n {
        return 1.0;
    }
    let favorable = (successes..=n)
        .map(|k| binomial_coefficient(n, k))
        .sum::<f64>();
    favorable / 2_f64.powi(n as i32)
}

fn binomial_coefficient(n: usize, k: usize) -> f64 {
    let k = k.min(n - k);
    if k == 0 {
        return 1.0;
    }
    (1..=k).fold(1.0, |acc, i| acc * (n - k + i) as f64 / i as f64)
}

fn sign_test_conclusion(p_value: f64, improved: usize, regressed: usize) -> &'static str {
    if improved <= regressed {
        "no_positive_signal"
    } else if p_value <= 0.05 {
        "significant_positive_signal"
    } else if p_value <= 0.10 {
        "suggestive_positive_signal"
    } else {
        "insufficient_evidence"
    }
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
    writeln!(out, "| Reworked keeps | {} |", report.reworked_keeps).unwrap();
    writeln!(out, "| Rework rate | {}% |", report.rework_rate).unwrap();
    writeln!(out, "| Discarded | {} |", report.discards).unwrap();
    writeln!(out, "| Crashes | {} |", report.crashes).unwrap();
    writeln!(out, "| Guard failures | {} |", report.guard_failures).unwrap();
    writeln!(
        out,
        "| Improved but guard failed | {} |",
        report.guard_failed_improvements
    )
    .unwrap();
    writeln!(
        out,
        "| Longest keep streak | {} |",
        report.longest_keep_streak
    )
    .unwrap();
    writeln!(
        out,
        "| Longest failure streak | {} |",
        report.longest_failure_streak
    )
    .unwrap();
    writeln!(out, "| Efficiency | {}% |", report.efficiency).unwrap();
    writeln!(out, "| Baseline | {} |", report.baseline).unwrap();
    writeln!(out, "| Final | {} |", report.final_metric).unwrap();
    writeln!(out, "| Best | {} |", report.best).unwrap();
    writeln!(
        out,
        "| Improvement | {} ({}) |",
        report.improvement,
        format_optional_percent(report.improvement_pct)
    )
    .unwrap();
    writeln!(out, "| Trend | {} |", report.trend).unwrap();
    writeln!(
        out,
        "| Longest plateau | {} iterations |",
        report.longest_plateau
    )
    .unwrap();
    if !report.unknown_columns.is_empty() {
        writeln!(
            out,
            "| Unknown columns | {} |",
            report.unknown_columns.join(", ")
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    if report.parallel_workers.total > 0 {
        writeln!(out, "### Parallel Worker Significance").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "- Batches: {}", report.parallel_workers.batches).unwrap();
        writeln!(
            out,
            "- Workers: {} improved, {} regressed, {} flat ({}% improved)",
            report.parallel_workers.improved,
            report.parallel_workers.regressed,
            report.parallel_workers.flat,
            report.parallel_workers.improvement_rate_pct
        )
        .unwrap();
        if let Some(sign_test) = &report.parallel_workers.sign_test {
            writeln!(
                out,
                "- Sign test: n={}, improvements={}, p={}, {}",
                sign_test.n, sign_test.improvements, sign_test.p_value, sign_test.conclusion
            )
            .unwrap();
        } else {
            writeln!(out, "- Sign test: insufficient non-flat worker results").unwrap();
        }
        writeln!(out).unwrap();
    }

    if !report.top_keeps.is_empty() {
        writeln!(out, "### Top Improvements").unwrap();
        writeln!(out).unwrap();
        for (i, (delta, desc)) in report.top_keeps.iter().take(5).enumerate() {
            writeln!(out, "{}. **{}** - {}", i + 1, delta, desc).unwrap();
        }
        writeln!(out).unwrap();
    }

    if !report.top_regressions.is_empty() {
        writeln!(out, "### Top Regressions").unwrap();
        writeln!(out).unwrap();
        for (i, (delta, desc)) in report.top_regressions.iter().take(5).enumerate() {
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

fn format_optional_percent(value: Option<&str>) -> String {
    value
        .map(|pct| format!("{pct}%"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn evals_recommendation(
    longest_plateau: u32,
    crashes: usize,
    keeps: usize,
    efficiency: u32,
    total_iterations: usize,
    trend: &str,
) -> &'static str {
    if longest_plateau >= 5 || trend == "declining" || (efficiency < 20 && total_iterations > 10) {
        "change_strategy"
    } else if crashes > keeps {
        "check_verify"
    } else {
        "continue"
    }
}

// ── Status ────────────────────────────────────────────────────────────

fn cmd_status(cwd: Option<PathBuf>, summary: bool) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");

    if !state_path.exists() {
        println!(r#"{{"active":false,"message":"No active autoresearch run."}}"#);
        return Ok(());
    }

    let state_content = std::fs::read_to_string(&state_path)?;
    let state: RunState = serde_json::from_str(&state_content)?;
    if summary {
        let out = serde_json::json!({
            "active": true,
            "iteration": state.iteration,
            "current_metric": state.current_metric.to_string(),
            "best_metric": state.best_metric.to_string(),
            "best_iteration": state.best_iteration,
            "keeps": state.keeps,
            "discards": state.discards,
            "crashes": state.crashes,
            "last_status": state.last_status.as_str(),
            "consecutive_discards": state.consecutive_discards,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

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

// ── MCP ───────────────────────────────────────────────────────────────

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

fn cmd_mcp_serve(cwd: Option<PathBuf>) -> Result<()> {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.context("failed to read MCP stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(request) => handle_mcp_request(&request, cwd.clone()),
            Err(err) => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": {
                    "code": -32700,
                    "message": format!("Parse error: {err}"),
                },
            })),
        };
        if let Some(response) = response {
            write_stdout_line(&serde_json::to_string(&response)?)?;
        }
    }
    Ok(())
}

fn cmd_mcp_call(
    server_command: &str,
    tool: &str,
    arguments: &str,
    cwd: Option<PathBuf>,
) -> Result<()> {
    verify::screen_command(server_command).context("unsafe MCP server command")?;
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).context("failed to parse MCP tool arguments JSON")?;
    if !arguments.is_object() {
        anyhow::bail!("MCP tool arguments must be a JSON object");
    }

    let workspace = resolve_cwd(cwd);
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(server_command)
        .current_dir(&workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start MCP server command: {server_command}"))?;

    let input = [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "autoresearch",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            },
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool,
                "arguments": arguments,
            },
        }),
    ]
    .into_iter()
    .map(|message| message.to_string())
    .collect::<Vec<_>>()
    .join("\n");

    {
        let mut stdin = child
            .stdin
            .take()
            .context("failed to open MCP server stdin")?;
        stdin
            .write_all(format!("{input}\n").as_bytes())
            .context("failed to write MCP request")?;
    }

    let output = child
        .wait_with_output()
        .context("failed to wait for MCP server command")?;
    if !output.status.success() {
        anyhow::bail!(
            "MCP server command exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let response = mcp_find_response(&output.stdout, 2)?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn mcp_find_response(stdout: &[u8], id: i64) -> Result<serde_json::Value> {
    let stdout = std::str::from_utf8(stdout).context("MCP server stdout was not UTF-8")?;
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).with_context(|| format!("invalid MCP response: {line}"))?;
        if value.get("id").and_then(|value| value.as_i64()) == Some(id) {
            return Ok(value);
        }
    }
    anyhow::bail!("MCP server did not return a response for id {id}")
}

fn handle_mcp_request(
    request: &serde_json::Value,
    default_cwd: Option<PathBuf>,
) -> Option<serde_json::Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(|value| value.as_str());

    if id.is_none() {
        return None;
    }

    let id = id.unwrap_or(serde_json::Value::Null);
    match method {
        Some("initialize") => Some(mcp_response(
            id,
            serde_json::json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {
                        "listChanged": false,
                    },
                },
                "serverInfo": {
                    "name": "autoresearch",
                    "title": "Autoresearch",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Read-only Autoresearch run inspection tools",
                },
                "instructions": "Use tools/list to discover read-only Autoresearch inspection tools.",
            }),
        )),
        Some("ping") => Some(mcp_response(id, serde_json::json!({}))),
        Some("tools/list") => Some(mcp_response(
            id,
            serde_json::json!({
                "tools": mcp_tool_definitions(),
            }),
        )),
        Some("tools/call") => Some(handle_mcp_tool_call(id, request, default_cwd)),
        Some(other) => Some(mcp_error(id, -32601, format!("Method not found: {other}"))),
        None => Some(mcp_error(id, -32600, "Invalid Request: missing method")),
    }
}

fn mcp_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "autoresearch_status",
            "title": "Autoresearch Status",
            "description": "Return the active Autoresearch run status for a workspace.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cwd": {
                        "type": "string",
                        "description": "Workspace or repo subdirectory to inspect"
                    }
                },
                "additionalProperties": false
            }
        }),
        serde_json::json!({
            "name": "autoresearch_watch_snapshot",
            "title": "Autoresearch Watch Snapshot",
            "description": "Return the current results.tsv snapshot using the same payload as watch --websocket --once.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cwd": {
                        "type": "string",
                        "description": "Workspace or repo subdirectory to inspect"
                    },
                    "lines": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 20,
                        "description": "Number of recent data rows to include"
                    }
                },
                "additionalProperties": false
            }
        }),
    ]
}

fn handle_mcp_tool_call(
    id: serde_json::Value,
    request: &serde_json::Value,
    default_cwd: Option<PathBuf>,
) -> serde_json::Value {
    let params = request
        .get("params")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();
    let Some(name) = params.get("name").and_then(|value| value.as_str()) else {
        return mcp_error(id, -32602, "tools/call params.name must be a string");
    };
    let arguments = params
        .get("arguments")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    let result = match name {
        "autoresearch_status" => mcp_status_payload(mcp_argument_cwd(&arguments, default_cwd)),
        "autoresearch_watch_snapshot" => {
            let lines = arguments
                .get("lines")
                .and_then(|value| value.as_u64())
                .unwrap_or(20) as usize;
            mcp_watch_snapshot_payload(mcp_argument_cwd(&arguments, default_cwd), lines)
        }
        other => return mcp_error(id, -32602, format!("Unknown tool: {other}")),
    };

    match result {
        Ok(payload) => mcp_response(id, mcp_tool_result(payload, false)),
        Err(err) => mcp_response(
            id,
            mcp_tool_result(
                serde_json::json!({
                    "error": err.to_string(),
                }),
                true,
            ),
        ),
    }
}

fn mcp_argument_cwd(
    arguments: &serde_json::Map<String, serde_json::Value>,
    default_cwd: Option<PathBuf>,
) -> Option<PathBuf> {
    arguments
        .get("cwd")
        .and_then(|value| value.as_str())
        .map(PathBuf::from)
        .or(default_cwd)
}

fn mcp_status_payload(cwd: Option<PathBuf>) -> Result<serde_json::Value> {
    let workspace = resolve_results_workspace(cwd);
    let state_path = workspace.join("autoresearch-results/state.json");
    if !state_path.exists() {
        return Ok(serde_json::json!({
            "active": false,
            "message": "No active autoresearch run.",
            "workspace": workspace.display().to_string(),
        }));
    }

    let state: RunState = serde_json::from_str(
        &std::fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read {}", state_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", state_path.display()))?;
    Ok(serde_json::json!({
        "active": true,
        "workspace": workspace.display().to_string(),
        "iteration": state.iteration,
        "current_metric": state.current_metric.to_string(),
        "best_metric": state.best_metric.to_string(),
        "best_iteration": state.best_iteration,
        "keeps": state.keeps,
        "discards": state.discards,
        "crashes": state.crashes,
        "last_status": state.last_status.as_str(),
        "consecutive_discards": state.consecutive_discards,
    }))
}

fn mcp_watch_snapshot_payload(cwd: Option<PathBuf>, lines: usize) -> Result<serde_json::Value> {
    let cwd = resolve_cwd(cwd);
    let tsv_path = default_results_tsv(&cwd)
        .context("No results.tsv found. Provide cwd inside a run workspace.")?;
    let (payloads, _) = watch_websocket_payloads(&tsv_path, lines, 0)?;
    Ok(serde_json::json!({
        "results_tsv": tsv_path.display().to_string(),
        "payloads": payloads,
    }))
}

fn mcp_tool_result(payload: serde_json::Value, is_error: bool) -> serde_json::Value {
    serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string()),
            }
        ],
        "structuredContent": payload,
        "isError": is_error,
    })
}

fn mcp_response(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn mcp_error(id: serde_json::Value, code: i32, message: impl Into<String>) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        },
    })
}

// ── Health ────────────────────────────────────────────────────────────

fn cmd_health(
    verify: Option<&str>,
    strict: bool,
    min_free_mb: u64,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let report = health::run_health_check(&workspace, verify, min_free_mb)?;
    let should_fail = report.has_blockers() || (strict && !report.warnings.is_empty());
    println!("{}", serde_json::to_string_pretty(&report)?);
    if should_fail {
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

#[derive(Debug, Clone, Copy)]
enum ParallelMergeStrategy {
    CherryPick,
    FastForward,
    Squash,
    Rebase,
}

impl ParallelMergeStrategy {
    fn as_str(self) -> &'static str {
        match self {
            Self::CherryPick => "cherry-pick",
            Self::FastForward => "fast-forward",
            Self::Squash => "squash",
            Self::Rebase => "rebase",
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ParallelPrepareManifest {
    #[serde(default)]
    workers: Vec<ParallelPreparedWorker>,
}

#[derive(Debug, serde::Deserialize)]
struct ParallelPreparedWorker {
    worker_id: String,
    branch: String,
    worktree: String,
    #[serde(default)]
    prompt_file: Option<String>,
}

struct ParallelRunningWorker {
    worker_id: String,
    child: Child,
    log_file: PathBuf,
    started_at: String,
}

fn default_completed_status() -> String {
    "completed".to_string()
}

fn cmd_parallel(command: ParallelCommands) -> Result<()> {
    match command {
        ParallelCommands::Prepare {
            workers,
            worktree_root,
            manifest,
            batch_file,
            branch_prefix,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let out = cmd_parallel_prepare(
                &workspace,
                workers,
                worktree_root,
                manifest,
                batch_file,
                &branch_prefix,
            )?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        ParallelCommands::Cleanup {
            manifest,
            keep_branches,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let out = cmd_parallel_cleanup(&workspace, manifest, keep_branches)?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        ParallelCommands::Run {
            manifest,
            execution_policy,
            codex_bin,
            timeout_seconds,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let out = cmd_parallel_run(
                &workspace,
                manifest,
                &execution_policy,
                &codex_bin,
                timeout_seconds,
            )?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        ParallelCommands::Template {
            workers,
            output,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let template = parallel_batch_template(workers);
            let content = serde_json::to_string_pretty(&template)?;
            if let Some(output) = output {
                let output_path = if output.is_absolute() {
                    output
                } else {
                    workspace.join(output)
                };
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&output_path, format!("{content}\n"))?;
                let out = serde_json::json!({
                    "status": "ok",
                    "workers": workers,
                    "path": output_path.display().to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("{content}");
            }
        }
        ParallelCommands::Closeout {
            batch_file,
            merge_strategy,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let merge_strategy = parse_parallel_merge_strategy(&merge_strategy)?;
            let out = cmd_parallel_closeout(&workspace, &batch_file, merge_strategy)?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
    }
    Ok(())
}

fn cmd_parallel_prepare(
    workspace: &Path,
    workers: u8,
    worktree_root: Option<PathBuf>,
    manifest: Option<PathBuf>,
    batch_file: Option<PathBuf>,
    branch_prefix: &str,
) -> Result<serde_json::Value> {
    let health = health::run_health_check(workspace, None, 500)?;
    if health.has_blockers() {
        let codes = health
            .blockers
            .iter()
            .map(|finding| finding.code)
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("parallel prepare preflight blocked: {codes}");
    }
    let git = GitRepo::open(workspace)?;
    if let WorktreeStatus::Dirty(files) = git.worktree_status()? {
        anyhow::bail!(
            "parallel prepare preflight blocked: unexpected worktree changes before parallel prepare: {}",
            files.join(", ")
        );
    }

    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    let state: RunState = serde_json::from_str(
        &std::fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read {}", state_path.display()))?,
    )?;
    let next_iteration = state.iteration + 1;
    let base_commit = git.head_full()?;
    let worktree_root = resolve_workspace_path(
        workspace,
        worktree_root.unwrap_or_else(|| {
            PathBuf::from(format!(
                "autoresearch-results/parallel-worktrees/iteration-{next_iteration}"
            ))
        }),
    );
    let manifest_path = resolve_workspace_path(
        workspace,
        manifest.unwrap_or_else(|| PathBuf::from("autoresearch-results/parallel-manifest.json")),
    );
    let batch_path = resolve_workspace_path(
        workspace,
        batch_file.unwrap_or_else(|| PathBuf::from("autoresearch-results/parallel-workers.json")),
    );

    std::fs::create_dir_all(&worktree_root)
        .with_context(|| format!("failed to create {}", worktree_root.display()))?;

    for index in 0..workers {
        let worker_id = ((b'a' + index) as char).to_string();
        let branch = format!("{branch_prefix}-{next_iteration}-{worker_id}");
        let worktree = worktree_root.join(format!("worker-{worker_id}"));
        if worktree.exists() {
            anyhow::bail!(
                "parallel worker worktree already exists: {}",
                worktree.display()
            );
        }
        if git_branch_exists(workspace, &branch)? {
            anyhow::bail!("parallel worker branch already exists: {branch}");
        }
    }

    let mut worker_entries = Vec::new();
    for index in 0..workers {
        let worker_id = ((b'a' + index) as char).to_string();
        let branch = format!("{branch_prefix}-{next_iteration}-{worker_id}");
        let worktree = worktree_root.join(format!("worker-{worker_id}"));
        run_git_command(
            workspace,
            &[
                "worktree".to_string(),
                "add".to_string(),
                "-b".to_string(),
                branch.clone(),
                worktree.display().to_string(),
                base_commit.clone(),
            ],
        )
        .with_context(|| format!("failed to create worker-{worker_id} worktree"))?;
        copy_pointer_to_worker(workspace, &worktree)?;
        let prompt_file = write_parallel_worker_prompt(&worktree, &state, &worker_id, &branch)?;
        worker_entries.push(serde_json::json!({
            "worker_id": worker_id,
            "branch": branch,
            "worktree": worktree.display().to_string(),
            "prompt_file": prompt_file.display().to_string(),
            "status": "prepared",
        }));
    }

    let manifest_json = serde_json::json!({
        "version": 1,
        "status": "prepared",
        "iteration": next_iteration,
        "base_commit": base_commit,
        "workspace": workspace.display().to_string(),
        "worktree_root": worktree_root.display().to_string(),
        "batch_file": batch_path.display().to_string(),
        "workers": worker_entries,
    });
    write_json_file(&manifest_path, &manifest_json)?;
    write_json_file(&batch_path, &parallel_batch_template(workers))?;

    Ok(serde_json::json!({
        "status": "ok",
        "iteration": next_iteration,
        "base_commit": manifest_json["base_commit"],
        "manifest": manifest_path.display().to_string(),
        "batch_file": batch_path.display().to_string(),
        "workers": manifest_json["workers"],
    }))
}

fn cmd_parallel_cleanup(
    workspace: &Path,
    manifest: PathBuf,
    keep_branches: bool,
) -> Result<serde_json::Value> {
    let manifest_path = resolve_workspace_path(workspace, manifest);
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest_json: serde_json::Value = serde_json::from_str(&manifest_text)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    let manifest: ParallelPrepareManifest = serde_json::from_value(manifest_json.clone())
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    if manifest.workers.is_empty() {
        anyhow::bail!("parallel cleanup manifest must contain at least one worker");
    }

    let mut cleaned = Vec::new();
    for worker in manifest.workers {
        let worktree = PathBuf::from(&worker.worktree);
        let mut removed_worktree = false;
        let mut removed_branch = false;
        if worktree.exists() {
            run_git_command(
                workspace,
                &[
                    "worktree".to_string(),
                    "remove".to_string(),
                    "--force".to_string(),
                    worktree.display().to_string(),
                ],
            )
            .with_context(|| format!("failed to remove worker-{} worktree", worker.worker_id))?;
            removed_worktree = true;
        }
        if !keep_branches && git_branch_exists(workspace, &worker.branch)? {
            run_git_command(
                workspace,
                &[
                    "branch".to_string(),
                    "-D".to_string(),
                    worker.branch.clone(),
                ],
            )
            .with_context(|| format!("failed to delete worker-{} branch", worker.worker_id))?;
            removed_branch = true;
        }
        cleaned.push(serde_json::json!({
            "worker_id": worker.worker_id,
            "worktree": worktree.display().to_string(),
            "branch": worker.branch,
            "removed_worktree": removed_worktree,
            "removed_branch": removed_branch,
        }));
    }

    let mut updated_manifest = manifest_json;
    if let Some(object) = updated_manifest.as_object_mut() {
        object.insert("status".to_string(), serde_json::json!("cleaned"));
        object.insert(
            "cleaned_workers".to_string(),
            serde_json::Value::Array(cleaned.clone()),
        );
    }
    write_json_file(&manifest_path, &updated_manifest)?;

    Ok(serde_json::json!({
        "status": "ok",
        "manifest": manifest_path.display().to_string(),
        "keep_branches": keep_branches,
        "workers": cleaned,
    }))
}

fn cmd_parallel_run(
    workspace: &Path,
    manifest: PathBuf,
    execution_policy: &str,
    codex_bin: &str,
    timeout_seconds: Option<u64>,
) -> Result<serde_json::Value> {
    let manifest_path = resolve_workspace_path(workspace, manifest);
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let mut manifest_json: serde_json::Value = serde_json::from_str(&manifest_text)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    let manifest: ParallelPrepareManifest = serde_json::from_value(manifest_json.clone())
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    if manifest.workers.is_empty() {
        anyhow::bail!("parallel run manifest must contain at least one worker");
    }

    let codex_args = parallel_codex_args(execution_policy)?;
    let mut running = Vec::new();
    for worker in &manifest.workers {
        let worktree = PathBuf::from(&worker.worktree);
        if !worktree.exists() {
            anyhow::bail!(
                "parallel worker worktree is missing for worker-{}: {}",
                worker.worker_id,
                worktree.display()
            );
        }
        let prompt_file = worker
            .prompt_file
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| worktree.join(".codex-autoresearch/parallel-worker.md"));
        let prompt = std::fs::read_to_string(&prompt_file)
            .with_context(|| format!("failed to read {}", prompt_file.display()))?;
        let log_file = worktree.join(".codex-autoresearch/parallel-worker.log");
        if let Some(parent) = log_file.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .with_context(|| format!("failed to open {}", log_file.display()))?;
        let err_log = log
            .try_clone()
            .context("failed to clone parallel worker log handle")?;
        let mut child = Command::new(codex_bin)
            .arg("exec")
            .args(&codex_args)
            .current_dir(&worktree)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err_log))
            .spawn()
            .with_context(|| format!("failed to launch worker-{} codex exec", worker.worker_id))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .with_context(|| format!("failed to write worker-{} prompt", worker.worker_id))?;
        }
        running.push(ParallelRunningWorker {
            worker_id: worker.worker_id.clone(),
            child,
            log_file,
            started_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    let mut results = Vec::new();
    for mut worker in running {
        let wait_result = wait_parallel_worker(&mut worker.child, timeout_seconds)
            .with_context(|| format!("failed to wait for worker-{}", worker.worker_id))?;
        let exit_code = wait_result.exit_code;
        results.push(serde_json::json!({
            "worker_id": worker.worker_id,
            "status": wait_result.status,
            "exit_code": exit_code,
            "log_file": worker.log_file.display().to_string(),
            "started_at": worker.started_at,
            "stopped_at": chrono::Utc::now().to_rfc3339(),
        }));
    }

    if let Some(object) = manifest_json.as_object_mut() {
        object.insert("status".to_string(), serde_json::json!("ran"));
        object.insert(
            "worker_runs".to_string(),
            serde_json::Value::Array(results.clone()),
        );
    }
    write_json_file(&manifest_path, &manifest_json)?;
    let success = results.iter().all(|result| {
        result
            .get("status")
            .is_some_and(|status| status == "completed")
    });

    Ok(serde_json::json!({
        "status": if success { "ok" } else { "completed_with_failures" },
        "manifest": manifest_path.display().to_string(),
        "workers": results,
    }))
}

struct ParallelWaitResult {
    status: &'static str,
    exit_code: Option<i32>,
}

fn wait_parallel_worker(
    child: &mut Child,
    timeout_seconds: Option<u64>,
) -> Result<ParallelWaitResult> {
    let Some(seconds) = timeout_seconds else {
        let status = child.wait()?;
        return Ok(ParallelWaitResult {
            status: if status.success() {
                "completed"
            } else {
                "crash"
            },
            exit_code: status.code(),
        });
    };
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(ParallelWaitResult {
                status: if status.success() {
                    "completed"
                } else {
                    "crash"
                },
                exit_code: status.code(),
            });
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait()?;
            return Ok(ParallelWaitResult {
                status: "timeout",
                exit_code: status.code(),
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn parallel_batch_template(workers: u8) -> serde_json::Value {
    let rows = (0..workers)
        .map(|index| {
            let worker_id = ((b'a' + index) as char).to_string();
            serde_json::json!({
                "worker_id": worker_id,
                "status": "completed",
                "metric": "<required>",
                "metrics": {},
                "guard": "skip",
                "commit": "<required-if-keepable>",
                "description": format!("worker-{worker_id} result summary"),
                "diff_size": 0,
                "labels": [],
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(rows)
}

fn cmd_parallel_closeout(
    workspace: &std::path::Path,
    batch_file: &std::path::Path,
    merge_strategy: ParallelMergeStrategy,
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

    let mut ordered_candidates = candidates.clone();
    ordered_candidates
        .sort_by(|left, right| compare_parallel_records(left, right, state.direction));
    let mut merge_failures = BTreeMap::new();
    let mut merged_winner = None;
    for candidate in ordered_candidates {
        match merge_and_verify_parallel_candidate(workspace, &state, &candidate, merge_strategy) {
            Ok((verified_candidate, retained_commit)) => {
                merged_winner = Some((verified_candidate, retained_commit));
                break;
            }
            Err(err) => {
                merge_failures.insert(
                    candidate.worker_id.clone(),
                    summarize_error(&err.to_string()),
                );
            }
        }
    }
    for record in &mut records {
        if let Some(reason) = merge_failures.get(&record.worker_id) {
            record.status = IterationStatus::Discard;
            record.description = format!("{} [MERGE failed] {reason}", record.description);
        }
    }
    let selected_worker = merged_winner
        .as_ref()
        .map(|(record, _)| record.worker_id.clone());
    let best_completed = if merged_winner.is_none() {
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

    let (main_status, main_metric, main_metrics, main_labels, main_commit, main_guard, main_description) = match merged_winner {
        Some((winner_record, retained_commit)) => {
            (
                IterationStatus::Keep,
                winner_record.metric,
                winner_record.metrics.clone(),
                winner_record.labels.clone(),
                Some(retained_commit),
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
            let retained_commit = main_commit
                .clone()
                .context("parallel keep decision requires a retained commit")?;
            if let Some(metrics) = main_metrics.clone() {
                state.record_keep_with_metrics_and_labels(
                    main_metric,
                    retained_commit.clone(),
                    metrics,
                    main_labels.clone(),
                );
            } else {
                state.record_keep_with_labels(main_metric, retained_commit, main_labels.clone());
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
        "merge_strategy": merge_strategy.as_str(),
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

fn parse_parallel_merge_strategy(value: &str) -> Result<ParallelMergeStrategy> {
    match value.trim() {
        "cherry-pick" => Ok(ParallelMergeStrategy::CherryPick),
        "fast-forward" | "ff-only" => Ok(ParallelMergeStrategy::FastForward),
        "squash" => Ok(ParallelMergeStrategy::Squash),
        "rebase" => Ok(ParallelMergeStrategy::Rebase),
        other => anyhow::bail!(
            "Unknown parallel merge strategy: {other}. Use cherry-pick, fast-forward, squash, or rebase."
        ),
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
        if print_tsv_fallback_resume(&results_dir, None)? {
            return Ok(());
        }
        println!(
            r#"{{"resumable":false,"decision":"fresh_start","recommendation":"fresh_start","reason":"no_artifacts"}}"#
        );
        return Ok(());
    }

    let state_content = match std::fs::read_to_string(&state_path) {
        Ok(content) => content,
        Err(err) => {
            if print_tsv_fallback_resume(&results_dir, Some(err.to_string()))? {
                return Ok(());
            }
            return Err(err.into());
        }
    };
    let state: RunState = match serde_json::from_str(&state_content) {
        Ok(state) => state,
        Err(err) => {
            if print_tsv_fallback_resume(&results_dir, Some(err.to_string()))? {
                return Ok(());
            }
            return Err(err.into());
        }
    };
    let tsv_path = results_dir.join("results.tsv");
    if !tsv_path.exists() {
        let out = serde_json::json!({
            "resumable": false,
            "decision": "fresh_start",
            "recommendation": "fresh_start",
            "reason": "missing_results",
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    let log = ResultsLog::open(tsv_path)?;
    if let Err(err) = log.validate() {
        let out = serde_json::json!({
            "resumable": false,
            "decision": "fresh_start",
            "recommendation": "fresh_start",
            "reason": "results_corrupt",
            "error": err.to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

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
    let decision = if recommendation == "resume" {
        "full_resume"
    } else {
        "fresh_start"
    };

    let out = serde_json::json!({
        "resumable": is_resumable,
        "decision": decision,
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

fn print_tsv_fallback_resume(results_dir: &Path, state_error: Option<String>) -> Result<bool> {
    let tsv_path = results_dir.join("results.tsv");
    if !tsv_path.exists() {
        return Ok(false);
    }

    let log = ResultsLog::open(tsv_path.clone())?;
    if let Err(err) = log.validate() {
        let out = serde_json::json!({
            "resumable": false,
            "decision": "fresh_start",
            "recommendation": "fresh_start",
            "reason": "results_corrupt",
            "error": err.to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(true);
    }

    let content = std::fs::read_to_string(&tsv_path)?;
    let rows = parse_results_tsv(&content)?;
    let Some(mut out) = tsv_fallback_resume(&rows, &content, log.tail(5)?) else {
        return Ok(false);
    };
    if let Some(error) = state_error {
        if let Some(map) = out.as_object_mut() {
            map.insert("state_error".to_string(), serde_json::json!(error));
        }
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(true)
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
            "keep" | "keep (reworked)" => {
                keeps += 1;
                current_metric = row.metric;
                if metric_is_better(row.metric, best_metric, direction) {
                    best_metric = row.metric;
                    best_iteration = row.iteration;
                }
            }
            "discard" => discards += 1,
            "crash" | "hook-blocked" | "metric-error" => crashes += 1,
            "no-op" => no_ops += 1,
            "blocked" => blocked += 1,
            "drift" => current_metric = row.metric,
            _ => {}
        }
    }

    let last = rows.last()?;
    Some(serde_json::json!({
        "resumable": true,
        "decision": "tsv_fallback",
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
        .and_then(parse_metric_direction_value)
        .unwrap_or(Direction::Higher)
}

fn is_keep_status(value: &str) -> bool {
    matches!(value, "keep" | "keep (reworked)")
}

fn is_failure_status(value: &str) -> bool {
    matches!(value, "crash" | "hook-blocked" | "metric-error")
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
        let keep_metrics: Vec<Decimal> = parse_results_tsv(&content)?
            .into_iter()
            .filter(|row| is_keep_status(&row.status))
            .map(|row| row.metric)
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

// ── Watch ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchFormat {
    Tsv,
    Jsonl,
}

fn parse_watch_format(value: &str) -> Result<WatchFormat> {
    match value.trim() {
        "tsv" => Ok(WatchFormat::Tsv),
        "jsonl" => Ok(WatchFormat::Jsonl),
        other => anyhow::bail!("Invalid watch format {other:?}; use tsv or jsonl"),
    }
}

fn cmd_watch(
    cwd: Option<PathBuf>,
    lines: usize,
    format: &str,
    once: bool,
    interval_ms: u64,
    websocket: bool,
    websocket_addr: &str,
) -> Result<()> {
    let format = if websocket {
        None
    } else {
        Some(parse_watch_format(format)?)
    };
    let cwd = resolve_cwd(cwd);
    let tsv_path = default_results_tsv(&cwd)
        .context("No results.tsv found. Provide --cwd inside a run workspace.")?;
    if websocket {
        return cmd_watch_websocket(tsv_path, lines, once, interval_ms, websocket_addr);
    }

    let format = format.expect("watch format is parsed when websocket mode is disabled");
    let mut printed_lines = 0usize;

    loop {
        let content = std::fs::read_to_string(&tsv_path)
            .with_context(|| format!("Cannot read {}", tsv_path.display()))?;
        let visible_lines: Vec<&str> = content
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .collect();

        if visible_lines.len() < printed_lines {
            printed_lines = 0;
        }

        let header = visible_lines
            .iter()
            .find(|line| line.starts_with("iteration\t"))
            .copied()
            .context("results.tsv is missing the data header")?;
        let header_columns = watch_header_columns(header);

        if printed_lines == 0 {
            if format == WatchFormat::Tsv {
                if !write_watch_line(header, format, &header_columns)? {
                    return Ok(());
                }
            }

            let data_rows: Vec<&str> = visible_lines
                .iter()
                .copied()
                .filter(|line| !line.starts_with("iteration\t"))
                .collect();
            let start = data_rows.len().saturating_sub(lines);
            for row in &data_rows[start..] {
                if !write_watch_line(row, format, &header_columns)? {
                    return Ok(());
                }
            }
            printed_lines = visible_lines.len();
        } else if visible_lines.len() > printed_lines {
            for row in &visible_lines[printed_lines..] {
                if !write_watch_line(row, format, &header_columns)? {
                    return Ok(());
                }
            }
            printed_lines = visible_lines.len();
        }

        if once {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
    }

    Ok(())
}

fn cmd_watch_websocket(
    tsv_path: PathBuf,
    lines: usize,
    once: bool,
    interval_ms: u64,
    addr: &str,
) -> Result<()> {
    if once {
        let (payloads, _) = watch_websocket_payloads(&tsv_path, lines, 0)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "websocket": true,
                "mode": "snapshot",
                "url": format!("ws://{addr}"),
                "payloads": payloads,
            }))?
        );
        return Ok(());
    }

    let runtime = tokio::runtime::Runtime::new().context("failed to start websocket runtime")?;
    runtime.block_on(run_watch_websocket_server(
        tsv_path,
        lines,
        interval_ms,
        addr.to_string(),
    ))
}

async fn run_watch_websocket_server(
    tsv_path: PathBuf,
    lines: usize,
    interval_ms: u64,
    addr: String,
) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind watch websocket on {addr}"))?;
    let local_addr = listener
        .local_addr()
        .context("failed to read websocket addr")?;
    write_stdout_line(
        &serde_json::json!({
            "websocket": true,
            "listening": local_addr.to_string(),
            "url": format!("ws://{local_addr}"),
        })
        .to_string(),
    )?;

    loop {
        let (stream, _) = listener.accept().await?;
        let client_tsv_path = tsv_path.clone();
        tokio::spawn(async move {
            if let Err(err) =
                serve_watch_websocket_client(stream, client_tsv_path, lines, interval_ms).await
            {
                eprintln!("watch websocket client error: {err:#}");
            }
        });
    }
}

async fn serve_watch_websocket_client(
    stream: tokio::net::TcpStream,
    tsv_path: PathBuf,
    lines: usize,
    interval_ms: u64,
) -> Result<()> {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;

    let mut socket = tokio_tungstenite::accept_async(stream)
        .await
        .context("failed to accept watch websocket client")?;
    let mut printed_lines = 0usize;

    loop {
        let (payloads, next_printed_lines) =
            watch_websocket_payloads(&tsv_path, lines, printed_lines)?;
        printed_lines = next_printed_lines;

        for payload in payloads {
            socket
                .send(Message::Text(payload.to_string().into()))
                .await
                .context("failed to send watch websocket payload")?;
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
}

fn watch_websocket_payloads(
    tsv_path: &Path,
    lines: usize,
    printed_lines: usize,
) -> Result<(Vec<serde_json::Value>, usize)> {
    let content = std::fs::read_to_string(tsv_path)
        .with_context(|| format!("Cannot read {}", tsv_path.display()))?;
    let visible_lines: Vec<&str> = content
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect();
    let printed_lines = if visible_lines.len() < printed_lines {
        0
    } else {
        printed_lines
    };
    let header = visible_lines
        .iter()
        .find(|line| line.starts_with("iteration\t"))
        .copied()
        .context("results.tsv is missing the data header")?;
    let header_columns = watch_header_columns(header);
    let data_rows: Vec<&str> = visible_lines
        .iter()
        .copied()
        .filter(|line| !line.starts_with("iteration\t"))
        .collect();

    if printed_lines == 0 {
        let start = data_rows.len().saturating_sub(lines);
        let rows = data_rows[start..]
            .iter()
            .map(|row| watch_row_json(row, &header_columns))
            .collect::<Vec<_>>();
        return Ok((
            vec![serde_json::json!({
                "type": "snapshot",
                "rows": rows,
            })],
            visible_lines.len(),
        ));
    }

    if visible_lines.len() <= printed_lines {
        return Ok((Vec::new(), printed_lines));
    }

    let payloads = visible_lines[printed_lines..]
        .iter()
        .map(|row| {
            serde_json::json!({
                "type": "row",
                "row": watch_row_json(row, &header_columns),
            })
        })
        .collect::<Vec<_>>();
    Ok((payloads, visible_lines.len()))
}

fn watch_header_columns(header: &str) -> Vec<&str> {
    header.split('\t').collect()
}

fn watch_row_json(row: &str, header_columns: &[&str]) -> serde_json::Value {
    let values = row.split('\t').collect::<Vec<_>>();
    let mut object = serde_json::Map::new();
    for (index, key) in header_columns.iter().enumerate() {
        object.insert(
            (*key).to_string(),
            serde_json::Value::String(values.get(index).copied().unwrap_or("").to_string()),
        );
    }
    serde_json::Value::Object(object)
}

fn write_watch_line(line: &str, format: WatchFormat, header_columns: &[&str]) -> Result<bool> {
    match format {
        WatchFormat::Tsv => write_stdout_line(line),
        WatchFormat::Jsonl => write_stdout_line(&serde_json::to_string(&watch_row_json(
            line,
            header_columns,
        ))?),
    }
}

fn write_stdout_line(line: &str) -> Result<bool> {
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = IoWrite::write_all(&mut stdout, line.as_bytes())
        .and_then(|_| IoWrite::write_all(&mut stdout, b"\n"))
    {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            return Ok(false);
        }
        return Err(err).context("failed writing to stdout");
    }
    Ok(true)
}

// ── Lessons ──────────────────────────────────────────────────────────

fn cmd_lessons(
    add: Option<&str>,
    category: &str,
    outcome: &str,
    context: &str,
    search: Option<&str>,
    last: Option<usize>,
    workspace_context: bool,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let log = LessonsLog::open_or_create(&results_dir)?;

    if let Some(strategy) = add {
        let lesson = lessons::Lesson {
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
            category: parse_lesson_category(category)?,
            strategy: strategy.to_string(),
            outcome: parse_lesson_outcome(outcome)?,
            context: context.to_string(),
        };
        log.append(&lesson)?;
        println!(
            "{}",
            serde_json::json!({
                "status": "ok",
                "path": log.path(),
            })
        );
        return Ok(());
    }

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

    let out = if workspace_context {
        let context = load_run_context(&workspace).ok();
        serde_json::to_string_pretty(&serde_json::json!({
            "workspace_root": workspace.display().to_string(),
            "path": log.path(),
            "repo_targets": context.map(|context| context.repo_targets).unwrap_or_default(),
            "lessons": tail,
        }))?
    } else {
        serde_json::to_string_pretty(&tail)?
    };
    println!("{out}");
    Ok(())
}

fn parse_lesson_category(value: &str) -> Result<lessons::LessonCategory> {
    match value.trim() {
        "positive" => Ok(lessons::LessonCategory::Positive),
        "negative" => Ok(lessons::LessonCategory::Negative),
        "strategic" => Ok(lessons::LessonCategory::Strategic),
        other => {
            anyhow::bail!("Invalid lesson category {other:?}; use positive, negative, or strategic")
        }
    }
}

fn parse_lesson_outcome(value: &str) -> Result<lessons::LessonOutcome> {
    match value.trim() {
        "success" => Ok(lessons::LessonOutcome::Success),
        "failure" => Ok(lessons::LessonOutcome::Failure),
        "neutral" => Ok(lessons::LessonOutcome::Neutral),
        other => {
            anyhow::bail!("Invalid lesson outcome {other:?}; use success, failure, or neutral")
        }
    }
}

// ── Search ───────────────────────────────────────────────────────────

fn cmd_search(
    query: Option<String>,
    from_state: bool,
    provider_command: Option<String>,
    limit: usize,
    refresh: bool,
    log: bool,
    cwd: Option<PathBuf>,
) -> Result<()> {
    if limit == 0 {
        anyhow::bail!("search --limit must be greater than zero");
    }
    let workspace = resolve_results_workspace(cwd);
    let query = resolve_search_query(&workspace, query, from_state)?;
    let result = run_search_request(&workspace, query, provider_command, limit, refresh, log)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn resolve_search_query(
    workspace: &Path,
    query: Option<String>,
    from_state: bool,
) -> Result<String> {
    match query {
        Some(query) if !query.trim().is_empty() => Ok(query.trim().to_string()),
        _ if from_state => search_query_from_state(workspace),
        _ => anyhow::bail!("search requires --query or --from-state"),
    }
}

fn run_search_request(
    workspace: &Path,
    query: String,
    provider_command: Option<String>,
    limit: usize,
    refresh: bool,
    log: bool,
) -> Result<serde_json::Value> {
    if limit == 0 {
        anyhow::bail!("search --limit must be greater than zero");
    }
    let provider_command = provider_command
        .or_else(|| std::env::var("AUTORESEARCH_SEARCH_CMD").ok())
        .filter(|command| !command.trim().is_empty());

    let Some(provider_command) = provider_command else {
        let mut result = serde_json::json!({
            "status": "skipped",
            "reason": "no provider command configured; pass --provider-command or set AUTORESEARCH_SEARCH_CMD",
            "query": query,
            "results": [],
        });
        if log {
            let iteration = log_search_result(
                &workspace,
                result["query"].as_str().unwrap_or(""),
                0,
                false,
                "skipped",
            )?;
            if let Some(object) = result.as_object_mut() {
                object.insert("logged_iteration".to_string(), serde_json::json!(iteration));
            }
        }
        return Ok(result);
    };
    verify::screen_command(&provider_command)?;

    let results_dir = workspace.join("autoresearch-results");
    let cache_dir = results_dir.join("search-cache");
    let cache_path = cache_dir.join(search_cache_key(&provider_command, &query, limit));
    if !refresh && cache_path.exists() {
        let mut cached: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&cache_path)
                .with_context(|| format!("failed to read {}", cache_path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", cache_path.display()))?;
        if let Some(object) = cached.as_object_mut() {
            object.insert("cache_hit".to_string(), serde_json::json!(true));
            object.insert(
                "cache_path".to_string(),
                serde_json::json!(cache_path.display().to_string()),
            );
        }
        if log {
            let iteration =
                log_search_result(&workspace, &query, search_result_count(&cached), true, "ok")?;
            if let Some(object) = cached.as_object_mut() {
                object.insert("logged_iteration".to_string(), serde_json::json!(iteration));
            }
        }
        return Ok(cached);
    }

    let output = Command::new("sh")
        .arg("-c")
        .arg(&provider_command)
        .current_dir(&workspace)
        .env("AUTORESEARCH_SEARCH_QUERY", &query)
        .env("AUTORESEARCH_SEARCH_LIMIT", limit.to_string())
        .output()
        .with_context(|| format!("failed to run search provider command: {provider_command}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        anyhow::bail!(
            "search provider exited with status {}. stderr: {}",
            output.status.code().unwrap_or(-1),
            stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ")
        );
    }

    let mut result = serde_json::json!({
        "status": "ok",
        "query": query,
        "provider": provider_command,
        "cache_hit": false,
        "cache_path": cache_path.display().to_string(),
        "results": parse_search_provider_results(&stdout),
        "raw_stdout": stdout,
    });
    std::fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;
    write_json_file(&cache_path, &result)?;
    if log {
        let iteration = log_search_result(
            &workspace,
            result["query"].as_str().unwrap_or(""),
            search_result_count(&result),
            false,
            "ok",
        )?;
        if let Some(object) = result.as_object_mut() {
            object.insert("logged_iteration".to_string(), serde_json::json!(iteration));
        }
    }
    Ok(result)
}

fn maybe_run_auto_web_search(
    workspace: &Path,
    escalation_action: Option<EscalationAction>,
    state_iteration: u32,
) -> Option<serde_json::Value> {
    if escalation_action != Some(EscalationAction::WebSearch) {
        return None;
    }
    match automatic_search_blocker(workspace, state_iteration) {
        Ok(Some(reason)) => {
            return Some(serde_json::json!({
                "status": "skipped",
                "reason": reason,
            }));
        }
        Ok(None) => {}
        Err(err) => {
            return Some(serde_json::json!({
                "status": "error",
                "reason": "auto_search_gate_failed",
                "error": err.to_string(),
            }));
        }
    }

    let result = search_query_from_state(workspace)
        .and_then(|query| run_search_request(workspace, query, None, 5, false, true));

    Some(match result {
        Ok(result) => result,
        Err(err) => serde_json::json!({
            "status": "error",
            "reason": "auto_search_failed",
            "error": err.to_string(),
        }),
    })
}

fn automatic_search_blocker(workspace: &Path, state_iteration: u32) -> Result<Option<String>> {
    if state_iteration < 3 {
        return Ok(Some(
            "automatic web search waits until after the first 3 iterations".to_string(),
        ));
    }

    let results_path = workspace.join("autoresearch-results/results.tsv");
    if !results_path.exists() {
        return Ok(None);
    }
    let rows = ResultsLog::open(results_path)?.tail(10)?;
    let mut search_iterations = Vec::new();
    for row in rows {
        let columns = row.split('\t').collect::<Vec<_>>();
        if columns.get(5) == Some(&"search") {
            if let Some(iteration) = columns.first().and_then(|raw| raw.parse::<u32>().ok()) {
                search_iterations.push(iteration);
            }
        }
    }

    if search_iterations.len() >= 3 {
        return Ok(Some(
            "automatic web search limit reached: 3 searches in the last 10 iterations".to_string(),
        ));
    }
    if search_iterations
        .last()
        .is_some_and(|last| state_iteration.saturating_sub(*last) < 2)
    {
        return Ok(Some(
            "automatic web search cooldown active: wait at least 2 iterations".to_string(),
        ));
    }

    Ok(None)
}

fn search_query_from_state(workspace: &Path) -> Result<String> {
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    let state: RunState = serde_json::from_str(
        &std::fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read {}", state_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", state_path.display()))?;
    let mut parts = Vec::new();
    if let Some(config) = state.config.as_ref() {
        if !config.goal.trim().is_empty() {
            parts.push(config.goal.trim().to_string());
        }
        if !config.metric.trim().is_empty() {
            parts.push(format!("metric {}", config.metric.trim()));
        }
    }
    parts.push(format!("status {}", state.last_status.as_str()));
    parts.push(format!("direction {}", state.direction.as_str()));
    if state.consecutive_discards > 0 {
        parts.push(format!(
            "{} consecutive discards",
            state.consecutive_discards
        ));
    }

    let esc_path = results_dir.join("escalation.json");
    if esc_path.exists() {
        let escalation: EscalationState = serde_json::from_str(
            &std::fs::read_to_string(&esc_path)
                .with_context(|| format!("failed to read {}", esc_path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", esc_path.display()))?;
        parts.push(format!("escalation {:?}", escalation.last_action));
        if escalation.pivots_since_last_keep > 0 {
            parts.push(format!(
                "{} pivots without keep",
                escalation.pivots_since_last_keep
            ));
        }
    }

    Ok(parts.join(" "))
}

fn parse_search_provider_results(stdout: &str) -> serde_json::Value {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return serde_json::Value::Array(Vec::new());
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if value.is_array() {
            return value;
        }
        if let Some(results) = value.get("results") {
            return results.clone();
        }
        return serde_json::Value::Array(vec![value]);
    }

    serde_json::Value::Array(
        trimmed
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::json!({ "title": line.trim() }))
            .collect(),
    )
}

fn search_cache_key(provider: &str, query: &str, limit: usize) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in format!("{provider}\0{query}\0{limit}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}.json")
}

fn search_result_count(value: &serde_json::Value) -> usize {
    value
        .get("results")
        .and_then(|results| results.as_array())
        .map(Vec::len)
        .unwrap_or(0)
}

fn log_search_result(
    workspace: &Path,
    query: &str,
    result_count: usize,
    cache_hit: bool,
    provider_status: &str,
) -> Result<u32> {
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    let tsv_path = results_dir.join("results.tsv");
    if !state_path.exists() || !tsv_path.exists() {
        anyhow::bail!("search --log requires an active autoresearch run");
    }

    let mut state: RunState = serde_json::from_str(
        &std::fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read {}", state_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", state_path.display()))?;
    let iteration = state.iteration + 1;
    let description = format!(
        "[SEARCH] \"{}\" -> {result_count} results ({provider_status}, cache_hit={cache_hit})",
        query.replace(['\n', '\r', '\t'], " ")
    );
    let row = ResultRow {
        iteration,
        commit: None,
        metric: state.current_metric,
        delta: Decimal::ZERO,
        guard: GuardResult::Skip,
        status: IterationStatus::Search,
        description,
    };
    ResultsLog::open(tsv_path)?.append(&row)?;
    state.record_meta_status(IterationStatus::Search, state.current_metric);
    std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
    Ok(iteration)
}

// ── Handoff ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn cmd_handoff(
    source: &str,
    status: &str,
    findings: Option<&str>,
    config: Option<&str>,
    chain: Option<&str>,
    evals: bool,
    evals_interval: Option<u32>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    std::fs::create_dir_all(&results_dir)?;

    let findings_val: serde_json::Value =
        serde_json::from_str(findings.unwrap_or("[]")).context("Invalid findings JSON")?;
    let config_val: serde_json::Value =
        serde_json::from_str(config.unwrap_or("{}")).context("Invalid config JSON")?;
    if !findings_val.is_array() {
        anyhow::bail!("handoff findings must be a JSON array");
    }
    if !config_val.is_object() {
        anyhow::bail!("handoff config must be a JSON object");
    }
    if !is_valid_handoff_source(source) {
        anyhow::bail!("invalid handoff source {source:?}");
    }
    if !is_valid_handoff_status(status) {
        anyhow::bail!("invalid handoff status {status:?}");
    }
    if evals_interval == Some(0) {
        anyhow::bail!("handoff evals interval must be greater than zero");
    }
    if evals_interval.is_some() && !evals {
        anyhow::bail!("handoff evals interval requires --evals");
    }
    let goal = config_val
        .get("goal")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let scope = config_val
        .get("scope")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let metric = config_val
        .get("metric")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let direction = config_val
        .get("direction")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let verify = config_val
        .get("verify")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let guard = config_val
        .get("guard")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let verify_format = config_val
        .get("verify_format")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let primary_metric_key = config_val
        .get("primary_metric_key")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let iterations = config_val
        .get("iterations")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let stop_condition = config_val
        .get("stop_condition")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let acceptance_criteria = config_val
        .get("acceptance_criteria")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let required_keep_criteria = config_val
        .get("required_keep_criteria")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let required_keep_labels = config_val
        .get("required_keep_labels")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let required_stop_labels = config_val
        .get("required_stop_labels")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let rollback_strategy = config_val
        .get("rollback_strategy")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let run_mode = config_val
        .get("run_mode")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let run_tag = config_val
        .get("run_tag")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let mode = config_val
        .get("mode")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let hypothesis_queue = config_val
        .get("hypothesis_queue")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let summary = config_val
        .get("summary")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let timestamp = chrono::Utc::now().to_rfc3339();
    let handoff_path = results_dir.join("handoff.json");
    let chain_targets = parse_handoff_chain_targets(chain)?;
    let next_target = chain_targets
        .first()
        .cloned()
        .map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null);
    let (primary_repo, repo_targets) = handoff_context_values(&results_dir)?;

    let handoff = serde_json::json!({
        "version": "2.1.0",
        "protocol_version": "2.1.0",
        "binary_version": env!("CARGO_PKG_VERSION"),
        "source": source,
        "source_command": source,
        "timestamp": timestamp,
        "status": status,
        "results_tsv": "autoresearch-results/results.tsv",
        "workspace_root": workspace.to_string_lossy().to_string(),
        "artifact_root": results_dir.to_string_lossy().to_string(),
        "primary_repo": primary_repo,
        "repo_targets": repo_targets,
        "results_path": results_dir.join("results.tsv").to_string_lossy().to_string(),
        "handoff_path": handoff_path.to_string_lossy().to_string(),
        "goal": goal,
        "scope": scope,
        "metric": metric,
        "direction": direction,
        "verify": verify,
        "guard": guard,
        "verify_format": verify_format,
        "primary_metric_key": primary_metric_key,
        "iterations": iterations,
        "stop_condition": stop_condition,
        "acceptance_criteria": acceptance_criteria,
        "required_keep_criteria": required_keep_criteria,
        "required_keep_labels": required_keep_labels,
        "required_stop_labels": required_stop_labels,
        "rollback_strategy": rollback_strategy,
        "run_mode": run_mode,
        "run_tag": run_tag,
        "mode": mode,
        "hypothesis_queue": hypothesis_queue,
        "summary": summary,
        "findings": findings_val,
        "config": config_val,
        "chain": chain_targets,
        "next_target": next_target,
        "chain_continue": should_continue_handoff_chain(status),
        "propagate_evals": evals,
        "evals_interval": evals_interval,
    });

    std::fs::write(&handoff_path, serde_json::to_string_pretty(&handoff)?)?;

    println!(r#"{{"status":"ok","path":"autoresearch-results/handoff.json"}}"#);
    Ok(())
}

fn handoff_context_values(results_dir: &Path) -> Result<(serde_json::Value, serde_json::Value)> {
    let context_path = results_dir.join("context.json");
    if !context_path.exists() {
        return Ok((serde_json::Value::Null, serde_json::Value::Null));
    }
    let context: context::RunContext =
        serde_json::from_str(&std::fs::read_to_string(&context_path)?)
            .with_context(|| format!("Invalid context JSON at {}", context_path.display()))?;
    Ok((
        serde_json::Value::String(context.primary_repo),
        serde_json::to_value(context.repo_targets)?,
    ))
}

// ── Exec ─────────────────────────────────────────────────────────────

fn cmd_exec(iterations: u32, cwd: Option<PathBuf>) -> Result<()> {
    match cmd_exec_inner(iterations, cwd) {
        Ok(()) => Ok(()),
        Err(err) => exec_hard_error("startup_failed", err.to_string()),
    }
}

fn cmd_exec_inner(iterations: u32, cwd: Option<PathBuf>) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    if iterations == 0 {
        return exec_hard_error(
            "invalid_iterations",
            "exec: --iterations must be greater than zero".to_string(),
        );
    }

    // Read config from stdin
    let mut config: RunConfig = serde_json::from_reader(std::io::stdin().lock())
        .context("exec: failed to parse RunConfig from stdin")?;
    validate_exec_config(&config)?;
    config.iterations = Some(iterations);

    // Extract display values before moving config
    let direction = config.direction;
    let verify_cmd = config.verify.clone();
    let guard_cmd = config.guard.clone();
    let fmt = config.verify_format;
    let primary_key = config.primary_metric_key.clone();

    // Screen
    if let Err(e) = verify::screen_command(&verify_cmd) {
        return exec_hard_error("unsafe_command", e.to_string());
    }
    if let Some(command) = guard_cmd
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
    {
        if let Err(e) = verify::screen_command(command) {
            return exec_hard_error("unsafe_command", e.to_string());
        }
    }

    // Git check
    let git = GitRepo::open(&workspace).context("exec: requires a git repository")?;
    match git.worktree_status()? {
        WorktreeStatus::Clean | WorktreeStatus::OnlyArtifacts => {}
        WorktreeStatus::Dirty(files) => {
            return exec_hard_error(
                "dirty_worktree",
                format!("unexpected worktree changes: {}", files.join(", ")),
            );
        }
    }

    // Baseline
    let result = verify::run_verify(&verify_cmd, fmt, primary_key.as_deref(), &workspace)
        .context("exec: baseline verification failed")?;
    if fmt == VerifyFormat::MetricsJson {
        let metrics = result
            .metrics
            .as_ref()
            .context("exec: verify_format=metrics_json requires structured baseline metrics")?;
        let primary_metric_key = primary_key.as_deref().unwrap_or("metric");
        if let Err(err) = ensure_metrics_json_keys(
            metrics,
            primary_metric_key,
            &config.acceptance_criteria,
            &config.required_keep_criteria,
        ) {
            return exec_hard_error("invalid_metrics_json", format!("exec: {err}"));
        }
    }
    let baseline_guard = match run_baseline_guard(guard_cmd.as_deref(), &workspace) {
        Ok(guard) => guard,
        Err(err) => return exec_hard_error("guard_failed", format!("exec: {err}")),
    };
    let head = git.head_short()?;

    // Init artifacts + protect from git staging
    let results_dir = ensure_results_dir_protected(&workspace)?;
    archive_existing_exec_artifacts(&results_dir)?;

    let log = ResultsLog::create(&results_dir, direction)?;
    log.append(&ResultRow {
        iteration: 0,
        commit: Some(head.clone()),
        metric: result.metric,
        delta: Decimal::ZERO,
        guard: baseline_guard,
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

fn validate_exec_config(config: &RunConfig) -> Result<()> {
    if config.goal.trim().is_empty() {
        anyhow::bail!("exec: missing required field: goal");
    }
    if config.scope.is_empty() || config.scope.iter().all(|scope| scope.trim().is_empty()) {
        anyhow::bail!("exec: missing required field: scope");
    }
    if config.metric.trim().is_empty() {
        anyhow::bail!("exec: missing required field: metric");
    }
    if config.verify.trim().is_empty() {
        anyhow::bail!("exec: missing required field: verify");
    }
    Ok(())
}

fn exec_hard_error(code: &str, reason: String) -> Result<()> {
    let out = serde_json::json!({
        "type": "error",
        "code": code,
        "error": reason,
        "exit_code": 2,
    });
    eprintln!("{}", serde_json::to_string(&out)?);
    std::process::exit(2);
}

fn archive_existing_exec_artifacts(results_dir: &Path) -> Result<()> {
    for name in ["results.tsv", "state.json", "context.json"] {
        archive_existing_file(&results_dir.join(name))?;
    }
    Ok(())
}

fn archive_existing_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let prev_path = path.with_extension(format!(
        "{}.prev",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
    ));
    if prev_path.exists() {
        std::fs::remove_file(&prev_path)
            .with_context(|| format!("failed to replace {}", prev_path.display()))?;
    }
    std::fs::rename(path, &prev_path).with_context(|| {
        format!(
            "failed to archive {} to {}",
            path.display(),
            prev_path.display()
        )
    })
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
    if let Some(pointer_workspace) = pointer_results_workspace(&workspace) {
        return pointer_workspace;
    }
    GitRepo::open(&workspace)
        .ok()
        .and_then(|repo| repo.workdir())
        .and_then(|root| {
            if root.join("autoresearch-results").exists() {
                Some(root)
            } else {
                pointer_results_workspace(&root)
            }
        })
        .unwrap_or(workspace)
}

fn pointer_results_workspace(repo: &Path) -> Option<PathBuf> {
    let pointer_path = repo.join(".codex-autoresearch/pointer.json");
    let pointer: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(pointer_path).ok()?).ok()?;

    if let Some(context_path) = pointer.get("context_path").and_then(|value| value.as_str()) {
        let context_path = PathBuf::from(context_path);
        let artifact_root = context_path.parent()?;
        if artifact_root
            .file_name()
            .is_some_and(|name| name == "autoresearch-results")
        {
            let workspace = artifact_root.parent()?.to_path_buf();
            if context_path.exists() && artifact_root.exists() {
                return Some(workspace);
            }
        }
        return None;
    }

    let workspace = PathBuf::from(pointer.get("workspace_root")?.as_str()?);
    if workspace.join("autoresearch-results").exists() {
        return Some(workspace);
    }
    None
}

fn resolve_workspace_path(workspace: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        workspace.join(path)
    }
}

fn parse_companion_repo_scopes(
    workspace: &Path,
    entries: Vec<String>,
) -> Result<Vec<RepoTargetConfig>> {
    let mut targets = Vec::new();
    for raw in entries {
        let (path_raw, scope_raw) = raw
            .split_once('=')
            .with_context(|| format!("Invalid companion repo scope {raw:?}; use PATH=SCOPE"))?;
        let path_raw = path_raw.trim();
        let scope = scope_raw.trim();
        if path_raw.is_empty() {
            anyhow::bail!("Invalid companion repo scope {raw:?}; PATH is required");
        }
        if scope.is_empty() {
            anyhow::bail!("Invalid companion repo scope {raw:?}; SCOPE is required");
        }

        let candidate = resolve_workspace_path(workspace, PathBuf::from(path_raw));
        let repo = GitRepo::open(&candidate)
            .with_context(|| format!("companion repo {path_raw:?} is not a git repository"))?;
        validate_companion_repo_clean(&repo, path_raw)?;
        let workdir = repo
            .workdir()
            .unwrap_or(candidate)
            .canonicalize()
            .with_context(|| format!("failed to canonicalize companion repo {path_raw:?}"))?;
        targets.push(RepoTargetConfig {
            path: workdir,
            scope: scope.to_string(),
            role: "companion".to_string(),
        });
    }
    Ok(targets)
}

fn validate_companion_repo_clean(repo: &GitRepo, label: &str) -> Result<()> {
    let lock_files = repo.lock_files();
    if !lock_files.is_empty() {
        anyhow::bail!(
            "init preflight blocked: companion repo {label} has stale git lock files: {}",
            lock_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if repo.head_detached()? {
        anyhow::bail!("init preflight blocked: companion repo {label} is detached_head");
    }
    let staged_artifacts = repo.staged_owned_artifacts()?;
    if !staged_artifacts.is_empty() {
        anyhow::bail!(
            "init preflight blocked: companion repo {label} has autoresearch-owned artifacts staged: {}",
            staged_artifacts.join(", ")
        );
    }
    if let WorktreeStatus::Dirty(files) = repo.worktree_status()? {
        anyhow::bail!(
            "init preflight blocked: companion repo {label} has unexpected worktree changes: {}",
            files.join(", ")
        );
    }
    Ok(())
}

fn write_json_file(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn run_git_command(workspace: &Path, args: &[String]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed: {}{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }
    Ok(())
}

fn git_branch_exists(workspace: &Path, branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["show-ref", "--verify", "--quiet"])
        .arg(format!("refs/heads/{branch}"))
        .output()
        .context("failed to inspect git branch")?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => anyhow::bail!(
            "git show-ref failed: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ),
    }
}

fn cherry_pick_parallel_commit(workspace: &Path, commit: &str) -> Result<String> {
    let before = GitRepo::open(workspace)?.head_full()?;
    let Some(resolved) = git_resolve_commit(workspace, commit)? else {
        anyhow::bail!("selected worker commit does not exist: {commit}");
    };
    if before == resolved {
        return GitRepo::open(workspace)?.head_short();
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["cherry-pick", "--no-edit", &resolved])
        .output()
        .context("failed to run git cherry-pick")?;
    if !output.status.success() {
        let _ = Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(["cherry-pick", "--abort"])
            .output();
        anyhow::bail!(
            "git cherry-pick {commit} failed: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }

    GitRepo::open(workspace)?.head_short()
}

fn fast_forward_parallel_commit(workspace: &Path, commit: &str) -> Result<String> {
    let before = GitRepo::open(workspace)?.head_full()?;
    let Some(resolved) = git_resolve_commit(workspace, commit)? else {
        anyhow::bail!("selected worker commit does not exist: {commit}");
    };
    if before == resolved {
        return GitRepo::open(workspace)?.head_short();
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["merge", "--ff-only", &resolved])
        .output()
        .context("failed to run git merge --ff-only")?;
    if !output.status.success() {
        anyhow::bail!(
            "git merge --ff-only {commit} failed: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }

    GitRepo::open(workspace)?.head_short()
}

fn squash_parallel_commit(workspace: &Path, commit: &str) -> Result<String> {
    let before = GitRepo::open(workspace)?.head_full()?;
    let Some(resolved) = git_resolve_commit(workspace, commit)? else {
        anyhow::bail!("selected worker commit does not exist: {commit}");
    };
    if before == resolved {
        return GitRepo::open(workspace)?.head_short();
    }

    let merge = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["merge", "--squash", &resolved])
        .output()
        .context("failed to run git merge --squash")?;
    if !merge.status.success() {
        let _ = Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(["merge", "--abort"])
            .output();
        anyhow::bail!(
            "git merge --squash {commit} failed: {}{}",
            String::from_utf8_lossy(&merge.stderr),
            String::from_utf8_lossy(&merge.stdout)
        );
    }

    let diff = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["diff", "--cached", "--quiet"])
        .output()
        .context("failed to inspect squashed changes")?;
    if diff.status.success() {
        return GitRepo::open(workspace)?.head_short();
    }

    let message = format!("autoresearch parallel squash {}", short_commit(&resolved));
    let commit_output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["commit", "-m", &message])
        .output()
        .context("failed to commit squashed parallel result")?;
    if !commit_output.status.success() {
        anyhow::bail!(
            "git commit after squash {commit} failed: {}{}",
            String::from_utf8_lossy(&commit_output.stderr),
            String::from_utf8_lossy(&commit_output.stdout)
        );
    }

    GitRepo::open(workspace)?.head_short()
}

fn rebase_parallel_commit(workspace: &Path, commit: &str) -> Result<String> {
    let before = GitRepo::open(workspace)?.head_full()?;
    let Some(resolved) = git_resolve_commit(workspace, commit)? else {
        anyhow::bail!("selected worker commit does not exist: {commit}");
    };
    if before == resolved {
        return GitRepo::open(workspace)?.head_short();
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["rebase", &resolved])
        .output()
        .context("failed to run git rebase")?;
    if !output.status.success() {
        let _ = Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(["rebase", "--abort"])
            .output();
        anyhow::bail!(
            "git rebase {commit} failed: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }

    GitRepo::open(workspace)?.head_short()
}

fn merge_parallel_commit(
    workspace: &Path,
    commit: &str,
    strategy: ParallelMergeStrategy,
) -> Result<String> {
    match strategy {
        ParallelMergeStrategy::CherryPick => cherry_pick_parallel_commit(workspace, commit),
        ParallelMergeStrategy::FastForward => fast_forward_parallel_commit(workspace, commit),
        ParallelMergeStrategy::Squash => squash_parallel_commit(workspace, commit),
        ParallelMergeStrategy::Rebase => rebase_parallel_commit(workspace, commit),
    }
}

fn merge_and_verify_parallel_candidate(
    workspace: &Path,
    state: &RunState,
    candidate: &ParallelWorkerRecord,
    merge_strategy: ParallelMergeStrategy,
) -> Result<(ParallelWorkerRecord, String)> {
    let Some(commit) = candidate.commit.as_deref() else {
        anyhow::bail!("missing commit");
    };
    let before = GitRepo::open(workspace)?.head_full()?;
    let retained_commit = match merge_parallel_commit(workspace, commit, merge_strategy) {
        Ok(commit) => commit,
        Err(err) => {
            reset_to_commit(workspace, &before)?;
            return Err(err);
        }
    };

    let result = verify_parallel_retained_candidate(workspace, state, candidate);
    match result {
        Ok(verified) => Ok((verified, retained_commit)),
        Err(err) => {
            reset_to_commit(workspace, &before)?;
            Err(err)
        }
    }
}

fn verify_parallel_retained_candidate(
    workspace: &Path,
    state: &RunState,
    candidate: &ParallelWorkerRecord,
) -> Result<ParallelWorkerRecord> {
    let config = state
        .config
        .as_ref()
        .context("parallel closeout requires run config for post-merge verification")?;
    verify::screen_command(&config.verify)?;
    let verify_result = verify::run_verify(
        &config.verify,
        config.verify_format,
        config.primary_metric_key.as_deref(),
        workspace,
    )
    .context("post-merge verify failed")?;
    let guard = match config.guard.as_deref() {
        Some(command) if !command.trim().is_empty() => {
            verify::screen_command(command)?;
            let result =
                verify::run_guard(command, workspace).context("post-merge guard failed")?;
            if result.passed {
                GuardResult::Pass
            } else {
                GuardResult::Fail
            }
        }
        _ => GuardResult::Skip,
    };
    if guard == GuardResult::Fail {
        anyhow::bail!("post-merge guard failed");
    }

    let primary_metric_key = config
        .primary_metric_key
        .clone()
        .or_else(|| {
            if config.metric.trim().is_empty() {
                None
            } else {
                Some(config.metric.clone())
            }
        })
        .unwrap_or_else(|| "metric".to_string());
    let mut metrics = verify_result
        .metrics
        .clone()
        .unwrap_or_else(|| BTreeMap::from([(primary_metric_key.clone(), verify_result.metric)]));
    metrics
        .entry("metric".to_string())
        .or_insert(verify_result.metric);
    metrics
        .entry(primary_metric_key)
        .or_insert(verify_result.metric);
    let delta = verify_result.metric - state.current_metric;
    if !state.direction.is_improvement(delta) {
        anyhow::bail!(
            "post-merge verify did not improve retained metric: {}",
            verify_result.metric
        );
    }
    let required_keep = criteria::evaluate_criteria(&config.required_keep_criteria, &metrics);
    if !required_keep.satisfied {
        anyhow::bail!(
            "post-merge keep criteria failed: {}",
            required_keep.failures.join("; ")
        );
    }
    let missing_labels = missing_required_labels(&config.required_keep_labels, &candidate.labels);
    if !missing_labels.is_empty() {
        anyhow::bail!(
            "post-merge required labels missing: {}",
            missing_labels.join(", ")
        );
    }

    let mut verified = candidate.clone();
    verified.metric = verify_result.metric;
    verified.metrics = Some(metrics);
    verified.guard = guard;
    verified.status = IterationStatus::Keep;
    Ok(verified)
}

fn reset_to_commit(workspace: &Path, commit: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["reset", "--hard", commit])
        .output()
        .context("failed to run git reset")?;
    if !output.status.success() {
        anyhow::bail!(
            "git reset --hard {commit} failed: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }
    Ok(())
}

fn git_resolve_commit(workspace: &Path, commit: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["rev-parse", "--verify"])
        .arg(format!("{commit}^{{commit}}"))
        .output()
        .context("failed to resolve git commit")?;
    match output.status.code() {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        )),
        Some(1) => Ok(None),
        _ => anyhow::bail!(
            "git rev-parse failed: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ),
    }
}

fn short_commit(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

fn summarize_error(message: &str) -> String {
    let mut summary = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if summary.len() > 180 {
        summary.truncate(180);
        summary.push_str("...");
    }
    summary
}

fn parallel_codex_args(execution_policy: &str) -> Result<Vec<String>> {
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

fn copy_pointer_to_worker(workspace: &Path, worktree: &Path) -> Result<()> {
    let pointer = workspace.join(".codex-autoresearch/pointer.json");
    if !pointer.exists() {
        return Ok(());
    }
    let worker_pointer_dir = worktree.join(".codex-autoresearch");
    std::fs::create_dir_all(&worker_pointer_dir)
        .with_context(|| format!("failed to create {}", worker_pointer_dir.display()))?;
    std::fs::copy(&pointer, worker_pointer_dir.join("pointer.json"))
        .with_context(|| format!("failed to copy {}", pointer.display()))?;
    Ok(())
}

fn write_parallel_worker_prompt(
    worktree: &Path,
    state: &RunState,
    worker_id: &str,
    branch: &str,
) -> Result<PathBuf> {
    let prompt_dir = worktree.join(".codex-autoresearch");
    std::fs::create_dir_all(&prompt_dir)
        .with_context(|| format!("failed to create {}", prompt_dir.display()))?;
    let prompt_path = prompt_dir.join("parallel-worker.md");
    let config = state.config.as_ref();
    let goal = config
        .map(|config| config.goal.as_str())
        .filter(|goal| !goal.trim().is_empty())
        .unwrap_or("<fill in goal>");
    let scope = config
        .map(|config| config.scope.join(", "))
        .filter(|scope| !scope.trim().is_empty())
        .unwrap_or_else(|| "<fill in scope>".to_string());
    let metric = config
        .map(|config| config.metric.as_str())
        .filter(|metric| !metric.trim().is_empty())
        .unwrap_or("metric");
    let verify = config
        .map(|config| config.verify.as_str())
        .filter(|verify| !verify.trim().is_empty())
        .unwrap_or("<fill in verify command>");
    let guard = config
        .and_then(|config| config.guard.as_deref())
        .filter(|guard| !guard.trim().is_empty())
        .unwrap_or("skip");
    let direction = state.direction.as_str();
    let mut content = String::new();
    writeln!(
        content,
        "# Parallel Worker {worker_id}\n\n\
You are a parallel experiment worker for Autoresearch.\n\n\
Goal: {goal}\n\
Scope: {scope}\n\
Worker branch: {branch}\n\
Metric: {metric}\n\
Metric direction: {direction}\n\
Current retained metric: {}\n\
Verify: {verify}\n\
Guard: {guard}\n\n\
Instructions:\n\
1. Apply exactly one focused hypothesis within scope.\n\
2. Create a scoped trial commit in this worktree.\n\
3. Run the verify command and record the metric.\n\
4. Run the guard command when it is not `skip`.\n\
5. Fill this worker's result in the shared parallel batch JSON.\n\n\
Do NOT modify files outside scope. Do NOT run multiple changes.\n\
Do NOT ask questions or interact with the user.",
        state.current_metric
    )?;
    std::fs::write(&prompt_path, content)
        .with_context(|| format!("failed to write {}", prompt_path.display()))?;
    Ok(prompt_path)
}

fn default_results_tsv(cwd: &Path) -> Option<PathBuf> {
    let workspace = resolve_results_workspace(Some(cwd.to_path_buf()));
    let canonical = workspace.join("autoresearch-results/results.tsv");
    if canonical.exists() {
        return Some(canonical);
    }

    discover_results_tsv(cwd).or_else(|| {
        if workspace != cwd {
            discover_results_tsv(&workspace)
        } else {
            None
        }
    })
}

fn discover_results_tsv(root: &Path) -> Option<PathBuf> {
    let mut candidates = BTreeSet::new();
    collect_results_tsvs_in_dir(root, &mut candidates);
    collect_results_tsvs_in_dir(&root.join("autoresearch-results"), &mut candidates);

    if let Ok(entries) = std::fs::read_dir(root.join("autoresearch")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
                collect_results_tsvs_in_dir(&path, &mut candidates);
            }
        }
    }

    candidates.into_iter().max_by(|left, right| {
        results_tsv_modified(left)
            .cmp(&results_tsv_modified(right))
            .then_with(|| left.cmp(right))
    })
}

fn is_valid_handoff_status(value: &str) -> bool {
    matches!(
        value,
        "COMPLETE"
            | "GOAL_MET"
            | "BOUNDED"
            | "BLOCKED"
            | "ERROR"
            | "USER_INTERRUPT"
            | "CONVERGED"
            | "SATURATED"
            | "DRY_RUN"
            | "ROLLBACK"
    )
}

fn is_valid_handoff_source(value: &str) -> bool {
    matches!(
        value,
        "loop"
            | "autoresearch"
            | "plan"
            | "debug"
            | "fix"
            | "security"
            | "scenario"
            | "predict"
            | "learn"
            | "reason"
            | "probe"
            | "improve"
            | "ship"
            | "evals"
            | "exec"
    )
}

fn parse_handoff_chain_targets(value: Option<&str>) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };

    let targets: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if targets.is_empty() {
        anyhow::bail!("handoff chain must include at least one target");
    }

    for target in &targets {
        if !is_valid_handoff_source(target) {
            anyhow::bail!("invalid handoff chain target {target:?}");
        }
    }

    Ok(targets)
}

fn should_continue_handoff_chain(status: &str) -> bool {
    matches!(
        status,
        "COMPLETE" | "GOAL_MET" | "BOUNDED" | "CONVERGED" | "SATURATED" | "DRY_RUN"
    )
}

fn collect_results_tsvs_in_dir(dir: &Path, candidates: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|file_type| file_type.is_file())
            && is_results_tsv_name(&path)
        {
            candidates.insert(path);
        }
    }
}

fn is_results_tsv_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "results.tsv" || name.ends_with("-results.tsv"))
}

fn results_tsv_modified(path: &Path) -> std::time::SystemTime {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
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

fn parse_format(s: &str) -> Result<VerifyFormat> {
    match s {
        "scalar" => Ok(VerifyFormat::Scalar),
        "metrics_json" => Ok(VerifyFormat::MetricsJson),
        _ => anyhow::bail!("Unknown verify format: {s}. Use 'scalar' or 'metrics_json'."),
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
        "keep (reworked)" => Ok(IterationStatus::KeepReworked),
        "discard" => Ok(IterationStatus::Discard),
        "crash" => Ok(IterationStatus::Crash),
        "no-op" => Ok(IterationStatus::NoOp),
        "blocked" => Ok(IterationStatus::Blocked),
        "hook-blocked" => Ok(IterationStatus::HookBlocked),
        "metric-error" => Ok(IterationStatus::MetricError),
        "pivot" => Ok(IterationStatus::Pivot),
        "refine" => Ok(IterationStatus::Refine),
        "search" => Ok(IterationStatus::Search),
        "drift" => Ok(IterationStatus::Drift),
        _ => anyhow::bail!("Unknown status: {s}"),
    }
}
