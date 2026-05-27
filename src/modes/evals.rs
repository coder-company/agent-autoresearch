use anyhow::Result;

/// Analyze iteration results: trends, plateaus, regressions
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch evals: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
