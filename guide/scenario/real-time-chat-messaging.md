# Real-Time Chat Messaging

## Context

Real-time messaging requires delivery guarantees (at-least-once, exactly-once), offline sync with conflict resolution, rate limiting to prevent abuse, read receipts across devices, and message ordering in group conversations. Network partitions and device state transitions create complex edge cases.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: User sends a message in a group chat while some members are offline and others have multiple devices
Domain: mobile app
Scope: src/messaging/** src/sync/** src/delivery/**
Depth: deep
Focus: concurrency
```

## Generated Scenarios (sample output)

### Dimension 4: Concurrency
| # | Scenario | Severity |
|---|----------|----------|
| 1 | Two users send messages at same millisecond — different ordering on each client's local view | High |
| 2 | User edits message on phone while deleting it on tablet — edit wins or delete wins? | High |
| 3 | Read receipt sync: message marked read on one device but unread badge persists on another for hours | Medium |

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 4 | WebSocket drops mid-send — client retries, server already processed first attempt — duplicate message | High |
| 5 | Push notification delivered but message fetch fails — user sees notification but empty conversation | Medium |
| 6 | Media upload completes but message metadata write fails — orphaned blob, no message | High |

### Dimension 6: Scale
| # | Scenario | Severity |
|---|----------|----------|
| 7 | Group with 500 members: single message triggers 500 delivery receipts + 500 push notifications simultaneously | High |
| 8 | User rejoins after 30 days offline — sync attempts to deliver 50K messages at once | Medium |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 9 | Device restored from backup has message IDs that conflict with messages sent from replacement device | High |
| 10 | Server-side message deletion propagated to 499/500 group members — one device retains deleted message forever | Medium |

### Dimension 8: Security
| # | Scenario | Severity |
|---|----------|----------|
| 11 | Rate-limited user sends messages via API directly, bypassing client-side throttle | High |
| 12 | Message contains 10MB of zero-width Unicode characters — renders as empty but consumes storage quota | Medium |

## Key Dimensions Explored

- **Concurrency** — Message ordering, multi-device sync, edit/delete conflicts
- **Failure** — Network partition recovery, duplicate detection, partial sends
- **Scale** — Large group fanout, offline sync backfill, notification storms
- **Recovery** — Device restore conflicts, propagation consistency
- **Security** — Rate limit bypass, storage abuse via invisible content
