---
name: flow-state-completer
version: 1.0.0
description: Enforces 4 UI sub-states and generates Playwright E2E coverage suites
capabilities_required: ["flow-graph", "test-generator"]
risk_tier: 2
---

# Flow State Completer Operating Skill

## 1. Overview
The `flow-state-completer` skill evaluates every node on the 2D DAG against the canonical 4 UI states (`Default`, `Loading`, `Empty`, `Error`). It leverages state machine algorithms to guarantee zero dead-ends and emits automated Playwright E2E test specs.

## 2. Progressive Disclosure & Operation
- **Phase 1: State Matrix Audit**: Audits every node for missing empty/error views.
- **Phase 2: Transition Wiring**: Connects retry buttons and recovery paths.
- **Phase 3: E2E Playwright Synthesis**: Emits executable `.spec.ts` test files.
