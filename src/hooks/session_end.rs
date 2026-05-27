use super::{HookInput, HookResponse};

/// Session end: no-op for now. Could persist session stats.
pub fn run(_input: Option<&HookInput>) -> HookResponse {
    HookResponse::allow()
}
