use super::{HookInput, HookResponse};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Periodically remind the agent of core autoresearch rules during long runs.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let Some(input) = input else {
        return HookResponse::allow();
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if !should_inject_now(input, &cwd) {
        return HookResponse::allow();
    }

    HookResponse::inject(format!(
        "## Dev context\n\
         - Plan: {} (check for active plan.md)\n\
         - Standards: {}",
        cwd.join("plans").display(),
        cwd.join("docs/code-standards.md").display()
    ))
}

fn should_inject_now(input: &HookInput, cwd: &Path) -> bool {
    let Some(session_id) = input.session_id.as_deref() else {
        return false;
    };
    let counter_path = session_counter_path(cwd, session_id);
    let count = std::fs::read_to_string(&counter_path)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    let _ = std::fs::write(counter_path, count.to_string());
    count % 5 == 0
}

fn session_counter_path(cwd: &Path, session_id: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cwd.hash(&mut hasher);
    session_id.hash(&mut hasher);
    std::env::temp_dir().join(format!(
        "autoresearch-dev-rules-reminder-{}.count",
        hasher.finish()
    ))
}
