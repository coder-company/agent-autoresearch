# Core Loop — `autoresearch`

The primary command. Iterate against any measurable metric until it improves or you run out of turns.

## When to Use

Use the core loop when you have:
- A single numeric metric to optimize
- A shell command that outputs that metric
- Files you want modified (scope)

If you're unsure how to configure these, use `/autoresearch:plan` first.

## Syntax

```
/autoresearch
Goal: <what to improve>
Scope: <file globs>
Metric: <what's being measured>
Verify: <command that outputs a number>
Guard: <optional safety command>
Iterations: <N or unlimited>
```

## Real Examples

### Increase Test Coverage

```
/autoresearch
Goal: Increase test coverage from 72% to 90%
Scope: src/**/*.ts, tests/**/*.ts
Verify: npm test -- --coverage 2>&1 | grep "All files" | awk '{print $10}'
Guard: tsc --noEmit
Iterations: 30
```

### Reduce Bundle Size

```
/autoresearch
Goal: Reduce production bundle below 200KB
Scope: src/**/*.{ts,tsx}
Verify: npm run build 2>&1 | grep "bundle size" | awk '{print $3}'
Guard: npm test
Iterations: 20
```

### Lower API Latency

```
/autoresearch
Goal: Reduce p95 latency below 100ms
Scope: src/api/**/*.ts
Verify: npm run bench -- --json | jq '.p95'
Guard: npm test
Iterations: 25
```

## The Loop in Detail

Each iteration follows 9 phases:

1. **Read** — Check results TSV, git log, lessons file
2. **Ideate** — Form one testable hypothesis
3. **Modify** — Make one focused change within scope
4. **Trial Commit** — `git commit -m "experiment: ..."`
5. **Verify** — Run verify command, extract metric
6. **Guard** — Run guard command (if configured)
7. **Decide** — Keep (improved) or discard (reverted)
8. **Log** — Append result to TSV, update state
9. **Escalation Check** — REFINE/PIVOT if stuck

## Flags

| Flag | Purpose |
|------|---------|
| `--evals` | Enable periodic progress checkpoints |
| `--evals-interval N` | Override checkpoint frequency |
| `--chain <targets>` | Chain to other commands after completion |
| `Iterations: unlimited` | No turn cap (runs until goal or interrupt) |

## Escalation System

The loop never gets permanently stuck:

- **3 consecutive discards → REFINE:** Adjust within current strategy
- **5 consecutive discards → PIVOT:** Fundamentally new approach
- **2 PIVOTs without keep → Web Search:** Look externally for solutions
- **3 PIVOTs without keep → Soft Blocker:** Stop and report

A single `keep` resets all escalation counters.

## Output

```
autoresearch-results/
├── results.tsv       # Every experiment (kept + discarded)
├── state.json        # Current metric, best metric, counts
├── lessons.md        # Extracted insights for future runs
└── context.json      # Run configuration
```

## Tips

- **Start bounded.** Use `Iterations: 25` until you trust the setup.
- **Fast verify wins.** A 2-second verify command means more experiments per hour than a 30-second one.
- **Tight scope.** Narrower scope = fewer false starts.
- **Guard everything.** If you have tests, use them as a guard to prevent regressions.
- **Check lessons.** The lessons file carries wisdom across runs. Future sessions start smarter.
