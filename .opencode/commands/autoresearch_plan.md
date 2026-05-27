---
name: autoresearch_plan
description: "Convert a goal into validated Scope, Metric, Direction, Verify config"
argument-hint: "[Goal: <text>] [--chain <targets>]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Goal:` — text after keyword, or full $ARGUMENTS if no keyword
- `--chain <targets>` — comma-separated downstream commands

## Setup (if Goal missing)

Ask user:
  Q1 (Goal): "What do you want to achieve?"
  Q2 (Type): "What kind of goal?" — improve a metric, fix errors, audit security, explore edge cases, document code, ship something

## Phase 1: Analyze Goal

Parse the goal to determine:
- Is it measurable? (metric-driven vs subjective)
- What's the natural scope? (files, modules, entire codebase)
- What subcommand fits best?

## Phase 2: Derive Scope

1. Scan project structure
2. Identify files relevant to the goal
3. Propose file globs

## Phase 3: Derive Metric + Direction

For metric-driven goals:
- Identify what to measure
- Determine direction: higher_is_better or lower_is_better

For subjective goals:
- Suggest proxy metrics or recommend /autoresearch_reason

## Phase 4: Derive Verify Command

1. Propose Verify command
2. **Safety screen:** check for dangerous patterns
3. Dry-run → confirm it outputs a valid number

## Phase 5: Derive Guard (optional)

Propose Guard if applicable: test suite, type check, build.

## Phase 6: Suggest Iterations

Based on goal complexity: 10-15 (simple), 20-25 (moderate), 30+ (complex).

## Phase 7: Present Config

Output ready-to-run config block:
```
/autoresearch
Goal: {derived goal}
Scope: {derived globs}
Metric: {derived metric}
Direction: {direction}
Verify: {derived command}
Guard: {derived guard or omit}
Iterations: {suggested count}
```

Ask user: "Run this config now, or adjust?"

## Chain Handoff

Write handoff.json with derived config. Invoke next target.
