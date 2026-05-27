# Scenario — `autoresearch:scenario`

Generate edge cases across 12 dimensions from a seed scenario. Explore what could go wrong before it does.

## When to Use

- Before implementing a feature — explore edge cases upfront
- After implementation — verify coverage of failure modes
- For test generation — produce scenarios to codify as tests
- Risk assessment — systematic exploration of what could break

## Syntax

```
/autoresearch:scenario
Scenario: User uploads a profile image
Domain: web
Scope: src/upload/**/*.ts
```

## Real Examples

### API Endpoint Exploration

```
/autoresearch:scenario
Scenario: POST /api/payments creates a new payment intent
Domain: API
Scope: src/api/payments/**/*.ts
--depth deep
```

### CLI Tool Edge Cases

```
/autoresearch:scenario
Scenario: User runs autoresearch with a verify command that takes 5 minutes
Domain: CLI
--focus concurrency
```

### Data Pipeline Scenarios

```
/autoresearch:scenario
Scenario: ETL pipeline processes 10M rows from S3 to Postgres
Domain: data pipeline
Scope: src/etl/**/*.py
--format gherkin
```

## 12 Dimensions

Every scenario is explored through these lenses:

| # | Dimension | Explores |
|---|-----------|----------|
| 1 | Happy path | Normal successful flows |
| 2 | Validation | Input boundaries, types, formats |
| 3 | Permissions | Auth, roles, access control |
| 4 | Concurrency | Race conditions, deadlocks, ordering |
| 5 | State | Invalid transitions, corruption |
| 6 | Scale | High volume, large data, many users |
| 7 | Failure | Network errors, timeouts, partial failures |
| 8 | Security | Injection, abuse, bypass attempts |
| 9 | Integration | Third-party failures, API contract violations |
| 10 | Data | Null, empty, unicode, injection, overflow |
| 11 | UX | Confusion, misuse, accessibility |
| 12 | Recovery | Retry, rollback, idempotency |

Use `--focus <dimension>` to prioritize a specific area.

## Saturation Detection

The agent tracks coverage per dimension. If 3 consecutive iterations produce only duplicates for a dimension, it's marked saturated. When all 12 dimensions saturate, the loop ends early.

## Output Formats

- `--format markdown` (default) — Structured scenarios with titles and descriptions
- `--format json` — Machine-readable for test generation tooling
- `--format gherkin` — Given/When/Then format for BDD frameworks

## Output

```
autoresearch/scenario-250527-1430/
├── scenarios.md         # Organized by dimension, severity-ranked
├── edge-cases.md        # Flat severity-ranked list
└── scenario-results.tsv # Raw iteration data
```

## Flags

| Flag | Purpose |
|------|---------|
| `--domain <type>` | web, mobile, API, CLI, data pipeline, infrastructure |
| `--focus <dimension>` | Prioritize a specific dimension |
| `--depth <level>` | shallow (10), standard (20), deep (40+) |
| `--format <type>` | markdown, json, gherkin |
| `--evals` | Periodic progress reports |

## Tips

- Scenario is exploration, not optimization — there's no single metric
- Pair with `debug` or `fix`: generate scenarios, then verify they're handled
- Use `--format gherkin` to feed directly into test frameworks
- The `--focus` flag is useful when you already know your weak area (e.g., `--focus concurrency`)
- Chain `probe → scenario` to first identify requirements, then explore their edge cases
