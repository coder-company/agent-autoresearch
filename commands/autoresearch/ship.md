---
name: autoresearch:ship
description: "Ship through 8 phases: checklist, test, lint, PR, deploy, verify"
argument-hint: "[Target: <what>] [--dry-run] [--auto] [--rollback]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- `Target:` — what to ship (path, PR, artifact)
- `--type` — code-pr, release, deployment, package
- `--dry-run` — validate without shipping
- `--auto` — auto-approve if checklist passes
- `--rollback` — undo last ship

## Setup (if Target unclear)
Auto-detect from context (uncommitted changes, version bump, deploy config).
AskUserQuestion:
  Q1: "What to ship?" — this PR, a release, a deployment

## Protocol
8 phases (set `/goal "PR merged and deploy healthy"` if not --dry-run):
1. **Checklist**: tests pass, lint clean, types pass, no secrets
2. **Test**: run full test suite
3. **Lint**: run linter
4. **Build**: run build
5. **Commit**: clean commit message
6. **PR**: create/update PR
7. **Deploy**: trigger deploy (if approved pre-launch)
8. **Verify**: health check post-deploy
