# PRD — Flow-first AI-native Design Workspace

**Version:** 0.1 (draft) · **Owner:** Product Founder · **Status:** proposed

## 1. Problem
Current design-to-code or prompt-to-code workflows create beautiful isolated screens quickly but leave ambiguity in business interaction, states, review cycles, and mapping approved intent to implementation. Non-technical product owners struggle to convey needs; vibe coders struggle to maintain coherent multi-screen UX and trace changes.

## 2. Promise and scope
Convert human intent and references into **structured, reviewable, runnable UI flows** with minimal code/design drift. MVP primarily supports transactional web applications on a controlled React stack. Position against *workflow disconnect*, not generic UI generation.

## 3. Users / jobs-to-be-done
- **Non-tech product owner:** “Help me explain what users should see and do, and approve changes visually without knowing code.”
- **Vibe coder / solo builder:** “Give me functioning source and flows that I can carry into my repo, and catch broken states early.”
- **Reviewer / stakeholder:** “Comment on the exact element/state and observe its resolution.”

## 4. MVP functional requirements
| ID | Priority | Requirement | Acceptance |
|---|---|---|---|
| FR-01 | P0 | Create project from text brief using constrained starter template | Project, repo files and preview generated; build passes |
| FR-02 | P0 | Flow map for screen-state nodes + transitions | Create/link/rearrange nodes; persist graph; no orphan node silent deletion |
| FR-03 | P0 | Click node to show routed preview with fixture | Render correct screen/state and viewport; errors explicit |
| FR-04 | P0 | Natural-language change request + target context | Submit text + selected node; create change proposal with diff |
| FR-05 | P0 | Intent clarification of ambiguous/risky request | Up to 3 grounded alternatives, record decision, allow skip/undo for low-risk |
| FR-06 | P0 | Comment on node or element + image/URL | Anchor, author, status, attachments stored and reviewable |
| FR-07 | P0 | Build/lint/typecheck validation after code changes | Failed build blocks merge/apply; results linked to proposal |
| FR-08 | P0 | Export full repository (zip/Git) with source, assets, flow spec | Runs via documented install/build commands outside platform |
| FR-09 | P0 | Project history and revert | Can inspect proposal and revert approved change |
| FR-10 | P1 | GitHub connect and create PR | Scoped OAuth/GitHub App; diff branch+PR only on explicit approval |
| FR-11 | P1 | BYO API key for 1–2 providers | No key leaked to client/logs; provider tests and cost telemetry |
| FR-12 | P1 | Read-only MCP server | External agent reads project, screen, flow and comments with auth |
| FR-13 | P1 | Automated flow scenario test | Playwright exercises core demo approval flow, captures screenshots |
| FR-14 | P2 | Visual element property editing | Only supported primitives; preserves semantic HTML/responsive behavior |
| FR-15 | P2 | Third-party skills and workflows | Permission model, manifest, sandbox and audit log required |

## 5. Non-functional requirements / initial SLO targets
- Preview open after node click p95 < 2 s **when sandbox already warm** (hypothesis, not guaranteed).
- Autosave graph updates p95 < 500 ms at normal network latency (target).
- Backend endpoints enforce tenant/project scoped auth and produce auditable changes.
- Exported template passes build/typecheck and smoke test reliably; target >= 95% on benchmark set after stabilization.
- All AI-generated edits are reviewable and reversible; avoid background writes to main branch.
- Accessible keyboard navigation of major app controls; aim WCAG 2.2 AA for first-party editor UI, measure with audits.
- Retain untrusted code and attachment isolation; quotas and timeouts.

## 6. UX layout
Three coordinated zones: **flow navigator** (left), **live running preview** (center), **intent/comments/inspect pane** (right). User may collapse panes. UI previews **one active state**; flow map can expand to graph-focused mode.

## 7. Key use cases and guardrails
1. New project → brief → ask only consequential clarifying questions → draft screens and graph → runnable preview.
2. Select `Approve` button → comment “add confirm step” → clarify pattern via variants → approved structured intent → agent patch → preview and test → accept/reject.
3. Click `Submitting` flow node → fixture hydrates loading state → inspect and comment.
4. Export → clone/unzip → install → run/build → share flow spec and design decisions.

## 8. Definition of MVP complete
- 5+ representative screens, >=3 non-happy-path states, >=8 transitions in seed demo.
- Requirement → proposed diff → build/test → user approval → export works end-to-end.
- 10 scripted tasks pass reliably using constrained template.
- Failure conditions are visible: missing fixture, failed mapping, build failure, auth denial.
- At least 5 non-tech user tests and 5 vibe-coder tests conducted; findings recorded before wider beta.

## 9. Product metrics
North star: **time to accepted executable flow**. Supporting: first-pass acceptance, flow coverage, accepted changes per user-hour, rework turns, export-build success, cost per accepted change, comment resolution time, and project retention.

## 10. Out of scope and risks
See `docs/00-DECISIONS-AND-ASSUMPTIONS.md`. Most severe risk: mapping real DOM back to source and preserving behavior under edits; second: delivering faster value than direct prompt-to-code; third: preview-production divergence.
