---
name: project-spec-synthesizer
version: 2.0.0
description: Synthesizes project PRD, Architecture, Data/API, UI specs, and host integration contracts cleanly into SSOT.
risk_tier: 2
---

# Project Spec Synthesizer (Host Runtime Skill)

## Purpose
Coordinates bidirectional specification synthesis across canonical project specs (PRD, Architecture, Data & API Contracts, Component Registry, Tech Stack, Host Integration Contract, Cross-Dependencies, and Feature Hierarchy) inside the host workspace.

## Minimum Floor, Never Maximum Ceiling
Cover at minimum the core product, data, UI, and host boundary specifications; expand into ADRs, migration checklists, threat models, and rollout runbooks whenever story risk tier warrants deeper rigor.

## Operating Protocol
1. **Intent & Brief Ingestion:** Extract user personas, jobs-to-be-done, acceptance criteria, and non-goals from raw briefs or clarified SDI inputs.
2. **Typed Contract & Schema Mapping:** Emit boundary-validated DTO schemas (Zod / Rust serde) and additive SQLite WAL DDL migrations mirrored cleanly to read-only Prisma models.
3. **Host Boundary & SSOT Verification:** Verify zero unmapped host dependencies, zero secret/PII leakage across boundaries, and synchronized cross-spec anchors before handoff.
