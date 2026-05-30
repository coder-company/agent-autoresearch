# Changelog

All notable changes to this project will be documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] — 2025-05-27

Initial release.

### Added

- **Core engine**: `init`, `verify`, `guard`, `log`, `decide`, `status`, `resume`, `progress`, `watch` CLI commands
- **State machine**: `RunPhase` enum (Setup → Baseline → Iterating → Complete/Blocked) with typed transitions
- **Results logging**: TSV format with iteration, commit, metric, delta, guard, status, description columns
- **State persistence**: `state.json` with full run context, resume support for interrupted sessions
- **Git integration**: libgit2-based revert and hard-reset rollback strategies, worktree status detection
- **Verify system**: scalar and `metrics_json` output formats, command screening for dangerous patterns
- **Escalation protocol**: 3-tier (refine → pivot → web search → stop) triggered by consecutive discards
- **Lessons log**: Markdown-based learnings that persist across sessions, with search and tail queries
- **12 subcommands**: improve, debug, fix, security, scenario, predict, learn, reason, probe, evals, ship, plan
- **Exec mode**: Non-interactive CI/CD mode — reads config from stdin, emits JSON lines
- **Background runtime**: `runtime run` managed relaunch loop plus `start/status/supervise/stop` artifacts, detached launch control, and relaunch/stop/needs_human supervisor recommendations
- **Parallel runtime**: `parallel prepare/run/closeout/cleanup` manages worker worktrees, records crashes/timeouts, cherry-picks verified winners, and logs one authoritative retained batch row
- **Handoff system**: Structured JSON handoff between modes for chained workflows
- **11 hook handlers**: session_init, session_end, iteration_context, stop_check, scout_block, dangerous_cmd, simplify_gate, compaction_reanchor, privacy_block, dev_rules_reminder, subagent_context
- **Claude Code plugin**: `.claude-plugin/plugin.json` manifest with hook definitions
- **Codex skill**: `.agents/skills/autoresearch/` for direct Codex installs, plus `plugins/autoresearch/` for the local Codex plugin marketplace
- **OpenCode package**: `.opencode/` commands and skill package with underscore command names
- **Agent commands**: `commands/autoresearch.md` root + 12 subcommand files
- **Reference docs**: 27 protocol and workflow reference documents
- **Release profile**: `opt-level = "z"`, LTO, strip, `panic = "abort"` — about 3MB binary with a 5MB contributor-gate ceiling
