use super::{HookInput, HookResponse};

/// Periodically remind the agent of core autoresearch rules during long runs.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    // Only inject if we haven't recently (managed via session state)
    // For now, this is a lightweight reminder every N prompts
    // The iteration-context hook handles the throttling

    HookResponse::allow()
}
