use super::{HookInput, HookResponse};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// Periodically remind the agent of core autoresearch rules during long runs.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let Some(input) = input else {
        return HookResponse::allow();
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"]).unwrap_or(cwd);
    if context_injected_recently(input, &project_root) {
        return HookResponse::allow();
    }
    if !should_inject_now(input, &project_root) {
        return HookResponse::allow();
    }

    HookResponse::inject(format!(
        "## Dev context\n\
         - Plan: {} (check for active plan.md)\n\
         - Standards: {}",
        project_root.join("plans").display(),
        project_root.join("docs/code-standards.md").display()
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
    count.is_multiple_of(5)
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

fn context_injected_recently(input: &HookInput, cwd: &Path) -> bool {
    let Some(session_id) = input.session_id.as_deref() else {
        return false;
    };
    let Ok(metadata) = std::fs::metadata(context_injection_path(cwd, session_id)) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .is_ok_and(|age| age < Duration::from_secs(2))
}

fn context_injection_path(cwd: &Path, session_id: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cwd.hash(&mut hasher);
    session_id.hash(&mut hasher);
    std::env::temp_dir().join(format!(
        "autoresearch-context-injected-{}.stamp",
        hasher.finish()
    ))
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<PathBuf> {
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
