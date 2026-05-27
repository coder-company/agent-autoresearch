---
name: autoresearch:plan
description: "Convert a goal into validated Scope, Metric, Direction, Verify config"
argument-hint: "[Goal: <text>]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- `Goal:` — what to achieve (or full $ARGUMENTS as goal)

## Setup (if Goal missing)
AskUserQuestion:
  Q1: "What do you want to achieve?"
  Q2: "What kind?" — metric improvement, fix errors, audit, explore, document

## Protocol (single turn, no /goal)
1. Analyze goal: measurable? natural scope? best subcommand?
2. Scan repo: identify relevant files, existing tooling, test commands
3. Propose config:
   - Scope (file globs)
   - Metric (what to measure)
   - Direction (higher/lower)
   - Verify (shell command)
   - Guard (safety command)
   - Suggested iterations
4. Dry-run verify command → confirm it produces a number
5. Present config for user approval
6. Output ready-to-paste command block
