use super::{HookInput, HookResponse};

/// Dangerous command patterns to block during autoresearch runs.
const DANGEROUS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "rm -rf .",
    ":(){ :|:& };:",
    "mkfs.",
    "> /dev/sd",
    "dd if=/dev/zero",
    "chmod -R 777 /",
    "git push --force",
    "git reset --hard", // Only allowed via approved rollback strategy
    "drop database",
    "drop table",
    "truncate table",
    "kubectl delete namespace",
    "docker system prune -af",
];

/// Commands that are only safe in specific contexts.
const CONTEXT_SENSITIVE: &[&str] = &[
    "npm publish",
    "cargo publish",
    "pip upload",
    "docker push",
    "helm install",
    "terraform apply",
    "terraform destroy",
];

/// Block dangerous commands during active autoresearch runs.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

    // Only applies to Bash tool calls
    if input.tool_name.as_deref() != Some("Bash") {
        return HookResponse::allow();
    }

    let command = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("command").or(v.get("cmd")))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if command.is_empty() {
        return HookResponse::allow();
    }

    let lower = command.to_lowercase();

    // Check absolute blockers
    for pattern in DANGEROUS {
        if lower.contains(&pattern.to_lowercase()) {
            return HookResponse::block(format!(
                "Blocked dangerous command during autoresearch: contains '{pattern}'"
            ));
        }
    }

    // Check context-sensitive (block during active runs)
    let has_active_run = std::env::current_dir()
        .ok()
        .map(|d| d.join("autoresearch-results/state.json").exists())
        .unwrap_or(false);

    if has_active_run {
        for pattern in CONTEXT_SENSITIVE {
            if lower.contains(&pattern.to_lowercase()) {
                return HookResponse::block(format!(
                    "Blocked during active autoresearch run: '{pattern}' requires explicit user approval"
                ));
            }
        }
    }

    HookResponse::allow()
}
