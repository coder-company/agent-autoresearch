# Cross-Border Wire Transfers

## Context

International wire transfers involve multi-currency conversion, timezone-aware cutoff windows, regulatory compliance checks (AML/KYC), and multi-hop correspondent banking. A single transfer touches 3-5 systems with eventual consistency guarantees and strict regulatory reporting deadlines.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: User initiates a cross-border wire transfer from USD to EUR via correspondent bank
Domain: API
Scope: src/transfers/** src/compliance/** src/fx/**
Depth: deep
Focus: failure
```

## Generated Scenarios (sample output)

### Dimension 4: Concurrency
| # | Scenario | Severity |
|---|----------|----------|
| 1 | Two operators approve the same transfer simultaneously — double-debit from source account | Critical |
| 2 | FX rate expires mid-approval; transfer locked at stale rate while new rate published | High |
| 3 | Compliance hold released at same instant as manual cancel — transfer enters both RELEASED and CANCELLED state | Critical |

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 4 | Correspondent bank SWIFT acknowledgement times out after debit committed — funds in limbo | Critical |
| 5 | AML screening service returns 503 after transfer already debited — no compliance decision | High |
| 6 | FX provider returns rate but connection drops before confirmation — rate accepted or not? | High |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 7 | Failed transfer retried — idempotency key expired, duplicate debit created | Critical |
| 8 | Partial rollback: debit reversed but correspondent already forwarded funds | High |

### Dimension 2: Validation
| # | Scenario | Severity |
|---|----------|----------|
| 9 | Beneficiary IBAN passes checksum but references a closed account at receiving bank | Medium |
| 10 | Transfer amount in source currency rounds to zero in destination currency (0.004 USD → 0 JPY) | High |

### Dimension 5: State
| # | Scenario | Severity |
|---|----------|----------|
| 11 | Transfer stuck in PENDING_COMPLIANCE for 30 days — SLA breach, no auto-escalation | Medium |
| 12 | Manual override moves transfer from FAILED to COMPLETED without re-running compliance | Critical |

## Key Dimensions Explored

- **Concurrency** — Multi-system race conditions with financial impact
- **Failure** — Partial failures across correspondent banking hops
- **Recovery** — Idempotency and rollback in distributed financial systems
- **Validation** — Currency precision, IBAN validation, cutoff windows
- **State** — Transfer lifecycle with compliance holds and manual overrides
