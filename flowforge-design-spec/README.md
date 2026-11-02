# Flow-First AI-Native Design Platform — Product & Engineering Spec Pack

**Status:** Discovery / Pre-MVP · **Date:** 2026-10-26 · **Working name:** FlowForge (placeholder; no naming/trademark validation).  
**Language:** Vietnamese, with English technical identifiers.  
**Audience:** Founder, Product, Design, Engineering, Codex, Antigravity.

## Product thesis
A flow-first, code-native design workspace that turns ambiguous human intent into **executable user flows**, verifiable UI states, reviewable code changes, and portable frontend source. Avoid building a generic Figma clone.

## Quick start for coding agents
1. Read `AGENTS.md`, `docs/00-DECISIONS-AND-ASSUMPTIONS.md` and `product/01-PRD.md`.
2. Read `architecture/02-SYSTEM-ARCHITECTURE.md`, `architecture/03-DATA-CONTRACTS.md`, `product/04-USER-FLOWS.md`.
3. Start at `execution/07-IMPLEMENTATION-BACKLOG.md`, Sprint 0, with an architecture decision record and skeleton. Do not implement entire backlog at once.
4. Use `agent/09-CODEX-ANTIGRAVITY-STARTER.md` as the first implementation brief.
5. Run gates described in `execution/08-QUALITY-AND-EVALUATION.md` at each slice.

## Files
| File | Purpose |
|---|---|
| `docs/00-DECISIONS-AND-ASSUMPTIONS.md` | Scope, assumptions, non-goals and open questions |
| `product/01-PRD.md` | PRD, requirements, MVP scope, acceptance criteria |
| `architecture/02-SYSTEM-ARCHITECTURE.md` | Architecture, services, stack, security |
| `architecture/03-DATA-CONTRACTS.md` | Entities, schema, APIs, event contracts |
| `product/04-USER-FLOWS.md` | Core user journeys, Mermaid diagrams, state transitions |
| `research/05-COMPETITOR-LANDSCAPE.md` | Competitive research, source-level confidence |
| `research/06-TECH-STACK-RESEARCH.md` | Stack options and decision tradeoffs |
| `execution/07-IMPLEMENTATION-BACKLOG.md` | Epics, vertical slices, dependencies and DoD |
| `execution/08-QUALITY-AND-EVALUATION.md` | Evaluation, benchmarks, metrics and safety gates |
| `agent/09-CODEX-ANTIGRAVITY-STARTER.md` | Agent prompts, project conventions, checkpoints |
| `strategy/10-POSITIONING-AND-MOAT.md` | Positioning, ICP, moat, GTM and validation |
| `schemas/*.json` | Draft structured contracts; validate before production |
| `examples/*.json` | Example project, flow and intent |
| `SOURCES.md` | URLs and verification notes |

## Readiness labels
- **Verified external fact:** cited by a primary/official document in `SOURCES.md` (as of research date).
- **Product decision / recommended:** proposed here, not user-approved.
- **Hypothesis:** must be validated with experiments, interviews or PoC.
- **Deferred:** explicitly excluded from MVP.

This is an **implementation-ready planning package**, not functioning product code. Estimates are directional. The name is temporary.
