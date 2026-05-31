# Fix — `autoresearch:fix`

Crush errors one by one until zero remain. Works with test failures, type errors, lint warnings, or build errors.

## When to Use

- Test suite has failures you want eliminated
- Type checker reports errors
- Linter has warnings to resolve
- Build is broken
- You just ran `debug` and want to fix what was found (`--from-debug`)

## Syntax

```
/autoresearch:fix
Target: npm test
Scope: src/**/*.ts
Guard: tsc --noEmit
```

## Real Examples

### Fix All Test Failures

```
/autoresearch:fix
Target: npm test
Scope: src/**/*.ts
Iterations: 20
```

### Fix Type Errors Only

```
/autoresearch:fix
Target: tsc --noEmit 2>&1 | grep "error TS" | wc -l
Scope: src/**/*.ts
--category type
```

### Chain from Debug

```
/autoresearch:debug
Scope: src/**/*.ts
--fix
```

Or use handoff directly:

```
/autoresearch:fix
--from-debug
```

Reads `handoff.json` from the last debug run for scope and findings.

## How It Works

1. Run target command → count errors (metric = error count, direction = lower)
2. Pick highest-priority error (crash → test failure → type error → lint)
3. Fix ONE error with ONE atomic commit
4. Run target again → verify error count decreased
5. Run guard → verify nothing else broke
6. Keep if count decreased, revert if not
7. Repeat until zero errors or iteration cap

## Priority Order

Within each category, easiest first (single-file fixes before cross-file):

1. Crashes / fatal errors
2. Test failures
3. Type errors
4. Lint errors
5. Warnings

## Output

```
autoresearch-results/fix/fix-250527-1430/
├── fix-results.tsv     # Every fix attempt
└── handoff.json        # Remaining errors for chaining
```

## Flags

| Flag | Purpose |
|------|---------|
| `--from-debug` | Read handoff.json from previous debug run |
| `--category <type>` | Filter: test, type, lint, build |
| `--evals` | Periodic progress reports |
| `Guard: <cmd>` | Safety command that must always pass |
| `Iterations: N` | Cap (default 20) |

## Tips

- Fix makes ONE change per iteration — this is deliberate. Atomic fixes are easier to revert.
- If a fix breaks the guard, it's reverted even if it fixed the target error.
- The agent prioritizes easiest fixes first — high success rate early, harder fixes later.
- Use `--category type` or `--category lint` to focus on one error class.
- Fix naturally reports remaining errors at the end — chain to another fix run if needed.
