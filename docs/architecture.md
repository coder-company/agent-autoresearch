# Architecture

## Binary Dual-Use

The `autoresearch` binary serves two roles from the same executable:

1. **CLI tool** — direct invocation via `autoresearch init`, `autoresearch decide`, etc.
2. **Hook handler** — invoked by the Claude Code plugin system via `autoresearch hook <name>`

The entry point in `main.rs` dispatches through `clap` subcommands. Hook mode parses
the hook name and delegates to the corresponding handler in `src/hooks/`.

```
╭──────────────╮     ╭──────────────────╮     ╭───────────────╮
│  Agent call   │────▶│  autoresearch    │────▶│  CLI dispatch │
│  (or hook)    │     │  binary          │     │  (clap)       │
╰──────────────╯     ╰──────────────────╯     ╰───────┬───────╯
                                                       │
                      ╭────────────────────────────────┼────────────────╮
                      │                                │                │
                      ▼                                ▼                ▼
              ╭──────────────╮                ╭──────────────╮  ╭─────────────╮
              │  CLI command │                │  Hook handler│  │  Exec mode  │
              │  (init/log/  │                │  (scout,     │  │  (CI/CD     │
              │   decide/..) │                │   stop,..)   │  │   JSON-line)│
              ╰──────┬───────╯                ╰──────┬───────╯  ╰──────┬──────╯
                     │                               │                 │
                     ▼                               ▼                 ▼
              ╭──────────────────────────────────────────────────────────────╮
              │                       src/core/                             │
              │  config.rs  state.rs  results.rs  git.rs  verify.rs        │
              ╰─────────────────────────────────────────────────────────────╯
```

## Module Breakdown

### `src/core/` — Shared foundation

| File | Purpose |
|------|---------|
| `config.rs` | `RunConfig`, `Direction`, `Mode`, `VerifyFormat`, `RollbackStrategy` |
| `state.rs` | `RunState`, `RunPhase` (state machine), `IterationStatus`, `StopReason` |
| `results.rs` | `ResultRow`, `ResultsLog` (TSV append/read), completion summary |
| `git.rs` | `GitRepo` wrapper around libgit2 — HEAD, revert, reset, worktree status |
| `verify.rs` | Run verify/guard commands, parse scalar or JSON output, screen for danger |
| `metrics.rs` | Metric parsing utilities, decimal handling |

### `src/hooks/` — Claude Code hook handlers

Each hook is a function that reads minimal state, makes a decision, and prints output.
Hooks must complete in <5ms. No network calls, no heavy I/O.

| Hook | Fires on | Purpose |
|------|----------|---------|
| `session_init` | Session start | Detect interrupted runs, load state |
| `session_end` | Session end | Write final state, cleanup |
| `iteration_context` | UserPromptSubmit | Inject iteration number + last result |
| `stop_check` | Stop | Check if iteration cap reached |
| `scout_block` | PreToolUse: Write/Edit/MultiEdit/Bash/Glob/Grep/Read | Block generated/vendor/sensitive paths, Bash reads, and out-of-scope writes |
| `dangerous_cmd` | PreToolUse: Bash | Screen for `rm -rf`, `DROP TABLE`, etc. |
| `simplify_gate` | UserPromptSubmit | Enforce "equal metric + less code = keep" |
| `compaction_reanchor` | Context compaction | Re-inject critical state after compaction |
| `privacy_block` | PreToolUse: Write/Edit/MultiEdit/Bash/Glob/Grep/Read | Block credential paths and secret-looking inputs; warn on sensitive Bash paths |
| `dev_rules_reminder` | UserPromptSubmit | Remind agent of project conventions |
| `subagent_context` | Subagent spawn | Inject autoresearch state into subagent prompt |

### `src/escalation/` — Failure recovery

| File | Purpose |
|------|---------|
| `pivot.rs` | `EscalationState` — tracks consecutive discards, triggers refine/pivot/search |
| `lessons.rs` | `LessonsLog` — append/search/read lessons.md |

### `src/modes/` — Mode-specific logic

Each mode file contains the structured output types and validation logic for that
subcommand. The actual iteration orchestration is done by the agent reading the
corresponding command markdown file.

### `src/agents/` — Multi-agent support

Agent detection, context injection for different agent runtimes (Claude Code, Codex CLI).

## State Machine

The `RunPhase` enum enforces valid transitions at the type level:

```
Setup → Baseline { metric }
Baseline → Iterating { iteration, current, best, best_iteration }
Iterating → Iterating (on keep/discard/crash/no-op)
Iterating → Complete { reason }
Iterating → Blocked { reason }
Blocked → Iterating (on resume)
```

`RunState` persists to `autoresearch-results/state.json` after every iteration.
On resume, the binary reads state.json and reconstructs the full context.

## Data Flow

```
Agent decides to modify code
    │
    ▼
autoresearch verify --command "..." → metric (Decimal)
    │
    ▼
autoresearch guard --command "..."  → pass/fail (optional)
    │
    ▼
autoresearch decide --decision auto --metric N --metrics-json '{...}'
    │
    ├── keep:    state.record_keep() → update state.json, append TSV
    └── discard: state.record_discard() → rollback, update state.json, append TSV
```
