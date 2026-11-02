---
epic_id: "Epic-01"
value_stream: "VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine"
layer: api
status: draft
cs: 6
endpoints_count: 5
---

# Layer-Epic-API: Epic 01: Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry

## 1. API Architecture Overview & Protocols
Defines the loopback-bound (`127.0.0.1:4170` with dynamic port fallback via `.uxui-os/daemon.lock`) Axum HTTP + WebSocket control plane in `crates/harness-daemon` for `Epic-01`. All request/response payloads use deterministic Rust `serde` structs exported to TypeScript via `ts-rs` and validated at runtime with Zod schemas in `@uxui-os/shared-types`.

## 2. Endpoint Inventory Table
Lists all endpoints bound to entities declared in `layer-epic-db.md` (`Project`, `ProjectSpec`, `SpecRevision`):

| Method | URI / Procedure Name | Auth / Permission | Bound Model(s) | Status | Description |
|--------|----------------------|-------------------|----------------|--------|-------------|
| GET | `/api/v1/health` | `Loopback Local` | `Project` | New | Daemon health, SQLite WAL status, and active port discovery |
| POST | `/api/v1/projects` | `Loopback Local` | `Project`, `ProjectSpec` | New | Bootstrap a new project from Product Brief or seed repo with optional `inherit_brand_from_project_id` |
| GET | `/api/v1/projects/:id/specs` | `Loopback Local` | `ProjectSpec`, `SpecRevision` | New | List all 9 Foundational Specs with SHA-256 hashes and provenance counts (`CONFIRMED`/`INFERRED`/`MISSING`) |
| PUT | `/api/v1/projects/:id/specs/:spec_key` | `Loopback Local` | `ProjectSpec`, `SpecRevision` | New | Atomically update one of the 9 Foundational Specs with mandatory `expected_sha256` `StaleFile` OCC check |
| POST | `/api/v1/projects/:id/specs/synthesize` | `Loopback Local` | `Project`, `ProjectSpec`, `SpecRevision` | New | Synthesize all 9 Foundational Project Context Specs from Product Brief with provenance tagging |

## 3. Request & Response Schemas

```typescript
import { z } from 'zod';

export const CanonicalSpecKeySchema = z.enum([
  '01-product-overview',
  '02-user-personas-and-jtbds',
  '03-information-architecture',
  '04-design-direction',
  '05-component-inventory',
  '06-interaction-patterns',
  '07-responsive-and-a11y',
  '08-brand-guidelines',
  '09-data-and-copy',
]);

export const ProvenanceStatusSchema = z.enum(['CONFIRMED', 'INFERRED', 'MISSING']);

export const CreateProjectInputSchema = z.object({
  slug: z.string().min(2).max(80),
  name: z.string().min(2).max(120),
  workspaceRoot: z.string().min(1),
  productBrief: z.string().min(1),
  inheritBrandFromProjectId: z.string().uuid().optional(),
});

export const UpdateProjectSpecInputSchema = z.object({
  specKey: CanonicalSpecKeySchema,
  markdownContent: z.string().min(1),
  expectedSha256: z.string().length(64).optional(),
  actorKind: z.enum(['HUMAN', 'COWOK_ORCHESTRATOR', 'ADVISOR', 'EXECUTION_BOT']),
  actorId: z.string().min(1),
});

export interface ProjectSpecResponse {
  id: string;
  projectId: string;
  specKey: z.infer<typeof CanonicalSpecKeySchema>;
  version: number;
  contentSha256: string;
  summaryDigest: string;
  provenanceSummary: {
    confirmedCount: number;
    inferredCount: number;
    missingCount: number;
  };
  sourceProjectId?: string | null;
  sourceRevisionSha256?: string | null;
  updatedAt: string;
}
```

## 4. Error Handling & HTTP Status Codes

| Status Code | Error Code | Trigger Condition |
|-------------|------------|-------------------|
| 400 Bad Request | `VALIDATION_ERROR` | Malformed JSON payload or missing required field |
| 404 Not Found | `PROJECT_NOT_FOUND` | Target `project_id` or `inherit_brand_from_project_id` does not exist in `PROJECTS` |
| 409 Conflict | `STALE_FILE` | `expected_sha256` does not match current `PROJECT_SPECS.content_sha256` (`EC-E01-02`) |
| 422 Unprocessable Entity | `INVALID_SPEC_KEY` | `spec_key` is outside the 9-variant `CanonicalSpecKey` enum (`EC-E01-03`) or `BRIEF_EMPTY` |
| 503 Service Unavailable | `SQLITE_BUSY_TIMEOUT` | SQLite WAL lock contention exceeded `busy_timeout = 5000ms` (`EC-E01-05`) |

## 5. Cross-Epic Contract & Integration Dependencies
- **Upstream Dependencies**: Root Substrate Anchor (`In-Degree = 0`).
- **Bound Data Models**: `Project`, `ProjectSpec`, `SpecRevision` from `layer-epic-db.md`.
- **Downstream Consumers**: `SpecVaultStatusPanel` in `layer-epic-fe.md`, `Epic-02` (`crates/constraint-engine`), `Epic-03` (`crates/flow-graph`), and `Epic-09` (`cowok` Orchestrator & `advisor` Meta-Agent).

## 6. Canonical Client SDK Interface Contract
- **Module Path**: `packages/ui-shell/src/api/project-specs.ts`
- **Canonical Namespace Export**:
  ```typescript
  export const apiProjectSpecs = {
    getHealth: () => apiClient.get('/api/v1/health', { timeoutMs: 3000 }),
    createProject: (input: CreateProjectInput) => apiClient.post('/api/v1/projects', input, { timeoutMs: 10000 }),
    listSpecs: (projectId: string) => apiClient.get(`/api/v1/projects/${projectId}/specs`, { timeoutMs: 5000 }),
    updateSpec: (projectId: string, specKey: CanonicalSpecKey, input: UpdateProjectSpecInput) =>
      apiClient.put(`/api/v1/projects/${projectId}/specs/${specKey}`, input, { timeoutMs: 5000 }),
    synthesizeSpecs: (projectId: string, brief: string) =>
      apiClient.post(`/api/v1/projects/${projectId}/specs/synthesize`, { brief }, { timeoutMs: 15000 }),
  };
  ```
