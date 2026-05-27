use super::{HookInput, HookResponse};

/// Block tool calls that attempt to read/write outside declared scope during an active run.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

    // Check if there's an active scope constraint
    let scope_file = std::env::current_dir()
        .ok()
        .map(|d| d.join("autoresearch-results/state.json"));

    let _scope = match scope_file.and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(content) => {
            // Parse scope from state.json
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(v) => v.get("scope").and_then(|s| s.as_array()).cloned(),
                Err(_) => return HookResponse::allow(),
            }
        }
        None => return HookResponse::allow(),
    };

    // If tool is file-modifying, check against scope
    let tool = input.tool_name.as_deref().unwrap_or("");
    if !matches!(tool, "Write" | "Edit" | "Bash") {
        return HookResponse::allow();
    }

    // Extract target path from tool input
    let target_path = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("file_path").or(v.get("path")))
        .and_then(|v| v.as_str());

    let _target = match target_path {
        Some(p) => p,
        None => return HookResponse::allow(),
    };

    // TODO: Implement glob matching against scope
    // For now, allow — the full implementation will use the glob crate
    // to match target paths against the scope patterns from state.json

    HookResponse::allow()
}
