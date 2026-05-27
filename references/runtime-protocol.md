# Runtime Protocol

## Closeout Order

For each iteration:
1. Finish the experiment (modify files).
2. Create scoped trial commit (`experiment: <desc>`).
3. Run verify command.
4. Run guard command (if configured).
5. Decide keep/discard/crash.
6. Apply rollback for non-kept trials (`git revert HEAD --no-edit`).
7. Log the iteration via `autoresearch log`.
8. Choose next idea.

## State Machine

```
Setup → Baseline → Iterating → Complete|Blocked
```

- **Setup**: scanning repo, confirming config.
- **Baseline**: first verify measurement, artifacts initialized.
- **Iterating**: the loop. Each turn is one iteration.
- **Complete**: goal reached, cap hit, or user stopped.
- **Blocked**: hard blocker (broken verify, disk full, 3+ PIVOTs).

## Artifacts (never committed)

All under `autoresearch-results/`:
- `results.tsv` — TSV log of every iteration
- `state.json` — machine-readable state snapshot
- `lessons.md` — extracted insights for future runs

## TSV Format

```tsv
# metric_direction: higher
iteration	commit	metric	delta	guard	status	description
0	a1b2c3d	85.2	0	-	baseline	initial state
1	b2c3d4e	87.1	+1.9	pass	keep	add auth edge case tests
2	-	86.5	-0.6	-	discard	refactor broke 2 tests
```

## Verify Contract

The verify command MUST output a single number on its final non-empty line.

```bash
# Good
npm test -- --coverage | grep "All files" | awk '{print $10}'
# Output: 85.2

# Good
grep -rc ':any' src/ | awk -F: '{s+=$2}END{print s}'
# Output: 47

# Bad (multiple lines of text, no clear number)
npm test
```

## Guard Contract

The guard command must exit 0 (pass) or non-zero (fail). No output parsing.

## Rollback

Default: `git revert HEAD --no-edit` (safe, creates revert commit).
Optional (dedicated branch only): `git reset --hard HEAD~1`.

## Stop Conditions

Hard blockers (stop immediately):
- Verify command no longer exists or produces unparseable output
- Scope files deleted externally
- Git repo broken
- Same crash 5+ times in a row
- External changes detected in worktree mid-loop

## Progress Reporting

Every 5 iterations, summarize:
- Baseline vs current vs best
- Keep/discard/crash counts
- Next likely direction
