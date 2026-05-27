# Healthcare Appointment Scheduling

## Context

Medical appointment scheduling handles overlapping bookings, provider availability across locations, cancellation policies, insurance verification windows, and regulatory requirements for patient access. Failures can delay critical care and trigger compliance violations.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: Patient books a specialist appointment with insurance pre-authorization requirement
Domain: web app
Scope: src/scheduling/** src/providers/** src/insurance/**
Depth: standard
Focus: concurrency
```

## Generated Scenarios (sample output)

### Dimension 4: Concurrency
| # | Scenario | Severity |
|---|----------|----------|
| 1 | Two patients select the same slot simultaneously — double-booked provider | Critical |
| 2 | Provider marks slot unavailable while patient is mid-checkout — orphaned booking | High |
| 3 | Insurance pre-auth response arrives after booking timeout — patient sees failure but auth approved | Medium |

### Dimension 5: State
| # | Scenario | Severity |
|---|----------|----------|
| 4 | Appointment cancelled after insurance pre-auth obtained — pre-auth wasted, cannot reuse for rebooking | Medium |
| 5 | Provider no-shows: system has no state for "provider cancelled" vs "patient cancelled" — affects no-show fee logic | High |

### Dimension 3: Permissions
| # | Scenario | Severity |
|---|----------|----------|
| 6 | Receptionist reschedules appointment for wrong patient — HIPAA exposure of schedule details | Critical |
| 7 | Patient accesses family member's appointment but insurance pre-auth tied to different subscriber | High |

### Dimension 11: UX
| # | Scenario | Severity |
|---|----------|----------|
| 8 | Patient in different timezone sees appointment time in provider's timezone — arrives 2 hours early/late | High |
| 9 | Waitlist notification fires at 2 AM — patient misses 15-minute acceptance window | Medium |

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 10 | Insurance API down during booking — should system allow provisional booking or block entirely? | High |
| 11 | SMS reminder delivery fails — patient no-shows, charged cancellation fee | Medium |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 12 | Cancelled appointment re-opened but original insurance pre-auth expired — new auth needed but slot now taken | Medium |

## Key Dimensions Explored

- **Concurrency** — Double-booking prevention across distributed slot inventory
- **State** — Complex lifecycle: requested → pre-auth → confirmed → completed/cancelled
- **Permissions** — HIPAA-compliant access control for family and delegate access
- **UX** — Timezone handling, notification timing, waitlist mechanics
- **Failure** — Graceful degradation when insurance/notification systems fail
