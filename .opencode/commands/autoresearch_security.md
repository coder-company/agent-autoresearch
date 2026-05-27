---
name: autoresearch_security
description: "STRIDE + OWASP security audit with red-team adversarial personas"
argument-hint: "[Scope: <glob>] [Focus: <area>] [Iterations: N] [--diff] [--fix] [--fail-on <severity>] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Scope:` or `--scope` — file globs to audit
- `Focus:` — specific area (auth, API, data handling, etc.)
- `Depth:` or `--depth` — quick (5), standard (15), deep (30+)
- `Iterations:` or `--iterations` — default 15. "unlimited" for unbounded.
- `--diff` — delta mode: only audit files changed since last audit
- `--fix` — after audit, auto-fix Critical/High findings
- `--fail-on <severity>` — CI gate: exit non-zero if findings at/above threshold
- `--evals`, `--evals-interval N`, `--chain`

## Setup (if Scope missing)

If Scope missing and no --diff:
1. Scan codebase for tech stack, frameworks, API routes
2. Ask user:
   Q1 (Scope): "What to audit?" — entire codebase, API + middleware, auth, external-facing
   Q2 (Depth): "How thorough?" — quick (5), standard (15), deep (30+), unlimited
   Q3 (Action): "What to do with findings?" — report only, report + auto-fix, report + CI gate

## Setup Phase (once, before loop)

1. **Reconnaissance** — scan deps, .env, Dockerfile, API routes, auth, DB schemas, CI configs
2. **Asset Identification** — catalog data stores, auth systems, external services, user inputs
3. **Trust Boundary Mapping** — browser↔server, public↔authenticated, user↔admin, CI↔prod
4. **STRIDE Threat Model** — generate threats per category. Load `references/security-checklist.md`.
5. **Attack Surface Map** — entry points, data flows, abuse paths

Create output directory. Write overview.md, threat-model.md, attack-surface-map.md.
TSV header: `iteration\ttimestamp\tfinding\tseverity\towasp\tstride\tevidence\tfile_line`

## Iteration Loop

### Phase 1: Review
- Check coverage tracking, identify untested attack vectors

### Phase 2: Attack
- Adopt red-team persona for this vector
- Deep-dive with adversarial mindset

### Phase 3: Validate
- Construct proof: file:line + specific attack scenario
- Every finding MUST have code evidence
- Classify severity, map to OWASP (A01-A10) and STRIDE (S/T/R/I/D/E)

### Phase 4: Log
Append finding to TSV. Update coverage tracking.

### Composite Metric
`score = (owasp_tested/10)*50 + (stride_tested/6)*30 + min(findings, 20)`

## After Loop

1. Write `findings.md`, `owasp-coverage.md`, `recommendations.md`
2. If `--fix` → chain to fix
3. If `--fail-on` → check threshold, exit non-zero if exceeded

## Summary

Print: total findings by severity, OWASP coverage X/10, STRIDE coverage Y/6, composite score.

## Chain Handoff

Write handoff.json. Invoke next target in --chain order.
