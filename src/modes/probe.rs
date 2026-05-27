use anyhow::Result;

/// 8 personas interrogate requirements until saturation
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch probe: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
