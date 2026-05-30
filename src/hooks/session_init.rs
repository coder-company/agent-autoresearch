use super::{HookInput, HookResponse};
use chrono::Utc;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

/// Session initialization: detect environment, check for resumable runs.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.clone());
    let branch = git_output(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let session_id = input
        .and_then(|input| input.session_id.as_deref())
        .unwrap_or("unknown");
    persist_session_state(&project_root, &branch, session_id);
    prune_stale_session_files();
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
    let state_path = project_root.join("autoresearch-results/state.json");
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

fn persist_session_state(project_root: &std::path::Path, branch: &str, session_id: &str) {
    let state = serde_json::json!({
        "projectRoot": project_root,
        "plansPath": project_root.join("plans"),
        "reportsPath": project_root.join("plans/reports"),
        "gitBranch": branch,
        "sessionId": session_id,
        "iterationCount": 0,
        "startedAt": Utc::now().to_rfc3339(),
    });
    let _ = std::fs::write(
        session_state_path(project_root, session_id),
        serde_json::to_string_pretty(&state).unwrap_or_else(|_| "{}".to_string()),
    );
}

fn prune_stale_session_files() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("ar-session-") || !name.ends_with(".json") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let stale = SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age > Duration::from_secs(24 * 60 * 60));
        if stale {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn session_state_path(project_root: &std::path::Path, session_id: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    project_root.hash(&mut hasher);
    session_id.hash(&mut hasher);
    std::env::temp_dir().join(format!("ar-session-{}.json", hasher.finish()))
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
