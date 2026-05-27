use super::{AgentAdapter, InputMethod};
use serde::{Deserialize, Serialize};

/// Codex CLI skill adapter.
///
/// Commands invoked as plain text: `$codex-autoresearch` or `autoresearch <mode>`.
/// Supports foreground and background run modes.
/// Uses structured JSON output in exec mode.
pub struct CodexAdapter;

impl AgentAdapter for CodexAdapter {
    fn format_output(&self, content: &str) -> String {
        content.to_string()
    }

    fn command_prefix(&self) -> &str {
        "$codex-autoresearch"
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
        r#"name: autoresearch
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
            enum: [loop, plan, debug, fix, security, ship, scenario, predict, learn, reason, probe, exec]
          goal:
            type: string
          scope:
            type: string
          iterations:
            type: integer
        required: [mode]
"#
    }
}
