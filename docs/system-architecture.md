# System Architecture

Autoresearch is a Rust binary plus agent-facing instruction packs. The binary owns
mechanical state transitions; agents own reasoning, code edits, and hypothesis
selection.

## Components

| Component | Role |
|-----------|------|
| `autoresearch` binary | CLI, hook dispatcher, verifier, rollback controller, runtime supervisor |
| `.claude-plugin/marketplace.json` | Claude marketplace manifest pointing at the repo-root plugin package |
| `commands/` | Claude Code slash command instructions |
| `skills/autoresearch/` | Claude/OpenCode skill package and shared references |
| `.agents/skills/autoresearch/` | Codex/generic agent skill package |
| `.agents/plugins/marketplace.json` | Local Codex marketplace root for the packaged plugin |
| `plugins/autoresearch/` | Codex plugin package generated from `.agents/skills/autoresearch/` |
| `.opencode/` | OpenCode command, skill, and helper-agent distribution |
| `references/` | Protocol source docs copied into installable packages |
| `autoresearch-results/` | Runtime artifacts created inside the user's target repo |

## Runtime Flow

```text
agent chooses one hypothesis
    |
    v
edits scoped files and creates a trial commit
    |
    v
autoresearch verify runs the metric command
    |
    v
autoresearch guard runs the regression command when configured
    |
    v
autoresearch decide keeps, discards, logs, and updates state
```

The binary writes `results.tsv` and `state.json` after each decision. A discarded
experiment is rolled back automatically, while a kept experiment remains in git
history as the next baseline.

## Parallel Flow

Parallel work is recorded as a batch:

```bash
autoresearch parallel prepare --workers 3
autoresearch parallel run --manifest autoresearch-results/parallel-manifest.json --timeout-seconds 1200
autoresearch parallel closeout --batch-file autoresearch-results/parallel-workers.json
autoresearch parallel cleanup --manifest autoresearch-results/parallel-manifest.json
```

Prepare creates branch-backed worker worktrees, prompt files, a manifest, and the
editable batch file. Run executes the prepared worker prompts in those worktrees
and records crashed or timed-out workers in the manifest.
Closeout cherry-picks the best worker, re-runs verify and guard in the main
worktree, falls back to the next worker on merge or verification failure, then
writes worker audit rows and one authoritative retained batch row. Cleanup
removes worker worktrees and branches.

## Background Runtime

`autoresearch runtime run` supervises Codex execution through persisted artifacts:

| Artifact | Purpose |
|----------|---------|
| `launch.json` | Command, cwd, repo targets, goal, iteration limit, and stop criteria |
| `runtime.json` | Current status and supervisor recommendation |
| `runtime.log` | Detached runtime output |

Manual controls remain available through `runtime start`, `runtime status`,
`runtime supervise`, and `runtime stop`.

## More Detail

See [Architecture](architecture.md) for module-level internals and
[Guide](GUIDE.md) for user-facing command flow.
