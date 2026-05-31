# Evals — `autoresearch:evals`

Post-hoc analysis of iteration results. Reads a TSV file and reports trends, plateaus, regressions, patterns, and recommendations.

## When to Use

- After a run completes — understand what happened
- Mid-run analysis — the `--evals` flag on other commands uses this internally
- Compare patterns — which strategies worked, which failed
- Decide whether to continue or stop

## Syntax

```
/autoresearch:evals autoresearch-results/results.tsv
```

Or let it auto-discover:

```
/autoresearch:evals
```

Scans `autoresearch-results/` and `autoresearch/*/` for TSV files.

## Real Examples

### Analyze Last Run

```
/autoresearch:evals
```

### Specific File

```
/autoresearch:evals autoresearch/fix-250527-1430/fix-results.tsv
--format md
```

### JSON Output for Tooling

```
/autoresearch:evals autoresearch-results/results.tsv
--format json
```

### Analyze Then Chain

```
/autoresearch:evals autoresearch-results/results.tsv
--recommend
--chain ship
```

Writes `handoff.json` beside the TSV with the recommendation, findings, and next target.

### Compare Runs

```
/autoresearch:evals --file autoresearch-results/results.tsv
--compare autoresearch-results/previous-results.tsv
--format json
```

Reports the winning run plus improvement, efficiency, and plateau deltas.

### Gate on Target

```
/autoresearch:evals --file autoresearch-results/results.tsv
--target 90
--recommend
--format json
```

Reports `goal_achieved` and switches the recommendation to `goal_met` when the final metric crosses the threshold for the run direction.

### Fail CI on Gate

```
/autoresearch:evals --file autoresearch-results/results.tsv
--target 90
--fail-on goal-not-met
--format json
```

Prints the JSON report, then exits non-zero when the selected gate condition is met. Conditions: `no-go`, `hold`, `goal-not-met`, `anomaly`.

## What It Reports

```
## Evals Summary — core loop (25 iterations)

### Key Metrics
- Total iterations: 25 | Kept: 14 | Reverted: 11 | Revert rate: 44%
- Starting metric: 47 | Final metric: 12 | Improvement: 74%

### Trend Analysis
- Metric progression: rapid improvement (iter 1-8), then diminishing returns
- Plateau detected at iteration 19 (metric stable for 6 iterations)
- Biggest win: iteration 3 (+8, type-narrowed entire auth module)
- Biggest loss: iteration 7 (-2, overly generic wrapper)

### Patterns
- What succeeded: targeted type narrowing, one-module-at-a-time approach
- What failed: generic wrappers, broad refactoring attempts
- File hotspots: src/auth/session.ts (4 kept changes)

### Recommendation
- STOP: plateau detected, remaining gains require different approach
- Suggestion: expand scope to include test files, or change metric
```

## Column-Aware Analysis

Evals adapts based on which columns exist in the TSV:

| Column | Unlocks |
|--------|---------|
| `metric` | Trend, plateau, diminishing returns |
| `delta` | Per-iteration efficiency, effort ratio |
| `status` | Keep/discard rate, success streaks |
| `severity` | Severity distribution, critical discovery rate |
| `hypothesis` | Confirmation rate, technique effectiveness |
| `dimension` | Coverage completeness (X/12) |
| `error_type` | Category distribution, fix rate per type |
| `convergence_count` | Convergence trajectory |

Unknown columns are reported but not analyzed — forward-compatible with future commands.

## Mid-Loop Checkpoints

When other commands use `--evals`, they run mini-eval checkpoints at intervals:

```
--- Eval Checkpoint (iterations 8-16) ---
Metric: 35 → 28 (-7) | Kept: 4/8 | Trend: down (good)
Continue — still making progress
---
```

Interval formula: `floor(max_iterations / 3)`, minimum 1. Fixed 10 for unbounded. Override with `--evals-interval N`.

If plateau detected for 3+ consecutive checkpoints → recommends early stop.

## Output

- Console: 30-50 line structured report
- `--format md` → writes `evals-summary.md` next to the input TSV
- `--format json` → writes `evals-summary.json` with structured data
- Anomalies → reports plateaus, failure streaks, guard failures, and declining trends when present
- `--target <number>` → reports goal achievement and a `goal_met` recommendation when achieved
- `--fail-on <condition>` → exits non-zero for CI gates after printing the report
- `--compare <path>` → includes cross-run deltas and a winner
- `--chain <targets>` → writes `handoff.json` next to the input TSV for downstream commands

## Backward Compatibility

- Reads v2.0.03 TSV files (missing `timestamp` column handled gracefully)
- Fuzzy column matching: `metric_value` → `metric`, `error_count` → `metric`
- Finds TSVs in project root or `autoresearch/` subdirectories

## Tips

- Run evals after every significant run — it's the retrospective
- The "Patterns" section tells you what strategy family to use next time
- Plateau detection saves time: if evals says stop, trust it
- `--format json` is useful for dashboards or trend tracking across runs
- The recommendation is actionable — "continue", "stop", or "change strategy"
