use super::{HookInput, HookResponse};
use regex::Regex;
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

/// Block operations that would expose sensitive data.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

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
