use super::{HookInput, HookResponse};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Inject run context when a subagent starts during an active autoresearch run.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.clone());
    let branch = git_output(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string());
    let state_path = project_root.join("autoresearch-results/state.json");
    let tsv_path = project_root.join("autoresearch-results/results.tsv");

    if !state_path.exists() && !tsv_path.exists() {
        return HookResponse::allow();
    }

    let state: Option<serde_json::Value> = std::fs::read_to_string(&state_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok());

    let iteration = state
        .as_ref()
        .and_then(|state| state.get("iteration"))
        .and_then(|i| i.as_u64())
        .unwrap_or(0);
    let metric = state
        .as_ref()
        .and_then(|state| state.get("current_metric"))
        .and_then(|m| m.as_str())
        .unwrap_or("?");
    let status = state
        .as_ref()
        .and_then(|state| state.get("last_status"))
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let latest = latest_tsv_summary(&tsv_path).unwrap_or_else(|| "none".to_string());
    let active_tsv = tsv_path
        .strip_prefix(&project_root)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| tsv_path.display().to_string());

    HookResponse::inject(format!(
        "## Autoresearch context (for subagent)\n\
         - Project: {}\n\
         - Branch: {branch}\n\
         - Plans: {}\n\
         - Reports: {}\n\
         - Active TSV: {active_tsv}\n\
         - Iteration: {iteration}\n\
         - Metric: {metric}\n\
         - Last: {status}\n\
         - Latest: {latest}\n\
         \n\
         You are a subagent within an active autoresearch run.\n\
         - Make ONE focused change per request.\n\
         - Do not modify autoresearch-results/ artifacts.\n\
         - Do not push, deploy, or publish.",
        project_root.display(),
        project_root.join("plans").display(),
        project_root.join("plans/reports").display()
    ))
}

fn latest_tsv_summary(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut header = "";
    let mut rows = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with("iteration\t") {
            header = line;
        } else {
            rows.push(line);
        }
    }
    let row = rows.last()?;
    let header_cols = header.split('\t').collect::<Vec<_>>();
    let row_cols = row.split('\t').collect::<Vec<_>>();
    let mut parts = Vec::new();
    for (index, value) in row_cols.iter().enumerate() {
        let column = header_cols
            .get(index)
            .copied()
            .unwrap_or("")
            .to_ascii_lowercase();
        if column.contains("status")
            || column.contains("result")
            || column.contains("pass")
            || column.contains("fail")
        {
            parts.push((*value).to_string());
        } else if value.parse::<f64>().is_ok() && parts.len() < 3 {
            let label = header_cols
                .get(index)
                .copied()
                .filter(|label| !label.is_empty())
                .map(|label| format!("{label}="))
                .unwrap_or_default();
            parts.push(format!("{label}{value}"));
        }
    }
    if parts.is_empty() {
        Some(
            row_cols
                .iter()
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join(", "),
        )
    } else {
        Some(parts.join(", "))
    }
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
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
