use super::{HookInput, HookResponse};
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

static SENSITIVE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(password|passwd|secret|api[_-]?key|token|credential)\s*[:=]").unwrap(),
        Regex::new(r"(?i)(aws|gcp|azure)[\w]*[_-]?(secret|key|token)").unwrap(),
        Regex::new(r"-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----").unwrap(),
        Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(), // GitHub PAT
        Regex::new(r"sk-[A-Za-z0-9]{48}").unwrap(),  // OpenAI key
        Regex::new(r"AKIA[A-Z0-9]{16}").unwrap(),    // AWS access key
    ]
});

const SENSITIVE_PATH_PATTERNS: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    ".env.development",
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    "id_rsa",
    "id_ed25519",
    ".ssh/",
    "credentials.json",
    "credentials.yaml",
    "secret",
    "api_key",
    "apikey",
    ".aws/credentials",
];

const PATH_EXCEPTIONS: &[&str] = &[".env.example", ".env.sample", ".env.template", ".env.test"];

/// Block operations that would expose sensitive data.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

    if let Some((path_key, path)) = tool_path(input) {
        if path.starts_with("APPROVED:") {
            let stripped_path = path.trim_start_matches("APPROVED:");
            let mut updated_input = serde_json::Map::new();
            updated_input.insert(path_key.to_string(), serde_json::json!(stripped_path));
            return HookResponse {
                permission_decision: Some("allow".to_string()),
                updated_input: Some(serde_json::Value::Object(updated_input)),
                ..Default::default()
            };
        }
        if is_sensitive_path(path) {
            let filename = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path);
            return HookResponse::block(format!(
                "Blocked: `{filename}` may contain secrets. Ask for permission, then retry with APPROVED: prefix on the file path."
            ));
        }
    }

    if input.tool_name.as_deref() == Some("Bash") {
        if let Some(path) = sensitive_bash_path(input) {
            return HookResponse::inject(format!(
                "WARNING: Bash command references potentially sensitive path `{path}`. Ensure no secrets are exposed or committed."
            ));
        }
    }

    // Check tool input content for sensitive patterns
    let content = match &input.tool_input {
        Some(v) => serde_json::to_string(v).unwrap_or_default(),
        None => return HookResponse::allow(),
    };

    for pattern in SENSITIVE_PATTERNS.iter() {
        if pattern.is_match(&content) {
            return HookResponse::block(format!(
                "Blocked: tool input may contain sensitive data matching pattern '{}'",
                pattern.as_str()
            ));
        }
    }

    HookResponse::allow()
}

fn sensitive_bash_path(input: &HookInput) -> Option<String> {
    let command = input
        .tool_input
        .as_ref()
        .and_then(|value| value.get("command").or_else(|| value.get("cmd")))
        .and_then(|value| value.as_str())?;
    command
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|ch| matches!(ch, '\'' | '"' | '`' | ';' | ','))
                .to_string()
        })
        .find(|token| !token.starts_with("APPROVED:") && is_sensitive_path(token))
}

fn tool_path(input: &HookInput) -> Option<(&str, &str)> {
    let tool_input = input.tool_input.as_ref()?;
    if let Some(path) = tool_input.get("file_path").and_then(|value| value.as_str()) {
        return Some(("file_path", path));
    }
    tool_input
        .get("path")
        .and_then(|value| value.as_str())
        .map(|path| ("path", path))
}

fn is_sensitive_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let basename = Path::new(&normalized)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(normalized.as_str());

    if PATH_EXCEPTIONS
        .iter()
        .any(|exception| basename == *exception || normalized.ends_with(&format!("/{exception}")))
    {
        return false;
    }

    SENSITIVE_PATH_PATTERNS.iter().any(|pattern| {
        let pattern = pattern.to_ascii_lowercase();
        let directory_pattern = pattern.ends_with('/');
        basename == pattern
            || normalized.ends_with(&format!("/{pattern}"))
            || normalized.ends_with(&pattern)
            || normalized.contains(&format!("/{pattern}/"))
            || (directory_pattern && normalized.contains(&pattern))
            || basename.contains(&pattern)
    })
}
