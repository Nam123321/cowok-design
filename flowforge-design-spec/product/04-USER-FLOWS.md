# User Flows & State Design

## Flow A — Create project from product description
```mermaid
flowchart TD
 A[Create Project] --> B[Enter brief + references]
 B --> C[Intent normalization]
 C --> D{Material ambiguity?}
 D -- Yes --> E[Show 2-3 visual alternatives]
 E --> F[User chooses or edits]
 D -- No --> G[Generate initial flow spec]
 F --> G
 G --> H[Scaffold constrained React code]
 H --> I[Build and load preview]
 I --> J{Validation passes?}
 J -- No --> K[Show failure + proposed repair]
 K --> H
 J -- Yes --> L[Review flow nodes + running screens]
```

**Acceptance:** user can create project, see initial flow, visit node, observe actual runtime; unresolved references shown explicitly.

## Flow B — Click flow node to preview specific state
```mermaid
sequenceDiagram
 participant U as User
 participant W as Workspace
 participant F as Flow Engine
 participant R as Preview Runtime
 U->>W: Click node Order/Submitting
 W->>F: Resolve node + graph/source version
 F-->>W: route, fixture, viewport, entry action
 W->>R: navigate + hydrate fixture
 R-->>W: ready + state assertions
 W-->>U: live page or explicit error
```

**State design:** `loading`, `ready`, `unresolved`, `sandbox_failed`, `fixture_invalid`, `stale_mapping`. Do not masquerade fixture screenshot as working runtime.

## Flow C — Element-linked change with clarification
```mermaid
flowchart TD
 A[Select Approve Button] --> B[Comment: Need confirmation]
 B --> C[Intent Engine]
 C --> D{High-impact / ambiguous?}
 D -- Yes --> E[Preview options: modal / inline / undo]
 E --> F[User selects modal]
 D -- No --> G[Ready SDI]
 F --> G
 G --> H[Generate diff in sandbox branch]
 H --> I[Build + Flow Test]
 I --> J{Pass?}
 J -- No --> H
 J -- Yes --> K[Compare before/after]
 K --> L{Approve?}
 L -- No --> M[Revise / reject]
 L -- Yes --> N[Commit change + update flow]
```

**Accept criteria:** cancel does not mutate order; confirmation precedes mutation; loading disables repeated action; success/error states handled.

## Flow D — Review and export
1. Open review inbox filtered by node, screen and unresolved comments.
2. Click comment → preview exact state; highlight selected element or show stale anchor status.
3. Resolve comment only after accepting corresponding diff or explicitly dismissing.
4. Export ZIP with `src/`, assets, flow JSON, fixtures, README and lockfile; build locally.
5. GitHub PR requires explicit project authorization, selected branch and user confirmation.

## Flow E — Optional MCP external agent
1. Authenticated external client lists projects.
2. Reads graph, screen details and outstanding comments.
3. Proposes design changes via agent's own work (P1 read-only; mutating workflow deferred).
4. User reviews in workspace; no silent main-branch writes.

## Seed project: Order approval
Nodes: `login`, `dashboard`, `order-list`, `order-detail-pending`, `confirmation-modal`, `submitting`, `approved`, `error`, `permission-denied`.
Edges: login success, navigate list, open detail, click approve, confirm/cancel, request success/failure, retry, forbidden. Include responsive desktop/mobile viewport variants as metadata, not duplicate entire flow by default.

## UX behavior rules
- Flow map is navigator and behavioral model, not a wall of screenshots.
- Keep selected state and preview state synchronized; `Inspect` mode distinct from `Interact` mode.
- Comments attach to node+element and include snapshot/version context.
- Clarify only high uncertainty/high impact; low-risk edits can proceed as revertible proposals.
- Loading, empty, error, permissions, cancel paths appear in flow where relevant.
