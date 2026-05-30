use super::{HookInput, HookResponse};
use std::path::PathBuf;
use std::process::Command;

/// Stop hook: fires after each Claude Code turn.
/// If an active autoresearch run exists but the turn didn't complete an iteration,
/// inject a reminder to continue the loop.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"]).unwrap_or(cwd);
    let state_path = project_root.join("autoresearch-results/state.json");

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

    // Only inject if phase is "iterating"
    let phase = state
        .get("phase")
        .and_then(|p| p.get("phase"))
        .and_then(|p| p.as_str())
        .unwrap_or("");

    if phase != "iterating" {
        return HookResponse::allow();
    }

    let iteration = state.get("iteration").and_then(|i| i.as_u64()).unwrap_or(0);
    let current = state
        .get("current_metric")
        .and_then(|m| m.as_str())
        .unwrap_or("?");
    let best = state
        .get("best_metric")
        .and_then(|m| m.as_str())
        .unwrap_or("?");
    let consecutive = state
        .get("consecutive_discards")
        .and_then(|c| c.as_u64())
        .unwrap_or(0);

    // Check escalation state
    let esc_path = project_root.join("autoresearch-results/escalation.json");
    let escalation_hint = if let Ok(esc_content) = std::fs::read_to_string(&esc_path) {
        if let Ok(esc) = serde_json::from_str::<serde_json::Value>(&esc_content) {
            let pivots = esc
                .get("pivots_since_last_keep")
                .and_then(|p| p.as_u64())
                .unwrap_or(0);
            if pivots >= 3 {
                "\n⚠️ SOFT BLOCKER: 3+ PIVOTs without progress. Consider stopping."
            } else if consecutive >= 5 {
                "\n🔄 PIVOT needed: 5+ consecutive discards. Try fundamentally different approach."
            } else if consecutive >= 3 {
                "\n⚡ REFINE: 3+ consecutive discards. Adjust within current strategy."
            } else {
                ""
            }
        } else {
            ""
        }
    } else {
        ""
    };

    HookResponse::inject(format!(
        "## Autoresearch loop active — continue iterating\n\
         **Iteration:** {iteration} | **Metric:** {current} (best: {best}) | **Consecutive discards:** {consecutive}\n\
         Next: read context → ideate → modify → commit → verify → decide{escalation_hint}"
    ))
}

fn git_output(cwd: &std::path::Path, args: &[&str]) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| PathBuf::from(value.trim()))
        .filter(|value| !value.as_os_str().is_empty())
}
