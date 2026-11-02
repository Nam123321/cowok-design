# Decisions, Assumptions & Open Questions

## Locked from user idea
- Users: vibe coders and non-technical people designing web/app UIs.
- UX: flow map and single active live screen rather than giant static screen-and-wire canvas.
- Design created in HTML-based runnable UI and exportable to project code.
- Feedback: comments, free-form requests, images, links and reference attachments.
- Agents: MCP compatibility, BYO model/provider integration and addable skills/workflows/GitHub repositories.
- Intent engine translates lay terminology into actionable design instructions and may ask clarifying questions with visuals.

## Recommended MVP decisions (NOT yet approved)
- Working name: FlowForge.
- Target: B2B SaaS dashboards and transactional web apps (not general-purpose art design).
- Single web stack: React 19-compatible + TypeScript + Vite + Tailwind; versions to be pinned in PoC.
- One project template, one preview runtime strategy selected after two-day engineering spike.
- Project model: `ScreenState` nodes linked by `Transition` edges; serialized to versioned JSON.
- Comments attached to screen/node and optionally to a stable element anchor; screenshot evidence when selector stale.
- External MCP **server** read-only first; mutation tools introduced behind approval/RBAC.
- API-key BYO initially; subscription-based login only through officially supported, permitted integrations.
- GitHub repo export/new-repo connection before arbitrary codebase import.

## Non-goals (MVP)
- Complete Figma-style canvas, vector tooling, real-time collaborative source editing.
- Arbitrary Next.js/Vue/Angular import and two-way transformations.
- Native mobile code generation; backend app builder; autonomous production deployment.
- Skills marketplace or unrestricted third-party code execution.
- Pixel-perfect production equivalence guarantee; subscription compatibility with every LLM provider.

## Design tradeoffs to verify
- WebContainers vs remote container for preview (browser limitations/licensing vs execution isolation/operational cost).
- AST-only source mapping vs source-instrumentation+AST hybrid.
- Finite state machine library vs plain transitions schema for flows.
- Offline/desktop-first vs browser SaaS (recommend browser SaaS for MVP).

## Open decisions / product-owner approval queue
1. Working product name and trademark/domain checks.
2. Web-based app first or integrated desktop IDE first.
3. Primary user: product owner vs solo vibe coder vs agency collaborator.
4. Cloud data residency, retention and enterprise security requirements.
5. First model providers and billing model; allowed OAuth/subscription integrations.
6. Business model: per-seat, credits, per-project or hybrid; should be tested.
7. Commercial license viability for runtime and imported libraries.
8. What degree of import fidelity is acceptable for beta.

## Confidence ledger
- High confidence: runnable React previews, graph UI, code export, comments, browser validation, basic MCP; existing tech.
- Medium confidence: stable element-to-code mapping in constrained stack, state fixtures, non-tech intent clarification.
- Low/unknown until PoC: arbitrary repo import, robust two-way visual editing, faithful production parity, strong moat from proprietary behavior graph.
