use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use rust_decimal::prelude::ToPrimitive;
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
use autoresearch::modes::debug::DebugPhase;
use autoresearch::modes::evals::{parse_results_tsv, ParsedRow};
use autoresearch::modes::fix::ErrorCategory;
use autoresearch::modes::learn::LearnSubMode;
use autoresearch::modes::plan::{scan_repo_files, suggest_metrics, PATTERN_INDICATORS};
use autoresearch::modes::predict::Persona;
use autoresearch::modes::probe::ProbePersona;
use autoresearch::modes::reason::{ReasonDomain, ReasoningMode};
use autoresearch::modes::scenario::{Dimension, ScenarioFormat};
use autoresearch::modes::security::{OwaspCategory, Severity, StrideCategory};
use autoresearch::modes::ship::ShipPhase;

const RUNTIME_HARD_INVARIANTS_DOC: &str = include_str!("../references/runtime-hard-invariants.md");
const CORE_PRINCIPLES_DOC: &str = include_str!("../references/core-principles.md");
const LOOP_WORKFLOW_DOC: &str = include_str!("../references/loop-workflow.md");

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
        /// Number of times to run scalar verification before aggregating
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        /// Aggregate for repeated scalar verification: median, mean, min, max, or last
        #[arg(long, default_value = "median")]
        aggregate: String,
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
        /// Path to results.tsv (alias for the positional path)
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
        /// Include explicit go/no-go decision and next-step guidance
        #[arg(long)]
        recommend: bool,
        /// Consecutive non-keep iterations that define a plateau
        #[arg(long, default_value_t = 5)]
        plateau_window: u32,
        /// Chain evals output to downstream command(s)
        #[arg(long)]
        chain: Option<String>,
        /// Compare against another results TSV
        #[arg(long, value_name = "PATH")]
        compare: Option<PathBuf>,
        /// Target metric threshold used to report goal achievement
        #[arg(long, value_name = "NUMBER", allow_hyphen_values = true)]
        target: Option<String>,
        /// Output format: text, json, or md
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Run evals only when the active run reaches a checkpoint interval
    Checkpoint {
        /// Checkpoint interval. Defaults to floor(iterations / 3), minimum 1, or 10 for unbounded runs.
        #[arg(long)]
        interval: Option<u32>,
        /// Output format passed to evals when checkpoint is due: text, json, or md
        #[arg(long, default_value = "text")]
        format: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
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

    /// Probe local resources and toolchains for run planning
    Env {
        /// Output format: json or text
        #[arg(long, default_value = "json")]
        format: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Check whether a protocol re-anchor is due and print reload references
    Reanchor {
        /// Output format: json or text
        #[arg(long, default_value = "json")]
        format: String,
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

    /// Estimate token/API spend for the active run
    Cost {
        /// Direct estimated USD cost per completed iteration
        #[arg(long)]
        per_iteration_usd: Option<String>,
        /// Estimated input tokens consumed per iteration
        #[arg(long)]
        input_tokens_per_iteration: Option<u64>,
        /// Estimated output tokens consumed per iteration
        #[arg(long)]
        output_tokens_per_iteration: Option<u64>,
        /// Input token price in USD per 1M tokens
        #[arg(long)]
        input_usd_per_million: Option<String>,
        /// Output token price in USD per 1M tokens
        #[arg(long)]
        output_usd_per_million: Option<String>,
        /// Output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Show a live terminal dashboard for the active run
    Dashboard {
        /// Number of recent result rows to show
        #[arg(long, default_value_t = 8)]
        lines: usize,
        /// Render one snapshot and exit
        #[arg(long)]
        once: bool,
        /// Refresh interval while following
        #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(1..))]
        interval_ms: u64,
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

    /// Suggest scope, metric, verify, guard, and iterations from a goal
    Plan {
        /// Goal description to turn into a launch-ready config
        #[arg(long)]
        goal: Option<String>,
        /// Output format: json or text
        #[arg(long, default_value = "json")]
        format: String,
        /// Chain into debug mode after writing the derived config
        #[arg(long)]
        debug: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate a PRD markdown artifact for a selected improvement
    Prd {
        /// Improvement title
        #[arg(long)]
        title: String,
        /// Problem statement tied to the target user or ICP
        #[arg(long)]
        problem: String,
        /// Ideal customer profile or target user
        #[arg(long)]
        icp: Option<String>,
        /// Proposed solution or mechanism
        #[arg(long)]
        solution: Option<String>,
        /// Success metric to optimize
        #[arg(long)]
        metric: Option<String>,
        /// Relevant implementation scope. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Output path. Relative paths resolve from the workspace root.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate improve-mode research artifacts for a product area
    Improve {
        /// Product, feature, or workflow to improve
        #[arg(long)]
        goal: String,
        /// Ideal customer profile or target user
        #[arg(long)]
        icp: Option<String>,
        /// Relevant implementation scope. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Research depth: shallow, standard, or deep
        #[arg(long, default_value = "standard")]
        depth: String,
        /// Override the depth-derived research iteration budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Number of seed ideas per research category
        #[arg(long)]
        seeds: Option<u8>,
        /// Enable discovery research metadata
        #[arg(long)]
        discover: bool,
        /// Disable discovery research metadata
        #[arg(long)]
        no_discover: bool,
        /// Record eval checkpoint metadata
        #[arg(long)]
        evals: bool,
        /// Eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Chain into learn mode after writing improvement research
        #[arg(long)]
        learn: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Output directory. Relative paths resolve from the workspace root.
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate a STRIDE + OWASP security audit artifact bundle
    Security {
        /// File globs to audit. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Optional focus area such as auth, API, data handling, or CI
        #[arg(long)]
        focus: Option<String>,
        /// Audit depth: quick, standard, or deep
        #[arg(long, default_value = "standard")]
        depth: String,
        /// Override the depth-derived audit iteration budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Delta mode: record that only files changed since the last audit should be audited
        #[arg(long)]
        diff: bool,
        /// Record a downstream fix handoff for confirmed Critical/High findings
        #[arg(long)]
        fix: bool,
        /// CI gate threshold: critical, high, medium, low, or info
        #[arg(long)]
        fail_on: Option<String>,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Output directory. Relative paths resolve from the workspace root.
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate an 8-phase ship checklist artifact bundle
    Ship {
        /// What is being shipped
        #[arg(long)]
        target: String,
        /// Shipment type: code-pr, code-release, deployment, content, docs, package, config
        #[arg(long = "type", default_value = "code-pr")]
        ship_type: String,
        /// Run all steps but do not perform external ship actions
        #[arg(long)]
        dry_run: bool,
        /// Auto-approve only if no blockers are found; recorded as metadata only
        #[arg(long)]
        auto: bool,
        /// Skip non-critical checklist items; blockers remain enforced
        #[arg(long)]
        force: bool,
        /// Record rollback intent instead of normal ship intent
        #[arg(long)]
        rollback: bool,
        /// Post-ship monitoring window in minutes
        #[arg(long)]
        monitor: Option<u32>,
        /// Generate only checklist artifacts
        #[arg(long)]
        checklist_only: bool,
        /// Chain into learn mode after writing ship artifacts
        #[arg(long)]
        learn: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Output directory. Relative paths resolve from the workspace root.
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate a hypothesis-driven debug investigation artifact bundle
    Debug {
        /// Symptom, failure, or behavior to investigate
        #[arg(long)]
        symptom: String,
        /// File globs to investigate. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Investigation technique to seed
        #[arg(long, default_value = "trace")]
        technique: String,
        /// Investigation depth: quick, standard, or deep
        #[arg(long, default_value = "standard")]
        depth: String,
        /// Override the depth-derived investigation iteration budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Severity filter: critical, high, medium, low, or info
        #[arg(long)]
        severity: Option<String>,
        /// Chain into fix mode after writing debug findings
        #[arg(long)]
        fix: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Output directory. Relative paths resolve from the workspace root.
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate an error-repair plan artifact bundle
    Fix {
        /// Verify/target command that counts remaining errors
        #[arg(long, required_unless_present = "from_debug")]
        target: Option<String>,
        /// File globs that may be edited. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Import scope and context from the latest debug handoff
        #[arg(long)]
        from_debug: bool,
        /// Optional guard command that must remain passing
        #[arg(long)]
        guard: Option<String>,
        /// Error category to prioritize: crash, test, type, lint, build, warning
        #[arg(long)]
        category: Option<String>,
        /// Error-repair iteration budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Chain into learn mode after writing the repair plan
        #[arg(long)]
        learn: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Output directory. Defaults under autoresearch-results/fix/.
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate a 12-dimension scenario exploration artifact
    Scenario {
        /// Feature, workflow, or system to explore
        #[arg(long, visible_alias = "scenario")]
        target: String,
        /// Domain: general, web, mobile, API, CLI, data-pipeline, or infrastructure
        #[arg(long, default_value = "general")]
        domain: String,
        /// Scenario format: use-cases, user-stories, test-scenarios, or threat-scenarios
        #[arg(long, default_value = "test-scenarios")]
        format: String,
        /// Focus area: edge-cases, failures, security, or scale
        #[arg(long, default_value = "edge-cases")]
        focus: String,
        /// Relevant implementation scope. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Exploration depth: shallow, standard, or deep
        #[arg(long, default_value = "standard")]
        depth: String,
        /// Override the depth-derived exploration iteration budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Record eval checkpoint metadata
        #[arg(long)]
        evals: bool,
        /// Eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Chain into debug mode after writing scenario findings
        #[arg(long)]
        debug: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Output path. Relative paths resolve from the workspace root.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate a five-persona prediction review artifact
    Predict {
        /// Proposal, change, or design decision to analyze
        #[arg(long, visible_alias = "goal")]
        proposal: String,
        /// Relevant implementation scope. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Review depth: shallow, standard, or deep
        #[arg(long, default_value = "standard")]
        depth: String,
        /// Use an adversarial review profile
        #[arg(long)]
        adversarial: bool,
        /// Override requested persona count, 3-8
        #[arg(long)]
        personas: Option<u8>,
        /// Override debate rounds, 1-3
        #[arg(long)]
        rounds: Option<u8>,
        /// Maximum findings budget
        #[arg(long)]
        budget: Option<u32>,
        /// CI gate threshold: critical, high, medium, low, or info
        #[arg(long)]
        fail_on: Option<String>,
        /// Only analyze changed files
        #[arg(long)]
        incremental: bool,
        /// Chain into debug mode after writing the prediction review
        #[arg(long)]
        debug: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Output path. Relative paths resolve from the workspace root.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate an adversarial reasoning debate artifact
    Reason {
        /// Question, decision, or problem to debate
        #[arg(long, visible_alias = "task")]
        question: String,
        /// Reasoning mode: convergent, creative, or debate
        #[arg(long, default_value = "debate")]
        mode: String,
        /// Domain: software, product, business, security, research, or content
        #[arg(long, default_value = "software")]
        domain: String,
        /// Debate/refinement iteration budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Relevant implementation scope. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Blind judge count
        #[arg(long)]
        judges: Option<u8>,
        /// Stop after the incumbent wins this many consecutive rounds
        #[arg(long)]
        convergence: Option<u8>,
        /// Comma-separated custom judge persona names
        #[arg(long)]
        judge_personas: Option<String>,
        /// Skip synthesis and run pure debate
        #[arg(long)]
        no_synthesis: bool,
        /// Generation temperature hint
        #[arg(long)]
        temperature: Option<String>,
        /// Chain into predict mode after writing the debate artifact
        #[arg(long)]
        predict: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Output path. Relative paths resolve from the workspace root.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate an eight-persona requirement probe artifact
    Probe {
        /// Requirement, feature, or workflow to interrogate
        #[arg(long, visible_alias = "topic")]
        subject: String,
        /// Relevant implementation scope. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Answer mode: interactive or autonomous
        #[arg(long, default_value = "interactive")]
        mode: String,
        /// Probe depth: shallow, standard, or deep
        #[arg(long, default_value = "standard")]
        depth: String,
        /// Override the depth-derived interrogation round budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Number of active personas, 3-8
        #[arg(long)]
        personas: Option<u8>,
        /// Put hostile personas first
        #[arg(long)]
        adversarial: bool,
        /// Net-new constraint threshold for saturation
        #[arg(long)]
        saturation_threshold: Option<u8>,
        /// Chain into plan mode after writing probe constraints
        #[arg(long)]
        plan: bool,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Output path. Relative paths resolve from the workspace root.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
    },

    /// Generate learn-mode documentation summary artifacts
    Learn {
        /// Learn mode: init, update, check, or summarize
        #[arg(long, default_value = "summarize")]
        mode: String,
        /// File globs to document or summarize. Repeatable.
        #[arg(long)]
        scope: Vec<String>,
        /// Documentation depth: overview, standard, or comprehensive
        #[arg(long, default_value = "standard")]
        depth: String,
        /// Override the depth-derived documentation iteration budget
        #[arg(long)]
        iterations: Option<u32>,
        /// Specific file to document. Repeatable.
        #[arg(long)]
        file: Vec<String>,
        /// Force a fresh codebase scout
        #[arg(long)]
        scan: bool,
        /// Comma-separated focus topics such as architecture, API, database, or testing
        #[arg(long)]
        topics: Option<String>,
        /// Validate only; do not auto-fix documentation issues
        #[arg(long)]
        no_fix: bool,
        /// Documentation format preference: markdown, json, or rst
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Comma-separated downstream command targets to record in handoff.json
        #[arg(long)]
        chain: Option<String>,
        /// Propagate eval checkpoints to downstream chain targets
        #[arg(long)]
        evals: bool,
        /// Propagated eval checkpoint interval
        #[arg(long)]
        evals_interval: Option<u32>,
        /// Output directory. Relative paths resolve from the workspace root.
        #[arg(long)]
        output_dir: Option<PathBuf>,
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

    /// Execute commands across primary and companion repo targets
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommands,
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
        /// Worker hypothesis text, repeated once per worker
        #[arg(long)]
        hypothesis: Vec<String>,
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
    /// Prepare a two-arm A/B experiment using isolated worker worktrees
    Compare {
        /// Hypothesis for arm A
        #[arg(long)]
        a: String,
        /// Hypothesis for arm B
        #[arg(long)]
        b: String,
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
        #[arg(long, default_value = "autoresearch/ab")]
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
enum WorkspaceCommands {
    /// Run one command in each repo target from the active run context
    Exec {
        /// Command to run in each target repo
        #[arg(long)]
        command: String,
        /// Reset attempted repo targets back to their original HEAD on failure
        #[arg(long)]
        rollback_on_failure: bool,
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
            repeat,
            aggregate,
            cwd,
        } => cmd_verify(&command, &format, key.as_deref(), repeat, &aggregate, cwd),

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

        Commands::Evals {
            path,
            file,
            recommend,
            plateau_window,
            chain,
            compare,
            target,
            format,
        } => {
            if path.is_some() && file.is_some() {
                anyhow::bail!("evals accepts either a positional path or --file, not both");
            }
            cmd_evals(
                path.or(file),
                &format,
                recommend,
                plateau_window,
                chain.as_deref(),
                compare.as_deref(),
                target.as_deref(),
            )
        }

        Commands::Checkpoint {
            interval,
            format,
            cwd,
        } => cmd_checkpoint(cwd, interval, &format),

        Commands::Status { summary, cwd } => cmd_status(cwd, summary),

        Commands::Health {
            verify,
            strict,
            min_free_mb,
            cwd,
        } => cmd_health(verify.as_deref(), strict, min_free_mb, cwd),

        Commands::Env { format, cwd } => cmd_env(cwd, &format),

        Commands::Reanchor { format, cwd } => cmd_reanchor(cwd, &format),

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

        Commands::Cost {
            per_iteration_usd,
            input_tokens_per_iteration,
            output_tokens_per_iteration,
            input_usd_per_million,
            output_usd_per_million,
            format,
            cwd,
        } => cmd_cost(
            cwd,
            per_iteration_usd.as_deref(),
            input_tokens_per_iteration,
            output_tokens_per_iteration,
            input_usd_per_million.as_deref(),
            output_usd_per_million.as_deref(),
            &format,
        ),

        Commands::Dashboard {
            lines,
            once,
            interval_ms,
            cwd,
        } => cmd_dashboard(cwd, lines, once, interval_ms),

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

        Commands::Plan {
            goal,
            format,
            debug,
            chain,
            cwd,
        } => cmd_plan(goal, &format, debug, chain, cwd),

        Commands::Prd {
            title,
            problem,
            icp,
            solution,
            metric,
            scope,
            output,
            cwd,
        } => cmd_prd(&title, &problem, icp, solution, metric, scope, output, cwd),

        Commands::Improve {
            goal,
            icp,
            scope,
            depth,
            iterations,
            seeds,
            discover,
            no_discover,
            evals,
            evals_interval,
            learn,
            chain,
            output_dir,
            cwd,
        } => cmd_improve(
            &goal,
            icp,
            scope,
            &depth,
            iterations,
            seeds,
            discover,
            no_discover,
            evals,
            evals_interval,
            learn,
            chain,
            output_dir,
            cwd,
        ),

        Commands::Security {
            scope,
            focus,
            depth,
            iterations,
            diff,
            fix,
            fail_on,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        } => cmd_security(
            scope,
            focus,
            &depth,
            iterations,
            diff,
            fix,
            fail_on,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        ),

        Commands::Ship {
            target,
            ship_type,
            dry_run,
            auto,
            force,
            rollback,
            monitor,
            checklist_only,
            learn,
            chain,
            output_dir,
            cwd,
        } => cmd_ship(
            &target,
            &ship_type,
            dry_run,
            auto,
            force,
            rollback,
            monitor,
            checklist_only,
            learn,
            chain,
            output_dir,
            cwd,
        ),

        Commands::Debug {
            symptom,
            scope,
            technique,
            depth,
            iterations,
            severity,
            fix,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        } => cmd_debug(
            &symptom,
            scope,
            &technique,
            &depth,
            iterations,
            severity,
            fix,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        ),

        Commands::Fix {
            target,
            scope,
            from_debug,
            guard,
            category,
            iterations,
            learn,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        } => cmd_fix(
            target,
            scope,
            from_debug,
            guard,
            category,
            iterations,
            learn,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        ),

        Commands::Scenario {
            target,
            domain,
            format,
            focus,
            scope,
            depth,
            iterations,
            evals,
            evals_interval,
            debug,
            chain,
            output,
            cwd,
        } => cmd_scenario(
            &target,
            &domain,
            &format,
            &focus,
            scope,
            &depth,
            iterations,
            evals,
            evals_interval,
            debug,
            chain,
            output,
            cwd,
        ),

        Commands::Predict {
            proposal,
            scope,
            depth,
            adversarial,
            personas,
            rounds,
            budget,
            fail_on,
            incremental,
            debug,
            chain,
            evals,
            evals_interval,
            output,
            cwd,
        } => cmd_predict(
            &proposal,
            scope,
            depth,
            adversarial,
            personas,
            rounds,
            budget,
            fail_on,
            incremental,
            debug,
            chain,
            evals,
            evals_interval,
            output,
            cwd,
        ),

        Commands::Reason {
            question,
            mode,
            domain,
            iterations,
            scope,
            judges,
            convergence,
            judge_personas,
            no_synthesis,
            temperature,
            predict,
            chain,
            evals,
            evals_interval,
            output,
            cwd,
        } => cmd_reason(
            &question,
            &mode,
            &domain,
            iterations,
            scope,
            judges,
            convergence,
            judge_personas,
            no_synthesis,
            temperature,
            predict,
            chain,
            evals,
            evals_interval,
            output,
            cwd,
        ),

        Commands::Probe {
            subject,
            scope,
            mode,
            depth,
            iterations,
            personas,
            adversarial,
            saturation_threshold,
            plan,
            chain,
            evals,
            evals_interval,
            output,
            cwd,
        } => cmd_probe(
            &subject,
            scope,
            mode,
            depth,
            iterations,
            personas,
            adversarial,
            saturation_threshold,
            plan,
            chain,
            evals,
            evals_interval,
            output,
            cwd,
        ),

        Commands::Learn {
            mode,
            scope,
            depth,
            iterations,
            file,
            scan,
            topics,
            no_fix,
            format,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        } => cmd_learn(
            &mode,
            scope,
            &depth,
            iterations,
            file,
            scan,
            topics,
            no_fix,
            &format,
            chain,
            evals,
            evals_interval,
            output_dir,
            cwd,
        ),

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
        Commands::Workspace { command } => match command {
            WorkspaceCommands::Exec {
                command,
                rollback_on_failure,
                cwd,
            } => cmd_workspace_exec(&command, rollback_on_failure, cwd),
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

const PLAN_SCAN_PATTERNS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "pyproject.toml",
    "pytest.ini",
    "go.mod",
    "Makefile",
    "jest.config*",
    "vitest.config*",
    ".eslintrc*",
    "vite.config*",
    "webpack.config*",
];

fn has_detected_file(files: &[String], name: &str) -> bool {
    files.iter().any(|file| file == name)
}

fn goal_contains(goal: &str, terms: &[&str]) -> bool {
    let goal = goal.to_lowercase();
    terms.iter().any(|term| goal.contains(term))
}

fn plan_recommendation(goal: &str, detected_files: &[String]) -> serde_json::Value {
    let has_cargo = has_detected_file(detected_files, "Cargo.toml");
    let has_package = has_detected_file(detected_files, "package.json");
    let has_ts = has_detected_file(detected_files, "tsconfig.json");
    let has_pytest = has_detected_file(detected_files, "pytest.ini")
        || has_detected_file(detected_files, "pyproject.toml");
    let has_go = has_detected_file(detected_files, "go.mod");

    let (metric, direction, verify, guard, scope, iterations, confidence, rationale) =
        if has_ts && goal_contains(goal, &["any", "types", "typescript", "tsc"]) {
            (
                "any_count",
                "lower",
                "rg -n \"\\bany\\b\" src tests --glob '*.ts' --glob '*.tsx' 2>/dev/null | wc -l",
                Some("npx tsc --noEmit"),
                vec![
                    "src/**/*.ts",
                    "src/**/*.tsx",
                    "tests/**/*.ts",
                    "tests/**/*.tsx",
                ],
                20,
                "high",
                "TypeScript project detected and the goal is type-related",
            )
        } else if has_ts {
            (
                "type_errors",
                "lower",
                "npx tsc --noEmit 2>&1 | grep -c 'error TS' || echo 0",
                if has_package { Some("npm test") } else { None },
                vec![
                    "src/**/*.ts",
                    "src/**/*.tsx",
                    "tests/**/*.ts",
                    "tests/**/*.tsx",
                ],
                20,
                "medium",
                "TypeScript project detected",
            )
        } else if has_cargo {
            (
                "failing_tests",
                "lower",
                "cargo test 2>&1 | grep -cE 'FAILED|panicked|error:' || echo 0",
                Some("cargo fmt -- --check"),
                vec!["src/**/*.rs", "tests/**/*.rs"],
                20,
                "high",
                "Rust project detected",
            )
        } else if has_pytest {
            (
                "failing_tests",
                "lower",
                "pytest -q 2>&1 | grep -cE 'FAILED|ERROR|failed' || echo 0",
                Some("python -m compileall ."),
                vec!["src/**/*.py", "tests/**/*.py"],
                20,
                "high",
                "Python test configuration detected",
            )
        } else if has_go {
            (
                "failing_tests",
                "lower",
                "go test ./... 2>&1 | grep -cE 'FAIL|panic' || echo 0",
                None,
                vec!["**/*.go"],
                20,
                "high",
                "Go module detected",
            )
        } else if has_package && goal_contains(goal, &["coverage", "test"]) {
            (
                "coverage",
                "higher",
                "npm test -- --coverage | tail -1",
                Some("npm test"),
                vec!["src/**/*", "tests/**/*"],
                15,
                "medium",
                "JavaScript package detected and the goal mentions tests or coverage",
            )
        } else {
            (
                "manual_metric",
                "higher",
                "printf '0\\n'",
                None,
                vec!["**/*"],
                10,
                "low",
                "No strong tooling pattern detected; replace the placeholder verify command",
            )
        };

    if let Err(err) = verify::screen_command(verify) {
        return serde_json::json!({
            "status": "unsafe",
            "reason": err.to_string(),
            "metric": metric,
            "verify": verify,
        });
    }
    if let Some(guard) = guard {
        if let Err(err) = verify::screen_command(guard) {
            return serde_json::json!({
                "status": "unsafe",
                "reason": err.to_string(),
                "metric": metric,
                "verify": verify,
                "guard": guard,
            });
        }
    }

    serde_json::json!({
        "status": if confidence == "low" { "needs_confirmation" } else { "ready" },
        "goal": goal,
        "scope": scope,
        "metric": metric,
        "direction": direction,
        "verify": verify,
        "guard": guard,
        "iterations": iterations,
        "confidence": confidence,
        "rationale": rationale,
    })
}

fn render_plan_text(plan: &serde_json::Value) -> String {
    let recommended = &plan["recommended"];
    let mut out = String::new();
    writeln!(out, "--- Autoresearch Plan ---").unwrap();
    writeln!(out, "Goal: {}", plan["goal"].as_str().unwrap_or("")).unwrap();
    writeln!(
        out,
        "Metric: {} ({})",
        recommended["metric"].as_str().unwrap_or(""),
        recommended["direction"].as_str().unwrap_or("")
    )
    .unwrap();
    writeln!(
        out,
        "Verify: {}",
        recommended["verify"].as_str().unwrap_or("")
    )
    .unwrap();
    if let Some(guard) = recommended["guard"].as_str() {
        writeln!(out, "Guard: {guard}").unwrap();
    }
    writeln!(
        out,
        "Iterations: {}",
        recommended["iterations"].as_u64().unwrap_or(10)
    )
    .unwrap();
    writeln!(out, "Scope:").unwrap();
    if let Some(scope) = recommended["scope"].as_array() {
        for item in scope {
            writeln!(out, "  - {}", item.as_str().unwrap_or("")).unwrap();
        }
    }
    writeln!(out, "Detected files:").unwrap();
    if let Some(files) = plan["detected_files"].as_array() {
        if files.is_empty() {
            writeln!(out, "  - none").unwrap();
        }
        for file in files {
            writeln!(out, "  - {}", file.as_str().unwrap_or("")).unwrap();
        }
    }
    out
}

fn cmd_plan(
    goal: Option<String>,
    format: &str,
    debug: bool,
    chain: Option<String>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let goal = goal.unwrap_or_default();
    let forced_targets = if debug { &["debug"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let detected_files = scan_repo_files(&workspace, PLAN_SCAN_PATTERNS);
    let indicator_patterns = PATTERN_INDICATORS
        .iter()
        .map(|(pattern, _)| *pattern)
        .collect::<Vec<_>>();
    let indicator_files = scan_repo_files(&workspace, &indicator_patterns);
    let metric_hints = suggest_metrics(&indicator_files);
    let metric_hints = metric_hints
        .iter()
        .map(|suggestion| {
            serde_json::json!({
                "name": suggestion.name,
                "metric": suggestion.metric,
                "direction": suggestion.direction,
                "verify": suggestion.verify_command,
                "rationale": suggestion.rationale,
            })
        })
        .collect::<Vec<_>>();
    let recommended = plan_recommendation(&goal, &detected_files);
    let handoff_path = if !chain_targets.is_empty() {
        let handoff_path =
            resolve_workspace_path(&workspace, default_artifact_path("plan", "handoff.json"));
        if let Some(parent) = handoff_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let next_target = next_chain_target_value(&chain_targets);
        let handoff = serde_json::json!({
            "version": "2.1.0",
            "source": "plan",
            "source_command": "plan",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "COMPLETE",
            "handoff_path": handoff_path.display().to_string(),
            "findings": [],
            "config": {
                "goal": goal.clone(),
                "recommended": recommended.clone(),
                "metric_hints": metric_hints.clone(),
                "detected_files": detected_files.clone(),
            },
            "chain": chain_targets.clone(),
            "next_target": next_target,
            "chain_continue": should_continue_handoff_chain("COMPLETE"),
        });
        write_json_file(&handoff_path, &handoff)?;
        Some(handoff_path)
    } else {
        None
    };
    let out = serde_json::json!({
        "goal": goal,
        "workspace": workspace.display().to_string(),
        "detected_files": detected_files,
        "recommended": recommended,
        "metric_hints": metric_hints,
        "handoff_path": handoff_path.as_ref().map(|path| path.display().to_string()),
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&out)?),
        "text" => print!("{}", render_plan_text(&out)),
        other => anyhow::bail!("Invalid plan format {other:?}; use json or text"),
    }
    Ok(())
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "improvement".to_string()
    } else {
        slug
    }
}

fn default_artifact_path(mode: &str, leaf: impl Into<PathBuf>) -> PathBuf {
    PathBuf::from("autoresearch-results")
        .join(mode)
        .join(leaf.into())
}

fn render_prd_markdown(
    title: &str,
    problem: &str,
    icp: Option<&str>,
    solution: Option<&str>,
    metric: Option<&str>,
    scope: &[String],
) -> String {
    let icp = icp
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("DECISION NEEDED: define the target user or ICP.");
    let solution = solution
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("DECISION NEEDED: choose the solution mechanism.");
    let metric = metric
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("DECISION NEEDED: choose a mechanical success metric.");
    let scope_items = if scope.is_empty() {
        vec!["DECISION NEEDED: identify implementation scope.".to_string()]
    } else {
        scope.to_vec()
    };
    let scope_lines = scope_items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");
    let scope_config = scope_items.join(", ");

    format!(
        "# {title}\n\n\
         > Auto-generated from autoresearch improve inputs. DECISION NEEDED items and low-confidence sections require human judgment.\n\n\
         ## Problem Statement\n\n{problem}\n\n\
         ## Target ICP\n\n{icp}\n\n\
         ## Proposed Solution\n\n{solution}\n\n\
         ## User Stories\n\n\
         - As the target ICP, I want this problem reduced so my core workflow is faster or more reliable.\n\
         - As an evaluator, I want a measurable success signal before this ships.\n\n\
         ## Requirements\n\n\
         ### Must Have\n\n\
         - Address the stated problem for the target ICP.\n\
         - Preserve existing verified behavior unless explicitly changed.\n\
         - Expose a mechanical metric for closeout.\n\n\
         ### Non-Goals\n\n\
         - Broad redesigns outside the listed scope.\n\
         - Shipping external actions without explicit approval.\n\n\
         ## Acceptance Criteria\n\n\
         - `{metric}` can be measured by a deterministic verify command.\n\
         - DECISION NEEDED: define the target threshold for `{metric}`.\n\
         - Guard checks pass after implementation.\n\n\
         ## Technical Approach\n\n\
         Scope:\n\n{scope_lines}\n\n\
         Suggested starting points:\n\n\
         - Inspect current flows and tests in the listed scope.\n\
         - Add the smallest behavior or instrumentation needed to move `{metric}`.\n\
         - Keep the first implementation reversible and verify-driven.\n\n\
         ## Risks And Mitigations\n\n\
         - Risk: optimizing the metric without improving the ICP workflow. Mitigation: keep the ICP gate explicit in review.\n\
         - Risk: hidden regressions. Mitigation: require guard checks and focused tests.\n\
         - Risk: uncertain tradeoff. Mitigation: mark unresolved decisions before implementation.\n\n\
         ## Success Metrics\n\n\
         - Primary: `{metric}`\n\
         - Secondary: guard pass rate and absence of new blocked iterations\n\n\
         ## Ready-To-Run Autoresearch Config\n\n\
         ```text\n\
         $autoresearch\n\
         Goal: {title} - {problem}\n\
         Scope: {scope_config}\n\
         Metric: {metric}\n\
         Direction: DECISION NEEDED\n\
         Verify: DECISION NEEDED\n\
         Guard: DECISION NEEDED\n\
         Iterations: 20\n\
         ```\n\n\
         ## Open Questions\n\n\
         - DECISION NEEDED: what exact threshold makes this shippable?\n\
         - DECISION NEEDED: which guard command best protects adjacent behavior?\n"
    )
}

#[allow(clippy::too_many_arguments)]
fn cmd_prd(
    title: &str,
    problem: &str,
    icp: Option<String>,
    solution: Option<String>,
    metric: Option<String>,
    scope: Vec<String>,
    output: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let output = output
        .unwrap_or_else(|| default_artifact_path("improve", format!("prd-{}.md", slugify(title))));
    let output = resolve_workspace_path(&workspace, output);
    let markdown = render_prd_markdown(
        title,
        problem,
        icp.as_deref(),
        solution.as_deref(),
        metric.as_deref(),
        &scope,
    );
    write_text_file(&output, &markdown)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "path": output.display().to_string(),
            "title": title,
        })
    );
    Ok(())
}

fn improve_categories() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "ICP Challenges",
            "Pain points, unmet needs, and workflow friction for the target persona",
        ),
        (
            "Competitor Gaps",
            "Weaknesses, missing features, and technical differentiators",
        ),
        (
            "Market Trends",
            "Emerging expectations, timing signals, and standards shifts",
        ),
        (
            "UX Patterns",
            "Interaction improvements, onboarding, accessibility, and retention",
        ),
        (
            "Revenue Growth",
            "Monetization, expansion, acquisition, and packaging opportunities",
        ),
    ]
}

#[derive(Debug, Clone)]
struct ImproveProfile {
    depth: String,
    category_limit: usize,
    iteration_budget: u32,
    seeds_per_category: u8,
    discover: bool,
    evals: bool,
    evals_interval: Option<u32>,
}

fn parse_improve_depth(value: &str) -> Result<(&'static str, usize, u32)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "shallow" | "quick" => Ok(("shallow", 3, 10)),
        "standard" | "normal" => Ok(("standard", 5, 20)),
        "deep" | "comprehensive" => Ok(("deep", 5, 40)),
        other => anyhow::bail!("Invalid improve depth {other:?}; use shallow, standard, or deep"),
    }
}

fn resolve_improve_profile(
    depth: &str,
    iterations: Option<u32>,
    seeds: Option<u8>,
    discover: bool,
    no_discover: bool,
    evals: bool,
    evals_interval: Option<u32>,
) -> Result<ImproveProfile> {
    validate_chain_evals_flags("improve", evals, evals_interval)?;
    if discover && no_discover {
        anyhow::bail!("improve cannot use both --discover and --no-discover");
    }
    let seeds_per_category = seeds.unwrap_or(5);
    if !(1..=20).contains(&seeds_per_category) {
        anyhow::bail!("improve seeds must be between 1 and 20");
    }
    let (depth, category_limit, iteration_budget) = parse_improve_depth(depth)?;
    if iterations == Some(0) {
        anyhow::bail!("improve iterations must be greater than zero");
    }
    Ok(ImproveProfile {
        depth: depth.to_string(),
        category_limit,
        iteration_budget: iterations.unwrap_or(iteration_budget),
        seeds_per_category,
        discover: !no_discover,
        evals,
        evals_interval,
    })
}

fn improve_active_categories(profile: &ImproveProfile) -> &'static [(&'static str, &'static str)] {
    &improve_categories()[..profile.category_limit]
}

fn improve_seed_title(goal: &str, category: &str) -> String {
    match category {
        "ICP Challenges" => format!("Reduce the highest-friction step in {goal}"),
        "Competitor Gaps" => format!("Expose a differentiator competitors miss in {goal}"),
        "Market Trends" => format!("Align {goal} with a new buyer expectation"),
        "UX Patterns" => format!("Make the first successful {goal} path measurable"),
        "Revenue Growth" => format!("Tie {goal} to expansion or activation value"),
        _ => format!("Improve {goal}"),
    }
}

fn improve_seed_title_at(goal: &str, category: &str, seed_index: u8) -> String {
    let base = improve_seed_title(goal, category);
    match seed_index {
        1 => base,
        2 => format!("{base} with lower setup cost"),
        3 => format!("{base} with clearer measurement"),
        4 => format!("{base} for the highest-intent segment"),
        5 => format!("{base} with automated follow-up"),
        _ => format!("{base} variant {seed_index}"),
    }
}

fn render_improve_research_markdown(
    goal: &str,
    icp: &str,
    scope: &[String],
    profile: &ImproveProfile,
) -> String {
    let mut out = String::new();
    let scope_items = if scope.is_empty() {
        vec!["DECISION NEEDED: identify implementation scope.".to_string()]
    } else {
        scope.to_vec()
    };
    let scope_lines = scope_items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    writeln!(out, "# Improve Research Findings: {goal}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "> Auto-generated seed research artifacts. Add citations and confidence upgrades as external research is completed."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## ICP").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{icp}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Research Profile").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Depth: {}", profile.depth).unwrap();
    writeln!(
        out,
        "- Categories active: {} of {}",
        improve_active_categories(profile).len(),
        improve_categories().len()
    )
    .unwrap();
    writeln!(out, "- Iteration budget: {}", profile.iteration_budget).unwrap();
    writeln!(out, "- Seeds per category: {}", profile.seeds_per_category).unwrap();
    writeln!(out, "- Discovery enabled: {}", profile.discover).unwrap();
    writeln!(out, "- Evals enabled: {}", profile.evals).unwrap();
    writeln!(
        out,
        "- Evals interval: {}",
        profile
            .evals_interval
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Findings").unwrap();
    writeln!(out).unwrap();
    for (category, focus) in improve_active_categories(profile) {
        writeln!(out, "### {category}").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "- Research focus: {focus}").unwrap();
        for seed_index in 1..=profile.seeds_per_category {
            writeln!(
                out,
                "- Seed insight {seed_index}: {}",
                improve_seed_title_at(goal, category, seed_index)
            )
            .unwrap();
        }
        writeln!(
            out,
            "- Confidence: LOW until backed by code or web evidence"
        )
        .unwrap();
        writeln!(out, "- Classification: new").unwrap();
        writeln!(out).unwrap();
    }
    out
}

fn render_improve_plan_markdown(goal: &str, icp: &str, profile: &ImproveProfile) -> String {
    let mut out = String::new();
    writeln!(out, "# Improvement Plan: {goal}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "ICP: {icp}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "Depth: {} ({} categories, {} iteration budget)",
        profile.depth,
        improve_active_categories(profile).len(),
        profile.iteration_budget
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Tiered Ranking").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "| Tier | Improvement | Rationale | Confidence | Next Artifact |"
    )
    .unwrap();
    writeln!(out, "|---|---|---|---|---|").unwrap();
    for (index, (category, _focus)) in improve_active_categories(profile).iter().enumerate() {
        let tier = match index {
            0 | 3 => "Must-have",
            1 | 2 => "Nice-to-have",
            _ => "Moonshot",
        };
        for seed_index in 1..=profile.seeds_per_category {
            let title = improve_seed_title_at(goal, category, seed_index);
            writeln!(
                out,
                "| {tier} | {title} | Serves the stated ICP and maps to {category}. | LOW | `autoresearch prd --title \"{title}\" --problem \"DECISION NEEDED\"` |"
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(out, "## Selection Rule").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "Prioritize Must-have items that can be verified mechanically within the current scope."
    )
    .unwrap();
    writeln!(
        out,
        "DECISION NEEDED: choose which improvements become PRDs."
    )
    .unwrap();
    out
}

fn render_improve_summary_markdown(
    goal: &str,
    icp: &str,
    output_dir: &Path,
    profile: &ImproveProfile,
) -> String {
    format!(
        "# Improve Summary: {goal}\n\n\
         - ICP: {icp}\n\
         - Depth: {}\n\
         - Categories covered: {}\n\
         - Categories available: {}\n\
         - Seed insights: {}\n\
         - Seeds per category: {}\n\
         - Discovery enabled: {}\n\
         - Iteration budget: {}\n\
         - Evals enabled: {}\n\
         - Evals interval: {}\n\
         - Saturation status: not evaluated\n\
         - Output directory: {}\n\n\
         Next: add citations/confidence, select top improvements, then run `autoresearch prd` for selected items.\n",
        profile.depth,
        improve_active_categories(profile).len(),
        improve_categories().len(),
        improve_active_categories(profile).len() * usize::from(profile.seeds_per_category),
        profile.seeds_per_category,
        profile.discover,
        profile.iteration_budget,
        profile.evals,
        profile
            .evals_interval
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        output_dir.display()
    )
}

fn render_improve_results_tsv(goal: &str, profile: &ImproveProfile) -> String {
    let mut out = String::new();
    let timestamp = chrono::Utc::now().to_rfc3339();
    writeln!(out, "# metric_direction: higher_is_better").unwrap();
    writeln!(
        out,
        "iteration\ttimestamp\tcategory\tidea\ticp_pass\ttier\tscore\tdescription"
    )
    .unwrap();
    let mut row = 1usize;
    for (index, (category, focus)) in improve_active_categories(profile).iter().enumerate() {
        let tier = match index {
            0 | 3 => "must_have",
            1 | 2 => "nice_to_have",
            _ => "moonshot",
        };
        let score = match tier {
            "must_have" => 81,
            "nice_to_have" => 64,
            _ => 45,
        };
        for seed_index in 1..=profile.seeds_per_category {
            writeln!(
                out,
                "{}\t{}\t{}\t{}\ttrue\t{}\t{}\t{}",
                row,
                timestamp,
                category,
                improve_seed_title_at(goal, category, seed_index),
                tier,
                score,
                focus
            )
            .unwrap();
            row += 1;
        }
    }
    out
}

fn cmd_improve(
    goal: &str,
    icp: Option<String>,
    scope: Vec<String>,
    depth: &str,
    iterations: Option<u32>,
    seeds: Option<u8>,
    discover: bool,
    no_discover: bool,
    evals: bool,
    evals_interval: Option<u32>,
    learn: bool,
    chain: Option<String>,
    output_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let profile = resolve_improve_profile(
        depth,
        iterations,
        seeds,
        discover,
        no_discover,
        evals,
        evals_interval,
    )?;
    let forced_targets = if learn { &["learn"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let next_target = next_chain_target_value(&chain_targets);
    let workspace = resolve_workspace_root(cwd);
    let icp = icp
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("DECISION NEEDED: define the target ICP.");
    let output_dir = output_dir
        .unwrap_or_else(|| default_artifact_path("improve", format!("improve-{}", slugify(goal))));
    let output_dir = resolve_workspace_path(&workspace, output_dir);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    write_text_file(
        &output_dir.join("research-findings.md"),
        &render_improve_research_markdown(goal, icp, &scope, &profile),
    )?;
    write_text_file(
        &output_dir.join("improvement-plan.md"),
        &render_improve_plan_markdown(goal, icp, &profile),
    )?;
    write_text_file(
        &output_dir.join("summary.md"),
        &render_improve_summary_markdown(goal, icp, &output_dir, &profile),
    )?;
    write_text_file(
        &output_dir.join("improve-results.tsv"),
        &render_improve_results_tsv(goal, &profile),
    )?;
    let findings = improve_active_categories(&profile)
        .iter()
        .flat_map(|(category, _)| {
            (1..=profile.seeds_per_category).map(move |seed_index| {
                serde_json::json!({
                    "category": category,
                    "title": improve_seed_title_at(goal, category, seed_index),
                    "confidence": "LOW",
                    "prd_path": null,
                })
            })
        })
        .collect::<Vec<_>>();
    let handoff = serde_json::json!({
        "version": "2.1.0",
        "source": "improve",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "COMPLETE",
        "results_tsv": output_dir.join("improve-results.tsv").display().to_string(),
        "findings": findings,
        "config": {
            "goal": goal,
            "icp": icp,
            "scope": scope,
            "depth": profile.depth.as_str(),
            "categories_explored": improve_active_categories(&profile).len(),
            "categories_available": improve_categories().len(),
            "insights_total": improve_active_categories(&profile).len() * usize::from(profile.seeds_per_category),
            "seeds_per_category": profile.seeds_per_category,
            "discover": profile.discover,
            "iteration_budget": profile.iteration_budget,
            "evals": profile.evals,
            "evals_interval": profile.evals_interval,
            "prds_generated": 0,
        },
        "chain": chain_targets,
        "next_target": next_target.clone(),
        "chain_continue": should_continue_handoff_chain("COMPLETE"),
        "propagate_evals": profile.evals,
        "evals_interval": profile.evals_interval,
    });
    write_json_file(&output_dir.join("handoff.json"), &handoff)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "output_dir": output_dir.display().to_string(),
            "goal": goal,
            "depth": profile.depth.as_str(),
            "categories": improve_active_categories(&profile).len(),
            "categories_available": improve_categories().len(),
            "insights": improve_active_categories(&profile).len() * usize::from(profile.seeds_per_category),
            "seeds_per_category": profile.seeds_per_category,
            "discover": profile.discover,
            "iteration_budget": profile.iteration_budget,
            "evals": profile.evals,
            "evals_interval": profile.evals_interval,
            "next_target": next_target,
            "prds_generated": 0,
        })
    );
    Ok(())
}

fn security_severities() -> &'static [Severity] {
    &[
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ]
}

fn parse_security_severity(value: Option<&str>, flag: &str) -> Result<Option<Severity>> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" | "crit" => Ok(Some(Severity::Critical)),
        "high" => Ok(Some(Severity::High)),
        "medium" | "med" => Ok(Some(Severity::Medium)),
        "low" => Ok(Some(Severity::Low)),
        "info" | "informational" => Ok(Some(Severity::Info)),
        other => anyhow::bail!(
            "Invalid {flag} severity {other:?}; use critical, high, medium, low, or info"
        ),
    }
}

#[derive(Debug, Clone)]
struct SecurityProfile {
    depth: String,
    iteration_budget: u32,
    diff: bool,
    evals: bool,
    evals_interval: Option<u32>,
}

fn parse_security_depth(value: &str) -> Result<(&'static str, u32)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "quick" | "shallow" => Ok(("quick", 5)),
        "standard" | "normal" => Ok(("standard", 15)),
        "deep" | "comprehensive" => Ok(("deep", 30)),
        other => anyhow::bail!("Invalid security depth {other:?}; use quick, standard, or deep"),
    }
}

fn resolve_security_profile(
    depth: &str,
    iterations: Option<u32>,
    diff: bool,
    evals: bool,
    evals_interval: Option<u32>,
) -> Result<SecurityProfile> {
    validate_chain_evals_flags("security", evals, evals_interval)?;
    let (depth, iteration_budget) = parse_security_depth(depth)?;
    if iterations == Some(0) {
        anyhow::bail!("security iterations must be greater than zero");
    }
    Ok(SecurityProfile {
        depth: depth.to_string(),
        iteration_budget: iterations.unwrap_or(iteration_budget),
        diff,
        evals,
        evals_interval,
    })
}

fn render_security_overview(
    focus: &str,
    scope: &[String],
    files: &[String],
    profile: &SecurityProfile,
) -> String {
    let mut out = String::new();
    let scope_lines = if scope.is_empty() {
        "- DECISION NEEDED: identify audit scope.".to_string()
    } else {
        scope
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    writeln!(out, "# Security Audit Overview").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Focus: {focus}").unwrap();
    writeln!(out, "- Depth: {}", profile.depth).unwrap();
    writeln!(out, "- Iteration budget: {}", profile.iteration_budget).unwrap();
    writeln!(out, "- Diff mode: {}", profile.diff).unwrap();
    writeln!(out, "- Evals enabled: {}", profile.evals).unwrap();
    writeln!(
        out,
        "- Evals interval: {}",
        profile
            .evals_interval
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    )
    .unwrap();
    writeln!(out, "- Files scanned: {}", files.len()).unwrap();
    writeln!(out, "- OWASP categories: {}", OwaspCategory::all().len()).unwrap();
    writeln!(out, "- STRIDE categories: {}", StrideCategory::all().len()).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## File Inventory").unwrap();
    writeln!(out).unwrap();
    if files.is_empty() {
        writeln!(out, "- DECISION NEEDED: no files matched audit scope.").unwrap();
    } else {
        for file in files.iter().take(25) {
            writeln!(out, "- {file}").unwrap();
        }
        if files.len() > 25 {
            writeln!(out, "- ... {} more", files.len() - 25).unwrap();
        }
    }
    out
}

fn render_security_threat_model(focus: &str) -> String {
    let mut out = String::new();
    writeln!(out, "# STRIDE Threat Model").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Focus: {focus}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| STRIDE | Audit Prompt |").unwrap();
    writeln!(out, "|---|---|").unwrap();
    for category in StrideCategory::all() {
        writeln!(
            out,
            "| {} | Identify code paths where {focus} may permit {}. |",
            category.label(),
            category.label().to_ascii_lowercase()
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "Every confirmed threat needs file:line evidence, attack scenario, impact, confidence, and mitigation."
    )
    .unwrap();
    out
}

fn render_security_attack_surface(files: &[String]) -> String {
    let mut out = String::new();
    writeln!(out, "# Attack Surface Map").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Surface | Evidence Source | Review Question |").unwrap();
    writeln!(out, "|---|---|---|").unwrap();
    let sources = if files.is_empty() {
        vec!["DECISION NEEDED".to_string()]
    } else {
        files.iter().take(10).cloned().collect()
    };
    for file in sources {
        writeln!(
            out,
            "| Entry point or data flow | {file} | What untrusted input, secret, permission, or dependency crosses this path? |"
        )
        .unwrap();
    }
    out
}

fn render_security_coverage() -> String {
    let mut out = String::new();
    writeln!(out, "# Security Coverage").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## OWASP Top 10").unwrap();
    writeln!(out).unwrap();
    for category in OwaspCategory::all() {
        writeln!(out, "- [ ] {}", category.label()).unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "## STRIDE").unwrap();
    writeln!(out).unwrap();
    for category in StrideCategory::all() {
        writeln!(out, "- [ ] {}", category.label()).unwrap();
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "Composite score = (owasp_covered / 10) * 50 + (stride_covered / 6) * 30 + min(findings, 20)."
    )
    .unwrap();
    out
}

fn render_security_findings() -> String {
    let mut out = String::new();
    writeln!(out, "# Security Findings").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "| Severity | Title | OWASP | STRIDE | Evidence | Mitigation |"
    )
    .unwrap();
    writeln!(out, "|---|---|---|---|---|---|").unwrap();
    writeln!(
        out,
        "| INFO | DECISION NEEDED: complete evidence-backed audit | - | - | no confirmed file:line yet | run the audit loop and fill confirmed findings |"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Severity Labels").unwrap();
    writeln!(out).unwrap();
    for severity in security_severities() {
        writeln!(out, "- {}", severity.label()).unwrap();
    }
    out
}

fn render_security_recommendations() -> String {
    let mut out = String::new();
    writeln!(out, "# Security Recommendations").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Prioritize unauthenticated remote exploit paths first."
    )
    .unwrap();
    writeln!(
        out,
        "- Require file:line evidence before promoting a theoretical risk to a finding."
    )
    .unwrap();
    writeln!(
        out,
        "- Chain confirmed Critical or High findings into `autoresearch fix` with a focused verify command."
    )
    .unwrap();
    writeln!(
        out,
        "- DECISION NEEDED: choose `--fail-on` severity for CI or release gating."
    )
    .unwrap();
    out
}

fn render_security_dependency_audit(files: &[String]) -> String {
    let mut out = String::new();
    writeln!(out, "# Dependency Audit").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Review manifests and lockfiles before closeout.").unwrap();
    writeln!(out).unwrap();
    for file in files
        .iter()
        .filter(|file| {
            file.ends_with("Cargo.toml")
                || file.ends_with("Cargo.lock")
                || file.ends_with("package.json")
                || file.ends_with("package-lock.json")
                || file.ends_with("pnpm-lock.yaml")
                || file.ends_with("requirements.txt")
                || file.ends_with("pyproject.toml")
        })
        .take(20)
    {
        writeln!(out, "- {file}").unwrap();
    }
    writeln!(
        out,
        "- DECISION NEEDED: run the ecosystem-specific dependency scanner."
    )
    .unwrap();
    out
}

fn render_security_results_tsv() -> String {
    let mut out = String::new();
    let timestamp = chrono::Utc::now().to_rfc3339();
    writeln!(out, "# metric_direction: higher_is_better").unwrap();
    writeln!(
        out,
        "iteration\ttimestamp\tfinding\tseverity\towasp\tstride\tevidence\tfile_line"
    )
    .unwrap();
    writeln!(
        out,
        "0\t{timestamp}\tbaseline security artifact bundle\tinfo\t-\t-\tpending audit\t-"
    )
    .unwrap();
    out
}

fn cmd_security(
    scope: Vec<String>,
    focus: Option<String>,
    depth: &str,
    iterations: Option<u32>,
    diff: bool,
    fix: bool,
    fail_on: Option<String>,
    chain: Option<String>,
    evals: bool,
    evals_interval: Option<u32>,
    output_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let fail_on = parse_security_severity(fail_on.as_deref(), "--fail-on")?;
    let profile = resolve_security_profile(depth, iterations, diff, evals, evals_interval)?;
    let forced_targets = if fix { &["fix"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let next_target = next_chain_target_value(&chain_targets);
    let workspace = resolve_workspace_root(cwd);
    let focus = focus
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("general");
    let scan_scope = if scope.is_empty() {
        vec!["**/*".to_string()]
    } else {
        scope.clone()
    };
    let files = collect_learn_files(&workspace, &scan_scope, &[], 50);
    let output_dir = output_dir.unwrap_or_else(|| {
        default_artifact_path("security", format!("security-{}", slugify(focus)))
    });
    let output_dir = resolve_workspace_path(&workspace, output_dir);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let confirmed_findings = 0usize;
    let gate_failed = fail_on.is_some() && confirmed_findings > 0;

    write_text_file(
        &output_dir.join("overview.md"),
        &render_security_overview(focus, &scope, &files, &profile),
    )?;
    write_text_file(
        &output_dir.join("threat-model.md"),
        &render_security_threat_model(focus),
    )?;
    write_text_file(
        &output_dir.join("attack-surface-map.md"),
        &render_security_attack_surface(&files),
    )?;
    let coverage = render_security_coverage();
    write_text_file(&output_dir.join("coverage.md"), &coverage)?;
    write_text_file(&output_dir.join("owasp-coverage.md"), &coverage)?;
    write_text_file(&output_dir.join("findings.md"), &render_security_findings())?;
    write_text_file(
        &output_dir.join("recommendations.md"),
        &render_security_recommendations(),
    )?;
    write_text_file(
        &output_dir.join("dependency-audit.md"),
        &render_security_dependency_audit(&files),
    )?;
    write_text_file(
        &output_dir.join("security-audit-results.tsv"),
        &render_security_results_tsv(),
    )?;
    let handoff = serde_json::json!({
        "version": "2.1.0",
        "source": "security",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "COMPLETE",
        "results_tsv": output_dir.join("security-audit-results.tsv").display().to_string(),
        "findings": [],
        "config": {
            "scope": scope,
            "focus": focus,
            "depth": profile.depth.as_str(),
            "iteration_budget": profile.iteration_budget,
            "diff": profile.diff,
            "fix_requested": fix,
            "fail_on": fail_on.map(|severity| severity.label()),
            "gate_failed": gate_failed,
            "confirmed_findings": confirmed_findings,
            "owasp_categories": OwaspCategory::all().len(),
            "stride_categories": StrideCategory::all().len(),
            "files_scanned": files.len(),
        },
        "chain": chain_targets,
        "next_target": next_target.clone(),
        "chain_continue": should_continue_handoff_chain("COMPLETE"),
        "propagate_evals": profile.evals,
        "evals_interval": profile.evals_interval,
    });
    write_json_file(&output_dir.join("handoff.json"), &handoff)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "output_dir": output_dir.display().to_string(),
            "focus": focus,
            "depth": profile.depth.as_str(),
            "iteration_budget": profile.iteration_budget,
            "diff": profile.diff,
            "owasp_categories": OwaspCategory::all().len(),
            "stride_categories": StrideCategory::all().len(),
            "files_scanned": files.len(),
            "fix_requested": fix,
            "fail_on": fail_on.map(|severity| severity.label()),
            "gate_failed": gate_failed,
            "confirmed_findings": confirmed_findings,
            "next_target": next_target,
            "evals": profile.evals,
            "evals_interval": profile.evals_interval,
        })
    );
    Ok(())
}

fn ship_type_checklist(ship_type: &str) -> &'static [&'static str] {
    match ship_type {
        "deployment" => &[
            "Environment variables confirmed",
            "Health checks configured",
            "Rollback path documented",
            "Monitoring window assigned",
        ],
        "code-release" | "package" => &[
            "Version bumped",
            "Changelog updated",
            "Package/build artifacts verified",
            "Breaking changes documented",
        ],
        "content" | "docs" => &[
            "Links checked",
            "Images and assets verified",
            "Metadata reviewed",
            "Spell and formatting pass completed",
        ],
        _ => &[
            "Tests pass",
            "Lint or type checks pass",
            "No secrets in diff",
            "PR description and reviewers ready",
        ],
    }
}

#[derive(Debug, Clone)]
struct ShipProfile {
    auto: bool,
    force: bool,
    rollback: bool,
    monitor_minutes: Option<u32>,
}

fn resolve_ship_profile(
    auto: bool,
    force: bool,
    rollback: bool,
    monitor_minutes: Option<u32>,
) -> Result<ShipProfile> {
    if monitor_minutes == Some(0) {
        anyhow::bail!("ship monitor minutes must be greater than zero");
    }
    Ok(ShipProfile {
        auto,
        force,
        rollback,
        monitor_minutes,
    })
}

fn ship_handoff_status(dry_run: bool, checklist_only: bool, profile: &ShipProfile) -> &'static str {
    if profile.rollback {
        "ROLLBACK"
    } else if dry_run || checklist_only {
        "DRY_RUN"
    } else {
        "COMPLETE"
    }
}

fn render_ship_checklist(
    target: &str,
    ship_type: &str,
    dry_run: bool,
    checklist_only: bool,
    profile: &ShipProfile,
) -> String {
    let mut out = String::new();
    writeln!(out, "# Ship Checklist: {target}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Type: {ship_type}").unwrap();
    writeln!(out, "- Dry run: {dry_run}").unwrap();
    writeln!(out, "- Checklist only: {checklist_only}").unwrap();
    writeln!(out, "- Auto approval requested: {}", profile.auto).unwrap();
    writeln!(out, "- Force non-critical items: {}", profile.force).unwrap();
    writeln!(out, "- Rollback requested: {}", profile.rollback).unwrap();
    writeln!(
        out,
        "- Monitor minutes: {}",
        profile
            .monitor_minutes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## 8 Phases").unwrap();
    writeln!(out).unwrap();
    for phase in ShipPhase::all() {
        writeln!(out, "- [ ] {}", phase.label()).unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "## Type-Specific Gates").unwrap();
    writeln!(out).unwrap();
    for item in ship_type_checklist(ship_type) {
        writeln!(out, "- [ ] {item}").unwrap();
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "DECISION NEEDED: explicit approval is required before external ship actions."
    )
    .unwrap();
    out
}

fn render_ship_summary(
    target: &str,
    ship_type: &str,
    dry_run: bool,
    checklist_only: bool,
    profile: &ShipProfile,
) -> String {
    format!(
        "# Ship Summary: {target}\n\n\
         - Type: {ship_type}\n\
         - Dry run: {dry_run}\n\
         - Checklist only: {checklist_only}\n\
         - Auto approval requested: {}\n\
         - Force non-critical items: {}\n\
         - Rollback requested: {}\n\
         - Monitor minutes: {}\n\
         - Phase count: {}\n\
         - Status: {}\n\n\
         This artifact does not perform external side effects. Run the checklist, capture blockers, and only execute Ship after explicit approval.\n",
        profile.auto,
        profile.force,
        profile.rollback,
        profile
            .monitor_minutes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        ShipPhase::all().len(),
        ship_handoff_status(dry_run, checklist_only, profile)
    )
}

fn render_ship_log(
    target: &str,
    ship_type: &str,
    dry_run: bool,
    checklist_only: bool,
    profile: &ShipProfile,
) -> String {
    let mut out = String::new();
    let timestamp = chrono::Utc::now().to_rfc3339();
    writeln!(
        out,
        "timestamp\ttype\ttarget\tchecklist_score\tdry_run\tauto\tforce\trollback\tmonitor_minutes\tshipped\tverified\tduration\tnotes"
    )
    .unwrap();
    let monitor_minutes = profile
        .monitor_minutes
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    writeln!(
        out,
        "{timestamp}\t{ship_type}\t{target}\t0\t{dry_run}\t{}\t{}\t{}\t{monitor_minutes}\tfalse\tfalse\t0\tchecklist_only={checklist_only}",
        profile.auto, profile.force, profile.rollback
    )
    .unwrap();
    out
}

fn cmd_ship(
    target: &str,
    ship_type: &str,
    dry_run: bool,
    auto: bool,
    force: bool,
    rollback: bool,
    monitor_minutes: Option<u32>,
    checklist_only: bool,
    learn: bool,
    chain: Option<String>,
    output_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let profile = resolve_ship_profile(auto, force, rollback, monitor_minutes)?;
    let forced_targets = if learn { &["learn"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let next_target = next_chain_target_value(&chain_targets);
    let status = ship_handoff_status(dry_run, checklist_only, &profile);
    let workspace = resolve_workspace_root(cwd);
    let output_dir = output_dir
        .unwrap_or_else(|| default_artifact_path("ship", format!("ship-{}", slugify(target))));
    let output_dir = resolve_workspace_path(&workspace, output_dir);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    write_text_file(
        &output_dir.join("checklist.md"),
        &render_ship_checklist(target, ship_type, dry_run, checklist_only, &profile),
    )?;
    write_text_file(
        &output_dir.join("summary.md"),
        &render_ship_summary(target, ship_type, dry_run, checklist_only, &profile),
    )?;
    write_text_file(
        &output_dir.join("ship-log.tsv"),
        &render_ship_log(target, ship_type, dry_run, checklist_only, &profile),
    )?;
    let handoff = serde_json::json!({
        "version": "2.1.0",
        "source": "ship",
        "source_command": "ship",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": status,
        "results_tsv": output_dir.join("ship-log.tsv").display().to_string(),
        "findings": [],
        "config": {
            "target": target,
            "type": ship_type,
            "dry_run": dry_run,
            "checklist_only": checklist_only,
            "auto": profile.auto,
            "force": profile.force,
            "rollback": profile.rollback,
            "monitor_minutes": profile.monitor_minutes,
            "phases": ShipPhase::all().len(),
        },
        "chain": chain_targets,
        "next_target": next_target.clone(),
        "chain_continue": should_continue_handoff_chain(status),
    });
    write_json_file(&output_dir.join("handoff.json"), &handoff)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "output_dir": output_dir.display().to_string(),
            "target": target,
            "type": ship_type,
            "phases": ShipPhase::all().len(),
            "dry_run": dry_run,
            "checklist_only": checklist_only,
            "auto": profile.auto,
            "force": profile.force,
            "rollback": profile.rollback,
            "monitor_minutes": profile.monitor_minutes,
            "handoff_status": status,
            "next_target": next_target,
        })
    );
    Ok(())
}

fn debug_phase_label(phase: DebugPhase) -> &'static str {
    match phase {
        DebugPhase::GatherEvidence => "Gather Evidence",
        DebugPhase::Hypothesize => "Hypothesize",
        DebugPhase::TestHypothesis => "Test Hypothesis",
        DebugPhase::Fix => "Fix",
    }
}

#[derive(Debug, Clone)]
struct DebugProfile {
    depth: String,
    iteration_budget: u32,
    severity: Option<Severity>,
}

fn parse_debug_depth(value: &str) -> Result<(&'static str, u32)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "quick" | "shallow" => Ok(("quick", 5)),
        "standard" | "normal" => Ok(("standard", 15)),
        "deep" | "comprehensive" => Ok(("deep", 30)),
        other => anyhow::bail!("Invalid debug depth {other:?}; use quick, standard, or deep"),
    }
}

fn resolve_debug_profile(
    depth: &str,
    iterations: Option<u32>,
    severity: Option<&str>,
) -> Result<DebugProfile> {
    let (depth, iteration_budget) = parse_debug_depth(depth)?;
    if iterations == Some(0) {
        anyhow::bail!("debug iterations must be greater than zero");
    }
    let severity = parse_security_severity(severity, "--severity")?;
    Ok(DebugProfile {
        depth: depth.to_string(),
        iteration_budget: iterations.unwrap_or(iteration_budget),
        severity,
    })
}

fn render_debug_summary(
    symptom: &str,
    technique: &str,
    scope: &[String],
    files: &[String],
    profile: &DebugProfile,
) -> String {
    let scope_lines = if scope.is_empty() {
        "- DECISION NEEDED: identify investigation scope.".to_string()
    } else {
        scope
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut out = String::new();
    writeln!(out, "# Debug Summary: {symptom}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Technique seed: {technique}").unwrap();
    writeln!(out, "- Depth: {}", profile.depth).unwrap();
    writeln!(out, "- Iteration budget: {}", profile.iteration_budget).unwrap();
    writeln!(
        out,
        "- Severity filter: {}",
        profile
            .severity
            .map(|severity| severity.label())
            .unwrap_or("all")
    )
    .unwrap();
    writeln!(out, "- Files scanned: {}", files.len()).unwrap();
    writeln!(out, "- Confirmed findings: 0").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Investigation Phases").unwrap();
    writeln!(out).unwrap();
    for phase in DebugPhase::all() {
        writeln!(out, "- [ ] {}", debug_phase_label(*phase)).unwrap();
    }
    out
}

fn render_debug_findings(symptom: &str, technique: &str) -> String {
    format!(
        "# Debug Findings\n\n\
         ## Seed Hypothesis\n\n\
         I hypothesize that `{symptom}` is caused by an unverified code path in the selected scope because no confirmed evidence has been recorded yet. Test by using `{technique}` and recording file:line evidence.\n\n\
         ## Confirmed Bugs\n\n\
         - DECISION NEEDED: run the investigation loop and promote only evidence-backed findings.\n"
    )
}

fn render_debug_eliminated(symptom: &str) -> String {
    format!(
        "# Eliminated Hypotheses\n\n\
         Symptom: {symptom}\n\n\
         | Hypothesis | Result | Evidence |\n\
         |---|---|---|\n\
         | DECISION NEEDED | untested | no evidence recorded |\n"
    )
}

fn render_debug_results_tsv(symptom: &str, technique: &str) -> String {
    let timestamp = chrono::Utc::now().to_rfc3339();
    format!(
        "# metric_direction: higher_is_better\n\
         iteration\ttimestamp\thypothesis\tstatus\ttechnique\tevidence\tfile_line\n\
         0\t{timestamp}\tseed investigation for {symptom}\tinconclusive\t{technique}\tpending\t-\n"
    )
}

fn chain_targets_with_forced(chain: Option<&str>, forced: &[&str]) -> Result<Vec<String>> {
    let mut targets = parse_handoff_chain_targets(chain)?;
    for target in forced {
        if !is_valid_handoff_source(target) {
            anyhow::bail!("invalid handoff chain target {target:?}");
        }
        if !targets.iter().any(|existing| existing == target) {
            targets.push((*target).to_string());
        }
    }
    Ok(targets)
}

fn next_chain_target_value(targets: &[String]) -> serde_json::Value {
    targets
        .first()
        .cloned()
        .map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null)
}

fn validate_chain_evals_flags(
    command: &str,
    evals: bool,
    evals_interval: Option<u32>,
) -> Result<()> {
    if evals_interval == Some(0) {
        anyhow::bail!("{command} evals interval must be greater than zero");
    }
    if evals_interval.is_some() && !evals {
        anyhow::bail!("{command} evals interval requires --evals");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_debug(
    symptom: &str,
    scope: Vec<String>,
    technique: &str,
    depth: &str,
    iterations: Option<u32>,
    severity: Option<String>,
    fix: bool,
    chain: Option<String>,
    evals: bool,
    evals_interval: Option<u32>,
    output_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    validate_chain_evals_flags("debug", evals, evals_interval)?;
    let profile = resolve_debug_profile(depth, iterations, severity.as_deref())?;
    let forced_targets = if fix { &["fix"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let workspace = resolve_workspace_root(cwd);
    let scan_scope = if scope.is_empty() {
        vec!["**/*".to_string()]
    } else {
        scope.clone()
    };
    let files = collect_learn_files(&workspace, &scan_scope, &[], 50);
    let output_dir = output_dir
        .unwrap_or_else(|| default_artifact_path("debug", format!("debug-{}", slugify(symptom))));
    let output_dir = resolve_workspace_path(&workspace, output_dir);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    write_text_file(
        &output_dir.join("summary.md"),
        &render_debug_summary(symptom, technique, &scope, &files, &profile),
    )?;
    write_text_file(
        &output_dir.join("findings.md"),
        &render_debug_findings(symptom, technique),
    )?;
    write_text_file(
        &output_dir.join("eliminated.md"),
        &render_debug_eliminated(symptom),
    )?;
    write_text_file(
        &output_dir.join("debug-results.tsv"),
        &render_debug_results_tsv(symptom, technique),
    )?;
    let next_target = next_chain_target_value(&chain_targets);
    let handoff = serde_json::json!({
        "version": "2.1.0",
        "source": "debug",
        "source_command": "debug",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "COMPLETE",
        "results_tsv": output_dir.join("debug-results.tsv").display().to_string(),
        "handoff_path": output_dir.join("handoff.json").display().to_string(),
        "findings": [],
        "config": {
            "scope": scope,
            "symptom": symptom,
            "technique": technique,
            "depth": profile.depth.as_str(),
            "iteration_budget": profile.iteration_budget,
            "severity": profile.severity.map(|severity| severity.label()),
            "files_scanned": files.len(),
        },
        "chain": chain_targets,
        "next_target": next_target,
        "chain_continue": should_continue_handoff_chain("COMPLETE"),
        "propagate_evals": evals,
        "evals_interval": evals_interval,
    });
    write_json_file(&output_dir.join("handoff.json"), &handoff)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "output_dir": output_dir.display().to_string(),
            "symptom": symptom,
            "technique": technique,
            "depth": profile.depth.as_str(),
            "iteration_budget": profile.iteration_budget,
            "severity": profile.severity.map(|severity| severity.label()),
            "phases": DebugPhase::all().len(),
            "files_scanned": files.len(),
        })
    );
    Ok(())
}

fn parse_fix_category(value: Option<&str>) -> Result<Option<ErrorCategory>> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "crash" | "panic" | "runtime" => Ok(Some(ErrorCategory::Crash)),
        "test" | "tests" | "test_failure" | "test-failure" => Ok(Some(ErrorCategory::TestFailure)),
        "type" | "types" | "compile" | "compiler" | "type_error" | "type-error" => {
            Ok(Some(ErrorCategory::TypeError))
        }
        "lint" | "linter" | "lint_error" | "lint-error" => Ok(Some(ErrorCategory::LintError)),
        "build" | "builds" | "build_error" | "build-error" | "package" | "packaging" => {
            Ok(Some(ErrorCategory::BuildError))
        }
        "warning" | "warnings" => Ok(Some(ErrorCategory::Warning)),
        other => {
            anyhow::bail!(
                "Invalid fix category {other:?}; use crash, test, type, lint, build, or warning"
            )
        }
    }
}

fn render_fix_summary(
    target: &str,
    scope: &[String],
    guard: Option<&str>,
    category: Option<ErrorCategory>,
    iterations: u32,
) -> String {
    let scope_lines = if scope.is_empty() {
        "- DECISION NEEDED: identify editable scope.".to_string()
    } else {
        scope
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut out = String::new();
    writeln!(out, "# Fix Summary").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Target: `{target}`").unwrap();
    writeln!(out, "- Guard: `{}`", guard.unwrap_or("none")).unwrap();
    writeln!(
        out,
        "- Category: {}",
        category.map(|value| value.label()).unwrap_or("auto")
    )
    .unwrap();
    writeln!(out, "- Iteration budget: {iterations}").unwrap();
    writeln!(out, "- Strategy: one error per iteration").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    out
}

fn render_fix_plan(target: &str, category: Option<ErrorCategory>) -> String {
    let mut out = String::new();
    writeln!(out, "# Repair Plan").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Verify target: `{target}`").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Priority Order").unwrap();
    writeln!(out).unwrap();
    for item in ErrorCategory::priority_order() {
        let marker = if Some(*item) == category {
            "selected"
        } else {
            "candidate"
        };
        writeln!(out, "- {} ({marker})", item.label()).unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "## Iteration Contract").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Fix exactly one error class or one concrete error per trial."
    )
    .unwrap();
    writeln!(out, "- Commit the trial before verification.").unwrap();
    writeln!(
        out,
        "- Keep only if the target error count decreases and guard passes."
    )
    .unwrap();
    writeln!(
        out,
        "- Discard or rework when the count is unchanged, worse, or guard fails."
    )
    .unwrap();
    out
}

