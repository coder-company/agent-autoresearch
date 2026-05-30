use super::{HookInput, HookResponse};
use glob::Pattern;
use std::path::{Component, Path};

/// Block tool calls that attempt to read/write outside declared scope during an active run.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

    let cwd = match std::env::current_dir() {
        Ok(path) => path,
        Err(_) => return HookResponse::allow(),
    };

    let scope = match load_scope(&cwd) {
        Some(scope) if !scope.is_empty() => scope,
        _ => return HookResponse::allow(),
    };

    let compiled_scope: Vec<String> = scope
        .iter()
        .filter_map(|pattern| normalize_scope_pattern(pattern, &cwd))
        .collect();

    if compiled_scope.is_empty() {
        return HookResponse::allow();
    }

    // If tool is file-modifying, check against scope.
    let tool = input.tool_name.as_deref().unwrap_or("");
    if !matches!(tool, "Write" | "Edit" | "MultiEdit") {
        return HookResponse::allow();
    }

    let target = match target_path(input) {
        Some(path) => path,
        None => return HookResponse::allow(),
    };

    let relative_target = match normalize_target_path(target, &cwd) {
        Some(path) => path,
        None => {
            return HookResponse::block(format!(
                "Blocked {tool}: target path `{target}` is outside the workspace"
            ));
        }
    };

    if matches_scope(&relative_target, &compiled_scope) {
        return HookResponse::allow();
    }

    HookResponse::block(format!(
        "Blocked {tool}: `{relative_target}` is outside autoresearch scope [{}]",
        compiled_scope.join(", ")
    ))
}

fn load_scope(cwd: &Path) -> Option<Vec<String>> {
    let state = std::fs::read_to_string(cwd.join("autoresearch-results/state.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&state).ok()?;
    scope_array(
        value
            .pointer("/config/scope")
            .or_else(|| value.get("scope")),
    )
}

fn scope_array(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let scope = value?
        .as_array()?
        .iter()
        .filter_map(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    Some(scope)
}

fn target_path(input: &HookInput) -> Option<&str> {
    input
        .tool_input
        .as_ref()
        .and_then(|value| value.get("file_path").or_else(|| value.get("path")))
        .and_then(|value| value.as_str())
}

fn normalize_target_path(path: &str, cwd: &Path) -> Option<String> {
    let target = Path::new(path);
    let relative = if target.is_absolute() {
        target.strip_prefix(cwd).ok()?.to_path_buf()
    } else {
        target.to_path_buf()
    };

    path_to_clean_slash_string(&relative)
}

fn normalize_scope_pattern(pattern: &str, cwd: &Path) -> Option<String> {
    let mut pattern = pattern.trim().replace('\\', "/");
    if pattern.is_empty() {
        return None;
    }

    let cwd = cwd.to_string_lossy().replace('\\', "/");
    if pattern == cwd {
        return Some("**/*".to_string());
    }
    if let Some(stripped) = pattern.strip_prefix(&(cwd + "/")) {
        pattern = stripped.to_string();
    }
    while let Some(stripped) = pattern.strip_prefix("./") {
        pattern = stripped.to_string();
    }

    Some(pattern.trim_start_matches('/').to_string())
}

fn path_to_clean_slash_string(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    if parts.is_empty() {
        Some(".".to_string())
    } else {
        Some(parts.join("/"))
    }
}

fn matches_scope(path: &str, scope: &[String]) -> bool {
    scope
        .iter()
        .any(|pattern| pattern_matches_path(pattern, path))
}

fn pattern_matches_path(pattern: &str, path: &str) -> bool {
    if matches!(pattern, "." | "**" | "**/*") {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix("/**") {
        if path == prefix || path.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }
    if let Some(prefix) = pattern.strip_suffix("/**/*") {
        if path == prefix || path.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }

    if !pattern_contains_glob(pattern)
        && (path == pattern || path.starts_with(&format!("{pattern}/")))
    {
        return true;
    }

    if Pattern::new(pattern).is_ok_and(|compiled| compiled.matches(path)) {
        return true;
    }

    pattern
        .contains("**/")
        .then(|| pattern.replace("**/", ""))
        .and_then(|fallback| Pattern::new(&fallback).ok())
        .is_some_and(|compiled| compiled.matches(path))
}

fn pattern_contains_glob(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}
