# Working with Codex / Antigravity

## One-time setup
Upload or unzip this spec pack as the root of a new development repository. Keep `AGENTS.md` at repo root; point either coding agent to it. The documentation is a design baseline, not an instruction to skip clarifying implementation unknowns.

## First prompt — Architecture and Sprint 0 (copy/paste)
> You are the tech lead implementing the attached Flow-First AI-Native Design Platform spec. Read AGENTS.md, product/01-PRD.md, architecture/02-SYSTEM-ARCHITECTURE.md, architecture/03-DATA-CONTRACTS.md, product/04-USER-FLOWS.md and execution/07-IMPLEMENTATION-BACKLOG.md. Do not implement everything at once. First produce ADR-001 (sandbox runtime), ADR-002 (schema), ADR-003 (element mapping), and a Sprint-0 plan with measurable engineering spikes, risks and acceptance criteria. Propose the minimal TypeScript monorepo structure, exact pinned dependencies after checking their current docs, plus tests. Then implement only the monorepo skeleton, typed domain contracts and the seed order-approval flow with unit tests. Do not access production credentials or connect external accounts without explicit approval. Report changed files, commands and results.

## Second prompt — First end-to-end slice
> Implement Slice 1 from execution/07-IMPLEMENTATION-BACKLOG.md: click a user-flow state node and render the correct running React preview state. Use the project's constrained React/Vite template. Persist versioned graph state and fixtures; surface unresolved/stale mapping errors. Add automated contract + Playwright tests. Keep all untrusted code isolated. Show screenshots or test evidence and document limitations. Do not implement arbitrary repository import, unrestricted shell tools or visual drag-to-code editing.

## Third prompt — Intent and proposed change
> Implement Slice 3 with a deterministic approval-modal benchmark: selected Approve button -> requirement -> approved SDI -> patch proposal -> build/tests -> user approval -> updated graph+code. Provide exact patch diff and test results. No direct writes to main. Instrument first-pass acceptance, failure reason and model/tool usage.

## Collaboration operating rules
- One ticket per PR. Include spec references, acceptance evidence, failure paths and rollback steps.
- If changing graph/SDI schema, bump version and migration; add fixtures and backward-compat tests.
- Separate design intent (what user asked) from engineering choices (how) and model guesses.
- No secret keys in prompts/commits; require provider secrets via secure server storage.
- For external MCP integration, read-only first; all write tools require scope and confirmation.
- Use `TODO(ADR)` rather than fabricating framework/library behavior.

## Suggested repository commands (illustrative; adjust once scaffold exists)
```bash
pnpm install
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm exec playwright test
```

## Successful first demo
Starting from a blank project, the app shows the Order Approval flow, supports click on `submitting` -> loading fixture, attaches a comment to Approve, creates a modal confirmation request and a reviewable patch, runs a test, then exports an independently runnable project.
