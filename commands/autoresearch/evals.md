---
name: autoresearch:evals
description: "Analyze iteration results: trends, plateaus, regressions"
argument-hint: "[path/to/results.tsv] [--format text|json|md]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- Positional: path to TSV file
- `--format` — text (default), json, md

## Protocol (single turn, no /goal)
1. Find results TSV (argument, or scan autoresearch-results/)
2. Parse header (metric_direction) + data rows
3. Analyze:
   - Total iterations, keeps, discards, crashes
   - Trend: improving, flat, declining
   - Plateau detection: longest streak without improvement
   - Best single iteration (largest positive delta)
   - Worst regression (if any keeps were later undone)
   - Efficiency: keeps / total iterations
4. Recommendations:
   - If plateau: suggest pivot strategy
   - If high crash rate: suggest verify command fix
   - If efficient: suggest continuing current approach
5. Output formatted report
