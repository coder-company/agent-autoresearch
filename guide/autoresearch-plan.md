# Plan — `autoresearch:plan`

Interactive wizard that converts a plain-language goal into a validated autoresearch configuration. Use this when you know *what* you want but not *how* to measure it.

## When to Use

- You have a goal but don't know the right verify command
- You want to validate that your metric is mechanically extractable
- You're new to autoresearch and want guided setup
- You want to derive scope from your project structure automatically

## Syntax

```
/autoresearch:plan
Goal: <plain-language description>
```

Or just describe what you want:

```
/autoresearch:plan I want to eliminate all TypeScript any types
```

## What It Does

1. **Analyze** — Parses your goal for measurability
2. **Scope** — Scans project structure, proposes file globs
3. **Metric + Direction** — Identifies what to measure, which direction is better
4. **Verify Command** — Constructs a shell command to extract the metric
5. **Safety Screen** — Checks the command for dangerous operations
6. **Dry Run** — Actually runs the verify command to confirm it outputs a number
7. **Guard** — Proposes a safety command (tests, type checker, build)
8. **Present** — Shows a ready-to-run config block

## Example Session

Input:
```
/autoresearch:plan
Goal: Get rid of all console.log statements in production code
```

Output:
```
/autoresearch
Goal: Remove console.log from production code
Scope: src/**/*.{ts,tsx}
Metric: console.log count
Direction: lower_is_better
Verify: grep -rc 'console\.log' src/ | awk -F: '{s+=$2}END{print s}'
Guard: npm test
Iterations: 15
```

The plan command then asks: "Run this config now, or adjust?"

## Chain Support

Plan chains naturally to the core loop:

```
/autoresearch:plan
Goal: Improve test coverage
--chain autoresearch
```

The derived config flows directly into execution.

## Tips

- If plan can't derive a verify command, it suggests `/autoresearch:reason` for subjective goals
- Plan always dry-runs the verify command before presenting — if it fails, it adjusts
- For complex metrics (JSON output, multi-step extraction), plan handles the piping
- Plan suggests iteration counts based on goal complexity: simple (10-15), moderate (20-25), complex (30+)
