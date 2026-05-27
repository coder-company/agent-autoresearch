---
name: autoresearch:security
description: "STRIDE + OWASP security audit with red-team personas"
argument-hint: "[Scope: <glob>] [Iterations: N] [--diff] [--fix] [--fail-on <severity>]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments
- `Scope:` or `--scope` — file globs to audit
- `Iterations:` or `--iterations` — default 15
- `--diff` — only audit changed files
- `--fix` — auto-fix Critical/High findings
- `--fail-on <severity>` — exit non-zero for CI gating

## Setup (if Scope missing)
Scan codebase for tech stack.
AskUserQuestion:
  Q1: "What to audit?" — entire codebase, API, auth, external-facing
  Q2: "How thorough?" — quick (5), standard (15), deep (30+)
  Q3: "Action on findings?" — report only, report + fix

## Protocol
1. Read all in-scope files
2. Audit through 6 STRIDE categories + OWASP Top 10
3. For each finding: severity, location, evidence, remediation
4. If --fix: set `/goal "all critical/high findings resolved"`, fix one per turn
5. Output structured findings report
