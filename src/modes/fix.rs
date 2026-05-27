use anyhow::Result;

/// Crush errors one-by-one until zero remain
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch fix: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
