use super::{HookInput, HookResponse};
use std::path::PathBuf;
use std::process::Command;

/// Session end: emit a terminal notification and brief completion summary.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"]).unwrap_or(cwd);
    let project = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let summary =
        iteration_summary(&project_root).unwrap_or_else(|| "no iterations recorded".to_string());
    HookResponse {
        terminal_sequence: Some(format!(
            "\x1b]777;notify;autoresearch;Session completed -- {project}\x07"
        )),
        additional_context: Some(format!(
            "## Session completed\n- Project: {project}\n- Summary: {summary}"
        )),
        ..Default::default()
    }
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
