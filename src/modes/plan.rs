use anyhow::Result;

/// Convert goal into validated Scope, Metric, Verify config
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch plan: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
