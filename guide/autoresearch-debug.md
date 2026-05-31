# Debug — `autoresearch:debug`

Scientific bug hunting. Form hypotheses, test them, confirm or falsify, repeat. Every finding has file:line evidence.

## When to Use

- You have a bug but don't know the root cause
- Tests are failing and you're not sure why
- You want a systematic sweep of a codebase area
- You want to find bugs *without* fixing them (report-only mode)

Use `--fix` or `--chain fix` when you also want automatic repair.

## Syntax

```
/autoresearch:debug
Scope: src/auth/**/*.ts
Symptom: Login fails silently for OAuth users
```

## Real Examples

### Hunt All Bugs in a Module

```
/autoresearch:debug
Scope: src/api/**/*.ts
Iterations: 20
```

No symptom → autoresearch auto-scans (runs tests, lint, typecheck) to find failures.

### Investigate a Specific Error

```
/autoresearch:debug
Symptom: TypeError: Cannot read properties of undefined (reading 'id')
Scope: src/handlers/**/*.ts
--technique trace
```

### Debug and Fix in One Pass

```
/autoresearch:debug
Scope: src/**/*.ts
Symptom: 3 tests failing in CI
--fix
```

The `--fix` flag chains to `autoresearch:fix` after all bugs are found.

## Investigation Techniques

The agent selects the best technique per hypothesis:

| Technique | When Used |
|-----------|-----------|
| Binary search | You know when it worked, need to find when it broke |
| Differential | Compare working vs broken state |
| Minimal reproduction | Simplify to smallest failing case |
| Trace | Follow execution path from error backward |
| Pattern search | Grep for known anti-patterns |
| Working backwards | Start from error message, trace to root cause |

Force a technique with `--technique <name>`.

## Output

```
autoresearch-results/debug/debug-250527-1430/
├── debug-results.tsv    # Every hypothesis tested
└── handoff.json         # For chaining to fix
```

The TSV logs each hypothesis with status (confirmed/disproven/inconclusive), technique used, and evidence.

## Flags

| Flag | Purpose |
|------|---------|
| `--fix` | Auto-chain to fix after debug completes |
| `--severity <level>` | Filter: critical, high, medium, low |
| `--technique <name>` | Force a specific investigation technique |
| `--evals` | Periodic checkpoint reports |
| `Iterations: N` | Cap investigation depth (default 15) |

## Tips

- Debug doesn't modify code — it's read-only investigation
- Each hypothesis must be falsifiable: "I hypothesize X because Y. Test by Z."
- No hypothesis is repeated — the agent tracks what's been tested
- If all hypotheses exhausted before iteration cap → early stop
- The confirmed findings in handoff.json feed directly into `fix` with file:line precision
