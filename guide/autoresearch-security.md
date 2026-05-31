# Security — `autoresearch:security`

STRIDE threat modeling + OWASP Top 10 audit with red-team adversarial personas. Systematic coverage tracking ensures no category is missed.

## When to Use

- Before shipping to production
- Periodic security audit of a codebase
- After adding auth, payments, or external integrations
- CI gate for security regressions (`--fail-on`)
- Delta audit after changes (`--diff`)

## Syntax

```
/autoresearch:security
Scope: src/**/*.ts
Focus: auth
Iterations: 20
```

## Real Examples

### Full Audit

```
/autoresearch:security
Scope: src/**/*.{ts,js}
Iterations: 30
```

### Auth-Focused Audit

```
/autoresearch:security
Scope: src/auth/**/*.ts src/middleware/**/*.ts
Focus: authentication and authorization
--depth deep
```

### CI Gate

```
/autoresearch:security
Scope: src/**/*.ts
--diff
--fail-on high
```

Exits non-zero if any High or Critical findings — use in CI pipelines.

### Audit + Auto-Fix

```
/autoresearch:security
Scope: src/**/*.ts
--fix
```

After audit, Critical/High findings are chained to `fix` automatically.

## STRIDE Coverage

The agent tracks coverage across all 6 STRIDE categories:

| Category | Threat Type |
|----------|-------------|
| **S**poofing | Identity impersonation, weak auth |
| **T**ampering | Data modification, missing integrity checks |
| **R**epudiation | Missing audit logs, unsigned transactions |
| **I**nfo Disclosure | Data leaks, verbose errors, exposed env vars |
| **D**enial of Service | Unbounded queries, missing rate limits |
| **E**levation of Privilege | Missing authz, IDOR, privilege escalation |

## OWASP Top 10

Coverage tracking for all 10 categories (A01–A10). The agent rotates through untested categories, prioritizing gaps.

## Composite Metric

```
score = (owasp_tested/10) × 50 + (stride_tested/6) × 30 + min(findings, 20)
```

Progress printed every 5 iterations:
```
OWASP: [A01✓ A02✓ A03✗ A04✗ A05✓ A06✗ A07✓ A08✗ A09✗ A10✗] 4/10
STRIDE: [S✓ T✓ R✗ I✓ D✗ E✗] 3/6
Score: 48.3 | Findings: 7
```

## Output

```
autoresearch-results/security/security-250527-1430/
├── overview.md              # Product + stack context
├── threat-model.md          # STRIDE threats per component
├── attack-surface-map.md    # Entry points and data flows
├── findings.md              # All findings, severity-ranked
├── owasp-coverage.md        # Category-by-category results
├── recommendations.md       # Prioritized fixes
└── security-results.tsv     # Raw iteration data
```

## Flags

| Flag | Purpose |
|------|---------|
| `--diff` | Delta mode: only audit changed files |
| `--fix` | Auto-fix Critical/High findings after audit |
| `--fail-on <severity>` | CI gate: exit non-zero above threshold |
| `--depth <level>` | quick (5), standard (15), deep (30+) |
| `--evals` | Periodic progress checkpoints |

## Tips

- Every finding requires file:line evidence + attack scenario — no theoretical fluff
- The agent rotates through 4 red-team personas (Security Adversary, Supply Chain, Insider, Infra)
- Use `--diff` in CI to avoid re-auditing unchanged code
- `--fail-on medium` is strict; `--fail-on critical` is lenient — pick based on risk tolerance
- Chain `security → fix → ship` for a full audit-to-deploy pipeline
