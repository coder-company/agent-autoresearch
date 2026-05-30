use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use super::{HookInput, HookResponse};

const MAX_AGE_SECS: u64 = 30 * 60; // 30 minutes

/// Inject iteration context from the active results TSV every N prompts.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let _input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

    // Find the active TSV file
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_root = git_output(&cwd, &["rev-parse", "--show-toplevel"]).unwrap_or(cwd);
    if !should_inject_now(_input, &project_root) {
        return HookResponse::allow();
    }
    let tsv_path = match find_recent_tsv(&project_root) {
        Some(p) => p,
        None => return HookResponse::allow(),
    };

    // Read and format tail
    let content = match fs::read_to_string(&tsv_path) {
        Ok(c) => c,
        Err(_) => return HookResponse::allow(),
    };

    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

    let header = lines
        .iter()
        .find(|l| l.starts_with("iteration\t") || l.starts_with('#'))
        .copied()
        .unwrap_or("");

    let data_lines: Vec<&str> = lines
        .iter()
        .filter(|l| !l.starts_with('#') && !l.starts_with("iteration\t"))
        .copied()
        .collect();

    let total = data_lines.len();
    let tail_n = 3.min(total);
    let tail = &data_lines[total.saturating_sub(tail_n)..];

    let rel_path = pathdiff(&project_root, &tsv_path);

    let mut text = format!(
        "## Active iteration state\n**TSV:** {}\n**Rows:** {}\n\n{}",
        rel_path,
        total,
        if !header.is_empty() {
            format!("{header}\n{}", tail.join("\n"))
        } else {
            tail.join("\n")
        }
    );

    text.push_str(&format!(
        "\n\n**Loop state:** active — {} iterations recorded",
        total
    ));

    mark_context_injected(_input, &project_root);
    HookResponse::inject(text)
}

fn should_inject_now(input: &HookInput, cwd: &Path) -> bool {
    let Some(session_id) = input.session_id.as_deref() else {
        return true;
    };
    let counter_path = session_counter_path(cwd, session_id);
    let count = fs::read_to_string(&counter_path)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    let _ = fs::write(counter_path, count.to_string());
    count % 5 == 0
}

fn session_counter_path(cwd: &Path, session_id: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cwd.hash(&mut hasher);
    session_id.hash(&mut hasher);
    std::env::temp_dir().join(format!(
        "autoresearch-iteration-context-{}.count",
        hasher.finish()
    ))
}

fn mark_context_injected(input: &HookInput, cwd: &Path) {
    let Some(session_id) = input.session_id.as_deref() else {
        return;
    };
    let _ = fs::write(context_injection_path(cwd, session_id), "1");
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

fn find_recent_tsv(cwd: &Path) -> Option<PathBuf> {
    let results_dir = cwd.join("autoresearch-results");
    let tsv_path = results_dir.join("results.tsv");

    if tsv_path.exists() {
        // Check age
        if let Ok(meta) = fs::metadata(&tsv_path) {
            if let Ok(modified) = meta.modified() {
                let age = SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                if age.as_secs() < MAX_AGE_SECS {
                    return Some(tsv_path);
                }
            }
        }
    }

    // Fallback: check legacy autoresearch/ directory
    let legacy_dir = cwd.join("autoresearch");
    if legacy_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&legacy_dir) {
            let mut best: Option<(PathBuf, SystemTime)> = None;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(sub_entries) = fs::read_dir(&path) {
                        for sub in sub_entries.flatten() {
                            let sp = sub.path();
                            if sp.extension().and_then(|e| e.to_str()) == Some("tsv") {
                                if let Ok(meta) = fs::metadata(&sp) {
                                    if let Ok(modified) = meta.modified() {
                                        let dominated = best
                                            .as_ref()
                                            .map(|(_, t)| modified > *t)
                                            .unwrap_or(true);
                                        if dominated {
                                            best = Some((sp, modified));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some((path, modified)) = best {
                let age = SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                if age.as_secs() < MAX_AGE_SECS {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn pathdiff(base: &Path, target: &Path) -> String {
    target
        .strip_prefix(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| target.display().to_string())
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
