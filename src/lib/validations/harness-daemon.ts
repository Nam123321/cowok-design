// @story 01.3
// @implements AC-01, AC-02, AC-03, AC-05, AC-06
import { z } from "zod";

export const CanonicalSpecKeySchema = z.enum([
  "01-product-overview.md",
  "02-database-spec.md",
  "03-ui-ux-spec.md",
  "04-architecture.md",
  "05-epics-and-stories.md",
  "06-cross-dependencies.md",
  "07-api-guidelines.md",
  "08-security-tenancy.md",
  "09-data-and-copy.md",
]);
export type CanonicalSpecKey = z.infer<typeof CanonicalSpecKeySchema>;

export const ProvenanceStatusSchema = z.enum(["CONFIRMED", "INFERRED", "MISSING"]);
export type ProvenanceStatus = z.infer<typeof ProvenanceStatusSchema>;

export const SafeJsIntegerSchema = z
  .number()
  .int()
  .min(-9_007_199_254_740_991)
  .max(9_007_199_254_740_991);

export const CreateProjectInputSchema = z.object({
  id: z.string().uuid().optional(),
  slug: z.string().trim().min(1).max(128),
  name: z.string().trim().min(1).max(128),
  product_brief: z.string().trim().min(1),
  workspace_root: z.string().trim().min(1).nullable().optional(),
  inherit_brand_from_project_id: z.string().uuid().nullable().optional(),
});
export type CreateProjectInput = z.infer<typeof CreateProjectInputSchema>;

export const ProjectDtoSchema = z.object({
  id: z.string().uuid(),
  slug: z.string(),
  name: z.string(),
  product_brief: z.string(),
  workspace_root: z.string().nullable().optional(),
  inherit_brand_from_project_id: z.string().uuid().nullable().optional(),
  created_at: SafeJsIntegerSchema,
  updated_at: SafeJsIntegerSchema,
});
export type ProjectDto = z.infer<typeof ProjectDtoSchema>;

export const SpecItemWithDriftSchema = z.object({
  id: z.string().uuid(),
  project_id: z.string().uuid(),
  spec_key: CanonicalSpecKeySchema,
  file_path: z.string(),
  status: ProvenanceStatusSchema,
  version: z.number().int().nonnegative(),
  content_sha256: z.string(),
  summary_digest: z.string(),
  provenance_summary_json: z.string(),
  source_project_id: z.string().uuid().nullable().optional(),
  source_revision_sha256: z.string().nullable().optional(),
  upstream_outdated: z.boolean(),
  markdown_content: z.string(),
  updated_at: SafeJsIntegerSchema,
});
export type SpecItemWithDrift = z.infer<typeof SpecItemWithDriftSchema>;

export const ProjectSpecsListResponseSchema = z.object({
  project_id: z.string().uuid(),
  upstream_brand_drift_detected: z.boolean(),
  specs: z.array(SpecItemWithDriftSchema),
});
export type ProjectSpecsListResponse = z.infer<typeof ProjectSpecsListResponseSchema>;

export const PutSpecInputSchema = z.object({
  markdown_content: z.string(),
  expected_sha256: z.string().min(1).nullable().optional(),
  status: ProvenanceStatusSchema.nullable().optional(),
  change_summary: z.string().trim().min(1).max(500).nullable().optional(),
  source_project_id: z.string().uuid().nullable().optional(),
  source_revision_sha256: z.string().min(1).nullable().optional(),
});
export type PutSpecInput = z.infer<typeof PutSpecInputSchema>;

export const DeleteSpecResponseSchema = z.object({
  deleted: z.boolean(),
  spec_key: CanonicalSpecKeySchema,
});
export type DeleteSpecResponse = z.infer<typeof DeleteSpecResponseSchema>;

export const SynthesizeSpecsInputSchema = z.object({
  product_brief: z.string().trim().min(1).nullable().optional(),
  seed_repo_path: z.string().trim().min(1).nullable().optional(),
  inherit_brand_from_project_id: z.string().uuid().nullable().optional(),
  actor_id: z.string().trim().min(1).max(128).nullable().optional(),
});
export type SynthesizeSpecsInput = z.infer<typeof SynthesizeSpecsInputSchema>;

