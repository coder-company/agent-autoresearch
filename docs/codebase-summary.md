# Autoresearch — Codebase Summary

> AI-friendly reference for agents working on this codebase.

## Entry Points

| Path | Purpose |
|------|---------|
| `src/main.rs` | CLI entry — Clap-based command dispatch (init, verify, guard, log, decide, evals, status, screen, hook, resume, progress, lessons, handoff, exec) |
| `hooks/hooks.json` | Claude Code plugin hook definitions — maps lifecycle events to binary invocations |
| `skills/autoresearch/SKILL.md` | Agent skill file — iteration protocol, subcommand table, references |
| `.agents/skills/autoresearch/` | Maintained Codex skill package used by direct Codex installs |
| `plugins/autoresearch/` | Codex plugin package generated from the `.agents` skill package |
| `.agents/plugins/marketplace.json` | Local Codex marketplace entry pointing at `plugins/autoresearch/` |
| `commands/autoresearch.md` | Root command protocol (core iteration loop) |
| `commands/autoresearch/*.md` | Subcommand protocols (debug, fix, security, scenario, etc.) |

## Core Modules (`src/core/`)

| File | Responsibility |
|------|---------------|
| `config.rs` | `RunConfig`, `Direction`, `VerifyFormat`, `RollbackStrategy` types |
| `git.rs` | `GitRepo` wrapper — status, head, revert, worktree checks |
| `verify.rs` | Run verify commands, parse scalar/JSON output, safety screening |
| `results.rs` | `ResultsLog` — append TSV rows, read history |
| `state.rs` | `RunState` — iteration count, metrics, keeps/discards, phase tracking |
| `metrics.rs` | Metric parsing, delta calculation, direction comparison |

## Escalation (`src/escalation/`)

| File | Responsibility |
|------|---------------|
| `pivot.rs` | `EscalationState` — track consecutive discards, trigger REFINE/PIVOT/SEARCH/STOP |
| `lessons.rs` | `LessonsLog` — read/write/search cross-run learning entries |

## Hooks (`src/hooks/`)

| File | Hook | Fires On |
|------|------|----------|
| `scout_block.rs` | scout-block | PreToolUse (Write/Edit/MultiEdit/Bash/Glob/Grep/Read) — blocks generated paths, Bash reads, and out-of-scope writes |
| `privacy_block.rs` | privacy-block | PreToolUse — blocks access to sensitive paths |
| `dangerous_cmd.rs` | dangerous-cmd-block | PreToolUse (Bash) — blocks rm -rf, fork bombs, etc. |
| `iteration_context.rs` | iteration-context | UserPromptSubmit — injects run state into agent context |
| `simplify_gate.rs` | simplify-gate | UserPromptSubmit — reminds agent of simplicity rule |
| `stop_check.rs` | stop-check | Stop — detects premature stop during active run |
| `compaction_reanchor.rs` | compaction-reanchor | PostCompact — re-injects critical state after context compaction |
| `session_init.rs` | session-init | SessionStart — detects interrupted runs |
| `session_end.rs` | session-end | SessionEnd — writes final state |
| `subagent_context.rs` | subagent-context | SubagentStart — passes run context to subagents |

## Modes (`src/modes/`)

Thin logic for mode-specific state (most protocol lives in markdown commands):
`loop_mode.rs`, `debug.rs`, `fix.rs`, `security.rs`, `scenario.rs`, `predict.rs`, `reason.rs`, `probe.rs`, `learn.rs`, `ship.rs`, `evals.rs`, `improve.rs`, `plan.rs`

## Agents (`src/agents/`)

| File | Purpose |
|------|---------|
| `claude.rs` | Claude Code-specific integration helpers |
| `codex.rs` | Codex CLI-specific integration helpers |

## Data Flow

```
User prompt → [hook: iteration-context injects state]
           → Agent reads state + TSV + git log
           → Agent makes ONE change
           → Agent calls: autoresearch verify --command "..."
           → Binary runs command, returns metric + metrics JSON
           → Agent calls: autoresearch decide --decision auto --metric N --metrics-json '{...}'
           → Binary: evaluates criteria, updates state.json, appends TSV, reverts if discard
           → [hook: stop-check ensures agent doesn't quit early]
           → Next iteration
```

## Key Types

| Type | Location | Fields |
|------|----------|--------|
| `RunConfig` | core/config.rs | verify, direction, format, scope, guard, primary_metric_key |
| `RunState` | core/state.rs | iteration, baseline_metric, current_metric, best_metric, keeps, discards, crashes, consecutive_discards, phase |
| `ResultRow` | core/results.rs | iteration, commit, metric, delta, guard, status, description |
| `EscalationState` | escalation/pivot.rs | consecutive_discards, pivots, last_action |
| `Direction` | core/config.rs | Higher, Lower |
| `IterationStatus` | core/state.rs | Baseline, Keep, Discard, Crash, NoOp, Blocked, Pivot, Refine, Search |

## How to Add...

### A new CLI command
1. Add variant to `Commands` enum in `src/main.rs`
2. Add match arm in `main()` dispatching to `cmd_<name>()` function
3. Implement function at bottom of `main.rs` (or extract to module if >100 lines)

### A new hook
1. Add handler in `src/hooks/<name>.rs`
2. Register in `src/hooks/mod.rs`
3. Add hook entry in `hooks/hooks.json` under the appropriate lifecycle event
4. Hook receives JSON on stdin, returns JSON on stdout, must complete in <5ms

### A new mode/subcommand
1. Add command protocol in `commands/autoresearch/<mode>.md`
2. Add mode-specific state logic in `src/modes/<mode>.rs` (if needed)
3. Register in `src/modes/mod.rs`
4. Update SKILL.md subcommand table
