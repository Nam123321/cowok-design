import { z } from "zod";

export const SnapshotFileItemSchema = z.object({
  path: z.string().min(1),
  content: z.string(),
  sha256: z.string().optional(),
  sizeBytes: z.number().nonnegative().optional(),
});

export type SnapshotFileItem = z.infer<typeof SnapshotFileItemSchema>;

export const SnapshotTriggerSchema = z.enum([
  "manual",
  "auto_proposal_apply",
  "pre_revert_safety",
]);

export type SnapshotTrigger = z.infer<typeof SnapshotTriggerSchema>;

export const CreateSnapshotSchema = z.object({
  id: z.string().uuid().optional(),
  label: z.string().min(1, "Label is required"),
  description: z.string().default(""),
  versionTag: z.string().optional(),
  trigger: SnapshotTriggerSchema.default("manual"),
  files: z.array(SnapshotFileItemSchema).min(1, "At least one file is required"),
  metadata: z.record(z.string()).optional(),
});

export type CreateSnapshotInput = z.input<typeof CreateSnapshotSchema>;

export const VersionSnapshotDtoSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string(),
  projectId: z.string(),
  versionTag: z.string(),
  label: z.string(),
  description: z.string(),
  trigger: SnapshotTriggerSchema,
  treeSha256: z.string(),
  files: z.array(SnapshotFileItemSchema),
  metadata: z.record(z.string()).optional(),
  createdAt: z.string(),
});

export type VersionSnapshotResponse = z.infer<typeof VersionSnapshotDtoSchema>;

export const RevertSnapshotSchema = z.object({
  safetyLabel: z.string().optional(),
});

export type RevertSnapshotInput = z.infer<typeof RevertSnapshotSchema>;

export const RevertResultSchema = z.object({
  success: z.boolean(),
  revertedToSnapshotId: z.string(),
  safetySnapshotId: z.string(),
  restoredFilesCount: z.number().int().nonnegative(),
  timestamp: z.string(),
});

export type RevertResultResponse = z.infer<typeof RevertResultSchema>;

export const ExportZipSchema = z.object({
  includeFixtures: z.boolean().default(true),
  includeTokens: z.boolean().default(true),
  includeIcons: z.boolean().default(true),
  format: z.literal("zip").default("zip"),
});

export type ExportZipInput = z.infer<typeof ExportZipSchema>;

export const ExportZipResultSchema = z.object({
  fileName: z.string(),
  byteLength: z.number().nonnegative(),
  sha256: z.string(),
  filesIncluded: z.array(z.string()),
  readyToRun: z.boolean(),
  archiveBase64: z.string().optional(),
  downloadUrl: z.string().optional(),
  artifactPath: z.string().optional(),
});

export type ExportZipResponse = z.infer<typeof ExportZipResultSchema>;

export const GitHubPRSchema = z.object({
  branchName: z.string().min(1),
  prTitle: z.string().min(1),
  prBody: z.string().default(""),
  targetBranch: z.string().default("main"),
});

export type GitHubPRInput = z.infer<typeof GitHubPRSchema>;

export const GitHubPRResultSchema = z.object({
  prUrl: z.string().url(),
  prNumber: z.number().int().positive(),
  branchName: z.string(),
  status: z.enum(["created", "updated"]),
});

export type GitHubPRResponse = z.infer<typeof GitHubPRResultSchema>;
