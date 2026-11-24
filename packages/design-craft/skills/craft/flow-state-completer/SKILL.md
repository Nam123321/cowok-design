---
name: flow-state-completer
version: 2.0.0
description: Audits UI flows for >= 4 core interaction states plus domain states and synthesizes E2E coverage specs.
risk_tier: 2
---

# Flow State Completer

## Purpose
Evaluates every interactive screen, flow node, and transition graph to eliminate dead ends, missing recovery paths, and unhandled edge states before code handoff.

## Minimum Floor, Never Maximum Ceiling
Every interactive view MUST cover at least the **4 core baseline states** (`default`, `loading`, `empty`, `error`), treated strictly as a **Minimum Floor**. Proactively add domain-specific states whenever user journeys require them—such as `permission-denied`, `partial-data`, `offline-queued`, `optimistic-pending`, `conflict-resolution`, `rate-limited`, or `read-only-archived`.

## Operating Protocol
1. **State Matrix Audit (`>= 4` Core + Domain States):** Inspect each flow node for missing `loading`, `empty`, `error`, and applicable domain states; flag dead-end screens lacking an explicit recovery or navigation exit.
2. **Transition & Recovery Wiring:** Connect every error, empty, or permission state to a clear, user-actionable recovery path (retry, adjust filter, request access, or return safely) preserving unsaved user input.
3. **E2E Verification Synthesis:** Generate deterministic state-transition test scenarios verifying happy paths, boundary failures, and recovery transitions across viewports.
