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

export const ManagedProjectSlugSchema = z
  .string()
  .trim()
  .regex(/^[a-z0-9][a-z0-9_-]{0,62}$/);
export type ManagedProjectSlug = z.infer<typeof ManagedProjectSlugSchema>;

export const ProjectProvenanceSchema = z
  .enum(["scratch", "managed", "folder-backed", "folder_backed"])
  .transform((v) => (v === "folder_backed" ? "folder-backed" : v));

export const CreateProjectInputSchema = z.object({
  id: z.string().uuid().optional(),
  slug: z.string().trim().min(1).max(128),
  name: z.string().trim().min(1).max(128),
  product_brief: z.string().trim().min(1),
  workspace_root: z.string().trim().min(1).nullable().optional(),
  inherit_brand_from_project_id: z.string().uuid().nullable().optional(),
  managed_slug: ManagedProjectSlugSchema.nullable().optional(),
  provenance: ProjectProvenanceSchema.optional(),
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
  project_code: z.string().nullable().optional(),
  provenance: ProjectProvenanceSchema.optional(),
  trash_deleted_at: SafeJsIntegerSchema.nullable().optional(),
});
export type ProjectDto = z.infer<typeof ProjectDtoSchema>;

export const DeleteProjectResponseSchema = z.object({
  deleted: z.boolean(),
  id: z.string().uuid(),
  project_id: z.string().uuid().optional(),
  trash_deleted_at: SafeJsIntegerSchema.optional(),
  retention_days: z.number().int().nonnegative().optional(),
});
export type DeleteProjectResponse = z.infer<typeof DeleteProjectResponseSchema>;

export const RestoreProjectInputSchema = z.object({
  restore_mode: z.enum(["active", "rename_on_collision"]).optional(),
}).strict();
export type RestoreProjectInput = z.infer<typeof RestoreProjectInputSchema>;

export const RestoreProjectResponseSchema = z.object({
  restored: z.boolean(),
  project: ProjectDtoSchema,
  renamed_slug: z.string().nullable().optional(),
});
export type RestoreProjectResponse = z.infer<typeof RestoreProjectResponseSchema>;

export const FsBrowseEntrySchema = z.object({
  name: z.string(),
  path: z.string(),
  is_dir: z.boolean(),
  is_git_repo: z.boolean(),
});
export type FsBrowseEntry = z.infer<typeof FsBrowseEntrySchema>;

export const FsBrowseResponseSchema = z.object({
  current_path: z.string(),
  parent_path: z.string().nullable(),
  managed_root: z.string(),
  entries: z.array(FsBrowseEntrySchema),
});
export type FsBrowseResponse = z.infer<typeof FsBrowseResponseSchema>;

export const ProjectBoundaryDtoSchema = z.object({
  project_id: z.string().uuid(),
  boundary_type: z.string(),
  workspace_path: z.string(),
  config_dir: z.string(),
  is_git_repository: z.boolean(),
  promoted_at: SafeJsIntegerSchema.nullable().optional(),
  created_at: SafeJsIntegerSchema,
  updated_at: SafeJsIntegerSchema,
});
export type ProjectBoundaryDto = z.infer<typeof ProjectBoundaryDtoSchema>;

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

export const IssuePathTokenInputSchema = z
  .object({
    relative_path: z.string().trim().min(1).optional(),
    target_path: z.string().trim().min(1).optional(),
    principal_id: z.string().trim().min(1).max(128).nullable().optional(),
    ttl_seconds: z.number().int().positive().max(86400).nullable().optional(),
  })
  .refine((v) => Boolean(v.relative_path || v.target_path), {
    message: "relative_path or target_path is required",
  });
export type IssuePathTokenInput = z.infer<typeof IssuePathTokenInputSchema>;

export const PathTokenResponseSchema = z.object({
  token: z.string(),
  path_token: z.string().optional(),
  expires_at: SafeJsIntegerSchema,
  normalized_relative_path: z.string(),
  daemon_nonce: z.string().optional(),
  canonical_path: z.string().optional(),
  dev_ino: z.string().optional(),
  ancestor_dev_ino: z.string().optional(),
  target_exists: z.boolean().optional(),
});
export type PathTokenResponse = z.infer<typeof PathTokenResponseSchema>;

export const VerifyPathTokenInputSchema = z
  .object({
    token: z.string().trim().min(1),
    target_path: z.string().trim().min(1).optional(),
    target_directory: z.string().trim().min(1).optional(),
    expected_relative_path: z.string().trim().min(1).optional(),
    principal_id: z.string().trim().min(1).max(128).nullable().optional(),
  })
  .refine((v) => Boolean(v.target_path || v.target_directory || v.expected_relative_path), {
    message: "target_path, target_directory, or expected_relative_path is required",
  });
export type VerifyPathTokenInput = z.infer<typeof VerifyPathTokenInputSchema>;

export const VerifyPathTokenResponseSchema = z.object({
  valid: z.boolean(),
  normalized_relative_path: z.string(),
  canonical_path: z.string(),
  project_id: z.string().uuid(),
  principal_id: z.string(),
  daemon_nonce: z.string(),
  ancestor_dev_ino: z.string(),
  target_exists: z.boolean(),
});
export type VerifyPathTokenResponse = z.infer<typeof VerifyPathTokenResponseSchema>;

export const PromoteFolderInputSchema = z
  .object({
    target_directory: z.string().trim().min(1),
    token: z.string().trim().min(1).optional(),
    path_token: z.string().trim().min(1).optional(),
  })
  .refine((v) => Boolean(v.token || v.path_token), {
    message: "token or path_token is required",
  });
export type PromoteFolderInput = z.infer<typeof PromoteFolderInputSchema>;

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
