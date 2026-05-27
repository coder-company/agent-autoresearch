# IoT Firmware Updates

## Context

IoT firmware updates are pushed to thousands of constrained devices over unreliable networks. Partial updates can brick devices, connectivity loss mid-update requires resumable downloads, and version rollback must work without network access. Fleet-wide rollouts need staged deployment with automatic halt on anomaly detection.

## Scenario Configuration

```
/autoresearch:scenario
Scenario: Push firmware update to fleet of 10K IoT sensors with staged rollout and automatic rollback
Domain: infrastructure
Scope: src/firmware/** src/fleet/** src/ota/**
Depth: standard
Focus: failure
```

## Generated Scenarios (sample output)

### Dimension 7: Failure
| # | Scenario | Severity |
|---|----------|----------|
| 1 | Power loss during flash write — device boots with corrupted firmware, bootloader intact but app partition invalid | Critical |
| 2 | Download completes but checksum validation uses wrong algorithm version — valid firmware rejected | High |
| 3 | Certificate pinned in old firmware expired — device cannot authenticate to download server for update | Critical |

### Dimension 12: Recovery
| # | Scenario | Severity |
|---|----------|----------|
| 4 | Rollback triggered but device has no previous firmware in backup partition — bricked | Critical |
| 5 | Partial download resumes after 48-hour network outage — server-side file rotated, offset no longer valid | High |
| 6 | Device rolls back successfully but configuration written by new firmware is incompatible with old version | High |

### Dimension 5: State
| # | Scenario | Severity |
|---|----------|----------|
| 7 | Device reports "update complete" but actual version check shows old firmware — false positive in fleet dashboard | High |
| 8 | Fleet rollout paused at 15% — devices in mixed firmware state, protocol incompatibility between old and new | High |

### Dimension 6: Scale
| # | Scenario | Severity |
|---|----------|----------|
| 9 | All 10K devices check for update simultaneously at midnight — CDN thundering herd, most get timeout | Medium |
| 10 | Staged rollout: 1% canary passes but 100% rollout reveals thermal issue only in high-density deployments | High |

### Dimension 2: Validation
| # | Scenario | Severity |
|---|----------|----------|
| 11 | Firmware image valid for hardware rev 2.0 pushed to rev 1.x devices — boots but GPIO pins mapped wrong | Critical |
| 12 | Firmware size exceeds flash partition by 12 bytes — download succeeds but flash write fails silently | High |

## Key Dimensions Explored

- **Failure** — Power loss, certificate expiry, download corruption
- **Recovery** — Rollback without backup, resume after long outage, config incompatibility
- **State** — Mixed fleet versions, false completion reports
- **Scale** — Thundering herd, density-dependent failures
- **Validation** — Hardware revision mismatch, partition size constraints
