# Reason — `autoresearch:reason`

Adversarial debate between AI personas until convergence. Two authors generate competing solutions, a critic attacks them, a synthesizer merges the best, and a blind judge panel picks the winner. Repeats until one answer wins consistently.

## When to Use

- Design decisions with multiple valid approaches
- Architecture proposals that need stress-testing
- Product strategy questions without a clear metric
- Any question where "it depends" isn't good enough — force a winner

## Syntax

```
/autoresearch:reason
Task: Should we use microservices or a monolith for our new payment service?
Domain: software
```

## Real Examples

### Architecture Decision

```
/autoresearch:reason
Task: Evaluate event sourcing vs CRUD for the order management system
Domain: software
--judges 5
```

### Product Strategy

```
/autoresearch:reason
Task: Should we build a CLI-first or GUI-first developer tool?
Domain: product
--mode debate
```

### Security Design

```
/autoresearch:reason
Task: Compare JWT vs session-based auth for our multi-tenant API
Domain: security
--convergence 4
```

## Modes

| Mode | Behavior |
|------|----------|
| `convergent` (default) | Stops when incumbent wins N consecutive rounds |
| `creative` | Never auto-stops — keeps generating alternatives |
| `debate` | No synthesis step — pure A vs B debate |

## How It Works

Each round:
1. **Generate-A** — First author produces a candidate (or incumbent continues)
2. **Critic** — Adversarial critic finds ≥3 weaknesses in candidate-A
3. **Generate-B** — Second author produces a candidate addressing the critique
4. **Synthesize** — Merges best of A + B into a hybrid (skipped in debate mode)
5. **Blind Judge Panel** — Judges see candidates with randomized labels, vote independently
6. **Convergence Check** — If winner repeats N times → converged

### Convergence

Default: incumbent must win 3 consecutive rounds. Adjustable with `--convergence N`.

### Oscillation Guard

If the incumbent changes 5+ times in 8 rounds, the debate isn't converging. The agent recommends early stop.

## Output

```
autoresearch/reason-250527-1430/
├── reason-results.tsv    # Per-round judge verdicts
├── lineage.md            # Full history: candidates, critiques, judge reasoning
└── summary.md            # Final winner + convergence trajectory
```

## Flags

| Flag | Purpose |
|------|---------|
| `--mode <type>` | convergent, creative, debate |
| `--judges N` | Blind judge count (3 default, 5 thorough, 7 deep) |
| `--convergence N` | Consecutive wins needed to stop (default 3) |
| `--no-synthesis` | Skip synthesis, pure debate |
| `--domain <type>` | software, product, business, security, research, content |
| `--evals` | Periodic checkpoint reports |
| `Iterations: N` | Max rounds (default 8) |

## Tips

- 3 judges is fast; 5 is more reliable; 7 is thorough but slower
- The blind judge panel prevents bias — judges see randomized labels, not "candidate A"
- Creative mode is useful for brainstorming where you want maximum diversity
- Convergence in 3-4 rounds usually means the answer is clear
- If oscillation is detected, the question may need reframing (more specific, different domain)
- Chain `reason → probe` to refine the winning approach into detailed requirements
