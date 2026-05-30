use super::{HookInput, HookResponse};
use std::path::PathBuf;
use std::process::Command;

/// Session initialization: detect environment, check for resumable runs.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.clone());
    let branch = git_output(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let mut context = format!(
        "## Session initialized\n\
         - Project: {}\n\
         - Branch: {}\n\
         - Plans: {}\n\
         - Reports: {}",
        project_root.display(),
        if branch.is_empty() {
            "unknown"
        } else {
            &branch
        },
        project_root.join("plans").display(),
        project_root.join("plans/reports").display()
    );

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

                    context.push_str(&format!(
                        "\n\n## Resumable autoresearch run detected\n\
                         **State:** iteration {iteration}, metric: {metric}\n\
                         Use `autoresearch` to resume or start fresh."
                    ));
                }
            }
        }
    }

    HookResponse::inject(context)
}

fn git_output(cwd: &std::path::Path, args: &[&str]) -> Option<String> {
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
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
