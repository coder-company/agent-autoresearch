use super::{HookInput, HookResponse};
use std::path::PathBuf;

/// Session initialization: detect environment, check for resumable runs.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Check if there's an existing run to resume
    let state_path = cwd.join("autoresearch-results/state.json");
    if state_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&state_path) {
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                let phase = state
                    .get("phase")
                    .and_then(|p| p.get("phase"))
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown");

                if phase == "iterating" {
                    let iteration = state.get("iteration").and_then(|i| i.as_u64()).unwrap_or(0);
                    let metric = state
                        .get("current_metric")
                        .and_then(|m| m.as_str())
                        .unwrap_or("?");

                    return HookResponse::inject(format!(
                        "## Resumable autoresearch run detected\n\
                         **State:** iteration {iteration}, metric: {metric}\n\
                         Use `autoresearch` to resume or start fresh."
                    ));
                }
            }
        }
    }

    HookResponse::allow()
}
