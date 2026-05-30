pub mod compaction_reanchor;
pub mod dangerous_cmd;
pub mod dev_rules_reminder;
pub mod iteration_context;
pub mod privacy_block;
pub mod scout_block;
pub mod session_end;
pub mod session_init;
pub mod simplify_gate;
pub mod stop_check;
pub mod subagent_context;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Hook response protocol — matches Claude Code plugin hook contract.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HookResponse {
    /// Additional context to inject into the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
    /// If set, blocks the tool call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<HookDecision>,
    /// Reason for blocking (shown to agent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Optional terminal notification sequence for session-end style hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_sequence: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecision {
    Allow,
    Block,
}

impl HookResponse {
    pub fn allow() -> Self {
        Self::default()
    }

    pub fn inject(context: String) -> Self {
        Self {
            additional_context: Some(context),
            ..Default::default()
        }
    }

    pub fn block(reason: String) -> Self {
        Self {
            decision: Some(HookDecision::Block),
            reason: Some(reason),
            ..Default::default()
        }
    }

    /// Write response to stdout and exit.
    pub fn emit(self) {
        let json = serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_string());
        print!("{json}");
    }
}

/// Stdin payload from Claude Code hook system.
#[derive(Debug, Clone, Deserialize)]
pub struct HookInput {
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Read and parse hook stdin. Returns None on parse failure (fail-open).
pub fn read_stdin() -> Option<HookInput> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).ok()?;
    serde_json::from_str(&buf).ok()
}

/// Dispatch a hook by name.
pub fn dispatch(hook_name: &str) -> Result<()> {
    let input = read_stdin();

    if hook_disabled(hook_name) {
        HookResponse::allow().emit();
        return Ok(());
    }

    let response = match hook_name {
        "iteration-context" => iteration_context::run(input.as_ref()),
        "scout-block" => scout_block::run(input.as_ref()),
        "privacy-block" => privacy_block::run(input.as_ref()),
        "dangerous-cmd-block" => dangerous_cmd::run(input.as_ref()),
        "session-init" => session_init::run(input.as_ref()),
        "session-end" => session_end::run(input.as_ref()),
        "simplify-gate" => simplify_gate::run(input.as_ref()),
        "stop-check" => stop_check::run(input.as_ref()),
        "compaction-reanchor" => compaction_reanchor::run(input.as_ref()),
        "subagent-context" => subagent_context::run(input.as_ref()),
        "dev-rules-reminder" => dev_rules_reminder::run(input.as_ref()),
        // Referenced in hooks.json but no custom logic needed — pass through.
        "verify-capture" | "subagent-collect" | "pre-compact-snapshot" | "notification-relay" => {
            HookResponse::allow()
        }
        _ => HookResponse::allow(),
    };

    response.emit();
    Ok(())
}

fn hook_disabled(hook_name: &str) -> bool {
    let env_key = format!(
        "AR_DISABLE_{}",
        hook_name.replace('-', "_").to_ascii_uppercase()
    );
    std::env::var_os(env_key).is_some_and(|value| !value.is_empty())
}
