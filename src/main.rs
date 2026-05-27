use anyhow::Result;
use clap::{Parser, Subcommand};

use autoresearch::core::config::Mode;
use autoresearch::hooks;
use autoresearch::modes;

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
    /// Run the autonomous iteration loop
    Loop {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Convert a goal into validated config
    Plan {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Hunt bugs with scientific method
    Debug {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Crush errors one-by-one until zero remain
    Fix {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// STRIDE + OWASP security audit
    Security {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Ship through 8 phases
    Ship {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Generate edge cases across 12 dimensions
    Scenario {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Multi-persona expert debate
    Predict {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Scout and auto-generate docs
    Learn {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Adversarial debate with blind judges
    Reason {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Interrogate requirements until saturation
    Probe {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Analyze iteration results
    Evals {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Non-interactive CI/CD mode
    Exec {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Run a hook (called by Claude Code plugin system)
    Hook {
        /// Hook name to execute
        name: String,
    },
    /// Verify current state and return metric
    Verify {
        /// Verify command to run
        #[arg(long)]
        command: String,
        /// Output format: scalar or metrics_json
        #[arg(long, default_value = "scalar")]
        format: String,
        /// Primary metric key (for metrics_json format)
        #[arg(long)]
        key: Option<String>,
    },
    /// Show run status
    Status,
    /// Initialize distribution files
    Dist {
        /// Target: claude-code or codex
        target: String,
        /// Output directory
        #[arg(long, default_value = "dist")]
        out: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Loop { args } => modes::dispatch(Mode::Loop, &args),
        Commands::Plan { args } => modes::dispatch(Mode::Plan, &args),
        Commands::Debug { args } => modes::dispatch(Mode::Debug, &args),
        Commands::Fix { args } => modes::dispatch(Mode::Fix, &args),
        Commands::Security { args } => modes::dispatch(Mode::Security, &args),
        Commands::Ship { args } => modes::dispatch(Mode::Ship, &args),
        Commands::Scenario { args } => modes::dispatch(Mode::Scenario, &args),
        Commands::Predict { args } => modes::dispatch(Mode::Predict, &args),
        Commands::Learn { args } => modes::dispatch(Mode::Learn, &args),
        Commands::Reason { args } => modes::dispatch(Mode::Reason, &args),
        Commands::Probe { args } => modes::dispatch(Mode::Probe, &args),
        Commands::Evals { args } => modes::dispatch(Mode::Evals, &args),
        Commands::Exec { args } => modes::dispatch(Mode::Exec, &args),
        Commands::Hook { name } => hooks::dispatch(&name),
        Commands::Verify {
            command,
            format,
            key,
        } => {
            let fmt = match format.as_str() {
                "metrics_json" => autoresearch::core::config::VerifyFormat::MetricsJson,
                _ => autoresearch::core::config::VerifyFormat::Scalar,
            };
            let cwd = std::env::current_dir()?;
            let result =
                autoresearch::core::verify::run_verify(&command, fmt, key.as_deref(), &cwd)?;
            println!("{}", serde_json::json!({
                "metric": result.metric.to_string(),
                "exit_code": result.exit_code,
                "duration_ms": result.duration.as_millis(),
            }));
            Ok(())
        }
        Commands::Status => {
            let cwd = std::env::current_dir()?;
            let state_path = cwd.join("autoresearch-results/state.json");
            if state_path.exists() {
                let content = std::fs::read_to_string(&state_path)?;
                println!("{content}");
            } else {
                println!("No active autoresearch run.");
            }
            Ok(())
        }
        Commands::Dist { target, out } => {
            generate_dist(&target, &out)
        }
    }
}

fn generate_dist(target: &str, out_dir: &str) -> Result<()> {
    use autoresearch::agents::claude::ClaudeAdapter;
    use std::fs;
    use std::path::Path;

    let base = Path::new(out_dir);

    match target {
        "claude-code" => {
            let plugin_dir = base.join("claude-code/.claude-plugin");
            let hooks_dir = base.join("claude-code/hooks");

            fs::create_dir_all(&plugin_dir)?;
            fs::create_dir_all(&hooks_dir)?;

            fs::write(
                plugin_dir.join("plugin.json"),
                serde_json::to_string_pretty(&ClaudeAdapter::plugin_json())?,
            )?;
            fs::write(
                hooks_dir.join("hooks.json"),
                serde_json::to_string_pretty(&ClaudeAdapter::hooks_json())?,
            )?;

            eprintln!("Generated Claude Code distribution in {}/claude-code/", out_dir);
        }
        "codex" => {
            let codex_dir = base.join("codex");
            let agents_dir = codex_dir.join("agents");

            fs::create_dir_all(&agents_dir)?;

            fs::write(
                agents_dir.join("openai.yaml"),
                autoresearch::agents::codex::CodexAdapter::agent_yaml(),
            )?;

            eprintln!("Generated Codex distribution in {}/codex/", out_dir);
        }
        _ => anyhow::bail!("Unknown target: {target}. Use 'claude-code' or 'codex'."),
    }

    Ok(())
}
