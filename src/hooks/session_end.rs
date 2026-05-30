use super::{HookInput, HookResponse};
use chrono::{DateTime, Utc};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Session end: emit a terminal notification and brief completion summary.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"]).unwrap_or(cwd);
    let session_id = input
        .and_then(|input| input.session_id.as_deref())
        .unwrap_or("unknown");
    let session_state = load_session_state(&project_root, session_id);
    let branch = session_state
        .as_ref()
        .and_then(|state| state.get("gitBranch"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| git_text(&project_root, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_default();
    let duration = session_state
        .as_ref()
        .and_then(|state| state.get("startedAt"))
        .and_then(|value| value.as_str())
        .map(format_duration)
        .unwrap_or_else(|| "unknown".to_string());
    let project = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let summary =
        iteration_summary(&project_root).unwrap_or_else(|| "no iterations recorded".to_string());
    if let Ok(webhook_url) = std::env::var("AR_NOTIFY_WEBHOOK") {
        post_webhook(&webhook_url, project, &branch, &duration, &summary);
    }
    cleanup_session_state(&project_root, session_id);
    HookResponse {
        terminal_sequence: Some(format!(
            "\x1b]777;notify;autoresearch;Session completed -- {project} ({duration})\x07"
        )),
        additional_context: Some(format!(
            "## Session completed\n- Project: {project}\n- Duration: {duration}\n- Summary: {summary}"
        )),
        ..Default::default()
    }
}

fn load_session_state(
    project_root: &std::path::Path,
    session_id: &str,
) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(session_state_path(project_root, session_id)).ok()?;
    serde_json::from_str(&content).ok()
}

fn cleanup_session_state(project_root: &std::path::Path, session_id: &str) {
    let _ = std::fs::remove_file(session_state_path(project_root, session_id));
}

fn session_state_path(project_root: &std::path::Path, session_id: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    project_root.hash(&mut hasher);
    session_id.hash(&mut hasher);
    std::env::temp_dir().join(format!("ar-session-{}.json", hasher.finish()))
}

fn format_duration(started_at: &str) -> String {
    let Ok(started_at) = DateTime::parse_from_rfc3339(started_at) else {
        return "unknown".to_string();
    };
    let seconds = (Utc::now() - started_at.with_timezone(&Utc)).num_seconds();
    if seconds < 0 {
        return "unknown".to_string();
    }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

fn post_webhook(webhook_url: &str, project: &str, branch: &str, duration: &str, summary: &str) {
    if webhook_url.trim().is_empty() {
        return;
    }
    let payload = serde_json::json!({
        "text": "autoresearch session completed",
        "project": project,
        "branch": branch,
        "duration": duration,
        "tsv_summary": summary,
    })
    .to_string();
    let _ = Command::new("curl")
        .args([
            "-fsS",
            "-m",
            "2",
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload,
            webhook_url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn iteration_summary(cwd: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(cwd.join("autoresearch-results/results.tsv")).ok()?;
    let rows = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("iteration\t")
        })
        .collect::<Vec<_>>();
    let last = rows.last()?;
    let metric = last.split('\t').nth(2).unwrap_or("n/a");
    Some(format!("{} iterations, metric: {metric}", rows.len()))
}

fn git_text(cwd: &std::path::Path, args: &[&str]) -> Option<String> {
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
