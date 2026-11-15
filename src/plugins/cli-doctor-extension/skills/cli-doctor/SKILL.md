---
name: cli-doctor
version: 1.0.0
description: Passive 16+ coding CLI/IDE diagnostics, bounded active auth probe, and SSRF-guarded BYOK verification.
risk_tier: 2
---

# CLI Doctor Extension Skill

## Purpose
Provides honest separation between Day-0 **Passive CLI/IDE Scan** (`GET /api/doctor`, `GET /api/v1/doctor/clis`) and explicit per-CLI **Active Auth Probe** (`POST /api/doctor/probe-cli` bounded by `concurrency <= 4` and `-PGID SIGKILL <= 1500ms`).

## Contracts
- **Provides**: `GET /api/doctor`, `POST /api/doctor/probe-cli`, `POST /api/doctor/verify-key`, `PUT /api/settings/vault`, `GET /api/plugins`
- **Consumes**: `GET /api/v1/health`, `GET /api/v1/projects`, `GET /api/v1/agent-settings`, `PUT /api/v1/agent-settings`
