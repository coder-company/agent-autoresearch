use super::{HookInput, HookResponse};
use std::path::PathBuf;

/// PostCompact hook: fires after context compaction.
/// Re-injects the core protocol and current state to prevent drift.
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

    HookResponse::inject(format!(
        "## ⚠️ Context compacted — Protocol Re-Anchor\n\n\
         **Active autoresearch run** (iteration {iteration}, metric: {current}, best: {best})\n\n\
         ### Core Protocol (each turn)\n\
         1. Read: `git log --oneline -10` + last rows of `autoresearch-results/results.tsv` + lessons.md\n\
         2. Ideate: ONE specific, testable hypothesis (different from all previous)\n\
         3. Modify: ONE focused change within scope\n\
         4. Commit: `git add -- <files>; git commit -m \"experiment: <desc>\"`\n\
         5. Verify: `autoresearch verify --command \"<cmd>\"`\n\
         6. Guard: `autoresearch guard --command \"<cmd>\"` (if configured)\n\
         7. Decide: `autoresearch decide --decision <keep|discard|crash> --metric <val> --description \"<text>\"`\n\n\
         ### Critical Rules\n\
         - ONE change per turn. Never stage autoresearch-results/.\n\
         - Mechanical verification only. Automatic rollback on failure.\n\
         - 3 discards → REFINE. 5 → PIVOT. 3 PIVOTs → stop.\n\
         - Never ask the user. Apply best practices. Keep iterating."
    ))
}
