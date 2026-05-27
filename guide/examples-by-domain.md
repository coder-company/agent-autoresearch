# Examples by Domain

Real-world configurations for common optimization targets.

---

## TypeScript: Test Coverage

```
/autoresearch
Goal: Increase test coverage from 72% to 90%
Scope: src/**/*.ts, src/**/*.test.ts
Verify: npx jest --coverage --silent 2>&1 | grep "All files" | awk '{print $4}'
Guard: npx tsc --noEmit
Iterations: 50
```

---

## TypeScript: Eliminate `any` Types

```
/autoresearch
Goal: Eliminate all any types
Scope: src/**/*.ts
Verify: grep -r ":any\b\|as any\b" src/ --include="*.ts" | wc -l
Direction: lower
Guard: npx tsc --noEmit
Iterations: 40
```

---

## Python: Type Error Reduction

```
/autoresearch
Goal: Reduce mypy errors to zero
Scope: src/**/*.py
Verify: mypy src/ --no-error-summary 2>&1 | grep "error:" | wc -l
Direction: lower
Guard: pytest --tb=short -q
Iterations: 30
```

---

## Python: Test Coverage

```
/autoresearch
Goal: Push coverage above 85%
Scope: src/**/*.py, tests/**/*.py
Verify: pytest --cov=src --cov-report=term 2>&1 | grep TOTAL | awk '{print $4}' | tr -d '%'
Guard: pytest --tb=short -q
```

---

## JavaScript: Bundle Size

```
/autoresearch
Goal: Reduce bundle size below 200KB
Scope: src/**/*.js, src/**/*.ts, webpack.config.js
Verify: npm run build 2>&1 && stat -f%z dist/bundle.js 2>/dev/null || stat -c%s dist/bundle.js
Direction: lower
Guard: npm test
Iterations: 25
```

---

## API: Response Latency

```
/autoresearch
Goal: Reduce p95 API response time below 100ms
Scope: src/api/**/*.ts, src/middleware/**/*.ts
Verify: npm run bench:api 2>&1 | grep "p95" | awk '{print $2}'
Direction: lower
Guard: npm test
Iterations: 20
```

---

## Rust: Compilation Time

```
/autoresearch
Goal: Reduce clean build time below 30 seconds
Scope: src/**/*.rs, Cargo.toml
Verify: cargo clean -q && /usr/bin/time -f "%e" cargo build --release 2>&1 | tail -1
Direction: lower
Guard: cargo test --no-fail-fast
Iterations: 15
```

---

## Security: Vulnerability Count

```
/autoresearch:security
Scope: src/**/*.ts
Iterations: 10
--fix
```

Or as a metric-driven loop:

```
/autoresearch
Goal: Zero critical/high security findings
Scope: src/**/*.ts
Verify: npm audit --json 2>/dev/null | jq '.metadata.vulnerabilities.critical + .metadata.vulnerabilities.high'
Direction: lower
Guard: npm test
```

---

## Lint: Warning Count

```
/autoresearch
Goal: Zero ESLint warnings
Scope: src/**/*.ts
Verify: npx eslint src/ --format compact 2>&1 | grep -c "Warning" || echo 0
Direction: lower
Guard: npx tsc --noEmit
Iterations: 30
```

---

## Documentation: Coverage

```
/autoresearch:learn --mode init
Scope: src/**/*.ts
```

Or measured:

```
/autoresearch
Goal: 100% JSDoc coverage on exported functions
Scope: src/**/*.ts
Verify: npx typedoc --emit none 2>&1 | grep -c "missing documentation" || echo 0
Direction: lower
```

---

## Content: Reading Level

```
/autoresearch
Goal: Reduce reading level to grade 8
Scope: docs/**/*.md
Verify: cat docs/**/*.md | textstat --metric flesch_kincaid_grade 2>/dev/null || echo 12
Direction: lower
Iterations: 15
```

---

## DevOps: Docker Image Size

```
/autoresearch
Goal: Reduce Docker image below 100MB
Scope: Dockerfile, .dockerignore
Verify: docker build -q -t app:test . && docker image inspect app:test --format '{{.Size}}' | awk '{print $1/1048576}'
Direction: lower
Guard: docker run --rm app:test npm test
Iterations: 10
```

---

## Tips

- **Always set a Guard** to prevent regressions while optimizing
- **Direction matters**: coverage/speed → `higher`, errors/size → `lower`
- **Scope tightly**: the narrower the scope, the faster each iteration
- **Use integers when possible**: integer metrics avoid floating-point ambiguity
- **Combine with debug**: if the metric isn't moving, switch to `/autoresearch:debug`
