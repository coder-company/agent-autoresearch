# Probe — `autoresearch:probe`

8 personas systematically interrogate a requirement until constraints saturate. Produces a complete constraint registry and a ready-to-run autoresearch config.

## When to Use

- Before building a feature — discover requirements you'd miss
- Ambiguous specs — force precision through interrogation
- Before writing an RFC/PRD — expose hidden assumptions
- After a design decision — verify it holds up under scrutiny

## Syntax

```
/autoresearch:probe
Topic: Real-time collaborative editing for our document system
Scope: src/editor/**/*.ts
```

## Real Examples

### Feature Requirements

```
/autoresearch:probe
Topic: Add multi-tenant support to the billing API
Scope: src/billing/**/*.ts
--depth standard
```

### Autonomous Mode (No User Input)

```
/autoresearch:probe
Topic: Migrate from REST to GraphQL
Scope: src/api/**/*.ts
--mode autonomous
```

In autonomous mode, the agent infers answers from the codebase instead of asking you. Labels confidence (high/medium/low).

### Adversarial Exploration

```
/autoresearch:probe
Topic: Rate limiting strategy for public API
--adversarial
--depth deep
```

Rotates hostile personas (Skeptic, Contradiction Finder, Edge-Case Hunter) to the front.

## The 8 Personas

| # | Persona | Focus |
|---|---------|-------|
| 1 | Domain Expert | Business rules, domain constraints, terminology |
| 2 | End User | Usability, expectations, error recovery |
| 3 | Skeptic | Assumptions that might be wrong |
| 4 | Edge-Case Hunter | Boundary conditions, rare scenarios |
| 5 | Ops Engineer | Deployment, monitoring, scaling, failure modes |
| 6 | Security Reviewer | Attack vectors, data protection, auth |
| 7 | Contradiction Finder | Conflicts between requirements |
| 8 | Scope Guardian | Feature creep, unnecessary complexity |

Each round activates 2-3 personas who generate probing questions from their perspective.

## Modes

| Mode | How Questions Are Answered |
|------|---------------------------|
| `interactive` (default) | Presents questions to you via AskUserQuestion |
| `autonomous` | Infers from codebase, labels confidence |

## Saturation Detection

Each round extracts atomic constraints from answers. When fewer than N net-new constraints appear for 3 consecutive rounds, the topic is saturated. Default threshold: 2.

## Output

```
autoresearch-results/probe/probe-250527-1430/
├── constraints.md        # Full registry organized by category
├── conflicts.md          # Unresolved contradictions
└── summary.md            # Includes ready-to-run autoresearch config
```

The summary includes a derived autoresearch config block — Goal, Scope, Metric, Verify — ready to copy-paste into the core loop.

## Flags

| Flag | Purpose |
|------|---------|
| `--mode <type>` | interactive (you answer) or autonomous (agent infers) |
| `--depth <level>` | shallow (5 rounds), standard (15), deep (30) |
| `--personas N` | Active persona count per round (3-8, default 6) |
| `--adversarial` | Hostile personas first |
| `--saturation-threshold N` | Net-new constraint threshold (default 2) |
| `--evals` | Periodic progress reports |

## Tips

- Interactive mode produces higher-quality constraints but requires your time
- Autonomous mode is faster but marks low-confidence answers — review those
- The constraint registry is the deliverable — it's what you take into implementation
- Use `--adversarial` when you suspect the happy path is well understood but edge cases aren't
- Chain `probe → scenario` to interrogate requirements, then explore their edge cases
- Chain `probe → improve` to feed discovered constraints into product improvement research
- The derived autoresearch config in summary.md saves you from manually configuring the next run
