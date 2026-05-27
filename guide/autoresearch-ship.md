# Ship — `autoresearch:ship`

8-phase ship workflow: identify what to ship, generate a checklist, prepare, dry-run, ship, verify. Works for code PRs, releases, deployments, content, and packages.

## When to Use

- You're ready to merge/deploy and want a safety net
- You want a pre-flight checklist before pushing
- You need a dry-run simulation
- You want post-ship monitoring
- You need to rollback a bad deploy

## Syntax

```
/autoresearch:ship
Target: current branch
--type code-pr
```

## Real Examples

### Ship a PR

```
/autoresearch:ship
Target: feature/auth-refactor
--type code-pr
```

### Dry-Run Only

```
/autoresearch:ship
Target: current branch
--dry-run
```

Shows what would happen without executing. Good for validating before the real thing.

### Deploy with Monitoring

```
/autoresearch:ship
Target: v2.1.0
--type deployment
--monitor 10
```

Ships, then watches for 10 minutes via smoke tests and error monitoring.

### Auto-Ship (CI Mode)

```
/autoresearch:ship
Target: main
--type code-release
--auto
```

Auto-approves if zero errors found during preparation. Use in automation.

### Rollback

```
/autoresearch:ship
--rollback
```

Reverses the last ship action (reverts PR, undeploys, unpublishes).

## The 8 Phases

1. **Identify** — Detect ship type from context (PR, release, deploy, content)
2. **Inventory** — Gather changed files, deps, configs, migrations
3. **Checklist** — Generate domain-specific gates (tests, types, lint, secrets, etc.)
4. **Prepare** — Run checks, flag blockers vs warnings
5. **Dry-Run** — Simulate without side effects
6. **Ship** — Execute (requires explicit approval unless `--auto`)
7. **Verify** — Confirm artifact is live, run smoke tests
8. **Log** — Record everything in ship log

## Ship Types

| Type | Auto-detected When |
|------|-------------------|
| `code-pr` | Uncommitted changes or open PR |
| `code-release` | Version bump + changelog |
| `deployment` | Dockerfile or deploy config present |
| `content` | Markdown or content files changed |
| `package` | package.json version change |

Override with `--type <type>`.

## Output

```
autoresearch/ship-250527-1430/
├── checklist.md    # Pass/fail per item
├── summary.md      # What shipped, verification results
└── ship-log.tsv    # Phase-by-phase log
```

## Flags

| Flag | Purpose |
|------|---------|
| `--type <type>` | Override auto-detected ship type |
| `--dry-run` | Validate without shipping |
| `--auto` | Auto-approve if zero blockers |
| `--force` | Skip non-critical items (blockers still enforced) |
| `--rollback` | Undo last ship action |
| `--monitor N` | Post-ship monitoring for N minutes |
| `--checklist-only` | Generate checklist without executing |

## Tips

- Ship never executes without explicit user approval (unless `--auto` with zero errors)
- Use `--checklist-only` to see what gates exist before committing to ship
- The agent distinguishes blockers (must fix) from warnings (can ship with)
- Chain `debug → fix → ship` for a full investigation-to-deploy pipeline
- Rollback is type-aware: reverts PRs, undeploys, or unpublishes depending on what shipped
