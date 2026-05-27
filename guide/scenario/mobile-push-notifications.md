# Mobile Push Notifications

## Context

Push notification delivery involves token management (registration, rotation, expiry), platform-specific APIs (APNs, FCM), batching for throughput, quiet hours enforcement, and rate limiting per user. Token expiry and device state transitions cause silent delivery failures that are hard to detect.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: Send targeted push notification campaign to 1M users across iOS and Android with quiet hours
Domain: API
Scope: src/notifications/** src/tokens/** src/campaigns/**
Depth: standard
Focus: scale
```

## Generated Scenarios (sample output)

### Dimension 6: Scale
| # | Scenario | Severity |
|---|----------|----------|
| 1 | 1M notifications queued — FCM batch API returns partial success (800K delivered, 200K unknown) — retry all 200K or skip? | High |
| 2 | Campaign triggers during timezone boundary — 500K users cross from quiet hours to active simultaneously — delivery spike | Medium |
| 3 | User has 5 devices registered — single notification multiplied to 5M device deliveries for 1M user campaign | High |

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 4 | APNs returns "BadDeviceToken" for 30% of tokens — bulk invalidation cascades into token refresh storm | High |
| 5 | FCM quota exceeded mid-batch — first 400K delivered, remaining 600K delayed 2+ hours | Medium |
| 6 | Notification delivered but app killed by OS — user sees notification, taps it, app cold-starts without deep link context | Medium |

### Dimension 2: Validation
| # | Scenario | Severity |
|---|----------|----------|
| 7 | Notification payload exceeds APNs 4KB limit after variable substitution — user's name is 200 characters | Medium |
| 8 | Quiet hours configured as "10 PM - 7 AM" but user relocated to different timezone — old timezone still enforced | High |

### Dimension 9: Integration
| # | Scenario | Severity |
|---|----------|----------|
| 9 | APNs certificate expires at 3 AM — all iOS notifications fail silently until cert rotated | Critical |
| 10 | FCM legacy API deprecated mid-campaign — 200K notifications in flight using old endpoint | High |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 11 | Campaign cancelled mid-send — 600K already delivered, 400K pending — users get inconsistent experience | Medium |
| 12 | Failed notifications retried after 1 hour — content now stale ("Your order arrives in 5 minutes" sent 65 minutes later) | High |

## Key Dimensions Explored

- **Scale** — Million-user fanout, multi-device multiplication, batch partial failures
- **Failure** — Token invalidation cascades, quota exhaustion, silent delivery failures
- **Validation** — Payload size limits, timezone-aware quiet hours
- **Integration** — Certificate expiry, API deprecation during active campaigns
- **Recovery** — Campaign cancellation, stale content on retry
