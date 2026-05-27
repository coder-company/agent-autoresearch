---
name: autoresearch_evals
description: "Analyze iteration results: trends, plateaus, regressions, recommendations"
argument-hint: "[path/to/results.tsv] [--format text|json|md]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- Positional path to a specific TSV file
- `--format` — output format: text (default), json, md (markdown file)

## Input Discovery

1. If path provided → use that TSV directly
2. If no path → scan for `autoresearch-results/results.tsv` and `autoresearch/*/` files
3. If multiple found → ask user which to analyze
4. If none found → ask user for path

## Parse TSV

1. Line 1: extract `# metric_direction:` comment
2. Line 2: header row → detect columns
3. Remaining: data rows

## Analysis (based on columns present)

| Column | Analysis |
|---|---|
| `metric` | Trend, plateau detection, diminishing returns, biggest jumps |
| `delta` | Per-iteration efficiency, cumulative improvement |
| `status` | Keep/discard rate, crash frequency, success streaks |
| `severity` | Severity distribution, critical discovery rate |
| `hypothesis` | Confirmation rate, investigation efficiency |
| `technique` | Technique effectiveness ranking |
| `dimension` | Coverage completeness (X/12) |

## Report Structure

```
## Evals Summary — {subcommand} ({N} iterations)

### Key Metrics
- Total iterations: N | Kept: X | Reverted: Y | Revert rate: Z%
- Starting metric: A | Final metric: B | Improvement: C%

### Trend Analysis
- Plateau detection, biggest win/loss, diminishing returns

### Patterns
- What succeeded vs failed, file hotspots, technique effectiveness

### Recommendation
- continue / stop / change strategy
```

## Output

- Console: structured report (30-50 lines)
- `--format md` → evals-summary.md
- `--format json` → evals-summary.json
