use anyhow::Result;

/// Non-interactive CI/CD mode with JSON output
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch exec: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
