---
name: research-decision-room
version: 1.0.0
description: Synthesizes design decisions with empirical citations and zero-fabrication rules
capabilities_required: ["spec-registry", "adr-writer"]
risk_tier: 1
---

# Research Decision Room Operating Skill

## 1. Overview
The `research-decision-room` skill moderates design trade-offs between UX, Engineering, and Business agents. It strictly enforces anti-fabrication guidelines: all claims, benchmark figures, and library recommendations must be backed by verifiable sources or empirical proofs.

## 2. Progressive Disclosure & Operation
- **Phase 1: Socratic Debate**: Structures agent discussions into Devil's Advocate and Defender roles.
- **Phase 2: Citation Verification**: Discards ungrounded or hallucinated metrics.
- **Phase 3: ADR Publication**: Records verdict into an immutable Architectural Decision Record (`ADR-xxx.md`).
