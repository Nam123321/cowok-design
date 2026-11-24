---
type: I-Wish Data & Contract Spec
story_id: "05.6"
title: "Data & Contract Specification: Story 05.6 (BrandProposalV1, PopularComponentAudit, FE Data Contract & Heritage Pack)"
status: draft
data_decision_records:
  - model: BrandProposalSnapshot
    action: NEW_AGGREGATE
    bounded_context: html-forge
    justification: "Persists LLM-generated BrandProposalV1 JSON (rationale, inspirationRefs, user-requested themeModes tokens, component overrides) for <5ms multi-page & gallery rendering"
  - model: HeritageDesignPack
    action: NEW_AGGREGATE
    bounded_context: spec-registry
    justification: "Stores reusable Brand ID, Component Registry, and Presentation Theme Packs across projects with tenant isolation"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "./story.md"
---

# Data & Contract Specification — Story 05.6

## 1. User-Driven `themeModes` & `BrandProposalV1` Contract

```typescript
import { z } from "zod";

export const ThemeModeIdSchema = z
  .string()
  .trim()
  .regex(/^[a-z0-9][a-z0-9-]{0,31}$/); // e.g. "light", "dark", "warm", "dim", "night", "high-contrast", or custom brand mode

export const BrandModeTokensSchema = z.object({
  modeId: ThemeModeIdSchema,
  label: z.string().min(1).max(48),
  tokens: z.record(z.string().regex(/^--[a-zA-Z0-9-]+$/), z.string().min(1).max(128)),
  wcagAaVerified: z.boolean(),
});

export const BrandProposalV1Schema = z.object({
  id: z.string().min(1).max(64),
  brandName: z.string().min(1).max(80),
  rationale: z.string().min(1).max(600),
  inspirationRefs: z.array(z.string().min(1).max(120)).max(6),
  themeModes: z.array(BrandModeTokensSchema).min(1).max(6), // strictly driven by user request (no forced light+dark default)
  fontPairing: z.object({ heading: z.string(), body: z.string(), mono: z.string() }),
  radiusScale: z.enum(["sharp", "balanced", "soft", "pill"]),
  density: z.enum(["airy", "comfortable", "compact"]),
  componentStyleOverrides: z.record(z.string(), z.record(z.string(), z.string())).default({}),
});

export const PopularComponentAuditItemSchema = z.object({
  componentKind: z.string().min(1).max(64),
  category: z.enum(["feedback", "overlay", "navigation", "input", "display", "deck"]),
  status: z.enum(["detected_in_sketch", "suggested_contextual", "optional", "not_needed"]),
  reason: z.string().max(240).optional(),
  selected: z.boolean(),
});

export const FeDataContractDraftSchema = z.object({
  projectId: z.string().uuid(),
  screenId: z.string().min(1).max(128),
  boundaryNotice: z.literal("FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY"),
  viewModelFields: z.array(
    z.object({
      ffId: z.string(),
      fieldName: z.string(),
      inferredType: z.enum(["string", "number", "currency", "boolean", "date", "enum", "array", "chart_series"]),
      sampleValue: z.unknown(),
    })
  ),
  actionInputs: z.array(
    z.object({
      actionId: z.string(),
      triggerFfId: z.string(),
      requiredFields: z.array(z.string()),
    })
  ),
  subStateFixtures: z.record(z.enum(["DEFAULT", "EMPTY", "LOADING", "ERROR"]), z.record(z.unknown())),
});
```

## 2. Persistence Schema (`V25__story_05_6_brand_proposals_and_heritage_packs.sql` — Composite Tenant Isolation, Sequential After `V24`)

```prisma
model BrandProposalSnapshot {
  id              String @id @default(uuid())
  tenantId        String @map("tenant_id")
  projectId       String @map("project_id")
  proposalId      String @map("proposal_id")
  proposalJson    String @map("proposal_json")
  themeModesJson  String @map("theme_modes_json")
  isSelected      Boolean @default(false) @map("is_selected")
  createdAt       BigInt @map("created_at")

  @@index([tenantId, projectId])
  @@map("brand_proposal_snapshots")
}

model HeritageDesignPack {
  id                  String @id @default(uuid())
  tenantId            String @map("tenant_id")
  name                String
  sourceProjectId     String @map("source_project_id")
  brandIdentityJson   String @map("brand_identity_json")
  componentRegistryJson String @map("component_registry_json")
  presentationThemeJson String @map("presentation_theme_json")
  themeModesJson      String @map("theme_modes_json")
  createdAt           BigInt @map("created_at")

  @@index([tenantId, sourceProjectId])
  @@map("heritage_design_packs")
}

model HiddenSpecDocument {
  id              String @id @default(uuid())
  tenantId        String @map("tenant_id")
  projectId       String @map("project_id")
  specKey         String @map("spec_key")
  title           String
  contentMarkdown String @map("content_markdown")
  contentSha256   String @map("content_sha256")
  revision        Int    @default(1)
  updatedAt       BigInt @map("updated_at")

  @@unique([tenantId, projectId, specKey])
  @@index([tenantId, projectId])
  @@map("hidden_spec_documents")
}
```

### Additional Verified Route (Stage 4 Polish)
- `POST /api/v1/cognitive/specs/reveal` (`RevealSpecsFolderRequestSchema` -> `RevealSpecsFolderResponseSchema`): Strictly enforces `roles: ["editor", "admin"]`, `MAX_BODY_BYTES = 4_096`, `parseJsonBodyStrict` (`rawBinary: true`), and `fs_jail::check_child_non_symlink_in_dir_fd` + `symlink_metadata` before invoking OS file manager on `.cowok-design/specs/`.
