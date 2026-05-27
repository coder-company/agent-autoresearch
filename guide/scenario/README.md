# Scenario Guide — Domain-Specific Examples

Real-world examples of `/autoresearch:scenario` applied to specific domains. Each shows the configuration used and sample output across multiple dimensions.

## Examples

| File | Domain | Key Dimensions |
|------|--------|---------------|
| [cross-border-wire-transfers.md](cross-border-wire-transfers.md) | Financial | Concurrency, Validation, Failure, Recovery |
| [healthcare-appointment-scheduling.md](healthcare-appointment-scheduling.md) | Healthcare | Concurrency, State, Permissions, UX |
| [multi-tenant-saas-onboarding.md](multi-tenant-saas-onboarding.md) | SaaS | Permissions, Data, Scale, Integration |
| [real-time-chat-messaging.md](real-time-chat-messaging.md) | Messaging | Concurrency, Failure, Scale, Recovery |
| [cicd-pipeline-deployment.md](cicd-pipeline-deployment.md) | DevOps | Failure, State, Recovery, Integration |
| [iot-firmware-updates.md](iot-firmware-updates.md) | IoT | Failure, Recovery, State, Scale |
| [mobile-push-notifications.md](mobile-push-notifications.md) | Mobile | Scale, Failure, Validation, Integration |
| [search-autocomplete.md](search-autocomplete.md) | Search | Scale, Data, UX, Concurrency |
| [social-media-content-moderation.md](social-media-content-moderation.md) | Social | Security, Data, UX, Permissions |
| [document-collaboration.md](document-collaboration.md) | Collaboration | Concurrency, State, Recovery, Data |
| [adversarial-architecture-decisions.md](adversarial-architecture-decisions.md) | Architecture | Scale, Failure, Integration, Recovery |

## How to Use

1. Pick the domain closest to your feature
2. Copy the scenario configuration block
3. Adjust the seed scenario to match your specific feature
4. Run `/autoresearch:scenario` with the config

## Dimension Reference

The 12 dimensions explored across scenarios:

1. **Happy path** — Normal successful flows
2. **Validation** — Input boundaries, types, formats
3. **Permissions** — Auth, roles, access control
4. **Concurrency** — Race conditions, deadlocks, ordering
5. **State** — Invalid transitions, corruption
6. **Scale** — High volume, large data, many users
7. **Failure** — Network errors, timeouts, partial failures
8. **Security** — Injection, abuse, bypass attempts
9. **Integration** — Third-party failures, API contract violations
10. **Data** — Null, empty, unicode, injection, overflow
11. **UX** — Confusion, misuse, accessibility
12. **Recovery** — Retry, rollback, idempotency
