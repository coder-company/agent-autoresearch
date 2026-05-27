---
name: autoresearch_ship
description: "Ship anything through 8 phases: checklist, dry-run, deploy, verify"
argument-hint: "[Target: <what>] [--type <type>] [--dry-run] [--auto] [--force] [--rollback] [--checklist-only] [--monitor N]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Target:` or `--target` — what to ship (path, PR, artifact, deployment)
- `--type <type>` — override: code-pr, code-release, deployment, content, docs, package, config
- `--dry-run` — validate everything but don't ship
- `--auto` — auto-approve if no errors found
- `--force` — skip non-critical items (blockers still enforced)
- `--rollback` — undo last ship action
- `--monitor N` — post-ship monitoring for N minutes
- `--checklist-only` — only generate checklist, don't execute
- `--chain`

## Setup (if Target or Type unclear)

1. Auto-detect ship type from context
2. If unclear → ask user:
   Q1 (What): "What are you shipping?"
   Q2 (Target): "Specific target?"
   Q3 (Mode): "How to ship?" — full workflow, dry-run only, checklist only

## 8 Phases

### Phase 1: Identify
Determine ship type, target artifacts, domain-specific checklist.

### Phase 2: Inventory
Gather: files changed, deps affected, config changes, migrations, breaking changes.

### Phase 3: Checklist
Generate domain-specific checklist. If `--checklist-only` → stop here.

### Phase 4: Prepare
Run tests, type checker, linter, secrets check. Flag blockers vs warnings.

### Phase 5: Dry-Run
Simulate ship action. If `--dry-run` → stop here.

### Phase 6: Ship
**REQUIRES EXPLICIT USER APPROVAL** (unless --auto with zero errors).

### Phase 7: Verify
Post-ship: confirm live, smoke tests, monitoring check.

### Phase 8: Log
Write checklist.md, summary.md, ship-log.tsv.

## Rollback

If `--rollback`: identify last ship action, reverse it, verify.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
