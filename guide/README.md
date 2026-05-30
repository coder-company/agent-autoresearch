# Autoresearch Guide

Practical documentation for every command and pattern.

## Getting Started

- [Getting Started](getting-started.md) — Install, first run, what to expect
- [Codex](autoresearch-codex.md) — `$autoresearch` syntax, background runtime, monitoring

## Command Guides

| Guide | Command | Purpose |
|-------|---------|---------|
| [Core Loop](autoresearch.md) | `autoresearch` | Iterate against any metric |
| [Plan](autoresearch-plan.md) | `autoresearch:plan` | Interactive wizard → config |
| [Debug](autoresearch-debug.md) | `autoresearch:debug` | Hunt bugs with hypotheses |
| [Fix](autoresearch-fix.md) | `autoresearch:fix` | Crush errors to zero |
| [Security](autoresearch-security.md) | `autoresearch:security` | STRIDE + OWASP audit |
| [Ship](autoresearch-ship.md) | `autoresearch:ship` | 8-phase ship workflow |
| [Scenario](autoresearch-scenario.md) | `autoresearch:scenario` | Edge case exploration |
| [Predict](autoresearch-predict.md) | `autoresearch:predict` | Multi-persona debate |
| [Learn](autoresearch-learn.md) | `autoresearch:learn` | Auto-generate docs |
| [Reason](autoresearch-reason.md) | `autoresearch:reason` | Adversarial refinement |
| [Probe](autoresearch-probe.md) | `autoresearch:probe` | Requirement interrogation |
| [Improve](autoresearch-improve.md) | `autoresearch:improve` | Product improvement engine |
| [Evals](autoresearch-evals.md) | `autoresearch:evals` | Analyze iteration results |

## Patterns & Reference

- [Chains & Combinations](chains-and-combinations.md) — How to pipe commands together
- [Examples by Domain](examples-by-domain.md) — Real configs for TypeScript, Python, bundlers, APIs
- [Hooks](hooks.md) — Hook system reference (9 hooks, enable/disable)
- [Advanced Patterns](advanced-patterns.md) — Parallel experiments, multi-repo, CI/CD

## Conventions

All guides use the same structure:
1. What the command does (one paragraph)
2. When to use it vs alternatives
3. Real examples with actual commands
4. Output you'll see
5. Common patterns and flags