fn render_fix_results_tsv(target: &str) -> String {
    let timestamp = chrono::Utc::now().to_rfc3339();
    format!(
        "# metric_direction: lower_is_better\n\
         iteration\ttimestamp\ttarget\tcategory\terror_count\tdelta\tguard\tstatus\tdescription\n\
         0\t{timestamp}\t{target}\tbaseline\tDECISION NEEDED\t0\t-\tbaseline\trepair plan created\n"
    )
}

#[derive(Debug, Clone)]
struct DebugHandoffInput {
    path: PathBuf,
    symptom: Option<String>,
    scope: Vec<String>,
    findings_count: usize,
}

fn collect_debug_handoff_candidates(dir: &Path, candidates: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            collect_debug_handoff_candidates(&path, candidates);
        } else if path.file_name().is_some_and(|name| name == "handoff.json") {
            candidates.push(path);
        }
    }
}

fn latest_debug_handoff_path(workspace: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in [
        workspace.join("autoresearch-results/debug"),
        workspace.join("debug"),
        workspace.join("autoresearch"),
    ] {
        collect_debug_handoff_candidates(&root, &mut candidates);
    }
    candidates.into_iter().max_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    })
}

fn string_array_from_value(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn load_latest_debug_handoff(workspace: &Path) -> Result<DebugHandoffInput> {
    let path = latest_debug_handoff_path(workspace)
        .context("fix --from-debug could not find a debug handoff.json")?;
    let handoff: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("invalid debug handoff JSON at {}", path.display()))?;
    let source = handoff
        .get("source")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            handoff
                .get("source_command")
                .and_then(serde_json::Value::as_str)
        });
    if source != Some("debug") {
        anyhow::bail!("latest handoff is not from debug: {}", path.display());
    }
    let config = handoff.get("config");
    let symptom = config
        .and_then(|value| value.get("symptom"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            handoff
                .get("goal")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        });
    let scope = string_array_from_value(config.and_then(|value| value.get("scope")))
        .into_iter()
        .chain(string_array_from_value(handoff.get("scope")))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let findings_count = handoff
        .get("findings")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    Ok(DebugHandoffInput {
        path,
        symptom,
        scope,
        findings_count,
    })
}

