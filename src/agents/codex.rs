use super::{AgentAdapter, InputMethod};
use serde::{Deserialize, Serialize};

/// Codex CLI skill adapter.
///
/// Commands invoked as plain text: `$autoresearch` or `$autoresearch <mode>`.
/// Supports foreground and background run modes.
/// Uses structured JSON output in exec mode.
pub struct CodexAdapter;

impl AgentAdapter for CodexAdapter {
    fn format_output(&self, content: &str) -> String {
        content.to_string()
    }

    fn command_prefix(&self) -> &str {
        "$autoresearch"
    }

    fn input_method(&self) -> InputMethod {
        InputMethod::RequestUserInput
    }
}

/// Codex exec mode structured output.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecOutput {
    Started {
        run_tag: String,
        mode: String,
        config: serde_json::Value,
    },
    Iteration {
        iteration: u32,
        metric: String,
        delta: String,
        status: String,
        description: String,
    },
    Complete {
        iterations: u32,
        baseline: String,
        final_metric: String,
        best: String,
        keeps: u32,
        discards: u32,
    },
    Blocked {
        iteration: u32,
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
}

impl ExecOutput {
    pub fn emit(&self) {
        let json = serde_json::to_string(self).expect("ExecOutput serialization failed");
        println!("{json}");
    }
}

impl CodexAdapter {
    /// Generate the openai.yaml agent config for Codex.
    pub fn agent_yaml() -> &'static str {
        r##"interface:
  display_name: "Autoresearch"
  short_description: "Autonomous improve-verify loops for Codex"
  brand_color: "#2563EB"
  default_prompt: "Use $autoresearch to improve a measurable goal through verified experiments."

policy:
  allow_implicit_invocation: false

name: autoresearch
description: "Autonomous goal-directed iteration for Codex CLI"
model: codex
tools:
  - type: function
    function:
      name: autoresearch
      description: "Run the autoresearch loop"
      parameters:
        type: object
        properties:
          mode:
            type: string
            enum: [loop, plan, debug, fix, security, ship, scenario, predict, learn, reason, probe, improve, evals, exec]
            description: "Operating mode: loop (core metric iteration), plan (goal to config), debug (bug hunting), fix (error crushing), security (STRIDE and OWASP), ship (8-phase deploy), scenario (edge cases), predict (persona debate), learn (doc generation), reason (adversarial refinement), probe (requirement interrogation), improve (ICP-driven product improvement), evals (results analysis), exec (non-interactive CI/CD)"
          goal:
            type: string
            description: "What to achieve or improve"
          scope:
            type: string
            description: "File globs to operate on"
          metric:
            type: string
            description: "What to measure (for loop mode)"
          verify:
            type: string
            description: "Shell command that outputs metric on final line"
          guard:
            type: string
            description: "Safety command that must exit 0"
          iterations:
            type: integer
            description: "Max iterations (default varies by mode)"
          direction:
            type: string
            enum: [higher_is_better, lower_is_better]
            description: "Metric optimization direction"
          depth:
            type: string
            enum: [shallow, standard, deep]
            description: "Analysis depth (for debug, security, predict, scenario, probe, improve)"
          chain:
            type: string
            description: "Comma-separated downstream command targets"
          evals:
            type: boolean
            description: "Enable mid-loop eval checkpoints"
          evals_interval:
            type: integer
            description: "Run eval checkpoints every N iterations"
        required: [mode]
"##
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentAdapter, CodexAdapter};

    #[test]
    fn agent_yaml_advertises_all_modes() {
        let yaml = CodexAdapter::agent_yaml();

        for mode in [
            "loop", "plan", "debug", "fix", "security", "ship", "scenario", "predict", "learn",
            "reason", "probe", "improve", "evals", "exec",
        ] {
            assert!(yaml.contains(mode), "missing mode {mode}");
        }
    }

    #[test]
    fn agent_yaml_advertises_chain_and_eval_controls() {
        let yaml = CodexAdapter::agent_yaml();

        for property in ["chain:", "evals:", "evals_interval:"] {
            assert!(yaml.contains(property), "missing property {property}");
        }
    }

    #[test]
    fn command_prefix_matches_codex_skill_name() {
        let adapter = CodexAdapter;

        assert_eq!(adapter.command_prefix(), "$autoresearch");
    }

    #[test]
    fn packaged_openai_yaml_matches_adapter() {
        assert_eq!(
            include_str!("../../agents/openai.yaml"),
            CodexAdapter::agent_yaml()
        );
    }
}