export const SynthesizedSpecItemDtoSchema = z.object({
  spec_key: CanonicalSpecKeySchema,
  status: ProvenanceStatusSchema,
  version: z.number().int().nonnegative(),
  content_sha256: z.string(),
  confirmed_count: z.number().int().nonnegative(),
  inferred_count: z.number().int().nonnegative(),
  missing_count: z.number().int().nonnegative(),
  source_project_id: z.string().uuid().nullable().optional(),
  source_revision_sha256: z.string().nullable().optional(),
});

export const SynthesizeSpecsResponseSchema = z.object({
  project_id: z.string().uuid(),
  sparse_brief_detected: z.boolean(),
  total_confirmed_count: z.number().int().nonnegative(),
  total_inferred_count: z.number().int().nonnegative(),
  total_missing_count: z.number().int().nonnegative(),
  open_questions: z.array(z.string()),
  specs: z.array(SynthesizedSpecItemDtoSchema),
});
export type SynthesizeSpecsResponse = z.infer<typeof SynthesizeSpecsResponseSchema>;

export const TaskHorizonSchema = z.enum(["short", "mid", "long"]);
export const DepartmentSchema = z.enum(["product_ui", "marketing_deck", "executive_report"]);
export const PortalSchema = z.enum([
  "super_admin",
  "tenant_admin",
  "sales_app",
  "marketing_web",
  "developer_portal",
  "presentation_studio",
]);

export const ContextSliceInputSchema = z.object({
  task_input: z.string().trim().min(1),
  override_horizon: TaskHorizonSchema.nullable().optional(),
  department: DepartmentSchema,
  portal: PortalSchema,
  affected_artifacts: z.array(z.string()).default([]),
});
export type ContextSliceInput = z.infer<typeof ContextSliceInputSchema>;

export const ChatSessionDtoSchema = z.object({
  id: z.string().uuid(),
  project_id: z.string().uuid(),
  title: z.string(),
  created_at: SafeJsIntegerSchema,
  updated_at: SafeJsIntegerSchema,
});
export type ChatSessionDto = z.infer<typeof ChatSessionDtoSchema>;

export const CreateChatSessionInputSchema = z.object({
  id: z.string().uuid().optional(),
  title: z.string().trim().min(1).max(200),
});
export type CreateChatSessionInput = z.infer<typeof CreateChatSessionInputSchema>;

export const UpdateChatSessionInputSchema = z.object({
  title: z.string().trim().min(1).max(200),
});
export type UpdateChatSessionInput = z.infer<typeof UpdateChatSessionInputSchema>;

export const IssuePathTokenInputSchema = z.object({
  relative_path: z.string().trim().min(1),
  principal_id: z.string().trim().min(1).max(128).nullable().optional(),
  ttl_seconds: z.number().int().positive().max(86400).nullable().optional(),
});
export type IssuePathTokenInput = z.infer<typeof IssuePathTokenInputSchema>;

export const PathTokenResponseSchema = z.object({
  token: z.string(),
  expires_at: SafeJsIntegerSchema,
  normalized_relative_path: z.string(),
});
export type PathTokenResponse = z.infer<typeof PathTokenResponseSchema>;

export const LocalSessionBootstrapInputSchema = z.object({
  tenantId: z.string().trim().min(1).max(128).optional(),
  userId: z.string().trim().min(1).max(128).optional(),
  role: z.enum(["viewer", "editor", "admin"]).default("editor"),
});
export type LocalSessionBootstrapInput = z.infer<typeof LocalSessionBootstrapInputSchema>;

export const LocalSessionResponseSchema = z.object({
  sessionToken: z.string().min(1),
  tenantId: z.string(),
  userId: z.string(),
  role: z.enum(["viewer", "editor", "admin"]),
  expiresAt: z.number().optional(),
});
export type LocalSessionResponse = z.infer<typeof LocalSessionResponseSchema>;