fn append_debug_import_section(out: &mut String, debug: Option<&DebugHandoffInput>) {
    let Some(debug) = debug else {
        return;
    };
    writeln!(out).unwrap();
    writeln!(out, "## Imported Debug Handoff").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Path: `{}`", debug.path.display()).unwrap();
    writeln!(
        out,
        "- Symptom: {}",
        debug.symptom.as_deref().unwrap_or("DECISION NEEDED")
    )
    .unwrap();
    writeln!(out, "- Findings imported: {}", debug.findings_count).unwrap();
}

fn cmd_fix(
    target: Option<String>,
    scope: Vec<String>,
    from_debug: bool,
    guard: Option<String>,
    category: Option<String>,
    iterations: Option<u32>,
    learn: bool,
    chain: Option<String>,
    evals: bool,
    evals_interval: Option<u32>,
    output_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    validate_chain_evals_flags("fix", evals, evals_interval)?;
    if iterations == Some(0) {
        anyhow::bail!("fix iterations must be greater than zero");
    }
    let iteration_budget = iterations.unwrap_or(20);
    let forced_targets = if learn { &["learn"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let next_target = next_chain_target_value(&chain_targets);
    let category = parse_fix_category(category.as_deref())?;
    let workspace = resolve_workspace_root(cwd);
    let debug_handoff = if from_debug {
        Some(load_latest_debug_handoff(&workspace)?)
    } else {
        None
    };
    let target = target
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            debug_handoff.as_ref().map(|debug| {
                format!(
                    "debug findings from {}",
                    debug.symptom.as_deref().unwrap_or("latest debug handoff")
                )
            })
        })
        .context("fix requires --target unless --from-debug is used")?;
    let scope = if scope.is_empty() {
        debug_handoff
            .as_ref()
            .map(|debug| debug.scope.clone())
            .unwrap_or_default()
    } else {
        scope
    };
    let output_dir = output_dir.unwrap_or_else(|| {
        PathBuf::from("autoresearch-results")
            .join("fix")
            .join(format!("fix-{}", slugify(&target)))
    });
    let output_dir = resolve_workspace_path(&workspace, output_dir);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let mut summary = render_fix_summary(
        &target,
        &scope,
        guard.as_deref(),
        category,
        iteration_budget,
    );
    append_debug_import_section(&mut summary, debug_handoff.as_ref());
    write_text_file(&output_dir.join("summary.md"), &summary)?;
    write_text_file(
        &output_dir.join("repair-plan.md"),
        &render_fix_plan(&target, category),
    )?;
    write_text_file(
        &output_dir.join("fix-results.tsv"),
        &render_fix_results_tsv(&target),
    )?;
    let handoff = serde_json::json!({
        "version": "2.1.0",
        "source": "fix",
        "source_command": "fix",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "COMPLETE",
        "results_tsv": output_dir.join("fix-results.tsv").display().to_string(),
        "findings": [],
        "config": {
            "target": target.clone(),
            "scope": scope,
            "guard": guard,
            "category": category.map(|value| value.label()),
            "iteration_budget": iteration_budget,
            "from_debug": from_debug,
            "debug_handoff_path": debug_handoff.as_ref().map(|debug| debug.path.display().to_string()),
            "debug_symptom": debug_handoff.as_ref().and_then(|debug| debug.symptom.clone()),
        },
        "chain": chain_targets,
        "next_target": next_target.clone(),
        "chain_continue": should_continue_handoff_chain("COMPLETE"),
        "propagate_evals": evals,
        "evals_interval": evals_interval,
    });
    write_json_file(&output_dir.join("handoff.json"), &handoff)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "output_dir": output_dir.display().to_string(),
            "target": target,
            "category": category.map(|value| value.label()).unwrap_or("auto"),
            "iteration_budget": iteration_budget,
            "from_debug": from_debug,
            "next_target": next_target,
            "evals": evals,
            "evals_interval": evals_interval,
        })
    );
    Ok(())
}

