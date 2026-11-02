# Implementation Roadmap & Backlog

**Planning hypothesis:** 4–6 skilled full-stack/platform engineers, 10–16 weeks for constrained MVP; not a contract. Engineering spikes may change plan drastically.

## Milestones
- **M0 Discovery/Spikes:** runtime, AST mapping, fixture-state, SDI evaluation; ADRs and narrowed architecture.
- **M1 Foundation:** monorepo, auth, project/template, DB migrations, job+preview bridge.
- **M2 Executable flows:** graph CRUD, serializer, state fixture adapter, selection and URL sync.
- **M3 Feedback and intent:** element comment anchors; attachments; SDI + clarification choices; change proposal.
- **M4 Agent/validation/export:** isolated code patch, compile/build, Playwright demo flows, diff approval and ZIP export.
- **M5 Beta hardening:** logging/quotas/cost metrics, accessibility, security tests, small user study; optional GitHub PR and read-only MCP.

## Epics and stories
| ID | Epic | Requirement | Depend on | DoD / test |
|---|---|---|---|---|
| SPIKE-01 | Runtime | Compare sandbox options | none | ADR, measured benchmark, legal notes |
| SPIKE-02 | Mapping | JSX DOM element→source mapping | none | 20-case test and stale fallback |
| SPIKE-03 | Flow fixture | State preview hydration | none | pending, submitting, success, error demo |
| FND-01 | Monorepo | typed packages and CI | spikes | lint/typecheck/test/build green |
| FND-02 | Project API | create/load template project | FND-01 | tenant RBAC and project isolation tests |
| FND-03 | Preview | isolated preview and iframe bridge | SPIKE-01 | cold/warm preview + permission fail states |
| FLOW-01 | Graph editor | CRUD state nodes, transitions | FND-02 | schema validation, persistence |
| FLOW-02 | Preview sync | click node → route+fixture | FLOW-01,FND-03,SPIKE-03 | 9 demo nodes visitable or explicit error |
| REVIEW-01 | Comments | node and element comments | FLOW-02,SPIKE-02 | user+anchor+snapshot persisted |
| REVIEW-02 | References | upload image/URL context | REVIEW-01 | file validation, signed links |
| INT-01 | SDI | prompt→typed intent | FND-02 | schema+provenance, contract tests |
| INT-02 | Clarify | 2–3 alternatives for ambiguity | INT-01 | decision saved, skip low-risk |
| CODE-01 | Proposal | isolated patch from SDI | INT-01,FND-03 | inspectable unified diff, no main write |
| CODE-02 | Gate | run build/typecheck/playwright | CODE-01 | failures block apply, logs available |
| CODE-03 | Approve | accept/reject/revert | CODE-02 | concurrency conflict test |
| EXP-01 | Export | zip runnable React source + flow spec | FLOW-02,CODE-03 | fresh machine build smoke |
| MCP-01 | Read-only MCP | expose list/read flow/screens | FND-02 | scoped auth, tool tests |
| GIT-01 | GitHub PR | branch+PR explicit approval | CODE-03 | permissions and revocation tests |
| OBS-01 | Usage metrics | outcome/cost traceability | CODE-01 | correlation IDs, redaction and dashboard |

## First 4 executable vertical slices
**Slice 1:** static seed template → click graph node → preview correct route+state; automated test.
**Slice 2:** select button → comment with screenshot → saved anchor → revisit.
**Slice 3:** request “confirm before approve” → SDI choices → selected modal variant → proposal diff → build/test → approve.
**Slice 4:** export repo → clean checkout / unzip → pnpm install → build and run same flow.

## DoD each slice
- Product acceptance and error states described.
- API/UI integration tests; add Playwright where interaction matters.
- Structured logs with run/project identifiers; no secrets.
- Build + typecheck green; PR references spec IDs and acceptance checks.
- Report known limitations, risks and next ticket.

## Stop/go gates
- Abort broad visual editing if element mapping <90% on constrained fixtures.
- Revisit architecture if export build success <95% on internal scenario set.
- Do not expand to arbitrary repo import until constrained workflow earns measurable advantage.
