use anyhow::Result;

/// Ship through 8 phases: checklist, dry-run, deploy, verify
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch ship: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