fn parse_scenario_format(value: &str) -> Result<ScenarioFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "use-case" | "use-cases" | "usecase" | "usecases" => Ok(ScenarioFormat::UseCase),
        "user-story" | "user-stories" | "userstory" | "userstories" => {
            Ok(ScenarioFormat::UserStory)
        }
        "test" | "tests" | "test-scenario" | "test-scenarios" | "testscenario"
        | "testscenarios" => Ok(ScenarioFormat::TestScenario),
        "threat" | "threats" | "threat-scenario" | "threat-scenarios" | "threatscenario"
        | "threatscenarios" => Ok(ScenarioFormat::ThreatScenario),
        other => anyhow::bail!(
            "Invalid scenario format {other:?}; use use-cases, user-stories, test-scenarios, or threat-scenarios"
        ),
    }
}

fn scenario_format_slug(format: ScenarioFormat) -> &'static str {
    match format {
        ScenarioFormat::UseCase => "use-cases",
        ScenarioFormat::UserStory => "user-stories",
        ScenarioFormat::TestScenario => "test-scenarios",
        ScenarioFormat::ThreatScenario => "threat-scenarios",
    }
}

#[derive(Debug, Clone)]
struct ScenarioProfile {
    domain: String,
    depth: String,
    exploration_budget: u32,
    evals: bool,
    evals_interval: Option<u32>,
}

fn parse_scenario_domain(value: &str) -> Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "general" | "generic" => Ok("general"),
        "web" | "web-app" | "webapp" | "web app" => Ok("web"),
        "mobile" | "mobile-app" | "mobileapp" | "mobile app" => Ok("mobile"),
        "api" | "service" | "backend" => Ok("api"),
        "cli" | "command-line" | "commandline" | "command line" => Ok("cli"),
        "data" | "data-pipeline" | "datapipeline" | "data pipeline" | "pipeline" => {
            Ok("data pipeline")
        }
        "infra" | "infrastructure" | "devops" => Ok("infrastructure"),
        other => anyhow::bail!(
            "Invalid scenario domain {other:?}; use general, web, mobile, api, cli, data-pipeline, or infrastructure"
        ),
    }
}

fn parse_scenario_depth(value: &str) -> Result<(&'static str, u32)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "shallow" | "quick" => Ok(("shallow", 10)),
        "standard" | "normal" => Ok(("standard", 20)),
        "deep" | "comprehensive" => Ok(("deep", 40)),
        other => anyhow::bail!("Invalid scenario depth {other:?}; use shallow, standard, or deep"),
    }
}

fn resolve_scenario_profile(
    domain: &str,
    depth: &str,
    iterations: Option<u32>,
    evals: bool,
    evals_interval: Option<u32>,
) -> Result<ScenarioProfile> {
    validate_chain_evals_flags("scenario", evals, evals_interval)?;
    let domain = parse_scenario_domain(domain)?;
    let (depth, exploration_budget) = parse_scenario_depth(depth)?;
    if iterations == Some(0) {
        anyhow::bail!("scenario iterations must be greater than zero");
    }
    Ok(ScenarioProfile {
        domain: domain.to_string(),
        depth: depth.to_string(),
        exploration_budget: iterations.unwrap_or(exploration_budget),
        evals,
        evals_interval,
    })
}

fn scenario_title(target: &str, dimension: Dimension, focus: &str) -> String {
    match focus.trim().to_ascii_lowercase().as_str() {
        "security" => format!("{} threat in {}", dimension.label(), target),
        "scale" => format!("{} scaling pressure in {}", dimension.label(), target),
        "failures" | "failure" => format!("{} failure path in {}", dimension.label(), target),
        _ => format!("{} edge case in {}", dimension.label(), target),
    }
}

fn scenario_expected(
    format: ScenarioFormat,
    dimension: Dimension,
    target: &str,
    focus: &str,
) -> String {
    let description = dimension.description();
    match format {
        ScenarioFormat::UseCase => format!(
            "Document how {target} should behave when {description} appears, including the fallback and user-visible result."
        ),
        ScenarioFormat::UserStory => format!(
            "As an affected user, I need {target} to handle {description} so the workflow remains recoverable."
        ),
        ScenarioFormat::TestScenario => format!(
            "Add a focused test for {description}; assert that {target} returns a bounded, observable, and reversible result."
        ),
        ScenarioFormat::ThreatScenario => format!(
            "Model how an attacker or failure source could exploit {description}; verify {target} preserves the {focus} boundary."
        ),
    }
}

fn render_scenario_markdown(
    target: &str,
    format: ScenarioFormat,
    focus: &str,
    scope: &[String],
    profile: &ScenarioProfile,
) -> String {
    let mut out = String::new();
    let scope_items = if scope.is_empty() {
        vec!["DECISION NEEDED: identify implementation scope.".to_string()]
    } else {
        scope.to_vec()
    };
    let scope_lines = scope_items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    writeln!(out, "# Scenario Exploration: {target}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "> Auto-generated scenario matrix. Use it to seed tests, threat modeling, debug hunts, or follow-up autoresearch runs."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Summary").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Format: {} ({})",
        scenario_format_slug(format),
        format.label()
    )
    .unwrap();
    writeln!(out, "- Domain: {}", profile.domain).unwrap();
    writeln!(out, "- Focus: {focus}").unwrap();
    writeln!(out, "- Depth: {}", profile.depth).unwrap();
    writeln!(out, "- Exploration budget: {}", profile.exploration_budget).unwrap();
    writeln!(out, "- Evals enabled: {}", profile.evals).unwrap();
    writeln!(
        out,
        "- Evals interval: {}",
        profile
            .evals_interval
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    )
    .unwrap();
    writeln!(out, "- Dimensions: {}", Dimension::all().len()).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scenario Matrix").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| # | Dimension | Scenario | Expected Investigation |").unwrap();
    writeln!(out, "|---|---|---|---|").unwrap();

    for (index, dimension) in Dimension::all().iter().enumerate() {
        let title = scenario_title(target, *dimension, focus);
        let expected = scenario_expected(format, *dimension, target, focus);
        writeln!(
            out,
            "| {} | {} | {} | {} |",
            index + 1,
            dimension.label(),
            title,
            expected
        )
        .unwrap();
    }

    writeln!(out).unwrap();
    writeln!(out, "## Follow-Up").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Convert high-severity rows into tests or `/autoresearch:debug` hypotheses."
    )
    .unwrap();
    writeln!(
        out,
        "- Use `autoresearch plan --goal \"cover {target} scenario gaps\"` to derive a metric and verify command."
    )
    .unwrap();
    writeln!(out, "- DECISION NEEDED: choose severity labels and owners.").unwrap();
    out
}

