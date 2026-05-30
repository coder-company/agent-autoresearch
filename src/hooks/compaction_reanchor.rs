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

    HookResponse::inject(build_reanchor_context(iteration, current, best))
}

fn build_reanchor_context(iteration: u64, current: &str, best: &str) -> String {
    format!(
        "## ⚠️ Context compacted — Protocol Re-Anchor\n\n\
         **Active autoresearch run** (iteration {iteration}, metric: {current}, best: {best})\n\n\
         ### Core Protocol (each turn)\n\
         1. Read: `git log --oneline -10` + last rows of `autoresearch-results/results.tsv` + lessons.md + context.json\n\
         2. Ideate: ONE specific, testable hypothesis (different from all previous)\n\
         3. Modify: ONE focused change within scope\n\
         4. Commit: `git add -- <files>; git commit -m \"experiment: <desc>\"`\n\
         5. Verify: `autoresearch verify --format metrics_json --key <metric> --command \"<cmd>\"` for structured metrics, otherwise scalar verify\n\
         6. Guard: `autoresearch guard --command \"<cmd>\"` (if configured)\n\
         7. Decide: `autoresearch decide --decision auto --metric <val> --metrics-json '<json>' --description \"<text>\"`\n\n\
         ### Critical Rules\n\
         - ONE change per turn. Never stage autoresearch-results/ or .codex-autoresearch/.\n\
         - Mechanical verification only. Let the binary handle criteria gates and rollback.\n\
         - 3 discards → REFINE. 5 → PIVOT. 3 PIVOTs → stop.\n\
         - Never ask the user. Apply best practices. Keep iterating."
    )
}

#[cfg(test)]
mod tests {
    use super::build_reanchor_context;

    #[test]
    fn reanchor_uses_current_binary_protocol() {
        let context = build_reanchor_context(7, "82.5", "84.0");

        assert!(context.contains("context.json"));
        assert!(context.contains("autoresearch verify --format metrics_json"));
        assert!(context.contains("autoresearch decide --decision auto"));
        assert!(context.contains(".codex-autoresearch/"));
        assert!(!context.contains("--decision <keep|discard|crash>"));
    }
}
