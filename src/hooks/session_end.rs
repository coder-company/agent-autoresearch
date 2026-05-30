use super::{HookInput, HookResponse};

/// Session end: emit a terminal notification and brief completion summary.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let project = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let summary = iteration_summary(&cwd).unwrap_or_else(|| "no iterations recorded".to_string());
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
