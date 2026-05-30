# Advanced Patterns

Patterns for power users: parallel experiments, multi-repo setups, CI/CD integration, and long-running overnight sessions.

---

## Parallel Experiments (Git Worktrees)

Test up to 3 hypotheses simultaneously using git worktrees:

```
/autoresearch
Goal: Reduce API latency below 50ms
Scope: src/api/**/*.ts
Verify: npm run bench:api | grep p95 | awk '{print $2}'
Direction: lower
Guard: npm test
```

When the loop detects a plateau (3+ consecutive discards with different strategies), it may attempt parallel experiments:

1. Create worktrees: `git worktree add ../experiment-a`, `../experiment-b`, `../experiment-c`
2. Apply 3 different hypotheses simultaneously
3. Run verify in each worktree
4. Keep the best result, discard others
5. Merge winning worktree back to main branch

### When Parallel Fires

- After a REFINE that didn't work
- When 3+ plausible hypotheses exist but none are clearly best
- Not in the first 5 iterations (need baseline exploration first)

### Limitations

- Requires clean working tree
- Worktrees share the .git directory
- Guard commands must work in the worktree context
- Max 3 parallel experiments per batch

---

## Multi-Repo Workspaces

For tasks that span multiple repositories:

```
/autoresearch
Goal: Reduce integration test failures to zero
Scope: api-service/src/**/*.ts, frontend/src/**/*.ts
Verify: cd api-service && npm test && cd ../frontend && npm test | grep "failing" | awk '{print $1}'
Direction: lower
```

The primary repo is where autoresearch-results/ lives. Companion repos must be accessible via relative paths.

### Rules for Multi-Repo

1. Declare all repos in scope explicitly
2. One repo is primary (owns artifacts)
3. Commit atomically within each repo
4. Guard must pass across all repos
5. Revert targets only the repo that changed

---

## CI/CD Integration (Exec Mode)

Non-interactive mode for automation pipelines:

```bash
echo '{"goal":"coverage above 90","scope":["src/**/*.ts"],"metric":"coverage %","direction":"higher","verify":"npm test -- --coverage | grep All | awk \"{print \\$4}\""}' | \
  autoresearch exec --iterations 10
```

Exec mode:
- Reads RunConfig from stdin (JSON)
- Emits JSON lines on stdout (started, iteration, complete, blocked, error)
- Exit codes: 0 (goal met), 1 (bounded/blocked), 2 (error)
- No interactive questions, no prompt injection

### GitHub Actions Integration

```yaml
- name: Autoresearch optimization
  run: |
    echo '${{ env.AUTORESEARCH_CONFIG }}' | autoresearch exec --iterations 20
  env:
    AUTORESEARCH_CONFIG: |
      {"goal":"zero lint warnings","scope":["src/**/*.ts"],"metric":"warning count","direction":"lower","verify":"eslint src/ --format compact 2>&1 | grep -c Warning || echo 0","guard":"npm test"}
```

---

## Overnight Runs

For long-running optimization sessions:

```
/autoresearch
Goal: Maximize test coverage
Scope: src/**/*.ts
Verify: npx jest --coverage --silent | grep "All files" | awk '{print $4}'
Guard: npx tsc --noEmit && npm run lint
Iterations: unlimited
```

### Best Practices for Overnight

1. **Always set a Guard** — prevents shipping broken code
2. **Use Git as backup** — every experiment is committed, failures reverted
3. **Check results.tsv in the morning** — full audit trail
4. **Set realistic scope** — narrower scope = faster iterations = more experiments per night
5. **Consider bounded first** — try `Iterations: 50` before unlimited

### Monitor From Another Terminal

```bash
autoresearch watch --lines 20
autoresearch runtime status
```

`watch` prints the active `results.tsv` header plus recent rows, then follows new rows until you stop it. Use `--once` for scripts or quick snapshots.

### Recovery After Interruption

If a session is interrupted:

```bash
autoresearch resume
```

Returns the state of the last run. The agent can pick up from where it left off using `autoresearch-results/state.json`.

---

## Eval Checkpoints for Long Runs

Monitor progress without interrupting:

```
/autoresearch
Goal: Reduce bundle size
Verify: npm run build && stat -c%s dist/main.js
Direction: lower
Iterations: 100
--evals-interval 20
```

Every 20 iterations, a checkpoint fires:

```
--- Eval Checkpoint (iterations 1-20) ---
Metric: 245000 → 198000 (-47000) | Kept: 8/20 | Trend: improving
Continue — strong downward trend
---
```

If 3+ checkpoints show a plateau, the system recommends stopping early.

---

## Custom Verify Scripts

For complex metrics, write a dedicated script:

```bash
#!/bin/bash
# verify-quality.sh
set -e

# Run tests
TEST_RESULT=$(npm test -- --coverage --silent 2>&1)
COVERAGE=$(echo "$TEST_RESULT" | grep "All files" | awk '{print $4}')

# Run type check
TYPE_ERRORS=$(npx tsc --noEmit 2>&1 | grep -c "error TS" || echo 0)

# Composite score: coverage - (type_errors * 5)
echo "$COVERAGE - ($TYPE_ERRORS * 5)" | bc
```

```
/autoresearch
Goal: Maximize composite quality score
Verify: ./verify-quality.sh
Guard: npm test
```

---

## Lessons Across Runs

The `autoresearch-results/lessons.md` file persists across runs. Strategies that worked before bias future hypothesis generation:

```bash
# Query lessons
autoresearch lessons --search "coverage"
autoresearch lessons --last 5
```

### Manual Lesson Injection

Add insights manually to `autoresearch-results/lessons.md`:

```markdown
- ✅ [2024-03-15] **Mock external APIs in tests** — reduces flakiness (worked)
- ❌ [2024-03-15] **Increase timeout values** — masks real issues (failed)
- 🔄 [2024-03-14] **Pivoted from: unit tests only** — integration tests found more bugs (neutral)
```

---

## Health Checks

Before each iteration, the system checks:

| Check | What | Recovery |
|-------|------|----------|
| Disk space | > 100MB free | Warn and stop |
| Git state | Clean worktree | Stash or abort |
| Verify command | Exits successfully | Re-screen, alert |
| State integrity | state.json valid | Rebuild from TSV |
| Lock files | No stale locks | Remove and continue |

Run manually:

```bash
autoresearch status
```

---

## Environment Detection

The binary auto-detects:
- Agent type (Claude Code, Codex, generic)
- Git repo presence and branch
- Available tooling (npm, cargo, pytest, etc.)
- OS and architecture
- Shell availability

This informs the verify command suggestions in `/autoresearch:plan`.
