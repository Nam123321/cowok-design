# Tech Stack Decision Research — snapshot 2026-10-26

## Recommendation matrix
| Problem | Default candidate | Alternatives | Why / caveat | Test |
|---|---|---|---|---|
| Flow editor | `@xyflow/react` | Cytoscape.js, custom SVG | Node/edge UX and custom node ecosystem; verify current API | render 50 nodes; keyboard, save, load |
| Web UI | React + TypeScript + Vite | Next.js | Low-friction runtime and template; product server can be separate | generated export builds locally |
| UI tokens | Tailwind + CSS vars + Radix/shadcn | Vanilla CSS, MUI | Consistency and inspectability | responsive component patch |
| Code editor | Monaco | CodeMirror | Familiar source diff editor, larger bundle | diff/read mode |
| Source transforms | TypeScript Compiler API + Recast/Babel | tree-sitter | Semantics vs formatting preservation; evaluate mapping | 20 edit fixtures, minimal diffs |
| Preview runtime | isolated cloud containers (default to spike) | WebContainers | Isolation and repeatability vs cost; WebContainers licensing/browser constraints | preview launch, TS install, quotas, cross origin |
| Browser validation | Playwright | Puppeteer | Flow interaction and screenshots | 8-edge flow replay |
| API | Node TS + Fastify/Hono | NestJS | Typed endpoints, limited complexity | auth, streaming, job run |
| DB | PostgreSQL | SQLite for local-only | relational audit/tenancy/graph metadata | multi-tenant access test |
| Asset store | S3-compatible | database blobs | media refs and snapshots; scan and signed URLs | upload/ownership rules |
| Job management | durable queue + isolated worker | inline POC job | long-running build/run; idempotency | cancellation/timeouts |
| Agent SDK | Provider adapters + AI SDK | direct provider SDK | Avoid model hard coupling; controlled structured responses | 2 providers same SDI suite |
| MCP | Official TS SDK v2 | bespoke JSON API only | Current stable SDK implements 2026-07-28 spec | auth and read-only tool fixture |
| State modeling | JSON graph + Zod, optional XState | XState everywhere | Keep authoring accessible, introduce machine semantics as needed | deterministic fixture hydration |

## Critical primary-source facts
- MCP TS SDK **v2** is stable, spec 2026-07-28; package names `@modelcontextprotocol/server`, `@modelcontextprotocol/client` ([M1], [M2]). Pin actual versions during implementation.
- WebContainers offer Node runtime in the browser; commercial production use has licensing requirements and usage conditions. Contact vendor / legal before choosing it ([W1]).
- Onlook's published architecture uses web container, iframe, code indexing, instrumentation, and source updates, demonstrating feasibility but not removing integration complexity ([O1]).
- Figma Make real-code beta is limited and includes visual edits, annotations, branches/PR; feature availability is plan/platform dependent ([F1], [F2]).

## Engineering spikes — timeboxes, pass/fail
**Spike A: runtime (2–4 engineering days)**
- Build `template-react` in two candidates: local-isolated remote sandbox and WebContainers.
- Measure cold/warm start, npm compatibility, preview attach, file update latency, screenshot, per-session resources, license, data security.
- Decision: select commercially viable mechanism with passing test suite; otherwise reduce MVP to prebuilt preview templates.

**Spike B: element mapping (3–5 engineering days)**
- Annotate JSX button and nested shared component; click DOM; reliably map to source file/AST node.
- Test after formatting, refactor, list rendering and conditional rendering.
- Failure case explicitly marked stale. **Do not ship unrestricted drag/drop on unproven mapping.**

**Spike C: deterministic flow fixtures (2–4 engineering days)**
- Map route+fixture into pending/loading/error states, replay approval transition in browser, assert UI and request mocks.

**Spike D: Intent Engine evaluation (3–5 engineering days)**
- 30 ambiguous prompts; record classification and clarification prompts, rate utility and false-clarification burden.

## Library and license due diligence
Review package/license for XYFlow, Monaco, Tailwind/shadcn, preview runtime, icon libraries and derived code. Protect generated code ownership/terms. Avoid copying competitor implementation wholesale without legal/license review.

## References
[M1] https://ts.sdk.modelcontextprotocol.io/v2/  
[M2] https://github.com/modelcontextprotocol/typescript-sdk  
[W1] https://developer.stackblitz.com/guides/user-guide/general-faqs  
[O1] https://github.com/onlook-dev/onlook  
[F1] https://www.figma.com/blog/figma-make-now-on-your-local-code/  
[F2] https://help.figma.com/hc/en-us/articles/40775535020695-Make-in-your-local-codebase
