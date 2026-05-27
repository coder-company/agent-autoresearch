# Adversarial Architecture Decisions

## Context

Architecture decisions (microservices vs monolith, database selection, caching strategy) involve tradeoffs that only manifest at scale or under failure. This scenario uses the adversarial dimension to stress-test architecture choices by exploring what breaks each option. Useful before committing to irreversible infrastructure decisions.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: Evaluate microservices decomposition of a monolith handling 100K RPM with shared PostgreSQL
Domain: infrastructure
Scope: src/** deploy/** docs/architecture/**
Depth: deep
Focus: failure
--adversarial
```

## Generated Scenarios (sample output)

### Dimension 7: Failure — Microservices Path
| # | Scenario | Severity |
|---|----------|----------|
| 1 | Service A calls B calls C — C is down, retry storm from A×B amplifies 3x, cascading failure across mesh | Critical |
| 2 | Distributed transaction across 3 services — compensating transaction for rollback fails on service 2 — inconsistent state | Critical |
| 3 | Service discovery cache stale for 30s — requests routed to decommissioned instance, silent data loss | High |

### Dimension 7: Failure — Monolith Path
| # | Scenario | Severity |
|---|----------|----------|
| 4 | Single memory leak in billing module causes OOM — entire application crashes, all features unavailable | Critical |
| 5 | Long-running report query locks shared DB connection pool — checkout handlers timeout for all users | High |

### Dimension 6: Scale — Database Choice
| # | Scenario | Severity |
|---|----------|----------|
| 6 | PostgreSQL single-writer bottleneck at 50K writes/sec — read replicas help reads but writes are the bottleneck | High |
| 7 | Switch to DynamoDB for writes — now joins across services require application-level scatter-gather, latency 10x | High |
| 8 | Shared PostgreSQL across microservices — schema migration requires coordinated deploy of all services | High |

### Dimension 9: Integration — Caching Strategy
| # | Scenario | Severity |
|---|----------|----------|
| 9 | Redis cache cluster split-brain during network partition — two app instances read different cached values | High |
| 10 | Cache invalidation event lost — stale price displayed for 2 hours, orders placed at wrong price | Critical |
| 11 | Cache warming after restart takes 20 minutes — cold-start thundering herd hits database directly | High |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 12 | Microservice database-per-service: cross-service consistency recovery requires manual reconciliation | High |
| 13 | Monolith rollback after schema migration: backward-incompatible column removed, rollback impossible without data restore | Critical |

## Key Dimensions Explored

- **Failure** — Cascade patterns differ: microservices amplify network failures, monoliths have blast-radius-of-everything
- **Scale** — Database write bottlenecks, join elimination tradeoffs
- **Integration** — Cache consistency across service boundaries, split-brain scenarios
- **Recovery** — Cross-service transaction rollback vs monolith schema rollback
- **State** — Distributed state consistency vs single-database ACID guarantees