fn cmd_scenario(
    target: &str,
    domain: &str,
    format: &str,
    focus: &str,
    scope: Vec<String>,
    depth: &str,
    iterations: Option<u32>,
    evals: bool,
    evals_interval: Option<u32>,
    debug: bool,
    chain: Option<String>,
    output: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let format = parse_scenario_format(format)?;
    let profile = resolve_scenario_profile(domain, depth, iterations, evals, evals_interval)?;
    let forced_targets = if debug { &["debug"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let workspace = resolve_workspace_root(cwd);
    let output = output.unwrap_or_else(|| {
        default_artifact_path("scenario", format!("scenario-{}.md", slugify(target)))
    });
    let output = resolve_workspace_path(&workspace, output);

    let markdown = render_scenario_markdown(target, format, focus, &scope, &profile);
    write_text_file(&output, &markdown)?;
    let handoff_path = if !chain_targets.is_empty() || profile.evals {
        let handoff_path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("handoff.json");
        let next_target = next_chain_target_value(&chain_targets);
        let handoff = serde_json::json!({
            "version": "2.1.0",
            "source": "scenario",
            "source_command": "scenario",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "COMPLETE",
            "report": output.display().to_string(),
            "handoff_path": handoff_path.display().to_string(),
            "findings": [],
            "config": {
                "target": target,
                "domain": profile.domain.as_str(),
                "format": scenario_format_slug(format),
                "focus": focus,
                "scope": scope,
                "depth": profile.depth.as_str(),
                "exploration_budget": profile.exploration_budget,
                "dimensions": Dimension::all().len(),
            },
            "chain": chain_targets,
            "next_target": next_target,
            "chain_continue": should_continue_handoff_chain("COMPLETE"),
            "propagate_evals": profile.evals,
            "evals_interval": profile.evals_interval,
        });
        write_json_file(&handoff_path, &handoff)?;
        Some(handoff_path)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "path": output.display().to_string(),
            "handoff_path": handoff_path.as_ref().map(|path| path.display().to_string()),
            "target": target,
            "domain": profile.domain.as_str(),
            "format": scenario_format_slug(format),
            "depth": profile.depth.as_str(),
            "exploration_budget": profile.exploration_budget,
            "evals": profile.evals,
            "evals_interval": profile.evals_interval,
            "dimensions": Dimension::all().len(),
        })
    );
    Ok(())
}

fn predict_persona_recommendation(persona: Persona) -> &'static str {
    match persona {
        Persona::Architect => "Keep module boundaries explicit, add an ADR if the proposal changes ownership, and avoid coupling unrelated paths.",
        Persona::SecurityExpert => "List trust boundaries, inputs, secrets, and permission checks before implementation; add a guard for the riskiest path.",
        Persona::PerformanceEngineer => "Define the latency, memory, or throughput budget and include a repeatable benchmark or lightweight proxy metric.",
        Persona::UxDesigner => "Map the first successful user workflow and verify that errors remain understandable and recoverable.",
        Persona::DevilsAdvocate => "Write the rollback plan, the smallest reversible first step, and the condition that should stop the work.",
    }
}

fn predict_persona_risk(persona: Persona) -> &'static str {
    match persona {
        Persona::Architect => {
            "Hidden shared-state or ownership changes make the design harder to reverse."
        }
        Persona::SecurityExpert => {
            "A convenience path could bypass validation, authorization, or secret handling."
        }
        Persona::PerformanceEngineer => {
            "The proposal may improve the happy path while increasing tail latency or resource use."
        }
        Persona::UxDesigner => {
            "The implementation may satisfy the metric while leaving the target workflow confusing."
        }
        Persona::DevilsAdvocate => {
            "Success criteria may be too vague to distinguish a real improvement from churn."
        }
    }
}

#[derive(Debug, Clone)]
struct PredictReviewProfile {
    depth: String,
    adversarial: bool,
    personas: u8,
    rounds: u8,
    budget: u32,
    fail_on: Option<Severity>,
    incremental: bool,
    confirmed_findings: usize,
    gate_failed: bool,
}

impl PredictReviewProfile {
    fn risk_level(&self) -> &'static str {
        if self.adversarial {
            "high"
        } else {
            "medium"
        }
    }
}

fn parse_predict_depth(value: &str) -> Result<(&'static str, u8, u8)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "shallow" | "quick" => Ok(("shallow", 3, 1)),
        "standard" | "normal" => Ok(("standard", 5, 2)),
        "deep" | "comprehensive" => Ok(("deep", 8, 3)),
        other => anyhow::bail!("Invalid predict depth {other:?}; use shallow, standard, or deep"),
    }
}

fn resolve_predict_profile(
    depth: &str,
    adversarial: bool,
    personas: Option<u8>,
    rounds: Option<u8>,
    budget: Option<u32>,
    fail_on: Option<String>,
    incremental: bool,
) -> Result<PredictReviewProfile> {
    let (depth, default_personas, default_rounds) = parse_predict_depth(depth)?;
    let personas = personas.unwrap_or(default_personas);
    if !(3..=8).contains(&personas) {
        anyhow::bail!("predict personas must be between 3 and 8");
    }
    let rounds = rounds.unwrap_or(default_rounds);
    if !(1..=3).contains(&rounds) {
        anyhow::bail!("predict rounds must be between 1 and 3");
    }
    let budget = budget.unwrap_or(40);
    if budget == 0 {
        anyhow::bail!("predict budget must be greater than zero");
    }
    let fail_on = parse_security_severity(fail_on.as_deref(), "--fail-on")?;
    let confirmed_findings = 0usize;
    let gate_failed = fail_on.is_some() && confirmed_findings > 0;
    Ok(PredictReviewProfile {
        depth: depth.to_string(),
        adversarial,
        personas,
        rounds,
        budget,
        fail_on,
        incremental,
        confirmed_findings,
        gate_failed,
    })
}

fn render_predict_markdown(
    proposal: &str,
    scope: &[String],
    profile: &PredictReviewProfile,
) -> String {
    let mut out = String::new();
    let scope_items = if scope.is_empty() {
        vec!["DECISION NEEDED: identify implementation scope.".to_string()]
    } else {
        scope.to_vec()
    };
    let scope_lines = scope_items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    writeln!(out, "# Predict Review: {proposal}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "> Auto-generated multi-persona review. Treat this as pre-implementation risk shaping, not final approval."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Review Profile").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Depth: {}", profile.depth).unwrap();
    writeln!(out, "- Requested personas: {}", profile.personas).unwrap();
    writeln!(out, "- Debate rounds: {}", profile.rounds).unwrap();
    writeln!(out, "- Findings budget: {}", profile.budget).unwrap();
    writeln!(out, "- Adversarial: {}", profile.adversarial).unwrap();
    writeln!(out, "- Incremental: {}", profile.incremental).unwrap();
    writeln!(
        out,
        "- Fail-on: {}",
        profile
            .fail_on
            .map(|severity| severity.label())
            .unwrap_or("none")
    )
    .unwrap();
    writeln!(out, "- Gate failed: {}", profile.gate_failed).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Persona Findings").unwrap();

    for persona in Persona::all() {
        writeln!(out).unwrap();
        writeln!(out, "### {}", persona.title()).unwrap();
        writeln!(out).unwrap();
        writeln!(out, "- Focus: {}", persona.focus()).unwrap();
        writeln!(out, "- Primary risk: {}", predict_persona_risk(*persona)).unwrap();
        writeln!(
            out,
            "- Recommendation: {}",
            predict_persona_recommendation(*persona)
        )
        .unwrap();
    }

    writeln!(out).unwrap();
    writeln!(out, "## Synthesis").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Agreement: the proposal needs explicit scope, mechanical verification, and a rollback path before implementation."
    )
    .unwrap();
    writeln!(
        out,
        "- Disagreement to resolve: whether the highest risk is architectural coupling, security exposure, performance cost, or user confusion."
    )
    .unwrap();
    writeln!(
        out,
        "- Next step: run `autoresearch scenario --target \"{proposal}\" --format test-scenarios` for concrete edge cases."
    )
    .unwrap();
    writeln!(
        out,
        "- DECISION NEEDED: choose the primary success metric and the guard command."
    )
    .unwrap();
    out
}

fn cmd_predict(
    proposal: &str,
    scope: Vec<String>,
    depth: String,
    adversarial: bool,
    personas: Option<u8>,
    rounds: Option<u8>,
    budget: Option<u32>,
    fail_on: Option<String>,
    incremental: bool,
    debug: bool,
    chain: Option<String>,
    evals: bool,
    evals_interval: Option<u32>,
    output: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    validate_chain_evals_flags("predict", evals, evals_interval)?;
    let profile = resolve_predict_profile(
        &depth,
        adversarial,
        personas,
        rounds,
        budget,
        fail_on,
        incremental,
    )?;
    let forced_targets = if debug { &["debug"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let workspace = resolve_workspace_root(cwd);
    let output = output.unwrap_or_else(|| {
        default_artifact_path("predict", format!("predict-{}.md", slugify(proposal)))
    });
    let output = resolve_workspace_path(&workspace, output);

    let markdown = render_predict_markdown(proposal, &scope, &profile);
    write_text_file(&output, &markdown)?;
    let handoff_path = if !chain_targets.is_empty() || evals {
        let handoff_path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("handoff.json");
        let next_target = next_chain_target_value(&chain_targets);
        let handoff = serde_json::json!({
            "version": "2.1.0",
            "source": "predict",
            "source_command": "predict",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "COMPLETE",
            "report": output.display().to_string(),
            "handoff_path": handoff_path.display().to_string(),
            "findings": [],
            "config": {
                "proposal": proposal,
                "scope": scope,
                "depth": profile.depth.clone(),
                "adversarial": profile.adversarial,
                "personas": profile.personas,
                "rounds": profile.rounds,
                "budget": profile.budget,
                "fail_on": profile.fail_on.map(|severity| severity.label()),
                "incremental": profile.incremental,
                "confirmed_findings": profile.confirmed_findings,
                "gate_failed": profile.gate_failed,
                "built_in_personas": Persona::all().len(),
                "risk_level": profile.risk_level(),
            },
            "chain": chain_targets,
            "next_target": next_target,
            "chain_continue": should_continue_handoff_chain("COMPLETE"),
            "propagate_evals": evals,
            "evals_interval": evals_interval,
        });
        write_json_file(&handoff_path, &handoff)?;
        Some(handoff_path)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "path": output.display().to_string(),
            "handoff_path": handoff_path.as_ref().map(|path| path.display().to_string()),
            "proposal": proposal,
            "depth": profile.depth.clone(),
            "adversarial": profile.adversarial,
            "personas": profile.personas,
            "rounds": profile.rounds,
            "budget": profile.budget,
            "fail_on": profile.fail_on.map(|severity| severity.label()),
            "incremental": profile.incremental,
            "gate_failed": profile.gate_failed,
            "risk_level": profile.risk_level(),
        })
    );
    Ok(())
}

fn parse_reasoning_mode(value: &str) -> Result<ReasoningMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "convergent" | "converge" => Ok(ReasoningMode::Convergent),
        "creative" | "divergent" => Ok(ReasoningMode::Creative),
        "debate" | "adversarial" => Ok(ReasoningMode::Debate),
        other => {
            anyhow::bail!("Invalid reason mode {other:?}; use convergent, creative, or debate")
        }
    }
}

fn reasoning_mode_label(mode: ReasoningMode) -> &'static str {
    match mode {
        ReasoningMode::Convergent => "Convergent",
        ReasoningMode::Creative => "Creative",
        ReasoningMode::Debate => "Debate",
    }
}

fn parse_reason_domain(value: &str) -> Result<ReasonDomain> {
    match value.trim().to_ascii_lowercase().as_str() {
        "software" | "engineering" => Ok(ReasonDomain::Software),
        "product" => Ok(ReasonDomain::Product),
        "business" => Ok(ReasonDomain::Business),
        "security" => Ok(ReasonDomain::Security),
        "research" => Ok(ReasonDomain::Research),
        "content" | "writing" => Ok(ReasonDomain::Content),
        other => anyhow::bail!(
            "Invalid reason domain {other:?}; use software, product, business, security, research, or content"
        ),
    }
}

fn reason_candidate_rows(
    question: &str,
    mode: ReasoningMode,
) -> Vec<(&'static str, String, &'static str)> {
    match mode {
        ReasoningMode::Creative => vec![
            (
                "A",
                format!("Explore a high-upside alternative for {question}"),
                "Maximizes differentiation but needs tighter risk controls.",
            ),
            (
                "B",
                format!("Combine two smaller mechanisms before committing to {question}"),
                "Balances novelty with reversibility.",
            ),
            (
                "C",
                format!("Prototype the riskiest assumption behind {question}"),
                "Finds unknowns early, but may not ship user value immediately.",
            ),
        ],
        ReasoningMode::Convergent => vec![
            (
                "A",
                format!("Pick the smallest measurable path for {question}"),
                "Best when speed and evidence matter more than breadth.",
            ),
            (
                "B",
                format!(
                    "Delay implementation until the metric and guard for {question} are explicit"
                ),
                "Reduces churn, but may slow momentum.",
            ),
            (
                "C",
                format!("Run a parallel compare before choosing how to handle {question}"),
                "Costs more upfront, but produces direct evidence.",
            ),
        ],
        ReasoningMode::Debate => vec![
            (
                "A",
                format!("Conservative implementation of {question}"),
                "Lowest blast radius, strongest rollback story.",
            ),
            (
                "B",
                format!("Parallel experiment for competing approaches to {question}"),
                "Higher evidence quality, more setup cost.",
            ),
            (
                "C",
                format!("Broad redesign around {question}"),
                "Potentially highest payoff, highest integration risk.",
            ),
        ],
    }
}

#[derive(Debug, Clone)]
struct ReasonProfile {
    iteration_budget: u32,
    judges: u8,
    convergence: u8,
    judge_personas: Vec<String>,
    synthesis: bool,
    temperature: Option<String>,
}

fn parse_reason_judge_personas(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|persona| !persona.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn resolve_reason_profile(
    mode: ReasoningMode,
    iterations: Option<u32>,
    judges: Option<u8>,
    convergence: Option<u8>,
    judge_personas: Option<&str>,
    no_synthesis: bool,
    temperature: Option<String>,
) -> Result<ReasonProfile> {
    if iterations == Some(0) {
        anyhow::bail!("reason iterations must be greater than zero");
    }
    let judges = judges.unwrap_or(3);
    if !(3..=7).contains(&judges) {
        anyhow::bail!("reason judges must be between 3 and 7");
    }
    let convergence = convergence.unwrap_or(3);
    if convergence == 0 {
        anyhow::bail!("reason convergence must be greater than zero");
    }
    if let Some(value) = temperature.as_deref() {
        value
            .parse::<f32>()
            .with_context(|| format!("invalid reason temperature {value:?}"))?;
    }
    Ok(ReasonProfile {
        iteration_budget: iterations.unwrap_or(8),
        judges,
        convergence,
        judge_personas: parse_reason_judge_personas(judge_personas),
        synthesis: !no_synthesis && mode != ReasoningMode::Debate,
        temperature,
    })
}

fn render_reason_markdown(
    question: &str,
    mode: ReasoningMode,
    domain: ReasonDomain,
    scope: &[String],
    profile: &ReasonProfile,
) -> String {
    let mut out = String::new();
    let scope_items = if scope.is_empty() {
        vec!["DECISION NEEDED: identify implementation scope.".to_string()]
    } else {
        scope.to_vec()
    };
    let scope_lines = scope_items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    writeln!(out, "# Reason Debate: {question}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "> Auto-generated adversarial reasoning artifact. Use it before implementation when the right answer is uncertain."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Setup").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Mode: {}", reasoning_mode_label(mode)).unwrap();
    writeln!(out, "- Domain: {}", domain.label()).unwrap();
    writeln!(out, "- Iteration budget: {}", profile.iteration_budget).unwrap();
    writeln!(out, "- Panel size: {} blind judges", profile.judges).unwrap();
    writeln!(
        out,
        "- Convergence threshold: {} matching winning rounds",
        profile.convergence
    )
    .unwrap();
    writeln!(out, "- Synthesis enabled: {}", profile.synthesis).unwrap();
    writeln!(
        out,
        "- Temperature hint: {}",
        profile.temperature.as_deref().unwrap_or("default")
    )
    .unwrap();
    writeln!(
        out,
        "- Judge personas: {}",
        if profile.judge_personas.is_empty() {
            "default blind panel".to_string()
        } else {
            profile.judge_personas.join(", ")
        }
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Candidate Solutions").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Candidate | Approach | Main Tradeoff |").unwrap();
    writeln!(out, "|---|---|---|").unwrap();
    for (id, title, tradeoff) in reason_candidate_rows(question, mode) {
        writeln!(out, "| {id} | {title} | {tradeoff} |").unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "## Blind Judge Rubric").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Correctness: can the answer satisfy the stated problem?"
    )
    .unwrap();
    writeln!(
        out,
        "- Verifiability: can an autoresearch metric prove progress?"
    )
    .unwrap();
    writeln!(
        out,
        "- Reversibility: can a failed trial be rolled back cleanly?"
    )
    .unwrap();
    writeln!(out, "- Risk: what breaks if the candidate is wrong?").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Initial Verdict").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "DECISION NEEDED: run at least one human or agent judge round before selecting a winner."
    )
    .unwrap();
    writeln!(
        out,
        "Default recommendation: choose Candidate A unless evidence justifies the added cost of B or C."
    )
    .unwrap();
    out
}

fn cmd_reason(
    question: &str,
    mode: &str,
    domain: &str,
    iterations: Option<u32>,
    scope: Vec<String>,
    judges: Option<u8>,
    convergence: Option<u8>,
    judge_personas: Option<String>,
    no_synthesis: bool,
    temperature: Option<String>,
    predict: bool,
    chain: Option<String>,
    evals: bool,
    evals_interval: Option<u32>,
    output: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    validate_chain_evals_flags("reason", evals, evals_interval)?;
    let forced_targets = if predict { &["predict"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let mode = parse_reasoning_mode(mode)?;
    let domain = parse_reason_domain(domain)?;
    let profile = resolve_reason_profile(
        mode,
        iterations,
        judges,
        convergence,
        judge_personas.as_deref(),
        no_synthesis,
        temperature,
    )?;
    let workspace = resolve_workspace_root(cwd);
    let output = output.unwrap_or_else(|| {
        default_artifact_path("reason", format!("reason-{}.md", slugify(question)))
    });
    let output = resolve_workspace_path(&workspace, output);

    let markdown = render_reason_markdown(question, mode, domain, &scope, &profile);
    write_text_file(&output, &markdown)?;
    let handoff_path = if !chain_targets.is_empty() || evals {
        let handoff_path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("handoff.json");
        let next_target = next_chain_target_value(&chain_targets);
        let handoff = serde_json::json!({
            "version": "2.1.0",
            "source": "reason",
            "source_command": "reason",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "CONVERGED",
            "report": output.display().to_string(),
            "handoff_path": handoff_path.display().to_string(),
            "findings": [],
            "config": {
                "question": question,
                "mode": reasoning_mode_label(mode).to_ascii_lowercase(),
                "domain": domain.label(),
                "scope": scope,
                "iteration_budget": profile.iteration_budget,
                "judges": profile.judges,
                "convergence": profile.convergence,
                "judge_personas": profile.judge_personas.clone(),
                "synthesis": profile.synthesis,
                "temperature": profile.temperature.clone(),
                "candidates": 3,
            },
            "chain": chain_targets,
            "next_target": next_target,
            "chain_continue": should_continue_handoff_chain("CONVERGED"),
            "propagate_evals": evals,
            "evals_interval": evals_interval,
        });
        write_json_file(&handoff_path, &handoff)?;
        Some(handoff_path)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "path": output.display().to_string(),
            "handoff_path": handoff_path.as_ref().map(|path| path.display().to_string()),
            "question": question,
            "mode": reasoning_mode_label(mode).to_ascii_lowercase(),
            "domain": domain.label(),
            "iteration_budget": profile.iteration_budget,
            "judges": profile.judges,
            "convergence": profile.convergence,
            "judge_personas": profile.judge_personas.clone(),
            "synthesis": profile.synthesis,
            "temperature": profile.temperature.clone(),
            "candidates": 3,
        })
    );
    Ok(())
}

