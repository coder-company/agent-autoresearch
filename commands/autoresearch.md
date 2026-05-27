---
name: autoresearch
description: "Autonomous iteration loop: modify, verify, keep/discard against any metric"
argument-hint: "[Goal: <text>] [Scope: <glob>] [Metric: <text>] [Verify: <cmd>] [Iterations: N]"
---

EXECUTE IMMEDIATELY — load the skill and follow the protocol.

## Parse Arguments

Extract from $ARGUMENTS:
- `Goal:` — what to improve
- `Scope:` or `--scope` — file globs
- `Metric:` — what to measure (natural language)
- `Direction:` — higher_is_better (default) or lower_is_better
- `Verify:` — shell command that outputs a number on its final line
- `Guard:` — optional safety command (must exit 0)
- `Iterations:` or `--iterations` — turn cap for /goal (default: 25). "unlimited" for no cap.

## Setup (if required context missing)

If Goal, Scope, or Verify missing → AskUserQuestion (single batch):
  Q1 (Goal): "What do you want to improve?"
  Q2 (Scope): "Which files?" — suggest globs from project
  Q3 (Verify): "Command that outputs the metric as a number on its last line?"
  Q4 (Guard): "Safety command that must always pass? (or skip)"
If ALL provided inline → skip setup.

## Preconditions

1. `git rev-parse --git-dir` — must be a git repo
2. `git status --porcelain` — warn if dirty (non-autoresearch files)
3. Screen verify command for dangerous patterns
4. Run verify once → extract baseline metric
5. If Guard: run guard → confirm it passes

## Establish Baseline

```bash
autoresearch verify --command "<verify>"
```

Record baseline in `autoresearch-results/results.tsv`:
```
# metric_direction: <higher|lower>
iteration	commit	metric	delta	guard	status	description
0	<sha>	<baseline>	0	-	baseline	initial state
```

## Set /goal

Compose and activate the goal:

```
/goal <metric description> <direction> from <baseline> toward <target if stated> as measured by `<verify command>`, or stop after <iterations> turns
```

If no target stated, use: "...keep improving until stuck or interrupted"

Examples:
- `/goal test coverage higher from 72% toward 90% as measured by \`npm test -- --coverage | tail -1\`, or stop after 50 turns`
- `/goal any-type count lower from 47 toward 0 as measured by \`grep -rc ':any' src/ | awk -F: '{s+=$2}END{print s}'\`, or stop after 30 turns`

## Each Turn (while /goal active)

Execute the core protocol from the skill:
1. Read results TSV tail + git log
2. Ideate ONE hypothesis
3. Modify within scope
4. Trial commit: `git add -- <files>; git commit -m "experiment: <description>"`
5. Verify: `autoresearch verify --command "<verify>"`
6. Guard (if set): run guard command
7. Decide: keep (improved + guard pass) or discard (`git revert HEAD --no-edit`)
8. Log: `autoresearch log --iteration <N> --metric <val> --delta <d> --status <status> --description "<text>"`

## Completion

When /goal clears (condition met) or turn cap reached:
- Print summary: baseline → final, keeps/discards, top improvements
- Results persist in `autoresearch-results/`
