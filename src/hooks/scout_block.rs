use super::{HookInput, HookResponse};
use glob::Pattern;
use std::path::{Component, Path};

const BASELINE_BLOCKED_PATTERNS: &[&str] = &[
    "node_modules/**",
    "__pycache__/**",
    ".git/**",
    "dist/**",
    "build/**",
    "out/**",
    "coverage/**",
    ".next/**",
    ".nuxt/**",
    "venv/**",
    ".venv/**",
    "env/**",
    ".terraform/**",
    ".aws/**",
    ".ssh/**",
    "*.log",
];

const PATH_READING_COMMANDS: &[&str] = &[
    "cat", "less", "more", "head", "tail", "sed", "awk", "grep", "rg", "find", "ls",
];

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

    let tool = input.tool_name.as_deref().unwrap_or("");
    if matches!(
        tool,
        "Read" | "Write" | "Edit" | "MultiEdit" | "Glob" | "Grep"
    ) {
        if let Some(target) = target_path(input) {
            if let Some(relative_target) = normalize_target_path(target, &cwd) {
                if matches_scope(&relative_target, BASELINE_BLOCKED_PATTERNS) {
                    return HookResponse::block(format!(
                        "Blocked {tool}: `{relative_target}` matches generated, vendor, or sensitive path pattern"
                    ));
                }
            }
        }
    }
    if tool == "Bash" {
        if let Some(command) = input
            .tool_input
            .as_ref()
            .and_then(|value| value.get("command").or_else(|| value.get("cmd")))
            .and_then(|value| value.as_str())
        {
            if let Some(blocked_path) = blocked_bash_path(command, &cwd) {
                return HookResponse::block(format!(
                    "Blocked Bash: `{blocked_path}` matches generated, vendor, or sensitive path pattern"
                ));
            }
        }
    }

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

fn matches_scope<T: AsRef<str>>(path: &str, scope: &[T]) -> bool {
    scope
        .iter()
        .any(|pattern| pattern_matches_path(pattern.as_ref(), path))
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

fn blocked_bash_path(command: &str, cwd: &Path) -> Option<String> {
    let words = shell_words(command)?;
    let command_index = words.iter().position(|word| !is_env_assignment(word))?;
    let executable = Path::new(&words[command_index])
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&words[command_index]);
    if !PATH_READING_COMMANDS.contains(&executable) {
        return None;
    }

    words
        .iter()
        .skip(command_index + 1)
        .filter(|word| !word.starts_with('-') && !matches!(word.as_str(), "|" | "<" | ">" | "2>"))
        .filter_map(|word| normalize_target_path(word, cwd))
        .find(|path| matches_scope(path, BASELINE_BLOCKED_PATTERNS))
}

fn shell_words(command: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match quote {
            Some('\'') if ch == '\'' => quote = None,
            Some('"') if ch == '"' => quote = None,
            Some('\'') => current.push(ch),
            Some(_) if ch == '\\' => escaped = true,
            Some(_) => current.push(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch == '\\' => escaped = true,
            None => current.push(ch),
        }
    }

    if quote.is_some() || escaped {
        return None;
    }
    if !current.is_empty() {
        words.push(current);
    }
    Some(words)
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _value)) = token.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
