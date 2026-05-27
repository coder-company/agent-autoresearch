use anyhow::Result;

/// Scout codebase, generate docs, validate, fix
pub fn run(_args: &[String]) -> Result<()> {
    eprintln!("autoresearch learn: awaiting config on stdin (JSON)");
    // Mode implementation will be expanded
    // The agent (Claude Code or Codex) drives the interactive part
    // This binary handles the mechanical execution
    Ok(())
}
