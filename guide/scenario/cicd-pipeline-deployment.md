# CI/CD Pipeline Deployment

## Context

CI/CD pipelines orchestrate build, test, and deploy stages across environments. Failures during deployment require rollback decisions, canary analysis interprets metrics under noise, and dependency conflicts between services create cascading failures. Pipeline state must be recoverable after interruption.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: Deploy a microservice update through canary analysis with automatic rollback on regression
Domain: infrastructure
Scope: .github/workflows/** deploy/** src/canary/**
Depth: standard
Focus: failure
```

## Generated Scenarios (sample output)

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 1 | Canary passes (latency within threshold) but error rate spikes 5 minutes after promotion to 100% | Critical |
| 2 | Deploy step succeeds but health check endpoint returns 200 from old cached response — stale verification | High |
| 3 | Rollback triggered but previous image was garbage-collected from registry — cannot restore | Critical |
| 4 | Database migration ran forward during deploy; rollback reverts code but schema is now incompatible | Critical |

### Dimension 5: State
| # | Scenario | Severity |
|---|----------|----------|
| 5 | Pipeline killed mid-deploy — 3/10 instances on new version, 7/10 on old — no automated recovery | High |
| 6 | Canary is healthy but dependent service deployed simultaneously and broke — blame attribution wrong | High |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 7 | Rollback of service A triggers cascading redeploys of services B, C that depend on A's new API | High |
| 8 | Pipeline retry after timeout re-runs migrations that are not idempotent — duplicate data | Critical |

### Dimension 9: Integration
| # | Scenario | Severity |
|---|----------|----------|
| 9 | Dependent service pinned to exact version of this service — deploy breaks downstream even though canary passed with old traffic | High |
| 10 | Secret rotation happens between build and deploy — deployed artifact has expired credentials | High |

### Dimension 4: Concurrency
| # | Scenario | Severity |
|---|----------|----------|
| 11 | Two PRs merged within 1 minute — both trigger deploys, second overwrites first before canary completes | High |
| 12 | Rollback and new deploy triggered simultaneously — race to which version ends up running | Critical |

## Key Dimensions Explored

- **Failure** — Canary false positives, stale health checks, missing rollback images
- **State** — Partial deploys, interrupted pipelines, blame attribution
- **Recovery** — Cascading rollbacks, non-idempotent migration retry
- **Integration** — Cross-service version pinning, secret rotation timing
- **Concurrency** — Concurrent deploys, rollback races
