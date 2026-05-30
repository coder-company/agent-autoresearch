---
name: autoresearch:improve
description: "Research-driven product improvement: ICP challenges → tiered features → PRDs"
argument-hint: "[Goal: <text>] [ICP: <persona>] [Scope: <glob>] [Iterations: N] [--depth shallow|standard|deep] [--seeds N] [--discover] [--no-discover] [--evals]"
---

EXECUTE IMMEDIATELY.

## Parse Arguments

Extract from $ARGUMENTS:
- `Goal:` — product or feature area to improve (or full $ARGUMENTS text if no keyword)
- `ICP:` or `--icp` — ideal customer profile description
- `Scope:` or `--scope` — file globs for codebase context
- `Depth:` or `--depth` — shallow (3 categories, 10 iterations), standard (5 categories, 20), deep (5 categories, 40+)
- `--seeds N` — number of seed ideas per research category (default 5)
- `--discover` — enable discovery research (competitor analysis, market trends) — default ON
- `--no-discover` — disable discovery research, use only codebase + ICP context
- `Iterations:` or `--iterations` — default 20. "unlimited" for unbounded.
- `--evals`, `--evals-interval N`, `--chain`, `--<subcommand>`

## Setup (if Goal or ICP missing)

AskUserQuestion (single batch):
  Q1 (Goal): "What product area to improve?" — open text describing product, feature, or domain
  Q2 (ICP): "Who is the ideal customer?" — persona description (role, pain points, context)
  Q3 (Scope): "Which files for product context?" — suggested globs + entire codebase
  Q4 (Depth): "How deep?" — shallow (quick scan, 10 iterations), standard (20), deep (40+), unlimited
  Q5 (Discovery): "Include external research?" — yes (competitor gaps, market trends), no (codebase + ICP only)
If all provided → skip.

## Phase 1: Resolve Product Context

1. Read codebase within scope — identify product surface, features, user flows
2. Parse existing docs (README, CHANGELOG, product specs) for feature inventory
3. Build product map: features × user touchpoints × current state
4. Identify ICP from provided description or infer from codebase signals

## Phase 2: Research Categories

Run research across 5 categories (3 for shallow depth):

| # | Category | Research Focus |
|---|---|---|
| 1 | ICP Challenges | Pain points, unmet needs, workflow friction for the target persona |
| 2 | Competitor Gaps | What competitors do poorly or miss entirely |
| 3 | Market Trends | Emerging patterns, new expectations, shifting standards |
| 4 | UX Patterns | Interaction improvements, accessibility, onboarding, retention |
| 5 | Revenue/Growth | Monetization opportunities, expansion paths, viral loops |

Shallow depth: categories 1-3 only.

## Establish Baseline

1. Create output directory: `improve/{YYMMDD}-{slug}/`
2. TSV header: `# metric_direction: higher_is_better\niteration\ttimestamp\tcategory\tidea\ticp_pass\ttier\tscore\tdescription`
3. Metric = cumulative ICP-validated improvement ideas
4. Seed initial ideas: --seeds per category (default 5)

## Set /goal

After baseline established, activate the completion condition:

```
/goal saturation reached across all research categories with ICP-validated improvements identified, or stop after {iterations} turns
```

## Iteration Loop

### Phase 1: Review
- Read results TSV, check category coverage
- Identify underexplored categories
- Assess saturation per category

### Phase 2: Research
- Pick next category (round-robin, or priority underexplored)
- If --discover enabled: use web search for competitor analysis, market data, UX benchmarks
- If --no-discover: derive ideas from codebase analysis, ICP needs, existing feature gaps
- Generate 3-5 specific improvement ideas per iteration

### Phase 3: ICP Binary Gate

Every idea passes through the ICP gate — a strict yes/no filter:

**"Does this improvement directly serve the ICP's core workflow or pain point?"**

- **YES** → proceed to ranking
- **NO** → discard immediately, log as `icp_pass: false`

No exceptions. Clever ideas that don't serve the ICP are noise.

### Phase 4: Tiered Ranking

Classify ICP-validated ideas into tiers:

| Tier | Criteria | Signal |
|---|---|---|
| Must-have | Solves a critical ICP pain point, high confidence, low-medium effort | ICP would switch products for this |
| Nice-to-have | Improves ICP workflow, moderate confidence, reasonable effort | ICP would appreciate but not demand |
| Moonshot | High potential impact, lower confidence, higher effort or risk | Could be transformative if it works |

Score each idea: impact (1-10) × confidence (1-10) × ICP alignment (1-10) / effort (1-10)

### Phase 5: Saturation Detection
- Track new ICP-validated ideas per category per iteration
- If 3 consecutive iterations produce zero new ICP-validated ideas in a category → category saturated
- If ALL active categories saturated → early stop

### Phase 6: Log
Append to TSV: iteration, timestamp, category, idea title, icp_pass (true/false), tier, score, description

### Eval Checkpoint
If --evals: check if current_iteration % interval == 0 → run checkpoint.

### Bounded Check
If bounded: current_iteration >= max_iterations → exit loop.

## User Selection

After loop completes (or early stops):

1. Present ranked improvements organized by tier:
   - **Must-have** — sorted by score descending
   - **Nice-to-have** — sorted by score descending
   - **Moonshot** — sorted by score descending
2. AskUserQuestion: "Select improvements for PRD generation" — multi-select from ranked list
3. If no selection → output report only, skip PRD generation

## PRD Generation

For each selected improvement:

1. Write a focused PRD to `improve/{date}-{slug}/{idea-slug}.md`:
   - Problem statement (tied to ICP)
   - Proposed solution
   - Success metrics (mechanical, measurable)
   - Scope and non-goals
   - Technical approach (informed by codebase context)
   - Risks and mitigations
   - Effort estimate
2. If the improvement maps to a measurable metric → include a ready-to-run autoresearch config block

## Output

- `improve/{date}-{slug}/report.md` — full research report with all ideas by category and tier
- `improve/{date}-{slug}/ranked.md` — flat ranked list of ICP-validated improvements
- `improve/{date}-{slug}/{idea-slug}.md` — PRD per selected improvement
- `improve/{date}-{slug}/improve-results.tsv` — raw iteration data

## Summary

Print: total ideas researched, ICP pass rate, tier distribution (must-have/nice-to-have/moonshot), categories covered, selected for PRD count.

## Eval Checkpoint (--evals flag)

If --evals present:
- Compute interval: floor(max_iterations / 3), min 1. Fixed 10 if unbounded.
- Print: `--- Eval Checkpoint (iterations {X}-{Y}) ---\nIdeas: {total} ({icp_pass} ICP-validated) | Categories: {x}/{n} | Saturation: {status}\n{recommendation}\n---`
- If 3+ checkpoints with no new ICP-validated ideas → recommend early stop.
- At loop end → full evals summary to evals-summary.md.

## Chain Support

This command is a **terminal emitter** — it produces PRDs and reports as final output. It consumes upstream handoffs but does not chain forward.

If invoked via --chain from an upstream command (e.g., probe → improve):
- Read handoff.json for constraints, requirements, or scope from upstream
- Use upstream findings to seed research categories
- Write handoff.json: version "0.1.0", source "improve", timestamp, status (COMPLETE|USER_INTERRUPT|BOUNDED|ERROR), findings = selected improvements with tier + score, config{goal, icp, scope}
