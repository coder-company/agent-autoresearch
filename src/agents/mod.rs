pub mod claude;
pub mod codex;

use serde::{Deserialize, Serialize};

/// Agent type detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    ClaudeCode,
    Codex,
    Generic,
}

impl AgentType {
    /// Detect agent type from environment.
    pub fn detect() -> Self {
        // Claude Code sets CLAUDE_PLUGIN_ROOT
        if std::env::var("CLAUDE_PLUGIN_ROOT").is_ok() {
            return Self::ClaudeCode;
        }
        // Codex sets CODEX_SKILL_ROOT or similar
        if std::env::var("CODEX_SKILL_ROOT").is_ok()
            || std::env::var("CODEX_SESSION").is_ok()
        {
            return Self::Codex;
        }
        Self::Generic
    }
}

/// Trait for agent-specific behavior.
pub trait AgentAdapter {
    /// Format output for the specific agent's protocol.
    fn format_output(&self, content: &str) -> String;

    /// Get the command prefix for this agent.
    fn command_prefix(&self) -> &str;

    /// How this agent handles user input requests.
    fn input_method(&self) -> InputMethod;
}

#[derive(Debug, Clone, Copy)]
pub enum InputMethod {
    /// Claude Code: uses AskUserQuestion tool
    AskUserQuestion,
    /// Codex: uses request_user_input or direct question
    RequestUserInput,
    /// Generic: inline prompt
    InlinePrompt,
}
