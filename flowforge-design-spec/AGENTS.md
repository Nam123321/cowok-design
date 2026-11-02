# AGENTS.md — Instructions for Codex / Antigravity / coding agents

## Mission
Develop a **flow-first, code-native design workspace** for non-technical product owners and vibe coders. The system converts intent into a structured design spec; produces React UI within a constrained template; represents screen + state flows; displays live running preview; attaches review comments; exports usable source.

## Priority documents
1. `product/01-PRD.md` (what)
2. `architecture/02-SYSTEM-ARCHITECTURE.md` and `architecture/03-DATA-CONTRACTS.md` (how)
3. `product/04-USER-FLOWS.md` (behavior)
4. `execution/07-IMPLEMENTATION-BACKLOG.md` (order)
5. `execution/08-QUALITY-AND-EVALUATION.md` (proof)

## Non-negotiables
- Do not claim HTML preview equals production under all environments.
- Source code is truth for **implemented UI**; graph and intent contracts are truth for **approved expected behavior**. Surface drift rather than silently reconciling.
- MVP supports **one supported React + TypeScript + Vite + Tailwind template**. Do not promise arbitrary repo imports, Next.js, Vue, mobile native or full visual editing.
- AI changes are proposed as **diffs**, build and test in isolated sandbox, and require human approval to apply in main branch.
- Every flow node targets a `route + state fixture + viewport`, not merely a screenshot.
- Every write API enforces project authorization; prevent path traversal, SSRF, prompt injection via attachments/repositories; redact secrets in logs.
- No unrestricted shell/network from generated code. Separate trusted app server from untrusted preview containers.
- If provider integration is uncertain, stub an interface and record an ADR; do not fabricate SDK capabilities.
- Prefer deterministic tools for file edits, AST transforms, build, route navigation and browser tests; use LLM for ambiguity resolution and structural proposals.

## Work protocol
For each ticket: read context → state assumptions → write a small plan → add failing test where appropriate → implement minimum vertical slice → run lint/typecheck/tests/build → report files changed, behavior, limitations, and follow-up. Keep patches small.

## Design principles
Flow > disconnected screens · runnable preview > static mockup · requirement provenance > prompt history · reversible change > hidden mutation · evaluation > vibe.
