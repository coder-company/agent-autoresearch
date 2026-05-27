use super::{HookInput, HookResponse};
use std::path::PathBuf;

/// Inject run context when a subagent starts during an active autoresearch run.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let state_path = cwd.join("autoresearch-results/state.json");

    if !state_path.exists() {
        return HookResponse::allow();
    }

    let content = match std::fs::read_to_string(&state_path) {
        Ok(c) => c,
        Err(_) => return HookResponse::allow(),
    };

    let state: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return HookResponse::allow(),
    };

    let iteration = state.get("iteration").and_then(|i| i.as_u64()).unwrap_or(0);
    let metric = state
        .get("current_metric")
        .and_then(|m| m.as_str())
        .unwrap_or("?");
    let status = state
        .get("last_status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    HookResponse::inject(format!(
        "## Autoresearch subagent context\n\
         **Iteration:** {iteration} | **Metric:** {metric} | **Last:** {status}\n\
         You are a subagent within an active autoresearch run.\n\
         - Make ONE focused change per request.\n\
         - Do not modify autoresearch-results/ artifacts.\n\
         - Do not push, deploy, or publish."
    ))
}
