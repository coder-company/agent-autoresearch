# Multi-Tenant SaaS Onboarding

## Context

SaaS onboarding provisions isolated tenant environments, migrates data from legacy systems, configures billing plans, and sets up initial user roles. Tenant isolation failures leak data between organizations. Billing edge cases cause revenue loss or customer disputes.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: New enterprise tenant signs up with data migration from existing system and custom billing plan
Domain: API
Scope: src/tenants/** src/billing/** src/migration/** src/provisioning/**
Depth: standard
Focus: permissions
```

## Generated Scenarios (sample output)

### Dimension 3: Permissions
| # | Scenario | Severity |
|---|----------|----------|
| 1 | Tenant admin invites user with email belonging to different tenant — cross-tenant data exposure | Critical |
| 2 | API key generated during onboarding has global scope instead of tenant-scoped — can read other tenants' data | Critical |
| 3 | Migrated user retains admin role from source system that doesn't map to destination role hierarchy | High |

### Dimension 10: Data
| # | Scenario | Severity |
|---|----------|----------|
| 4 | Migration imports 500K records — tenant hits row limit mid-import, partial data with broken foreign keys | High |
| 5 | Source system uses UTF-16 field names with emoji — migration mapping fails silently, data mapped to wrong columns | High |
| 6 | Duplicate email across source accounts — which user record wins during dedup? | Medium |

### Dimension 6: Scale
| # | Scenario | Severity |
|---|----------|----------|
| 7 | 50 tenants provision simultaneously during product launch — shared provisioning queue starves smaller tenants | High |
| 8 | Enterprise tenant with 10K users triggers welcome emails — rate limit blocks all other tenant notifications | Medium |

### Dimension 9: Integration
| # | Scenario | Severity |
|---|----------|----------|
| 9 | Stripe webhook for subscription creation arrives before tenant provisioning completes — billing active but no environment | High |
| 10 | SSO/SAML configuration fails validation but tenant already partially provisioned — stuck in limbo state | Medium |

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 11 | Migration halfway complete when source system revokes API access — tenant has partial data, cannot retry | High |
| 12 | DNS propagation for custom domain takes 48 hours — tenant can't access for 2 days post-signup | Medium |

## Key Dimensions Explored

- **Permissions** — Tenant isolation boundaries, cross-tenant data leakage vectors
- **Data** — Migration edge cases: encoding, dedup, partial imports
- **Scale** — Multi-tenant resource contention during provisioning spikes
- **Integration** — Billing/SSO webhook ordering and partial failure states
- **Failure** — Partial provisioning recovery and rollback
