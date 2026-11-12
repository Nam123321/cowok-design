---
name: project-spec-synthesizer
version: 1.0.0
description: Synthesizes PRD, Data Specs, UI Specs, and Host Integration Contracts
capabilities_required: ["spec-registry", "schema-builder"]
risk_tier: 2
---

# Project Spec Synthesizer Operating Skill

## 1. Overview
The `project-spec-synthesizer` skill coordinates specification generation across the 9 recognized spec documents: PRD, Architecture, Data Model, API Registry, Component Registry, Tech Stack, Host Contract, Cross Dependencies, and Feature Hierarchy.

## 2. Progressive Disclosure & Operation
- **Phase 1: PRD Ingestion**: Extracts core user goals and acceptance criteria.
- **Phase 2: Contract Mapping**: Generates exact typed schemas and database tables.
- **Phase 3: Host Verification**: Ensures zero unmapped host dependencies.
