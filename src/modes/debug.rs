use anyhow::Result;

/// Hunt bugs: hypothesize, test, falsify, repeat
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch debug: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