fn probe_persona_question(persona: ProbePersona, subject: &str) -> String {
    match persona {
        ProbePersona::EndUser => {
            format!("What does a successful first use of {subject} look like for the target user?")
        }
        ProbePersona::EdgeCaseHunter => {
            format!(
                "Which unusual inputs, boundaries, or workflow interruptions can break {subject}?"
            )
        }
        ProbePersona::SecurityAnalyst => {
            format!("What data, authorization, or trust boundary does {subject} touch?")
        }
        ProbePersona::PerformanceTester => {
            format!("What latency, throughput, or resource limit should {subject} stay within?")
        }
        ProbePersona::BusinessAnalyst => {
            format!("Which business outcome makes {subject} worth shipping now?")
        }
        ProbePersona::Skeptic => {
            format!("Which assumption behind {subject} would invalidate the approach if false?")
        }
        ProbePersona::ComplianceOfficer => {
            format!("Which privacy, retention, consent, or audit requirement constrains {subject}?")
        }
        ProbePersona::DevOpsEngineer => {
            format!("How should {subject} be deployed, monitored, and rolled back?")
        }
    }
}

#[derive(Debug, Clone)]
struct ProbeProfile {
    mode: String,
    depth: String,
    rounds: u32,
    personas: u8,
    adversarial: bool,
    saturation_threshold: u8,
}

fn parse_probe_mode(value: &str) -> Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "interactive" | "ask" => Ok("interactive"),
        "autonomous" | "auto" => Ok("autonomous"),
        other => anyhow::bail!("Invalid probe mode {other:?}; use interactive or autonomous"),
    }
}

fn parse_probe_depth(value: &str) -> Result<(&'static str, u32)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "shallow" | "quick" => Ok(("shallow", 5)),
        "standard" | "normal" => Ok(("standard", 15)),
        "deep" | "comprehensive" => Ok(("deep", 30)),
        other => anyhow::bail!("Invalid probe depth {other:?}; use shallow, standard, or deep"),
    }
}

fn resolve_probe_profile(
    mode: &str,
    depth: &str,
    iterations: Option<u32>,
    personas: Option<u8>,
    adversarial: bool,
    saturation_threshold: Option<u8>,
) -> Result<ProbeProfile> {
    let mode = parse_probe_mode(mode)?;
    let (depth, rounds) = parse_probe_depth(depth)?;
    if iterations == Some(0) {
        anyhow::bail!("probe iterations must be greater than zero");
    }
    let personas = personas.unwrap_or(6);
    if !(3..=8).contains(&personas) {
        anyhow::bail!("probe personas must be between 3 and 8");
    }
    let saturation_threshold = saturation_threshold.unwrap_or(2);
    if saturation_threshold == 0 {
        anyhow::bail!("probe saturation threshold must be greater than zero");
    }
    Ok(ProbeProfile {
        mode: mode.to_string(),
        depth: depth.to_string(),
        rounds: iterations.unwrap_or(rounds),
        personas,
        adversarial,
        saturation_threshold,
    })
}

fn render_probe_markdown(subject: &str, scope: &[String], profile: &ProbeProfile) -> String {
    let mut out = String::new();
    let scope_items = if scope.is_empty() {
        vec!["DECISION NEEDED: identify implementation scope.".to_string()]
    } else {
        scope.to_vec()
    };
    let scope_lines = scope_items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");

    writeln!(out, "# Requirement Probe: {subject}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "> Auto-generated requirement interrogation artifact. Answer these before turning the work into an autoresearch loop."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Probe Profile").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Mode: {}", profile.mode).unwrap();
    writeln!(out, "- Depth: {}", profile.depth).unwrap();
    writeln!(out, "- Rounds: {}", profile.rounds).unwrap();
    writeln!(out, "- Active personas: {}", profile.personas).unwrap();
    writeln!(out, "- Adversarial: {}", profile.adversarial).unwrap();
    writeln!(
        out,
        "- Saturation threshold: {}",
        profile.saturation_threshold
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Persona Questions").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Persona | Focus | Question | Constraint Slot |").unwrap();
    writeln!(out, "|---|---|---|---|").unwrap();
    for persona in ProbePersona::all() {
        writeln!(
            out,
            "| {} | {} | {} | DECISION NEEDED |",
            persona.title(),
            persona.focus(),
            probe_persona_question(*persona, subject)
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "## Saturation Rule").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Stop probing after {} consecutive rounds add fewer than {} new constraints.",
        profile.saturation_threshold, profile.saturation_threshold
    )
    .unwrap();
    writeln!(
        out,
        "- Convert confirmed constraints into acceptance criteria, required keep labels, or guard commands."
    )
    .unwrap();
    writeln!(
        out,
        "- DECISION NEEDED: mark must-have constraints before implementation."
    )
    .unwrap();
    out
}

fn cmd_probe(
    subject: &str,
    scope: Vec<String>,
    mode: String,
    depth: String,
    iterations: Option<u32>,
    personas: Option<u8>,
    adversarial: bool,
    saturation_threshold: Option<u8>,
    plan: bool,
    chain: Option<String>,
    evals: bool,
    evals_interval: Option<u32>,
    output: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    validate_chain_evals_flags("probe", evals, evals_interval)?;
    let profile = resolve_probe_profile(
        &mode,
        &depth,
        iterations,
        personas,
        adversarial,
        saturation_threshold,
    )?;
    let forced_targets = if plan { &["plan"][..] } else { &[][..] };
    let chain_targets = chain_targets_with_forced(chain.as_deref(), forced_targets)?;
    let workspace = resolve_workspace_root(cwd);
    let output = output.unwrap_or_else(|| {
        default_artifact_path("probe", format!("probe-{}.md", slugify(subject)))
    });
    let output = resolve_workspace_path(&workspace, output);

    let markdown = render_probe_markdown(subject, &scope, &profile);
    write_text_file(&output, &markdown)?;
    let handoff_path = if !chain_targets.is_empty() || evals {
        let handoff_path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("handoff.json");
        let next_target = next_chain_target_value(&chain_targets);
        let handoff = serde_json::json!({
            "version": "2.1.0",
            "source": "probe",
            "source_command": "probe",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "SATURATED",
            "report": output.display().to_string(),
            "handoff_path": handoff_path.display().to_string(),
            "findings": [],
            "config": {
                "subject": subject,
                "scope": scope,
                "mode": profile.mode.clone(),
                "depth": profile.depth.clone(),
                "rounds": profile.rounds,
                "personas": profile.personas,
                "adversarial": profile.adversarial,
                "saturation_threshold": profile.saturation_threshold,
                "built_in_personas": ProbePersona::all().len(),
                "saturation_rule": format!("{} consecutive rounds below {} new constraints", profile.saturation_threshold, profile.saturation_threshold),
            },
            "chain": chain_targets,
            "next_target": next_target,
            "chain_continue": should_continue_handoff_chain("SATURATED"),
            "propagate_evals": evals,
            "evals_interval": evals_interval,
        });
        write_json_file(&handoff_path, &handoff)?;
        Some(handoff_path)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "path": output.display().to_string(),
            "handoff_path": handoff_path.as_ref().map(|path| path.display().to_string()),
            "subject": subject,
            "mode": profile.mode.clone(),
            "depth": profile.depth.clone(),
            "rounds": profile.rounds,
            "personas": profile.personas,
            "adversarial": profile.adversarial,
            "saturation_threshold": profile.saturation_threshold,
            "saturation_rule": format!("{} consecutive rounds below {} new constraints", profile.saturation_threshold, profile.saturation_threshold),
        })
    );
    Ok(())
}

fn parse_learn_sub_mode(value: &str) -> Result<LearnSubMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "init" => Ok(LearnSubMode::Init),
        "update" => Ok(LearnSubMode::Update),
        "check" => Ok(LearnSubMode::Check),
        "summarize" | "summary" => Ok(LearnSubMode::Summarize),
        other => {
            anyhow::bail!("Invalid learn mode {other:?}; use init, update, check, or summarize")
        }
    }
}

fn learn_sub_mode_label(mode: LearnSubMode) -> &'static str {
    match mode {
        LearnSubMode::Init => "init",
        LearnSubMode::Update => "update",
        LearnSubMode::Check => "check",
        LearnSubMode::Summarize => "summarize",
    }
}

#[derive(Debug, Clone)]
struct LearnProfile {
    depth: String,
    iteration_budget: u32,
    scan_limit: usize,
    inventory_limit: usize,
    scan: bool,
    topics: Vec<String>,
    auto_fix: bool,
    format: String,
    evals: bool,
    evals_interval: Option<u32>,
}

fn parse_learn_depth(value: &str) -> Result<(&'static str, usize, u32)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "overview" | "shallow" | "quick" => Ok(("overview", 10, 5)),
        "standard" | "normal" => Ok(("standard", 25, 10)),
        "comprehensive" | "deep" => Ok(("comprehensive", 50, 20)),
        other => {
            anyhow::bail!("Invalid learn depth {other:?}; use overview, standard, or comprehensive")
        }
    }
}

fn parse_learn_format(value: &str) -> Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "markdown" | "md" => Ok("markdown"),
        "json" => Ok("json"),
        "rst" | "restructuredtext" | "restructured-text" => Ok("rst"),
        other => anyhow::bail!("Invalid learn format {other:?}; use markdown, json, or rst"),
    }
}

fn parse_learn_topics(topics: Option<&str>) -> Vec<String> {
    topics
        .unwrap_or("all")
        .split(',')
        .map(str::trim)
        .filter(|topic| !topic.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn resolve_learn_profile(
    depth: &str,
    iterations: Option<u32>,
    scan: bool,
    topics: Option<&str>,
    no_fix: bool,
    format: &str,
    evals: bool,
    evals_interval: Option<u32>,
) -> Result<LearnProfile> {
    validate_chain_evals_flags("learn", evals, evals_interval)?;
    let (depth, scan_limit, iteration_budget) = parse_learn_depth(depth)?;
    if iterations == Some(0) {
        anyhow::bail!("learn iterations must be greater than zero");
    }
    let format = parse_learn_format(format)?;
    let mut topics = parse_learn_topics(topics);
    if topics.is_empty() {
        topics.push("all".to_string());
    }
    Ok(LearnProfile {
        depth: depth.to_string(),
        iteration_budget: iterations.unwrap_or(iteration_budget),
        scan_limit,
        inventory_limit: scan_limit.min(25),
        scan,
        topics,
        auto_fix: !no_fix,
        format: format.to_string(),
        evals,
        evals_interval,
    })
}

fn collect_learn_files(
    workspace: &Path,
    scope: &[String],
    explicit_files: &[String],
    limit: usize,
) -> Vec<String> {
    let mut patterns = if scope.is_empty() && explicit_files.is_empty() {
        vec![
            "README.md".to_string(),
            "src/**/*".to_string(),
            "docs/**/*.md".to_string(),
        ]
    } else {
        scope.to_vec()
    };
    patterns.extend(explicit_files.iter().cloned());
    let mut files = BTreeSet::new();
    for pattern in patterns {
        if files.len() >= limit {
            break;
        }
        let full = format!("{}/{}", workspace.display(), pattern);
        if let Ok(entries) = glob::glob(&full) {
            for entry in entries.flatten().filter(|path| path.is_file()) {
                let rel = entry.strip_prefix(workspace).unwrap_or(&entry);
                files.insert(rel.display().to_string());
                if files.len() >= limit {
                    break;
                }
            }
        }
    }
    files.into_iter().collect()
}

fn render_learn_summary(
    mode: LearnSubMode,
    files: &[String],
    scope: &[String],
    explicit_files: &[String],
    profile: &LearnProfile,
) -> String {
    let mut out = String::new();
    let scope_lines = if scope.is_empty() && explicit_files.is_empty() {
        "- README.md\n- src/**/*\n- docs/**/*.md".to_string()
    } else {
        scope
            .iter()
            .chain(explicit_files.iter())
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let topics = profile.topics.join(", ");
    writeln!(out, "# Learn Summary").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Mode: {}", learn_sub_mode_label(mode)).unwrap();
    writeln!(out, "- Depth: {}", profile.depth).unwrap();
    writeln!(out, "- Iteration budget: {}", profile.iteration_budget).unwrap();
    writeln!(out, "- Format: {}", profile.format).unwrap();
    writeln!(out, "- Topics: {topics}").unwrap();
    writeln!(out, "- Fresh scan requested: {}", profile.scan).unwrap();
    writeln!(out, "- Auto-fix enabled: {}", profile.auto_fix).unwrap();
    writeln!(out, "- Evals enabled: {}", profile.evals).unwrap();
    writeln!(
        out,
        "- Evals interval: {}",
        profile
            .evals_interval
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    )
    .unwrap();
    writeln!(out, "- Files scanned: {}", files.len()).unwrap();
    writeln!(out, "- Validation status: not run").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## Scope").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "{scope_lines}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## File Inventory").unwrap();
    writeln!(out).unwrap();
    if files.is_empty() {
        writeln!(
            out,
            "- DECISION NEEDED: no files matched the requested scope."
        )
        .unwrap();
    } else {
        for file in files.iter().take(profile.inventory_limit) {
            writeln!(out, "- {file}").unwrap();
        }
        if files.len() > profile.inventory_limit {
            writeln!(out, "- ... {} more", files.len() - profile.inventory_limit).unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(out, "## Documentation Next Steps").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "- Fill missing module, command, and artifact descriptions for the highest-traffic files first."
    )
    .unwrap();
    writeln!(
        out,
        "- Validate examples and links before treating generated docs as complete."
    )
    .unwrap();
    writeln!(
        out,
        "- DECISION NEEDED: choose the documentation acceptance metric."
    )
    .unwrap();
    out
}

fn render_learn_validation(files: &[String], profile: &LearnProfile) -> String {
    let mut out = String::new();
    writeln!(out, "# Learn Validation Report").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- Files considered: {}", files.len()).unwrap();
    writeln!(out, "- Issues found: DECISION NEEDED").unwrap();
    writeln!(out, "- Issues fixed: 0").unwrap();
    writeln!(out, "- Auto-fix enabled: {}", profile.auto_fix).unwrap();
    writeln!(out, "- Format: {}", profile.format).unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "Run project-specific doc tests, link checks, or examples before closing this learn pass."
    )
    .unwrap();
    out
}

fn render_learn_results_tsv(files: &[String], profile: &LearnProfile) -> String {
    let mut out = String::new();
    let timestamp = chrono::Utc::now().to_rfc3339();
    writeln!(out, "# metric_direction: higher_is_better").unwrap();
    writeln!(
        out,
        "iteration\ttimestamp\tfile_documented\tvalidation_status\tissues_found\tissues_fixed\tdescription"
    )
    .unwrap();
    for (index, file) in files.iter().take(profile.inventory_limit).enumerate() {
        writeln!(
            out,
            "{}\t{}\t{}\tpending\t0\t0\tinventory",
            index + 1,
            timestamp,
            file
        )
        .unwrap();
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn cmd_learn(
    mode: &str,
    scope: Vec<String>,
    depth: &str,
    iterations: Option<u32>,
    explicit_files: Vec<String>,
    scan: bool,
    topics: Option<String>,
    no_fix: bool,
    format: &str,
    chain: Option<String>,
    evals: bool,
    evals_interval: Option<u32>,
    output_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let mode = parse_learn_sub_mode(mode)?;
    let profile = resolve_learn_profile(
        depth,
        iterations,
        scan,
        topics.as_deref(),
        no_fix,
        format,
        evals,
        evals_interval,
    )?;
    let chain_targets = chain_targets_with_forced(chain.as_deref(), &[])?;
    let next_target = next_chain_target_value(&chain_targets);
    let workspace = resolve_workspace_root(cwd);
    let output_dir = output_dir.unwrap_or_else(|| {
        default_artifact_path("learn", format!("learn-{}", learn_sub_mode_label(mode)))
    });
    let output_dir = resolve_workspace_path(&workspace, output_dir);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let files = collect_learn_files(&workspace, &scope, &explicit_files, profile.scan_limit);

    write_text_file(
        &output_dir.join("summary.md"),
        &render_learn_summary(mode, &files, &scope, &explicit_files, &profile),
    )?;
    write_text_file(
        &output_dir.join("validation-report.md"),
        &render_learn_validation(&files, &profile),
    )?;
    write_text_file(
        &output_dir.join("learn-results.tsv"),
        &render_learn_results_tsv(&files, &profile),
    )?;
    let handoff = serde_json::json!({
        "version": "2.1.0",
        "source": "learn",
        "source_command": "learn",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "COMPLETE",
        "results_tsv": output_dir.join("learn-results.tsv").display().to_string(),
        "findings": [],
        "config": {
            "mode": learn_sub_mode_label(mode),
            "scope": scope,
            "files": explicit_files,
            "depth": profile.depth.as_str(),
            "iteration_budget": profile.iteration_budget,
            "scan_limit": profile.scan_limit,
            "scan": profile.scan,
            "topics": profile.topics.clone(),
            "auto_fix": profile.auto_fix,
            "format": profile.format.as_str(),
            "files_scanned": files.len(),
        },
        "chain": chain_targets.clone(),
        "next_target": next_target.clone(),
        "chain_continue": should_continue_handoff_chain("COMPLETE"),
        "propagate_evals": profile.evals,
        "evals_interval": profile.evals_interval,
    });
    write_json_file(&output_dir.join("handoff.json"), &handoff)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "written",
            "output_dir": output_dir.display().to_string(),
            "mode": learn_sub_mode_label(mode),
            "depth": profile.depth.as_str(),
            "iteration_budget": profile.iteration_budget,
            "format": profile.format.as_str(),
            "topics": profile.topics.clone(),
            "auto_fix": profile.auto_fix,
            "evals": profile.evals,
            "evals_interval": profile.evals_interval,
            "next_target": next_target,
            "files_scanned": files.len(),
        })
    );
    Ok(())
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

fn cmd_workspace_exec(
    command: &str,
    rollback_on_failure: bool,
    cwd: Option<PathBuf>,
) -> Result<()> {
    verify::screen_command(command).context("unsafe workspace exec command")?;
    let workspace = resolve_results_workspace(cwd);
    let run_context = load_run_context(&workspace)?;
    let targets = prepare_workspace_exec_targets(&run_context)?;
    let mut results = Vec::new();
    let mut attempted = Vec::new();
    let mut failure = None;

    for target in &targets {
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&target.path)
            .env("AUTORESEARCH_REPO_PATH", &target.path)
            .env("AUTORESEARCH_REPO_ROLE", &target.role)
            .env("AUTORESEARCH_REPO_SCOPE", &target.scope)
            .output()
            .with_context(|| {
                format!(
                    "failed to run workspace exec command in {}",
                    target.path.display()
                )
            })?;
        let success = output.status.success();
        attempted.push(target.clone());
        results.push(serde_json::json!({
            "path": target.path.display().to_string(),
            "role": &target.role,
            "scope": &target.scope,
            "head_before": &target.head,
            "exit_code": output.status.code(),
            "success": success,
            "stdout": String::from_utf8_lossy(&output.stdout).trim_end().to_string(),
            "stderr": String::from_utf8_lossy(&output.stderr).trim_end().to_string(),
        }));
        if !success {
            failure = Some(format!(
                "workspace exec failed in {} with {}",
                target.path.display(),
                output.status
            ));
            break;
        }
    }

    let mut rolled_back = false;
    if failure.is_some() && rollback_on_failure {
        rollback_workspace_exec_targets(&attempted)?;
        rolled_back = true;
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": failure.is_none(),
            "rolled_back": rolled_back,
            "command": command,
            "repo_results": results,
        }))?
    );

    if let Some(message) = failure {
        anyhow::bail!(message);
    }
    Ok(())
}

#[derive(Clone)]
struct WorkspaceExecTarget {
    path: PathBuf,
    role: String,
    scope: String,
    head: String,
}

fn prepare_workspace_exec_targets(
    run_context: &context::RunContext,
) -> Result<Vec<WorkspaceExecTarget>> {
    let mut targets = Vec::new();
    for target in &run_context.repo_targets {
        let path = PathBuf::from(&target.path);
        let repo = GitRepo::open(&path)
            .with_context(|| format!("workspace target {} is not a git repository", target.path))?;
        if repo.head_detached()? {
            anyhow::bail!("workspace target {} is detached_head", target.path);
        }
        match repo.worktree_status()? {
            WorktreeStatus::Clean | WorktreeStatus::OnlyArtifacts => {}
            WorktreeStatus::Dirty(paths) => {
                anyhow::bail!(
                    "workspace target {} has unexpected worktree changes: {}",
                    target.path,
                    paths.join(", ")
                );
            }
        }
        targets.push(WorkspaceExecTarget {
            path,
            role: target.role.clone(),
            scope: target.scope.clone(),
            head: repo.head_full()?,
        });
    }
    Ok(targets)
}

