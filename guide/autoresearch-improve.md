# Improve — `autoresearch:improve`

Research-driven product improvement engine. Identifies what to build next by researching ICP challenges, competitor gaps, market trends, UX patterns, and growth opportunities. Produces tiered feature rankings and PRDs.

## When to Use

- Product planning — what should we build next?
- Feature prioritization — which improvements matter most to our ICP?
- Competitive research — where are gaps we can exploit?
- PRD generation — go from idea to structured spec

## Syntax

```
/autoresearch:improve
Goal: Improve developer onboarding experience
ICP: Backend engineer, 3-7 years experience, familiar with TypeScript, new to our tool
Scope: src/**/*.ts docs/**/*.md
```

## Real Examples

### Product Improvement Research

```
/autoresearch:improve
Goal: Make the CLI more productive for daily use
ICP: Full-stack developer shipping features daily, uses terminal heavily
Scope: src/cli/**/*.ts
--depth standard
```

### Quick Discovery

```
/autoresearch:improve
Goal: Reduce churn in the first 7 days
ICP: Solo developer, just signed up, evaluating alternatives
--depth shallow
--no-discover
```

Shallow + no-discover = codebase-only analysis, fastest mode.

### Deep Competitive Research

```
/autoresearch:improve
Goal: Win against competitor X in the enterprise segment
ICP: Engineering manager, 50+ person team, cares about governance and reporting
--depth deep
--seeds 8
```

## Research Categories

| # | Category | Research Focus |
|---|----------|---------------|
| 1 | ICP Challenges | Pain points, unmet needs, workflow friction |
| 2 | Competitor Gaps | What competitors do poorly or miss |
| 3 | Market Trends | Emerging patterns, shifting expectations |
| 4 | UX Patterns | Interaction improvements, onboarding, retention |
| 5 | Revenue/Growth | Monetization, expansion paths, viral loops |

Shallow depth uses categories 1-3 only.

## ICP Binary Gate

Every idea passes through a strict yes/no filter:

> "Does this improvement directly serve the ICP's core workflow or pain point?"

- **YES** → proceeds to ranking
- **NO** → discarded immediately

No clever ideas that don't serve the ICP. This keeps output focused.

## Tiered Ranking

ICP-validated ideas are classified:

| Tier | Signal |
|------|--------|
| **Must-have** | ICP would switch products for this |
| **Nice-to-have** | ICP would appreciate but not demand |
| **Moonshot** | Could be transformative if it works |

Score = impact × confidence × ICP alignment / effort (each 1-10).

## Output

```
autoresearch-results/improve/250527-cli-productivity/
├── report.md              # Full research by category
├── ranked.md              # Flat ranked list of ICP-validated ideas
├── {idea-slug}.md         # PRD per selected improvement
├── improve-results.tsv    # Raw iteration data
└── evals-summary.md       # If --evals used
```

## PRD Generation

After research, you select which improvements get PRDs. Each PRD includes:
- Problem statement tied to ICP
- Proposed solution
- Success metrics (mechanical, measurable)
- Technical approach from codebase context
- Ready-to-run autoresearch config (when the improvement maps to a metric)

## Flags

| Flag | Purpose |
|------|---------|
| `--depth <level>` | shallow (3 categories, 10 iterations), standard (5, 20), deep (5, 40+) |
| `--seeds N` | Ideas per category to start (default 5) |
| `--discover` | Enable web research (default ON) |
| `--no-discover` | Codebase + ICP only, no external research |
| `--evals` | Periodic progress reports |
| `Iterations: N` | Cap (default 20) |

## Chain Support

Improve is a **terminal emitter** — it produces PRDs as final output. It consumes upstream handoffs:

- `probe → improve` — Use discovered constraints to seed research
- `predict → improve` — Use persona findings to identify improvement areas

## Tips

- The ICP gate is strict by design — most ideas fail it. That's the point.
- `--no-discover` is faster but misses market context. Use for internal tools.
- Start with `--depth shallow` to get a quick sense before committing to deep research.
- The PRDs include autoresearch configs — run them directly to start implementing.
- If no ICP is provided, the agent asks — don't skip this. Unfocused research produces noise.
