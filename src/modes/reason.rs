use anyhow::Result;

/// Adversarial debate with blind judges until convergence
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch reason: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
