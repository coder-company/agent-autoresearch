# Autoresearch Examples

Copy one block into your agent prompt after installing Autoresearch. Adjust the
scope and commands to match your project.

## TypeScript: Remove `any`

```text
/autoresearch
Goal: Remove all explicit any usage
Scope: src/**/*.ts src/**/*.tsx
Metric: explicit any count
Direction: lower
Verify: rg -n ": any| as any|<any>" src 2>/dev/null | wc -l
Guard: npm test && npm run typecheck
Iterations: 30
```

## Python: Raise Coverage

```text
/autoresearch
Goal: Raise test coverage to 90%
Scope: src/**/*.py tests/**/*.py
Metric: coverage percent
Direction: higher
Verify: pytest --cov=src --cov-report=term | awk '/TOTAL/ {gsub("%", "", $4); print $4}'
Guard: pytest
Iterations: 25
```

## Rust: Reduce Clippy Warnings

```text
/autoresearch
Goal: Reduce clippy warnings to zero
Scope: src/**/*.rs tests/**/*.rs
Metric: clippy warning count
Direction: lower
Verify: cargo clippy --message-format short 2>&1 | tee /tmp/autoresearch-clippy.txt >/dev/null; rg -c "warning:" /tmp/autoresearch-clippy.txt || true
Guard: cargo test
Iterations: 20
```

## Web App: Shrink Bundle

```text
/autoresearch
Goal: Reduce production JavaScript bundle size
Scope: src/**/* package.json vite.config.* webpack.config.*
Metric: bundle bytes
Direction: lower
Verify: npm run build -- --json > /tmp/autoresearch-stats.json && node -e "const s=require('/tmp/autoresearch-stats.json'); console.log(s.assets.filter(a => a.name.endsWith('.js')).reduce((n, a) => n + a.size, 0))"
Guard: npm test
Iterations: 20
```

## API: Lower Latency

```text
/autoresearch
Goal: Lower p95 latency for the health endpoint
Scope: src/**/* routes/**/* handlers/**/*
Metric: p95 latency milliseconds
Direction: lower
Verify: hey -z 30s -c 10 http://localhost:3000/health | awk '/95%/ {print $2 * 1000}'
Guard: npm test
Iterations: 15
```

## Parallel Experiments

Use this when several hypotheses are plausible and the run has enough CPU, RAM,
and disk for isolated worker worktrees:

```bash
autoresearch parallel prepare --workers 3
autoresearch parallel run --manifest autoresearch-results/parallel-manifest.json --timeout-seconds 1200
# Fill in each worker metric, guard status, commit, and description.
autoresearch parallel closeout --batch-file autoresearch-results/parallel-workers.json
autoresearch parallel cleanup --manifest autoresearch-results/parallel-manifest.json
```

More domain-specific examples are in
[Examples by Domain](../guide/examples-by-domain.md).
