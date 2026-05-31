# Predict — `autoresearch:predict`

Multi-persona code analysis. 5 expert personas independently review your code, then debate their findings. Surfaces issues no single perspective would catch.

## When to Use

- Before a major refactoring — get multi-angle analysis
- Architecture review — personas evaluate different concerns simultaneously
- Pre-launch audit — comprehensive code quality assessment
- CI gate — fail on findings above a severity threshold

## Syntax

```
/autoresearch:predict
Scope: src/**/*.ts
Goal: code quality and security
```

## Real Examples

### Full Code Review

```
/autoresearch:predict
Scope: src/**/*.ts
Goal: overall quality
--depth standard
```

5 personas (Architect, Security Analyst, Performance Engineer, Reliability Engineer, Devil's Advocate) independently review, then debate.

### Adversarial Review

```
/autoresearch:predict
Scope: src/auth/**/*.ts
Goal: find vulnerabilities
--adversarial
```

Hostile personas: Breaker, Cheater, Scaler, Newbie, Malicious Insider.

### Deep Architecture Review

```
/autoresearch:predict
Scope: src/**/*.ts
Goal: architecture and scalability
--depth deep
--personas 8
--rounds 3
```

8 personas, 3 rounds of debate — thorough but slower.

### CI Gate

```
/autoresearch:predict
Scope: src/**/*.ts
--fail-on high
--incremental
```

Exits non-zero if any High+ findings. `--incremental` only analyzes changed files.

## How It Works

1. **Reconnaissance** — Scan scope, build knowledge (imports, API surface, data flow)
2. **Persona Generation** — Load 5 expert personas with domain-specific criteria
3. **Independent Analysis** — Each persona reviews code through their lens, isolated
4. **Debate** — Personas challenge each other's findings with evidence
5. **Consensus** — Synthesizer deduplicates, resolves conflicts, ranks findings
6. **Report** — Output ranked findings with severity × confidence
7. **CI Gate** — Exit non-zero if `--fail-on` threshold exceeded

## Default Personas

| Persona | Focus |
|---------|-------|
| Architect | Structure, coupling, abstraction boundaries |
| Security Analyst | Vulnerabilities, auth, data protection |
| Performance Engineer | Hot paths, memory, latency, scaling |
| Reliability Engineer | Error handling, retries, observability |
| Devil's Advocate | Assumptions, over-engineering, simpler alternatives |

## Anti-Herd Check

If all personas agree on everything, the synthesizer MUST find at least one counter-argument. This prevents groupthink.

## Output

```
autoresearch-results/predict/predict-250527-1430/
├── summary.md    # Top findings, consensus view, risk assessment
└── debate.md     # Full persona analysis + debate transcript
```

## Flags

| Flag | Purpose |
|------|---------|
| `--depth <level>` | shallow (3 personas, 1 round), standard (5, 2), deep (8, 3) |
| `--adversarial` | Use hostile reviewer personas |
| `--personas N` | Override persona count (3-8) |
| `--rounds N` | Override debate rounds (1-3) |
| `--budget N` | Max findings across all personas (default 40) |
| `--fail-on <severity>` | CI gate: exit non-zero above threshold |
| `--incremental` | Only analyze changed files |
| `--chain <targets>` | Chain to debug, fix, security, etc. |

## Tips

- Standard depth (5 personas, 2 rounds) is the sweet spot for most codebases
- Use `--adversarial` specifically for security-sensitive code
- Findings include file:line references — they're actionable, not vague
- Chain `predict → debug → fix` for a full analysis-to-fix pipeline
- `--budget 20` keeps output focused; `--budget 60` is exhaustive
