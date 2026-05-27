use anyhow::Result;

/// STRIDE + OWASP audit with red-team personas
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch security: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