fn rollback_workspace_exec_targets(targets: &[WorkspaceExecTarget]) -> Result<()> {
    for target in targets.iter().rev() {
        let reset = Command::new("git")
            .arg("-C")
            .arg(&target.path)
            .arg("reset")
            .arg("--hard")
            .arg(&target.head)
            .output()
            .with_context(|| format!("failed to reset {}", target.path.display()))?;
        if !reset.status.success() {
            anyhow::bail!(
                "failed to reset {}: {}",
                target.path.display(),
                String::from_utf8_lossy(&reset.stderr).trim()
            );
        }
        let clean = Command::new("git")
            .arg("-C")
            .arg(&target.path)
            .arg("clean")
            .arg("-fd")
            .output()
            .with_context(|| format!("failed to clean {}", target.path.display()))?;
        if !clean.status.success() {
            anyhow::bail!(
                "failed to clean {}: {}",
                target.path.display(),
                String::from_utf8_lossy(&clean.stderr).trim()
            );
        }
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
    let environment_summary = resolve_environment_summary(
        &workspace,
        environment_summary.or(project_config.environment_summary.clone()),
    );
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

#[derive(Debug, Clone, Copy)]
enum VerifyAggregate {
    Median,
    Mean,
    Min,
    Max,
    Last,
}

fn parse_verify_aggregate(value: &str) -> Result<VerifyAggregate> {
    match value {
        "median" => Ok(VerifyAggregate::Median),
        "mean" => Ok(VerifyAggregate::Mean),
        "min" => Ok(VerifyAggregate::Min),
        "max" => Ok(VerifyAggregate::Max),
        "last" => Ok(VerifyAggregate::Last),
        other => {
            anyhow::bail!("Unknown verify aggregate: {other}. Use median, mean, min, max, or last.")
        }
    }
}

fn aggregate_verify_samples(samples: &[Decimal], aggregate: VerifyAggregate) -> Decimal {
    match aggregate {
        VerifyAggregate::Median => {
            let mut sorted = samples.to_vec();
            sorted.sort();
            let midpoint = sorted.len() / 2;
            if sorted.len() % 2 == 1 {
                sorted[midpoint]
            } else {
                (sorted[midpoint - 1] + sorted[midpoint]) / Decimal::from(2_u32)
            }
        }
        VerifyAggregate::Mean => {
            let mut total = Decimal::ZERO;
            for sample in samples {
                total += *sample;
            }
            total / Decimal::from(samples.len() as u64)
        }
        VerifyAggregate::Min => *samples.iter().min().expect("samples must not be empty"),
        VerifyAggregate::Max => *samples.iter().max().expect("samples must not be empty"),
        VerifyAggregate::Last => *samples.last().expect("samples must not be empty"),
    }
}

fn cmd_verify(
    command: &str,
    format_str: &str,
    key: Option<&str>,
    repeat: usize,
    aggregate_str: &str,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_cwd(cwd);
    let fmt = parse_format(format_str)?;
    let aggregate = parse_verify_aggregate(aggregate_str)?;
    if repeat == 0 {
        anyhow::bail!("verify repeat must be greater than zero");
    }
    if repeat > 1 && fmt == VerifyFormat::MetricsJson {
        anyhow::bail!("repeated verify currently supports scalar format only");
    }

    verify::screen_command(command)?;
    let mut results = Vec::with_capacity(repeat);
    for _ in 0..repeat {
        results.push(verify::run_verify(command, fmt, key, &workspace)?);
    }
    let samples = results
        .iter()
        .map(|result| result.metric)
        .collect::<Vec<_>>();
    let metric = aggregate_verify_samples(&samples, aggregate);
    let last_result = results.last().expect("repeat must be greater than zero");
    let duration_ms = results
        .iter()
        .map(|result| result.duration.as_millis())
        .sum::<u128>();

    let out = serde_json::json!({
        "metric": metric.to_string(),
        "metrics": last_result.metrics.as_ref().map(|metrics| {
            metrics
                .iter()
                .map(|(key, value)| (key.clone(), value.to_string()))
                .collect::<std::collections::BTreeMap<_, _>>()
        }),
        "repeat": repeat,
        "aggregate": aggregate_str,
        "samples": samples.iter().map(|sample| sample.to_string()).collect::<Vec<_>>(),
        "exit_code": last_result.exit_code,
        "duration_ms": duration_ms,
        "stdout_tail": last_result.stdout.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
        "stderr_tail": last_result.stderr.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
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

fn cmd_evals(
    path: Option<PathBuf>,
    format: &str,
    recommend: bool,
    plateau_window: u32,
    chain: Option<&str>,
    compare: Option<&Path>,
    target: Option<&str>,
) -> Result<()> {
    if plateau_window == 0 {
        anyhow::bail!("evals plateau window must be greater than zero");
    }
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
    let goal_target = target
        .map(|value| {
            Decimal::from_str(value).with_context(|| format!("Invalid evals target {value:?}"))
        })
        .transpose()?;
    let goal_achieved = goal_target.map(|target| match direction {
        "lower" => final_metric <= target,
        _ => final_metric >= target,
    });
    let recommendation = evals_recommendation(
        longest_plateau,
        crashes,
        keeps,
        efficiency,
        total_iterations,
        trend,
        plateau_window,
        goal_achieved,
    );
    let go_no_go = evals_go_no_go(recommendation);
    let next_step = evals_next_step(recommendation);
    let plateau_detected = longest_plateau >= plateau_window;
    let anomalies = evals_anomalies(
        plateau_detected,
        longest_plateau,
        plateau_window,
        trend,
        guard_failures,
        guard_failed_improvements,
        longest_failure_streak,
    );
    let summary_dir = tsv_path.parent().unwrap_or_else(|| Path::new("."));
    let comparison = if let Some(compare_path) = compare {
        let compare_content = std::fs::read_to_string(compare_path)
            .with_context(|| format!("Cannot read {}", compare_path.display()))?;
        Some(evals_comparison(
            compare_path,
            &compare_content,
            direction,
            improvement,
            efficiency,
            longest_plateau,
        )?)
    } else {
        None
    };
    let chain_targets = parse_handoff_chain_targets(chain)?;
    let handoff_path = if chain_targets.is_empty() {
        None
    } else {
        let handoff_path = summary_dir.join("handoff.json");
        let next_target = next_chain_target_value(&chain_targets);
        let workspace = std::env::current_dir()?;
        let (primary_repo, repo_targets) = handoff_context_values(summary_dir)?;
        let mut findings: Vec<serde_json::Value> = anomalies
            .iter()
            .map(|anomaly| serde_json::json!(anomaly))
            .collect();
        if findings.is_empty() {
            findings.push(serde_json::json!({
                "type": "recommendation",
                "severity": "info",
                "message": recommendation,
            }));
        }

        let handoff = serde_json::json!({
            "version": "2.1.0",
            "protocol_version": "2.1.0",
            "binary_version": env!("CARGO_PKG_VERSION"),
            "source": "evals",
            "source_command": "evals",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "COMPLETE",
            "workspace_root": workspace.display().to_string(),
            "artifact_root": summary_dir.display().to_string(),
            "primary_repo": primary_repo,
            "repo_targets": repo_targets,
            "results_tsv": tsv_path.display().to_string(),
            "results_path": tsv_path.display().to_string(),
            "handoff_path": handoff_path.display().to_string(),
            "summary": {
                "direction": direction,
                "total_iterations": total_iterations,
                "keeps": keeps,
                "discards": discards,
                "crashes": crashes,
                "guard_failures": guard_failures,
                "baseline": baseline.to_string(),
                "final": final_metric.to_string(),
                "best": best.to_string(),
                "improvement": improvement.to_string(),
                "improvement_pct": improvement_pct.as_deref(),
                "efficiency_pct": efficiency,
                "longest_plateau": longest_plateau,
                "plateau_window": plateau_window,
                "plateau_detected": plateau_detected,
                "trend": trend,
                "recommendation": recommendation,
                "go_no_go": go_no_go,
                "next_step": next_step,
                "goal_target": goal_target.as_ref().map(ToString::to_string),
                "goal_achieved": goal_achieved,
                "comparison": &comparison,
                "anomalies": &anomalies,
            },
            "findings": findings,
            "config": {
                "format": format,
                "recommend": recommend,
                "plateau_window": plateau_window,
                "target": goal_target.as_ref().map(ToString::to_string),
                "unknown_columns": &unknown_columns,
                "comparison": &comparison,
            },
            "chain": chain_targets.clone(),
            "next_target": next_target,
            "chain_continue": should_continue_handoff_chain("COMPLETE"),
            "propagate_evals": false,
            "evals_interval": serde_json::Value::Null,
        });
        write_json_file(&handoff_path, &handoff)?;
        Some(handoff_path)
    };

    match format {
        "json" => {
            let mut out = serde_json::json!({
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
                "plateau_window": plateau_window,
                "plateau_detected": plateau_detected,
                "goal_target": goal_target.as_ref().map(ToString::to_string),
                "goal_achieved": goal_achieved,
                "trend": trend,
                "recommendation": recommendation,
                "unknown_columns": &unknown_columns,
                "parallel_workers": &parallel_workers,
                "comparison": &comparison,
                "anomalies": &anomalies,
                "top_improvements": top_keeps.iter().take(5).map(|(d, desc)| {
                    serde_json::json!({"delta": d.to_string(), "description": desc})
                }).collect::<Vec<_>>(),
                "top_regressions": top_regressions.iter().take(5).map(|(d, desc)| {
                    serde_json::json!({"delta": d.to_string(), "description": desc})
                }).collect::<Vec<_>>(),
            });
            if recommend {
                if let Some(object) = out.as_object_mut() {
                    object.insert(
                        "go_no_go".to_string(),
                        serde_json::Value::String(go_no_go.to_string()),
                    );
                    object.insert(
                        "next_step".to_string(),
                        serde_json::Value::String(next_step.to_string()),
                    );
                }
            }
            if !chain_targets.is_empty() {
                if let Some(object) = out.as_object_mut() {
                    object.insert("chain".to_string(), serde_json::json!(&chain_targets));
                    object.insert(
                        "next_target".to_string(),
                        next_chain_target_value(&chain_targets),
                    );
                    object.insert(
                        "chain_continue".to_string(),
                        serde_json::Value::Bool(should_continue_handoff_chain("COMPLETE")),
                    );
                    object.insert(
                        "handoff_path".to_string(),
                        serde_json::Value::String(
                            handoff_path
                                .as_ref()
                                .expect("chain handoff path must exist when targets are present")
                                .display()
                                .to_string(),
                        ),
                    );
                }
            }
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
                plateau_window,
                goal_target,
                goal_achieved,
                recommendation,
                recommend,
                unknown_columns: &unknown_columns,
                parallel_workers: &parallel_workers,
                top_keeps: &top_keeps,
                top_regressions: &top_regressions,
                comparison: comparison.as_ref(),
                anomalies: &anomalies,
                chain_targets: &chain_targets,
                handoff_path: handoff_path.as_deref(),
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
                plateau_window,
                goal_target,
                goal_achieved,
                recommendation,
                recommend,
                unknown_columns: &unknown_columns,
                parallel_workers: &parallel_workers,
                top_keeps: &top_keeps,
                top_regressions: &top_regressions,
                comparison: comparison.as_ref(),
                anomalies: &anomalies,
                chain_targets: &chain_targets,
                handoff_path: handoff_path.as_deref(),
            });
            print!("{report}");
        }
        other => anyhow::bail!("Invalid evals format {other:?}; use text, json, or md"),
    }

    Ok(())
}

fn checkpoint_interval(state: &RunState, interval: Option<u32>) -> Result<u32> {
    if interval == Some(0) {
        anyhow::bail!("checkpoint interval must be greater than zero");
    }
    Ok(interval.unwrap_or_else(|| {
        state
            .config
            .as_ref()
            .and_then(|config| config.iterations)
            .map(|iterations| std::cmp::max(1, iterations / 3))
            .unwrap_or(10)
    }))
}

fn cmd_checkpoint(cwd: Option<PathBuf>, interval: Option<u32>, format: &str) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    if !state_path.exists() {
        anyhow::bail!("No active run (state.json not found)");
    }
    let state: RunState = serde_json::from_str(&std::fs::read_to_string(&state_path)?)?;
    let interval = checkpoint_interval(&state, interval)?;
    let due = state.iteration > 0 && state.iteration % interval == 0;
    if !due {
        let next_iteration = if state.iteration == 0 {
            interval
        } else {
            ((state.iteration / interval) + 1) * interval
        };
        let out = serde_json::json!({
            "status": "skipped",
            "iteration": state.iteration,
            "interval": interval,
            "next_checkpoint_iteration": next_iteration,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    cmd_evals(
        Some(results_dir.join("results.tsv")),
        format,
        false,
        5,
        None,
        None,
        None,
    )
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
    plateau_window: u32,
    goal_target: Option<Decimal>,
    goal_achieved: Option<bool>,
    recommendation: &'a str,
    recommend: bool,
    unknown_columns: &'a [String],
    top_keeps: &'a [(Decimal, &'a str)],
    top_regressions: &'a [(Decimal, &'a str)],
    parallel_workers: &'a ParallelWorkerStats,
    comparison: Option<&'a EvalsComparison>,
    anomalies: &'a [EvalsAnomaly],
    chain_targets: &'a [String],
    handoff_path: Option<&'a Path>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EvalsAnomaly {
    kind: &'static str,
    severity: &'static str,
    message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EvalsComparison {
    compared_path: String,
    compared_iterations: usize,
    compared_keeps: usize,
    compared_baseline: String,
    compared_final: String,
    compared_improvement: String,
    compared_improvement_pct: Option<String>,
    compared_efficiency_pct: u32,
    compared_longest_plateau: u32,
    winner: &'static str,
    improvement_delta: String,
    efficiency_delta: i32,
    plateau_delta: i32,
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
    if let Some(target) = report.goal_target {
        writeln!(out, "| Goal target | {} |", target).unwrap();
    }
    if let Some(achieved) = report.goal_achieved {
        writeln!(out, "| Goal achieved | {} |", achieved).unwrap();
    }
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

    if let Some(comparison) = report.comparison {
        writeln!(out, "### Comparison").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "- Compared run: {}", comparison.compared_path).unwrap();
        writeln!(out, "- Winner: {}", comparison.winner).unwrap();
        writeln!(
            out,
            "- Compared improvement: {} ({})",
            comparison.compared_improvement,
            format_optional_percent(comparison.compared_improvement_pct.as_deref())
        )
        .unwrap();
        writeln!(out, "- Improvement delta: {}", comparison.improvement_delta).unwrap();
        writeln!(out, "- Efficiency delta: {}%", comparison.efficiency_delta).unwrap();
        writeln!(out, "- Plateau delta: {}", comparison.plateau_delta).unwrap();
        writeln!(out).unwrap();
    }

    if !report.anomalies.is_empty() {
        writeln!(out, "### Anomalies").unwrap();
        writeln!(out).unwrap();
        for anomaly in report.anomalies {
            writeln!(
                out,
                "- **{}** ({}) - {}",
                anomaly.kind, anomaly.severity, anomaly.message
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    writeln!(out, "### Recommendations").unwrap();
    writeln!(out).unwrap();
    if report.longest_plateau >= report.plateau_window {
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
    if report.recommend {
        writeln!(out).unwrap();
        writeln!(out, "### Go / No-Go").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "- Decision: {}", evals_go_no_go(report.recommendation)).unwrap();
        writeln!(
            out,
            "- Next step: {}",
            evals_next_step(report.recommendation)
        )
        .unwrap();
    }
    if let Some(handoff_path) = report.handoff_path {
        writeln!(out).unwrap();
        writeln!(out, "### Chain Handoff").unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "- Next target: {}",
            report
                .chain_targets
                .first()
                .map(String::as_str)
                .unwrap_or("none")
        )
        .unwrap();
        writeln!(out, "- Handoff: {}", handoff_path.display()).unwrap();
    }
    out
}

fn format_optional_percent(value: Option<&str>) -> String {
    value
        .map(|pct| format!("{pct}%"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn evals_anomalies(
    plateau_detected: bool,
    longest_plateau: u32,
    plateau_window: u32,
    trend: &str,
    guard_failures: usize,
    guard_failed_improvements: usize,
    longest_failure_streak: u32,
) -> Vec<EvalsAnomaly> {
    let mut anomalies = Vec::new();

    if plateau_detected {
        anomalies.push(EvalsAnomaly {
            kind: "plateau",
            severity: "medium",
            message: format!(
                "{longest_plateau} consecutive non-keep iterations met the plateau window of {plateau_window}"
            ),
        });
    }
    if longest_failure_streak >= 3 {
        anomalies.push(EvalsAnomaly {
            kind: "failure_streak",
            severity: "medium",
            message: format!("{longest_failure_streak} consecutive discard/crash iterations"),
        });
    }
    if guard_failures > 0 {
        anomalies.push(EvalsAnomaly {
            kind: "guard_failures",
            severity: "high",
            message: format!("{guard_failures} guard failure(s) recorded"),
        });
    }
    if guard_failed_improvements > 0 {
        anomalies.push(EvalsAnomaly {
            kind: "guard_failed_improvements",
            severity: "high",
            message: format!(
                "{guard_failed_improvements} metric improvement(s) were rejected by guard failures"
            ),
        });
    }
    if trend == "declining" {
        anomalies.push(EvalsAnomaly {
            kind: "declining_trend",
            severity: "medium",
            message: "Recent kept metrics are declining".to_string(),
        });
    }

    anomalies
}

fn evals_comparison(
    compared_path: &Path,
    compared_content: &str,
    expected_direction: &str,
    primary_improvement: Decimal,
    primary_efficiency: u32,
    primary_longest_plateau: u32,
) -> Result<EvalsComparison> {
    let compared_direction = evals_direction(compared_content)?;
    if compared_direction != expected_direction {
        anyhow::bail!(
            "evals compare direction mismatch: primary is {expected_direction}, comparison is {compared_direction}"
        );
    }

    let compared_metrics = parse_results_tsv(compared_content)?;
    if compared_metrics.is_empty() {
        anyhow::bail!("No main data rows in comparison results TSV.");
    }

    let compared_total = compared_metrics.len();
    let compared_has_baseline = compared_metrics
        .first()
        .is_some_and(|row| row.iteration == 0 || row.status == "baseline");
    let compared_iterations = compared_total.saturating_sub(usize::from(compared_has_baseline));
    let compared_keeps = compared_metrics
        .iter()
        .filter(|row| is_keep_status(&row.status))
        .count();
    let compared_baseline = compared_metrics
        .first()
        .map(|row| row.metric)
        .unwrap_or_default();
    let compared_final = compared_metrics
        .last()
        .map(|row| row.metric)
        .unwrap_or_default();
    let compared_improvement = if expected_direction == "higher" {
        compared_final - compared_baseline
    } else {
        compared_baseline - compared_final
    };
    let compared_improvement_pct = (compared_baseline != Decimal::ZERO).then(|| {
        ((compared_improvement / compared_baseline.abs()) * Decimal::from(100))
            .round_dp(2)
            .to_string()
    });
    let compared_efficiency = if compared_iterations > 0 {
        (compared_keeps as f64 / compared_iterations as f64 * 100.0).round() as u32
    } else {
        0
    };
    let compared_longest_plateau = longest_evals_plateau(&compared_metrics);
    let winner = if primary_improvement > compared_improvement {
        "primary"
    } else if primary_improvement < compared_improvement {
        "comparison"
    } else {
        "tie"
    };

    Ok(EvalsComparison {
        compared_path: compared_path.display().to_string(),
        compared_iterations,
        compared_keeps,
        compared_baseline: compared_baseline.to_string(),
        compared_final: compared_final.to_string(),
        compared_improvement: compared_improvement.to_string(),
        compared_improvement_pct,
        compared_efficiency_pct: compared_efficiency,
        compared_longest_plateau,
        winner,
        improvement_delta: (primary_improvement - compared_improvement).to_string(),
        efficiency_delta: primary_efficiency as i32 - compared_efficiency as i32,
        plateau_delta: primary_longest_plateau as i32 - compared_longest_plateau as i32,
    })
}

fn longest_evals_plateau(metrics: &[ParsedRow]) -> u32 {
    let mut longest_plateau = 0u32;
    let mut current_plateau = 0u32;
    for row in metrics {
        if !is_keep_status(&row.status) && row.status != "baseline" {
            current_plateau += 1;
            longest_plateau = longest_plateau.max(current_plateau);
        } else {
            current_plateau = 0;
        }
    }
    longest_plateau
}

fn evals_recommendation(
    longest_plateau: u32,
    crashes: usize,
    keeps: usize,
    efficiency: u32,
    total_iterations: usize,
    trend: &str,
    plateau_window: u32,
    goal_achieved: Option<bool>,
) -> &'static str {
    if goal_achieved == Some(true) {
        "goal_met"
    } else if longest_plateau >= plateau_window
        || trend == "declining"
        || (efficiency < 20 && total_iterations > 10)
    {
        "change_strategy"
    } else if crashes > keeps {
        "check_verify"
    } else {
        "continue"
    }
}

fn evals_go_no_go(recommendation: &str) -> &'static str {
    match recommendation {
        "continue" | "goal_met" => "GO",
        "check_verify" => "HOLD",
        _ => "NO-GO",
    }
}

fn evals_next_step(recommendation: &str) -> &'static str {
    match recommendation {
        "continue" => "Continue the current approach; keep eval checkpoints enabled.",
        "goal_met" => "Goal met; stop the loop or hand off to ship.",
        "check_verify" => "Stabilize the verify or guard command before continuing.",
        _ => "Stop this line of attack and pivot to a new hypothesis or scope.",
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

fn env_disk_free_mb(path: &Path) -> Option<u64> {
    let output = Command::new("df").arg("-Pk").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().nth(1)?;
    let available_kb = line.split_whitespace().nth(3)?.parse::<u64>().ok()?;
    Some(available_kb / 1024)
}

fn binary_available(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[derive(Debug)]
struct EnvironmentProbe {
    cpu_cores: usize,
    free_mb: Option<u64>,
    in_container: bool,
    toolchains: BTreeMap<String, bool>,
    recommended_parallel_workers: usize,
    summary: String,
}

fn build_environment_probe(workspace: &Path) -> EnvironmentProbe {
    let cpu_cores = std::thread::available_parallelism()
        .map(|cores| cores.get())
        .unwrap_or(1);
    let free_mb = env_disk_free_mb(workspace);
    let in_container = Path::new("/.dockerenv").exists() || std::env::var("container").is_ok();
    let mut toolchains = BTreeMap::new();
    for binary in [
        "cargo", "rustc", "python3", "node", "npm", "go", "java", "codex",
    ] {
        toolchains.insert(binary.to_string(), binary_available(binary));
    }
    let present_toolchains = toolchains
        .iter()
        .filter_map(|(binary, present)| present.then_some(binary.as_str()))
        .collect::<Vec<_>>();
    let recommended_parallel_workers = std::cmp::min(3, std::cmp::max(1, cpu_cores / 2));
    let summary = format!(
        "cpu={} disk_mb={} container={} toolchains={}",
        cpu_cores,
        free_mb
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        if in_container { "yes" } else { "no" },
        if present_toolchains.is_empty() {
            "none".to_string()
        } else {
            present_toolchains.join(",")
        }
    );
    EnvironmentProbe {
        cpu_cores,
        free_mb,
        in_container,
        toolchains,
        recommended_parallel_workers,
        summary,
    }
}

fn resolve_environment_summary(workspace: &Path, summary: Option<String>) -> Option<String> {
    summary.map(|value| {
        if value.trim().eq_ignore_ascii_case("auto") {
            build_environment_probe(workspace).summary
        } else {
            value
        }
    })
}

fn cmd_env(cwd: Option<PathBuf>, format: &str) -> Result<()> {
    let workspace = resolve_workspace_root(cwd);
    let probe = build_environment_probe(&workspace);
    let out = serde_json::json!({
        "workspace": workspace.display().to_string(),
        "cpu_cores": probe.cpu_cores,
        "free_mb": probe.free_mb,
        "container": probe.in_container,
        "toolchains": probe.toolchains,
        "recommended_parallel_workers": probe.recommended_parallel_workers,
        "environment_summary": probe.summary,
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&out)?),
        "text" => {
            println!("--- Environment Probe ---");
            println!("Workspace: {}", workspace.display());
            println!("CPU cores: {}", probe.cpu_cores);
            println!(
                "Free disk MB: {}",
                probe
                    .free_mb
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "Container: {}",
                if probe.in_container { "yes" } else { "no" }
            );
            println!(
                "Recommended parallel workers: {}",
                probe.recommended_parallel_workers
            );
            println!("Toolchains:");
            for (binary, present) in probe.toolchains {
                println!(
                    "  {binary}: {}",
                    if present { "present" } else { "missing" }
                );
            }
            println!("Environment summary: {}", probe.summary);
        }
        other => anyhow::bail!("Invalid env format {other:?}; use json or text"),
    }
    Ok(())
}

fn protocol_fingerprint_items() -> Vec<&'static str> {
    vec![
        "baseline before init",
        "log every completed experiment before the next one starts",
        "the autoresearch binary owns authoritative TSV/JSON updates and keep/stop gating",
        "artifact paths come from workspace_root + autoresearch-results/ and repo-local pointer",
        "background decisions come from runtime run or runtime supervise",
        "current stop conditions",
        "current rollback strategy",
        "active pivot/refine escalation thresholds",
        "selected mode workflow deviation from default loop",
    ]
}

fn canonical_display(path: PathBuf) -> String {
    path.canonicalize().unwrap_or(path).display().to_string()
}

fn run_phase_label(phase: &RunPhase) -> &'static str {
    match phase {
        RunPhase::Setup => "setup",
        RunPhase::Baseline { .. } => "baseline",
        RunPhase::Iterating { .. } => "iterating",
        RunPhase::Complete { .. } => "complete",
        RunPhase::Blocked { .. } => "blocked",
    }
}

fn reanchor_reference_checks(
    workspace: &Path,
    context: &context::RunContext,
) -> Vec<serde_json::Value> {
    let expected_results = canonical_display(workspace.join("autoresearch-results/results.tsv"));
    let expected_state = canonical_display(workspace.join("autoresearch-results/state.json"));
    vec![
        serde_json::json!({
            "name": "context_results_path",
            "ok": context.results_path == expected_results,
            "expected": expected_results,
            "actual": context.results_path,
        }),
        serde_json::json!({
            "name": "context_state_path",
            "ok": context.state_path == expected_state,
            "expected": expected_state,
            "actual": context.state_path,
        }),
        serde_json::json!({
            "name": "runtime_hard_invariants_reference",
            "ok": RUNTIME_HARD_INVARIANTS_DOC.contains("Protocol Fingerprint Check"),
            "reference": "references/runtime-hard-invariants.md",
        }),
        serde_json::json!({
            "name": "core_principles_reference",
            "ok": CORE_PRINCIPLES_DOC.contains("One Change Per Iteration"),
            "reference": "references/core-principles.md",
        }),
        serde_json::json!({
            "name": "selected_mode_workflow_reference",
            "ok": LOOP_WORKFLOW_DOC.contains("Iterate toward a measurable outcome"),
            "reference": "references/loop-workflow.md",
        }),
    ]
}

fn render_reanchor_text(out: &serde_json::Value) -> String {
    let mut text = String::new();
    writeln!(text, "--- Protocol Reanchor ---").unwrap();
    writeln!(
        text,
        "Status: {}",
        out["status"].as_str().unwrap_or("unknown")
    )
    .unwrap();
    writeln!(
        text,
        "Workspace: {}",
        out["workspace"].as_str().unwrap_or("")
    )
    .unwrap();
    writeln!(
        text,
        "Iteration: {}",
        out["iteration"].as_u64().unwrap_or(0)
    )
    .unwrap();
    writeln!(
        text,
        "Due now: {}",
        if out["due"].as_bool().unwrap_or(false) {
            "yes"
        } else {
            "no"
        }
    )
    .unwrap();
    writeln!(
        text,
        "Next due iteration: {}",
        out["next_due_iteration"].as_u64().unwrap_or(10)
    )
    .unwrap();
    writeln!(text, "Reload references:").unwrap();
    if let Some(references) = out["reload_references"].as_array() {
        for reference in references {
            writeln!(text, "  - {}", reference.as_str().unwrap_or("")).unwrap();
        }
    }
    writeln!(text, "Fingerprint items:").unwrap();
    if let Some(items) = out["fingerprint_items"].as_array() {
        for item in items {
            writeln!(text, "  - {}", item.as_str().unwrap_or("")).unwrap();
        }
    }
    if out["due"].as_bool().unwrap_or(false) {
        writeln!(text, "Next logged iteration should include [RE-ANCHOR] if this check required re-reading protocol files.").unwrap();
    }
    text
}

fn cmd_reanchor(cwd: Option<PathBuf>, format: &str) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    let context = load_run_context(&workspace)?;
    let state: RunState = serde_json::from_str(
        &std::fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read {}", state_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", state_path.display()))?;
    let due = state.iteration > 0 && state.iteration % 10 == 0;
    let next_due_iteration = ((state.iteration / 10) + 1) * 10;
    let checks = reanchor_reference_checks(&workspace, &context);
    let status = if checks
        .iter()
        .all(|check| check["ok"].as_bool().unwrap_or(false))
    {
        "ok"
    } else {
        "failed"
    };
    let out = serde_json::json!({
        "status": status,
        "workspace": workspace.display().to_string(),
        "active": context.active,
        "iteration": state.iteration,
        "due": due,
        "next_due_iteration": next_due_iteration,
        "selected_mode": "loop",
        "phase": run_phase_label(&state.phase),
        "last_status": state.last_status.as_str(),
        "fingerprint_name": "Protocol Fingerprint Check",
        "reload_references": [
            "references/runtime-hard-invariants.md",
            "references/core-principles.md",
            "references/loop-workflow.md",
        ],
        "fingerprint_items": protocol_fingerprint_items(),
        "checks": checks,
        "log_tag": "[RE-ANCHOR]",
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&out)?),
        "text" => print!("{}", render_reanchor_text(&out)),
        other => anyhow::bail!("Invalid reanchor format {other:?}; use json or text"),
    }
    if status != "ok" {
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
            hypothesis,
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
                &hypothesis,
            )?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        ParallelCommands::Compare {
            a,
            b,
            worktree_root,
            manifest,
            batch_file,
            branch_prefix,
            cwd,
        } => {
            let workspace = resolve_results_workspace(cwd);
            let hypotheses = vec![format!("A: {a}"), format!("B: {b}")];
            let mut out = cmd_parallel_prepare(
                &workspace,
                2,
                worktree_root,
                manifest,
                batch_file,
                &branch_prefix,
                &hypotheses,
            )?;
            if let Some(object) = out.as_object_mut() {
                object.insert("mode".to_string(), serde_json::json!("ab_compare"));
                object.insert(
                    "arms".to_string(),
                    serde_json::json!([
                        {"worker_id": "a", "hypothesis": a},
                        {"worker_id": "b", "hypothesis": b},
                    ]),
                );
            }
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
    hypotheses: &[String],
) -> Result<serde_json::Value> {
    if !hypotheses.is_empty() && hypotheses.len() != usize::from(workers) {
        anyhow::bail!("--hypothesis must be repeated exactly once per worker");
    }
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
        let prompt_file = write_parallel_worker_prompt(
            &worktree,
            &state,
            &worker_id,
            &branch,
            hypotheses.get(index as usize).map(String::as_str),
        )?;
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
    write_json_file(
        &batch_path,
        &parallel_batch_template_with_hypotheses(workers, hypotheses),
    )?;

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
    parallel_batch_template_with_hypotheses(workers, &[])
}

fn parallel_batch_template_with_hypotheses(
    workers: u8,
    hypotheses: &[String],
) -> serde_json::Value {
    let rows = (0..workers)
        .map(|index| {
            let worker_id = ((b'a' + index) as char).to_string();
            let description = hypotheses
                .get(index as usize)
                .map(|hypothesis| format!("{hypothesis} result summary"))
                .unwrap_or_else(|| format!("worker-{worker_id} result summary"));
            serde_json::json!({
                "worker_id": worker_id,
                "status": "completed",
                "metric": "<required>",
                "metrics": {},
                "guard": "skip",
                "commit": "<required-if-keepable>",
                "description": description,
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

fn metric_history_sparkline(metrics: &[Decimal]) -> Option<String> {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    let min = metrics.iter().copied().min()?;
    let max = metrics.iter().copied().max()?;
    if min == max {
        return Some(std::iter::repeat(LEVELS[0]).take(metrics.len()).collect());
    }

    let span = max - min;
    let max_index = Decimal::from((LEVELS.len() - 1) as u32);
    let sparkline = metrics
        .iter()
        .map(|metric| {
            let index = ((*metric - min) * max_index / span)
                .round()
                .to_usize()
                .unwrap_or(0)
                .min(LEVELS.len() - 1);
            LEVELS[index]
        })
        .collect();
    Some(sparkline)
}

fn progress_trend_from_rows(rows: &[ParsedRow], direction: Direction) -> &'static str {
    let keep_metrics: Vec<Decimal> = rows
        .iter()
        .filter(|row| is_keep_status(&row.status))
        .map(|row| row.metric)
        .collect();
    let last5: Vec<&Decimal> = keep_metrics.iter().rev().take(5).collect();
    if last5.len() < 2 {
        "insufficient_data"
    } else {
        match direction {
            Direction::Lower if last5.windows(2).all(|w| w[0] <= w[1]) => "improving",
            Direction::Lower if last5.windows(2).all(|w| w[0] >= w[1]) => "declining",
            _ if last5.windows(2).all(|w| w[0] >= w[1]) => "improving",
            _ if last5.windows(2).all(|w| w[0] <= w[1]) => "declining",
            _ => "flat",
        }
    }
}

fn retained_metric_history(rows: &[ParsedRow]) -> Vec<Decimal> {
    rows.iter()
        .filter(|row| {
            row.status == "baseline" || row.status == "drift" || is_keep_status(&row.status)
        })
        .map(|row| row.metric)
        .collect()
}

fn parse_cost_decimal(label: &str, value: Option<&str>) -> Result<Option<Decimal>> {
    value
        .map(|raw| {
            let parsed =
                Decimal::from_str(raw.trim()).with_context(|| format!("Invalid {label}: {raw}"))?;
            if parsed < Decimal::ZERO {
                anyhow::bail!("{label} must be non-negative");
            }
            Ok(parsed)
        })
        .transpose()
}

fn estimate_cost_per_iteration(
    per_iteration_usd: Option<&str>,
    input_tokens_per_iteration: Option<u64>,
    output_tokens_per_iteration: Option<u64>,
    input_usd_per_million: Option<&str>,
    output_usd_per_million: Option<&str>,
) -> Result<(Decimal, serde_json::Value)> {
    if let Some(per_iteration_usd) = parse_cost_decimal("per_iteration_usd", per_iteration_usd)? {
        return Ok((
            per_iteration_usd,
            serde_json::json!({
                "method": "direct",
                "per_iteration_usd": per_iteration_usd.to_string(),
            }),
        ));
    }

    let input_tokens = input_tokens_per_iteration.unwrap_or(0);
    let output_tokens = output_tokens_per_iteration.unwrap_or(0);
    if input_tokens == 0 && output_tokens == 0 {
        anyhow::bail!(
            "cost requires --per-iteration-usd or at least one token count with matching USD-per-million rate"
        );
    }

    let input_rate = parse_cost_decimal("input_usd_per_million", input_usd_per_million)?;
    let output_rate = parse_cost_decimal("output_usd_per_million", output_usd_per_million)?;
    if input_tokens > 0 && input_rate.is_none() {
        anyhow::bail!("--input-usd-per-million is required when input tokens are provided");
    }
    if output_tokens > 0 && output_rate.is_none() {
        anyhow::bail!("--output-usd-per-million is required when output tokens are provided");
    }
    let input_rate = input_rate.unwrap_or_default();
    let output_rate = output_rate.unwrap_or_default();

    let one_million = Decimal::from(1_000_000u64);
    let input_cost = Decimal::from(input_tokens) * input_rate / one_million;
    let output_cost = Decimal::from(output_tokens) * output_rate / one_million;
    let per_iteration = input_cost + output_cost;
    Ok((
        per_iteration,
        serde_json::json!({
            "method": "token_rates",
            "input_tokens_per_iteration": input_tokens,
            "output_tokens_per_iteration": output_tokens,
            "input_usd_per_million": input_rate.to_string(),
            "output_usd_per_million": output_rate.to_string(),
            "input_usd_per_iteration": input_cost.to_string(),
            "output_usd_per_iteration": output_cost.to_string(),
        }),
    ))
}

fn cmd_cost(
    cwd: Option<PathBuf>,
    per_iteration_usd: Option<&str>,
    input_tokens_per_iteration: Option<u64>,
    output_tokens_per_iteration: Option<u64>,
    input_usd_per_million: Option<&str>,
    output_usd_per_million: Option<&str>,
    format: &str,
) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    if !state_path.exists() {
        anyhow::bail!("No active run (state.json not found)");
    }

    let state: RunState = serde_json::from_str(&std::fs::read_to_string(&state_path)?)?;
    let (per_iteration, breakdown) = estimate_cost_per_iteration(
        per_iteration_usd,
        input_tokens_per_iteration,
        output_tokens_per_iteration,
        input_usd_per_million,
        output_usd_per_million,
    )?;

    let completed_iterations = state.iteration;
    let configured_iterations = state.config.as_ref().and_then(|config| config.iterations);
    let projected_iterations = configured_iterations
        .unwrap_or(completed_iterations)
        .max(completed_iterations);
    let remaining_iterations = projected_iterations.saturating_sub(completed_iterations);
    let completed_usd = per_iteration * Decimal::from(completed_iterations);
    let remaining_usd = per_iteration * Decimal::from(remaining_iterations);
    let projected_total_usd = per_iteration * Decimal::from(projected_iterations);

    let out = serde_json::json!({
        "workspace": workspace.display().to_string(),
        "completed_iterations": completed_iterations,
        "configured_iterations": configured_iterations,
        "projected_iterations": projected_iterations,
        "remaining_iterations": remaining_iterations,
        "per_iteration_usd": per_iteration.to_string(),
        "completed_usd": completed_usd.to_string(),
        "remaining_usd": remaining_usd.to_string(),
        "projected_total_usd": projected_total_usd.to_string(),
        "breakdown": breakdown,
    });

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&out)?),
        "text" => {
            println!("--- Cost Estimate ---");
            println!("Completed iterations: {}", completed_iterations);
            if let Some(configured_iterations) = configured_iterations {
                println!("Configured iterations: {}", configured_iterations);
                println!("Remaining iterations: {}", remaining_iterations);
            } else {
                println!("Configured iterations: unbounded");
            }
            println!("Per iteration: ${}", per_iteration);
            println!("Completed: ${}", completed_usd);
            println!("Projected total: ${}", projected_total_usd);
            println!("Remaining: ${}", remaining_usd);
        }
        other => anyhow::bail!("Invalid cost format {other:?}; use text or json"),
    }
    Ok(())
}

fn render_dashboard(workspace: &Path, lines: usize) -> Result<String> {
    let results_dir = workspace.join("autoresearch-results");
    let state_path = results_dir.join("state.json");
    if !state_path.exists() {
        anyhow::bail!("No active run (state.json not found)");
    }

    let state: RunState = serde_json::from_str(&std::fs::read_to_string(&state_path)?)?;
    let esc_path = results_dir.join("escalation.json");
    let escalation_label = if esc_path.exists() {
        let esc: EscalationState = serde_json::from_str(&std::fs::read_to_string(&esc_path)?)?;
        format!("{:?}", esc.last_action).to_lowercase()
    } else {
        "none".to_string()
    };

    let tsv_path = results_dir.join("results.tsv");
    let (trend, metric_history, recent_rows) = if tsv_path.exists() {
        let content = std::fs::read_to_string(&tsv_path)?;
        let rows = parse_results_tsv(&content)?;
        let trend = progress_trend_from_rows(&rows, state.direction);
        let metric_history = metric_history_sparkline(&retained_metric_history(&rows));
        let recent_rows = ResultsLog::open(tsv_path)?.tail(lines)?;
        (trend, metric_history, recent_rows)
    } else {
        ("insufficient_data", None, Vec::new())
    };

    let mut out = String::new();
    writeln!(out, "Autoresearch Dashboard").unwrap();
    writeln!(out, "Workspace: {}", workspace.display()).unwrap();
    writeln!(out, "Iteration: {}", state.iteration).unwrap();
    writeln!(
        out,
        "Metric: {} -> {} (best: {} at {})",
        state.baseline_metric, state.current_metric, state.best_metric, state.best_iteration
    )
    .unwrap();
    if let Some(metric_history) = metric_history {
        writeln!(out, "Metric history: {}", metric_history).unwrap();
    }
    writeln!(
        out,
        "Kept: {} | Discarded: {} | Crashes: {} | No-op: {} | Blocked: {}",
        state.keeps, state.discards, state.crashes, state.no_ops, state.blocked
    )
    .unwrap();
    writeln!(
        out,
        "Trend: {} | Consecutive discards: {} | Escalation: {}",
        trend, state.consecutive_discards, escalation_label
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Recent results:").unwrap();
    if recent_rows.is_empty() {
        writeln!(out, "  (none)").unwrap();
    } else {
        for row in recent_rows {
            writeln!(out, "  {row}").unwrap();
        }
    }
    Ok(out)
}

fn cmd_dashboard(cwd: Option<PathBuf>, lines: usize, once: bool, interval_ms: u64) -> Result<()> {
    let workspace = resolve_results_workspace(cwd);
    loop {
        if !once {
            print!("\x1b[2J\x1b[H");
        }
        print!("{}", render_dashboard(&workspace, lines)?);
        std::io::stdout().flush()?;
        if once {
            break;
        }
        std::thread::sleep(Duration::from_millis(interval_ms));
    }
    Ok(())
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
    let mut metric_history = None;
    let trend = if tsv_path.exists() {
        let content = std::fs::read_to_string(&tsv_path)?;
        let rows = parse_results_tsv(&content)?;
        metric_history = metric_history_sparkline(&retained_metric_history(&rows));
        progress_trend_from_rows(&rows, state.direction)
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
    if let Some(metric_history) = metric_history {
        let direction_label = match state.direction {
            Direction::Higher => "higher is better",
            Direction::Lower => "lower is better",
        };
        println!("Metric history: {} ({})", metric_history, direction_label);
    }
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
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn write_text_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
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
    hypothesis: Option<&str>,
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
    let assigned_hypothesis = hypothesis
        .map(|hypothesis| format!("Assigned hypothesis: {hypothesis}\n"))
        .unwrap_or_default();
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
Guard: {guard}\n\
{assigned_hypothesis}\n\
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
